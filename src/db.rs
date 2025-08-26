use once_cell::sync::OnceCell;
use rusqlite::{types::ValueRef, Connection};
use std::collections::HashMap;
use std::sync::Mutex;

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
                Err("Failed to acquire connection lock".to_string())
            }
        }
        Err(e) => Err(e.to_string()),
    }
}

/// Executes a SQL query and returns the results.
pub fn execute_query(sql: &str) -> Result<QueryResult, String> {
    let state_cell = DB_STATE.get().ok_or("No database connection")?;
    let mut state_guard = state_cell
        .lock()
        .map_err(|_| "Failed to acquire connection lock")?;

    // Update transaction state based on SQL command
    let sql_upper = sql.trim().to_uppercase();
    if sql_upper == "BEGIN" || sql_upper == "BEGIN TRANSACTION" {
        if state_guard.transaction_state != TransactionState::Autocommit {
            return Err("Transaction already in progress".to_string());
        }
        state_guard.transaction_state = TransactionState::Transaction;
    } else if sql_upper == "COMMIT" {
        if state_guard.transaction_state != TransactionState::Transaction {
            return Err("No transaction in progress".to_string());
        }
        state_guard.transaction_state = TransactionState::Autocommit;
    } else if sql_upper == "ROLLBACK" {
        if state_guard.transaction_state != TransactionState::Transaction {
            return Err("No transaction in progress".to_string());
        }
        state_guard.transaction_state = TransactionState::Autocommit;
    }
    let conn = state_guard
        .connection
        .as_ref()
        .ok_or("No active database connection")?;

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

/// Retrieves schema information for the connected database.
pub fn get_schema() -> Result<Schema, String> {
    let state_cell = DB_STATE.get().ok_or("No database connection")?;
    let state_guard = state_cell
        .lock()
        .map_err(|_| "Failed to acquire connection lock")?;
    let conn = state_guard
        .connection
        .as_ref()
        .ok_or("No active database connection")?;

    let mut tables = HashMap::new();

    // Get all tables
    let mut stmt = conn
        .prepare(
            "SELECT name, sql FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
        )
        .map_err(|e| e.to_string())?;

    let table_iter = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;

    for table_result in table_iter {
        let (table_name, _sql) = table_result.map_err(|e| e.to_string())?;

        // Get columns for this table
        let mut columns = Vec::new();
        let mut col_stmt = conn
            .prepare(&format!("PRAGMA table_info('{}')", table_name))
            .map_err(|e| e.to_string())?;

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
            .map_err(|e| e.to_string())?;

        for col_result in col_iter {
            columns.push(col_result.map_err(|e| e.to_string())?);
        }

        // Get indexes for this table
        let mut indexes = Vec::new();
        let mut idx_stmt = conn
            .prepare(&format!("PRAGMA index_list('{}')", table_name))
            .map_err(|e| e.to_string())?;

        let idx_iter = idx_stmt
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(1)?, // index name
                    row.get::<_, bool>(2)?,   // unique
                ))
            })
            .map_err(|e| e.to_string())?;

        for idx_result in idx_iter {
            let (idx_name, unique) = idx_result.map_err(|e| e.to_string())?;

            // Get columns for this index
            let mut idx_col_stmt = conn
                .prepare(&format!("PRAGMA index_info('{}')", idx_name))
                .map_err(|e| e.to_string())?;

            let mut idx_columns = Vec::new();
            let idx_col_iter = idx_col_stmt
                .query_map([], |row| row.get::<_, String>(2))
                .map_err(|e| e.to_string())?;

            for col_result in idx_col_iter {
                idx_columns.push(col_result.map_err(|e| e.to_string())?);
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn setup_test_db() {
        // Initialize the connection singleton first
        DB_STATE.get_or_init(|| {
            let conn = Connection::open_in_memory().unwrap();
            // Reset the database to ensure we have exactly 2 rows
            conn.execute_batch(
                "
                CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT, value REAL);
                CREATE INDEX idx_test_name ON test(name);
                CREATE UNIQUE INDEX idx_test_value ON test(value);
                INSERT INTO test (name, value) VALUES ('test1', 1.1);
                INSERT INTO test (name, value) VALUES ('test2', 2.2);
            ",
            )
            .unwrap();

            Mutex::new(DbState {
                connection: Some(conn),
                current_path: Some(":memory:".to_string()),
                transaction_state: TransactionState::default(),
            })
        });
    }

    /// Helper function to reset the test database
    pub fn reset_test_db() {
        if let Ok(mut guard) = DB_STATE.get().unwrap().lock() {
            if let Some(_conn) = guard.connection.take() {
                // Create a new connection and reset the database
                let new_conn = Connection::open_in_memory().unwrap();
                new_conn
                    .execute_batch(
                        "
                    CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT, value REAL);
                    CREATE INDEX idx_test_name ON test(name);
                    CREATE UNIQUE INDEX idx_test_value ON test(value);
                    INSERT INTO test (name, value) VALUES ('test1', 1.1);
                    INSERT INTO test (name, value) VALUES ('test2', 2.2);
                ",
                    )
                    .unwrap();
                guard.connection = Some(new_conn);
                guard.transaction_state = TransactionState::default();
            }
        }
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
    fn test_transaction_management() {
        setup_test_db();

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
        setup_test_db();
        execute_query("INSERT INTO test (name, value) VALUES (NULL, NULL)").unwrap();

        let result = execute_query("SELECT * FROM test WHERE name IS NULL").unwrap();
        assert_eq!(result.rows[0][1], "NULL");
        assert_eq!(result.rows[0][2], "NULL");
    }
    #[test]
    fn test_schema_introspection() {
        setup_test_db();

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
}
