// Query Editor Module for TUIQL
//
// This module provides a query editor for TUIQL with features like multiline editing,
// syntax highlighting, autocompletion of SQL keywords and table names, bracket balance checking,
// linting for dangerous SQL operations, and query formatting.

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
    pub fn execute(&self) -> Result<String, String> {
        if self.query_buffer.trim().is_empty() {
            Err("Query is empty".to_string())
        } else {
            Ok(format!("Executing query: {}", self.query_buffer))
        }
    }

    /// Lints the current query for dangerous operations.
    pub fn lint_query(&self) -> Result<(), String> {
        let query = self.query_buffer.trim().to_lowercase();
        if query.starts_with("delete") && !query.contains("where") {
            Err("Dangerous operation: DELETE without WHERE clause".to_string())
        } else if query.starts_with("update") && !query.contains("where") {
            Err("Dangerous operation: UPDATE without WHERE clause".to_string())
        } else {
            Ok(())
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
        assert_eq!(result.unwrap_err(), "Query is empty");
    }

    #[test]
    fn test_execute_valid_query() {
        let mut editor = QueryEditor::new();
        editor.set_query("SELECT name FROM users;");
        let result = editor.execute();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Executing query: SELECT name FROM users;");
    }
    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn test_lint_query_safe() {
            let mut editor = QueryEditor::new();
            editor.set_query("SELECT * FROM users;");
            let result = editor.lint_query();
            assert!(result.is_ok());
        }

        #[test]
        fn test_lint_query_dangerous_delete() {
            let mut editor = QueryEditor::new();
            editor.set_query("DELETE FROM users;");
            let result = editor.lint_query();
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                "Dangerous operation: DELETE without WHERE clause"
            );
        }

        #[test]
        fn test_lint_query_dangerous_update() {
            let mut editor = QueryEditor::new();
            editor.set_query("UPDATE users SET name = 'John';");
            let result = editor.lint_query();
            assert!(result.is_err());
            assert_eq!(
                result.unwrap_err(),
                "Dangerous operation: UPDATE without WHERE clause"
            );
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
    }
}
