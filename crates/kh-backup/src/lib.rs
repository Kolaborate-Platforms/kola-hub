//! Sealed passphrase backup and restore. Format: `docs/contracts/backup-format.md`.
//!
//! One `.khbackup` file holds one or more project folders. It is encrypted with a key derived from a
//! passphrase and cannot be opened without it. Nothing here touches the network.

mod create;
mod error;
mod header;
mod paths;
mod restore;
mod seal;
mod verify;
mod walk;

#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

pub use error::{Error, Result};
pub use header::KdfParams;

/// One project folder to include, with the name it is stored under.
#[derive(Clone, Debug)]
pub struct Source {
    /// First part of every stored path. A single plain name.
    pub label: String,
    /// The folder on disk.
    pub folder: PathBuf,
}

/// Running totals, passed to the progress callback after each file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    /// Files finished so far.
    pub files: u64,
    /// Bytes finished so far.
    pub bytes: u64,
}

/// Limits applied when reading an archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Most plaintext bytes accepted.
    pub max_bytes: u64,
    /// Most files accepted.
    pub max_files: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_bytes: 50 * 1024 * 1024 * 1024,
            max_files: 200_000,
        }
    }
}

/// What `create` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateReport {
    /// Files saved.
    pub files: u64,
    /// Bytes saved.
    pub bytes: u64,
    /// Links and other special entries left out, as `label/relative/path`.
    pub skipped: Vec<String>,
}

/// What `restore` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestoreReport {
    /// Files restored.
    pub files: u64,
    /// Bytes restored.
    pub bytes: u64,
}

/// What `test` checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TestReport {
    /// Files whose hashes were checked.
    pub files: u64,
    /// Bytes read.
    pub bytes: u64,
    /// Files also written out and compared.
    pub sampled: u64,
}

/// Seals the sources into a new file at `destination`. Never replaces an existing file.
pub fn create(
    sources: &[Source],
    destination: &Path,
    passphrase: &str,
    progress: &mut dyn FnMut(Progress),
) -> Result<CreateReport> {
    create::create_impl(
        sources,
        destination,
        passphrase,
        KdfParams::default(),
        progress,
    )
}

/// Like [`create`] with a chosen Argon2 cost. `KdfParams` cannot hold values below the format minimum.
pub fn create_with(
    sources: &[Source],
    destination: &Path,
    passphrase: &str,
    kdf: KdfParams,
    progress: &mut dyn FnMut(Progress),
) -> Result<CreateReport> {
    create::create_impl(sources, destination, passphrase, kdf, progress)
}

/// Restores into `folder`, which must be empty or missing. On any failure nothing is left behind.
pub fn restore(
    archive: &Path,
    folder: &Path,
    passphrase: &str,
    limits: Limits,
    progress: &mut dyn FnMut(Progress),
) -> Result<RestoreReport> {
    restore::restore_impl(archive, folder, passphrase, limits, progress)
}

/// Reads the whole archive, checks every hash, and writes then compares a few sample files in `scratch`.
pub fn test(archive: &Path, passphrase: &str, scratch: &Path) -> Result<TestReport> {
    verify::test_impl(archive, passphrase, scratch)
}
