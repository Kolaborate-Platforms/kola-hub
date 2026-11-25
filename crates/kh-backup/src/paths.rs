//! Which names a backup may hold. The same rules apply when making and when restoring.

use std::collections::HashSet;

use crate::error::{Error, Result};

pub(crate) const MAX_PARTS: usize = 32;
pub(crate) const MAX_PART_BYTES: usize = 255;
/// The longest path that can pass `check_path`; longer ones are refused without reading them.
pub(crate) const MAX_PATH_BYTES: usize = MAX_PARTS * (MAX_PART_BYTES + 1);

const FORBIDDEN: &[char] = &['\\', ':', '*', '?', '"', '<', '>', '|'];

fn unsafe_name(why: &str) -> Error {
    Error::UnsafeName(why.to_owned())
}

fn is_device_name(part: &str) -> bool {
    let stem = part
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_ascii_uppercase();
    match stem.as_str() {
        "CON" | "PRN" | "AUX" | "NUL" => true,
        s => {
            let numbered = s.strip_prefix("COM").or_else(|| s.strip_prefix("LPT"));
            matches!(numbered, Some(n) if n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
        }
    }
}

/// One folder or file name.
pub(crate) fn check_part(part: &str) -> Result<()> {
    if part.is_empty() || part == "." || part == ".." {
        return Err(unsafe_name("an empty or relative name"));
    }
    if part.len() > MAX_PART_BYTES {
        return Err(unsafe_name("a name that is too long"));
    }
    if part.ends_with('.') || part.ends_with(' ') {
        return Err(unsafe_name("a name that ends in a dot or space"));
    }
    if part
        .chars()
        .any(|c| c.is_control() || FORBIDDEN.contains(&c))
    {
        return Err(unsafe_name("a name with a character that is not allowed"));
    }
    if is_device_name(part) {
        return Err(unsafe_name("a name that Windows reserves"));
    }
    Ok(())
}

/// A whole `/` separated path.
pub(crate) fn check_path(path: &str) -> Result<Vec<&str>> {
    if path.is_empty() {
        return Err(unsafe_name("an empty name"));
    }
    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() > MAX_PARTS {
        return Err(unsafe_name("folders nested too deeply"));
    }
    for part in &parts {
        check_part(part)?;
    }
    Ok(parts)
}

/// Tracks names already seen, to refuse repeats, case-only differences and file/folder clashes.
pub(crate) struct PathSet {
    files: HashSet<String>,
    dirs: HashSet<String>,
    max_files: u64,
}

impl PathSet {
    pub fn new(max_files: u64) -> Self {
        Self {
            files: HashSet::new(),
            dirs: HashSet::new(),
            max_files,
        }
    }

    pub fn count(&self) -> u64 {
        self.files.len() as u64
    }

    /// Validates `path` and records it as a file.
    pub fn add_file(&mut self, path: &str) -> Result<()> {
        let parts = check_path(path)?;
        if self.files.len() as u64 >= self.max_files {
            return Err(Error::TooManyFiles);
        }
        let clash = || unsafe_name("two names that are the same or differ only by case");
        let mut prefix = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                prefix.push('/');
            }
            prefix.push_str(&part.to_lowercase());
            if i + 1 < parts.len() && self.files.contains(&prefix) {
                return Err(clash());
            }
        }
        if self.files.contains(&prefix) || self.dirs.contains(&prefix) {
            return Err(clash());
        }
        let mut dir = String::new();
        for part in &parts[..parts.len() - 1] {
            if !dir.is_empty() {
                dir.push('/');
            }
            dir.push_str(&part.to_lowercase());
            self.dirs.insert(dir.clone());
        }
        self.files.insert(prefix);
        Ok(())
    }
}
