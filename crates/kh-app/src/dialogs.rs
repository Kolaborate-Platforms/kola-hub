//! Folder and file choosing, from Rust only, so paths never reach the interface.

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};

use crate::error::{AppError, AppResult};

const BACKUP_FILTER_NAME: &str = "Kola Hub backup";
const BACKUP_EXTENSION: &str = "khbackup";
const BACKUP_FILE_NAME: &str = "kola-hub.khbackup";

/// What the person is asked to choose.
#[derive(Debug, Clone, Copy)]
pub enum Pick {
    /// A project folder, or an empty folder to restore into.
    Folder,
    /// A backup file to read.
    ArchiveToRead,
    /// Where a new backup file goes.
    ArchiveToWrite,
}

fn show(app: &AppHandle, what: Pick) -> Option<FilePath> {
    let dialog = app.dialog().file();
    let backups = |d: tauri_plugin_dialog::FileDialogBuilder<_>| {
        d.add_filter(BACKUP_FILTER_NAME, &[BACKUP_EXTENSION])
    };
    match what {
        Pick::Folder => dialog.blocking_pick_folder(),
        Pick::ArchiveToRead => backups(dialog).blocking_pick_file(),
        Pick::ArchiveToWrite => backups(dialog)
            .set_file_name(BACKUP_FILE_NAME)
            .blocking_save_file(),
    }
}

/// Shows the dialog and waits on a worker thread. `None` means the person cancelled.
pub async fn pick(app: &AppHandle, what: Pick) -> AppResult<Option<PathBuf>> {
    let app = app.clone();
    let chosen = tauri::async_runtime::spawn_blocking(move || show(&app, what))
        .await
        .map_err(|_| AppError::internal())?;
    chosen
        .map(|file| {
            file.into_path().map_err(|_| {
                AppError::new(
                    "unusable-location",
                    "That place cannot be used. Choose a folder on this computer.",
                )
            })
        })
        .transpose()
}
