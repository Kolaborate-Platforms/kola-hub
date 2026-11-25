//! The archive header: fixed size, checked before any key is derived.

use crate::error::{Error, Result};

pub(crate) const MAGIC: &[u8; 4] = b"KHBK";
pub(crate) const VERSION: u8 = 1;
pub(crate) const HEADER_LEN: usize = 44;
/// Plaintext bytes in every chunk except possibly the last.
pub(crate) const CHUNK: usize = 65_536;

/// Argon2id cost. Values outside the allowed range cannot be built, so no caller can weaken it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    memory_kib: u32,
    passes: u32,
    lanes: u32,
}

impl KdfParams {
    /// Memory range in KiB.
    pub const MEMORY_RANGE: (u32, u32) = (19_456, 262_144);
    /// Pass range.
    pub const PASS_RANGE: (u32, u32) = (1, 10);
    /// Lane range.
    pub const LANE_RANGE: (u32, u32) = (1, 4);
    /// The cheapest cost the format allows. Used by tests to stay fast.
    pub const CHEAPEST: Self = Self {
        memory_kib: 19_456,
        passes: 1,
        lanes: 1,
    };

    /// Checks the three numbers against the format limits.
    pub fn new(memory_kib: u32, passes: u32, lanes: u32) -> Option<Self> {
        let ok = |v: u32, (lo, hi): (u32, u32)| (lo..=hi).contains(&v);
        (ok(memory_kib, Self::MEMORY_RANGE)
            && ok(passes, Self::PASS_RANGE)
            && ok(lanes, Self::LANE_RANGE))
        .then_some(Self {
            memory_kib,
            passes,
            lanes,
        })
    }

    pub(crate) fn memory_kib(self) -> u32 {
        self.memory_kib
    }
    pub(crate) fn passes(self) -> u32 {
        self.passes
    }
    pub(crate) fn lanes(self) -> u32 {
        self.lanes
    }
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 65_536,
            passes: 3,
            lanes: 1,
        }
    }
}

/// A parsed, validated header and its exact bytes (the associated data of every chunk).
#[derive(Clone, Debug)]
pub(crate) struct Header {
    pub kdf: KdfParams,
    pub salt: [u8; 16],
    pub nonce_prefix: [u8; 7],
    pub raw: [u8; HEADER_LEN],
}

impl Header {
    /// Builds a header with fresh random salt and nonce prefix.
    pub fn generate(kdf: KdfParams) -> Result<Self> {
        let mut salt = [0u8; 16];
        let mut nonce_prefix = [0u8; 7];
        getrandom::fill(&mut salt)
            .map_err(|_| Error::Io(std::io::Error::other("no random source")))?;
        getrandom::fill(&mut nonce_prefix)
            .map_err(|_| Error::Io(std::io::Error::other("no random source")))?;
        Ok(Self::assemble(kdf, salt, nonce_prefix))
    }

    pub(crate) fn assemble(kdf: KdfParams, salt: [u8; 16], nonce_prefix: [u8; 7]) -> Self {
        let mut raw = [0u8; HEADER_LEN];
        raw[0..4].copy_from_slice(MAGIC);
        raw[4] = VERSION;
        raw[5..9].copy_from_slice(&kdf.memory_kib.to_le_bytes());
        raw[9..13].copy_from_slice(&kdf.passes.to_le_bytes());
        raw[13..17].copy_from_slice(&kdf.lanes.to_le_bytes());
        raw[17..33].copy_from_slice(&salt);
        raw[33..40].copy_from_slice(&nonce_prefix);
        raw[40..44].copy_from_slice(&(CHUNK as u32).to_le_bytes());
        Self {
            kdf,
            salt,
            nonce_prefix,
            raw,
        }
    }

    /// Refuses a bad header before any key is derived.
    pub fn parse(raw: &[u8; HEADER_LEN]) -> Result<Self> {
        if &raw[0..4] != MAGIC {
            return Err(Error::NotABackup);
        }
        if raw[4] != VERSION {
            return Err(Error::NewerVersion);
        }
        let num = |at: usize| u32::from_le_bytes([raw[at], raw[at + 1], raw[at + 2], raw[at + 3]]);
        let kdf = KdfParams::new(num(5), num(9), num(13)).ok_or(Error::Damaged)?;
        if num(40) as usize != CHUNK {
            return Err(Error::Damaged);
        }
        let mut salt = [0u8; 16];
        salt.copy_from_slice(&raw[17..33]);
        let mut nonce_prefix = [0u8; 7];
        nonce_prefix.copy_from_slice(&raw[33..40]);
        Ok(Self {
            kdf,
            salt,
            nonce_prefix,
            raw: *raw,
        })
    }
}
