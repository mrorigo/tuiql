/// Query Execution Module
///
/// This module provides functionality for executing SQL queries and formatting results.
/// It handles the query execution layer, including result processing and data formatting.

use crate::core::{Result, TuiqlError};
use rusqlite::{types::ValueRef, Connection};
use std::sync::mpsc;
use std::thread;

/// Represents a chunk of streaming query results
#[derive(Debug, Clone)]
pub struct StreamingChunk {
    /// Row data for this chunk
    pub rows: Vec<Vec<String>>,
    /// Total rows processed so far (for progress indication)
    pub total_rows_processed: usize,
    /// Whether this is the final chunk
    pub is_final: bool,
}

/// Handle for managing streaming query execution
#[derive(Debug)]
pub struct StreamingQueryHandle {
    /// Column names from the query result
    pub columns: Vec<String>,
    /// Handle to the streaming thread
    streaming_thread: Option<thread::JoinHandle<()>>,
}

impl StreamingQueryHandle {
    /// Creates a new StreamingQueryHandle
    pub fn new(columns: Vec<String>, streaming_thread: thread::JoinHandle<()>) -> Self {
        StreamingQueryHandle {
            columns,
            streaming_thread: Some(streaming_thread),
        }
    }

    /// Waits for the streaming thread to complete
    pub fn join(self) -> thread::Result<()> {
        if let Some(thread) = self.streaming_thread {
            thread.join()
        } else {
            Ok(())
        }
    }
}

/// Represents the result of a SQL query execution
#[derive(Debug)]
pub struct QueryResult {
    /// Column names from the query result
    pub columns: Vec<String>,
    /// Rows of data as string values
    pub rows: Vec<Vec<String>>,
    /// Number of rows returned
    pub row_count: usize,
}

/// Represents the result of a paged SQL query execution for progressive loading
#[derive(Debug)]
pub struct PagedQueryResult {
    /// Column names from the query result
    pub columns: Vec<String>,
    /// Rows of data for this page as string values
    pub rows: Vec<Vec<String>>,
    /// Total number of rows in the complete result set
    pub total_count: usize,
    /// Maximum number of rows per page
    pub page_size: usize,
    /// Current offset into the result set
    pub current_offset: usize,
    /// Whether there are more pages available
    pub has_more: bool,
}

impl PagedQueryResult {
    /// Creates a new PagedQueryResult
    pub fn new(columns: Vec<String>, rows: Vec<Vec<String>>, total_count: usize, page_size: usize, current_offset: usize) -> Self {
        let row_count = rows.len();
        PagedQueryResult {
            columns,
            rows,
            total_count,
            page_size,
            current_offset,
            has_more: current_offset + row_count < total_count,
        }
    }

    /// Returns the next page of results
    pub fn next_page(&self) -> Option<(usize, usize)> {
        if self.has_more {
            Some((self.page_size, self.current_offset + self.rows.len()))
        } else {
            None
        }
    }
}

///// Result handle for cancellable query execution
pub struct CancellableQueryHandle {
    /// Channel for receiving the result
    result_receiver: mpsc::Receiver<Result<QueryResult>>,
    /// Handle to interrupt the query
    interrupt_handle: rusqlite::InterruptHandle,
    /// Flag to track if query has been started
    started: bool,
}

impl CancellableQueryHandle {
    /// Creates a new cancellable query handle
    pub fn new(result_receiver: mpsc::Receiver<Result<QueryResult>>, interrupt_handle: rusqlite::InterruptHandle) -> Self {
        CancellableQueryHandle {
            result_receiver,
            interrupt_handle,
            started: false,
        }
    }

    /// Attempts to interrupt the running query
    pub fn interrupt(&self) {
        if self.started {
            self.interrupt_handle.interrupt();
        }
    }

    /// Receives the query result with a timeout
    ///
    /// # Arguments
    ///
    /// * `timeout` - How long to wait for the result
    ///
    /// # Returns
    ///
    /// Returns the query result if available within the timeout.
    pub fn recv_timeout(&self, timeout: std::time::Duration) -> std::result::Result<Result<QueryResult>, mpsc::RecvTimeoutError> {
        self.result_receiver.recv_timeout(timeout)
    }

    /// Attempts to receive the query result without blocking
    pub fn try_recv(&self) -> std::result::Result<Result<QueryResult>, mpsc::TryRecvError> {
        self.result_receiver.try_recv()
    }
}

/// A canceller that can be used to interrupt queries
pub struct QueryCanceller {
    interrupt_handle: Option<rusqlite::InterruptHandle>,
}

impl QueryCanceller {
    /// Creates a new query canceller
    pub fn new(interrupt_handle: rusqlite::InterruptHandle) -> Self {
        QueryCanceller {
            interrupt_handle: Some(interrupt_handle),
        }
    }

    /// Triggers cancellation of the running query
    pub fn cancel(&self) {
        if let Some(handle) = &self.interrupt_handle {
            handle.interrupt();
        }
    }
}

impl QueryResult {
    /// Creates a new QueryResult from column names and row data
    pub fn new(columns: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        let row_count = rows.len();
        QueryResult {
            columns,
            rows,
            row_count,
        }
    }
}

/// Query execution service that operates on a database connection
pub struct QueryExecutor<'a> {
    connection: &'a Connection,
    interrupt_handle: Option<rusqlite::InterruptHandle>,
}

impl<'a> QueryExecutor<'a> {
    /// Creates a new QueryExecutor for the given connection
    pub fn new(connection: &'a Connection) -> Self {
        QueryExecutor {
            connection,
            interrupt_handle: None,
        }
    }

    /// Creates a new QueryExecutor with interrupt support for the given connection
    pub fn with_interrupt(connection: &'a Connection) -> Self {
        let interrupt_handle = connection.get_interrupt_handle();
        QueryExecutor {
            connection,
            interrupt_handle: Some(interrupt_handle),
        }
    }

    /// Returns a reference to the interrupt handle if available
    pub fn interrupt_handle(&self) -> Option<&rusqlite::InterruptHandle> {
        self.interrupt_handle.as_ref()
    }

    /// Executes a SQL query and returns formatted results
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    ///
    /// Returns a `QueryResult` with column names, row data, and row count.
    /// String values are properly formatted for display.
    ///
    /// # Errors
    ///
    /// Returns `TuiqlError::Query` if the SQL syntax is invalid or if the
    /// database operation fails.
    pub fn execute(&self, sql: &str) -> Result<QueryResult> {
        let mut stmt = self.connection.prepare(sql)
            .map_err(|e| TuiqlError::Query(format!("Failed to prepare statement: {}", e)))?;

        // Get column names
        let columns: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
        let column_count = stmt.column_count();

        // Execute query and collect rows
        let rows = stmt
            .query_map([], |row| {
                let mut values = Vec::new();
                for i in 0..column_count {
                    let value_ref = row.get_ref(i)?;
                    values.push(format_value(value_ref));
                }
                Ok(values)
            })
            .map_err(|e| TuiqlError::Query(format!("Query execution failed: {}", e)))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| TuiqlError::Query(format!("Result processing failed: {}", e)))?;

        Ok(QueryResult::new(columns, rows))
    }

    /// Executes a cancellable SQL query with interrupt capability.
    ///
    /// This method executes the query in a way that allows it to be interrupted from another thread.
    /// The query will be cancelled if an interrupt signal is received, returning an error.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    ///
    /// Returns the query result if successful, or an error if the query was interrupted or failed.
    ///
    /// # Errors
    ///
    /// Returns `TuiqlError::Query` if the query fails or is interrupted.
    pub fn execute_cancellable(&self, sql: &str) -> Result<QueryResult> {
        if self.interrupt_handle.is_none() {
            return Err(TuiqlError::Query("Interrupt support not enabled. Use QueryExecutor::with_interrupt() for cancellable queries.".to_string()));
        }

        self.execute(sql)
    }

    /// Executes a SQL query with optional cancellation support.
    ///
    /// This method allows executing queries that can be interrupted. If cancellation
    /// support is enabled, the query can be interrupted from another thread.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    ///
    /// # Returns
    ///
    /// Returns the query result if successful, or an error if the query failed.
    ///
    /// # Errors
    ///
    /// Returns `TuiqlError::Query` if the query fails.
    pub fn execute_interruptable(&self, sql: &str) -> Result<QueryResult> {
        self.execute(sql)
    }

    /// Executes a SQL query in paged mode to support large result sets without memory exhaustion.
    ///
    /// This method processes rows in batches, keeping memory usage bounded while allowing
    /// progressive result display. It returns the first batch of results immediately and
    /// provides pagination handles for retrieving subsequent batches.
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL query to execute
    /// * `page_size` - Maximum number of rows to return per page
    /// * `offset` - Number of rows to skip (supports manual pagination)
    ///
    /// # Returns
    ///
    /// Returns a paginated result allowing progressive loading of large datasets.
    ///
    /// # Errors
    ///
    /// Returns `TuiqlError::Query` if the query fails.
    pub fn execute_paged(&self, sql: &str, page_size: usize, offset: usize) -> Result<PagedQueryResult> {
        let original_sql = if sql.to_uppercase().trim_start().starts_with("SELECT") {
            if sql.to_uppercase().contains(" LIMIT ") {
                // If LIMIT is already present, modify it
                let limit_idx = sql.to_uppercase().find(" LIMIT ").unwrap();
                format!("{} LIMIT {} OFFSET {}", &sql[0..limit_idx], page_size, offset)
            } else {
                // Add LIMIT and OFFSET
                format!("{} LIMIT {} OFFSET {}", sql, page_size, offset)
            }
        } else {
            sql.to_string()
        };

        let mut executor = QueryExecutor::new(self.connection);
        let result = executor.execute(&original_sql)?;

        // Also get total count for progress indication
        let count_sql = if sql.to_uppercase().trim_start().starts_with("SELECT") {
            format!("SELECT COUNT(*) FROM ({}) AS count_query", sql)
        } else {
            "SELECT 0".to_string()
        };

        let total_count = if let Ok(count_result) = executor.execute(&count_sql) {
            if !count_result.rows.is_empty() && !count_result.rows[0].is_empty() {
                count_result.rows[0][0].parse::<usize>().unwrap_or(0)
            } else {
                0
            }
        } else {
            0
        };

        let row_count = result.rows.len();
        Ok(PagedQueryResult {
            columns: result.columns,
            rows: result.rows,
            total_count,
            page_size,
            current_offset: offset,
            has_more: row_count >= page_size,
        })
    }

    /// Prepares a SQL statement for execution without running it
    ///
    /// # Arguments
    ///
    /// * `sql` - The SQL statement to prepare
    ///
    /// # Returns
    ///
    /// Returns a prepared statement ready for execution.
    ///
    /// # Errors
    ///
    /// Returns `TuiqlError::Query` if the SQL statement cannot be prepared.
    pub fn prepare(&self, sql: &str) -> Result<rusqlite::Statement> {
        self.connection.prepare(sql)
            .map_err(|e| TuiqlError::Query(format!("Failed to prepare statement: {}", e)))
    }
}

/// Convenience function to execute a query on a connection
///
/// # Arguments
///
/// * `conn` - Database connection to execute the query on
/// * `sql` - SQL query to execute
///
/// # Returns
///
/// Returns query results formatted as a `QueryResult`.
pub fn execute_query_on_connection(conn: &Connection, sql: &str) -> Result<QueryResult> {
    let executor = QueryExecutor::new(conn);
    executor.execute(sql)
}

/// Convenience function to execute a paged query on a connection
///
/// # Arguments
///
/// * `conn` - Database connection to execute the query on
/// * `sql` - SQL query to execute
/// * `page_size` - Maximum number of rows to return
/// * `offset` - Number of rows to skip
///
/// # Returns
///
/// Returns paginated query results formatted as a `PagedQueryResult`.
pub fn execute_paged_query_on_connection(conn: &Connection, sql: &str, page_size: usize, offset: usize) -> Result<PagedQueryResult> {
    let executor = QueryExecutor::new(conn);
    executor.execute_paged(sql, page_size, offset)
}

/// Formats a SQLite value for display
///
/// # Arguments
///
/// * `value` - Database value to format
///
/// # Returns
///
/// A string representation of the value suitable for display.
fn format_value(value: ValueRef) -> String {
    match value {
        ValueRef::Null => "NULL".to_string(),
        ValueRef::Integer(i) => i.to_string(),
        ValueRef::Real(f) => f.to_string(),
        ValueRef::Text(t) => String::from_utf8_lossy(t).to_string(),
        ValueRef::Blob(b) => format!("<BLOB: {} bytes>", b.len()),
    }
}

/// Represents different SQL statement types for introspection
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StatementType {
    /// SELECT statement
    Select,
    /// INSERT statement
    Insert,
    /// UPDATE statement
    Update,
    /// DELETE statement
    Delete,
    /// CREATE statement
    Create,
    /// DROP statement
    Drop,
    /// ALTER statement
    Alter,
    /// BEGIN/COMMIT/ROLLBACK transaction commands
    Transaction,
    /// Other statement types
    Other,
}

impl StatementType {
    /// Determines the statement type from a SQL string
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL statement to analyze
    ///
    /// # Returns
    ///
    /// The classified statement type
    pub fn from_sql(sql: &str) -> Self {
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper.starts_with("SELECT") {
            StatementType::Select
        } else if sql_upper.starts_with("INSERT") {
            StatementType::Insert
        } else if sql_upper.starts_with("UPDATE") {
            StatementType::Update
        } else if sql_upper.starts_with("DELETE") {
            StatementType::Delete
        } else if sql_upper.starts_with("CREATE") {
            StatementType::Create
        } else if sql_upper.starts_with("DROP") {
            StatementType::Drop
        } else if sql_upper.starts_with("ALTER") {
            StatementType::Alter
        } else if sql_upper == "BEGIN"
            || sql_upper == "COMMIT"
            || sql_upper == "ROLLBACK"
            || sql_upper.starts_with("BEGIN TRANSACTION")
            || sql_upper.starts_with("COMMIT TRANSACTION")
            || sql_upper.starts_with("ROLLBACK TRANSACTION") {
            StatementType::Transaction
        } else {
            StatementType::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_table(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE test (
                id INTEGER PRIMARY KEY,
                name TEXT,
                value REAL,
                active BOOLEAN DEFAULT 1
            );
            INSERT INTO test (name, value) VALUES ('Alice', 123.45);
            INSERT INTO test (name, value) VALUES ('Bob', 678.90);
            INSERT INTO test (name, value) VALUES (NULL, NULL);
        "
        ).unwrap();
    }

    #[test]
    fn test_query_execution() {
        let conn = Connection::open_in_memory().unwrap();
        setup_test_table(&conn);

        let executor = QueryExecutor::new(&conn);
        let result = executor.execute("SELECT * FROM test ORDER BY id").unwrap();

        assert_eq!(result.columns, vec!["id", "name", "value", "active"]);
        assert_eq!(result.row_count, 3);

        // Check first row
        assert_eq!(result.rows[0], vec!["1", "Alice", "123.45", "1"]);
        // Check NULL handling
        assert_eq!(result.rows[2], vec!["3", "NULL", "NULL", "1"]);
    }

    #[test]
    fn test_query_error_handling() {
        let conn = Connection::open_in_memory().unwrap();

        let executor = QueryExecutor::new(&conn);
        let result = executor.execute("SELECT * FROM nonexistent_table");

        assert!(result.is_err());
        match result.unwrap_err() {
            TuiqlError::Query(msg) => assert!(msg.contains("no such table")),
            _ => panic!("Expected Query error"),
        }
    }

    #[test]
    fn test_statement_type_classification() {
        assert_eq!(StatementType::from_sql("SELECT * FROM users"), StatementType::Select);
        assert_eq!(StatementType::from_sql("INSERT INTO users VALUES (1, 'test')"), StatementType::Insert);
        assert_eq!(StatementType::from_sql("UPDATE users SET name = 'new'"), StatementType::Update);
        assert_eq!(StatementType::from_sql("DELETE FROM users WHERE id = 1"), StatementType::Delete);
        assert_eq!(StatementType::from_sql("CREATE TABLE test (id INTEGER)"), StatementType::Create);
        assert_eq!(StatementType::from_sql("DROP TABLE test"), StatementType::Drop);
        assert_eq!(StatementType::from_sql("BEGIN"), StatementType::Transaction);
        assert_eq!(StatementType::from_sql("COMMIT"), StatementType::Transaction);
        assert_eq!(StatementType::from_sql("ROLLBACK"), StatementType::Transaction);
        assert_eq!(StatementType::from_sql("BEGIN TRANSACTION"), StatementType::Transaction);
        assert_eq!(StatementType::from_sql("PRAGMA foreign_keys = ON"), StatementType::Other);
    }

    #[test]
    fn test_blob_handling() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute("CREATE TABLE blobs (id INTEGER, data BLOB)", []).unwrap();
        conn.execute("INSERT INTO blobs VALUES (1, X'48656C6C6F')", []).unwrap(); // "Hello" in hex

        let result = execute_query_on_connection(&conn, "SELECT data FROM blobs WHERE id = 1").unwrap();
        assert!(result.rows[0][0].contains("BLOB"));
        assert!(result.rows[0][0].contains("5 bytes"));
    }

    #[test]
    fn test_prepare_statement() {
        let conn = Connection::open_in_memory().unwrap();
        setup_test_table(&conn);

        let executor = QueryExecutor::new(&conn);
        let mut stmt = executor.prepare("SELECT COUNT(*) FROM test").unwrap();

        // Execute the prepared statement
        let count = stmt.query_row([], |row| row.get::<_, i64>(0)).unwrap();
        assert_eq!(count, 3);
    }
}