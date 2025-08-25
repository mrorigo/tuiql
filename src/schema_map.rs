use serde_json;

/*
 * Schema Map Module Stub for ER-like Diagram Visualization
 *
 * This module provides functionality for generating an
 * ER-like diagram of a database schema. It parses detailed schema information (tables, columns, foreign keys)
 * and produces a visual representation of the relationships between tables.
 *
 * The current stub implementation returns a fixed dummy schema map and renders it
 * as a simple ASCII diagram.
 */

#[derive(Debug, PartialEq)]
pub struct SchemaMap {
    pub nodes: Vec<SchemaNode>,
}

#[derive(Debug, PartialEq)]
pub struct SchemaNode {
    pub table_name: String,
    // List of table names that this table references via foreign keys.
    pub foreign_keys: Vec<String>,
}

/// Generates a dummy schema map from the provided schema data.
/// In a real implementation, `schema_data` might be a structured representation
/// of the database schema (e.g., JSON or a specialized data structure).
pub fn generate_schema_map(schema_data: &str) -> SchemaMap {
    // Parse the input `schema_data` to generate a schema map.
    // Assume `schema_data` is a JSON string representing the schema.
    let parsed_data: serde_json::Value = serde_json::from_str(schema_data).unwrap();
    let mut nodes = Vec::new();

    if let Some(tables) = parsed_data["tables"].as_array() {
        for table in tables {
            let table_name = table["name"].as_str().unwrap().to_string();
            let foreign_keys = table["foreign_keys"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|fk| fk.as_str().unwrap().to_string())
                .collect();

            nodes.push(SchemaNode {
                table_name,
                foreign_keys,
            });
        }
    }

    SchemaMap { nodes }
}

/// Renders the schema map as a simple ASCII diagram.
/// This implementation converts the schema map into a string diagram for visualization.
pub fn render_schema_map(map: &SchemaMap) -> String {
    let mut diagram = String::new();
    for node in &map.nodes {
        diagram.push_str(&format!("Table: {}\n", node.table_name));
        if !node.foreign_keys.is_empty() {
            diagram.push_str("  Foreign Keys:\n");
            for fk in &node.foreign_keys {
                diagram.push_str(&format!("    -> {}\n", fk));
            }
        } else {
            diagram.push_str("  No Foreign Keys\n");
        }
    }
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_schema_map() {
        let schema_data = r#"
        {
            "tables": [
                {
                    "name": "users",
                    "foreign_keys": ["orders"]
                },
                {
                    "name": "orders",
                    "foreign_keys": []
                }
            ]
        }
        "#;
        let map = generate_schema_map(schema_data);
        assert_eq!(map.nodes.len(), 2);
        assert_eq!(map.nodes[0].table_name, "users");
        assert_eq!(map.nodes[0].foreign_keys, vec!["orders"]);
    }

    #[test]
    fn test_render_schema_map() {
        let map = SchemaMap {
            nodes: vec![
                SchemaNode {
                    table_name: String::from("users"),
                    foreign_keys: vec![String::from("orders")],
                },
                SchemaNode {
                    table_name: String::from("orders"),
                    foreign_keys: Vec::new(),
                },
            ],
        };
        let output = render_schema_map(&map);
        assert!(output.contains("Table: users"));
        assert!(output.contains("-> orders"));
        assert!(output.contains("Table: orders"));
    }
}
