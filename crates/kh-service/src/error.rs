//! Failures that cross to the interface: a stable code and a plain English message, never a path.

use std::fmt;

use kh_store::StoreError;
use serde::Serialize;

/// What every command returns when it fails.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AppError {
    /// Stable text the interface can switch on.
    pub code: String,
    /// A sentence for a person.
    pub message: String,
}

impl AppError {
    /// An error with a code and a message.
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            message: message.into(),
        }
    }

    /// Something went wrong inside the app that the person cannot fix.
    pub fn internal() -> Self {
        Self::new(
            "internal",
            "Something went wrong inside Kola Hub. Nothing was changed. Please try again.",
        )
    }

    /// The thing asked for is not there, or does not belong to the project.
    pub fn not_found() -> Self {
        Self::new("not-found", "That item could not be found.")
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

impl From<StoreError> for AppError {
    fn from(error: StoreError) -> Self {
        Self::new(error.code(), error.message())
    }
}

impl From<kh_backup::Error> for AppError {
    fn from(error: kh_backup::Error) -> Self {
        use kh_backup::Error as E;
        let code = match &error {
            E::PassphraseTooShort => "passphrase-too-short",
            E::NoSources => "nothing-to-back-up",
            E::DestinationExists => "destination-exists",
            E::FolderNotEmpty => "folder-not-empty",
            E::NotABackup => "not-a-backup",
            E::NewerVersion => "newer-version",
            E::Damaged => "wrong-passphrase-or-damaged",
            _ => "backup",
        };
        // This one names a file inside the backup. Only its own name may cross to the interface.
        let message = match &error {
            E::FileChanged(path) => {
                let name = path.rsplit('/').next().unwrap_or(path);
                E::FileChanged(name.to_owned()).to_string()
            }
            _ => error.to_string(),
        };
        Self::new(code, message)
    }
}

impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        Self::new(
            "io",
            "A file could not be read or saved. Check that the disk is not full and the folder is not read only.",
        )
    }
}

/// Result alias for command logic.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_file_is_named_without_its_folders() {
        let error = AppError::from(kh_backup::Error::FileChanged(
            "shop/private/sales.csv".into(),
        ));
        assert!(error.message.contains("sales.csv"));
        assert!(!error.message.contains("private"));
    }
}
