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

    /// Removes the key; a key that was never written is not an error.
    pub fn state_delete(&mut self, key: &str) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM app_state WHERE key = ?1", [key])?;
        Ok(())
    }

    /// `(key, value)` of every key that starts with `prefix` (compared literally), ordered by key.
    pub fn state_entries(&self, prefix: &str) -> Result<Vec<(String, String)>, StoreError> {
        // `substr` instead of `LIKE`: `%` and `_` in the prefix stay ordinary characters.
        let mut statement = self
            .conn
            .prepare("SELECT key, value FROM app_state WHERE substr(key, 1, length(?1)) = ?1 ORDER BY key")?;
        let entries = statement.query_map([prefix], |row| Ok((row.get(0)?, row.get(1)?)))?.collect::<Result<_, _>>()?;
        Ok(entries)
    }
}
