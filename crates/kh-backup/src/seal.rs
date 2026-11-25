//! Key derivation and the chunked, authenticated stream (ChaCha20-Poly1305 in STREAM mode).

use std::io::{self, Read, Write};

use aead_stream::{DecryptorBE32, EncryptorBE32};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::Payload;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit};
use zeroize::Zeroizing;

use crate::error::{Error, Result};
use crate::header::{CHUNK, HEADER_LEN, Header};

const TAG: usize = 16;
const CHUNK_CT: usize = CHUNK + TAG;

pub(crate) fn derive_key(passphrase: &str, header: &Header) -> Result<Zeroizing<[u8; 32]>> {
    let k = header.kdf;
    let params =
        Params::new(k.memory_kib(), k.passes(), k.lanes(), Some(32)).map_err(|_| Error::Damaged)?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), &header.salt, &mut key[..])
        .map_err(|_| Error::Io(io::Error::other("key derivation failed")))?;
    Ok(key)
}

fn cipher(key: &Zeroizing<[u8; 32]>) -> ChaCha20Poly1305 {
    ChaCha20Poly1305::new((&**key).into())
}

fn bad() -> io::Error {
    io::Error::from(io::ErrorKind::InvalidData)
}

/// Encrypts whatever is written to it. Call [`SealWriter::finish`] to write the last chunk.
pub(crate) struct SealWriter<W: Write> {
    inner: W,
    enc: Option<EncryptorBE32<ChaCha20Poly1305>>,
    aad: [u8; HEADER_LEN],
    buf: Zeroizing<Vec<u8>>,
}

impl<W: Write> SealWriter<W> {
    /// Writes the header, then is ready for plaintext.
    pub fn new(mut inner: W, header: &Header, key: &Zeroizing<[u8; 32]>) -> io::Result<Self> {
        inner.write_all(&header.raw)?;
        let enc = EncryptorBE32::from_aead(cipher(key), &header.nonce_prefix.into());
        Ok(Self {
            inner,
            enc: Some(enc),
            aad: header.raw,
            buf: Zeroizing::new(Vec::with_capacity(CHUNK)),
        })
    }

    fn emit_full(&mut self) -> io::Result<()> {
        let enc = self.enc.as_mut().ok_or_else(bad)?;
        let ct = enc
            .encrypt_next(Payload {
                msg: &self.buf,
                aad: &self.aad,
            })
            .map_err(|_| bad())?;
        self.buf.clear();
        self.inner.write_all(&ct)
    }

    /// Seals the remaining bytes as the last chunk and returns the inner writer.
    pub fn finish(mut self) -> io::Result<W> {
        let enc = self.enc.take().ok_or_else(bad)?;
        let ct = enc
            .encrypt_last(Payload {
                msg: &self.buf,
                aad: &self.aad,
            })
            .map_err(|_| bad())?;
        self.inner.write_all(&ct)?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for SealWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        // A full chunk is only sealed once more data follows, so the last one can carry the end flag.
        if self.buf.len() == CHUNK && !data.is_empty() {
            self.emit_full()?;
        }
        let room = CHUNK - self.buf.len();
        let n = room.min(data.len());
        self.buf.extend_from_slice(&data[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Decrypts and authenticates as it is read. Truncation, extra bytes, reordering and damage
/// all surface as `InvalidData`, and the end of the stream is only reported after the last chunk verifies.
pub(crate) struct SealReader<R: Read> {
    inner: R,
    dec: Option<DecryptorBE32<ChaCha20Poly1305>>,
    aad: [u8; HEADER_LEN],
    pending: Vec<u8>,
    plain: Zeroizing<Vec<u8>>,
    pos: usize,
    failed: bool,
}

impl<R: Read> SealReader<R> {
    /// `header` must already be parsed from the first bytes of `inner`.
    pub fn new(mut inner: R, header: &Header, key: &Zeroizing<[u8; 32]>) -> io::Result<Self> {
        let dec = DecryptorBE32::from_aead(cipher(key), &header.nonce_prefix.into());
        let pending = read_full(&mut inner)?;
        Ok(Self {
            inner,
            dec: Some(dec),
            aad: header.raw,
            pending,
            plain: Zeroizing::new(Vec::new()),
            pos: 0,
            failed: false,
        })
    }

    fn next_chunk(&mut self) -> io::Result<bool> {
        let Some(mut dec) = self.dec.take() else {
            return Ok(false);
        };
        let ahead = read_full(&mut self.inner).inspect_err(|_| self.failed = true)?;
        let payload = Payload {
            msg: &self.pending,
            aad: &self.aad,
        };
        let plain = if ahead.is_empty() {
            dec.decrypt_last(payload)
        } else {
            let p = dec.decrypt_next(payload);
            self.dec = Some(dec);
            p
        };
        let Ok(plain) = plain else {
            self.failed = true;
            return Err(bad());
        };
        self.plain = Zeroizing::new(plain);
        self.pos = 0;
        self.pending = ahead;
        Ok(true)
    }
}

fn read_full<R: Read>(r: &mut R) -> io::Result<Vec<u8>> {
    let mut v = vec![0u8; CHUNK_CT];
    let mut n = 0;
    while n < CHUNK_CT {
        match r.read(&mut v[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
    v.truncate(n);
    Ok(v)
}

impl<R: Read> Read for SealReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.failed {
            return Err(bad());
        }
        while self.pos >= self.plain.len() {
            if !self.next_chunk()? {
                return Ok(0);
            }
        }
        let n = out.len().min(self.plain.len() - self.pos);
        out[..n].copy_from_slice(&self.plain[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}
