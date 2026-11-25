//! Errors that reach the interface as a stable code plus a plain English message.

use std::fmt;
use std::io;

use serde::{Serialize, Serializer};

/// Everything `kh-store` can refuse or fail at.
#[derive(Debug)]
pub enum StoreError {
    /// A file or folder could not be read or written.
    Io(io::Error),
    /// The project database reported a problem.
    Db(rusqlite::Error),
    /// The project was made by a newer version of the app.
    NewerVersion {
        /// Version found in the project.
        found: u32,
        /// Newest version this build understands.
        supported: u32,
    },
    /// The project database is not readable. It is left exactly as it was.
    DatabaseDamaged,
    /// A stored blob does not match its name.
    BlobCorrupt,
    /// A name that is empty, absolute, uses `..` or otherwise cannot be a plain file name.
    BadName,
    /// The path would leave the project folder, for example through a link.
    OutsideProject,
    /// The project, file or trash entry does not exist.
    NotFound,
    /// The call was valid but cannot be done now, for example a missing trash payload.
    Conflict(&'static str),
}

/// Result alias used across the crate.
pub type Result<T> = std::result::Result<T, StoreError>;

impl StoreError {
    /// A stable code the interface can switch on.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io",
            Self::Db(_) => "database",
            Self::NewerVersion { .. } => "newer-version",
            Self::DatabaseDamaged => "database-damaged",
            Self::BlobCorrupt => "blob-corrupt",
            Self::BadName => "bad-name",
            Self::OutsideProject => "outside-project",
            Self::NotFound => "not-found",
            Self::Conflict(_) => "conflict",
        }
    }

    /// A message for a person. It never contains a file path.
    pub fn message(&self) -> String {
        match self {
            Self::Io(_) => "A file could not be read or saved. Check that the disk is not full and the folder is not read only.".into(),
            Self::Db(_) => "The project's saved work could not be updated. Nothing was changed.".into(),
            Self::NewerVersion { .. } => {
                "This project was made with a newer version of Kola Hub. Update the app to open it. Nothing was changed.".into()
            }
            Self::DatabaseDamaged => {
                "The saved work in this project is damaged. Your own files are safe and have not been touched.".into()
            }
            Self::BlobCorrupt => "A stored copy of a file does not match what was saved, so it was not used.".into(),
            Self::BadName => "That name cannot be used. Use a plain file name without folders or dots at the start.".into(),
            Self::OutsideProject => "That file is outside the project folder, so it was left alone.".into(),
            Self::NotFound => "That item could not be found.".into(),
            Self::Conflict(why) => (*why).into(),
        }
    }
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Db(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        use rusqlite::ErrorCode::{DatabaseCorrupt, NotADatabase};
        match e.sqlite_error_code() {
            Some(NotADatabase | DatabaseCorrupt) => Self::DatabaseDamaged,
            _ => Self::Db(e),
        }
    }
}

#[derive(Serialize)]
struct Wire<'a> {
    code: &'a str,
    message: String,
}

impl Serialize for StoreError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        Wire {
            code: self.code(),
            message: self.message(),
        }
        .serialize(s)
    }
}
