use std::fs;

use super::*;
use crate::{Error, Limits};

fn tree(root: &std::path::Path) -> Vec<(String, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(d).expect("read_dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p
                    .strip_prefix(root)
                    .expect("prefix")
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((rel, fs::read(&p).expect("read")));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn many_files_round_trip_across_chunk_sizes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let (a, b) = (dir.path().join("a"), dir.path().join("b"));
    write(&a, "empty.txt", b"");
    write(&a, "one.bin", b"x");
    write(&a, "deep/er/still/note.txt", b"hello");
    write(&a, "exact.bin", &pattern(65_536, 1));
    write(&a, "big.bin", &pattern(65_536 * 4 + 123, 2));
    write(&b, "sheet.csv", &pattern(70_000, 3));
    for i in 0..40 {
        write(&b, &format!("many/f{i}.dat"), &pattern(i * 997, i as u8));
    }
    let archive = dir.path().join("all.khbackup");
    let mut seen = Vec::new();
    let report = crate::create_with(
        &[source("alpha", &a), source("beta", &b)],
        &archive,
        PASS,
        FAST,
        &mut |p| seen.push(p),
    )
    .expect("create");
    assert_eq!(report.files, 46);
    assert_eq!(seen.last().expect("progress").files, 46);
    assert!(report.skipped.is_empty());

    let out = dir.path().join("restored");
    let got = crate::restore(&archive, &out, PASS, Limits::default(), &mut no_progress())
        .expect("restore");
    assert_eq!((got.files, got.bytes), (report.files, report.bytes));
    assert_eq!(tree(&out.join("alpha")), tree(&a));
    assert_eq!(tree(&out.join("beta")), tree(&b));
    assert_eq!(
        fs::read_dir(&out).expect("ls").count(),
        2,
        "no leftover scratch folder"
    );
}

#[test]
fn plaintext_that_exactly_fills_chunks_round_trips() {
    // Record overhead for path "p/a" is 63 bytes, so these sizes end a chunk exactly, one short, one over.
    for total in [65_536usize, 131_072] {
        for delta in [-1i64, 0, 1] {
            let size = (total as i64 - 63 + delta) as usize;
            let bytes = make_archive(&[("a", pattern(size, 9))]);
            let got = restore_bytes(&bytes, Limits::default()).expect("restore");
            assert_eq!(got.bytes as usize, size);
        }
    }
}

#[test]
fn wrong_passphrase_gives_the_one_message_and_leaves_nothing() {
    let bytes = make_archive(&[("a.txt", b"hi".to_vec())]);
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("a.khbackup");
    fs::write(&archive, bytes).expect("write");
    let out = dir.path().join("out");
    let err = crate::restore(
        &archive,
        &out,
        "not the passphrase",
        Limits::default(),
        &mut no_progress(),
    )
    .unwrap_err();
    assert!(matches!(err, Error::Damaged));
    assert_eq!(
        err.to_string(),
        "The passphrase is wrong or the backup is damaged."
    );
    assert!(!out.exists());
    assert!(matches!(
        crate::test(&archive, "nope nope nope", &dir.path().join("s")),
        Err(Error::Damaged)
    ));
}

#[test]
fn restore_refuses_a_non_empty_folder_and_changes_nothing() {
    let bytes = make_archive(&[("a.txt", b"hi".to_vec())]);
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("a.khbackup");
    fs::write(&archive, bytes).expect("write");
    let out = dir.path().join("out");
    write(&out, "mine.txt", b"keep");
    let err =
        crate::restore(&archive, &out, PASS, Limits::default(), &mut no_progress()).unwrap_err();
    assert!(matches!(err, Error::FolderNotEmpty));
    assert_eq!(tree(&out), vec![("mine.txt".to_string(), b"keep".to_vec())]);
    // An existing empty folder is fine.
    let empty = dir.path().join("empty");
    fs::create_dir(&empty).expect("mkdir");
    crate::restore(
        &archive,
        &empty,
        PASS,
        Limits::default(),
        &mut no_progress(),
    )
    .expect("restore");
}

#[test]
fn create_never_overwrites_and_checks_the_passphrase() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("src");
    write(&src, "a.txt", b"hi");
    let dest = dir.path().join("a.khbackup");
    fs::write(&dest, b"precious").expect("write");
    let err = crate::create_with(&[source("p", &src)], &dest, PASS, FAST, &mut no_progress())
        .unwrap_err();
    assert!(matches!(err, Error::DestinationExists));
    assert_eq!(fs::read(&dest).expect("read"), b"precious");
    let fresh = dir.path().join("b.khbackup");
    let err = crate::create_with(
        &[source("p", &src)],
        &fresh,
        "short",
        FAST,
        &mut no_progress(),
    )
    .unwrap_err();
    assert!(matches!(err, Error::PassphraseTooShort));
    assert!(!fresh.exists());
    let err = crate::create_with(&[], &fresh, PASS, FAST, &mut no_progress()).unwrap_err();
    assert!(matches!(err, Error::NoSources));
}

#[test]
fn leftover_temp_files_are_skipped_and_bad_labels_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("src");
    write(&src, "keep.txt", b"k");
    write(&src, ".keep.txt.1-2.craft_tmp", b"junk");
    let dest = dir.path().join("a.khbackup");
    let report = crate::create_with(&[source("p", &src)], &dest, PASS, FAST, &mut no_progress())
        .expect("create");
    assert_eq!(report.files, 1);
    for label in ["", "a/b", "..", "CON"] {
        let d = dir.path().join(format!("x{}.khbackup", label.len()));
        let err = crate::create_with(&[source(label, &src)], &d, PASS, FAST, &mut no_progress())
            .unwrap_err();
        assert!(matches!(err, Error::UnsafeName(_)), "{label:?}");
        assert!(!d.exists());
    }
}

#[cfg(unix)]
#[test]
fn symlinks_are_skipped_and_listed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("src");
    write(&src, "real.txt", b"r");
    write(dir.path(), "outside.txt", b"secret");
    std::os::unix::fs::symlink(dir.path().join("outside.txt"), src.join("link.txt"))
        .expect("symlink");
    std::os::unix::fs::symlink(dir.path(), src.join("dirlink")).expect("symlink");
    let dest = dir.path().join("a.khbackup");
    let report = crate::create_with(&[source("p", &src)], &dest, PASS, FAST, &mut no_progress())
        .expect("create");
    assert_eq!(report.files, 1);
    assert_eq!(
        report.skipped,
        vec!["p/dirlink".to_string(), "p/link.txt".to_string()]
    );
}

#[cfg(unix)]
#[test]
fn restore_refuses_a_linked_target_folder() {
    let bytes = make_archive(&[("a.txt", b"hi".to_vec())]);
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("a.khbackup");
    fs::write(&archive, bytes).expect("write");
    let real = dir.path().join("real");
    fs::create_dir(&real).expect("mkdir");
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&real, &link).expect("symlink");
    let err =
        crate::restore(&archive, &link, PASS, Limits::default(), &mut no_progress()).unwrap_err();
    assert!(matches!(err, Error::FolderNotEmpty));
    assert_eq!(fs::read_dir(&real).expect("ls").count(), 0);
}

#[cfg(unix)]
#[test]
fn failure_during_create_leaves_no_archive_and_no_staging_file() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("src");
    write(&src, "a_first.txt", &pattern(200_000, 1));
    write(&src, "z_locked.txt", b"x");
    let locked = src.join("z_locked.txt");
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).expect("chmod");
    if fs::File::open(&locked).is_ok() {
        return; // running as a user who can read anything
    }
    let out = dir.path().join("out");
    fs::create_dir(&out).expect("mkdir");
    let dest = out.join("a.khbackup");
    let err = crate::create_with(&[source("p", &src)], &dest, PASS, FAST, &mut no_progress())
        .unwrap_err();
    assert!(matches!(err, Error::Io(_)));
    assert_eq!(
        fs::read_dir(&out).expect("ls").count(),
        0,
        "no archive or staging file"
    );
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o600)).expect("chmod");
}

#[test]
fn restore_limits_apply() {
    let bytes = make_archive(&[("a", vec![1; 10]), ("b", vec![2; 10]), ("c", vec![3; 10])]);
    let err = restore_bytes(
        &bytes,
        Limits {
            max_files: 2,
            max_bytes: u64::MAX,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::TooManyFiles));
    let err = restore_bytes(
        &bytes,
        Limits {
            max_files: 10,
            max_bytes: 29,
        },
    )
    .unwrap_err();
    assert!(matches!(err, Error::TooBig));
    restore_bytes(
        &bytes,
        Limits {
            max_files: 3,
            max_bytes: 30,
        },
    )
    .expect("exact limits pass");
}

#[test]
fn test_backup_passes_and_cleans_up() {
    let files: Vec<(String, Vec<u8>)> = (0..12)
        .map(|i| (format!("d/f{i}"), pattern(1000 + i, i as u8)))
        .collect();
    let refs: Vec<(&str, Vec<u8>)> = files.iter().map(|(n, b)| (n.as_str(), b.clone())).collect();
    let bytes = make_archive(&refs);
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("a.khbackup");
    fs::write(&archive, &bytes).expect("write");
    let scratch = dir.path().join("scratch");
    let report = crate::test(&archive, PASS, &scratch).expect("test");
    assert_eq!((report.files, report.sampled), (12, 5));
    assert!(!scratch.exists(), "scratch removed");
    let mut bad = bytes;
    let mid = bad.len() / 2;
    bad[mid] ^= 1;
    fs::write(&archive, &bad).expect("write");
    assert!(matches!(
        crate::test(&archive, PASS, &scratch),
        Err(Error::Damaged)
    ));
    assert!(!scratch.exists());
}
