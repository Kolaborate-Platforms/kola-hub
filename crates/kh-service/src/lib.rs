//! What every tool can be asked to do, as plain functions over serialisable types. No window code.
//! Nothing returned from here holds a file path.

#![forbid(unsafe_code)]

pub mod backup;
pub mod clean;
#[cfg(test)]
mod contract_tests;
pub mod data_dir;
pub mod error;
pub mod projects;
pub mod shell;

pub use error::{AppError, AppResult};
pub use shell::Shell;
