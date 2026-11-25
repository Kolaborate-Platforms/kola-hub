//! Archives built by hand with a valid seal but hostile contents.

use super::*;
use crate::header::Header;
use crate::seal::{SealWriter, derive_key};
use crate::{Error, Limits};
use std::io::Write;

fn seal(plain: &[u8]) -> Vec<u8> {
    let header = Header::generate(FAST).expect("header");
    let key = derive_key(PASS, &header).expect("key");
    let mut w = SealWriter::new(Vec::new(), &header, &key).expect("writer");
    w.write_all(plain).expect("write");
    w.finish().expect("finish")
}

fn file_rec(path: &[u8], data: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut v = vec![1u8];
    v.extend((path.len() as u16).to_le_bytes());
    v.extend(path);
    v.extend((data.len() as u64).to_le_bytes());
    v.extend(data);
    v.extend(Sha256::digest(data));
    v
}

fn end_rec(files: u64, bytes: u64) -> Vec<u8> {
    let mut v = vec![0u8];
    v.extend(files.to_le_bytes());
    v.extend(bytes.to_le_bytes());
    v
}

fn archive_of(paths: &[&[u8]]) -> Vec<u8> {
    let mut plain = Vec::new();
    for p in paths {
        plain.extend(file_rec(p, b"data"));
    }
    plain.extend(end_rec(paths.len() as u64, 4 * paths.len() as u64));
    seal(&plain)
}

fn restore(bytes: &[u8]) -> crate::Result<crate::RestoreReport> {
    restore_bytes(bytes, Limits::default())
}

#[test]
fn the_hand_built_writer_makes_a_good_archive() {
    restore(&archive_of(&[b"p/a.txt", b"p/sub/b.txt", b"loose"])).expect("restore");
}

#[test]
fn every_refused_path_is_refused() {
    let long_part = format!("p/{}", "x".repeat(256));
    let deep = vec!["d"; 33].join("/");
    let mut bad: Vec<Vec<u8>> = vec![
        b"".to_vec(),
        b"/abs".to_vec(),
        b"p//x".to_vec(),
        b"p/".to_vec(),
        b"p/./x".to_vec(),
        b"p/../x".to_vec(),
        b"..".to_vec(),
        b"p/x.".to_vec(),
        b"p/x ".to_vec(),
        b"C:/x".to_vec(),
        b"p/a\\b".to_vec(),
        b"p/a*b".to_vec(),
        b"p/a?b".to_vec(),
        b"p/a\"b".to_vec(),
        b"p/a<b".to_vec(),
        b"p/a>b".to_vec(),
        b"p/a|b".to_vec(),
        b"p/a\x01b".to_vec(),
        b"p/a\x7fb".to_vec(),
        long_part.into_bytes(),
        deep.into_bytes(),
        vec![0xff, 0xfe, b'x'],
    ];
    for dev in [
        "CON",
        "prn",
        "Aux",
        "NUL",
        "COM1",
        "com9",
        "LPT1",
        "lpt9",
        "con.txt",
        "Nul.tar.gz",
        "COM3.log",
    ] {
        bad.push(format!("p/{dev}").into_bytes());
    }
    for path in bad {
        let err = restore(&archive_of(&[&path])).unwrap_err();
        assert!(
            matches!(err, Error::UnsafeName(_)),
            "{:?} gave {err:?}",
            String::from_utf8_lossy(&path)
        );
    }
    // Close lookalikes are fine.
    restore(&archive_of(&[
        b"p/COM0",
        b"p/CONSOLE",
        b"p/COM10",
        b"p/a.b",
        b"p/.hidden",
    ]))
    .expect("restore");
}

#[test]
fn repeated_and_case_only_names_are_refused() {
    let pairs: [[&[u8]; 2]; 4] = [
        [b"p/a", b"p/a"],
        [b"p/a", b"p/A"],
        [b"p/x", b"p/X/y"],
        [b"p/x/y", b"p/X"],
    ];
    for pair in pairs {
        let err = restore(&archive_of(&pair)).unwrap_err();
        assert!(matches!(err, Error::UnsafeName(_)), "{pair:?}");
    }
}

#[test]
fn a_path_longer_than_any_allowed_name_is_refused_unread() {
    let mut plain = vec![1u8];
    plain.extend(u16::MAX.to_le_bytes());
    plain.extend(vec![b'a'; 100]);
    assert!(matches!(restore(&seal(&plain)), Err(Error::UnsafeName(_))));
}

#[test]
fn bad_records_are_refused() {
    let good_file = file_rec(b"p/a", b"data");
    let with_end = |end: Vec<u8>| [good_file.clone(), end].concat();

    // Missing end record, wrong counts, extra data after the end, unknown tag, empty plaintext.
    assert!(matches!(restore(&seal(&good_file)), Err(Error::Damaged)));
    assert!(matches!(
        restore(&seal(&with_end(end_rec(2, 4)))),
        Err(Error::Damaged)
    ));
    assert!(matches!(
        restore(&seal(&with_end(end_rec(1, 5)))),
        Err(Error::Damaged)
    ));
    let mut trailing = with_end(end_rec(1, 4));
    trailing.push(0);
    assert!(matches!(restore(&seal(&trailing)), Err(Error::Damaged)));
    assert!(matches!(restore(&seal(&[2u8])), Err(Error::Damaged)));
    assert!(matches!(restore(&seal(&[])), Err(Error::Damaged)));

    // A wrong hash.
    let mut wrong = with_end(end_rec(1, 4));
    let hash_at = wrong.len() - 17 - 32;
    wrong[hash_at] ^= 1;
    assert!(matches!(restore(&seal(&wrong)), Err(Error::Damaged)));
}

#[test]
fn declared_sizes_are_never_trusted() {
    // Declares 1 TiB but holds a few bytes.
    let mut plain = vec![1u8, 3, 0];
    plain.extend(b"p/a");
    plain.extend((1u64 << 40).to_le_bytes());
    plain.extend(b"tiny");
    let big = Limits {
        max_bytes: u64::MAX,
        max_files: 10,
    };
    assert!(matches!(
        restore_bytes(&seal(&plain), big),
        Err(Error::Damaged)
    ));
    assert!(matches!(restore(&seal(&plain)), Err(Error::TooBig)));
    let mut huge = plain[..3 + 3].to_vec();
    huge.extend(u64::MAX.to_le_bytes());
    assert!(matches!(restore(&seal(&huge)), Err(Error::TooBig)));
}

#[test]
fn a_hostile_archive_cannot_name_a_file_outside_the_folder() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("a.khbackup");
    std::fs::write(&archive, archive_of(&[b"p/../../escape.txt"])).expect("write");
    let out = dir.path().join("deep/out");
    assert!(crate::restore(&archive, &out, PASS, Limits::default(), &mut no_progress()).is_err());
    assert!(
        !dir.path().join("escape.txt").exists() && !dir.path().join("deep/escape.txt").exists()
    );
    assert!(!out.exists());
}
