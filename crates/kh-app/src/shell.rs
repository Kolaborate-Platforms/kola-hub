//! State shared by every command: the recent projects list, the per-user folder and open Clean sessions.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use kh_store::{Library, Project, ProjectId};

use crate::clean::Session;
use crate::error::{AppError, AppResult};

/// Most Clean sessions open at once. Each holds the whole table of its file in memory, so a second
/// open of the same file costs a full extra copy, and a cleaned view costs one more on top.
pub const MAX_SESSIONS: usize = 3;

pub(crate) type SharedSession = Arc<Mutex<Session>>;

/// Everything the commands share. Held behind an `Arc` so long work can run on a worker thread.
pub struct Shell {
    data_dir: PathBuf,
    library: Mutex<Library>,
    sessions: Mutex<HashMap<String, SharedSession>>,
    next_session: AtomicU64,
}

fn locked<T>(mutex: &Mutex<T>) -> AppResult<MutexGuard<'_, T>> {
    mutex.lock().map_err(|_| AppError::internal())
}

impl Shell {
    /// Opens the shell on a per-user data folder, creating it when missing.
    pub fn new(data_dir: PathBuf) -> AppResult<Self> {
        let library = Library::open(&data_dir)?;
        Ok(Self {
            data_dir,
            library: Mutex::new(library),
            sessions: Mutex::new(HashMap::new()),
            next_session: AtomicU64::new(1),
        })
    }

    /// The per-user folder, for Rust code only.
    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub(crate) fn library(&self) -> AppResult<MutexGuard<'_, Library>> {
        locked(&self.library)
    }

    /// The folder of a listed project. An id that is not in the list is refused.
    pub(crate) fn root_of(&self, id: &ProjectId) -> AppResult<PathBuf> {
        Ok(self.library()?.project_path(id)?)
    }

    /// Opens a listed project without changing its place in the recent list.
    pub(crate) fn project(&self, id: &ProjectId) -> AppResult<Project> {
        Ok(Project::open_or_create(&self.root_of(id)?)?)
    }

    /// Adds a session and returns its id.
    pub(crate) fn add_session(&self, session: Session) -> AppResult<String> {
        let mut sessions = locked(&self.sessions)?;
        if sessions.len() >= MAX_SESSIONS {
            return Err(AppError::new(
                "too-many-open",
                "Too many files are open for cleaning. Close one first.",
            ));
        }
        let id = format!("s{}", self.next_session.fetch_add(1, Ordering::Relaxed));
        sessions.insert(id.clone(), Arc::new(Mutex::new(session)));
        Ok(id)
    }

    /// The session behind an id, or not found.
    pub(crate) fn session(&self, id: &str) -> AppResult<SharedSession> {
        locked(&self.sessions)?
            .get(id)
            .cloned()
            .ok_or_else(AppError::not_found)
    }

    /// Drops a session and its table. Closing one that is not open is not an error.
    pub(crate) fn remove_session(&self, id: &str) -> AppResult<()> {
        locked(&self.sessions)?.remove(id);
        Ok(())
    }

    /// How many Clean sessions are open.
    pub fn open_sessions(&self) -> usize {
        self.sessions.lock().map_or(0, |s| s.len())
    }
}

pub(crate) fn lock_session(session: &SharedSession) -> AppResult<MutexGuard<'_, Session>> {
    locked(session)
}
