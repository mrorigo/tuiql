/// TUIQL Error Module
///
/// This module defines comprehensive error types for the TUIQL application.
/// It provides structured error handling with proper error propagation and
/// user-friendly error messages.
use thiserror::Error;

/// Comprehensive error type for the TUIQL application.
///
/// This enum covers all error scenarios that can occur within TUIQL:
/// - Database operations (connection, queries, transactions)
/// - Query execution and parsing
/// - Configuration management
/// - UI operations and export formats
/// - File system operations
/// - JSON parsing and validation
#[derive(Error, Debug)]
pub enum TuiqlError {
    /// Database-related errors from SQLite operations
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// SQL query errors (syntax, execution, missing tables, etc.)
    #[error("Query error: {0}")]
    Query(String),

    /// Configuration loading and validation errors
    #[error("Configuration error: {0}")]
    Config(String),

    /// UI-related errors (export formats, display issues)
    #[error("UI error: {0}")]
    Ui(String),

    /// File system and I/O errors
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing and validation errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Command validation and execution errors
    #[error("Command error: {0}")]
    Command(String),

    /// Transaction-related errors
    #[error("Transaction error: {0}")]
    Transaction(String),

    /// Schema-related errors
    #[error("Schema error: {0}")]
    Schema(String),

    /// Generic application errors for unexpected conditions
    #[error("Application error: {0}")]
    App(String),
}

/// Type alias for Result to use TuiqlError as the error type.
///
/// This provides a consistent error type across the entire application
/// instead of using `Result<T, String>` or mixed error types.
pub type Result<T> = std::result::Result<T, TuiqlError>;

/// Type alias for CLI command results that may need to return success/failure status
pub type CommandResult = Result<Option<String>>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let db_err = TuiqlError::Database(rusqlite::Error::ExecuteReturnedResults);
        assert!(db_err.to_string().contains("Database error"));

        let query_err = TuiqlError::Query("Syntax error".to_string());
        assert!(query_err.to_string().contains("Query error"));

        let config_err = TuiqlError::Config("Invalid config".to_string());
        assert!(config_err.to_string().contains("Configuration error"));
    }

    #[test]
    fn test_error_conversion() {
        // Test IO error conversion
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let tuiql_err: TuiqlError = io_err.into();
        match tuiql_err {
            TuiqlError::Io(_) => {}
            _ => panic!("Expected IO error"),
        }

        // Test JSON error conversion
        let json_str = "{ invalid json }";
        let json_err: std::result::Result<serde_json::Value, serde_json::Error> = serde_json::from_str(json_str);
        let tuiql_err: TuiqlError = json_err.unwrap_err().into();
        match tuiql_err {
            TuiqlError::Json(_) => {}
            _ => panic!("Expected JSON error"),
        }
    }
}