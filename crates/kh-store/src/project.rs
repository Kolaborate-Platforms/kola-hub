//! The hidden `.kola-hub/` folder: database, blobs and trash.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OpenFlags, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::blobs::{BlobId, BlobStore, StoredBlob};
use crate::error::{Result, StoreError};
use crate::names::{WORK_DIR, clean_relative, opaque_id, resolve_existing, resolve_new};
use crate::trash::RepairReport;

const DB_FILE: &str = "project.sqlite";
const BLOBS_DIR: &str = "blobs";
pub(crate) const TRASH_DIR: &str = "trash";
const QUICK_CHECK_OK: &str = "ok";
const BUSY_TIMEOUT_MS: u32 = 5_000;

/// Each entry moves the database one version forward. Never edit an old entry, add a new one.
const MIGRATIONS: &[&str] = &["CREATE TABLE files (
        id TEXT PRIMARY KEY,
        rel_path TEXT NOT NULL UNIQUE,
        added_at INTEGER NOT NULL
    );
    CREATE TABLE blob_refs (
        owner TEXT NOT NULL,
        blob TEXT NOT NULL,
        label TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        PRIMARY KEY (owner, blob, label)
    );"];

/// The newest database version this build understands.
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// Opaque id of a project. Carries no path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectId(pub(crate) String);

/// Opaque id of a file in a project. Carries no path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct FileId(pub(crate) String);

impl ProjectId {
    pub(crate) fn for_root(root: &Path) -> Self {
        Self(opaque_id("project", &root.to_string_lossy()))
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FileId {
    pub(crate) fn for_rel(rel: &str) -> Self {
        Self(opaque_id("file", rel))
    }

    /// The id text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(crate) fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

pub(crate) fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// An open project: its folder, database, blob store and trash.
#[derive(Debug)]
pub struct Project {
    pub(crate) root: PathBuf,
    pub(crate) work: PathBuf,
    conn: Connection,
    blobs: BlobStore,
    recovery: RepairReport,
}

impl Project {
    /// Opens the project in `root`, creating `.kola-hub/` on first use. A project from a newer app
    /// version is refused before anything is written. A damaged database is reported and left alone.
    /// Interrupted trash moves are repaired; see [`Project::recovery`].
    pub fn open_or_create(root: &Path) -> Result<Self> {
        let root = root.canonicalize().map_err(|_| StoreError::NotFound)?;
        if !root.is_dir() {
            return Err(StoreError::NotFound);
        }
        let work = root.join(WORK_DIR);
        make_real_dir(&work)?;
        refuse_unusable_database(&work.join(DB_FILE))?;
        make_real_dir(&work.join(TRASH_DIR))?;
        make_real_dir(&work.join(BLOBS_DIR))?;
        let conn = open_database(&work.join(DB_FILE))?;
        let blobs = BlobStore::open(real_dir(&work.join(BLOBS_DIR))?)?;
        let mut project = Self {
            root,
            work,
            conn,
            blobs,
            recovery: RepairReport::default(),
        };
        project.recovery = project.repair_trash()?;
        Ok(project)
    }

    /// The id the library uses for this project.
    pub fn id(&self) -> ProjectId {
        ProjectId::for_root(&self.root)
    }

    /// What was repaired when the project opened, for the "Restored your last changes" notice.
    pub fn recovery(&self) -> &RepairReport {
        &self.recovery
    }

    /// The database, for tools that keep their own tables here.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// The content store.
    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }

    /// The database version stored in the file.
    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Remembers a file that exists inside the project and returns its opaque id.
    pub fn register_file(&self, name: &str) -> Result<FileId> {
        let rel = clean_relative(name)?;
        let full = resolve_new(&self.root, &rel)?;
        fs::symlink_metadata(&full).map_err(|_| StoreError::NotFound)?;
        let rel_text = rel_text(&self.root, &full)?;
        let known: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM files WHERE rel_path = ?1",
                params![rel_text],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = known {
            return Ok(FileId(id));
        }
        let mut id = FileId::for_rel(&rel_text);
        if self.rel_of(&id).is_ok() {
            // The default id belongs to a file that was restored under another name.
            id = FileId(opaque_id("file", &format!("{rel_text}#{}", now_nanos())));
        }
        self.conn.execute(
            "INSERT INTO files (id, rel_path, added_at) VALUES (?1, ?2, ?3)",
            params![id.0, rel_text, now_unix()],
        )?;
        Ok(id)
    }

    /// Where a file is, for Rust code only. The interface never gets this.
    pub fn file_path(&self, id: &FileId) -> Result<PathBuf> {
        let rel = self.rel_of(id)?;
        resolve_existing(&self.root, Path::new(&rel))
    }

    /// The file's own name for display, without any folders.
    pub fn display_name(&self, id: &FileId) -> Result<String> {
        let rel = self.rel_of(id)?;
        Path::new(&rel)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or(StoreError::NotFound)
    }

    /// Opens a file for reading. A link that leads outside the project is refused.
    pub fn open_file(&self, id: &FileId) -> Result<File> {
        Ok(File::open(self.file_path(id)?)?)
    }

    pub(crate) fn rel_of(&self, id: &FileId) -> Result<String> {
        self.conn
            .query_row(
                "SELECT rel_path FROM files WHERE id = ?1",
                params![id.0],
                |r| r.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)
    }

    pub(crate) fn set_rel(&self, id: &FileId, rel: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE files SET rel_path = ?2 WHERE id = ?1",
            params![id.0, rel],
        )?;
        Ok(())
    }

    /// Stores content for a file under a label, such as a snapshot, and records the link in the database.
    /// The blob is whole on disk before the record is written, so a record never points at nothing.
    pub fn store_blob(&self, owner: &FileId, label: &str, reader: impl Read) -> Result<StoredBlob> {
        self.rel_of(owner)?;
        let stored = self.blobs.put(reader)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO blob_refs (owner, blob, label, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![owner.0, stored.id.as_str(), label, now_unix()],
        )?;
        Ok(stored)
    }

    /// Blobs recorded for a file, oldest first.
    pub fn blobs_of(&self, owner: &FileId) -> Result<Vec<(String, BlobId)>> {
        let mut stmt = self.conn.prepare(
            "SELECT label, blob FROM blob_refs WHERE owner = ?1 ORDER BY created_at, label",
        )?;
        let rows = stmt.query_map(params![owner.0], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (label, hash) = row?;
            out.push((label, BlobId::parse(&hash)?));
        }
        Ok(out)
    }
}

/// The relative name, with `/` between folders, of `full` under `root`.
pub(crate) fn rel_text(root: &Path, full: &Path) -> Result<String> {
    let rel = full
        .strip_prefix(root)
        .map_err(|_| StoreError::OutsideProject)?;
    let parts: Vec<_> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Ok(parts.join("/"))
}

/// Creates `dir` if missing and refuses a link or file standing where a folder must be.
fn make_real_dir(dir: &Path) -> Result<()> {
    if fs::symlink_metadata(dir).is_err() {
        fs::create_dir(dir)?;
    }
    real_dir(dir).map(drop)
}

fn real_dir(dir: &Path) -> Result<PathBuf> {
    let meta = fs::symlink_metadata(dir)?;
    if meta.file_type().is_symlink() || !meta.is_dir() {
        return Err(StoreError::OutsideProject);
    }
    Ok(dir.to_path_buf())
}

/// Refuses a database that is a link, or one made by a newer version, before anything is written.
fn refuse_unusable_database(path: &Path) -> Result<()> {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if !meta.is_file() {
        return Err(StoreError::OutsideProject);
    }
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let found: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if found > SCHEMA_VERSION {
        return Err(StoreError::NewerVersion {
            found,
            supported: SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn open_database(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_millis(u64::from(BUSY_TIMEOUT_MS)))?;
    let found: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != QUICK_CHECK_OK {
        return Err(StoreError::DatabaseDamaged);
    }
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&mut conn, found)?;
    Ok(conn)
}

fn migrate(conn: &mut Connection, from: u32) -> Result<()> {
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(from as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(
            None,
            "user_version",
            i64::try_from(index + 1).unwrap_or(i64::MAX),
        )?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn creates_layout_with_wal_full_and_version() {
        let d = root();
        let p = Project::open_or_create(d.path()).expect("open");
        for sub in ["project.sqlite", "blobs", "trash"] {
            assert!(d.path().join(WORK_DIR).join(sub).exists(), "{sub}");
        }
        assert_eq!(p.schema_version().expect("v"), SCHEMA_VERSION);
        let mode: String = p
            .connection()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .expect("mode");
        let sync: i64 = p
            .connection()
            .query_row("PRAGMA synchronous", [], |r| r.get(0))
            .expect("sync");
        assert_eq!(mode, "wal");
        assert_eq!(sync, 2, "FULL");
    }

    #[test]
    fn reopening_keeps_data() {
        let d = root();
        fs::write(d.path().join("a.csv"), "x\n1\n").expect("file");
        let id = Project::open_or_create(d.path())
            .expect("open")
            .register_file("a.csv")
            .expect("reg");
        let p = Project::open_or_create(d.path()).expect("reopen");
        assert_eq!(p.display_name(&id).expect("name"), "a.csv");
    }

    #[test]
    fn newer_version_is_refused_and_not_touched() {
        let d = root();
        drop(Project::open_or_create(d.path()).expect("open"));
        let db = d.path().join(WORK_DIR).join(DB_FILE);
        Connection::open(&db)
            .expect("db")
            .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .expect("bump");
        let before = fs::read(&db).expect("bytes");
        let err = Project::open_or_create(d.path()).expect_err("must refuse");
        assert!(matches!(err, StoreError::NewerVersion { .. }));
        assert!(err.message().contains("newer version"));
        assert_eq!(fs::read(&db).expect("bytes"), before);
    }

    #[test]
    fn damaged_database_is_reported_and_left_in_place() {
        let d = root();
        drop(Project::open_or_create(d.path()).expect("open"));
        let db = d.path().join(WORK_DIR).join(DB_FILE);
        let garbage = vec![0x42u8; 8192];
        fs::write(&db, &garbage).expect("damage");
        let _ = fs::remove_file(db.with_extension("sqlite-wal"));
        let _ = fs::remove_file(db.with_extension("sqlite-shm"));
        let err = Project::open_or_create(d.path()).expect_err("must refuse");
        assert!(matches!(err, StoreError::DatabaseDamaged), "{err}");
        assert_eq!(fs::read(&db).expect("bytes"), garbage);
    }

    #[test]
    fn missing_folder_and_linked_work_dir_are_refused() {
        let d = root();
        assert!(matches!(
            Project::open_or_create(&d.path().join("nope")),
            Err(StoreError::NotFound)
        ));
        #[cfg(unix)]
        {
            let elsewhere = root();
            std::os::unix::fs::symlink(elsewhere.path(), d.path().join(WORK_DIR)).expect("link");
            assert!(matches!(
                Project::open_or_create(d.path()),
                Err(StoreError::OutsideProject)
            ));
        }
    }

    #[test]
    fn hostile_names_and_links_cannot_reach_outside() {
        let d = root();
        let outside = root();
        fs::write(outside.path().join("secret.txt"), "s").expect("secret");
        let p = Project::open_or_create(d.path()).expect("open");
        assert!(matches!(p.register_file("../x"), Err(StoreError::BadName)));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(outside.path(), d.path().join("out")).expect("dir link");
            std::os::unix::fs::symlink(outside.path().join("secret.txt"), d.path().join("s.txt"))
                .expect("file link");
            assert!(matches!(
                p.register_file("out/secret.txt"),
                Err(StoreError::OutsideProject)
            ));
            let id = p.register_file("s.txt").expect("a link can be listed");
            assert!(matches!(p.open_file(&id), Err(StoreError::OutsideProject)));
        }
    }

    #[test]
    fn blob_records_point_at_whole_blobs() {
        let d = root();
        fs::write(d.path().join("a.csv"), "x").expect("file");
        let p = Project::open_or_create(d.path()).expect("open");
        let id = p.register_file("a.csv").expect("reg");
        let stored = p
            .store_blob(&id, "before cleaning", &b"snapshot"[..])
            .expect("store");
        let listed = p.blobs_of(&id).expect("list");
        assert_eq!(
            listed,
            vec![("before cleaning".to_owned(), stored.id.clone())]
        );
        assert!(p.blobs().verify(&stored.id).is_ok());
    }

    #[test]
    fn a_newer_database_is_refused_before_any_folder_is_made() {
        let d = root();
        drop(Project::open_or_create(d.path()).expect("open"));
        let work = d.path().join(WORK_DIR);
        let conn = Connection::open(work.join(DB_FILE)).expect("db");
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .expect("bump");
        drop(conn);
        fs::remove_dir_all(work.join(TRASH_DIR)).expect("trash");
        fs::remove_dir_all(work.join(BLOBS_DIR)).expect("blobs");
        assert!(matches!(
            Project::open_or_create(d.path()),
            Err(StoreError::NewerVersion { .. })
        ));
        assert!(!work.join(TRASH_DIR).exists() && !work.join(BLOBS_DIR).exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_database_that_is_a_link_is_refused() {
        let d = root();
        let outside = root();
        fs::create_dir(d.path().join(WORK_DIR)).expect("work");
        let target = outside.path().join("other.sqlite");
        std::os::unix::fs::symlink(&target, d.path().join(WORK_DIR).join(DB_FILE)).expect("link");
        assert!(matches!(
            Project::open_or_create(d.path()),
            Err(StoreError::OutsideProject)
        ));
        assert!(!target.exists());
    }
}
