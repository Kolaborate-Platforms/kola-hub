//! Saving beside the original: `name (cleaned).csv`, then `name (cleaned 2).csv`. Never over it.

use std::fs::{self, File};
use std::io;

use serde::Serialize;

use crate::atomic::write_new_with;
use crate::error::{Result, StoreError};
use crate::names::clean_single;
use crate::project::{FileId, Project, rel_text};
use crate::trash::split_name;

const MAX_NAME_TRIES: u32 = 1000;

/// A file written by [`Project::save_beside`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedFile {
    /// Id of the new file.
    pub id: FileId,
    /// Its name, for display.
    pub name: String,
}

/// `stem (label).ext` for `n` of 1, then `stem (label 2).ext`.
pub(crate) fn numbered_name(stem: &str, label: &str, n: u32, ext: Option<&str>) -> String {
    let tag = if n <= 1 {
        label.to_owned()
    } else {
        format!("{label} {n}")
    };
    match ext {
        Some(ext) => format!("{stem} ({tag}).{ext}"),
        None => format!("{stem} ({tag})"),
    }
}

impl Project {
    /// Writes a new file next to `original`. The bytes come from `write`, which may run again if a
    /// name is taken in the meantime. The original is never opened for writing, and an existing file
    /// is never replaced. A crash leaves no half written file under the final name.
    pub fn save_beside(
        &self,
        original: &FileId,
        label: &str,
        mut write: impl FnMut(&mut File) -> io::Result<()>,
    ) -> Result<SavedFile> {
        let label = clean_single(label)?;
        let source = self.file_parent_and_name(original)?;
        let (dir, name) = source;
        let (stem, ext) = split_name(&name);
        for n in 1..=MAX_NAME_TRIES {
            let candidate = dir.join(numbered_name(stem, &label, n, ext));
            if fs::symlink_metadata(&candidate).is_ok() {
                continue;
            }
            match write_new_with(&candidate, &mut write) {
                Ok(()) => {
                    let rel = rel_text(&self.root, &candidate)?;
                    let id = self.register_file(&rel)?;
                    let name = self.display_name(&id)?;
                    return Ok(SavedFile { id, name });
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.into()),
            }
        }
        Err(StoreError::Conflict(
            "Too many saved copies share that name. Move some away and try again.",
        ))
    }

    fn file_parent_and_name(&self, id: &FileId) -> Result<(std::path::PathBuf, String)> {
        let rel = self.rel_of(id)?;
        let full = crate::names::resolve_new(&self.root, std::path::Path::new(&rel))?;
        let name = full
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or(StoreError::BadName)?;
        let dir = full.parent().ok_or(StoreError::BadName)?.to_path_buf();
        Ok((dir, name))
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn project() -> (tempfile::TempDir, Project, FileId) {
        let d = tempfile::tempdir().expect("tempdir");
        fs::write(d.path().join("data.csv"), "a,b\n1,2\n").expect("file");
        let p = Project::open_or_create(d.path()).expect("open");
        let id = p.register_file("data.csv").expect("reg");
        (d, p, id)
    }

    #[test]
    fn names_count_up_and_the_original_never_changes() {
        let (d, p, id) = project();
        let first = p
            .save_beside(&id, "cleaned", |f| f.write_all(b"v1"))
            .expect("save");
        let second = p
            .save_beside(&id, "cleaned", |f| f.write_all(b"v2"))
            .expect("save");
        let third = p
            .save_beside(&id, "cleaned", |f| f.write_all(b"v3"))
            .expect("save");
        assert_eq!(first.name, "data (cleaned).csv");
        assert_eq!(second.name, "data (cleaned 2).csv");
        assert_eq!(third.name, "data (cleaned 3).csv");
        assert_eq!(
            fs::read_to_string(d.path().join("data.csv")).expect("read"),
            "a,b\n1,2\n"
        );
        assert_eq!(
            fs::read_to_string(d.path().join("data (cleaned 2).csv")).expect("read"),
            "v2"
        );
    }

    #[test]
    fn an_existing_file_with_the_target_name_is_skipped_not_replaced() {
        let (d, p, id) = project();
        fs::write(d.path().join("data (cleaned).csv"), "someone else's").expect("existing");
        let saved = p
            .save_beside(&id, "cleaned", |f| f.write_all(b"mine"))
            .expect("save");
        assert_eq!(saved.name, "data (cleaned 2).csv");
        assert_eq!(
            fs::read_to_string(d.path().join("data (cleaned).csv")).expect("read"),
            "someone else's"
        );
    }

    #[test]
    fn a_failed_write_leaves_nothing_behind() {
        let (d, p, id) = project();
        let err = p
            .save_beside(&id, "cleaned", |_| Err(io::Error::other("disk full")))
            .expect_err("must fail");
        assert!(matches!(err, StoreError::Io(_)));
        let names: Vec<_> = fs::read_dir(d.path())
            .expect("dir")
            .map(|e| e.expect("e").file_name())
            .collect();
        assert_eq!(
            names.len(),
            2,
            "only data.csv and the work folder remain: {names:?}"
        );
    }

    #[test]
    fn hostile_labels_are_refused() {
        let (_d, p, id) = project();
        for bad in ["../x", "a/b", "", "x\\y"] {
            assert!(
                matches!(
                    p.save_beside(&id, bad, |_| Ok(())),
                    Err(StoreError::BadName)
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn files_without_an_extension_and_with_dots_in_names() {
        let d = tempfile::tempdir().expect("tempdir");
        fs::write(d.path().join("notes"), "n").expect("file");
        fs::write(d.path().join("a.b.csv"), "c").expect("file");
        let p = Project::open_or_create(d.path()).expect("open");
        let n = p
            .save_beside(&p.register_file("notes").expect("r"), "cleaned", |f| {
                f.write_all(b"x")
            })
            .expect("save");
        let ab = p
            .save_beside(&p.register_file("a.b.csv").expect("r"), "cleaned", |f| {
                f.write_all(b"x")
            })
            .expect("save");
        assert_eq!(
            (n.name.as_str(), ab.name.as_str()),
            ("notes (cleaned)", "a.b (cleaned).csv")
        );
    }
}
