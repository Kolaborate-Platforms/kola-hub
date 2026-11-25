//! Recently deleted: a move into `.kola-hub/trash/<id>/payload` guarded by a journal entry.
//!
//! The entry is written before the move and rewritten after it, so a crash at any point leaves
//! either the original in place or the payload in the trash, never neither and never a half move.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::atomic::atomic_write;
use crate::budget::tree_size;
use crate::error::{Result, StoreError};
use crate::names::{clean_relative, opaque_id, resolve_new};
use crate::project::{FileId, Project, TRASH_DIR, now_nanos, now_unix, rel_text};
use crate::save::numbered_name;

const ENTRY_FILE: &str = "entry.json";
const PAYLOAD: &str = "payload";
const DELETING_PREFIX: &str = ".deleting-";
const ID_CHARS: usize = 32;
const MAX_ENTRY_BYTES: u64 = 64 * 1024;
const SECONDS_PER_DAY: i64 = 86_400;
const RESTORE_LABEL: &str = "restored";
const MAX_NAME_TRIES: u32 = 1000;

/// Opaque id of one trash entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrashId(String);

impl TrashId {
    /// Accepts only ids this crate makes, which keeps path tricks out.
    pub fn parse(text: &str) -> Result<Self> {
        let ok = text.len() == ID_CHARS
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(StoreError::BadName)
        }
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One line of the "Recently deleted" screen. It carries a display name, never a path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    /// Id to restore or delete by.
    pub id: TrashId,
    /// The file's name.
    pub name: String,
    /// When it was moved to the trash, in seconds since 1970.
    pub deleted_at: i64,
    /// Size in bytes.
    pub size_bytes: u64,
    /// True when the journal was damaged and the item was found without its original name.
    pub recovered: bool,
}

/// What opening a project repaired.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    /// Moves that had finished but were not yet written down, now recorded.
    pub finished_moves: u32,
    /// Moves that had not happened, or restores that had not happened, now cleared.
    pub rolled_back: u32,
    /// Items found with a damaged journal entry and given a new one.
    pub recovered: u32,
    /// Journal entries with nothing left to show for them, removed.
    pub dropped: u32,
}

impl RepairReport {
    /// True when anything needed fixing.
    pub fn any(&self) -> bool {
        *self != Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum State {
    Moving,
    Trashed,
    Restoring,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    original_rel: String,
    deleted_at: i64,
    size_bytes: u64,
    state: State,
    #[serde(default)]
    recovered: bool,
    /// Where an unfinished restore was putting the file.
    #[serde(default)]
    restore_rel: Option<String>,
}

impl Project {
    fn trash_root(&self) -> PathBuf {
        self.work.join(TRASH_DIR)
    }

    /// Moves a file to Recently deleted. The file keeps its id for a later restore.
    pub fn trash_file(&self, id: &FileId) -> Result<TrashId> {
        let rel = self.rel_of(id)?;
        let full = resolve_new(&self.root, Path::new(&rel))?;
        fs::symlink_metadata(&full).map_err(|_| StoreError::NotFound)?;
        let tid = TrashId(new_trash_id(&rel));
        let dir = self.trash_root().join(&tid.0);
        fs::create_dir(&dir)?;
        let mut entry = Entry {
            original_rel: rel,
            deleted_at: now_unix(),
            size_bytes: tree_size(&full),
            state: State::Moving,
            recovered: false,
            restore_rel: None,
        };
        let moved = write_entry(&dir, &entry)
            .and_then(|()| fs::rename(&full, dir.join(PAYLOAD)).map_err(Into::into));
        if let Err(e) = moved {
            let _ = fs::remove_dir_all(&dir);
            return Err(e);
        }
        entry.state = State::Trashed;
        // The move is done. If this rewrite is lost the next open finishes it.
        let _ = write_entry(&dir, &entry);
        Ok(tid)
    }

    /// Everything in Recently deleted, newest first.
    pub fn list_trash(&self) -> Result<Vec<TrashItem>> {
        let mut items = Vec::new();
        for (id, dir) in self.trash_dirs()? {
            let Ok(entry) = read_entry(&dir) else {
                continue;
            };
            if entry.state != State::Trashed || !payload_exists(&dir) {
                continue;
            }
            let name = Path::new(&entry.original_rel)
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            items.push(TrashItem {
                id,
                name,
                deleted_at: entry.deleted_at,
                size_bytes: entry.size_bytes,
                recovered: entry.recovered,
            });
        }
        items.sort_by(|a, b| {
            b.deleted_at
                .cmp(&a.deleted_at)
                .then_with(|| a.id.0.cmp(&b.id.0))
        });
        Ok(items)
    }

    /// Puts an item back. If its name is taken it comes back as `name (restored).ext`. Nothing is overwritten.
    pub fn restore_from_trash(&self, id: &TrashId) -> Result<FileId> {
        let dir = self.trash_root().join(&id.0);
        let mut entry = read_entry(&dir).map_err(|_| StoreError::NotFound)?;
        if entry.state != State::Trashed || !payload_exists(&dir) {
            return Err(StoreError::Conflict(
                "That item is not in Recently deleted any more.",
            ));
        }
        let wanted = clean_relative(&entry.original_rel)?;
        let dest = self.free_destination(&wanted)?;
        let rel = rel_text(&self.root, &dest)?;
        entry.state = State::Restoring;
        entry.restore_rel = Some(rel.clone());
        write_entry(&dir, &entry)?;
        if let Err(e) = move_no_clobber(&dir.join(PAYLOAD), &dest) {
            entry.state = State::Trashed;
            let _ = write_entry(&dir, &entry);
            return Err(e);
        }
        discard_entry(&self.trash_root(), &id.0)?;
        let file_id = FileId::for_rel(&entry.original_rel);
        if self.rel_of(&file_id).is_ok() {
            self.set_rel(&file_id, &rel)?;
            return Ok(file_id);
        }
        self.register_file(&rel)
    }

    /// Deletes everything in Recently deleted for good. Only call this when the person asked.
    pub fn empty_trash_now(&self) -> Result<u32> {
        self.delete_where(|_| true)
    }

    /// Deletes items older than `retention_days`, judged at `now` (seconds since 1970). The caller
    /// decides when to run this; the store never does it by itself.
    pub fn purge_trash_older_than(&self, retention_days: u32, now: i64) -> Result<u32> {
        let keep_for = i64::from(retention_days).saturating_mul(SECONDS_PER_DAY);
        self.delete_where(|e| now.saturating_sub(e.deleted_at) >= keep_for)
    }

    fn delete_where(&self, due: impl Fn(&Entry) -> bool) -> Result<u32> {
        let mut gone = 0;
        for (id, dir) in self.trash_dirs()? {
            let Ok(entry) = read_entry(&dir) else {
                continue;
            };
            if entry.state == State::Trashed && due(&entry) {
                discard_entry(&self.trash_root(), &id.0)?;
                gone += 1;
            }
        }
        self.sweep_deleting()?;
        Ok(gone)
    }

    /// Finishes or undoes whatever a crash interrupted. Runs when a project opens.
    pub(crate) fn repair_trash(&self) -> Result<RepairReport> {
        let mut report = RepairReport::default();
        self.sweep_deleting()?;
        for (id, dir) in self.trash_dirs()? {
            let has_payload = payload_exists(&dir);
            match (read_entry(&dir), has_payload) {
                (Ok(mut e), true) if e.state != State::Trashed => {
                    if e.state == State::Moving {
                        report.finished_moves += 1;
                    } else {
                        report.rolled_back += 1;
                        self.clear_placeholder(e.restore_rel.take());
                    }
                    e.state = State::Trashed;
                    write_entry(&dir, &e)?;
                }
                (Ok(e), false) if e.state == State::Moving => {
                    report.rolled_back += 1;
                    discard_entry(&self.trash_root(), &id.0)?;
                }
                (Ok(e), false) => {
                    self.record_restored(&e)?;
                    report.dropped += 1;
                    discard_entry(&self.trash_root(), &id.0)?;
                }
                (Ok(_), true) => {}
                (Err(_), true) => {
                    report.recovered += 1;
                    write_entry(&dir, &recovered_entry(&id, &dir))?;
                }
                (Err(_), false) => {
                    report.dropped += 1;
                    discard_entry(&self.trash_root(), &id.0)?;
                }
            }
        }
        Ok(report)
    }

    /// Removes the empty file a stopped restore claimed its name with. Anything with content stays.
    fn clear_placeholder(&self, rel: Option<String>) {
        let Some(path) = rel
            .and_then(|r| clean_relative(&r).ok())
            .and_then(|r| resolve_new(&self.root, &r).ok())
        else {
            return;
        };
        if fs::symlink_metadata(&path).is_ok_and(|m| m.is_file() && m.len() == 0) {
            // If this fails the next restore simply picks another name.
            let _ = fs::remove_file(path);
        }
    }

    /// A restore that moved the file but stopped before the database learned its new name.
    fn record_restored(&self, entry: &Entry) -> Result<()> {
        let Some(rel) = &entry.restore_rel else {
            return Ok(());
        };
        let id = FileId::for_rel(&entry.original_rel);
        if clean_relative(rel).is_ok() && self.rel_of(&id).is_ok() {
            self.set_rel(&id, rel)?;
        }
        Ok(())
    }

    fn trash_dirs(&self) -> Result<Vec<(TrashId, PathBuf)>> {
        let mut out = Vec::new();
        for item in fs::read_dir(self.trash_root())? {
            let item = item?;
            let Ok(id) = TrashId::parse(&item.file_name().to_string_lossy()) else {
                continue;
            };
            if item.file_type()?.is_dir() {
                out.push((id, item.path()));
            }
        }
        Ok(out)
    }

    fn sweep_deleting(&self) -> Result<()> {
        for item in fs::read_dir(self.trash_root())? {
            let item = item?;
            if item
                .file_name()
                .to_string_lossy()
                .starts_with(DELETING_PREFIX)
            {
                fs::remove_dir_all(item.path())?;
            }
        }
        Ok(())
    }

    /// A name in the original folder that nothing uses yet. A vanished folder falls back to the project root.
    fn free_destination(&self, wanted: &Path) -> Result<PathBuf> {
        match resolve_new(&self.root, wanted) {
            Err(StoreError::NotFound) => {
                let name = PathBuf::from(wanted.file_name().ok_or(StoreError::BadName)?);
                self.first_free(resolve_new(&self.root, &name)?)
            }
            other => self.first_free(other?),
        }
    }

    fn first_free(&self, first: PathBuf) -> Result<PathBuf> {
        if fs::symlink_metadata(&first).is_err() {
            return Ok(first);
        }
        let dir = first.parent().ok_or(StoreError::BadName)?;
        let name = first
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or(StoreError::BadName)?;
        let (stem, ext) = split_name(&name);
        for n in 1..=MAX_NAME_TRIES {
            let candidate = dir.join(numbered_name(stem, RESTORE_LABEL, n, ext));
            if fs::symlink_metadata(&candidate).is_err() {
                return Ok(candidate);
            }
        }
        Err(StoreError::Conflict("Too many files share that name."))
    }
}

/// Splits `report.csv` into `report` and `csv`. A leading dot is part of the stem.
pub(crate) fn split_name(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => (&name[..i], Some(&name[i + 1..])),
        _ => (name, None),
    }
}

fn recovered_entry(id: &TrashId, dir: &Path) -> Entry {
    Entry {
        original_rel: format!("Recovered item {}", &id.0[..8]),
        deleted_at: now_unix(),
        size_bytes: tree_size(&dir.join(PAYLOAD)),
        state: State::Trashed,
        recovered: true,
        restore_rel: None,
    }
}

fn new_trash_id(rel: &str) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let key = format!(
        "{rel}#{}#{}#{}",
        now_nanos(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    opaque_id("trash", &key)
}

fn payload_exists(dir: &Path) -> bool {
    fs::symlink_metadata(dir.join(PAYLOAD)).is_ok()
}

fn write_entry(dir: &Path, entry: &Entry) -> Result<()> {
    let bytes = serde_json::to_vec(entry)
        .map_err(|_| StoreError::Conflict("The trash journal could not be written."))?;
    atomic_write(&dir.join(ENTRY_FILE), &bytes)?;
    Ok(())
}

fn read_entry(dir: &Path) -> Result<Entry> {
    let mut text = Vec::new();
    File::open(dir.join(ENTRY_FILE))?
        .take(MAX_ENTRY_BYTES + 1)
        .read_to_end(&mut text)?;
    if text.len() as u64 > MAX_ENTRY_BYTES {
        return Err(StoreError::Conflict("The trash journal is damaged."));
    }
    serde_json::from_slice(&text).map_err(|_| StoreError::Conflict("The trash journal is damaged."))
}

/// Removes an entry folder in two steps so a crash never leaves a payload without its journal.
fn discard_entry(trash_root: &Path, id: &str) -> Result<()> {
    let doomed = trash_root.join(format!("{DELETING_PREFIX}{id}"));
    fs::rename(trash_root.join(id), &doomed)?;
    fs::remove_dir_all(doomed)?;
    Ok(())
}

/// Moves without ever replacing something that already exists at `to`. A file's name is first claimed
/// with an empty file, and the rename then replaces only that, so the content is never in two places.
fn move_no_clobber(from: &Path, to: &Path) -> Result<()> {
    if fs::symlink_metadata(from)?.is_dir() {
        // Renaming a folder cannot replace a file or a folder with something in it.
        if fs::symlink_metadata(to).is_ok() {
            return Err(std::io::Error::from(std::io::ErrorKind::AlreadyExists).into());
        }
        return Ok(fs::rename(from, to)?);
    }
    File::options().write(true).create_new(true).open(to)?;
    if let Err(e) = fs::rename(from, to) {
        let _ = fs::remove_file(to);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(files: &[(&str, &str)]) -> (tempfile::TempDir, Project) {
        let d = tempfile::tempdir().expect("tempdir");
        for (name, body) in files {
            fs::write(d.path().join(name), body).expect("file");
        }
        let p = Project::open_or_create(d.path()).expect("open");
        (d, p)
    }

    #[test]
    fn trash_and_restore_round_trip() {
        let (d, p) = setup(&[("a.csv", "one")]);
        let id = p.register_file("a.csv").expect("reg");
        let t = p.trash_file(&id).expect("trash");
        assert!(!d.path().join("a.csv").exists());
        let list = p.list_trash().expect("list");
        assert_eq!((list.len(), list[0].name.as_str()), (1, "a.csv"));
        let back = p.restore_from_trash(&t).expect("restore");
        assert_eq!(back, id);
        assert_eq!(
            fs::read_to_string(d.path().join("a.csv")).expect("read"),
            "one"
        );
        assert!(p.list_trash().expect("list").is_empty());
    }

    #[test]
    fn restore_never_overwrites_a_new_file_with_the_same_name() {
        let (d, p) = setup(&[("a.csv", "old")]);
        let id = p.register_file("a.csv").expect("reg");
        let t = p.trash_file(&id).expect("trash");
        fs::write(d.path().join("a.csv"), "new").expect("new file");
        let back = p.restore_from_trash(&t).expect("restore");
        assert_eq!(
            fs::read_to_string(d.path().join("a.csv")).expect("read"),
            "new"
        );
        assert_eq!(
            fs::read_to_string(d.path().join("a (restored).csv")).expect("read"),
            "old"
        );
        assert_eq!(p.display_name(&back).expect("name"), "a (restored).csv");
    }

    #[test]
    fn empty_now_and_retention_purge_only_what_is_due() {
        let (_d, p) = setup(&[("a.csv", "1"), ("b.csv", "2")]);
        let a = p
            .trash_file(&p.register_file("a.csv").expect("a"))
            .expect("trash a");
        let now = now_unix();
        assert_eq!(
            p.purge_trash_older_than(30, now + 29 * SECONDS_PER_DAY)
                .expect("early"),
            0
        );
        assert_eq!(p.list_trash().expect("list").len(), 1);
        p.trash_file(&p.register_file("b.csv").expect("b"))
            .expect("trash b");
        assert_eq!(
            p.purge_trash_older_than(30, now + 31 * SECONDS_PER_DAY)
                .expect("late"),
            2
        );
        assert!(matches!(
            p.restore_from_trash(&a),
            Err(StoreError::NotFound)
        ));
        assert_eq!(p.empty_trash_now().expect("empty"), 0);
    }

    fn interrupted(state: State, with_payload: bool) -> (tempfile::TempDir, TrashId) {
        let (d, p) = setup(&[("a.csv", "keep me")]);
        let id = p.register_file("a.csv").expect("reg");
        let t = p.trash_file(&id).expect("trash");
        let dir = d
            .path()
            .join(WORK_DIR_FOR_TESTS)
            .join(TRASH_DIR)
            .join(t.as_str());
        let mut e = read_entry(&dir).expect("entry");
        e.state = state;
        write_entry(&dir, &e).expect("rewrite");
        if !with_payload {
            fs::rename(dir.join(PAYLOAD), d.path().join("a.csv")).expect("undo the move");
        }
        (d, t)
    }
    const WORK_DIR_FOR_TESTS: &str = crate::names::WORK_DIR;

    #[test]
    fn move_done_but_not_written_down_is_finished_on_open() {
        let (d, t) = interrupted(State::Moving, true);
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(p.recovery().finished_moves, 1);
        assert_eq!(p.list_trash().expect("list")[0].id, t);
    }

    #[test]
    fn move_that_never_happened_is_cleared_and_the_original_stays() {
        let (d, _t) = interrupted(State::Moving, false);
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(p.recovery().rolled_back, 1);
        assert_eq!(
            fs::read_to_string(d.path().join("a.csv")).expect("read"),
            "keep me"
        );
        assert!(p.list_trash().expect("list").is_empty());
    }

    #[test]
    fn restore_cut_short_goes_back_to_the_trash_or_completes() {
        let (d, _t) = interrupted(State::Restoring, true);
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(p.list_trash().expect("list").len(), 1);
        let (d2, _t2) = interrupted(State::Restoring, false);
        let p2 = Project::open_or_create(d2.path()).expect("reopen");
        assert_eq!(p2.recovery().dropped, 1);
        assert_eq!(
            fs::read_to_string(d2.path().join("a.csv")).expect("read"),
            "keep me"
        );
    }

    #[test]
    fn damaged_journal_keeps_the_payload_and_offers_it_back() {
        let (d, p) = setup(&[("a.csv", "precious")]);
        let t = p
            .trash_file(&p.register_file("a.csv").expect("reg"))
            .expect("trash");
        drop(p);
        let dir = d
            .path()
            .join(WORK_DIR_FOR_TESTS)
            .join(TRASH_DIR)
            .join(t.as_str());
        fs::write(dir.join(ENTRY_FILE), b"{ not json").expect("damage");
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(p.recovery().recovered, 1);
        let item = p.list_trash().expect("list").remove(0);
        assert!(item.recovered && item.name.starts_with("Recovered item"));
        p.restore_from_trash(&item.id).expect("restore");
        assert_eq!(
            fs::read_to_string(d.path().join(&item.name)).expect("read"),
            "precious"
        );
    }

    #[test]
    fn hostile_journal_cannot_restore_outside_the_project() {
        let (d, p) = setup(&[("a.csv", "x")]);
        let t = p
            .trash_file(&p.register_file("a.csv").expect("reg"))
            .expect("trash");
        let dir = d
            .path()
            .join(WORK_DIR_FOR_TESTS)
            .join(TRASH_DIR)
            .join(t.as_str());
        let mut e = read_entry(&dir).expect("entry");
        e.original_rel = "../escaped.csv".into();
        write_entry(&dir, &e).expect("rewrite");
        assert!(matches!(p.restore_from_trash(&t), Err(StoreError::BadName)));
        assert!(
            !d.path()
                .parent()
                .expect("parent")
                .join("escaped.csv")
                .exists()
        );
        assert!(TrashId::parse("../../x").is_err());
    }

    #[test]
    fn a_link_is_trashed_as_a_link_and_its_target_is_untouched() {
        #[cfg(unix)]
        {
            let (d, p) = setup(&[]);
            let outside = tempfile::tempdir().expect("outside");
            fs::write(outside.path().join("t.txt"), "target").expect("target");
            std::os::unix::fs::symlink(outside.path().join("t.txt"), d.path().join("l.txt"))
                .expect("link");
            let t = p
                .trash_file(&p.register_file("l.txt").expect("reg"))
                .expect("trash");
            assert!(fs::symlink_metadata(d.path().join("l.txt")).is_err());
            assert_eq!(
                fs::read_to_string(outside.path().join("t.txt")).expect("read"),
                "target"
            );
            p.empty_trash_now().expect("empty");
            assert_eq!(
                fs::read_to_string(outside.path().join("t.txt")).expect("read"),
                "target"
            );
            let _ = t;
        }
    }

    #[test]
    fn a_restore_stopped_after_claiming_the_name_leaves_one_copy() {
        let (d, p) = setup(&[("a.csv", "keep me")]);
        let t = p
            .trash_file(&p.register_file("a.csv").expect("reg"))
            .expect("trash");
        drop(p);
        let dir = d
            .path()
            .join(WORK_DIR_FOR_TESTS)
            .join(TRASH_DIR)
            .join(t.as_str());
        let mut e = read_entry(&dir).expect("entry");
        e.state = State::Restoring;
        e.restore_rel = Some("a.csv".into());
        write_entry(&dir, &e).expect("rewrite");
        fs::write(d.path().join("a.csv"), b"").expect("placeholder");
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert!(
            !d.path().join("a.csv").exists(),
            "the empty placeholder is cleared"
        );
        p.restore_from_trash(&t).expect("restore");
        assert_eq!(
            fs::read_to_string(d.path().join("a.csv")).expect("read"),
            "keep me"
        );
        assert!(!d.path().join("a (restored).csv").exists());
    }

    #[test]
    fn a_finished_restore_keeps_the_new_name_after_a_stop() {
        let (d, p) = setup(&[("a.csv", "old")]);
        let id = p.register_file("a.csv").expect("reg");
        let t = p.trash_file(&id).expect("trash");
        let dir = d
            .path()
            .join(WORK_DIR_FOR_TESTS)
            .join(TRASH_DIR)
            .join(t.as_str());
        let mut e = read_entry(&dir).expect("entry");
        e.state = State::Restoring;
        e.restore_rel = Some("a (restored).csv".into());
        write_entry(&dir, &e).expect("rewrite");
        fs::rename(dir.join(PAYLOAD), d.path().join("a (restored).csv")).expect("moved");
        drop(p);
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(p.display_name(&id).expect("name"), "a (restored).csv");
    }

    #[test]
    fn moving_never_replaces_an_existing_file() {
        let d = tempfile::tempdir().expect("tempdir");
        let (from, to) = (d.path().join("from"), d.path().join("to"));
        fs::write(&from, "new").expect("from");
        fs::write(&to, "theirs").expect("to");
        assert!(move_no_clobber(&from, &to).is_err());
        assert_eq!(fs::read_to_string(&to).expect("read"), "theirs");
        assert!(from.exists());
    }
}
