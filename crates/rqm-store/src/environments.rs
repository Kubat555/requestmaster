//! Environments and globals: the `environment` table.

use rqm_model::{Environment, GLOBALS_ID};
use rusqlite::OptionalExtension;

use crate::{Store, StoreError};

/// Builds an environment from the table's columns.
fn read_environment(id: &str, name: &str, content: &str) -> Result<Environment, StoreError> {
    Environment::from_parts(id, name, content)
        .map_err(|e| StoreError::CorruptContent { id: id.to_string(), error: e.to_string() })
}

impl Store {
    /// Environments ordered by `name.to_lowercase()` — sorted in Rust, because SQLite `NOCASE` folds ASCII only;
    /// globals are not included.
    pub fn environments(&self) -> Result<Vec<Environment>, StoreError> {
        let mut statement = self.conn.prepare("SELECT id, name, content FROM environment WHERE id <> ?1")?;
        let rows = statement.query_map([GLOBALS_ID], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
        })?;
        let mut environments = Vec::new();
        for row in rows {
            let (id, name, content) = row?;
            environments.push(read_environment(&id, &name, &content)?);
        }
        environments.sort_by_cached_key(|environment| environment.name.to_lowercase());
        Ok(environments)
    }

    /// Stored globals, or `Environment::empty_globals()` when nothing is stored yet.
    pub fn globals(&self) -> Result<Environment, StoreError> {
        let stored: Option<(String, String)> = self
            .conn
            .query_row("SELECT name, content FROM environment WHERE id = ?1", [GLOBALS_ID], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .optional()?;
        match stored {
            Some((name, content)) => read_environment(GLOBALS_ID, &name, &content),
            None => Ok(Environment::empty_globals()),
        }
    }

    /// Inserts the environment or replaces the stored one with the same id (globals have the id `globals`).
    pub fn save_environment(&mut self, env: &Environment) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO environment (id, name, content) VALUES (?1, ?2, ?3) \
             ON CONFLICT (id) DO UPDATE SET name = excluded.name, content = excluded.content",
            [&env.id, &env.name, &env.content_json()],
        )?;
        Ok(())
    }

    /// Deletes the environment; `NotFound` when there is no environment with this id.
    pub fn delete_environment(&mut self, id: &str) -> Result<(), StoreError> {
        match self.conn.execute("DELETE FROM environment WHERE id = ?1", [id])? {
            0 => Err(StoreError::NotFound(id.to_string())),
            _ => Ok(()),
        }
    }
}
