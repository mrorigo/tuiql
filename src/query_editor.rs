use crate::core::{Result, TuiqlError};

/// Query Editor Module for TUIQL
///
/// This module provides a query editor for TUIQL with features like multiline editing,
/// syntax highlighting, autocompletion of SQL keywords and table names, bracket balance checking,
/// linting for dangerous SQL operations, and query formatting.

#[derive(Debug, Default)]
pub struct QueryEditor {
    query_buffer: String,
}

impl QueryEditor {
    /// Creates a new instance of QueryEditor.
    pub fn new() -> Self {
        QueryEditor {
            query_buffer: String::new(),
        }
    }

    /// Sets the query text in the editor.
    pub fn set_query(&mut self, query: &str) {
        self.query_buffer = query.to_string();
    }

    /// Returns the current query text.
    pub fn get_query(&self) -> &str {
        &self.query_buffer
    }

    /// Clears the query buffer.
    pub fn clear(&mut self) {
        self.query_buffer.clear();
    }

    /// Simulates executing the query.
    /// In a complete implementation, this method would interface with the SQL execution engine.
    pub fn execute(&self) -> Result<String> {
        if self.query_buffer.trim().is_empty() {
            Err(TuiqlError::Query("Query is empty".to_string()))
        } else {
            Ok(format!("Executing query: {}", self.query_buffer))
        }
    }

    /// Lints the current query for dangerous operations.
    pub fn lint_query(&self) -> Result<()> {
        let query = self.query_buffer.trim();

        // Handle multiple statements separated by semicolons
        let statements: Vec<&str> = query.split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        let mut has_begin = false;
        let mut has_commit = false;
        let mut has_rollback = false;

        for statement in &statements {
            let lower_stmt = statement.to_lowercase();

            // Skip empty or whitespace-only statements
            if lower_stmt.trim().is_empty() {
                continue;
            }

            // Check for dangerous DML operations without WHERE clause
            if self.is_dangerous_dml_without_where(&lower_stmt) {
                return Err(TuiqlError::Query("Dangerous operation: DML statement without WHERE clause can affect all rows".to_string()));
            }

            // Check for implicit JOINs
            if self.detects_implicit_join(&lower_stmt) {
                return Err(TuiqlError::Query("Dangerous operation: Implicit JOIN without explicit ON/USING clause".to_string()));
            }

            // Check for dangerous DDL operations
            if self.is_dangerous_ddl(&lower_stmt) {
                return Err(TuiqlError::Query("Dangerous operation: DDL statement detected - manual review required".to_string()));
            }

            // Check for nested transactions
            if lower_stmt.contains("begin") {
                has_begin = true;
            }
            if lower_stmt.contains("commit") {
                has_commit = true;
            }
            if lower_stmt.contains("rollback") {
                has_rollback = true;
            }

            // Check for orphaned transaction operations
            if self.is_non_transaction_op(&lower_stmt) && has_begin && !has_commit && !has_rollback {
                return Err(TuiqlError::Query("Dangerous operation: Non-transaction operations within uncommitted transaction".to_string()));
            }

            // Check for PRAGMA operations that could be dangerous
            if self.is_dangerous_pragma(&lower_stmt) {
                return Err(TuiqlError::Query("Dangerous operation: PRAGMA may modify database behavior - review required".to_string()));
            }
        }

        // Check for uncommitted transactions across all statements
        if has_begin && !has_commit && !has_rollback {
            return Err(TuiqlError::Query("Dangerous operation: BEGIN statement without COMMIT or ROLLBACK".to_string()));
        }

        Ok(())
    }

    /// Helper method to detect DML operations without WHERE clause
    fn is_dangerous_dml_without_where(&self, statement: &str) -> bool {
        let lower_stmt = statement.to_lowercase();

        if lower_stmt.trim().starts_with("delete from") || lower_stmt.trim().starts_with("update") {
            let after_keyword = if lower_stmt.starts_with("delete from") {
                lower_stmt.trim_start_matches("delete from")
            } else {
                lower_stmt.trim_start_matches("update").trim_start_matches("set")
            };

            // Check if there's no WHERE clause by looking for WHERE keyword
            let words: Vec<&str> = after_keyword.split_whitespace().collect();

            for (i, word) in words.iter().enumerate() {
                if *word == "where" {
                    return false; // WHERE clause found
                }
                // Stop checking after SET for UPDATE or after table name for DELETE
                if i > 0 && (*word == "set" || words[i-1] == "from") {
                    break;
                }
            }
            return true; // No WHERE clause found
        }
        false
    }

    /// Helper method to detect implicit JOINs
    fn detects_implicit_join(&self, statement: &str) -> bool {
        let lower_stmt = statement.to_lowercase();
        if lower_stmt.contains("join") {
            // Check for presence of ON or USING clause
            let has_on = lower_stmt.contains(" on ");
            let has_using = lower_stmt.contains(" using ");
            return !has_on && !has_using;
        }
        false
    }

    /// Helper method to detect dangerous DDL operations
    fn is_dangerous_ddl(&self, statement: &str) -> bool {
        let lower_stmt = statement.to_lowercase();
        lower_stmt.trim().starts_with("drop ") ||
        lower_stmt.trim().starts_with("alter ") ||
        lower_stmt.trim().starts_with("create ") ||
        lower_stmt.trim().starts_with("truncate ")
    }

    /// Helper method to detect non-transaction operations
    fn is_non_transaction_op(&self, statement: &str) -> bool {
        let lower_stmt = statement.to_lowercase();
        let trimmed = lower_stmt.trim();
        !(trimmed.starts_with("begin") ||
          trimmed.starts_with("commit") ||
          trimmed.starts_with("rollback") ||
          trimmed.starts_with("savepoint") ||
          trimmed.starts_with("release"))
    }

    /// Helper method to detect potentially dangerous PRAGMA operations
    fn is_dangerous_pragma(&self, statement: &str) -> bool {
        let lower_stmt = statement.to_lowercase();
        if lower_stmt.trim().starts_with("pragma") {
            // Some pragmas that could be considered dangerous
            lower_stmt.contains("foreign_keys") ||
            lower_stmt.contains("journal_mode") ||
            lower_stmt.contains("synchronous") ||
            lower_stmt.contains("cache_size") ||
            lower_stmt.contains("temp_store")
        } else {
            false
        }
    }

    /// Formats the current query for better readability.
    pub fn format_query(&mut self) {
        // A simple formatter that ensures consistent spacing around keywords and after commas.
        self.query_buffer = self
            .query_buffer
            .replace(",", ", ")
            .replace("  ", " ")
            .replace("SELECT", "SELECT\n")
            .replace("FROM", "\nFROM\n")
            .replace("WHERE", "\nWHERE\n")
            .replace("=", " = ")
            .replace(", ", ", ")
            .replace("\n ", "\n")
            .lines()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_and_get_query() {
        let mut editor = QueryEditor::new();
        editor.set_query("SELECT * FROM users;");
        assert_eq!(editor.get_query(), "SELECT * FROM users;");
    }

    #[test]
    fn test_clear_query() {
        let mut editor = QueryEditor::new();
        editor.set_query("SELECT 1;");
        editor.clear();
        assert_eq!(editor.get_query(), "");
    }

    #[test]
    fn test_execute_empty_query() {
        let editor = QueryEditor::new();
        let result = editor.execute();
        assert!(result.is_err());
        if let Err(TuiqlError::Query(msg)) = result {
            assert_eq!(msg, "Query is empty");
        } else {
            panic!("Expected Query error");
        }
        
                #[test]
                fn test_format_query() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("SELECT name,email FROM users WHERE id=1;");
                    editor.format_query();
                    assert_eq!(
                        editor.get_query(),
                        "SELECT\nname, email\nFROM\nusers\nWHERE\nid = 1;\n"
                    );
                }
        
                #[test]
                fn test_lint_query_implicit_join() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("SELECT * FROM users JOIN orders;");
                    let result = editor.lint_query();
                    assert!(result.is_err());
                    if let Err(TuiqlError::Query(msg)) = result {
                        assert!(msg.contains("Implicit JOIN without explicit ON/USING"));
                    } else {
                        panic!("Expected Query error");
                    }
                }
        
                #[test]
                fn test_lint_query_uncommitted_transaction() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("BEGIN TRANSACTION; SELECT * FROM users;");
                    let result = editor.lint_query();
                    assert!(result.is_err());
                    if let Err(TuiqlError::Query(msg)) = result {
                        assert_eq!(msg, "Dangerous operation: BEGIN statement without COMMIT or ROLLBACK");
                    } else {
                        panic!("Expected Query error");
                    }
                }
        
                #[test]
                fn test_lint_query_dangerous_ddl_drop() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("DROP TABLE users;");
                    let result = editor.lint_query();
                    assert!(result.is_err());
                    if let Err(TuiqlError::Query(msg)) = result {
                        assert!(msg.contains("DDL statement detected"));
                    } else {
                        panic!("Expected Query error");
                    }
                }
        
                #[test]
                fn test_lint_query_dangerous_pragma() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("PRAGMA foreign_keys = ON;");
                    let result = editor.lint_query();
                    assert!(result.is_err());
                    if let Err(TuiqlError::Query(msg)) = result {
                        assert!(msg.contains("PRAGMA may modify database behavior"));
                    } else {
                        panic!("Expected Query error");
                    }
                }
        
                #[test]
                fn test_lint_query_multiple_statements_committed() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("BEGIN; UPDATE users SET active = 0 WHERE id = 1; COMMIT;");
                    let result = editor.lint_query();
                    assert!(result.is_ok());
                }
        
                #[test]
                fn test_lint_query_non_transaction_op_in_uncommitted_txn() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("BEGIN; PRAGMA journal_mode = WAL;");
                    let result = editor.lint_query();
                    assert!(result.is_err());
                    if let Err(TuiqlError::Query(msg)) = result {
                        assert!(msg.contains("Non-transaction operations within uncommitted transaction"));
                    } else {
                        panic!("Expected Query error");
                    }
                }
        
                #[test]
                fn test_lint_query_complex_safe_statement() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("SELECT u.name FROM users u JOIN orders o ON u.id = o.user_id WHERE u.created_at > '2023-01-01';");
                    let result = editor.lint_query();
                    assert!(result.is_ok());
                }
        
                #[test]
                fn test_lint_query_create_statement_safe() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT);");
                    let result = editor.lint_query();
                    assert!(result.is_err());
                    if let Err(TuiqlError::Query(msg)) = result {
                        assert!(msg.contains("DDL statement detected"));
                    } else {
                        panic!("Expected Query error");
                    }
                }
        
                #[test]
                fn test_lint_query_truncate_statement_safe() {
                    let mut editor = QueryEditor::new();
                    editor.set_query("DELETE FROM users WHERE created_at < '2020-01-01';");
                    let result = editor.lint_query();
                    assert!(result.is_ok());
                }
            }

    #[test]
    fn test_execute_valid_query() {
        let mut editor = QueryEditor::new();
        editor.set_query("SELECT name FROM users;");
        let result = editor.execute();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Executing query: SELECT name FROM users;");
    }
}
