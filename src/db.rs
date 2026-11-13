//! Opens validated RECON databases and owns common transaction and scope checks.

use crate::{
    error::{Error, Result},
    validate,
};
use rusqlite::{Connection, OpenFlags, OptionalExtension, TransactionBehavior, params};
use std::path::Path;
use std::time::Duration;

pub struct Db {
    pub connection: Connection,
    pub project: String,
}

pub struct Event<'a> {
    pub task: Option<&'a str>,
    pub actor: &'a str,
    pub kind: &'a str,
    pub subject: &'a str,
    pub reason: Option<&'a str>,
}

impl Db {
    /// Opens an existing recognized store without creating or migrating unknown files.
    pub fn open(path: &Path, project: &str) -> Result<Self> {
        validate::id(project)?;
        if !path.is_file() {
            return Err(Error::NotFound(
                "database; initialize an explicit --db path first".into(),
            ));
        }
        let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        let app: i64 = connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
        let version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if app != 0x5245434F || version != 1 {
            return Err(Error::Conflict(
                "unrecognized database or schema version; file left unchanged".into(),
            ));
        }
        configure(&connection)?;
        Ok(Self {
            connection,
            project: project.into(),
        })
    }

    /// Creates a new schema or registers a project in a recognized existing database.
    pub fn init(path: &Path, project: &str, root: &Path) -> Result<Self> {
        validate::id(project)?;
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(Error::Invalid("repository root must be a directory".into()));
        }
        let root = root
            .to_str()
            .ok_or_else(|| Error::Invalid("repository root must be UTF-8".into()))?;
        let mut db = if path.exists() {
            Self::open(path, project)?
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            let mut connection =
                Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
            configure(&connection)?;
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(include_str!("../migrations/001_init.sql"))?;
            tx.commit()?;
            Self {
                connection,
                project: project.into(),
            }
        };
        let tx = db
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row("SELECT root FROM projects WHERE id=?1", [project], |row| {
                row.get(0)
            })
            .optional()?;
        if existing.as_deref().is_some_and(|value| value != root) {
            return Err(Error::Conflict(
                "project already belongs to a different repository root".into(),
            ));
        }
        tx.execute(
            "INSERT INTO projects(id,root) VALUES(?1,?2) ON CONFLICT(id) DO NOTHING",
            params![project, root],
        )?;
        tx.commit()?;
        Ok(db)
    }

    /// Returns this project's canonical root, or fails instead of crossing project scopes.
    pub fn root(&self) -> Result<std::path::PathBuf> {
        self.connection
            .query_row(
                "SELECT root FROM projects WHERE id=?1",
                [&self.project],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .map(Into::into)
            .ok_or_else(|| Error::NotFound("project; register it with init".into()))
    }
}

/// Enables integrity, durability and bounded lock waits on a recognized connection.
fn configure(connection: &Connection) -> Result<()> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=FULL;",
    )?;
    Ok(())
}

/// Requires a task in the project; active writes reject archived tasks.
pub fn task(connection: &Connection, project: &str, id: &str, active: bool) -> Result<()> {
    validate::id(id)?;
    let status: Option<String> = connection
        .query_row(
            "SELECT status FROM tasks WHERE project=?1 AND id=?2",
            params![project, id],
            |row| row.get(0),
        )
        .optional()?;
    let status = status.ok_or_else(|| Error::NotFound("task".into()))?;
    if active && status != "active" {
        return Err(Error::Conflict("task is archived".into()));
    }
    Ok(())
}

/// Checks optional task scope before any memory query or map construction.
pub fn scope(connection: &Connection, project: &str, id: Option<&str>) -> Result<()> {
    if let Some(id) = id {
        task(connection, project, id, false)?;
    }
    Ok(())
}

/// Appends an audit event containing identifiers only, without claim bodies.
pub fn event(connection: &Connection, project: &str, entry: Event<'_>) -> Result<()> {
    connection.execute("INSERT INTO events(project,task_id,event_type,actor,subject_id,reason) VALUES(?1,?2,?3,?4,?5,?6)",
        params![project,entry.task,entry.kind,entry.actor,entry.subject,entry.reason])?;
    Ok(())
}

/// Advances the project knowledge version inside its caller's mutation transaction.
pub fn touch(connection: &Connection, project: &str) -> Result<()> {
    connection.execute(
        "UPDATE projects SET knowledge_version=knowledge_version+1 WHERE id=?1",
        [project],
    )?;
    Ok(())
}
