//! RequestMaster store: one SQLite file with the collections tree, environments, application state and the
//! history of sends.
//!
//! The schema is `schema.sql` (the three tables of version 1) plus `schema_v2.sql` (the `history` table):
//! four tables, no indexes; its version is kept in `PRAGMA user_version`.
//! Every operation that changes several rows runs in one transaction.

mod environments;
mod history;
mod nodes;
mod state;

use std::path::{Path, PathBuf};

use rusqlite::Connection;

pub use history::HistoryRow;
pub use nodes::TreeRow;

/// DDL of schema version 1, executed once on an empty database.
const SCHEMA: &str = include_str!("schema.sql");

/// DDL that turns schema version 1 into version 2: executed on an empty database right after [`SCHEMA`],
/// and alone on a database of version 1.
const SCHEMA_V2: &str = include_str!("schema_v2.sql");

/// Version written to `PRAGMA user_version` once [`SCHEMA`] and [`SCHEMA_V2`] are in place.
const SCHEMA_VERSION: i64 = 2;

/// Why a store operation failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// SQLite refused the operation; the text is SQLite's own message.
    #[error("database error: {0}")]
    Sqlite(String),
    /// The database folder cannot be found or created; the text is the system's message.
    #[error("file system error: {0}")]
    Io(String),
    /// The file was written by a newer application; holds the `user_version` found in it.
    #[error("the database has schema version {0}, this application supports up to {SCHEMA_VERSION}")]
    NewerSchema(i64),
    /// No node or environment with this id.
    #[error("not found: {0}")]
    NotFound(String),
    /// The tree rules forbid this placement of a node; the text names the broken rule.
    #[error("invalid move: {0}")]
    InvalidMove(String),
    /// The stored JSON of a node or an environment cannot be read back into the model.
    #[error("content of {id} cannot be read: {error}")]
    CorruptContent {
        /// Id of the node or environment the content belongs to.
        id: String,
        /// Message of the JSON parser.
        error: String,
    },
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        StoreError::Sqlite(error.to_string())
    }
}

/// The application's database: an open SQLite connection with the schema in place.
#[derive(Debug)]
pub struct Store {
    conn: Connection,
}

impl Store {
    /// Opens the database file at `path`.
    /// Creates parent directories and the file when missing.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| StoreError::Io(e.to_string()))?;
        }
        Self::prepare(Connection::open(path)?)
    }

    /// Opens an empty database that lives in memory until the store is dropped.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    /// Location of the application's database: `%LOCALAPPDATA%\RequestMaster\app.db`.
    pub fn default_path() -> Result<PathBuf, StoreError> {
        let local_app_data = std::env::var_os("LOCALAPPDATA")
            .filter(|value| !value.is_empty())
            .ok_or_else(|| StoreError::Io("LOCALAPPDATA is not set".to_string()))?;
        Ok(PathBuf::from(local_app_data).join("RequestMaster").join("app.db"))
    }

    /// Switches foreign keys on and brings the schema to [`SCHEMA_VERSION`]: an empty database gets both DDL
    /// files, a database of version 1 only the second one (its tables and data are not touched). Each case
    /// is one transaction.
    fn prepare(conn: Connection) -> Result<Self, StoreError> {
        conn.execute_batch("PRAGMA foreign_keys = ON")?;
        let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        let ddl = match version {
            SCHEMA_VERSION => return Ok(Store { conn }),
            newer if newer > SCHEMA_VERSION => return Err(StoreError::NewerSchema(newer)),
            1 => SCHEMA_V2.to_string(),
            _ => {
                let tables: i64 =
                    conn.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table'", [], |row| row.get(0))?;
                if tables != 0 {
                    return Err(StoreError::Sqlite(format!(
                        "the file has tables but schema version {version}: not a RequestMaster database"
                    )));
                }
                format!("{SCHEMA}\n{SCHEMA_V2}")
            }
        };
        conn.execute_batch(&format!("BEGIN;\n{ddl}\nPRAGMA user_version = {SCHEMA_VERSION};\nCOMMIT;"))?;
        Ok(Store { conn })
    }
}
