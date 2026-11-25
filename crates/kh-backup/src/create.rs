//! Making a backup.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::header::{Header, KdfParams};
use crate::paths::{PathSet, check_part};
use crate::seal::{SealWriter, derive_key};
use crate::walk::{REC_END, REC_FILE};
use crate::{CreateReport, Limits, Progress, Source};

/// Leftover temporary files from `kh_store::atomic`.
const TEMP_SUFFIX: &str = ".craft_tmp";

struct Entry {
    path: String,
    abs: PathBuf,
}

struct Scan {
    entries: Vec<Entry>,
    skipped: Vec<String>,
    bytes: u64,
}

fn is_link_like(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // A junction or other reparse point.
        return meta.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    false
}

fn name_text(name: &std::ffi::OsStr) -> Result<&str> {
    name.to_str()
        .ok_or_else(|| Error::UnsafeName("a name that is not valid text".into()))
}

/// Lists every regular file below the sources, in a stable order, applying the same name rules as restore.
fn scan(sources: &[Source], limits: &Limits) -> Result<Scan> {
    let mut names = PathSet::new(limits.max_files);
    let mut out = Scan {
        entries: Vec::new(),
        skipped: Vec::new(),
        bytes: 0,
    };
    for source in sources {
        check_part(&source.label)?;
        if source.label.contains('/') {
            return Err(Error::UnsafeName("a project name with a slash".into()));
        }
        let mut stack = vec![(source.folder.clone(), source.label.clone())];
        while let Some((dir, prefix)) = stack.pop() {
            scan_dir(&dir, &prefix, &mut stack, &mut names, &mut out)?;
            // Refuse now: restore would reject a larger backup after it had been trusted.
            if out.bytes > limits.max_bytes {
                return Err(Error::TooBig);
            }
        }
    }
    Ok(out)
}

fn scan_dir(
    dir: &Path,
    prefix: &str,
    stack: &mut Vec<(PathBuf, String)>,
    names: &mut PathSet,
    out: &mut Scan,
) -> Result<()> {
    let mut children = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    children.sort_by_key(|e| e.file_name());
    for child in children {
        let file_name = child.file_name();
        let name = name_text(&file_name)?;
        let path = format!("{prefix}/{name}");
        let meta = fs::symlink_metadata(child.path())?;
        if is_link_like(&meta) {
            out.skipped.push(path);
        } else if meta.is_dir() {
            stack.push((child.path(), path));
        } else if meta.is_file() && !name.ends_with(TEMP_SUFFIX) {
            names.add_file(&path)?;
            out.bytes = out.bytes.saturating_add(meta.len());
            out.entries.push(Entry {
                path,
                abs: child.path(),
            });
        } else {
            // Everything left out is listed, leftover temporary files too.
            out.skipped.push(path);
        }
    }
    Ok(())
}

/// The name was checked during the scan. Make sure it was not swapped for a link before the open.
fn still_the_scanned_file(path: &Path, opened: &fs::Metadata) -> Result<bool> {
    let named = fs::symlink_metadata(path)?;
    if is_link_like(&named) {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        Ok(named.dev() == opened.dev() && named.ino() == opened.ino())
    }
    #[cfg(not(unix))]
    Ok(opened.is_file())
}

fn write_file_record<W: Write>(w: &mut W, entry: &Entry) -> Result<u64> {
    let changed = || Error::FileChanged(entry.path.clone());
    let mut file = File::open(&entry.abs)?;
    let meta = file.metadata()?;
    if !meta.is_file() || !still_the_scanned_file(&entry.abs, &meta)? {
        return Err(changed());
    }
    let size = meta.len();
    let path_len = u16::try_from(entry.path.len())
        .map_err(|_| Error::UnsafeName("a name that is too long".into()))?;
    w.write_all(&[REC_FILE])?;
    w.write_all(&path_len.to_le_bytes())?;
    w.write_all(entry.path.as_bytes())?;
    w.write_all(&size.to_le_bytes())?;

    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut left = size;
    while left > 0 {
        let want = usize::try_from(left).unwrap_or(buf.len()).min(buf.len());
        let n = file.read(&mut buf[..want])?;
        if n == 0 {
            return Err(changed());
        }
        hasher.update(&buf[..n]);
        w.write_all(&buf[..n])?;
        left -= n as u64;
    }
    let mut probe = [0u8; 1];
    let rewritten = file.metadata()?.modified().ok() != meta.modified().ok();
    if rewritten || file.read(&mut probe)? != 0 {
        return Err(changed());
    }
    w.write_all(&hasher.finalize())?;
    Ok(size)
}

fn write_archive<W: Write>(
    w: &mut W,
    entries: &[Entry],
    progress: &mut dyn FnMut(Progress),
) -> Result<(u64, u64)> {
    let mut bytes = 0u64;
    for (i, entry) in entries.iter().enumerate() {
        bytes += write_file_record(w, entry)?;
        progress(Progress {
            files: i as u64 + 1,
            bytes,
        });
    }
    let files = entries.len() as u64;
    w.write_all(&[REC_END])?;
    w.write_all(&files.to_le_bytes())?;
    w.write_all(&bytes.to_le_bytes())?;
    Ok((files, bytes))
}

pub(crate) fn create_impl(
    sources: &[Source],
    destination: &Path,
    passphrase: &str,
    kdf: KdfParams,
    progress: &mut dyn FnMut(Progress),
) -> Result<CreateReport> {
    if passphrase.chars().count() < 8 {
        return Err(Error::PassphraseTooShort);
    }
    if sources.is_empty() {
        return Err(Error::NoSources);
    }
    if destination.symlink_metadata().is_ok() {
        return Err(Error::DestinationExists);
    }
    let found = scan(sources, &Limits::default())?;
    let header = Header::generate(kdf)?;
    let key = derive_key(passphrase, &header)?;

    let mut failure: Option<Error> = None;
    let mut totals = (0, 0);
    let written = kh_store::atomic::write_new_with(destination, |file| {
        let run = SealWriter::new(&mut *file, &header, &key)
            .map_err(Error::Io)
            .and_then(|mut sealed| {
                totals = write_archive(&mut sealed, &found.entries, progress)?;
                sealed.finish()?;
                Ok(())
            });
        run.map_err(|e| {
            failure = Some(e);
            io::Error::other("backup not written")
        })
    });
    match (written, failure) {
        (_, Some(e)) => Err(e),
        (Err(e), None) if e.kind() == io::ErrorKind::AlreadyExists => Err(Error::DestinationExists),
        (Err(e), None) => Err(Error::Io(e)),
        (Ok(()), None) => Ok(CreateReport {
            files: totals.0,
            bytes: totals.1,
            skipped: found.skipped,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_project_larger_than_restore_accepts_is_refused_before_writing() {
        let d = tempfile::tempdir().expect("tempdir");
        fs::write(d.path().join("a.csv"), "123456").expect("file");
        let source = Source {
            label: "p".into(),
            folder: d.path().to_path_buf(),
        };
        let limits = Limits {
            max_bytes: 5,
            ..Limits::default()
        };
        assert!(matches!(scan(&[source], &limits), Err(Error::TooBig)));
    }
}
