use once_cell::sync::OnceCell;
use rusqlite::{types::ValueRef, Connection};
use std::sync::Mutex;

static DB_CONNECTION: OnceCell<Mutex<Option<Connection>>> = OnceCell::new();

#[derive(Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub row_count: usize,
}

impl QueryResult {
    fn new(columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        let row_count = rows.len();
        QueryResult {
            columns,
            rows,
            row_count,
        }
    }
}

fn format_value(value: ValueRef) -> String {
    match value {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        ValueRef::Blob(b) => format!("<BLOB: {} bytes>", b.len()),
    }
}

/// Attempts to connect to a SQLite database using the provided `db_path`.
pub fn connect(db_path: &str) -> Result<(), String> {
    match Connection::open(db_path) {
        Ok(conn) => {
            // Initialize the connection with some sensible defaults
            if let Err(e) = conn.execute_batch(
                "
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;
            ",
            ) {
                return Err(format!("Failed to set initial pragmas: {}", e));
            }

            // Store the connection in our global state
            DB_CONNECTION.get_or_init(|| Mutex::new(None));
            if let Ok(mut guard) = DB_CONNECTION.get().unwrap().lock() {
                *guard = Some(conn);
                Ok(())
            } else {
                Err("Failed to acquire connection lock".to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Executes a SQL query and returns the results.
pub fn execute_query(sql: &str) -> Result<QueryResult, String> {
    let conn_cell = DB_CONNECTION.get().ok_or("No database connection")?;
    let conn_guard = conn_cell
        .lock()
        .map_err(|_| "Failed to acquire connection lock")?;
    let conn = conn_guard.as_ref().ok_or("No active database connection")?;

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    // Get column names
    let columns: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let column_count = stmt.column_count();

    // Execute query and collect rows
    let rows = stmt
        .query_map([], |row| {
            let mut values = Vec::new();
            for i in 0..column_count {
                values.push(format_value(row.get_ref(i).unwrap()));
            }
            Ok(values)
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(QueryResult::new(columns, rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_test_db() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT, value REAL);
            INSERT INTO test (name, value) VALUES ('test1', 1.1);
            INSERT INTO test (name, value) VALUES ('test2', 2.2);
        ",
        )
        .unwrap();

        DB_CONNECTION.get_or_init(|| Mutex::new(None));
        let mut guard = DB_CONNECTION.get().unwrap().lock().unwrap();
        *guard = Some(conn);
    }

    #[test]
    fn test_connect_and_query() {
        setup_test_db();

        let result = execute_query("SELECT * FROM test ORDER BY id").unwrap();

        assert_eq!(result.columns, vec!["id", "name", "value"]);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0], vec!["1", "test1", "1.1"]);
        assert_eq!(result.rows[1], vec!["2", "test2", "2.2"]);
    }

    #[test]
    fn test_query_error() {
        setup_test_db();

        let result = execute_query("SELECT * FROM nonexistent_table");
        assert!(result.is_err());
    }

    #[test]
    fn test_null_and_blob_handling() {
        setup_test_db();
        execute_query("INSERT INTO test (name, value) VALUES (NULL, NULL)").unwrap();

        let result = execute_query("SELECT * FROM test WHERE name IS NULL").unwrap();
        assert_eq!(result.rows[0][1], "NULL");
        assert_eq!(result.rows[0][2], "NULL");
    }
}
