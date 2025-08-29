use crate::core::{Result, TuiqlError};
use crate::db;

/*
 * Schema Map Module for ER-like Diagram Visualization
 *
 * This module provides functionality for generating an
 * ER-like diagram of a database schema. It parses detailed schema information (tables, columns, foreign keys)
 * and produces a visual representation of the relationships between tables.
 *
 * Uses the enhanced schema introspection system to capture real database relationships.
 */

#[derive(Debug, Clone)]
pub struct SchemaMap {
    pub tables: Vec<TableNode>,
    pub relationships: Vec<Relationship>,
}

#[derive(Debug, Clone)]
pub struct TableNode {
    pub name: String,
    pub columns: Vec<String>,
    pub primary_keys: Vec<String>,
    pub outgoing_references: Vec<String>, // Tables this table references
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub from_table: String,
    pub from_column: String,
    pub to_table: String,
    pub to_column: String,
}

/// Generates a schema map from the current database schema.
/// Uses the enhanced schema introspection system to capture real database relationships.
pub fn generate_schema_map() -> Result<SchemaMap> {
    use crate::core::db::schema::Schema;

    // Access the database connection and use the core schema system
    let state_cell = db::DB_STATE.get().ok_or(TuiqlError::Schema("No database connection found. Please connect to a database first.".to_string()))?;
    let state_guard = state_cell
        .lock()
        .map_err(|_| TuiqlError::Schema("Failed to acquire database connection lock".to_string()))?;
    let conn = state_guard
        .connection
        .as_ref()
        .ok_or(TuiqlError::Schema("No active database connection".to_string()))?;

    let schema = Schema::from_connection(conn)?;

    let mut tables = Vec::new();
    let mut relationships = Vec::new();

    for (table_name, table_info) in &schema.tables {
        // Extract column names and find primary keys
        let mut columns = Vec::new();
        let mut primary_keys = Vec::new();

        for col in &table_info.columns {
            columns.push(format!("{} {}", col.name, col.type_name));
            if col.pk {
                primary_keys.push(col.name.clone());
            }
        }

        // Track outgoing references for this table
        let mut outgoing_references = Vec::new();

        // Process foreign keys to build relationships
        for fk in &table_info.foreign_keys {
            relationships.push(Relationship {
                from_table: table_name.clone(),
                from_column: fk.from_column.clone(),
                to_table: fk.referenced_table.clone(),
                to_column: fk.to_column.clone(),
            });

            if !outgoing_references.contains(&fk.referenced_table) {
                outgoing_references.push(fk.referenced_table.clone());
            }
        }

        tables.push(TableNode {
            name: table_name.clone(),
            columns,
            primary_keys,
            outgoing_references,
        });
    }

    Ok(SchemaMap { tables, relationships })
}

/// Renders the schema map as a comprehensive ER-like ASCII diagram.
/// Shows tables, columns, relationships, and important metadata.
pub fn render_schema_map(map: &SchemaMap) -> String {
    let mut diagram = String::new();

    diagram.push_str("=== Database Schema Map (ER Diagram) ===\n\n");

    if map.tables.is_empty() {
        diagram.push_str("No tables found in the database.\n");
        return diagram;
    }

    // Sort tables alphabetically for consistent output
    let mut sorted_tables = map.tables.clone();
    sorted_tables.sort_by(|a, b| a.name.cmp(&b.name));

    // Render each table
    for table in &sorted_tables {
        diagram.push_str(&format!("📋 Table: {}\n", table.name));

        // Show primary keys
        if !table.primary_keys.is_empty() {
            diagram.push_str(&format!("  🔑 Primary Keys: {}\n", table.primary_keys.join(", ")));
        }

        // Show all columns
        diagram.push_str("  📝 Columns:\n");
        for col in &table.columns {
            diagram.push_str(&format!("    - {}\n", col));
        }

        // Show outgoing relationships
        if !table.outgoing_references.is_empty() {
            diagram.push_str("  🔗 References:\n");
            for ref_table in &table.outgoing_references {
                // Find the specific relationship details
                let relationships = map.relationships.iter()
                    .filter(|r| r.from_table == table.name && r.to_table == *ref_table)
                    .collect::<Vec<_>>();

                if relationships.len() == 1 {
                    let rel = &relationships[0];
                    diagram.push_str(&format!("    → {} ({} → {})\n", ref_table, rel.from_column, rel.to_column));
                } else {
                    // Multiple relationships to the same table
                    diagram.push_str(&format!("    → {} (", ref_table));
                    for (i, rel) in relationships.iter().enumerate() {
                        if i > 0 { diagram.push_str(", "); }
                        diagram.push_str(&format!("{}→{}", rel.from_column, rel.to_column));
                    }
                    diagram.push_str(")\n");
                }
            }
        }

        // Show count of incoming relationships (tables that reference this table)
        let incoming_count = map.relationships.iter()
            .filter(|r| r.to_table == table.name)
            .count();
        if incoming_count > 0 {
            diagram.push_str(&format!("  ↙ Referenced by {} table(s)\n", incoming_count));
        }

        diagram.push_str("\n");
    }

    // Add a visual representation of the main relationships
    if !map.relationships.is_empty() {
        diagram.push_str("=== Relationship Overview ===\n");

        // Group relationships by from_table for better display
        let mut relationships_by_from: std::collections::HashMap<String, Vec<&Relationship>> =
            std::collections::HashMap::new();

        for rel in &map.relationships {
            relationships_by_from.entry(rel.from_table.clone()).or_default().push(rel);
        }

        for (from_table, relationships) in relationships_by_from {
            for rel in relationships {
                diagram.push_str(&format!("{} → {} ({} → {})\n",
                    from_table, rel.to_table, rel.from_column, rel.to_column));
            }
        }

        // Check for circular references
        let mut circular_refs = Vec::new();
        for rel in &map.relationships {
            for reverse_rel in &map.relationships {
                if rel.from_table == reverse_rel.to_table &&
                   rel.to_table == reverse_rel.from_table {
                    let key = if rel.from_table < rel.to_table {
                        format!("{} <-> {}", rel.from_table, reverse_rel.to_table)
                    } else {
                        format!("{} <-> {}", rel.to_table, rel.from_table)
                    };
                    if !circular_refs.contains(&key) {
                        circular_refs.push(key);
                    }
                }
            }
        }

        if !circular_refs.is_empty() {
            diagram.push_str("\n⚠️  Circular References:\n");
            for circ_ref in circular_refs {
                diagram.push_str(&format!("  {}\n", circ_ref));
            }
        }
    } else {
        diagram.push_str("\nNo relationships found between tables.\n");
    }

    diagram.push_str("\n=== End Schema Map ===\n");
    diagram
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_schema_map_no_tables() {
        let map = SchemaMap {
            tables: vec![],
            relationships: vec![],
        };
        let output = render_schema_map(&map);
        assert!(output.contains("No tables found in the database"));
    }

    #[test]
    fn test_render_schema_map_with_tables() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER".to_string(), "name TEXT".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["orders".to_string()],
                },
                TableNode {
                    name: "orders".to_string(),
                    columns: vec!["id INTEGER".to_string(), "user_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![],
                },
            ],
            relationships: vec![
                Relationship {
                    from_table: "orders".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                }
            ],
        };
        let output = render_schema_map(&map);
        assert!(output.contains("Table: users"));
        assert!(output.contains("Table: orders"));
        assert!(output.contains("🔑 Primary Keys: id"));
        assert!(output.contains("→ orders"));
        assert!(output.contains("Referenced by 1 table(s)"));
    }
}
