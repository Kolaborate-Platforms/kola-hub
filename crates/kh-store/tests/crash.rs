//! Kill during write. The test binary starts itself as a worker that saves, trashes and stores blobs
//! in a loop, kills it many times, and checks nothing was lost, doubled or half written.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use kh_store::Project;
use kh_store::atomic::atomic_write;
use sha2::{Digest, Sha256};

const DIR_VAR: &str = "KH_CRASH_DIR";
const WORKER: &str = "crash_child_worker";
const KILL_ROUNDS: u64 = 25;
const MIN_WAIT_MS: u64 = 120;
const WAIT_SPREAD_MS: u64 = 380;
const BLOB_LINES: usize = 600;
const ORIGINAL: &str = "orig.csv";
const ORIGINAL_TEXT: &str = "a,b\n1,2\n3,4\n";
const END_MARK: &str = "END\n";
const MIN_TOTAL_BLOBS: usize = 15;
const WORK: &str = ".kola-hub";

fn body(tag: &str) -> String {
    let mut text = String::new();
    for n in 0..BLOB_LINES {
        text.push_str(&format!("{tag}-{n}\n"));
    }
    text.push_str(END_MARK);
    text
}

/// Runs only when the parent test starts this binary with the folder in the environment.
#[test]
fn crash_child_worker() {
    let Ok(dir) = std::env::var(DIR_VAR) else {
        return;
    };
    let root = Path::new(&dir);
    let project = Project::open_or_create(root).expect("worker open");
    let original = project.register_file(ORIGINAL).expect("register");
    let run = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let mut previous = None;
    for i in 0u64.. {
        let tag = format!("{run}-{i}");
        project
            .store_blob(&original, &format!("b{tag}"), body(&tag).as_bytes())
            .expect("blob");
        project
            .save_beside(&original, "cleaned", |f| f.write_all(body(&tag).as_bytes()))
            .expect("save");
        let name = format!("t-{run}-{i}.txt");
        atomic_write(&root.join(&name), format!("{name}\n").as_bytes()).expect("create");
        let id = project.register_file(&name).expect("register");
        let trashed = project.trash_file(&id).expect("trash");
        if i % 3 == 2
            && let Some(old) = previous.take()
        {
            project.restore_from_trash(&old).expect("restore");
        }
        previous = Some(trashed);
    }
}

fn spawn_worker(dir: &Path) -> std::process::Child {
    Command::new(std::env::current_exe().expect("exe"))
        .args([WORKER, "--exact", "--nocapture", "--test-threads=1"])
        .env(DIR_VAR, dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn worker")
}

fn sha(path: &Path) -> String {
    let mut h = Sha256::new();
    h.update(fs::read(path).expect("read"));
    format!("{:x?}", h.finalize())
}

/// Every `t-<run>-<i>.txt` identity found in the project folder or in the trash.
fn identities(root: &Path) -> Vec<String> {
    let mut found = Vec::new();
    for entry in fs::read_dir(root).expect("root") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name.starts_with("t-") && name.ends_with(".txt") {
            found.push(name_check(&root.join(&name), &name));
        }
    }
    for entry in fs::read_dir(root.join(WORK).join("trash")).expect("trash") {
        let payload = entry.expect("entry").path().join("payload");
        if payload.exists() {
            let text = fs::read_to_string(&payload).expect("payload");
            found.push(text.trim_end().to_owned());
        }
    }
    found
}

fn name_check(path: &Path, name: &str) -> String {
    let text = fs::read_to_string(path).expect("content");
    assert_eq!(
        text,
        format!("{name}\n"),
        "a half written file appeared under its final name"
    );
    name.to_owned()
}

fn check_after_kill(root: &Path) {
    assert_eq!(
        fs::read_to_string(root.join(ORIGINAL)).expect("orig"),
        ORIGINAL_TEXT,
        "original changed"
    );
    let project = Project::open_or_create(root).expect("database must open after a kill");
    let ok: String = project
        .connection()
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .expect("check");
    assert_eq!(ok, "ok");
    assert!(
        fs::read_dir(root.join(WORK).join("blobs/tmp"))
            .expect("tmp")
            .next()
            .is_none(),
        "temporary blobs left behind"
    );

    for id in project.blobs().list().expect("list") {
        project
            .blobs()
            .verify(&id)
            .expect("every stored blob matches its name");
    }
    let mut stmt = project
        .connection()
        .prepare("SELECT blob FROM blob_refs")
        .expect("stmt");
    let refs: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .expect("rows")
        .map(|r| r.expect("row"))
        .collect();
    for hash in refs {
        let id = kh_store::BlobId::parse(&hash).expect("hash");
        project
            .blobs()
            .verify(&id)
            .expect("a record never points at a missing or damaged blob");
    }

    for entry in fs::read_dir(root).expect("root") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name.starts_with("orig (cleaned") {
            let text = fs::read_to_string(root.join(&name)).expect("saved copy");
            assert!(text.ends_with(END_MARK), "{name} is half written");
        }
    }

    let ids = identities(root);
    let unique: BTreeSet<_> = ids.iter().cloned().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "a file exists twice, so a move was half done"
    );
    assert_eq!(
        project.list_trash().expect("trash").len(),
        fs::read_dir(root.join(WORK).join("trash"))
            .expect("dir")
            .count()
    );
    assert_no_gaps(&unique);
}

/// Each run creates files 0, 1, 2, ... in order and deletes none, so a gap means something was lost.
fn assert_no_gaps(names: &BTreeSet<String>) {
    let mut by_run: std::collections::BTreeMap<String, BTreeSet<u64>> = Default::default();
    for n in names {
        let core = n.trim_start_matches("t-").trim_end_matches(".txt");
        let (run, i) = core.rsplit_once('-').expect("name shape");
        by_run
            .entry(run.to_owned())
            .or_default()
            .insert(i.parse().expect("index"));
    }
    for (run, seen) in by_run {
        let top = *seen.last().expect("one");
        let want: BTreeSet<u64> = (0..=top).collect();
        assert_eq!(seen, want, "run {run} lost a file");
    }
}

#[test]
fn killing_the_writer_many_times_loses_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    fs::write(root.join(ORIGINAL), ORIGINAL_TEXT).expect("original");
    let before = sha(&root.join(ORIGINAL));
    Project::open_or_create(root).expect("first open");

    for round in 0..KILL_ROUNDS {
        let mut child = spawn_worker(root);
        let wait = MIN_WAIT_MS + (round * 97) % WAIT_SPREAD_MS;
        std::thread::sleep(Duration::from_millis(wait));
        child.kill().expect("kill");
        child.wait().expect("reap");
        check_after_kill(root);
    }

    assert_eq!(sha(&root.join(ORIGINAL)), before);
    let project = Project::open_or_create(root).expect("final open");
    let total = project.blobs().list().expect("list").len();
    let strays = fs::read_dir(root)
        .expect("root")
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".craft_tmp"))
        .count();
    let trashed = project.list_trash().expect("trash").len();
    eprintln!("blobs {total}, in trash {trashed}, stray stage files {strays}");
    assert!(
        total >= MIN_TOTAL_BLOBS,
        "the worker did too little work to prove anything: {total} blobs"
    );
}
