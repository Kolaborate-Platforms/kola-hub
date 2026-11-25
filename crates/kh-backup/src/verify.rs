//! "Test the last backup": verify everything, then write a few sample files and compare them.

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::restore::{DirSink, scratch_dir};
use crate::walk::{Sink, open_stream, walk};
use crate::{Limits, TestReport};

/// Samples bigger than this are covered by the full read but not written out.
const MAX_SAMPLE_BYTES: u64 = 256 * 1024 * 1024;
const SAMPLES: u64 = 5;

struct Discard;

impl Sink for Discard {
    fn open(&mut self, _: u64, _: &str, _: u64) -> Result<Option<File>> {
        Ok(None)
    }
    fn done(&mut self, _: u64, _: [u8; 32]) -> Result<()> {
        Ok(())
    }
}

struct Sampler {
    inner: DirSink,
    wanted: Vec<u64>,
    written: Vec<(u64, std::path::PathBuf)>,
}

impl Sink for Sampler {
    fn open(&mut self, index: u64, path: &str, size: u64) -> Result<Option<File>> {
        if !self.wanted.contains(&index) || size > MAX_SAMPLE_BYTES {
            return Ok(None);
        }
        self.written.push((index, self.inner.root.join(path)));
        self.inner.open(index, path, size)
    }

    fn done(&mut self, index: u64, hash: [u8; 32]) -> Result<()> {
        let Some((_, path)) = self.written.iter().find(|(i, _)| *i == index) else {
            return Ok(());
        };
        if hash_file(path)? != hash {
            return Err(Error::Damaged);
        }
        Ok(())
    }
}

fn hash_file(path: &Path) -> Result<[u8; 32]> {
    let mut f = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(Error::Io)?;
        if n == 0 {
            return Ok(hasher.finalize().into());
        }
        hasher.update(&buf[..n]);
    }
}

fn sample_indexes(files: u64) -> Vec<u64> {
    let k = files.min(SAMPLES);
    if k < 2 {
        return (0..k).collect();
    }
    (0..k).map(|i| i * (files - 1) / (k - 1)).collect()
}

pub(crate) fn test_impl(archive: &Path, passphrase: &str, scratch: &Path) -> Result<TestReport> {
    let limits = Limits::default();
    let mut ignore = |_| {};
    let totals = walk(
        &mut open_stream(archive, passphrase)?,
        &limits,
        &mut Discard,
        &mut ignore,
    )?;

    let created_scratch = !scratch.exists();
    fs::create_dir_all(scratch)?;
    let temp = scratch_dir(scratch, "kh-test")?;
    let wanted = sample_indexes(totals.files);
    let mut sampled = 0;
    let result = fs::create_dir(&temp).map_err(Error::from).and_then(|()| {
        let mut sampler = Sampler {
            inner: DirSink {
                root: temp.clone(),
                written: Vec::new(),
            },
            wanted,
            written: Vec::new(),
        };
        let totals = walk(
            &mut open_stream(archive, passphrase)?,
            &limits,
            &mut sampler,
            &mut ignore,
        );
        // Only files that were written and compared count as samples.
        sampled = sampler.written.len() as u64;
        totals
    });
    let _ = fs::remove_dir_all(&temp);
    if created_scratch {
        let _ = fs::remove_dir(scratch);
    }
    let again = result?;
    if again != totals {
        return Err(Error::Damaged);
    }
    Ok(TestReport {
        files: totals.files,
        bytes: totals.bytes,
        sampled,
    })
}
