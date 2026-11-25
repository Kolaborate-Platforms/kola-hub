//! Project folder, project database, content store, trash, atomic writes and storage budget.

// Kept as upstream wrote it, so later updates are a plain diff.
#[rustfmt::skip]
pub mod atomic;
pub mod blobs;
pub mod budget;
pub mod clean_steps;
pub mod error;
pub mod library;
pub mod names;
pub mod project;
pub mod save;
pub mod trash;

pub use blobs::{BlobId, BlobStore, StoredBlob};
pub use budget::{BudgetStatus, DEFAULT_LIMIT_BYTES, StorageBudget};
pub use error::{Result, StoreError};
pub use library::{Library, RecentProject};
pub use project::{FileId, Project, ProjectId, SCHEMA_VERSION};
pub use save::SavedFile;
pub use trash::{RepairReport, TrashId, TrashItem};
