//! History of sends: the `history` table.

use rusqlite::{params, OptionalExtension};

use crate::{Store, StoreError};

/// One history entry as the list shows it: everything except the detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    /// Number of the entry, assigned by SQLite; a newer entry has a greater one.
    pub id: i64,
    /// Moment the send started, as the caller passed it to [`Store::history_add`].
    pub sent_at: String,
    /// JSON of the caller with the short facts for the list.
    pub summary: String,
}

/// SQLite takes row counts and limits as `i64`; a count beyond it means "no limit" all the same.
fn sql_count(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

impl Store {
    /// Appends an entry and deletes the oldest ones beyond `keep` (one transaction); returns the new id.
    pub fn history_add(&mut self, sent_at: &str, summary: &str, detail: &str, keep: usize) -> Result<i64, StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO history (sent_at, summary, detail) VALUES (?1, ?2, ?3)", [sent_at, summary, detail])?;
        let id = tx.last_insert_rowid();
        tx.execute(
            "DELETE FROM history WHERE id NOT IN (SELECT id FROM history ORDER BY id DESC LIMIT ?1)",
            [sql_count(keep)],
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// Newest first; `before_id` = only entries older than that id.
    pub fn history_list(&self, limit: usize, before_id: Option<i64>) -> Result<Vec<HistoryRow>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT id, sent_at, summary FROM history WHERE ?1 IS NULL OR id < ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![before_id, sql_count(limit)], |row| {
            Ok(HistoryRow { id: row.get(0)?, sent_at: row.get(1)?, summary: row.get(2)? })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<HistoryRow>>>()?)
    }

    /// The detail JSON of the entry; `NotFound` when there is no entry with this id.
    pub fn history_detail(&self, id: i64) -> Result<String, StoreError> {
        self.conn
            .query_row("SELECT detail FROM history WHERE id = ?1", [id], |row| row.get(0))
            .optional()?
            .ok_or_else(|| StoreError::NotFound(id.to_string()))
    }

    /// Deletes every history entry.
    pub fn history_clear(&mut self) -> Result<(), StoreError> {
        self.conn.execute("DELETE FROM history", [])?;
        Ok(())
    }
}
