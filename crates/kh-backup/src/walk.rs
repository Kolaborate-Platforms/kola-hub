//! Reading the plaintext records. Shared by restore and test so both apply the same checks.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::header::{HEADER_LEN, Header};
use crate::paths::{MAX_PATH_BYTES, PathSet};
use crate::seal::{SealReader, derive_key};
use crate::{Limits, Progress};

pub(crate) const REC_FILE: u8 = 1;
pub(crate) const REC_END: u8 = 0;

/// Where the bytes of each file go. `None` from `open` means check the hash and discard.
pub(crate) trait Sink {
    fn open(&mut self, index: u64, path: &str, size: u64) -> Result<Option<File>>;
    fn done(&mut self, index: u64, hash: [u8; 32]) -> Result<()>;
}

/// What the end record confirmed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Totals {
    pub files: u64,
    pub bytes: u64,
}

/// Opens the archive, checks the header, derives the key and returns the verified stream.
pub(crate) fn open_stream(archive: &Path, passphrase: &str) -> Result<SealReader<File>> {
    let mut file = File::open(archive)?;
    let mut raw = [0u8; HEADER_LEN];
    file.read_exact(&mut raw).map_err(|_| Error::NotABackup)?;
    let header = Header::parse(&raw)?;
    let key = derive_key(passphrase, &header)?;
    Ok(SealReader::new(file, &header, &key)?)
}

fn read_u64<R: Read>(r: &mut R) -> Result<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

fn read_path<R: Read>(r: &mut R) -> Result<String> {
    let mut len = [0u8; 2];
    r.read_exact(&mut len)?;
    let len = usize::from(u16::from_le_bytes(len));
    if len > MAX_PATH_BYTES {
        return Err(Error::UnsafeName("a name that is too long".into()));
    }
    let mut bytes = vec![0u8; len];
    r.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| Error::UnsafeName("a name that is not valid text".into()))
}

/// Copies exactly `size` bytes, hashing them, and returns the hash.
fn copy_body<R: Read>(r: &mut R, size: u64, mut out: Option<File>) -> Result<[u8; 32]> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut left = size;
    while left > 0 {
        let want = usize::try_from(left).unwrap_or(buf.len()).min(buf.len());
        let n = r.read(&mut buf[..want])?;
        if n == 0 {
            return Err(Error::Damaged);
        }
        hasher.update(&buf[..n]);
        if let Some(f) = out.as_mut() {
            f.write_all(&buf[..n]).map_err(Error::Io)?;
        }
        left -= n as u64;
    }
    Ok(hasher.finalize().into())
}

/// Reads every record, checking names, limits, hashes and the end record.
pub(crate) fn walk<R: Read>(
    r: &mut R,
    limits: &Limits,
    sink: &mut dyn Sink,
    progress: &mut dyn FnMut(Progress),
) -> Result<Totals> {
    let mut seen = PathSet::new(limits.max_files);
    let mut bytes = 0u64;
    loop {
        let mut tag = [0u8; 1];
        r.read_exact(&mut tag)?;
        match tag[0] {
            REC_FILE => {
                let path = read_path(r)?;
                seen.add_file(&path)?;
                let size = read_u64(r)?;
                bytes = bytes
                    .checked_add(size)
                    .filter(|b| *b <= limits.max_bytes)
                    .ok_or(Error::TooBig)?;
                let index = seen.count() - 1;
                let out = sink.open(index, &path, size)?;
                let hash = copy_body(r, size, out)?;
                let mut stored = [0u8; 32];
                r.read_exact(&mut stored)?;
                if stored != hash {
                    return Err(Error::Damaged);
                }
                sink.done(index, hash)?;
                progress(Progress {
                    files: seen.count(),
                    bytes,
                });
            }
            REC_END => return finish(r, seen.count(), bytes),
            _ => return Err(Error::Damaged),
        }
    }
}

fn finish<R: Read>(r: &mut R, files: u64, bytes: u64) -> Result<Totals> {
    if read_u64(r)? != files || read_u64(r)? != bytes {
        return Err(Error::Damaged);
    }
    // Nothing may follow, and the last chunk only verifies once the stream reports its end.
    let mut extra = [0u8; 1];
    if r.read(&mut extra)? != 0 {
        return Err(Error::Damaged);
    }
    Ok(Totals { files, bytes })
}
