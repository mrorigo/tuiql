/// Schema Navigator Module
///
/// This module provides functionality for navigating the database schema
/// in a tree-based structure. It includes features such as displaying
/// row counts, primary/foreign key indicators, and index details.
use crate::core::{Result, TuiqlError};
use crate::db;
use std::collections::HashMap;

/// Represents a table in the schema navigator.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub row_count: Option<usize>,
    pub columns: Vec<db::Column>,
    pub indexes: Vec<db::Index>,
}

/// Represents the schema navigator structure.
#[derive(Debug, Clone)]
pub struct SchemaNavigator {
    pub tables: HashMap<String, Table>,
}

impl SchemaNavigator {
    /// Creates a new, empty SchemaNavigator.
    pub fn new() -> Result<Self> {
        let schema = db::get_schema().map_err(|err| TuiqlError::App(err))?;
        let mut tables = HashMap::new();

        for (name, db_table) in schema.tables {
            // Get row count
            let row_count = match db::execute_query(&format!("SELECT COUNT(*) FROM '{}'", name)) {
                Ok(result) => Some(result.rows[0][0].parse::<usize>().unwrap_or(0)),
                Err(e) => {
                    // Log the query error but continue with None for row count
                    eprintln!("Warning: Failed to get row count for table '{}': {}", name, e);
                    None
                }
            };

            let table = Table {
                name: name.clone(),
                row_count,
                columns: db_table.columns,
                indexes: db_table.indexes,
            };
            tables.insert(name, table);
        }

        Ok(SchemaNavigator { tables })
    }

    /// Renders the schema navigator as a tree-like string.
    pub fn render(&self) -> String {
        let mut output = String::new();
        for table in self.tables.values() {
            output.push_str(&format!("Table: {}\n", table.name));
            if let Some(count) = table.row_count {
                output.push_str(&format!("  Row Count: {}\n", count));
            }

            output.push_str("  Columns:\n");
            for col in &table.columns {
                let flags = match (col.pk, col.notnull) {
                    (true, _) => "[PK]",
                    (false, true) => "[NOT NULL]",
                    (false, false) => "",
                };
                output.push_str(&format!("    {} {} {}\n", col.name, col.type_name, flags));
            }

            if !table.indexes.is_empty() {
                output.push_str("  Indexes:\n");
                for index in &table.indexes {
                    let unique = if index.unique { "[UNIQUE] " } else { "" };
                    output.push_str(&format!(
                        "    - {}{} ({})\n",
                        unique,
                        index.name,
                        index.columns.join(", ")
                    ));
                }
            }
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_navigator_render() {
        // Set up a clean test database
        db::tests::setup_test_db();
        db::tests::reset_test_db();

        // Create and verify the navigator
        let navigator = SchemaNavigator::new().unwrap();
        let rendered = navigator.render();

        // Verify table structure
        assert!(rendered.contains("Table: test"));
        assert!(
            rendered.contains("Row Count: 2"),
            "Expected row count of 2 in output:\n{}",
            rendered
        ); // Two test rows were inserted

        // Verify column definitions
        assert!(rendered.contains("id INTEGER [PK]"));
        assert!(rendered.contains("name TEXT"));
        assert!(rendered.contains("value REAL"));

        // Verify indexes
        assert!(rendered.contains("- idx_test_name (name)"));
        assert!(rendered.contains("[UNIQUE] idx_test_value (value)"));
    }

    #[test]
    fn test_schema_navigator_empty_db() {
        // Set up empty in-memory database
        db::tests::setup_test_db();
        if let Ok(guard) = db::DB_STATE.get().unwrap().lock() {
            if let Some(conn) = &guard.connection {
                conn.execute("DROP TABLE IF EXISTS test", []).unwrap();
            }
        }

        // Create navigator and verify empty state
        let navigator = SchemaNavigator::new().unwrap();
        let rendered = navigator.render();
        assert_eq!(rendered, ""); // Empty schema should render nothing
    }
}
