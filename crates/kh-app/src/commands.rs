//! The Tauri commands. Each is a thin wrapper over a plain function that takes the shell.

use std::sync::Arc;

use kh_clean::{Preview, Profile, SpellingGroup, Step};
use kh_settings::Settings;
use kh_store::{FileId, ProjectId, RecentProject, SavedFile, TrashId, TrashItem};
use tauri::{AppHandle, Emitter, State};

use crate::backup::{self, CreateReportView, ProgressView, RestoreReportView, TestReportView};
use crate::clean::{self, CleanSession, CleanState, ReuseResult, WindowView};
use crate::dialogs::{self, Pick};
use crate::error::{AppError, AppResult};
use crate::projects::{self, AppInfo, FileRow, ProjectView};
use crate::shell::Shell;

type Shared<'a> = State<'a, Arc<Shell>>;

const PROGRESS_EVENT: &str = "backup-progress";

/// Runs `work` on a worker thread so the window keeps responding.
async fn blocking<T, F>(shell: &Shared<'_>, work: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Shell) -> AppResult<T> + Send + 'static,
{
    let shell = Arc::clone(shell.inner());
    tauri::async_runtime::spawn_blocking(move || work(&shell))
        .await
        .map_err(|_| AppError::internal())?
}

fn progress_sender(app: AppHandle) -> impl FnMut(ProgressView) {
    move |progress| {
        // A lost progress event must never stop a backup.
        let _ = app.emit(PROGRESS_EVENT, progress);
    }
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    projects::app_info()
}

#[tauri::command]
pub fn strings() -> AppResult<serde_json::Value> {
    projects::strings()
}

#[tauri::command]
pub async fn settings_get(shell: Shared<'_>) -> AppResult<Settings> {
    blocking(&shell, projects::settings_get).await
}

#[tauri::command]
pub async fn settings_save(shell: Shared<'_>, settings: Settings) -> AppResult<Settings> {
    blocking(&shell, move |s| projects::settings_save(s, settings)).await
}

#[tauri::command]
pub async fn projects_recent(shell: Shared<'_>) -> AppResult<Vec<RecentProject>> {
    blocking(&shell, projects::projects_recent).await
}

#[tauri::command]
pub async fn project_pick(app: AppHandle, shell: Shared<'_>) -> AppResult<Option<ProjectView>> {
    let Some(folder) = dialogs::pick(&app, Pick::Folder).await? else {
        return Ok(None);
    };
    blocking(&shell, move |s| projects::project_open_folder(s, &folder))
        .await
        .map(Some)
}

#[tauri::command]
pub async fn project_open(shell: Shared<'_>, project_id: ProjectId) -> AppResult<ProjectView> {
    blocking(&shell, move |s| projects::project_open(s, &project_id)).await
}

#[tauri::command]
pub async fn project_forget(shell: Shared<'_>, project_id: ProjectId) -> AppResult<()> {
    blocking(&shell, move |s| projects::project_forget(s, &project_id)).await
}

#[tauri::command]
pub async fn files_list(shell: Shared<'_>, project_id: ProjectId) -> AppResult<Vec<FileRow>> {
    blocking(&shell, move |s| projects::files_list(s, &project_id)).await
}

#[tauri::command]
pub async fn file_trash(
    shell: Shared<'_>,
    project_id: ProjectId,
    file_id: FileId,
) -> AppResult<()> {
    blocking(&shell, move |s| {
        projects::file_trash(s, &project_id, &file_id)
    })
    .await
}

#[tauri::command]
pub async fn trash_list(shell: Shared<'_>, project_id: ProjectId) -> AppResult<Vec<TrashItem>> {
    blocking(&shell, move |s| projects::trash_list(s, &project_id)).await
}

#[tauri::command]
pub async fn trash_restore(
    shell: Shared<'_>,
    project_id: ProjectId,
    trash_id: TrashId,
) -> AppResult<()> {
    blocking(&shell, move |s| {
        projects::trash_restore(s, &project_id, &trash_id)
    })
    .await
}

#[tauri::command]
pub async fn trash_empty(shell: Shared<'_>, project_id: ProjectId) -> AppResult<u32> {
    blocking(&shell, move |s| projects::trash_empty(s, &project_id)).await
}

#[tauri::command]
pub async fn backup_create(
    app: AppHandle,
    shell: Shared<'_>,
    project_ids: Vec<ProjectId>,
    passphrase: String,
) -> AppResult<Option<CreateReportView>> {
    let Some(destination) = dialogs::pick(&app, Pick::ArchiveToWrite).await? else {
        return Ok(None);
    };
    let mut send = progress_sender(app);
    blocking(&shell, move |s| {
        backup::backup_create(s, &project_ids, &passphrase, &destination, &mut send)
    })
    .await
    .map(Some)
}

#[tauri::command]
pub async fn backup_restore(
    app: AppHandle,
    shell: Shared<'_>,
    passphrase: String,
) -> AppResult<Option<RestoreReportView>> {
    let Some(archive) = dialogs::pick(&app, Pick::ArchiveToRead).await? else {
        return Ok(None);
    };
    let Some(folder) = dialogs::pick(&app, Pick::Folder).await? else {
        return Ok(None);
    };
    let mut send = progress_sender(app);
    blocking(&shell, move |_| {
        backup::backup_restore(&archive, &folder, &passphrase, &mut send)
    })
    .await
    .map(Some)
}

#[tauri::command]
pub async fn backup_test(
    app: AppHandle,
    shell: Shared<'_>,
    passphrase: String,
) -> AppResult<Option<TestReportView>> {
    let Some(archive) = dialogs::pick(&app, Pick::ArchiveToRead).await? else {
        return Ok(None);
    };
    blocking(&shell, move |s| {
        backup::backup_test(s, &archive, &passphrase)
    })
    .await
    .map(Some)
}

#[tauri::command]
pub async fn clean_open(
    shell: Shared<'_>,
    project_id: ProjectId,
    file_id: FileId,
) -> AppResult<CleanSession> {
    blocking(&shell, move |s| clean::clean_open(s, &project_id, &file_id)).await
}

#[tauri::command]
pub async fn clean_close(shell: Shared<'_>, session_id: String) -> AppResult<()> {
    blocking(&shell, move |s| clean::clean_close(s, &session_id)).await
}

#[tauri::command]
pub async fn clean_profile(shell: Shared<'_>, session_id: String) -> AppResult<Profile> {
    blocking(&shell, move |s| clean::clean_profile(s, &session_id)).await
}

#[tauri::command]
pub async fn clean_window(
    shell: Shared<'_>,
    session_id: String,
    start: usize,
    count: usize,
) -> AppResult<WindowView> {
    blocking(&shell, move |s| {
        clean::clean_window(s, &session_id, start, count)
    })
    .await
}

#[tauri::command]
pub async fn clean_preview(
    shell: Shared<'_>,
    session_id: String,
    step: Step,
    start: usize,
    count: usize,
) -> AppResult<Preview> {
    blocking(&shell, move |s| {
        clean::clean_preview(s, &session_id, &step, start, count)
    })
    .await
}

#[tauri::command]
pub async fn clean_suggest_groups(
    shell: Shared<'_>,
    session_id: String,
    column: String,
) -> AppResult<Vec<SpellingGroup>> {
    blocking(&shell, move |s| {
        clean::clean_suggest_groups(s, &session_id, &column)
    })
    .await
}

#[tauri::command]
pub async fn clean_state(shell: Shared<'_>, session_id: String) -> AppResult<CleanState> {
    blocking(&shell, move |s| clean::clean_state(s, &session_id)).await
}

#[tauri::command]
pub async fn clean_add_step(
    shell: Shared<'_>,
    session_id: String,
    step: Step,
) -> AppResult<CleanState> {
    blocking(&shell, move |s| clean::clean_add_step(s, &session_id, step)).await
}

#[tauri::command]
pub async fn clean_undo(shell: Shared<'_>, session_id: String) -> AppResult<CleanState> {
    blocking(&shell, move |s| clean::clean_undo(s, &session_id)).await
}

#[tauri::command]
pub async fn clean_redo(shell: Shared<'_>, session_id: String) -> AppResult<CleanState> {
    blocking(&shell, move |s| clean::clean_redo(s, &session_id)).await
}

#[tauri::command]
pub async fn clean_move_step(
    shell: Shared<'_>,
    session_id: String,
    from: usize,
    to: usize,
) -> AppResult<CleanState> {
    blocking(&shell, move |s| {
        clean::clean_move_step(s, &session_id, from, to)
    })
    .await
}

#[tauri::command]
pub async fn clean_set_enabled(
    shell: Shared<'_>,
    session_id: String,
    index: usize,
    enabled: bool,
) -> AppResult<CleanState> {
    blocking(&shell, move |s| {
        clean::clean_set_enabled(s, &session_id, index, enabled)
    })
    .await
}

#[tauri::command]
pub async fn clean_remove_step(
    shell: Shared<'_>,
    session_id: String,
    index: usize,
) -> AppResult<CleanState> {
    blocking(&shell, move |s| {
        clean::clean_remove_step(s, &session_id, index)
    })
    .await
}

#[tauri::command]
pub async fn clean_fix_cell(
    shell: Shared<'_>,
    session_id: String,
    row: usize,
    column: usize,
    value: String,
) -> AppResult<CleanState> {
    blocking(&shell, move |s| {
        clean::clean_fix_cell(s, &session_id, row, column, &value)
    })
    .await
}

#[tauri::command]
pub async fn clean_save(shell: Shared<'_>, session_id: String) -> AppResult<SavedFile> {
    blocking(&shell, move |s| clean::clean_save(s, &session_id)).await
}

#[tauri::command]
pub async fn clean_reuse(
    shell: Shared<'_>,
    session_id: String,
    from_file_id: FileId,
) -> AppResult<ReuseResult> {
    blocking(&shell, move |s| {
        clean::clean_reuse(s, &session_id, &from_file_id)
    })
    .await
}
