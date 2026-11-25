//! Tests. `hostile` builds archives by hand, outside the production writer.

mod hostile;
mod roundtrip;
mod tamper;

use std::fs;
use std::path::Path;

use crate::{KdfParams, Progress, Source};

pub(crate) const PASS: &str = "correct horse battery";
pub(crate) const FAST: KdfParams = KdfParams::CHEAPEST;

pub(crate) fn write(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    fs::create_dir_all(p.parent().expect("has parent")).expect("mkdir");
    fs::write(p, bytes).expect("write");
}

pub(crate) fn source(label: &str, folder: &Path) -> Source {
    Source {
        label: label.into(),
        folder: folder.into(),
    }
}

pub(crate) fn no_progress() -> impl FnMut(Progress) {
    |_| {}
}

/// Deterministic bytes that do not repeat on a chunk boundary.
pub(crate) fn pattern(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u32).wrapping_mul(2_654_435_761).to_le_bytes()[2] ^ seed)
        .collect()
}

/// Restores raw archive bytes into a fresh folder; checks that a failure leaves the folder empty or absent.
pub(crate) fn restore_bytes(
    bytes: &[u8],
    limits: crate::Limits,
) -> crate::Result<crate::RestoreReport> {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("a.khbackup");
    fs::write(&archive, bytes).expect("write archive");
    let out = dir.path().join("out");
    let result = crate::restore(&archive, &out, PASS, limits, &mut no_progress());
    if result.is_err() {
        assert!(
            !out.exists() || fs::read_dir(&out).expect("read out").next().is_none(),
            "failed restore left files"
        );
    }
    result
}

/// A real archive of the given files, as bytes.
pub(crate) fn make_archive(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let dir = tempfile::tempdir().expect("tempdir");
    let src = dir.path().join("src");
    for (rel, bytes) in files {
        write(&src, rel, bytes);
    }
    let dest = dir.path().join("a.khbackup");
    crate::create_with(&[source("p", &src)], &dest, PASS, FAST, &mut no_progress())
        .expect("create");
    fs::read(dest).expect("read archive")
}
