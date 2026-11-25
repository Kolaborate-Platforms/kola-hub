//! Content store keyed by SHA-256 (`blobs/ab/<hash>`), streaming and verified on read.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atomic::sync_file;
use crate::error::{Result, StoreError};
use crate::names::hex;
use crate::project::make_real_dir;

const CHUNK_BYTES: usize = 64 * 1024;
const HASH_HEX_CHARS: usize = 64;
const SHARD_CHARS: usize = 2;
const TMP_DIR: &str = "tmp";

/// What a stored blob is called: its SHA-256 as 64 lowercase hex characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BlobId(String);

impl BlobId {
    /// Accepts only 64 lowercase hex characters, which also rules out path tricks.
    pub fn parse(text: &str) -> Result<Self> {
        let ok = text.len() == HASH_HEX_CHARS
            && text
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
        if ok {
            Ok(Self(text.to_owned()))
        } else {
            Err(StoreError::BadName)
        }
    }

    /// The hash text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What `put` learned about the content it stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredBlob {
    /// The content hash.
    pub id: BlobId,
    /// Size in bytes.
    pub size: u64,
}

/// The blob folder of one project.
#[derive(Debug, Clone)]
pub struct BlobStore {
    dir: PathBuf,
}

impl BlobStore {
    /// Opens the folder, creating it if needed, and removes half written temporary files left by a crash.
    pub fn open(dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&dir)?;
        make_real_dir(&dir.join(TMP_DIR))?;
        let store = Self { dir };
        store.sweep_tmp()?;
        Ok(store)
    }

    fn sweep_tmp(&self) -> Result<()> {
        for entry in fs::read_dir(self.dir.join(TMP_DIR))? {
            let entry = entry?;
            // The entry's own type, so a link is never followed.
            if entry.file_type()?.is_file() {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    /// Where a blob lives.
    pub fn path_of(&self, id: &BlobId) -> PathBuf {
        self.dir.join(&id.0[..SHARD_CHARS]).join(&id.0)
    }

    /// True when a blob file exists. It may still be damaged; [`BlobStore::verify`] checks that.
    pub fn contains(&self, id: &BlobId) -> bool {
        self.path_of(id).is_file()
    }

    /// Streams `reader` into the store. Same content is stored once. A crash leaves either the whole
    /// blob or nothing under its name.
    pub fn put(&self, mut reader: impl Read) -> Result<StoredBlob> {
        let tmp = self.dir.join(TMP_DIR).join(temp_name());
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        let copied =
            copy_hashing(&mut reader, &mut file).and_then(|done| sync_file(&file).map(|()| done));
        drop(file);
        let (hash, size) = match copied {
            Ok(done) => done,
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(e.into());
            }
        };
        let id = BlobId(hash);
        self.place(&tmp, &id)?;
        Ok(StoredBlob { id, size })
    }

    /// Stores a byte slice.
    pub fn put_bytes(&self, bytes: &[u8]) -> Result<StoredBlob> {
        self.put(bytes)
    }

    fn place(&self, tmp: &Path, id: &BlobId) -> Result<()> {
        let dest = self.path_of(id);
        let shard = dest.parent().ok_or(StoreError::BadName)?;
        make_real_dir(shard)?;
        if dest.is_file() && self.verify(id).is_ok() {
            fs::remove_file(tmp)?;
            return Ok(());
        }
        // A damaged copy under this name is replaced in one step, never removed first.
        if let Err(e) = fs::rename(tmp, &dest) {
            let _ = fs::remove_file(tmp);
            return Err(e.into());
        }
        sync_folder(shard);
        Ok(())
    }

    /// Reads the whole blob and checks it matches its name.
    pub fn verify(&self, id: &BlobId) -> Result<u64> {
        let mut file = File::open(self.path_of(id)).map_err(not_found)?;
        let (hash, size) = copy_hashing(&mut file, &mut io::sink())?;
        if hash == id.0 {
            Ok(size)
        } else {
            Err(StoreError::BlobCorrupt)
        }
    }

    /// Checks the blob, then opens it at the start. A blob that does not match its name is refused.
    pub fn open_verified(&self, id: &BlobId) -> Result<File> {
        self.verify(id)?;
        Ok(File::open(self.path_of(id))?)
    }

    /// Streams a verified blob into `out`.
    pub fn copy_to(&self, id: &BlobId, out: &mut impl Write) -> Result<u64> {
        let mut file = self.open_verified(id)?;
        Ok(io::copy(&mut file, out)?)
    }

    /// All blob ids currently on disk, in no particular order. Names that are not hashes are skipped.
    pub fn list(&self) -> Result<Vec<BlobId>> {
        let mut ids = Vec::new();
        for shard in fs::read_dir(&self.dir)? {
            let shard = shard?;
            if shard.file_name() == TMP_DIR || !shard.file_type()?.is_dir() {
                continue;
            }
            for blob in fs::read_dir(shard.path())? {
                if let Ok(id) = BlobId::parse(&blob?.file_name().to_string_lossy()) {
                    ids.push(id);
                }
            }
        }
        Ok(ids)
    }
}

fn not_found(e: io::Error) -> StoreError {
    if e.kind() == io::ErrorKind::NotFound {
        StoreError::NotFound
    } else {
        e.into()
    }
}

fn temp_name() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    format!(
        "{}-{}.part",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}

fn copy_hashing(from: &mut impl Read, to: &mut impl Write) -> io::Result<(String, u64)> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; CHUNK_BYTES];
    let mut size = 0u64;
    loop {
        let n = match from.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        hasher.update(&buf[..n]);
        to.write_all(&buf[..n])?;
        size += n as u64;
    }
    Ok((hex(&hasher.finalize()), size))
}

/// Best effort: a failed folder flush cannot undo a rename that already happened.
fn sync_folder(dir: &Path) {
    #[cfg(unix)]
    if let Ok(f) = File::open(dir) {
        let _ = f.sync_all();
    }
    #[cfg(not(unix))]
    let _ = dir;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, BlobStore) {
        let dir = tempfile::tempdir().expect("tempdir");
        let s = BlobStore::open(dir.path().join("blobs")).expect("open");
        (dir, s)
    }

    #[test]
    fn same_content_is_stored_once_and_reads_back() {
        let (_d, s) = store();
        let a = s.put_bytes(b"hello").expect("put");
        let b = s.put_bytes(b"hello").expect("put again");
        assert_eq!(a, b);
        assert_eq!(a.size, 5);
        assert_eq!(s.list().expect("list").len(), 1);
        let mut out = Vec::new();
        s.copy_to(&a.id, &mut out).expect("read");
        assert_eq!(out, b"hello");
    }

    #[test]
    fn empty_content_is_allowed() {
        let (_d, s) = store();
        let e = s.put_bytes(b"").expect("put");
        assert_eq!(e.size, 0);
        assert!(s.verify(&e.id).is_ok());
    }

    #[test]
    fn a_blob_that_does_not_match_its_name_is_refused_and_can_be_repaired() {
        let (_d, s) = store();
        let a = s.put_bytes(b"good bytes").expect("put");
        fs::write(s.path_of(&a.id), b"evil bytes").expect("damage");
        assert!(matches!(s.verify(&a.id), Err(StoreError::BlobCorrupt)));
        assert!(matches!(
            s.open_verified(&a.id),
            Err(StoreError::BlobCorrupt)
        ));
        s.put_bytes(b"good bytes").expect("put again repairs");
        assert!(s.verify(&a.id).is_ok());
    }

    #[test]
    fn hostile_ids_are_refused() {
        for bad in [
            "../../etc/passwd",
            "ABC",
            "",
            &"g".repeat(64),
            &"A".repeat(64),
        ] {
            assert!(BlobId::parse(bad).is_err());
        }
    }

    #[test]
    fn missing_blob_is_not_found_and_open_sweeps_temp_files() {
        let (d, s) = store();
        let id = BlobId::parse(&"0".repeat(64)).expect("id");
        assert!(matches!(s.verify(&id), Err(StoreError::NotFound)));
        fs::write(d.path().join("blobs/tmp/1-1.part"), b"half").expect("leftover");
        BlobStore::open(d.path().join("blobs")).expect("reopen");
        assert_eq!(
            fs::read_dir(d.path().join("blobs/tmp"))
                .expect("dir")
                .count(),
            0
        );
    }
}
