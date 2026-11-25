//! Name checks and safe path resolution inside a project folder.

use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::{Result, StoreError};

/// The hidden working folder inside every project.
pub const WORK_DIR: &str = ".kola-hub";
const MAX_COMPONENT_BYTES: usize = 255;
const MAX_NAME_BYTES: usize = 4096;
const ID_HEX_CHARS: usize = 32;

/// Lowercase hex of `bytes`.
pub(crate) fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from(DIGITS[usize::from(b >> 4)]));
        out.push(char::from(DIGITS[usize::from(b & 0x0f)]));
    }
    out
}

/// A stable opaque id derived from a text key. The key itself never reaches the interface.
pub(crate) fn opaque_id(domain: &str, key: &str) -> String {
    let mut h = Sha256::new();
    h.update(domain.as_bytes());
    h.update([0]);
    h.update(key.as_bytes());
    let mut id = hex(&h.finalize());
    id.truncate(ID_HEX_CHARS);
    id
}

/// Accepts a relative name made only of plain components, with `/` between folders.
pub fn clean_relative(name: &str) -> Result<PathBuf> {
    let bad = name.is_empty()
        || name.len() > MAX_NAME_BYTES
        || name.contains(['\0', '\\', ':', '*', '?', '"', '<', '>', '|'])
        || name.chars().any(char::is_control);
    if bad {
        return Err(StoreError::BadName);
    }
    let path = PathBuf::from(name);
    let mut first = true;
    for part in path.components() {
        let Component::Normal(s) = part else {
            return Err(StoreError::BadName);
        };
        if s.len() > MAX_COMPONENT_BYTES || windows_treats_specially(&s.to_string_lossy()) {
            return Err(StoreError::BadName);
        }
        if first && s.to_string_lossy().eq_ignore_ascii_case(WORK_DIR) {
            return Err(StoreError::BadName);
        }
        first = false;
    }
    if name.ends_with('/') || name.contains("//") {
        return Err(StoreError::BadName);
    }
    Ok(path)
}

/// Windows drops a trailing dot or space and treats device names as devices, whatever the extension.
fn windows_treats_specially(part: &str) -> bool {
    if part.ends_with(['.', ' ']) {
        return true;
    }
    let stem = part
        .split('.')
        .next()
        .unwrap_or(part)
        .trim_end()
        .to_ascii_uppercase();
    let numbered = stem.len() == 4
        && (stem.starts_with("COM") || stem.starts_with("LPT"))
        && stem.ends_with(|c: char| ('1'..='9').contains(&c));
    numbered || ["CON", "PRN", "AUX", "NUL"].contains(&stem.as_str())
}

/// The final name must be one plain component, with no folders.
pub fn clean_single(name: &str) -> Result<String> {
    if name.contains('/') {
        return Err(StoreError::BadName);
    }
    clean_relative(name).map(|_| name.to_owned())
}

/// Joins `rel` to `root` and proves the folder it sits in is really inside `root` once links are
/// followed. The last component is not followed, so a link can still be moved or replaced as itself.
pub fn resolve_new(root: &Path, rel: &Path) -> Result<PathBuf> {
    let full = root.join(rel);
    let parent = full.parent().ok_or(StoreError::BadName)?;
    let real_parent = parent.canonicalize().map_err(|_| StoreError::NotFound)?;
    if !real_parent.starts_with(root) {
        return Err(StoreError::OutsideProject);
    }
    let name = full.file_name().ok_or(StoreError::BadName)?;
    Ok(real_parent.join(name))
}

/// Like [`resolve_new`] but also follows the last component, for reading a file's content.
pub fn resolve_existing(root: &Path, rel: &Path) -> Result<PathBuf> {
    let real = root
        .join(rel)
        .canonicalize()
        .map_err(|_| StoreError::NotFound)?;
    if !real.starts_with(root) {
        return Err(StoreError::OutsideProject);
    }
    Ok(real)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_hostile_names() {
        for bad in [
            "",
            "..",
            "../x",
            "a/../b",
            "/abs",
            "a//b",
            "a/",
            "a\\b",
            "c:x",
            ".kola-hub/x",
            ".KOLA-HUB",
            "a\0b",
            "./a",
        ] {
            assert!(clean_relative(bad).is_err(), "{bad:?} should be refused");
        }
    }

    #[test]
    fn accepts_plain_names() {
        for good in ["a.csv", "sub/a.csv", "name (cleaned).csv", "a..b"] {
            assert!(clean_relative(good).is_ok(), "{good:?} should pass");
        }
    }

    #[test]
    fn ids_are_stable_and_separate_domains() {
        assert_eq!(opaque_id("p", "x"), opaque_id("p", "x"));
        assert_ne!(opaque_id("p", "x"), opaque_id("f", "x"));
        assert_eq!(opaque_id("p", "x").len(), ID_HEX_CHARS);
    }

    #[test]
    fn rejects_names_windows_treats_specially() {
        for bad in [
            "CON",
            "con.csv",
            "sub/NUL",
            "Com1.txt",
            "LPT9",
            "report.",
            "report ",
            "a./b",
            "x?y",
            "x*y",
            ".kola-hub./x",
        ] {
            assert!(clean_relative(bad).is_err(), "{bad:?} should be refused");
        }
        for good in ["console.csv", "COM10", "nullable", "a .csv"] {
            assert!(clean_relative(good).is_ok(), "{good:?} should pass");
        }
    }
}
