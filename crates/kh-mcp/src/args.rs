//! Reads tool arguments. Every failure is the same plain message that names the argument.

use kh_service::{AppError, AppResult};
use kh_store::{FileId, ProjectId, TrashId};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

/// Most rows one call may return.
pub(crate) const MAX_ROWS: usize = 100;
const DEFAULT_ROWS: usize = 20;

pub(crate) struct Args<'a>(pub(crate) &'a Map<String, Value>);

fn bad(key: &str) -> AppError {
    AppError::new(
        "bad-arguments",
        format!("The argument \"{key}\" is missing or is not a valid value."),
    )
}

impl Args<'_> {
    fn value<T: DeserializeOwned>(&self, key: &str) -> AppResult<T> {
        let value = self.0.get(key).ok_or_else(|| bad(key))?;
        serde_json::from_value(value.clone()).map_err(|_| bad(key))
    }

    pub(crate) fn text(&self, key: &str) -> AppResult<String> {
        self.value(key)
    }

    pub(crate) fn project(&self) -> AppResult<ProjectId> {
        self.value("projectId")
    }

    pub(crate) fn file(&self, key: &str) -> AppResult<FileId> {
        self.value(key)
    }

    pub(crate) fn trash(&self) -> AppResult<TrashId> {
        self.value("trashId")
    }

    pub(crate) fn step(&self) -> AppResult<kh_clean::Step> {
        self.value("step")
    }

    pub(crate) fn index(&self, key: &str) -> AppResult<usize> {
        self.value(key)
    }

    /// First row and number of rows, with the count held to the most one call may return.
    pub(crate) fn window(&self) -> AppResult<(usize, usize)> {
        let start = if self.0.contains_key("start") {
            self.index("start")?
        } else {
            0
        };
        let count = if self.0.contains_key("count") {
            self.index("count")?
        } else {
            DEFAULT_ROWS
        };
        Ok((start, count.min(MAX_ROWS)))
    }
}
