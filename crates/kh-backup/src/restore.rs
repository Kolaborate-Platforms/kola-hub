//! Restoring a backup into an empty folder.

use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::walk::{Sink, open_stream, walk};
use crate::{Limits, Progress, RestoreReport};

/// Creates new files below `root`. Nothing already there is opened, and links are never followed.
pub(crate) struct DirSink {
    pub root: PathBuf,
    /// Every file created, relative to `root`, so a failed restore removes only these.
    pub written: Vec<PathBuf>,
}

impl Sink for DirSink {
    fn open(&mut self, _index: u64, path: &str, _size: u64) -> Result<Option<File>> {
        let mut target = self.root.clone();
        target.extend(path.split('/'));
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)?;
        self.written.push(path.split('/').collect());
        Ok(Some(file))
    }

    fn done(&mut self, _index: u64, _hash: [u8; 32]) -> Result<()> {
        Ok(())
    }
}

/// A random hidden folder name inside `parent`.
pub(crate) fn scratch_dir(parent: &Path, prefix: &str) -> Result<PathBuf> {
    let mut id = [0u8; 8];
    getrandom::fill(&mut id).map_err(|_| Error::Io(std::io::Error::other("no random source")))?;
    let hex: String = id.iter().map(|b| format!("{b:02x}")).collect();
    Ok(parent.join(format!(".{prefix}-{hex}")))
}

fn is_empty_dir(folder: &Path) -> Result<bool> {
    match fs::symlink_metadata(folder) {
        Ok(m) if m.is_dir() => Ok(fs::read_dir(folder)?.next().is_none()),
        Ok(_) => Ok(false),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn restore_impl(
    archive: &Path,
    folder: &Path,
    passphrase: &str,
    limits: Limits,
    progress: &mut dyn FnMut(Progress),
) -> Result<RestoreReport> {
    if !is_empty_dir(folder)? {
        return Err(Error::FolderNotEmpty);
    }
    let mut stream = open_stream(archive, passphrase)?;
    let created_folder = !folder.exists();
    fs::create_dir_all(folder)?;
    let temp = scratch_dir(folder, "kh-restore")?;
    let mut moved: Vec<OsString> = Vec::new();
    let mut sink = DirSink {
        root: temp.clone(),
        written: Vec::new(),
    };
    let result = fs::create_dir(&temp).map_err(Error::from).and_then(|()| {
        let totals = walk(&mut stream, &limits, &mut sink, progress)?;
        move_into_place(&temp, folder, &mut moved)?;
        Ok(RestoreReport {
            files: totals.files,
            bytes: totals.bytes,
        })
    });
    if result.is_err() {
        remove_written(folder, &moved, &sink.written);
    }
    let _ = fs::remove_dir_all(&temp);
    if result.is_err() && created_folder {
        let _ = fs::remove_dir(folder);
    }
    result
}

/// Removes the files this restore moved in, then any folder that left empty. Other files stay.
fn remove_written(folder: &Path, moved: &[OsString], written: &[PathBuf]) {
    let ours = written.iter().filter(|rel| {
        rel.components()
            .next()
            .is_some_and(|top| moved.iter().any(|m| m == top.as_os_str()))
    });
    for rel in ours {
        // Best effort: the restore has already failed and says so.
        let _ = fs::remove_file(folder.join(rel));
        for parent in rel.ancestors().skip(1) {
            if !parent.as_os_str().is_empty() {
                // Only succeeds on an empty folder.
                let _ = fs::remove_dir(folder.join(parent));
            }
        }
    }
}

/// The checked files sit in `temp`; each project folder moves up one level.
fn move_into_place(temp: &Path, folder: &Path, moved: &mut Vec<OsString>) -> Result<()> {
    for entry in fs::read_dir(temp)? {
        let entry = entry?;
        let target = folder.join(entry.file_name());
        // The folder was empty at the start, so anything here now belongs to someone else.
        if target.symlink_metadata().is_ok() {
            return Err(Error::FolderNotEmpty);
        }
        fs::rename(entry.path(), &target)?;
        moved.push(entry.file_name());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_restore_removes_only_what_it_wrote() {
        let d = tempfile::tempdir().expect("tempdir");
        let project = d.path().join("p");
        fs::create_dir_all(project.join("sub")).expect("dirs");
        fs::write(project.join("sub/ours.txt"), "ours").expect("ours");
        fs::write(project.join("theirs.txt"), "theirs").expect("theirs");
        let written = [
            PathBuf::from("p/sub/ours.txt"),
            PathBuf::from("q/not-moved.txt"),
        ];
        remove_written(d.path(), &[OsString::from("p")], &written);
        assert!(!project.join("sub").exists());
        assert_eq!(
            fs::read_to_string(project.join("theirs.txt")).expect("read"),
            "theirs"
        );
    }
}
