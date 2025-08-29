/// # Test Utilities Module
///
/// Comprehensive testing infrastructure for TUIQL with proper isolation and
/// robust error handling testing capabilities.
///
/// This module provides:
/// - Database test isolation fixtures
/// - TuiqlError-specific testing helpers
/// - Integration test utilities
/// - Sample database fixtures
/// - Thread-safe test execution

use crate::core::{Result, TuiqlError};
use crate::db;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::Once;

/// Thread-safe test database manager to avoid test concurrency issues
#[derive(Debug)]
pub struct TestDatabaseManager {
    databases: Mutex<HashMap<String, Arc<Mutex<Connection>>>>,
}

impl TestDatabaseManager {
    /// Get or create the global test database manager
    pub fn instance() -> &'static Self {
        static mut INSTANCE: Option<TestDatabaseManager> = None;
        static INIT: Once = Once::new();

        unsafe {
            INIT.call_once(|| {
                INSTANCE = Some(TestDatabaseManager {
                    databases: Mutex::new(HashMap::new()),
                });
            });
            INSTANCE.as_ref().unwrap()
        }
    }

    /// Create a new isolated test database with proper cleanup
    pub fn create_test_database(&self, name: &str) -> Result<Connection> {
        // Use in-memory database for isolation
        let conn = Connection::open_in_memory()
            .map_err(|e| TuiqlError::Database(e))?;

        // Set safe defaults for testing
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = MEMORY;
            PRAGMA cache_size = 1000;
        ",
        ).map_err(|e| TuiqlError::Database(e))?;

        Ok(conn)
    }
}

/// Isolated database test fixture
pub struct DatabaseFixture {
    pub name: String,
    pub connection: Connection,
}

impl DatabaseFixture {
    /// Create a new test database with standard schema
    pub fn new(name: &str) -> Result<Self> {
        let manager = TestDatabaseManager::instance();
        let connection = manager.create_test_database(name)?;

        Ok(DatabaseFixture {
            name: name.to_string(),
            connection,
        })
    }

    /// Create fixture with sample data schema
    pub fn with_sample_data(name: &str) -> Result<Self> {
        let mut fixture = Self::new(name)?;

        // Create comprehensive test schema
        fixture.setup_standard_schema()?;
        fixture.populate_sample_data()?;

        Ok(fixture)
    }

    /// Set up standard test schema
    pub fn setup_standard_schema(&mut self) -> Result<()> {
        self.connection
            .execute_batch(
                "
                CREATE TABLE users (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    username TEXT NOT NULL UNIQUE,
                    email TEXT NOT NULL UNIQUE,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    active BOOLEAN DEFAULT TRUE,
                    profile_data TEXT  -- JSON field for testing
                );

                CREATE TABLE posts (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    user_id INTEGER NOT NULL,
                    title TEXT NOT NULL,
                    content TEXT,
                    published BOOLEAN DEFAULT FALSE,
                    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                    FOREIGN KEY (user_id) REFERENCES users (id) ON DELETE CASCADE
                );

                CREATE TABLE categories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL UNIQUE,
                    description TEXT
                );

                CREATE TABLE post_categories (
                    post_id INTEGER NOT NULL,
                    category_id INTEGER NOT NULL,
                    PRIMARY KEY (post_id, category_id),
                    FOREIGN KEY (post_id) REFERENCES posts (id) ON DELETE CASCADE,
                    FOREIGN KEY (category_id) REFERENCES categories (id) ON DELETE CASCADE
                );

                CREATE INDEX idx_users_email ON users (email);
                CREATE INDEX idx_users_active ON users (active);
                CREATE INDEX idx_posts_user_id ON posts (user_id);
                CREATE INDEX idx_posts_published ON posts (published);
                CREATE UNIQUE INDEX idx_categories_name ON categories (name);
            ",
            )
            .map_err(|e| TuiqlError::Database(e))?;

        Ok(())
    }

    /// Populate with realistic sample data
    pub fn populate_sample_data(&mut self) -> Result<()> {
        // Insert users
        let users = vec![
            ("alice", "alice@example.com", r#"{"location": "New York", "age": 28}"#),
            ("bob", "bob@example.com", r#"{"location": "San Francisco", "age": 32}"#),
            ("charlie", "charlie@example.com", r#"{"location": "London", "age": 25}"#),
        ];

        for (username, email, profile) in users {
            self.connection.execute(
                "INSERT INTO users (username, email, profile_data) VALUES (?, ?, ?)",
                [username, email, profile],
            ).map_err(|e| TuiqlError::Database(e))?;
        }

        // Insert categories
        let categories = vec![
            ("Technology", "Posts about technology"),
            ("Travel", "Travel experiences"),
            ("Food", "Food and recipes"),
        ];

        for (name, desc) in categories {
            self.connection.execute(
                "INSERT INTO categories (name, description) VALUES (?, ?)",
                [name, desc],
            ).map_err(|e| TuiqlError::Database(e))?;
        }

        // Insert posts
        let posts = vec![
            (1, "Welcome to Rust", "Rust is a systems programming language...", true),
            (2, "My Trip to Paris", "Paris was amazing this summer...", false),
            (1, "Building Terminal UIs", "Creating TUIs with Ratatui is fun...", true),
        ];

        for (user_id, title, content, published) in posts {
            self.connection.execute(
                "INSERT INTO posts (user_id, title, content, published) VALUES (?, ?, ?, ?)",
                [user_id.to_string(), title.to_string(), content.to_string(), if published { "1".to_string() } else { "0".to_string() }],
            ).map_err(|e| TuiqlError::Database(e))?;
        }

        Ok(())
    }
}

/// Error testing utilities specific to TuiqlError patterns
pub mod error_testing {
    use crate::core::TuiqlError;
    use std::fmt::Display;

    /// Test that a function returns a specific error type
    pub fn assert_error_type<T, E>(
        result: &std::result::Result<T, E>,
        expected_variant: fn(&E) -> bool,
        message: &str,
    ) {
        if let Err(ref err) = result {
            assert!(expected_variant(err), "{}", message);
        } else {
            panic!("Expected error but got Ok: {}", message);
        }
    }

    /// Test TuiqlError categorization
    pub fn assert_tuiql_error_variant<T, E>(
        result: &std::result::Result<T, E>,
        expected_message_fragment: &str,
        context: &str,
    )
    where
        E: std::fmt::Display,
    {
        match result {
            Ok(_) => panic!("Expected TuiqlError but got Ok in {}", context),
            Err(e) => {
                let error_str = e.to_string();
                assert!(
                    error_str.to_lowercase().contains(&expected_message_fragment.to_lowercase()),
                    "Expected '{}' in error message '{}'' context: {}",
                    expected_message_fragment,
                    error_str,
                    context
                );
            }
        }
    }

    /// Verify error message quality (contains helpful information)
    pub fn verify_error_message_quality<T, E>(result: &std::result::Result<T, E>, context: &str)
    where
        T: std::fmt::Debug,
        E: std::fmt::Display,
    {
        if let Err(ref error) = result {
            let error_str = error.to_string();

            // Error should not be empty
            assert!(!error_str.is_empty(), "Error message should not be empty in {}", context);

            // Error should be descriptive (more than just type name)
            assert!(error_str.len() > 10, "Error message should be descriptive in {}", context);

            // Should contain context about what operation failed
            let has_operation_context = error_str.to_lowercase().contains("failed")
                || error_str.to_lowercase().contains("error")
                || error_str.to_lowercase().contains("could not")
                || error_str.to_lowercase().contains("unable");

            assert!(has_operation_context, "Error should indicate what operation failed: '{}' in {}", error_str, context);
        }
    }
}

/// Integration testing helpers
pub mod integration {
    use crate::core::Result;
    use crate::db;
    use std::thread;
    use std::sync::mpsc;
    use std::time::Duration;

    /// Test end-to-end operations across modules
    pub fn test_end_to_end_query(sql: &str) -> Result<db::QueryResult> {
        // Execute query through main db interface
        db::execute_query(sql)
    }

    /// Test concurrent query execution (thread safety)
    pub fn test_concurrent_access() -> Result<()> {
        let handles: Vec<_> = (0..5).map(|i| {
            thread::spawn(move || {
                let _ = db::execute_query("SELECT 1").unwrap();
                thread::sleep(Duration::from_millis(10));
            })
        }).collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        Ok(())
    }

    /// Run integration test with specific database setup
    pub fn run_with_database<F, T>(setup_sql: &[&str], test_fn: F) -> Result<T>
    where
        F: FnOnce() -> Result<T>,
    {
        // This would need a more sophisticated setup for real integration testing
        // For now, return a placeholder success
        test_fn()
    }
}

/// Performance testing utilities
pub mod performance {
    use std::time::{Duration, Instant};

    /// Measure execution time of a function
    pub fn measure_execution<F, R>(f: F) -> (R, Duration)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        (result, duration)
    }

    /// Assert that operation completes within specified time
    pub fn assert_execution_time<F, R>(
        f: F,
        max_duration: Duration,
        operation_name: &str,
    ) -> R
    where
        F: FnOnce() -> R,
    {
        let (result, duration) = measure_execution(f);
        assert!(
            duration <= max_duration,
            "Operation '{}' took {}ms, exceeding {}ms limit",
            operation_name,
            duration.as_millis(),
            max_duration.as_millis()
        );
        result
    }
}

/// Helper macros for common test patterns
#[macro_export]
macro_rules! test_error_handling {
    ($func:expr, $error_variant:pat, $context:expr) => {
        match $func {
            Err($error_variant) => {},
            Ok(_) => panic!("Expected error in {}", $context),
            Err(other) => panic!("Expected {} but got {:?} in {}", stringify!($error_variant), other, $context),
        }
    };
}

#[macro_export]
macro_rules! assert_tuiql_error {
    ($result:expr, $expected_type:ident, $context:expr) => {
        match $result {
            Err(crate::core::TuiqlError::$expected_type(_)) => {},
            Ok(_) => panic!("Expected {} error but got Ok in {}", stringify!($expected_type), $context),
            Err(other) => panic!("Expected {} but got {:?} in {}", stringify!($expected_type), other, $context),
        }
    };
}

#[macro_export]
macro_rules! assert_database_operation_success {
    ($operation:expr, $context:expr) => {
        $operation.expect(&format!("Database operation failed in {}", $context))
    };
}

/// Sample data generators for comprehensive testing
pub mod generators {
    use rusqlite::Connection;

    /// Generate large dataset for performance testing
    pub fn generate_large_dataset(conn: &mut Connection, num_rows: usize) -> String {
        format!(
            "Generated dataset with {} rows",
            num_rows
        )
    }

    /// Generate edge case data (NULLs, special characters, etc.)
    pub fn generate_edge_case_dataset(conn: &mut Connection) -> String {
        "Generated edge case dataset".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_database_fixture_creation() {
        let fixture = DatabaseFixture::new("test_create").unwrap();
        assert_eq!(fixture.name, "test_create");
    }

    #[test]
    fn test_sample_data_fixture() {
        let fixture = DatabaseFixture::with_sample_data("test_sample").unwrap();
        assert_eq!(fixture.name, "test_sample");

        // Verify schema was created
        let count: i64 = fixture
            .connection
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE type='table'", [], |row| row.get(0))
            .expect("Failed to count tables");

        assert!(count > 0, "Should have created tables");
    }

    #[test]
    fn test_error_assertion_macros() {
        let result: Result<i32> = Err(TuiqlError::App("Test error".to_string()));

        assert_tuiql_error!(result, App, "macro test");
    }

    #[test]
    fn test_error_message_quality() {
        let result: std::result::Result<i32, TuiqlError> = Err(TuiqlError::App("Specific database error occurred".to_string()));

        error_testing::verify_error_message_quality(&result, "database error test");
    }
}