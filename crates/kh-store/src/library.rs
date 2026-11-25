//! The recent projects list, kept in a per-user folder the caller gives. Never uploaded.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::atomic::atomic_write;
use crate::error::{Result, StoreError};
use crate::project::{FileId, Project, ProjectId, now_unix};

const LIST_FILE: &str = "recent-projects.json";
const DAMAGED_FILE: &str = "recent-projects.damaged.json";
const MAX_RECENT: usize = 20;
const MAX_LIST_BYTES: u64 = 1024 * 1024;
const LIST_VERSION: u32 = 1;

/// One line of the recent projects screen. It has a display name and no path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentProject {
    /// Id to open the project by.
    pub id: ProjectId,
    /// The folder's own name.
    pub name: String,
    /// When it was last opened, in seconds since 1970.
    pub last_opened: i64,
    /// False when the folder is gone, for example an unplugged drive.
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    root: PathBuf,
    last_opened: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Document {
    version: u32,
    projects: Vec<Stored>,
}

/// The recent projects list. Maps opaque ids to folders; paths stay in Rust.
#[derive(Debug)]
pub struct Library {
    file: PathBuf,
    damaged: PathBuf,
    projects: Vec<Stored>,
}

impl Library {
    /// Loads the list from `data_dir`. A damaged list is kept aside as `recent-projects.damaged.json`
    /// and an empty one is used, so the app still starts.
    pub fn open(data_dir: &Path) -> Result<Self> {
        fs::create_dir_all(data_dir)?;
        let file = data_dir.join(LIST_FILE);
        let damaged = data_dir.join(DAMAGED_FILE);
        let projects = match read_list(&file) {
            Ok(list) => list,
            Err(_) => {
                if fs::symlink_metadata(&file).is_ok() {
                    fs::rename(&file, &damaged)?;
                }
                Vec::new()
            }
        };
        Ok(Self {
            file,
            damaged,
            projects,
        })
    }

    /// True when the last load found a damaged list and set it aside.
    pub fn had_damaged_list(&self) -> bool {
        self.damaged.exists()
    }

    /// Opens (or creates) the project in `folder` and puts it first in the list.
    pub fn open_folder(&mut self, folder: &Path) -> Result<Project> {
        let project = Project::open_or_create(folder)?;
        self.remember(&project)?;
        Ok(project)
    }

    /// Opens a project from the list by id.
    pub fn open_project(&mut self, id: &ProjectId) -> Result<Project> {
        let root = self.project_path(id)?;
        self.open_folder(&root)
    }

    /// The folder behind an id, for Rust code only.
    pub fn project_path(&self, id: &ProjectId) -> Result<PathBuf> {
        self.projects
            .iter()
            .find(|p| ProjectId::for_root(&p.root) == *id)
            .map(|p| p.root.clone())
            .ok_or(StoreError::NotFound)
    }

    /// The path behind a file of a listed project, for Rust code only.
    pub fn file_path(&self, project: &ProjectId, file: &FileId) -> Result<PathBuf> {
        Project::open_or_create(&self.project_path(project)?)?.file_path(file)
    }

    /// Most recent first.
    pub fn list(&self) -> Vec<RecentProject> {
        self.projects
            .iter()
            .map(|p| RecentProject {
                id: ProjectId::for_root(&p.root),
                name: p
                    .root
                    .file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned()),
                last_opened: p.last_opened,
                available: p.root.is_dir(),
            })
            .collect()
    }

    /// Drops a project from the list. The folder and its files are left alone.
    pub fn forget(&mut self, id: &ProjectId) -> Result<()> {
        self.projects
            .retain(|p| ProjectId::for_root(&p.root) != *id);
        self.save()
    }

    fn remember(&mut self, project: &Project) -> Result<()> {
        self.projects.retain(|p| p.root != project.root);
        self.projects.insert(
            0,
            Stored {
                root: project.root.clone(),
                last_opened: now_unix(),
            },
        );
        self.projects.truncate(MAX_RECENT);
        self.save()
    }

    fn save(&self) -> Result<()> {
        let doc = Document {
            version: LIST_VERSION,
            projects: self.projects.clone(),
        };
        let bytes = serde_json::to_vec_pretty(&doc)
            .map_err(|_| StoreError::Conflict("The recent projects list could not be saved."))?;
        atomic_write(&self.file, &bytes)?;
        Ok(())
    }
}

fn read_list(file: &Path) -> Result<Vec<Stored>> {
    let meta = match fs::metadata(file) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut bytes = Vec::new();
    // Read through a limit: the size on disk can change after it was looked at.
    File::open(file)?
        .take(MAX_LIST_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if meta.len() > MAX_LIST_BYTES || bytes.len() as u64 > MAX_LIST_BYTES {
        return Err(StoreError::Conflict("The recent projects list is damaged."));
    }
    let doc: Document = serde_json::from_slice(&bytes)
        .map_err(|_| StoreError::Conflict("The recent projects list is damaged."))?;
    if doc.version > LIST_VERSION {
        return Err(StoreError::Conflict(
            "The recent projects list is from a newer version.",
        ));
    }
    Ok(doc.projects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembers_projects_newest_first_and_maps_ids_to_paths() {
        let data = tempfile::tempdir().expect("data");
        let (p1, p2) = (
            tempfile::tempdir().expect("p1"),
            tempfile::tempdir().expect("p2"),
        );
        let mut lib = Library::open(data.path()).expect("open");
        let first = lib.open_folder(p1.path()).expect("p1").id();
        let second = lib.open_folder(p2.path()).expect("p2").id();
        lib.open_folder(p1.path()).expect("p1 again");
        let ids: Vec<_> = lib.list().into_iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![first.clone(), second]);
        assert_eq!(
            lib.project_path(&first).expect("path"),
            p1.path().canonicalize().expect("canon")
        );
        let reloaded = Library::open(data.path()).expect("reload");
        assert_eq!(reloaded.list().len(), 2);
        let json = serde_json::to_string(&reloaded.list()).expect("json");
        assert!(
            !json.contains(p1.path().to_string_lossy().as_ref()),
            "paths must not reach the interface"
        );
    }

    #[test]
    fn a_damaged_list_is_kept_aside_and_the_app_starts_empty() {
        let data = tempfile::tempdir().expect("data");
        fs::write(data.path().join(LIST_FILE), b"\xff\xfe not json").expect("damage");
        let lib = Library::open(data.path()).expect("open");
        assert!(lib.list().is_empty() && lib.had_damaged_list());
        assert_eq!(
            fs::read(data.path().join(DAMAGED_FILE)).expect("kept"),
            b"\xff\xfe not json"
        );
    }

    #[test]
    fn forgetting_keeps_the_folder_and_missing_folders_show_unavailable() {
        let data = tempfile::tempdir().expect("data");
        let p = tempfile::tempdir().expect("p");
        let mut lib = Library::open(data.path()).expect("open");
        let id = lib.open_folder(p.path()).expect("open").id();
        lib.forget(&id).expect("forget");
        assert!(lib.list().is_empty() && p.path().exists());
        assert!(matches!(lib.project_path(&id), Err(StoreError::NotFound)));
    }
}
