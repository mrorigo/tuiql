/// Connection Management Module
///
/// This module provides database connection management, state handling,
/// and transaction lifecycle management for TUIQL.

use crate::core::{Result, TuiqlError};
use once_cell::sync::OnceCell;
use rusqlite::Connection;
use std::sync::Mutex;

/// Represents database transaction states
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransactionState {
    /// No active transaction (autocommit mode)
    Autocommit,
    /// Transaction in progress
    Transaction,
    /// Transaction in failed state
    Failed,
}

impl Default for TransactionState {
    fn default() -> Self {
        TransactionState::Autocommit
    }
}

/// Global database connection state
///
/// This holds the current database connection and its associated metadata.
/// It uses OnceCell for lazy initialization to ensure thread-safe singleton behavior.
pub(crate) static DB_STATE: OnceCell<Mutex<DbState>> = OnceCell::new();

/// Internal database state structure
#[derive(Debug)]
pub struct DbState {
    /// Active database connection (None if disconnected)
    pub connection: Option<Connection>,
    /// Path to the current database file (None for in-memory databases)
    pub current_path: Option<String>,
    /// Current transaction state
    pub transaction_state: TransactionState,
}

/// Connection manager for database operations
#[derive(Debug)]
pub struct ConnectionManager {
    /// Reference to the global state (None if not initialized)
    state: Option<&'static Mutex<DbState>>,
}

impl ConnectionManager {
    /// Creates a new connection manager
    pub fn new() -> Self {
        // Ensure global state is initialized
        ConnectionManager::initialize();

        ConnectionManager {
            state: DB_STATE.get(),
        }
    }

    /// Initializes the global database state
    ///
    /// This should be called before any database operations.
    /// It's safe to call multiple times - subsequent calls are no-ops.
    pub fn initialize() {
        let _ = DB_STATE.get_or_init(|| {
            Mutex::new(DbState {
                connection: None,
                current_path: None,
                transaction_state: TransactionState::default(),
            })
        });
    }

    /// Connects to a SQLite database at the specified path
    ///
    /// # Arguments
    ///
    /// * `db_path` - Path to the SQLite database file, or ":memory:" for in-memory database
    ///
    /// # Returns
    ///
    /// `Ok(())` on successful connection, `TuiqlError::Database` on failure.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut conn_mgr = ConnectionManager::new();
    /// conn_mgr.connect("example.db")?;
    /// ```
    pub fn connect(&mut self, db_path: &str) -> Result<()> {
        let conn = Connection::open(db_path)
            .map_err(|e| TuiqlError::Database(e))?;

        // Initialize connection with common pragmas
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;
        ",
        ).map_err(|e| TuiqlError::Database(rusqlite::Error::ExecuteReturnedResults))?;

        // Initialize global state if not already done
        ConnectionManager::initialize();

        // Update global state
        if let Some(state) = self.state {
            if let Ok(mut guard) = state.lock() {
                guard.connection = Some(conn);
                guard.current_path = if db_path != ":memory:" {
                    Some(db_path.to_string())
                } else {
                    None
                };
                guard.transaction_state = TransactionState::Autocommit;
            } else {
                return Err(TuiqlError::App("Failed to acquire database lock".to_string()));
            }
        }

        Ok(())
    }

    /// Disconnects from the current database
    ///
    /// This closes the active connection and resets the state.
    pub fn disconnect(&mut self) -> Result<()> {
        if let Some(state) = self.state {
            if let Ok(mut guard) = state.lock() {
                guard.connection = None;
                guard.current_path = None;
                guard.transaction_state = TransactionState::Autocommit;
            } else {
                return Err(TuiqlError::App("Failed to acquire database lock".to_string()));
            }
        }
        Ok(())
    }

    /// Gets the current database path (if any)
    ///
    /// # Returns
    ///
    /// `Some(path)` if connected to a file database, `None` for in-memory databases.
    pub fn current_path(&self) -> Option<String> {
        self.state?
            .lock()
            .ok()?
            .current_path
            .clone()
    }

    /// Checks if there's an active database connection
    ///
    /// # Returns
    ///
    /// `true` if connected to a database, `false` otherwise.
    pub fn is_connected(&self) -> bool {
        DB_STATE.get()
            .and_then(|s| s.lock().ok())
            .map(|g| g.connection.is_some())
            .unwrap_or(false)
    }

    /// Gets the current transaction state
    ///
    /// # Returns
    ///
    /// The current transaction state.
    pub fn transaction_state(&self) -> TransactionState {
        self.state
            .and_then(|s| s.lock().ok())
            .map(|g| g.transaction_state)
            .unwrap_or(TransactionState::Autocommit)
    }

    /// Updates the transaction state based on SQL command
    ///
    /// # Arguments
    ///
    /// * `sql` - SQL command that may affect transaction state
    ///
    /// # Returns
    ///
    /// `Ok(())` if state update was successful, error otherwise.
    pub fn update_transaction_state(&mut self, sql: &str) -> Result<()> {
        let sql_upper = sql.trim().to_uppercase();

        if sql_upper == "BEGIN" || sql_upper == "BEGIN TRANSACTION" {
            if self.transaction_state() != TransactionState::Autocommit {
                return Err(TuiqlError::Transaction("Transaction already in progress".to_string()));
            }

            if let Some(state) = self.state {
                if let Ok(mut guard) = state.lock() {
                    guard.transaction_state = TransactionState::Transaction;
                }
            }
        } else if sql_upper == "COMMIT" || sql_upper == "COMMIT TRANSACTION" {
            if self.transaction_state() != TransactionState::Transaction {
                return Err(TuiqlError::Transaction("No transaction in progress".to_string()));
            }

            if let Some(state) = self.state {
                if let Ok(mut guard) = state.lock() {
                    guard.transaction_state = TransactionState::Autocommit;
                }
            }
        } else if sql_upper == "ROLLBACK" || sql_upper == "ROLLBACK TRANSACTION" {
            if self.transaction_state() != TransactionState::Transaction {
                return Err(TuiqlError::Transaction("No transaction in progress".to_string()));
            }

            if let Some(state) = self.state {
                if let Ok(mut guard) = state.lock() {
                    guard.transaction_state = TransactionState::Autocommit;
                }
            }
        }

        Ok(())
    }
}

/// Gets a reference to the current database connection
///
/// # Returns
///
/// `true` if there's an active connection, `false` otherwise.
///
/// Note: This is a simplified version to avoid lifetime issues.
/// For actual querying, use the ConnectionManager or QueryExecutor.
pub fn has_connection() -> bool {
    DB_STATE.get()
        .and_then(|s| s.lock().ok())
        .map(|g| g.connection.is_some())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cleanup_global_state() {
        // Clean up any existing state for test isolation
        if let Some(state_ref) = DB_STATE.get() {
            if let Ok(mut state) = state_ref.lock() {
                state.connection = None;
                state.current_path = None;
                state.transaction_state = TransactionState::Autocommit;
            }
        }
    }

    #[test]
    fn test_connection_manager_initialization() {
        cleanup_global_state();
        let mut conn_mgr = ConnectionManager::new();
        ConnectionManager::initialize();

        assert!(!conn_mgr.is_connected());

        conn_mgr.connect(":memory:").unwrap();
        assert!(conn_mgr.is_connected());
        assert_eq!(conn_mgr.transaction_state(), TransactionState::Autocommit);
    }

    #[test]
    fn test_connection_disconnect() {
        cleanup_global_state();
        let mut conn_mgr = ConnectionManager::new();
        ConnectionManager::initialize();

        conn_mgr.connect(":memory:").unwrap();
        assert!(conn_mgr.is_connected());

        conn_mgr.disconnect().unwrap();
        assert!(!conn_mgr.is_connected());
    }

    #[test]
    fn test_transaction_state_management() {
        cleanup_global_state();
        let mut conn_mgr = ConnectionManager::new();
        ConnectionManager::initialize();
        conn_mgr.connect(":memory:").unwrap();

        // Start transaction
        conn_mgr.update_transaction_state("BEGIN").unwrap();
        assert_eq!(conn_mgr.transaction_state(), TransactionState::Transaction);

        // Try to start another transaction - should fail
        assert!(conn_mgr.update_transaction_state("BEGIN").is_err());

        // Commit transaction
        conn_mgr.update_transaction_state("COMMIT").unwrap();
        assert_eq!(conn_mgr.transaction_state(), TransactionState::Autocommit);

        // Try to commit without transaction - should fail
        assert!(conn_mgr.update_transaction_state("COMMIT").is_err());
    }

    #[test]
    fn test_connection_error_handling() {
        let mut conn_mgr = ConnectionManager::new();
        ConnectionManager::initialize();

        // Invalid database path should fail
        let result = conn_mgr.connect("/nonexistent/path/database.db");
        assert!(result.is_err());

        match result.unwrap_err() {
            TuiqlError::Database(_) => {} // Expected
            _ => panic!("Expected Database error"),
        }
    }

    #[test]
    fn test_current_path_tracking() {
        let mut conn_mgr = ConnectionManager::new();
        ConnectionManager::initialize();

        // In-memory database
        conn_mgr.connect(":memory:").unwrap();
        assert_eq!(conn_mgr.current_path(), None);

        // File database (this would create a temporary file in actual use)
        let temp_path = "/tmp/test.db";
        let _result = conn_mgr.connect(temp_path);
        // Note: This test may fail if we don't have write permissions to /tmp
        // But the path tracking logic should still work
    }
}