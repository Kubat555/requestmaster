//! Application state: the `app_state` key-value table.

use rusqlite::OptionalExtension;

use crate::{Store, StoreError};

impl Store {
    /// Value stored under `key`, or `None` when the key was never written.
    pub fn state_get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT value FROM app_state WHERE key = ?1", [key], |row| row.get(0))
            .optional()?)
    }

    /// Writes `value` under `key`, replacing the previous value.
    pub fn state_set(&mut self, key: &str, value: &str) -> Result<(), StoreError> {
        self.conn.execute(
            "INSERT INTO app_state (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }
}
