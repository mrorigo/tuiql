//! Storage module for managing persistent data like query history and configuration
use rusqlite::{Connection, Result as SqlResult};
use std::path::PathBuf;
use std::time::SystemTime;
use tracing::{debug, error};

const HISTORY_TABLE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS query_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query TEXT NOT NULL,
    timestamp INTEGER NOT NULL,
    database_path TEXT NOT NULL,
    success BOOLEAN NOT NULL,
    duration_ms INTEGER,
    row_count INTEGER
)"#;

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub id: i64,
    pub query: String,
    pub timestamp: i64,
    pub database_path: String,
    pub success: bool,
    pub duration_ms: Option<i64>,
    pub row_count: Option<i64>,
}

impl HistoryEntry {
    pub fn new(
        query: String,
        database_path: String,
        success: bool,
        duration_ms: Option<i64>,
        row_count: Option<i64>,
    ) -> Self {
        Self {
            id: 0, // Will be set by database
            query,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            database_path,
            success,
            duration_ms,
            row_count,
        }
    }
}

pub struct Storage {
    conn: Connection,
}

impl Storage {
    /// Initialize storage with the given path
    pub fn new(path: PathBuf) -> SqlResult<Self> {
        debug!("Initializing storage at {:?}", path);
        let conn = Connection::open(path)?;
        let storage = Self { conn };
        storage.init()?;
        Ok(storage)
    }

    /// Initialize the storage schema
    fn init(&self) -> SqlResult<()> {
        self.conn.execute(HISTORY_TABLE_SQL, [])?;
        Ok(())
    }

    /// Add a query execution to history
    pub fn add_history(&self, entry: HistoryEntry) -> SqlResult<i64> {
        let result = self.conn.execute(
            "INSERT INTO query_history (query, timestamp, database_path, success, duration_ms, row_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                &entry.query,
                entry.timestamp,
                &entry.database_path,
                entry.success,
                entry.duration_ms,
                entry.row_count,
            ),
        );

        match result {
            Ok(_) => {
                let id = self.conn.last_insert_rowid();
                debug!("Added history entry with id {}", id);
                Ok(id)
            }
            Err(e) => {
                error!("Failed to add history entry: {}", e);
                Err(e)
            }
        }
    }

    /// Get the most recent history entries, limited to count
    pub fn get_recent_history(&self, count: usize) -> SqlResult<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, query, timestamp, database_path, success, duration_ms, row_count
             FROM query_history
             ORDER BY timestamp DESC
             LIMIT ?1",
        )?;

        let entries = stmt.query_map([count as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                query: row.get(1)?,
                timestamp: row.get(2)?,
                database_path: row.get(3)?,
                success: row.get(4)?,
                duration_ms: row.get(5)?,
                row_count: row.get(6)?,
            })
        })?;

        entries.collect()
    }

    /// Search query history
    pub fn search_history(&self, search_term: &str) -> SqlResult<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, query, timestamp, database_path, success, duration_ms, row_count
             FROM query_history
             WHERE query LIKE ?1
             ORDER BY timestamp DESC",
        )?;

        let entries = stmt.query_map([format!("%{}%", search_term)], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                query: row.get(1)?,
                timestamp: row.get(2)?,
                database_path: row.get(3)?,
                success: row.get(4)?,
                duration_ms: row.get(5)?,
                row_count: row.get(6)?,
            })
        })?;

        entries.collect()
    }

    /// Get a specific history entry by ID
    pub fn get_history_entry(&self, id: i64) -> SqlResult<Option<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, query, timestamp, database_path, success, duration_ms, row_count
             FROM query_history
             WHERE id = ?1",
        )?;

        let mut entries = stmt.query_map([id], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                query: row.get(1)?,
                timestamp: row.get(2)?,
                database_path: row.get(3)?,
                success: row.get(4)?,
                duration_ms: row.get(5)?,
                row_count: row.get(6)?,
            })
        })?;

        entries.next().transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use uuid::Uuid;

    fn create_test_storage() -> Storage {
        let mut path = temp_dir();
        path.push(format!("test_history_{}.db", Uuid::new_v4()));
        Storage::new(path).unwrap()
    }

    #[test]
    fn test_add_and_get_history() {
        let storage = create_test_storage();
        let entry = HistoryEntry::new(
            "SELECT * FROM test".to_string(),
            "test.db".to_string(),
            true,
            Some(100),
            Some(10),
        );

        let id = storage.add_history(entry.clone()).unwrap();
        let retrieved = storage.get_history_entry(id).unwrap().unwrap();

        assert_eq!(retrieved.query, entry.query);
        assert_eq!(retrieved.database_path, entry.database_path);
        assert_eq!(retrieved.success, entry.success);
        assert_eq!(retrieved.duration_ms, entry.duration_ms);
        assert_eq!(retrieved.row_count, entry.row_count);
    }

    #[test]
    fn test_get_recent_history() {
        let storage = create_test_storage();

        // Add multiple entries
        for i in 0..5 {
            let entry = HistoryEntry::new(
                format!("SELECT * FROM test{}", i),
                "test.db".to_string(),
                true,
                Some(100),
                Some(10),
            );
            storage.add_history(entry).unwrap();
        }

        let entries = storage.get_recent_history(3).unwrap();
        assert_eq!(entries.len(), 3);

        // Verify they're in reverse chronological order
        for i in 0..entries.len() - 1 {
            assert!(entries[i].timestamp >= entries[i + 1].timestamp);
        }
    }

    #[test]
    fn test_search_history() {
        let storage = create_test_storage();

        let entries = vec![
            "SELECT * FROM users",
            "INSERT INTO users VALUES (1)",
            "SELECT * FROM posts",
            "UPDATE users SET name = 'test'",
        ];

        for query in entries {
            let entry = HistoryEntry::new(
                query.to_string(),
                "test.db".to_string(),
                true,
                Some(100),
                Some(10),
            );
            storage.add_history(entry).unwrap();
        }

        let results = storage.search_history("users").unwrap();
        assert_eq!(results.len(), 3);

        let results = storage.search_history("posts").unwrap();
        assert_eq!(results.len(), 1);

        let results = storage.search_history("nonexistent").unwrap();
        assert_eq!(results.len(), 0);
    }
}
