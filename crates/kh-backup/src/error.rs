//! Errors, in plain language. Messages never contain file paths.

use std::fmt;
use std::io;

/// Everything that can go wrong while making, restoring or testing a backup.
#[derive(Debug)]
pub enum Error {
    /// The passphrase has fewer than 8 characters.
    PassphraseTooShort,
    /// There is nothing to back up.
    NoSources,
    /// The backup file to create already exists.
    DestinationExists,
    /// The folder to restore into has files in it.
    FolderNotEmpty,
    /// The file is not a Kola Hub backup.
    NotABackup,
    /// The backup was made by a newer version.
    NewerVersion,
    /// Wrong passphrase, or the backup is damaged. One message on purpose.
    Damaged,
    /// A name in the backup (or on disk, when making one) is not allowed.
    UnsafeName(String),
    /// More files than allowed.
    TooManyFiles,
    /// More data than the limit allows.
    TooBig,
    /// A file changed while it was being read.
    FileChanged(String),
    /// The disk refused a read or a write.
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PassphraseTooShort => f.write_str("The passphrase needs at least 8 characters."),
            Self::NoSources => f.write_str("There is nothing to back up."),
            Self::DestinationExists => {
                f.write_str("A file with that name already exists. Choose another name.")
            }
            Self::FolderNotEmpty => f.write_str("Choose an empty folder to restore into."),
            Self::NotABackup => f.write_str("This file is not a Kola Hub backup."),
            Self::NewerVersion => {
                f.write_str("This backup was made by a newer version of Kola Hub.")
            }
            Self::Damaged => f.write_str("The passphrase is wrong or the backup is damaged."),
            Self::UnsafeName(why) => write!(
                f,
                "The backup holds a name that cannot be used safely: {why}."
            ),
            Self::TooManyFiles => f.write_str("The backup holds more files than allowed."),
            Self::TooBig => f.write_str("The backup is larger than the size limit."),
            Self::FileChanged(name) => write!(
                f,
                "The file {name} changed while it was being saved. Try again."
            ),
            Self::Io(e) => write!(f, "A file could not be read or written ({:?}).", e.kind()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    /// A damaged stream surfaces as `InvalidData`, which is the one plain message.
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::InvalidData || e.kind() == io::ErrorKind::UnexpectedEof {
            Self::Damaged
        } else {
            Self::Io(e)
        }
    }
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;
