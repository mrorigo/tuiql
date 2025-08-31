use crate::core::{Result, TuiqlError};
use crate::core::db::schema::{Schema, Column, Index, ForeignKey, Table};
use std::collections::HashMap;
use rusqlite::Connection;

/// Represents the type of schema difference
#[derive(Debug, Clone, PartialEq)]
pub enum DiffType {
    TableAdded,
    TableRemoved,
    TableChanged,
    ColumnAdded,
    ColumnRemoved,
    ColumnChanged,
    IndexAdded,
    IndexRemoved,
    ForeignKeyAdded,
    ForeignKeyRemoved,
}

/// Represents a single diff item
#[derive(Debug, Clone)]
pub struct SchemaDiff {
    pub diff_type: DiffType,
    pub table_name: String,
    pub field_name: Option<String>,
    pub description: String,
}

/// Result of comparing two schemas
#[derive(Debug)]
pub struct SchemaComparison {
    pub added_tables: Vec<String>,
    pub removed_tables: Vec<String>,
    pub changed_tables: Vec<String>,
    pub detailed_diffs: Vec<SchemaDiff>,
}

/// Compares schemas from two database files
///
/// # Arguments
///
/// * `path_a` - Path to the first database file
/// * `path_b` - Path to the second database file
///
/// # Returns
///
/// * `Ok(SchemaComparison)` with the detailed comparison
/// * `Err(TuiqlError)` if either database cannot be opened or schemas cannot be introspected
pub fn compare_databases(path_a: &str, path_b: &str) -> Result<SchemaComparison> {
    let conn_a = Connection::open(path_a)
        .map_err(|e| TuiqlError::Database(e))?;
    let conn_b = Connection::open(path_b)
        .map_err(|e| TuiqlError::Database(e))?;

    let schema_a = Schema::from_connection(&conn_a)?;
    let schema_b = Schema::from_connection(&conn_b)?;

    compare_schemas(&schema_a, &schema_b)
}

/// Compares two Schema objects and returns detailed differences
pub fn compare_schemas(schema_a: &Schema, schema_b: &Schema) -> Result<SchemaComparison> {
    let mut added_tables = Vec::new();
    let mut removed_tables = Vec::new();
    let mut changed_tables = Vec::new();
    let mut detailed_diffs = Vec::new();

    // Find added tables (in B but not in A)
    for (table_name, _) in &schema_b.tables {
        if !schema_a.tables.contains_key(table_name) {
            added_tables.push(table_name.clone());
            detailed_diffs.push(SchemaDiff {
                diff_type: DiffType::TableAdded,
                table_name: table_name.clone(),
                field_name: None,
                description: format!("Table '{}' was added", table_name),
            });
        }
    }

    // Find removed tables (in A but not in B)
    for (table_name, _) in &schema_a.tables {
        if !schema_b.tables.contains_key(table_name) {
            removed_tables.push(table_name.clone());
            detailed_diffs.push(SchemaDiff {
                diff_type: DiffType::TableRemoved,
                table_name: table_name.clone(),
                field_name: None,
                description: format!("Table '{}' was removed", table_name),
            });
        }
    }

    // Compare common tables
    for (table_name, table_a) in &schema_a.tables {
        if let Some(table_b) = schema_b.tables.get(table_name) {
            if let Some(table_diffs) = compare_tables(table_a, table_b) {
                changed_tables.push(table_name.clone());
                detailed_diffs.extend(table_diffs);
            }
        }
    }

    Ok(SchemaComparison {
        added_tables,
        removed_tables,
        changed_tables,
        detailed_diffs,
    })
}

/// Compares two tables and returns their differences if any
fn compare_tables(table_a: &Table, table_b: &Table) -> Option<Vec<SchemaDiff>> {
    let mut diffs = Vec::new();

    // Compare columns
    diffs.extend(compare_columns(&table_a.name, &table_a.columns, &table_b.columns));

    // Compare indexes
    diffs.extend(compare_indexes(&table_a.name, &table_a.indexes, &table_b.indexes));

    // Compare foreign keys
    diffs.extend(compare_foreign_keys(&table_a.name, &table_a.foreign_keys, &table_b.foreign_keys));

    if diffs.is_empty() {
        None
    } else {
        Some(diffs)
    }
}

/// Compares columns between two tables
fn compare_columns(table_name: &str, cols_a: &[Column], cols_b: &[Column]) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();
    let cols_a_map: HashMap<&str, &Column> = cols_a.iter().map(|c| (c.name.as_str(), c)).collect();
    let cols_b_map: HashMap<&str, &Column> = cols_b.iter().map(|c| (c.name.as_str(), c)).collect();

    // Find added columns
    for (col_name, col_b) in &cols_b_map {
        if !cols_a_map.contains_key(col_name) {
            diffs.push(SchemaDiff {
                diff_type: DiffType::ColumnAdded,
                table_name: table_name.to_string(),
                field_name: Some(col_name.to_string()),
                description: format!("Column '{}' was added with type {}", col_name, col_b.type_name),
            });
        }
    }

    // Find removed columns
    for (col_name, _) in &cols_a_map {
        if !cols_b_map.contains_key(col_name) {
            diffs.push(SchemaDiff {
                diff_type: DiffType::ColumnRemoved,
                table_name: table_name.to_string(),
                field_name: Some(col_name.to_string()),
                description: format!("Column '{}' was removed", col_name),
            });
        }
    }

    // Compare common columns
    for (col_name, col_a) in &cols_a_map {
        if let Some(col_b) = cols_b_map.get(col_name) {
            if let Some(diff) = compare_column(col_a, col_b) {
                diffs.push(SchemaDiff {
                    diff_type: DiffType::ColumnChanged,
                    table_name: table_name.to_string(),
                    field_name: Some(col_name.to_string()),
                    description: diff,
                });
            }
        }
    }

    diffs
}

/// Compares two individual columns and returns description of differences if any
fn compare_column(col_a: &Column, col_b: &Column) -> Option<String> {
    let mut differences = Vec::new();

    if col_a.type_name != col_b.type_name {
        differences.push(format!("type: {} → {}", col_a.type_name, col_b.type_name));
    }
    if col_a.notnull != col_b.notnull {
        differences.push(format!("notnull: {} → {}", col_a.notnull, col_b.notnull));
    }
    if col_a.pk != col_b.pk {
        differences.push(format!("primary key: {} → {}", col_a.pk, col_b.pk));
    }
    if col_a.dflt_value != col_b.dflt_value {
        let a_val = col_a.dflt_value.as_deref().unwrap_or("NULL");
        let b_val = col_b.dflt_value.as_deref().unwrap_or("NULL");
        differences.push(format!("default: {} → {}", a_val, b_val));
    }

    if differences.is_empty() {
        None
    } else {
        Some(differences.join(", "))
    }
}

/// Compares indexes between two tables
fn compare_indexes(table_name: &str, indexes_a: &[Index], indexes_b: &[Index]) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();
    let idx_a_map: HashMap<&str, &Index> = indexes_a.iter().map(|i| (i.name.as_str(), i)).collect();
    let idx_b_map: HashMap<&str, &Index> = indexes_b.iter().map(|i| (i.name.as_str(), i)).collect();

    // Find added indexes
    for (idx_name, _) in &idx_b_map {
        if !idx_a_map.contains_key(idx_name) {
            diffs.push(SchemaDiff {
                diff_type: DiffType::IndexAdded,
                table_name: table_name.to_string(),
                field_name: Some(idx_name.to_string()),
                description: format!("Index '{}' was added", idx_name),
            });
        }
    }

    // Find removed indexes
    for (idx_name, _) in &idx_a_map {
        if !idx_b_map.contains_key(idx_name) {
            diffs.push(SchemaDiff {
                diff_type: DiffType::IndexRemoved,
                table_name: table_name.to_string(),
                field_name: Some(idx_name.to_string()),
                description: format!("Index '{}' was removed", idx_name),
            });
        }
    }

    diffs
}

/// Compares foreign keys between two tables
fn compare_foreign_keys(table_name: &str, fks_a: &[ForeignKey], fks_b: &[ForeignKey]) -> Vec<SchemaDiff> {
    let mut diffs = Vec::new();

    // Simple comparison: check if the number of FKs differs
    if fks_a.len() != fks_b.len() {
        if fks_a.len() < fks_b.len() {
            diffs.push(SchemaDiff {
                diff_type: DiffType::ForeignKeyAdded,
                table_name: table_name.to_string(),
                field_name: None,
                description: format!("Foreign key(s) were added to table '{}'", table_name),
            });
        } else {
            diffs.push(SchemaDiff {
                diff_type: DiffType::ForeignKeyRemoved,
                table_name: table_name.to_string(),
                field_name: None,
                description: format!("Foreign key(s) were removed from table '{}'", table_name),
            });
        }
    }

    diffs
}

/// Generates a human-readable summary from a SchemaComparison
pub fn format_comparison(comparison: &SchemaComparison, path_a: &str, path_b: &str) -> String {
    let mut output = format!("Schema comparison between '{}' and '{}'\n", path_a, path_b);
    output.push_str(&"=".repeat(60));
    output.push('\n');

    if comparison.added_tables.is_empty() && comparison.removed_tables.is_empty() && comparison.changed_tables.is_empty() {
        output.push_str("✅ No differences found between the schemas.\n");
        return output;
    }

    if !comparison.added_tables.is_empty() {
        output.push_str(&format!("\n📋 Tables added ({}):\n", comparison.added_tables.len()));
        for table in &comparison.added_tables {
            output.push_str(&format!("  + {}\n", table));
        }
    }

    if !comparison.removed_tables.is_empty() {
        output.push_str(&format!("\n🗑️  Tables removed ({}):\n", comparison.removed_tables.len()));
        for table in &comparison.removed_tables {
            output.push_str(&format!("  - {}\n", table));
        }
    }

    if !comparison.changed_tables.is_empty() {
        output.push_str(&format!("\n🔄 Tables changed ({}):\n", comparison.changed_tables.len()));
        for table in &comparison.changed_tables {
            output.push_str(&format!("  ~ {}\n", table));
        }
    }

    if !comparison.detailed_diffs.is_empty() {
        output.push_str(&format!("\n📝 Detailed changes ({}):\n", comparison.detailed_diffs.len()));
        for diff in &comparison.detailed_diffs {
            output.push_str(&format!("  {}: {}\n", get_diff_symbol(&diff.diff_type), diff.description));
        }
    }

    output
}

/// Helper function to get a symbol for each diff type
fn get_diff_symbol(diff_type: &DiffType) -> &'static str {
    match diff_type {
        DiffType::TableAdded => "+",
        DiffType::TableRemoved => "-",
        DiffType::TableChanged => "~",
        DiffType::ColumnAdded => "++",
        DiffType::ColumnRemoved => "--",
        DiffType::ColumnChanged => "~~",
        DiffType::IndexAdded => "+i",
        DiffType::IndexRemoved => "-i",
        DiffType::ForeignKeyAdded => "+f",
        DiffType::ForeignKeyRemoved => "-f",
    }
}

/// Legacy function for backward compatibility - kept for existing tests
///
/// # Deprecated
/// Use `compare_databases` instead
pub fn diff_schemas(schema_a: &str, schema_b: &str) -> Result<String> {
    if schema_a.is_empty() || schema_b.is_empty() {
        return Err(TuiqlError::Schema(
            "One or both schema inputs are empty - cannot generate diff".to_string()
        ));
    }

    // Stub implementation:
    // If the schemas are identical, return a message indicating no differences.
    // Otherwise, return a message indicating differences.
    if schema_a == schema_b {
        Ok("No differences found.".to_string())
    } else {
        Ok("Differences found between schemas.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use tempfile::NamedTempFile;

    fn setup_test_schema_a(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT UNIQUE,
                age INTEGER
            );
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                title TEXT NOT NULL,
                content TEXT,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
            CREATE INDEX idx_users_age ON users(age);
        "
        )
    }

    fn setup_test_schema_b(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT,
                age INTEGER,
                phone TEXT
            );
            CREATE TABLE orders (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                amount REAL,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
            CREATE INDEX idx_users_name ON users(name);
        "
        )
    }

    fn create_test_db<F>(setup_fn: F) -> NamedTempFile
    where
        F: Fn(&Connection) -> rusqlite::Result<()>,
    {
        let tmp_file = NamedTempFile::new().unwrap();
        let conn = Connection::open(tmp_file.path()).unwrap();
        setup_fn(&conn).unwrap();
        tmp_file
    }

    #[test]
    fn test_compare_databases_identical() {
        let db1 = create_test_db(setup_test_schema_a);
        let db2 = create_test_db(setup_test_schema_a);

        let result = compare_databases(
            db1.path().to_str().unwrap(),
            db2.path().to_str().unwrap(),
        );

        assert!(result.is_ok());
        let comparison = result.unwrap();
        assert!(comparison.added_tables.is_empty());
        assert!(comparison.removed_tables.is_empty());
        assert!(comparison.changed_tables.is_empty());
        assert!(comparison.detailed_diffs.is_empty());
    }

    #[test]
    fn test_compare_databases_with_differences() {
        let db1 = create_test_db(setup_test_schema_a);
        let db2 = create_test_db(setup_test_schema_b);

        let result = compare_databases(
            db1.path().to_str().unwrap(),
            db2.path().to_str().unwrap(),
        );

        assert!(result.is_ok());
        let comparison = result.unwrap();

        // posts table removed, orders table added
        assert_eq!(comparison.removed_tables, vec!["posts"]);
        assert_eq!(comparison.added_tables, vec!["orders"]);

        // users table should be marked as changed
        assert!(comparison.changed_tables.contains(&"users".to_string()));

        // Should have detailed diffs
        assert!(!comparison.detailed_diffs.is_empty());
    }

    #[test]
    fn test_compare_databases_invalid_file() {
        let result = compare_databases("/dev/null/nonexistent.db", "/tmp/random_dir");
        // Should fail because these are not valid database files
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), TuiqlError::Database(_)));
    }

    #[test]
    fn test_format_comparison_no_differences() {
        let comparison = SchemaComparison {
            added_tables: Vec::new(),
            removed_tables: Vec::new(),
            changed_tables: Vec::new(),
            detailed_diffs: Vec::new(),
        };

        let output = format_comparison(&comparison, "db1.db", "db2.db");
        assert!(output.contains("No differences found"));
        assert!(output.contains("db1.db"));
        assert!(output.contains("db2.db"));
    }

    #[test]
    fn test_format_comparison_with_differences() {
        let comparison = SchemaComparison {
            added_tables: vec!["new_table".to_string()],
            removed_tables: vec!["old_table".to_string()],
            changed_tables: vec!["modified_table".to_string()],
            detailed_diffs: vec![
                SchemaDiff {
                    diff_type: DiffType::TableAdded,
                    table_name: "new_table".to_string(),
                    field_name: None,
                    description: "Test add".to_string(),
                },
                SchemaDiff {
                    diff_type: DiffType::ColumnAdded,
                    table_name: "modified_table".to_string(),
                    field_name: Some("new_column".to_string()),
                    description: "Test column change".to_string(),
                },
                SchemaDiff {
                    diff_type: DiffType::ColumnRemoved,
                    table_name: "modified_table".to_string(),
                    field_name: Some("old_column".to_string()),
                    description: "Test column removal".to_string(),
                },
                SchemaDiff {
                    diff_type: DiffType::TableRemoved,
                    table_name: "old_table".to_string(),
                    field_name: None,
                    description: "Test table removal".to_string(),
                },
            ],
        };

        let output = format_comparison(&comparison, "a.db", "b.db");
        assert!(output.contains("Tables added (1)"));
        assert!(output.contains("Tables removed (1)"));
        assert!(output.contains("Tables changed (1)"));
        assert!(output.contains("Detailed changes (4)"));
        assert!(output.contains("+ new_table"));
        assert!(output.contains("- old_table"));
        assert!(output.contains("~ modified_table"));
        assert!(output.contains("++"));
        assert!(output.contains("--"));
    }

    #[test]
    fn test_compare_columns() {
        use crate::core::db::schema::Column;

        let cols_a = vec![
            Column {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                notnull: false,
                pk: true,
                dflt_value: None,
            },
            Column {
                name: "name".to_string(),
                type_name: "TEXT".to_string(),
                notnull: true,
                pk: false,
                dflt_value: None,
            },
        ];

        let cols_b = vec![
            Column {
                name: "id".to_string(),
                type_name: "INTEGER".to_string(),
                notnull: true,
                pk: true,
                dflt_value: None,
            },
            Column {
                name: "email".to_string(),
                type_name: "TEXT".to_string(),
                notnull: false,
                pk: false,
                dflt_value: None,
            },
        ];

        let diffs = compare_columns("test_table", &cols_a, &cols_b);
        assert!(!diffs.is_empty());

        // Should have column added (email) and column changed (id)
        let added_count = diffs.iter().filter(|d| d.diff_type == DiffType::ColumnAdded).count();
        let changed_count = diffs.iter().filter(|d| d.diff_type == DiffType::ColumnChanged).count();
        assert_eq!(added_count, 1);
        assert_eq!(changed_count, 1);
    }

    #[test]
    fn test_diff_schemas_identical() {
        let schema = r#"{"tables": [{"name": "users"}]}"#;
        let result = diff_schemas(schema, schema);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "No differences found.");
    }

    #[test]
    fn test_diff_schemas_different() {
        let schema_a = r#"{"tables": [{"name": "users"}]}"#;
        let schema_b = r#"{"tables": [{"name": "orders"}]}"#;
        let result = diff_schemas(schema_a, schema_b);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Differences found between schemas.");
    }

    #[test]
    fn test_diff_schemas_empty() {
        let result = diff_schemas("", r#"{"tables": [{"name": "users"}]}"#);
        assert!(result.is_err());

        // Verify the error type and message content
        if let Err(TuiqlError::Schema(msg)) = result {
            assert!(msg.contains("One or both schema inputs are empty"));
            // Enhanced error message validation
            println!("Diff error validation: Valid schema error detected with message: {}", msg);
        } else {
            panic!("Expected Schema error for empty schemas, got: {:?}", result);
        }
    }

    #[test]
    fn test_table_diffs_with_same_schema() {
        let db1 = create_test_db(setup_test_schema_a);
        let conn1 = Connection::open(db1.path()).unwrap();
        let schema1 = crate::core::db::schema::Schema::from_connection(&conn1).unwrap();
        let schema2 = schema1.clone();

        let result = compare_schemas(&schema1, &schema2);
        assert!(result.is_ok());
        let comparison = result.unwrap();
        assert!(comparison.added_tables.is_empty());
        assert!(comparison.removed_tables.is_empty());
        assert!(comparison.changed_tables.is_empty());
    }

    #[test]
    fn test_table_diffs_with_different_schemas() {
        let db1 = create_test_db(setup_test_schema_a);
        let db2 = create_test_db(setup_test_schema_b);

        let conn1 = Connection::open(db1.path()).unwrap();
        let conn2 = Connection::open(db2.path()).unwrap();

        let schema1 = crate::core::db::schema::Schema::from_connection(&conn1).unwrap();
        let schema2 = crate::core::db::schema::Schema::from_connection(&conn2).unwrap();

        let result = compare_schemas(&schema1, &schema2);
        assert!(result.is_ok());
        let comparison = result.unwrap();

        // schema_a has: users, posts
        // schema_b has: users, orders
        assert_eq!(comparison.added_tables, vec!["orders"]);
        assert_eq!(comparison.removed_tables, vec!["posts"]);
        assert!(comparison.changed_tables.contains(&"users".to_string()));
    }
}
