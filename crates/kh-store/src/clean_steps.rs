//! The saved Clean step list of each file. The text is the tool's own JSON; this crate does not read it.

use rusqlite::{OptionalExtension, params};

use crate::error::{Result, StoreError};
use crate::project::{FileId, Project, now_unix};

/// Largest step list text accepted, so a runaway list cannot fill the database.
pub const MAX_STEPS_BYTES: usize = 1024 * 1024;

impl Project {
    /// Saves the step list text for a file, replacing the earlier one.
    pub fn save_clean_steps(&self, file: &FileId, steps: &str) -> Result<()> {
        if steps.len() > MAX_STEPS_BYTES {
            return Err(StoreError::Conflict(
                "This list of steps is too long to save.",
            ));
        }
        self.rel_of(file)?;
        self.connection().execute(
            "INSERT OR REPLACE INTO clean_steps (file_id, steps, updated_at) VALUES (?1, ?2, ?3)",
            params![file.as_str(), steps, now_unix()],
        )?;
        Ok(())
    }

    /// The saved step list text of a file, or `None` when none was saved.
    pub fn load_clean_steps(&self, file: &FileId) -> Result<Option<String>> {
        self.rel_of(file)?;
        Ok(self
            .connection()
            .query_row(
                "SELECT steps FROM clean_steps WHERE file_id = ?1",
                params![file.as_str()],
                |r| r.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn project() -> (tempfile::TempDir, Project, FileId) {
        let d = tempfile::tempdir().expect("tempdir");
        fs::write(d.path().join("a.csv"), "x\n1\n").expect("file");
        let p = Project::open_or_create(d.path()).expect("open");
        let id = p.register_file("a.csv").expect("reg");
        (d, p, id)
    }

    #[test]
    fn steps_are_saved_replaced_and_survive_a_reopen() {
        let (d, p, id) = project();
        assert_eq!(p.load_clean_steps(&id).expect("none"), None);
        p.save_clean_steps(&id, "one").expect("save");
        p.save_clean_steps(&id, "two").expect("replace");
        drop(p);
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(
            p.load_clean_steps(&id).expect("load").as_deref(),
            Some("two")
        );
    }

    #[test]
    fn an_unknown_file_and_an_oversized_list_are_refused() {
        let (_d, p, id) = project();
        let other = tempfile::tempdir().expect("other");
        fs::write(other.path().join("b.csv"), "x").expect("file");
        let stranger = Project::open_or_create(other.path())
            .expect("open")
            .register_file("b.csv")
            .expect("reg");
        assert!(matches!(
            p.save_clean_steps(&stranger, "x"),
            Err(StoreError::NotFound)
        ));
        assert!(matches!(
            p.load_clean_steps(&stranger),
            Err(StoreError::NotFound)
        ));
        let big = "x".repeat(MAX_STEPS_BYTES + 1);
        assert!(matches!(
            p.save_clean_steps(&id, &big),
            Err(StoreError::Conflict(_))
        ));
    }

    #[test]
    fn an_older_project_gains_the_table_when_opened() {
        let (d, p, _id) = project();
        p.connection()
            .execute_batch("DROP TABLE clean_steps; PRAGMA user_version = 1;")
            .expect("downgrade");
        drop(p);
        let p = Project::open_or_create(d.path()).expect("reopen");
        let id = p.register_file("a.csv").expect("reg");
        p.save_clean_steps(&id, "x").expect("save");
    }
}
