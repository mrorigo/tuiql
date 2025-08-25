/// Schema Navigator Module
///
/// This module provides functionality for navigating the database schema
/// in a tree-based structure. It includes features such as displaying
/// row counts, primary/foreign key indicators, and index details.
use std::collections::HashMap;

/// Represents a table in the schema navigator.
#[derive(Debug, Clone)]
pub struct Table {
    pub name: String,
    pub row_count: usize,
    pub primary_key: Option<String>,
    pub foreign_keys: Vec<String>,
    pub indexes: Vec<String>,
}

/// Represents the schema navigator structure.
#[derive(Debug, Clone)]
pub struct SchemaNavigator {
    pub tables: HashMap<String, Table>,
}

impl SchemaNavigator {
    /// Creates a new, empty SchemaNavigator.
    pub fn new() -> Self {
        SchemaNavigator {
            tables: HashMap::new(),
        }
    }

    /// Adds a table to the schema navigator.
    pub fn add_table(
        &mut self,
        name: String,
        row_count: usize,
        primary_key: Option<String>,
        foreign_keys: Vec<String>,
        indexes: Vec<String>,
    ) {
        let table = Table {
            name: name.clone(),
            row_count,
            primary_key,
            foreign_keys,
            indexes,
        };
        self.tables.insert(name, table);
    }

    /// Renders the schema navigator as a tree-like string.
    pub fn render(&self) -> String {
        let mut output = String::new();
        for table in self.tables.values() {
            output.push_str(&format!("Table: {}\n", table.name));
            output.push_str(&format!("  Row Count: {}\n", table.row_count));
            if let Some(pk) = &table.primary_key {
                output.push_str(&format!("  Primary Key: {}\n", pk));
            }
            if !table.foreign_keys.is_empty() {
                output.push_str("  Foreign Keys:\n");
                for fk in &table.foreign_keys {
                    output.push_str(&format!("    -> {}\n", fk));
                }
            }
            if !table.indexes.is_empty() {
                output.push_str("  Indexes:\n");
                for index in &table.indexes {
                    output.push_str(&format!("    - {}\n", index));
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
        let mut navigator = SchemaNavigator::new();
        navigator.add_table(
            "users".to_string(),
            100,
            Some("id".to_string()),
            vec!["orders".to_string()],
            vec!["idx_users_name".to_string()],
        );
        navigator.add_table(
            "orders".to_string(),
            50,
            Some("order_id".to_string()),
            vec![],
            vec!["idx_orders_date".to_string()],
        );

        let rendered = navigator.render();
        assert!(rendered.contains("Table: users"));
        assert!(rendered.contains("Row Count: 100"));
        assert!(rendered.contains("Primary Key: id"));
        assert!(rendered.contains("-> orders"));
        assert!(rendered.contains("- idx_users_name"));

        assert!(rendered.contains("Table: orders"));
        assert!(rendered.contains("Row Count: 50"));
        assert!(rendered.contains("Primary Key: order_id"));
        assert!(rendered.contains("- idx_orders_date"));
    }
}
