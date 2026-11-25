//! Projects, the files at the top of a project, Recently deleted, settings and the string catalogue.

use std::fs;
use std::path::Path;

use kh_clean::StepList;
use kh_settings::Settings;
use kh_store::names::WORK_DIR;
use kh_store::{
    FileId, Project, ProjectId, RecentProject, RepairReport, StorageBudget, TrashId, TrashItem,
};
use serde::Serialize;

use crate::error::{AppError, AppResult};
use crate::shell::Shell;

/// Most files listed for one folder, so a huge folder cannot flood the window.
const MAX_LISTED_FILES: usize = 10_000;
const MAX_LABELLER_CHARS: usize = 100;
const MAX_RETENTION_DAYS: u32 = 3650;
const MIN_STORAGE_LIMIT: u64 = 16 * 1024 * 1024;
const ENGLISH: &str = "en";

/// A project as the interface sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectView {
    /// Id to open it by.
    pub id: ProjectId,
    /// The folder's own name.
    pub name: String,
    /// What opening repaired, for the "Restored your last changes" notice.
    pub recovered: RepairReport,
    /// Working state against the limit.
    pub budget: StorageBudget,
}

/// What kind of file a row is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FileKind {
    /// A CSV file, which Clean can open.
    Csv,
    /// Anything else, including Excel files until they are supported.
    Other,
}

/// One file at the top of a project folder.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRow {
    /// Id to use the file by.
    pub id: FileId,
    /// The file's name.
    pub name: String,
    /// Size in bytes.
    pub size_bytes: u64,
    /// Whether Clean can open it.
    pub kind: FileKind,
    /// True when Clean has saved steps for this file, so they can be reused.
    pub has_steps: bool,
}

/// The app's own version.
#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    /// Version text.
    pub version: &'static str,
}

/// Version of the running app.
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION"),
    }
}

/// The English catalogue as a JSON value.
pub fn strings() -> AppResult<serde_json::Value> {
    serde_json::from_str(kh_settings::ENGLISH_JSON).map_err(|_| AppError::internal())
}

/// Current settings, or the defaults on a first run.
pub fn settings_get(shell: &Shell) -> AppResult<Settings> {
    Ok(kh_settings::load(shell.data_dir())?)
}

/// Checks and saves settings, returning what was stored.
pub fn settings_save(shell: &Shell, settings: Settings) -> AppResult<Settings> {
    let too_small = settings.storage_limit < MIN_STORAGE_LIMIT;
    let too_long = settings.trash_retention_days > MAX_RETENTION_DAYS;
    let name_too_long = settings
        .labeller_name
        .as_ref()
        .is_some_and(|n| n.chars().count() > MAX_LABELLER_CHARS);
    if settings.language != ENGLISH || too_small || too_long || name_too_long {
        return Err(AppError::new(
            "bad-settings",
            "One of the settings is not a value Kola Hub can use. Nothing was saved.",
        ));
    }
    kh_settings::save(shell.data_dir(), &settings)?;
    Ok(settings)
}

/// The recent projects list.
pub fn projects_recent(shell: &Shell) -> AppResult<Vec<RecentProject>> {
    Ok(shell.library()?.list())
}

/// Opens the project in a folder the person chose and puts it first in the list.
pub fn project_open_folder(shell: &Shell, folder: &Path) -> AppResult<ProjectView> {
    let project = shell.library()?.open_folder(folder)?;
    view_of(shell, &project)
}

/// Opens a listed project and puts it first in the list.
pub fn project_open(shell: &Shell, id: &ProjectId) -> AppResult<ProjectView> {
    let project = shell.library()?.open_project(id)?;
    view_of(shell, &project)
}

/// Drops a project from the list. Its folder and files are left alone.
pub fn project_forget(shell: &Shell, id: &ProjectId) -> AppResult<()> {
    Ok(shell.library()?.forget(id)?)
}

fn view_of(shell: &Shell, project: &Project) -> AppResult<ProjectView> {
    let id = project.id();
    let name = shell
        .library()?
        .list()
        .into_iter()
        .find(|p| p.id == id)
        .map(|p| p.name)
        .unwrap_or_default();
    let limit = settings_get(shell)?.storage_limit;
    Ok(ProjectView {
        id,
        name,
        recovered: project.recovery().clone(),
        budget: project.storage_budget(limit)?,
    })
}

fn kind_of(name: &str) -> FileKind {
    let is_csv = Path::new(name)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if is_csv {
        FileKind::Csv
    } else {
        FileKind::Other
    }
}

/// The regular files at the top of the folder. Folders, links and hidden files are left out.
pub fn files_list(shell: &Shell, id: &ProjectId) -> AppResult<Vec<FileRow>> {
    let root = shell.root_of(id)?;
    let project = Project::open_or_create(&root)?;
    let mut rows = Vec::new();
    for entry in fs::read_dir(&root)?.flatten() {
        if rows.len() >= MAX_LISTED_FILES {
            break;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_file = entry.file_type().is_ok_and(|t| t.is_file());
        if !is_file || name.starts_with('.') || name == WORK_DIR {
            continue;
        }
        let Ok(file_id) = project.register_file(&name) else {
            continue;
        };
        let size_bytes = entry.metadata().map_or(0, |m| m.len());
        let has_steps = has_saved_steps(&project, &file_id)?;
        rows.push(FileRow {
            has_steps,
            id: file_id,
            kind: kind_of(&name),
            name,
            size_bytes,
        });
    }
    rows.sort_by_key(|r| r.name.to_lowercase());
    Ok(rows)
}

/// An unreadable or empty saved list counts as none: there is nothing to reuse.
fn has_saved_steps(project: &Project, file: &FileId) -> AppResult<bool> {
    let saved = project.load_clean_steps(file)?;
    Ok(saved
        .and_then(|text| StepList::from_json(&text).ok())
        .is_some_and(|list| !list.steps.is_empty()))
}

/// Moves a file to Recently deleted.
pub fn file_trash(shell: &Shell, project: &ProjectId, file: &FileId) -> AppResult<()> {
    shell.project(project)?.trash_file(file)?;
    Ok(())
}

/// What is in Recently deleted.
pub fn trash_list(shell: &Shell, project: &ProjectId) -> AppResult<Vec<TrashItem>> {
    Ok(shell.project(project)?.list_trash()?)
}

/// Puts a deleted file back.
pub fn trash_restore(shell: &Shell, project: &ProjectId, item: &TrashId) -> AppResult<()> {
    shell.project(project)?.restore_from_trash(item)?;
    Ok(())
}

/// Deletes everything in Recently deleted for good and returns how many items went.
pub fn trash_empty(shell: &Shell, project: &ProjectId) -> AppResult<u32> {
    Ok(shell.project(project)?.empty_trash_now()?)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn shell_with_project() -> (tempfile::TempDir, tempfile::TempDir, Shell, ProjectView) {
        let data = tempfile::tempdir().expect("data");
        let folder = tempfile::tempdir().expect("folder");
        let shell = Shell::new(data.path().to_path_buf()).expect("shell");
        let view = project_open_folder(&shell, folder.path()).expect("open");
        (data, folder, shell, view)
    }

    fn json_of(value: &impl Serialize) -> String {
        serde_json::to_string(value).expect("json")
    }

    #[test]
    fn opening_lists_and_forgetting_keep_paths_out_of_every_result() {
        let (data, folder, shell, view) = shell_with_project();
        fs::write(folder.path().join("b.CSV"), "x\n1\n").expect("b");
        fs::write(folder.path().join("a.xlsx"), "z").expect("a");
        fs::write(folder.path().join(".hidden.csv"), "z").expect("h");
        fs::create_dir(folder.path().join("sub")).expect("sub");
        let files = files_list(&shell, &view.id).expect("files");
        let names: Vec<_> = files.iter().map(|f| (f.name.as_str(), f.kind)).collect();
        assert_eq!(
            names,
            vec![("a.xlsx", FileKind::Other), ("b.CSV", FileKind::Csv)]
        );
        let recent = projects_recent(&shell).expect("recent");
        assert_eq!(recent.len(), 1);
        for text in [json_of(&view), json_of(&files), json_of(&recent)] {
            assert!(!text.contains(&*folder.path().to_string_lossy()), "{text}");
            assert!(!text.contains(&*data.path().to_string_lossy()), "{text}");
        }
        project_forget(&shell, &view.id).expect("forget");
        assert!(projects_recent(&shell).expect("recent").is_empty());
        assert!(folder.path().join("b.CSV").exists());
        assert_eq!(
            project_open(&shell, &view.id).expect_err("unknown").code,
            "not-found"
        );
    }

    #[test]
    fn trash_round_trip_and_ids_from_another_project_are_refused() {
        let (_d, folder, shell, view) = shell_with_project();
        fs::write(folder.path().join("a.csv"), "x\n1\n").expect("a");
        let id = files_list(&shell, &view.id).expect("files")[0].id.clone();
        file_trash(&shell, &view.id, &id).expect("trash");
        assert!(!folder.path().join("a.csv").exists());
        let items = trash_list(&shell, &view.id).expect("list");
        assert_eq!(items.len(), 1);
        assert!(!json_of(&items).contains(&*folder.path().to_string_lossy()));
        trash_restore(&shell, &view.id, &items[0].id).expect("restore");
        assert!(folder.path().join("a.csv").exists());
        file_trash(&shell, &view.id, &id).expect("again");
        assert_eq!(trash_empty(&shell, &view.id).expect("empty"), 1);

        let other = tempfile::tempdir().expect("other");
        let other_view = project_open_folder(&shell, other.path()).expect("other");
        assert_eq!(
            file_trash(&shell, &other_view.id, &id)
                .expect_err("foreign")
                .code,
            "not-found"
        );
    }

    #[test]
    fn settings_round_trip_and_bad_values_are_refused() {
        let (_d, _f, shell, _v) = shell_with_project();
        let mut settings = settings_get(&shell).expect("defaults");
        settings.trash_retention_days = 7;
        assert_eq!(
            settings_save(&shell, settings.clone()).expect("save"),
            settings
        );
        assert_eq!(settings_get(&shell).expect("get"), settings);
        settings.storage_limit = 1;
        assert_eq!(
            settings_save(&shell, settings).expect_err("small").code,
            "bad-settings"
        );
        assert!(strings().expect("strings").is_object());
        assert!(!app_info().version.is_empty());
    }

    #[test]
    fn a_file_says_whether_it_has_steps_to_reuse() {
        let (_d, folder, shell, view) = shell_with_project();
        fs::write(folder.path().join("a.csv"), "x\n 1 \n").expect("a");
        let before = files_list(&shell, &view.id).expect("files");
        assert!(!before[0].has_steps);
        let session = crate::clean::clean_open(&shell, &view.id, &before[0].id).expect("open");
        let step = kh_clean::Step::TrimSpaces { columns: None };
        crate::clean::clean_add_step(&shell, &session.session_id, step).expect("step");
        assert!(files_list(&shell, &view.id).expect("files")[0].has_steps);
    }
}
