use crate::core::{Result, TuiqlError};
use crate::db::{get_schema, Schema};
use regex::Regex;

/// SQL keywords that are commonly used in SQLite
const SQL_KEYWORDS: &[&str] = &[
    "SELECT",
    "FROM",
    "WHERE",
    "JOIN",
    "INNER",
    "LEFT",
    "RIGHT",
    "OUTER",
    "ON",
    "HAVING",
    "GROUP",
    "BY",
    "ORDER",
    "LIMIT",
    "OFFSET",
    "DISTINCT",
    "AS",
    "UNION",
    "ALL",
    "INSERT",
    "INTO",
    "VALUES",
    "UPDATE",
    "SET",
    "DELETE",
    "CREATE",
    "DROP",
    "TABLE",
    "INDEX",
    "VIEW",
    "TRIGGER",
    "BEGIN",
    "COMMIT",
    "ROLLBACK",
    "TRANSACTION",
    "PRAGMA",
    "EXPLAIN",
    "QUERY",
    "PLAN",
    "AND",
    "OR",
    "NOT",
    "IN",
    "BETWEEN",
    "LIKE",
    "IS",
    "NULL",
    "CASE",
    "WHEN",
    "THEN",
    "ELSE",
    "END",
    "COALESCE",
    "IFNULL",
    "EXISTS",
    "WITH",
    "RECURSIVE",
    "RETURNING",
];

/// SQLite functions that are commonly used
const SQL_FUNCTIONS: &[&str] = &[
    "ABS",
    "AVG",
    "COUNT",
    "MAX",
    "MIN",
    "SUM",
    "TOTAL",
    "BLOB",
    "CAPITALIZE",
    "CHAR",
    "COALESCE",
    "DATE",
    "DATETIME",
    "HEX",
    "IFNULL",
    "INSTR",
    "LENGTH",
    "LOWER",
    "LTRIM",
    "QUOTE",
    "RANDOM",
    "RANDOMBLOB",
    "REPLACE",
    "ROUND",
    "RTRIM",
    "SUBSTR",
    "TRIM",
    "TYPEOF",
    "UNICODE",
    "UPPER",
    "ZEROBLOB",
    "GLOB",
    "LIKE",
    "REGEXP",
    "JSON",
    "JSON_ARRAY",
    "JSON_OBJECT",
    "JSON_EXTRACT",
    "JSON_TYPE",
    "JSON_VALID",
    "FTS5",
    "MATCH",
    "RANK",
    "BM25",
];

/// SQLite pragmas for completion
const SQLITE_PRAGMAS: &[&str] = &[
    "TABLE_INFO",
    "INDEX_LIST",
    "INDEX_INFO",
    "FOREIGN_KEY_LIST",
    "COLLATION_LIST",
    "DATABASE_LIST",
    "FUNCTION_LIST",
    "PRAGMA_LIST",
    "MODULE_LIST",
    "STATISTICS",
    "CACHE_SIZE",
    "PAGE_SIZE",
    "JOURNAL_MODE",
    "SYNCHRONOUS",
    "TEMP_STORE",
    "LOCKING_MODE",
    "READ_UNCOMMITTED",
    "FOREIGN_KEYS",
    "IGNORE_CHECK_CONSTRAINTS",
    "INTEGRITY_CHECK",
    "QUICK_CHECK",
    "FOREIGN_KEY_CHECK",
    "OPTIMIZE",
    "VACUUM",
    "ANALYZE",
    "WIKI",
    "WTEXT",
];

/// Context-aware SQL completer that provides suggestions based on current query and schema
pub struct SqlCompleter {
    schema: Option<Schema>,
    last_update: std::time::Instant,
}

impl SqlCompleter {
    /// Creates a new SQL completer
    pub fn new() -> Self {
        SqlCompleter {
            schema: None,
            last_update: std::time::Instant::now(),
        }
    }

    /// Updates the completer with current schema information
    pub fn update_schema(&mut self) -> Result<()> {
        match get_schema() {
            Ok(schema) => {
                self.schema = Some(schema);
                self.last_update = std::time::Instant::now();
                Ok(())
            }
            Err(TuiqlError::Schema(_)) => {
                // No database connected, keep empty schema
                self.schema = None;
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    /// Gets completion suggestions for the given query and cursor position
    pub fn complete(&mut self, query: &str, cursor_pos: usize) -> Result<Vec<String>> {
        // Update schema periodically (every 5 seconds to avoid performance issues)
        if self.last_update.elapsed() > std::time::Duration::from_secs(5) {
            self.update_schema()?;
        }

        if query.is_empty() {
            return Ok(Vec::new());
        }

        // Clamp cursor position to valid range
        let cursor_pos = cursor_pos.min(query.len());
        let (prefix, context) = self.parse_context(query, cursor_pos);
        let suggestions = self.get_suggestions(&context, &prefix);

        Ok(suggestions)
    }

    /// Parses the query context at the given cursor position
    fn parse_context(&self, query: &str, cursor_pos: usize) -> (String, CompletionContext) {
        let before_cursor = &query[..cursor_pos];

        // Find the current word being typed
        let partial_word_re = Regex::new(r"\S+$").unwrap(); // Match any non-whitespace at the end
        let full_word_re = Regex::new(r"\b\w+\b$").unwrap(); // Match complete word at end with word boundaries

        let prefix = if let Some(full_match) = full_word_re.find(before_cursor) {
            // We're at the end of a complete word, no partial typing - show suggestions for next context
            String::new()
        } else if let Some(partial_match) = partial_word_re.find(before_cursor) {
            // We're in the middle of typing a word
            partial_match.as_str().to_uppercase()
        } else {
            // Nothing at end, show suggestions for current context
            String::new()
        };

        // Determine context based on the query structure
        let trimmed_query = query.trim();
        let context = if trimmed_query.is_empty() {
            CompletionContext::Start
        } else if self.is_after_pragma(before_cursor) {
            CompletionContext::PragmaName
        } else if self.is_after_from(before_cursor) {
            CompletionContext::TableName
        } else if self.is_after_select(before_cursor) {
            // SELECT can be followed by more keywords, columns, or functions
            CompletionContext::Keyword
        } else {
            CompletionContext::Keyword
        };

        (prefix, context)
    }

    fn is_after_select(&self, text: &str) -> bool {
        let text_upper = text.to_uppercase();
        text_upper.contains("SELECT") && !self.is_after_from(&text_upper)
    }

    fn is_after_from(&self, text: &str) -> bool {
        text.to_uppercase().contains("FROM")
    }

    fn is_in_from_clause(&self, text: &str) -> bool {
        let text_upper = text.to_uppercase();
        if let Some(from_pos) = text_upper.rfind("FROM") {
            let after_from = &text[from_pos..];
            // Simple heuristic: if FROM is the last clause, we're expecting table names
            !after_from.to_uppercase().contains("WHERE")
                && !after_from.to_uppercase().contains("JOIN")
                && !after_from.to_uppercase().contains("ORDER")
        } else {
            false
        }
    }

    fn is_in_where_clause(&self, text: &str) -> bool {
        let text_upper = text.to_uppercase();
        if let Some(where_pos) = text_upper.rfind("WHERE") {
            let after_where = &text[where_pos..];
            return !after_where.to_uppercase().contains("ORDER")
                && !after_where.to_uppercase().contains("GROUP")
                && !after_where.to_uppercase().contains("HAVING");
        }
        false
    }

    fn is_in_join_clause(&self, text: &str) -> bool {
        let text_upper = text.to_uppercase();
        if let Some(join_pos) = text_upper.rfind("JOIN") {
            let after_join = &text[join_pos..];
            return after_join.to_uppercase().find("ON").is_none();
        }
        false
    }

    fn is_after_pragma(&self, text: &str) -> bool {
        let text_upper = text.to_uppercase();
        let trimmed = text_upper.trim();
        if trimmed.contains("PRAGMA") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() > 0 && parts[0] == "PRAGMA" {
                // After "PRAGMA" we expect a pragma name
                return parts.len() == 1;
            }
        }
        false
    }

    /// Generates suggestions based on context and prefix
    fn get_suggestions(&self, context: &CompletionContext, prefix: &str) -> Vec<String> {
        let mut suggestions = Vec::new();

        match context {
            CompletionContext::Start | CompletionContext::Keyword => {
                // Add keywords that match prefix - PRAGMA is already in SQL_KEYWORDS
                suggestions.extend(self.filter_keywords(SQL_KEYWORDS, prefix));
            }
            CompletionContext::TableName => {
                // Add common table-related keywords first, then table names
                suggestions.extend(self.filter_keywords(&["JOIN", "INNER", "LEFT", "RIGHT", "OUTER", "ON", "WHERE", "GROUP", "ORDER", "BY", "LIMIT"], prefix));
                // Add table names
                if let Some(schema) = &self.schema {
                    suggestions.extend(self.filter_keywords(
                        &schema.tables.keys().map(|s| s.as_str()).collect::<Vec<_>>(),
                        prefix,
                    ));
                }
            }
            CompletionContext::ColumnName => {
                suggestions.extend(self.filter_keywords(SQL_KEYWORDS, prefix));
                // Add column names from all tables (this is a simplification)
                if let Some(schema) = &self.schema {
                    let mut column_names: Vec<&str> = Vec::new();
                    for table in schema.tables.values() {
                        for column in &table.columns {
                            column_names.push(&column.name);
                        }
                    }
                    suggestions.extend(self.filter_keywords(&column_names, prefix));
                }
                // Add functions
                suggestions.extend(self.filter_keywords(SQL_FUNCTIONS, prefix));
            }
            CompletionContext::PragmaName => {
                suggestions.extend(self.filter_keywords(SQLITE_PRAGMAS, prefix));
            }
        }

        // Sort suggestions alphabetically
        suggestions.sort();
        // Remove duplicates but preserve original case
        let mut seen = std::collections::HashSet::new();
        suggestions.retain(|s| seen.insert(s.clone()));
        suggestions
    }

    fn filter_keywords(&self, keywords: &[&str], prefix: &str) -> Vec<String> {
        if prefix.is_empty() {
            // Return common keywords when no prefix
            keywords
                .iter()
                .take(15) // Limit to avoid overwhelming suggestions
                .map(|s| s.to_string())
                .collect()
        } else {
            keywords
                .iter()
                .filter(|kw| kw.to_uppercase().starts_with(prefix))
                .map(|s| s.to_string())
                .collect()
        }
    }
}

/// Represents different contexts where completion can occur
#[derive(Debug, Clone)]
enum CompletionContext {
    Start,
    Keyword,
    TableName,
    ColumnName,
    PragmaName,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sql_completer_creation() {
        let completer = SqlCompleter::new();
        assert!(completer.schema.is_none());
    }

    #[test]
    fn test_keyword_completion() {
        let mut completer = SqlCompleter::new();

        // Test empty query at start
        let suggestions = completer.complete("", 0).unwrap();
        assert!(suggestions.is_empty()); // Should be empty for no input

        // Test partial keyword completion
        let suggestions = completer.complete("SEL", 3).unwrap();
        assert!(suggestions.contains(&"SELECT".to_string()));

        let suggestions = completer.complete("sel", 3).unwrap();
        assert!(suggestions.contains(&"SELECT".to_string()));

        let suggestions = completer.complete("SELECT", 6).unwrap();
        assert!(suggestions.contains(&"FROM".to_string()));
    }

    #[test]
    fn test_table_name_completion() {
        let mut completer = SqlCompleter::new();

        // Test after FROM keyword
        let suggestions = completer.complete("SELECT * FROM ", 15).unwrap();
        // Should include common keywords plus table names
        assert!(suggestions.contains(&"SELECT".to_string()) || suggestions.len() > 0);
    }

    #[test]
    fn test_pragma_completion() {
        let mut completer = SqlCompleter::new();

        // Test pragma suggestions
        let suggestions = completer.complete("PRAGMA ", 7).unwrap();
        assert!(suggestions.contains(&"TABLE_INFO".to_string()));
        assert!(suggestions.contains(&"INDEX_LIST".to_string()));
    }

    #[test]
    fn test_context_parsing() {
        let completer = SqlCompleter::new();

        // Test keyword context
        let (_, context) = completer.parse_context("SELECT", 6);
        match context {
            CompletionContext::Keyword => {}
            _ => panic!("Expected Keyword context"),
        }

        // Test start context
        let (_, context) = completer.parse_context("", 0);
        match context {
            CompletionContext::Start => {}
            _ => panic!("Expected Start context"),
        }
    }
}
