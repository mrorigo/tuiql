/*
 * JSON1 Helper Module
 *
 * This module provides comprehensive helpers for SQLite's JSON1 extension,
 * enabling easier JSON querying, flattening, and analysis within TUIQL.
 *
 * Features:
 * - JSON path navigation and extraction helpers
 * - json_each/json_tree table generator SQL builders
 * - JSON validation and formatting utilities
 * - Query builder for common JSON patterns
 * - Integration with existing database workflow
 */

use crate::core::{Result, TuiqlError};
use crate::db;
use std::collections::HashMap;

/// JSON query configuration
#[derive(Debug, Clone)]
pub struct Json1Config {
    pub table_name: String,
    pub json_column: String,
    pub path_expressions: Vec<String>,
}

/// JSON path extraction result
#[derive(Debug, Clone)]
pub struct JsonPathResult {
    pub path: String,
    pub value_type: String,
    pub value: String,
    pub full_path: String,
}

/// JSON validation result
#[derive(Debug, Clone)]
pub struct JsonValidationResult {
    pub is_valid: bool,
    pub error_message: Option<String>,
    pub json_type: Option<String>,
}

/// Generates SQL for json_each table function
///
/// # Arguments
///
/// * `json_expr` - JSON expression or column name
/// * `path` - JSONPath to extract (optional, defaults to '$')
///
/// # Returns
///
/// SQL query string for json_each usage
///
pub fn create_json_each_query(json_expr: &str, path: Option<&str>) -> String {
    let json_path = path.unwrap_or("$");
    format!(
        "SELECT key, value, type, atom, json(json_type(value)) as value_type
         FROM json_each({}, '{}')",
        json_expr, json_path
    )
}

/// Generates SQL for json_tree table function
///
/// # Arguments
///
/// * `json_expr` - JSON expression or column name
/// * `path` - JSONPath to extract (optional)
/// * `max_depth` - Maximum depth to traverse (optional, defaults to 10)
///
/// # Returns
///
/// SQL query string for json_tree usage
///
pub fn create_json_tree_query(json_expr: &str, path: Option<&str>, max_depth: Option<usize>) -> String {
    let json_path = path.unwrap_or("$");
    let depth_limit = max_depth.unwrap_or(10);
    format!(
        "SELECT key, value, type, atom, json_type(value) as value_type, fullkey, path
         FROM json_tree({}, '{}')
         WHERE json_type(value) IS NOT NULL
         ORDER BY fullkey
         LIMIT {}",
        json_expr, json_path, depth_limit
    )
}

/// Flattens JSON array or object into tabular format
///
/// # Arguments
///
/// * `json_expr` - JSON expression to flatten
/// * `output_columns` - Column names to extract from each JSON item
///
/// # Returns
///
/// SQL query string for flattening JSON
///
pub fn create_json_flatten_query(json_expr: &str, output_columns: &Vec<String>) -> Result<String> {
    if output_columns.is_empty() {
        return Err(TuiqlError::Query(
            "Flattening requires at least one output column specification".to_string(),
        ));
    }

    let select_columns: Vec<String> = output_columns
        .iter()
        .enumerate()
        .map(|(_i, col)| format!("json_extract(value, '$.{}') as {}", col, col))
        .collect();

    Ok(format!(
        "SELECT {}

         FROM json_each({})",
        select_columns.join(", "),
        json_expr
    ))
}

/// Creates JSON extraction patterns for common use cases
///
/// # Arguments
///
/// * `base_table` - Name of table containing JSON column
/// * `json_column` - Name of JSON column
/// * `extraction_patterns` - Map of output names to JSON paths
///
/// # Returns
///
/// SQL query string with multiple JSON extractions
///
pub fn create_json_extract_query(
    base_table: &str,
    json_column: &str,
    extraction_patterns: &HashMap<String, String>,
) -> Result<String> {
    if extraction_patterns.is_empty() {
        return Err(TuiqlError::Query(
            "At least one extraction pattern required".to_string(),
        ));
    }

    let select_extractions: Vec<String> = extraction_patterns
        .iter()
        .map(|(output_name, json_path)| {
            format!("json_extract({}, '{}') as {}", json_column, json_path, output_name)
        })
        .collect();

    Ok(format!(
        "SELECT {}, {}

         FROM {}",
        "rowid, *".to_string(),
        select_extractions.join(", "),
        base_table
    ))
}

/// Validates JSON content and provides type information
///
/// # Arguments
///
/// * `json_text` - JSON string to validate
///
/// # Returns
///
/// Validation result with type information
///
pub fn validate_json(json_text: &str) -> JsonValidationResult {
    match serde_json::from_str::<serde_json::Value>(json_text) {
        Ok(value) => {
            let json_type = match &value {
                serde_json::Value::Object(_) => "object",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Null => "null",
            }.to_string();

            JsonValidationResult {
                is_valid: true,
                error_message: None,
                json_type: Some(json_type),
            }
        }
        Err(e) => JsonValidationResult {
            is_valid: false,
            error_message: Some(format!("JSON parsing error: {}", e)),
            json_type: None,
        }
    }
}

/// Analyzes JSON structure using json_tree
///
/// # Arguments
///
/// * `json_expr` - JSON expression to analyze
/// * `depth_limit` - Maximum depth to analyze
///
/// # Returns
///
/// Vector of path analysis results
///
pub fn analyze_json_structure(json_expr: &str, depth_limit: usize) -> Result<Vec<JsonPathResult>> {
    let state_cell = db::DB_STATE.get().ok_or(TuiqlError::App("No database connection found".to_string()))?;
    let state_guard = state_cell
        .lock()
        .map_err(|_| TuiqlError::App("Failed to acquire database lock".to_string()))?;
    let conn = state_guard.connection.as_ref().ok_or(TuiqlError::App("No active connection".to_string()))?;

    let query = format!(
        "SELECT fullkey, json_type(value) as type, value, path
         FROM json_tree({})
         WHERE json_type(value) IS NOT NULL
         AND length(fullkey) - length(replace(fullkey, '.', '')) <= {}
         ORDER BY fullkey",
        json_expr, depth_limit
    );

    let mut stmt = conn.prepare(&query)?;
    let mut results = Vec::new();

    let rows = stmt.query_map([], |row| {
        Ok(JsonPathResult {
            path: row.get::<_, String>(0)?,
            value_type: row.get::<_, String>(1)?,
            value: row.get::<_, String>(2)?,
            full_path: row.get::<_, String>(3)?,
        })
    })?;

    for result in rows {
        results.push(result?);
    }

    Ok(results)
}

/// Generates helpful JSON1 usage examples
///
/// # Returns
///
/// Formatted help text with JSON1 examples
///
pub fn json1_help() -> String {
    format!(
        "🎯 SQLite JSON1 Extension Helper\n\n\
          JSON1 provides powerful JSON querying capabilities within SQLite.\n\
          This helper builds SQL queries and provides common patterns.\n\n\
          📝 USAGE PATTERNS:\n\
          • Extract values: json_extract(data, '$.users[0].name')\n\
          • Navigate arrays: json_each(data, '$.items')\n\
          • Query objects: json_tree(data, '$.user') \n\
          • Validate JSON: json_valid(column)\n\
          • Pretty print: json_pretty(data)\n\n\
          🛠️ COMMON QUERIES:\n\
          • List array elements: SELECT key, value FROM json_each(data)\n\
          • Extract nested values: SELECT json_extract(data, '$.parent.child') FROM table\n\
          • Filter by JSON: WHERE json_extract(data, '$.active') = 'true'\n\
          • Tree exploration: SELECT fullkey, json_type(value) FROM json_tree(data)\n\n\
          💡 JSON FUNCTIONS:\n\
          • json_extract(obj, path) - Extract value at path\n\
          • json_each(json[, path]) - Table of array/object elements\n\
          • json_tree(json[, path]) - Recursive tree of all values\n\
          • json_valid(json) - Check if valid JSON\n\
          • json_type(json[, path]) - Get JSON type\n\
          • json_pretty(json) - Pretty-printed JSON\n\n\
          💡 QUERY BUILDER:\n\
          Use :json1 each <table.column> [path] for array/object iteration\n\
          Use :json1 tree <table.column> [path] for tree exploration\n\
          Use :json1 flatten <column> <columns...> for tabular formatting\n\
          Use :json1 validate <expression> for JSON validation"
    )
}

/// Executes JSON analysis and displays results in the REPL
///
/// # Arguments
///
/// * `command` - Complete json1 command (e.g., "each data $.users", "tree data", etc.)
///
/// # Returns
///
/// Result indicating success
///
pub fn execute_json1_command(command: &str) -> Result<()> {
    let parts: Vec<&str> = command.split_whitespace().collect();

    if parts.is_empty() {
        println!("{}", json1_help());
        return Ok(());
    }

    match parts[0] {
        "help" => {
            println!("{}", json1_help());
        }
        "each" => {
            if parts.len() < 2 {
                println!("❌ Usage: :json1 each <json_expr> [path]");
                println!("Example: :json1 each data $.users");
                return Ok(());
            }

            let json_expr = parts[1];
            let path = parts.get(2).map(|s| s.to_string());
            let query = if path.is_some() {
                create_json_each_query(json_expr, path.as_deref())
            } else {
                create_json_each_query(json_expr, None)
            };

            println!("📊 JSON Each Query Generated:");
            println!("SQL: {}", query);
            println!("\n📝 Execute this query to see the results:");
            println!("  {}", query);
        }
        "tree" => {
            if parts.len() < 2 {
                println!("❌ Usage: :json1 tree <json_expr> [path] [max_depth]");
                println!("Example: :json1 tree data $.user 3");
                return Ok(());
            }

            let json_expr = parts[1];
            let path = parts.get(2).map(|s| s.to_string());
            let max_depth = parts.get(3).and_then(|s| s.parse().ok());

            let query = create_json_tree_query(json_expr, path.as_deref(), max_depth);

            println!("🌳 JSON Tree Query Generated:");
            println!("SQL: {}", query);
            println!("\n📝 Execute this query to see the results:");
            println!("  {}", query);
        }
        "flatten" => {
            if parts.len() < 3 {
                println!("❌ Usage: :json1 flatten <json_expr> <col1> [col2] ...");
                println!("Example: :json1 flatten items name,price,active");
                return Ok(());
            }

            let json_expr = parts[1];
            let output_columns: Vec<String> = parts[2..]
                .iter()
                .flat_map(|col| col.split(','))
                .map(|s| s.trim().to_string())
                .collect();

            match create_json_flatten_query(json_expr, &output_columns) {
                Ok(query) => {
                    println!("📋 JSON Flatten Query Generated:");
                    println!("SQL: {}", query);
                    println!("\n📝 Execute this query to see the results:");
                    println!("  {}", query);
                }
                Err(e) => println!("❌ Error: {}", e),
            }
        }
        "analyze" => {
            if parts.len() < 2 {
                println!("❌ Usage: :json1 analyze <json_expr> [depth]");
                println!("Example: :json1 analyze '<some_json_column>'");
                return Ok(());
            }

            let json_expr = parts[1];
            let depth = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(5);

            match analyze_json_structure(json_expr, depth) {
                Ok(results) => {
                    if results.is_empty() {
                        println!("📊 No JSON structure found or invalid JSON");
                    } else {
                        println!("📊 JSON Structure Analysis:");
                        println!("{:<30} {:<10} {}", "PATH", "TYPE", "VALUE");
                        println!("{:-<30} {:-<10} {:-<50}", "", "", "");

                        for result in results.iter().take(20) { // Limit for readability
                            let value_preview = if result.value.len() > 40 {
                                format!("{}...", &result.value[..40])
                            } else {
                                result.value.clone()
                            };
                            println!("{:<30} {:<10} {}", result.path, result.value_type, value_preview);
                        }

                        if results.len() > 20 {
                            println!("... and {} more paths", results.len() - 20);
                        }
                    }
                }
                Err(e) => println!("❌ Analysis failed: {}", e),
            }
        }
        "validate" => {
            if parts.len() < 2 {
                println!("❌ Usage: :json1 validate <json_text>");
                return Ok(());
            }

            let json_text = parts[1..].join(" ");
            let result = validate_json(&json_text);

            if result.is_valid {
                println!("✅ Valid JSON - Type: {}", result.json_type.unwrap_or("unknown".to_string()));
            } else {
                println!("❌ Invalid JSON: {}", result.error_message.unwrap_or("Unknown error".to_string()));
            }
        }
        _ => {
            println!("❓ Unknown command: '{}'", command);
            println!("Available commands: help, each, tree, flatten, analyze, validate");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json1_help_format() {
        let help = json1_help();
        assert!(help.contains("JSON1"));
        assert!(help.contains("json_extract"));
        assert!(help.contains("USAGE PATTERNS"));
        assert!(help.contains("json_each"));
    }

    #[test]
    fn test_create_json_each_query() {
        let query = create_json_each_query("data", Some("$.users"));
        assert!(query.contains("json_each(data, '$.users')"));
        assert!(query.contains("key, value, type"));
    }

    #[test]
    fn test_create_json_each_query_no_path() {
        let query = create_json_each_query("data", None);
        assert!(query.contains("json_each(data, '$')"));
    }

    #[test]
    fn test_json_flatten_requires_columns() {
        let columns = vec![];
        let result = create_json_flatten_query("data", &columns);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_extract_requires_patterns() {
        let patterns = HashMap::new();
        let result = create_json_extract_query("users", "data", &patterns);
        assert!(result.is_err());
    }

    #[test]
    fn test_json_validation_valid_object() {
        let result = validate_json(r#"{"name": "test", "active": true}"#);
        assert!(result.is_valid);
        assert_eq!(result.json_type, Some("object".to_string()));
    }

    #[test]
    fn test_json_validation_valid_array() {
        let result = validate_json(r#"["item1", "item2"]"#);
        assert!(result.is_valid);
        assert_eq!(result.json_type, Some("array".to_string()));
    }

    #[test]
    fn test_json_validation_invalid() {
        let result = validate_json(r#"{"invalid": json"#);
        assert!(!result.is_valid);
        assert!(result.error_message.is_some());
        assert!(result.json_type.is_none());
    }

    #[test]
    #[ignore = "Requires database connection for full testing"]
    fn test_analyze_json_structure() {
        // Test JSON structure analysis - requires DB connection
        let json_expr = r#"{"users": [{"name": "Alice", "active": true}]}"#;
        let result = analyze_json_structure(json_expr, 3);
        // Should fail gracefully without database connection
        assert!(result.is_err());
    }
}