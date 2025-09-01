use crate::core::{Result, TuiqlError};
use crate::db;
use std::collections::{HashMap, HashSet};

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

#[derive(Debug, Clone)]
struct TableGroup {
    tables: Vec<String>,
    connections: usize,
}

/// Generates connectivity-based groups of tables for better auto-layout organization
fn generate_table_groups(map: &SchemaMap) -> Vec<TableGroup> {
    // Create adjacency lists for graph traversal
    let mut adjacency: HashMap<&String, Vec<&String>> = HashMap::new();
    let mut reverse_adjacency: HashMap<&String, Vec<&String>> = HashMap::new();

    for rel in &map.relationships {
        adjacency.entry(&rel.from_table).or_default().push(&rel.to_table);
        reverse_adjacency.entry(&rel.to_table).or_default().push(&rel.from_table);
    }

    // Find strongly connected components using DFS
    let mut visited: HashSet<String> = HashSet::new();
    let mut components = Vec::new();

    for table in &map.tables {
        if !visited.contains(&table.name) {
            let mut component = Vec::new();
            find_connected_component(&table.name, &adjacency, &reverse_adjacency, &mut visited, &mut component);
            if !component.is_empty() {
                let connections = component.len() * 2; // Simple heuristic based on component size
                components.push(TableGroup {
                    tables: component,
                    connections,
                });
            }
        }
    }

    // Create groups for orphaned tables (no relationships)
    let grouped_names: HashSet<String> = components.iter()
        .flat_map(|g| g.tables.iter())
        .cloned()
        .collect();

    for table in &map.tables {
        if !grouped_names.contains(&table.name) {
            components.push(TableGroup {
                tables: vec![table.name.clone()],
                connections: 0,
            });
        }
    }

    // Sort groups by connectivity (highly connected first)
    components.sort_by(|a, b| b.connections.cmp(&a.connections));

    components
}

/// Helper function for finding connected components
fn find_connected_component(
    table: &String,
    adjacency: &HashMap<&String, Vec<&String>>,
    reverse_adj: &HashMap<&String, Vec<&String>>,
    visited: &mut HashSet<String>,
    component: &mut Vec<String>
) {
    if visited.contains(table) {
        return;
    }

    visited.insert(table.clone());
    component.push(table.clone());

    if let Some(neighbors) = adjacency.get(table) {
        for neighbor in neighbors {
            find_connected_component(neighbor, adjacency, reverse_adj, visited, component);
        }
    }

    if let Some(reverse_neighbors) = reverse_adj.get(table) {
        for neighbor in reverse_neighbors {
            find_connected_component(neighbor, adjacency, reverse_adj, visited, component);
        }
    }
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

    diagram.push_str("=== Database Schema Map (Auto-layout ER Diagram) ===\n\n");

    if map.tables.is_empty() {
        diagram.push_str("No tables found in the database.\n");
        return diagram;
    }

    // Auto-layout: Group tables by connectivity for better visualization
    let table_groups = generate_table_groups(map);

    // Render each group
    for (group_idx, group) in table_groups.iter().enumerate() {
        if table_groups.len() > 1 {
            let group_type = if group.connections > 0 {
                "Highly Connected Tables"
            } else {
                "Independent Tables"
            };
            diagram.push_str(&format!("📂 Group {}: {}\n", group_idx + 1, group_type));
        }

        // Sort tables within group alphabetically
        let mut sorted_tables_in_group: Vec<&String> = group.tables.iter().collect();
        sorted_tables_in_group.sort();

        // Render each table in the group
        for table_name in &sorted_tables_in_group {
            if let Some(table) = map.tables.iter().find(|t| &t.name == *table_name) {
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

                diagram.push('\n');
            }
        }

        if table_groups.len() > 1 {
            diagram.push('\n');
        }
    }

    // Add a visual representation of the main relationships
    if !map.relationships.is_empty() {
        diagram.push_str("=== Relationship Overview ===\n");

        // Group relationships by from_table for better display
        let mut relationships_by_from: std::collections::BTreeMap<String, Vec<&Relationship>> =
            std::collections::BTreeMap::new();

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
    use insta::assert_snapshot;

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

    #[test]
    fn test_generate_table_groups_connected_tables() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["orders".to_string(), "posts".to_string()],
                },
                TableNode {
                    name: "orders".to_string(),
                    columns: vec!["id INTEGER".to_string(), "user_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["products".to_string()],
                },
                TableNode {
                    name: "posts".to_string(),
                    columns: vec!["id INTEGER".to_string(), "user_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![],
                },
                TableNode {
                    name: "products".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![],
                },
                TableNode {
                    name: "categories".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![], // No relationships - should be in independent group
                },
            ],
            relationships: vec![
                Relationship {
                    from_table: "orders".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                },
                Relationship {
                    from_table: "posts".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                },
                Relationship {
                    from_table: "orders".to_string(),
                    from_column: "product_id".to_string(),
                    to_table: "products".to_string(),
                    to_column: "id".to_string(),
                },
            ],
        };

        let groups = generate_table_groups(&map);

        // Should have groups sorted by connectivity (highly connected first)
        assert!(groups.len() >= 2); // At least connected group and independent group

        // First group should be highly connected
        let first_group = &groups[0];
        assert!(first_group.connections > 0);

        // Check that connected tables are grouped together
        let connected_names: HashSet<String> = vec!["users", "orders", "posts", "products"]
            .into_iter()
            .map(String::from)
            .collect();

        // Find the highly connected group
        let mut found_connected_group = false;
        for group in &groups {
            if group.connections > 0 {
                found_connected_group = true;
                // Group should contain at least users and orders (directly connected)
                let group_names: HashSet<String> = group.tables.iter().cloned().collect();
                assert!(group_names.contains("users"), "Connected group should contain 'users' table");
                assert!(group_names.contains("orders"), "Connected group should contain 'orders' table");

                // The group may or may not contain other connected tables depending on the algorithm
                // But it should definitely have the core connected tables
                break; // Found the highly connected group, done testing this part
            }
        }
        assert!(found_connected_group, "Should have at least one highly connected group");
    }

    #[test]
    fn test_find_connected_component() {
        let mut adjacency: HashMap<&String, Vec<&String>> = HashMap::new();
        let mut reverse_adjacency: HashMap<&String, Vec<&String>> = HashMap::new();

        // Set up test relationships: A->B->C
        let table_a = String::from("A");
        let table_b = String::from("B");
        let table_c = String::from("C");

        adjacency.insert(&table_a, vec![&table_b]);
        adjacency.insert(&table_b, vec![&table_c]);
        reverse_adjacency.insert(&table_c, vec![&table_b]);
        reverse_adjacency.insert(&table_b, vec![&table_a]);

        let mut visited: HashSet<String> = HashSet::new();
        let mut component = Vec::new();

        find_connected_component(&table_a, &adjacency, &reverse_adjacency, &mut visited, &mut component);

        assert_eq!(component.len(), 3);
        assert!(component.contains(&table_a));
        assert!(component.contains(&table_b));
        assert!(component.contains(&table_c));
    }

    #[test]
    fn test_render_schema_map_with_multiple_groups() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["orders".to_string()],
                },
                TableNode {
                    name: "orders".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![],
                },
                TableNode {
                    name: "products".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![], // No relationships
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

        // Should contain the new auto-layout header
        assert!(output.contains("Auto-layout ER Diagram"));

        // Should show group headers when there are multiple groups
        assert!(output.contains("Group 1"));

        // Should contain the original table content
        assert!(output.contains("Table: users"));
        assert!(output.contains("Table: orders"));
        assert!(output.contains("Table: products"));
    }

    #[test]
    fn test_render_schema_map_golden_simple_relationship() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER PRIMARY KEY".to_string(), "name TEXT".to_string(), "email TEXT".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["orders".to_string()],
                },
                TableNode {
                    name: "orders".to_string(),
                    columns: vec!["id INTEGER PRIMARY KEY".to_string(), "user_id INTEGER".to_string(), "amount REAL".to_string()],
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
        assert_snapshot!("schema_map_simple_relationship", output);
    }

    #[test]
    fn test_render_schema_map_golden_complex_relationships() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER".to_string(), "name TEXT".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["orders".to_string(), "posts".to_string()],
                },
                TableNode {
                    name: "orders".to_string(),
                    columns: vec!["id INTEGER".to_string(), "user_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["products".to_string()],
                },
                TableNode {
                    name: "posts".to_string(),
                    columns: vec!["id INTEGER".to_string(), "user_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![],
                },
                TableNode {
                    name: "products".to_string(),
                    columns: vec!["id INTEGER".to_string(), "category_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["categories".to_string()],
                },
                TableNode {
                    name: "categories".to_string(),
                    columns: vec!["id INTEGER".to_string(), "name TEXT".to_string()],
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
                },
                Relationship {
                    from_table: "posts".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                },
                Relationship {
                    from_table: "orders".to_string(),
                    from_column: "product_id".to_string(),
                    to_table: "products".to_string(),
                    to_column: "id".to_string(),
                },
                Relationship {
                    from_table: "products".to_string(),
                    from_column: "category_id".to_string(),
                    to_table: "categories".to_string(),
                    to_column: "id".to_string(),
                },
            ],
        };
        let output = render_schema_map(&map);
        assert_snapshot!("schema_map_complex_relationships", output);
    }

    #[test]
    fn test_render_schema_map_golden_no_relationships() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER".to_string(), "name TEXT".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec![],
                },
                TableNode {
                    name: "logs".to_string(),
                    columns: vec!["timestamp DATETIME".to_string(), "message TEXT".to_string()],
                    primary_keys: vec![],
                    outgoing_references: vec![],
                },
            ],
            relationships: vec![],
        };
        let output = render_schema_map(&map);
        assert_snapshot!("schema_map_no_relationships", output);
    }

    #[test]
    fn test_render_schema_map_golden_circular_reference() {
        let map = SchemaMap {
            tables: vec![
                TableNode {
                    name: "users".to_string(),
                    columns: vec!["id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["posts".to_string()],
                },
                TableNode {
                    name: "posts".to_string(),
                    columns: vec!["id INTEGER".to_string(), "user_id INTEGER".to_string(), "parent_post_id INTEGER".to_string()],
                    primary_keys: vec!["id".to_string()],
                    outgoing_references: vec!["users".to_string(), "posts".to_string()], // Circular
                },
            ],
            relationships: vec![
                Relationship {
                    from_table: "posts".to_string(),
                    from_column: "user_id".to_string(),
                    to_table: "users".to_string(),
                    to_column: "id".to_string(),
                },
                Relationship {
                    from_table: "posts".to_string(),
                    from_column: "parent_post_id".to_string(),
                    to_table: "posts".to_string(),
                    to_column: "id".to_string(),
                },
                // Add reverse relationship to create circular
                Relationship {
                    from_table: "users".to_string(),
                    from_column: "main_post_id".to_string(),
                    to_table: "posts".to_string(),
                    to_column: "id".to_string(),
                },
            ],
        };
        let output = render_schema_map(&map);
        assert_snapshot!("schema_map_circular_reference", output);
    }
}
