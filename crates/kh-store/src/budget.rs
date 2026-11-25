//! Storage budget: how much working state a project holds against its limit. Nothing is deleted automatically.

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::project::Project;

/// Default limit for a project's working state: 5 GB.
pub const DEFAULT_LIMIT_BYTES: u64 = 5 * 1024 * 1024 * 1024;
const WARN_PERCENT: u128 = 80;
const PERCENT: u128 = 100;

/// How full the budget is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BudgetStatus {
    /// Under 80 percent of the limit.
    Ok,
    /// At or past 80 percent of the limit.
    Warn,
    /// Past the limit.
    Over,
}

/// Usage and limit of one project's `.kola-hub/` folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageBudget {
    /// Bytes used by the database, stored content and Recently deleted.
    pub used_bytes: u64,
    /// The limit the person set.
    pub limit_bytes: u64,
    /// Where usage stands against the limit.
    pub status: BudgetStatus,
}

/// Classifies `used` against `limit`.
pub fn classify(used: u64, limit: u64) -> BudgetStatus {
    if used > limit {
        BudgetStatus::Over
    } else if u128::from(used) * PERCENT >= u128::from(limit) * WARN_PERCENT {
        BudgetStatus::Warn
    } else {
        BudgetStatus::Ok
    }
}

/// Total size of a file or folder tree. Links count as links and are never followed.
pub(crate) fn tree_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut pending = vec![path.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(meta) = fs::symlink_metadata(&next) else {
            continue;
        };
        if !meta.is_dir() {
            total = total.saturating_add(meta.len());
            continue;
        }
        if let Ok(entries) = fs::read_dir(&next) {
            pending.extend(entries.flatten().map(|e| e.path()));
        }
    }
    total
}

impl Project {
    /// Usage against `limit_bytes`. Only working state counts; the person's own files do not.
    pub fn storage_budget(&self, limit_bytes: u64) -> Result<StorageBudget> {
        let used_bytes = tree_size(&self.work);
        Ok(StorageBudget {
            used_bytes,
            limit_bytes,
            status: classify(used_bytes, limit_bytes),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thresholds() {
        assert_eq!(classify(0, 100), BudgetStatus::Ok);
        assert_eq!(classify(79, 100), BudgetStatus::Ok);
        assert_eq!(classify(80, 100), BudgetStatus::Warn);
        assert_eq!(classify(100, 100), BudgetStatus::Warn);
        assert_eq!(classify(101, 100), BudgetStatus::Over);
        assert_eq!(classify(1, 0), BudgetStatus::Over);
        assert_eq!(classify(u64::MAX, u64::MAX), BudgetStatus::Warn);
    }

    #[test]
    fn counts_blobs_and_trash_and_deletes_nothing() {
        let d = tempfile::tempdir().expect("tempdir");
        fs::write(d.path().join("a.csv"), "x").expect("file");
        let p = Project::open_or_create(d.path()).expect("open");
        let id = p.register_file("a.csv").expect("reg");
        let before = p.storage_budget(DEFAULT_LIMIT_BYTES).expect("budget");
        p.store_blob(&id, "snap", &vec![7u8; 10_000][..])
            .expect("blob");
        p.trash_file(&id).expect("trash");
        let after = p.storage_budget(10_000).expect("budget");
        assert!(after.used_bytes >= before.used_bytes + 10_000);
        assert_eq!(after.status, BudgetStatus::Over);
        assert_eq!(before.status, BudgetStatus::Ok);
        assert_eq!(
            p.list_trash().expect("list").len(),
            1,
            "going over the limit deletes nothing"
        );
        assert_eq!(p.blobs().list().expect("blobs").len(), 1);
    }
}
