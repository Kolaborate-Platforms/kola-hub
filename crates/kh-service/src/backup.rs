//! Backup commands over `kh-backup`. Paths come from dialogs and stay in Rust.

use std::fs;
use std::path::{Path, PathBuf};

use kh_backup::{Limits, Progress, Source};
use kh_store::ProjectId;
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::shell::Shell;

const SCRATCH_DIR: &str = "backup-check";
const FALLBACK_LABEL: &str = "project";

/// Running totals sent as the `backup-progress` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ProgressView {
    /// Files finished.
    pub files: u64,
    /// Bytes finished.
    pub bytes: u64,
}

/// What a finished backup holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportView {
    /// Files saved.
    pub files: u64,
    /// Bytes saved.
    pub bytes: u64,
    /// Links and special entries left out.
    pub skipped: Vec<String>,
}

/// What a restore wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RestoreReportView {
    /// Files restored.
    pub files: u64,
    /// Bytes restored.
    pub bytes: u64,
}

/// What a backup test checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TestReportView {
    /// Files whose hashes were checked.
    pub files: u64,
    /// Bytes read.
    pub bytes: u64,
    /// Files also written out and compared.
    pub sampled: u64,
}

fn view(progress: Progress) -> ProgressView {
    ProgressView {
        files: progress.files,
        bytes: progress.bytes,
    }
}

/// The last part of an archive path. Folder names do not cross to the interface.
fn file_name(archive_path: &str) -> String {
    archive_path
        .rsplit('/')
        .next()
        .unwrap_or(archive_path)
        .to_owned()
}

/// One label per project: the folder's name, made unique with a number when two share it.
fn sources_for(shell: &Shell, ids: &[ProjectId]) -> AppResult<Vec<Source>> {
    let mut sources: Vec<Source> = Vec::new();
    for id in ids {
        let folder = shell.root_of(id)?;
        let base = folder
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| FALLBACK_LABEL.to_owned());
        let mut label = base.clone();
        let mut n = 1;
        while sources.iter().any(|s| s.label == label) {
            n += 1;
            label = format!("{base} {n}");
        }
        sources.push(Source { label, folder });
    }
    Ok(sources)
}

/// Seals the chosen projects into a new backup file at `destination`.
pub fn backup_create(
    shell: &Shell,
    ids: &[ProjectId],
    passphrase: &str,
    destination: &Path,
    progress: &mut dyn FnMut(ProgressView),
) -> AppResult<CreateReportView> {
    let sources = sources_for(shell, ids)?;
    let report = kh_backup::create(&sources, destination, passphrase, &mut |p| {
        progress(view(p))
    })?;
    Ok(CreateReportView {
        files: report.files,
        bytes: report.bytes,
        skipped: report.skipped.iter().map(|p| file_name(p)).collect(),
    })
}

/// Restores a backup into an empty folder.
pub fn backup_restore(
    archive: &Path,
    folder: &Path,
    passphrase: &str,
    progress: &mut dyn FnMut(ProgressView),
) -> AppResult<RestoreReportView> {
    let report = kh_backup::restore(archive, folder, passphrase, Limits::default(), &mut |p| {
        progress(view(p));
    })?;
    Ok(RestoreReportView {
        files: report.files,
        bytes: report.bytes,
    })
}

/// Checks a backup file end to end. The scratch folder is removed afterwards.
pub fn backup_test(shell: &Shell, archive: &Path, passphrase: &str) -> AppResult<TestReportView> {
    let scratch: PathBuf = shell.data_dir().join(SCRATCH_DIR);
    if scratch.exists() {
        fs::remove_dir_all(&scratch)?;
    }
    fs::create_dir_all(&scratch)?;
    let outcome = kh_backup::test(archive, passphrase, &scratch);
    let cleanup = fs::remove_dir_all(&scratch);
    let report = outcome?;
    cleanup.map_err(|_| AppError::internal())?;
    Ok(TestReportView {
        files: report.files,
        bytes: report.bytes,
        sampled: report.sampled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projects::tests::shell_with_project;

    fn ignore(_: ProgressView) {}

    #[test]
    fn create_test_and_restore_round_trip_without_leaking_paths() {
        let (data, folder, shell, view) = shell_with_project();
        fs::write(folder.path().join("a.csv"), "x\n1\n").expect("a");
        let out = tempfile::tempdir().expect("out");
        let archive = out.path().join("one.khbackup");
        let mut seen = 0;
        let report = backup_create(
            &shell,
            std::slice::from_ref(&view.id),
            "correct horse",
            &archive,
            &mut |_| {
                seen += 1;
            },
        )
        .expect("create");
        assert!(report.files >= 1 && seen >= 1);
        let tested = backup_test(&shell, &archive, "correct horse").expect("test");
        assert!(tested.files >= 1);
        assert!(!data.path().join(SCRATCH_DIR).exists(), "scratch removed");
        let restored = out.path().join("back");
        backup_restore(&archive, &restored, "correct horse", &mut ignore).expect("restore");
        let label = folder.path().file_name().expect("name");
        assert_eq!(
            fs::read_to_string(restored.join(label).join("a.csv")).expect("copy"),
            "x\n1\n"
        );
        let text = serde_json::to_string(&report).expect("json");
        assert!(!text.contains(&*folder.path().to_string_lossy()));
    }

    #[test]
    fn wrong_passphrase_short_passphrase_and_unknown_project_fail_plainly() {
        let (_d, _f, shell, view) = shell_with_project();
        let out = tempfile::tempdir().expect("out");
        let archive = out.path().join("one.khbackup");
        let short = backup_create(
            &shell,
            std::slice::from_ref(&view.id),
            "short",
            &archive,
            &mut ignore,
        );
        assert_eq!(short.expect_err("short").code, "passphrase-too-short");
        backup_create(
            &shell,
            std::slice::from_ref(&view.id),
            "correct horse",
            &archive,
            &mut ignore,
        )
        .expect("create");
        let wrong = backup_test(&shell, &archive, "battery staple");
        assert_eq!(
            wrong.expect_err("wrong").code,
            "wrong-passphrase-or-damaged"
        );
        let other = tempfile::tempdir().expect("other");
        let stranger = shell
            .library()
            .expect("lib")
            .open_folder(other.path())
            .expect("open")
            .id();
        shell
            .library()
            .expect("lib")
            .forget(&stranger)
            .expect("forget");
        let unknown = backup_create(
            &shell,
            &[stranger],
            "correct horse",
            &out.path().join("x"),
            &mut ignore,
        );
        assert_eq!(unknown.expect_err("unknown").code, "not-found");
    }
}
