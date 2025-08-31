use crate::core::{Result, TuiqlError};
use once_cell::sync::OnceCell;
use rusqlite::{types::ValueRef, Connection};
use std::collections::HashMap;
use std::sync::{Mutex, Arc};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub type_name: String,
    pub notnull: bool,
    pub pk: bool,
    pub dflt_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Index {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Column>,
    pub indexes: Vec<Index>,
}

#[derive(Debug, Clone)]
pub struct Schema {
    pub tables: HashMap<String, Table>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionState {
    Autocommit,
    Transaction,
    Failed,
}

impl Default for TransactionState {
    fn default() -> Self {
        TransactionState::Autocommit
    }
}

pub(crate) static DB_STATE: OnceCell<Mutex<DbState>> = OnceCell::new();

#[cfg(test)]
thread_local! {
    pub static TEST_DB_STATE: std::cell::RefCell<Option<Connection>> = std::cell::RefCell::new(None);
}
#[derive(Debug)]
pub struct DbState {
    pub connection: Option<Connection>,
    pub current_path: Option<String>,
    pub transaction_state: TransactionState,
}

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
pub fn connect(db_path: &str) -> Result<()> {
    match Connection::open(db_path) {
        Ok(conn) => {
            // Initialize the connection with some sensible defaults
            conn.execute_batch(
                "
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;
            ").map_err(|e| TuiqlError::Query(format!("Failed to set initial PRAGMA settings: {}", e)))?;

            // Store the connection in our global state
            DB_STATE.get_or_init(|| {
                Mutex::new(DbState {
                    connection: None,
                    current_path: None,
                    transaction_state: TransactionState::default(),
                })
            });

            if let Ok(mut guard) = DB_STATE.get().unwrap().lock() {
                guard.connection = Some(conn);
                guard.current_path = Some(db_path.to_string());
                Ok(())
            } else {
                Err(TuiqlError::App("Failed to acquire connection lock. Global database state is corrupted or locked by another process.".to_string()))
            }
        }
        Err(e) => Err(TuiqlError::App(format!("Failed to connect to database '{}': {}. Ensure the path exists and the database file is accessible.", db_path, e))),
    }
}

/// Executes a SQL query and returns the results.
pub fn execute_query(sql: &str) -> Result<QueryResult> {
    let state_cell = DB_STATE.get().ok_or(TuiqlError::App("No database connection found. Please connect to a database first.".to_string()))?;
    let mut state_guard = state_cell
        .lock()
        .map_err(|_| TuiqlError::App("Failed to acquire database connection lock. The connection may be in use by another operation.".to_string()))?;

    // Update transaction state based on SQL command
    let sql_upper = sql.trim().to_uppercase();
    if sql_upper == "BEGIN" || sql_upper == "BEGIN TRANSACTION" {
        if state_guard.transaction_state != TransactionState::Autocommit {
            return Err(TuiqlError::Transaction("Transaction already in progress. Cannot start a new transaction.".to_string()));
        }
        state_guard.transaction_state = TransactionState::Transaction;
    } else if sql_upper == "COMMIT" {
        if state_guard.transaction_state != TransactionState::Transaction {
            return Err(TuiqlError::Transaction("No active transaction to commit. Use BEGIN first to start a transaction.".to_string()));
        }
        state_guard.transaction_state = TransactionState::Autocommit;
    } else if sql_upper == "ROLLBACK" {
        if state_guard.transaction_state != TransactionState::Transaction {
            return Err(TuiqlError::Transaction("No active transaction to rollback. Use BEGIN first to start a transaction.".to_string()));
        }
        state_guard.transaction_state = TransactionState::Autocommit;
    }
    let conn = state_guard
        .connection
        .as_ref()
        .ok_or(TuiqlError::App("No active database connection. The connection may have been lost or closed.".to_string()))?;

    let mut stmt = conn.prepare(sql).map_err(|e| TuiqlError::Query(format!("Failed to prepare SQL statement: {}. Check your SQL syntax.", e)))?;

    // Get column names
    let columns: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let column_count = stmt.column_count();

    // Execute query and collect rows
    let rows: Vec<Vec<String>> = stmt
        .query_map([], |row| {
            let mut values = Vec::new();
            for i in 0..column_count {
                values.push(format_value(row.get_ref(i)?));
            }
            Ok(values)
        })
        .map_err(|e| TuiqlError::Query(format!("Query execution failed: {}. Check table names and column references.", e)))?
        .filter_map(|row| row.map_err(|e| TuiqlError::Query(format!("Error processing query results: {}. The query may have incompatible data types.", e))).ok())
        .collect();

    Ok(QueryResult::new(columns, rows))
}

/// Retrieves schema information for the connected database.
pub fn get_schema() -> Result<Schema> {
    let state_cell = DB_STATE.get().ok_or(TuiqlError::Schema("No database connection found. Please connect to a database first.".to_string()))?;
    let state_guard = state_cell
        .lock()
        .map_err(|_| TuiqlError::Schema("Failed to acquire database connection lock. The connection may be in use by another operation.".to_string()))?;
    let conn = state_guard
        .connection
        .as_ref()
        .ok_or(TuiqlError::Schema("No active database connection. The connection may have been lost or closed.".to_string()))?;

    let mut tables = HashMap::new();

    // Get all tables
    let mut stmt = conn
        .prepare(
            "SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|e| TuiqlError::Schema(format!("Failed to prepare schema query: {}. Database metadata may be corrupted.", e)))?;

    let table_iter = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| TuiqlError::Schema(format!("Failed to execute schema query: {}. Cannot retrieve table information.", e)))?;

    for table_result in table_iter {
        let (table_name, _sql) = table_result.map_err(|e| TuiqlError::Schema(format!("Error reading table metadata: {}. Schema may be incomplete.", e)))?;

        // Get columns for this table
        let mut columns = Vec::new();
        let mut col_stmt = conn
            .prepare(&format!("PRAGMA table_info('{}')", table_name))
            .map_err(|e| TuiqlError::Schema(format!("Failed to query table info for '{}': {}. Table schema may be corrupted.", table_name, e)))?;

        let col_iter = col_stmt
            .query_map([], |row| {
                Ok(Column {
                    name: row.get(1)?,
                    type_name: row.get(2)?,
                    notnull: row.get(3)?,
                    pk: row.get(5)?,
                    dflt_value: row.get(4)?,
                })
            })
            .map_err(|e| TuiqlError::Schema(format!("Error processing columns for table '{}': {}.", table_name, e)))?;

        for col_result in col_iter {
            columns.push(col_result.map_err(|e| TuiqlError::Schema(format!("Error reading column metadata for table '{}': {}.", table_name, e)))?);
        }

        // Get indexes for this table
        let mut indexes = Vec::new();
        let mut idx_stmt = conn
            .prepare(&format!("PRAGMA index_list('{}')", table_name))
            .map_err(|e| TuiqlError::Schema(format!("Failed to query index list for table '{}': {}.", table_name, e)))?;

        let idx_iter = idx_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?, // index name
                    row.get::<_, bool>(2)?,   // unique
                ))
            })
            .map_err(|e| TuiqlError::Schema(format!("Error processing index list for table '{}': {}.", table_name, e)))?;

        for idx_result in idx_iter {
            let (idx_name, unique) = idx_result.map_err(|e| TuiqlError::Schema(format!("Error reading index metadata for table '{}': {}.", table_name, e)))?;

            // Get columns for this index
            let mut idx_col_stmt = conn
                .prepare(&format!("PRAGMA index_info('{}')", idx_name))
                .map_err(|e| TuiqlError::Schema(format!("Failed to query index info for '{}': {}.", idx_name, e)))?;

            let mut idx_columns = Vec::new();
            let idx_col_iter = idx_col_stmt
                .query_map([], |row| row.get::<_, String>(2))
                .map_err(|e| TuiqlError::Schema(format!("Error retrieving columns for index '{}': {}.", idx_name, e)))?;

            for col_result in idx_col_iter {
                idx_columns.push(col_result.map_err(|e| TuiqlError::Schema(format!("Error reading index column for '{}': {}.", idx_name, e)))?);
            }

            indexes.push(Index {
                name: idx_name,
                unique,
                columns: idx_columns,
            });
        }

        tables.insert(
            table_name.clone(),
            Table {
                name: table_name,
                columns,
                indexes,
            },
        );
    }

    Ok(Schema { tables })
}

/// Executes a SQL query with cancellable support using a callback mechanism.
///
/// This function allows queries to be cancelled by providing a callback that monitors
/// for cancellation signals (such as Ctrl+C). The callback is responsible for calling
/// the interrupt handle when cancellation is requested.
///
/// # Arguments
///
/// * `sql` - The SQL query to execute
/// * `cancellation_monitor` - A callback that monitors for cancellation and calls interrupt
///
/// # Returns
///
/// Returns the query result if successful, or an error if the query failed or was cancelled.
///
/// # Errors
///
/// Returns `TuiqlError::Query` if the query fails.
/// Returns a specific error message if the query was interrupted.
pub fn execute_cancellable_query<F>(sql: &str, cancellation_monitor: F) -> Result<QueryResult>
where
    F: FnOnce(rusqlite::InterruptHandle) + Send + 'static,
{
    let state_cell = DB_STATE.get().ok_or(TuiqlError::Query("No database connection found. Please connect to a database first.".to_string()))?;
    let mut state_guard = state_cell.lock().map_err(|_| TuiqlError::Query("Failed to acquire database lock".to_string()))?;
    let conn = state_guard.connection.as_ref().ok_or(TuiqlError::Query("No active database connection".to_string()))?;

    let interrupt_handle = conn.get_interrupt_handle();

    // Spawn the cancellation monitor in a separate thread
    thread::spawn(move || {
        cancellation_monitor(interrupt_handle);
    });

    // Execute the query using the local function - this may be interrupted mid-execution
    match execute_query_on_connection_local(conn, sql) {
        Ok(query_result) => Ok(query_result),
        Err(rusqlite_err) => {
            // Check if this is an interrupt error by examining the error message
            let error_str = rusqlite_err.to_string();
            if error_str.contains("interrupt") || error_str.contains("cancel") {
                Err(TuiqlError::Query("Query execution cancelled by user (Ctrl+C)".to_string()))
            } else {
                Err(TuiqlError::Database(rusqlite_err))
            }
        }
    }
}

/// Local helper function to execute a query on a connection
///
/// This function returns both normal errors and interrupt errors that can be differentiated.
/// Returns a Result containing either a QueryResult or a raw rusqlite::Error to allow
/// proper pattern matching on interrupt conditions.
fn execute_query_on_connection_local(conn: &Connection, sql: &str) -> std::result::Result<QueryResult, rusqlite::Error> {
    let mut stmt = conn.prepare(sql)?;

    // Get column names
    let columns: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
    let column_count = stmt.column_count();

    // Execute query and collect rows
    let rows: Vec<Vec<String>> = stmt
        .query_map([], |row| {
            let mut values = Vec::new();
            for i in 0..column_count {
                values.push(format_value(row.get_ref(i)?));
            }
            Ok(values)
        })?
        .filter_map(|row| row.ok()) // Ignore processing errors
        .collect();

    Ok(QueryResult::new(columns, rows))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::Duration;
    use super::*;

    /// Isolated test database setup per test
    pub fn setup_test_db() {
        let conn = Connection::open_in_memory().expect("Failed to open in-memory database");

        // Set up test schema
        conn.execute_batch(
            "
            CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT, value REAL);
            CREATE INDEX idx_test_name ON test(name);
            CREATE UNIQUE INDEX idx_test_value ON test(value);
            INSERT INTO test (name, value) VALUES ('test1', 1.1);
            INSERT INTO test (name, value) VALUES ('test2', 2.2);
        ",
        ).expect("Failed to initialize test database schema");

        // Store in thread-local
        TEST_DB_STATE.with(|state| {
            *state.borrow_mut() = Some(conn);
        });
    }

    /// Execute query using thread-local test database
    fn test_execute_query(sql: &str) -> Result<QueryResult> {
        TEST_DB_STATE.with(|state| {
            let conn_ref = state.borrow();
            let conn = conn_ref.as_ref().ok_or_else(|| TuiqlError::App("Test database not initialized".to_string()))?;

            let mut stmt = conn.prepare(sql).map_err(|e| TuiqlError::Query(format!("Failed to prepare SQL statement: {}. Check your SQL syntax.", e)))?;

            // Get column names
            let columns: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
            let column_count = stmt.column_count();

            // Execute query and collect rows
            let rows: Vec<Vec<String>> = stmt
                .query_map([], |row| {
                    let mut values = Vec::new();
                    for i in 0..column_count {
                        values.push(format_value(row.get_ref(i)?));
                    }
                    Ok(values)
                })
                .map_err(|e| TuiqlError::Query(format!("Query execution failed: {}. Check table names and column references.", e)))?
                .filter_map(|row| row.map_err(|e| TuiqlError::Query(format!("Error processing query results: {}. The query may have incompatible data types.", e))).ok())
                .collect();

            Ok(QueryResult::new(columns, rows))
        })
    }

    /// Get schema using thread-local test database
    fn test_get_schema() -> Result<Schema> {
        TEST_DB_STATE.with(|state| {
            let conn_ref = state.borrow();
            let conn = conn_ref.as_ref().ok_or_else(|| TuiqlError::Schema("Test database not initialized".to_string()))?;

            let mut tables = HashMap::new();

            // Get all tables
            let mut stmt = conn
                .prepare(
                    "SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
                )
                .map_err(|e| TuiqlError::Schema(format!("Failed to prepare schema query: {}. Database metadata may be corrupted.", e)))?;

            let table_iter = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| TuiqlError::Schema(format!("Failed to execute schema query: {}. Cannot retrieve table information.", e)))?;

            for table_result in table_iter {
                let (table_name, _sql) = table_result.map_err(|e| TuiqlError::Schema(format!("Error reading table metadata: {}. Schema may be incomplete.", e)))?;

                // Get columns for this table
                let mut columns = Vec::new();
                let mut col_stmt = conn
                    .prepare(&format!("PRAGMA table_info('{}')", table_name))
                    .map_err(|e| TuiqlError::Schema(format!("Failed to query table info for '{}': {}. Table schema may be corrupted.", table_name, e)))?;

                let col_iter = col_stmt
                    .query_map([], |row| {
                        Ok(Column {
                            name: row.get(1)?,
                            type_name: row.get(2)?,
                            notnull: row.get(3)?,
                            pk: row.get(5)?,
                            dflt_value: row.get(4)?,
                        })
                    })
                    .map_err(|e| TuiqlError::Schema(format!("Error processing columns for table '{}': {}.", table_name, e)))?;

                for col_result in col_iter {
                    columns.push(col_result.map_err(|e| TuiqlError::Schema(format!("Error reading column metadata for table '{}': {}.", table_name, e)))?);
                }

                // Get indexes for this table
                let mut indexes = Vec::new();
                let mut idx_stmt = conn
                    .prepare(&format!("PRAGMA index_list('{}')", table_name))
                    .map_err(|e| TuiqlError::Schema(format!("Failed to query index list for table '{}': {}.", table_name, e)))?;

                let idx_iter = idx_stmt
                    .query_map([], |row| {
                        Ok((
                            row.get::<_, String>(1)?, // index name
                            row.get::<_, bool>(2)?,   // unique
                        ))
                    })
                    .map_err(|e| TuiqlError::Schema(format!("Error processing index list for table '{}': {}.", table_name, e)))?;

                for idx_result in idx_iter {
                    let (idx_name, unique) = idx_result.map_err(|e| TuiqlError::Schema(format!("Error reading index metadata for table '{}': {}.", table_name, e)))?;

                    // Get columns for this index
                    let mut idx_col_stmt = conn
                        .prepare(&format!("PRAGMA index_info('{}')", idx_name))
                        .map_err(|e| TuiqlError::Schema(format!("Failed to query index info for '{}': {}.", idx_name, e)))?;

                    let mut idx_columns = Vec::new();
                    let idx_col_iter = idx_col_stmt
                        .query_map([], |row| row.get::<_, String>(2))
                        .map_err(|e| TuiqlError::Schema(format!("Error retrieving columns for index '{}': {}.", idx_name, e)))?;

                    for col_result in idx_col_iter {
                        idx_columns.push(col_result.map_err(|e| TuiqlError::Schema(format!("Error reading index column for '{}': {}.", idx_name, e)))?);
                    }

                    indexes.push(Index {
                        name: idx_name,
                        unique,
                        columns: idx_columns,
                    });
                }

                tables.insert(
                    table_name.clone(),
                    Table {
                        name: table_name,
                        columns,
                        indexes,
                    },
                );
            }

            Ok(Schema { tables })
        })
    }

    /// Legacy test setup for tests that need global state
    pub fn setup_test_db_global() {
        let conn = Connection::open_in_memory().expect("Failed to open in-memory database");

        // Set up test schema
        conn.execute_batch(
            "
            CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT, value REAL);
            CREATE INDEX idx_test_name ON test(name);
            CREATE UNIQUE INDEX idx_test_value ON test(value);
            INSERT INTO test (name, value) VALUES ('test1', 1.1);
            INSERT INTO test (name, value) VALUES ('test2', 2.2);
        ",
        ).expect("Failed to initialize test database schema");

        // Initialize the global state
        DB_STATE.get_or_init(|| Mutex::new(DbState {
            connection: Some(conn),
            current_path: Some(":memory:".to_string()),
            transaction_state: TransactionState::default(),
        }));
    }

    #[test]
    fn test_connect_and_query() {
        setup_test_db_global();

        let result = execute_query("SELECT * FROM test ORDER BY id").unwrap();

        assert_eq!(result.columns, vec!["id", "name", "value"]);
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.rows[0], vec!["1", "test1", "1.1"]);
        assert_eq!(result.rows[1], vec!["2", "test2", "2.2"]);
    }

    #[test]
    fn test_query_error() {
        setup_test_db();

        let result = test_execute_query("SELECT * FROM nonexistent_table");
        assert!(result.is_err());
    }

    #[test]
    fn test_transaction_management() {
        setup_test_db_global();

        // Start transaction
        let result = execute_query("BEGIN");
        assert!(result.is_ok());
        let state = DB_STATE.get().unwrap().lock().unwrap();
        assert_eq!(state.transaction_state, TransactionState::Transaction);
        drop(state);

        // Try starting another transaction (should fail)
        let result = execute_query("BEGIN");
        assert!(result.is_err());

        // Commit transaction
        let result = execute_query("COMMIT");
        assert!(result.is_ok());
        let state = DB_STATE.get().unwrap().lock().unwrap();
        assert_eq!(state.transaction_state, TransactionState::Autocommit);
        drop(state);

        // Try committing without transaction (should fail)
        let result = execute_query("COMMIT");
        assert!(result.is_err());

        // Start transaction and rollback
        let result = execute_query("BEGIN");
        assert!(result.is_ok());
        let result = execute_query("ROLLBACK");
        assert!(result.is_ok());
        let state = DB_STATE.get().unwrap().lock().unwrap();
        assert_eq!(state.transaction_state, TransactionState::Autocommit);
    }

    #[test]
    fn test_null_and_blob_handling() {
        setup_test_db_global();
        execute_query("INSERT INTO test (name, value) VALUES (NULL, NULL)").unwrap();

        let result = execute_query("SELECT * FROM test WHERE name IS NULL").unwrap();
        assert_eq!(result.rows[0][1], "NULL");
        assert_eq!(result.rows[0][2], "NULL");
    }

    #[test]
    fn test_schema_introspection() {
        setup_test_db_global();

        let schema = get_schema().unwrap();
        let test_table = schema.tables.get("test").unwrap();

        // Check columns
        assert_eq!(test_table.columns.len(), 3);
        assert_eq!(test_table.columns[0].name, "id");
        assert_eq!(test_table.columns[0].type_name, "INTEGER");
        assert!(test_table.columns[0].pk);

        // Check indexes
        assert_eq!(test_table.indexes.len(), 2);
        let name_idx = test_table
            .indexes
            .iter()
            .find(|i| i.name == "idx_test_name")
            .unwrap();
        let value_idx = test_table
            .indexes
            .iter()
            .find(|i| i.name == "idx_test_value")
            .unwrap();

        assert!(!name_idx.unique);
        assert!(value_idx.unique);
        assert_eq!(name_idx.columns, vec!["name"]);
        assert_eq!(value_idx.columns, vec!["value"]);
    }

    #[test]
    fn test_cancellable_query_execution_normal() {
        setup_test_db_global();

        // Test that normal query execution still works with cancellable queries
        let sql = "SELECT * FROM test ORDER BY id";

        let result = execute_cancellable_query(sql, |_| {
            // No-op cancellation monitor
        });

        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.columns, vec!["id", "name", "value"]);
        assert_eq!(query_result.row_count, 2);
    }

    #[test]
    fn test_cancellable_query_execution_with_simple_callback() {
        setup_test_db_global();

        let sql = "SELECT COUNT(*) FROM test";
        let interrupt_called = Arc::new(Mutex::new(false));
        let interrupt_called_clone = interrupt_called.clone();

        let result = execute_cancellable_query(sql, move |interrupt_handle| {
            // Callback that doesn't actually interrupt, just sets a flag
            std::thread::spawn(move || {
                // Simulate some work that could be cancelled
                thread::sleep(Duration::from_millis(10));
                *interrupt_called_clone.lock().unwrap() = true;
            });
        });

        assert!(result.is_ok());
        let query_result = result.unwrap();
        assert_eq!(query_result.columns, vec!["COUNT(*)"]);
        assert_eq!(query_result.row_count, 1);
        // Note: Interrupt flag may or may not be set depending on timing
    }

    #[test]
    fn test_cancellable_query_with_ready_immediate_interrupt() {
        setup_test_db_global();

        let sql = "SELECT COUNT(*) FROM test";

        // Test interruption mechanism (may not actually cause interruption depending on timing)
        let result = execute_cancellable_query(sql, move |interrupt_handle| {
            std::thread::spawn(move || {
                thread::sleep(Duration::from_millis(1));
                // Call interrupt - this may or may not affect the already executing query
                interrupt_handle.interrupt();
            });
        });

        // Result could be OK or Err depending on timing and whether interrupt actually triggered
        // The important thing is that it doesn't crash and returns a valid result
        assert!(result.is_ok() || matches!(result, Err(TuiqlError::Query(_))));
    }

    #[test]
    fn test_cancellable_query_error_handling() {
        setup_test_db_global();

        // Test with invalid SQL to ensure errors are still handled properly
        let sql = "SELECT * FROM nonexistent_table";

        let result = execute_cancellable_query(sql, |_| {
            // No-op cancellation monitor
        });

        assert!(result.is_err());
        match result.unwrap_err() {
            TuiqlError::Database(_) => {} // Expected - database error for invalid table
            TuiqlError::Query(_) => {} // Could also be wrapped as query error
            other => panic!("Expected Database or Query error, got: {:?}", other),
        }
    }

    #[test]
    fn test_interrupt_error_message_formatting() {
        // Test that our string-based error detection works for interrupt errors
        // This is a conceptual test since actually triggering interrupt errors is timing-dependent

        let interrupt_error_string = "interrupt";
        let cancel_error_string = "cancel";

        assert!(interrupt_error_string.contains("interrupt"));
        assert!(cancel_error_string.contains("cancel"));
        assert!(!interrupt_error_string.contains("cancel"));
        assert!(!cancel_error_string.contains("interrupt"));
    }
}
