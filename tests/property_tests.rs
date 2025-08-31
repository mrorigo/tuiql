//! Property-based tests for DDL operations and schema comparison
//!
//! These tests verify the correctness of schema diff operations through
//! property-based testing, ensuring that:
//! - Schema diff generation is deterministic
//! - Round-trip DDL operations are consistent
//! - Edge cases are properly handled

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rusqlite::Connection;
    use tempfile::NamedTempFile;
    use std::collections::HashSet;
    use std::sync::{Arc, Mutex};

    // Import our schema diff functions (and any other necessary items)
    use tuiql::diff::{compare_databases, compare_schemas, format_comparison};
    use tuiql::core::db::schema::{Schema, Table, Column, Index, ForeignKey};
    use tuiql::core::{Result as TuiqlResult, TuiqlError};

    // Test infrastructure

    /// Creates a temporary SQLite database for testing
    fn create_temp_db() -> NamedTempFile {
        let temp_file = NamedTempFile::new().unwrap();
        // Create an empty database
        Connection::open(&temp_file).unwrap();
        temp_file
    }

    /// Generate random column properties for property-based testing
    fn arb_column_name() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9_]{0,29}".prop_map(|s: String| s)
    }

    fn arb_column_type() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("INTEGER".to_string()),
            Just("TEXT".to_string()),
            Just("REAL".to_string()),
            Just("BLOB".to_string()),
            Just("NUMERIC".to_string()),
            Just("BOOLEAN".to_string()),
            Just("DATE".to_string()),
            Just("DATETIME".to_string())
        ]
    }

    fn arb_column() -> impl Strategy<Value = Column> {
        (arb_column_name(), arb_column_type(), any::<bool>(), any::<bool>(),
         prop_oneof![
             Just(None),
             Just(Some("NULL".to_string())),
             Just(Some("CURRENT_TIME".to_string())),
             Just(Some("1".to_string())),
             Just(Some("'default'".to_string()))
         ]).prop_map(|(name, type_name, notnull, pk, dflt_value)| {
            Column {
                name,
                type_name,
                notnull,
                pk,
                dflt_value,
            }
        })
    }

    fn arb_table_name() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9_]{0,29}".prop_map(|s: String| s)
    }

    fn arb_table() -> impl Strategy<Value = Table> {
        (arb_table_name(), (1usize..=10usize)).prop_flat_map(|(name, column_count)| {
            prop::collection::vec(arb_column(), column_count).prop_map(move |columns| {
                Table {
                    name: name.clone(),
                    columns,
                    indexes: Vec::new(), // For simplicity, start without indexes
                    foreign_keys: Vec::new(), // For simplicity, start without FKs
                }
            })
        })
    }

    fn arb_schema() -> impl Strategy<Value = Schema> {
        (1usize..=5usize).prop_flat_map(|table_count| {
            prop::collection::vec(arb_table(), table_count).prop_map(|tables| {
                let table_map = tables.into_iter()
                    .map(|t| (t.name.clone(), t))
                    .collect();

                Schema { tables: table_map }
            })
        })
    }

    // Property tests

    proptest! {
        /// Test that comparing identical schemas always yields no differences
        #[test]
        fn prop_identical_schemas_have_no_diffs(schema in arb_schema()) {
            let result = compare_schemas(&schema, &schema).unwrap();

            prop_assert!(result.added_tables.is_empty(),
                        "Identical schemas should have no added tables");
            prop_assert!(result.removed_tables.is_empty(),
                        "Identical schemas should have no removed tables");
            prop_assert!(result.changed_tables.is_empty(),
                        "Identical schemas should have no changed tables");
            prop_assert!(result.detailed_diffs.is_empty(),
                        "Identical schemas should have no detailed diffs");
        }

        /// Test that schema comparison is commutative for identical changes
        /// (diff(A, B) should be inverse of diff(B, A))
        #[test]
        fn prop_schema_diff_is_commutative(schema_a in arb_schema(), schema_b in arb_schema()) {
            let diff_ab = compare_schemas(&schema_a, &schema_b).unwrap();
            let diff_ba = compare_schemas(&schema_b, &schema_a).unwrap();

            // The differences should be inverses of each other
            prop_assert_eq!(diff_ab.added_tables.len(), diff_ba.removed_tables.len(),
                          "Added tables in A->B should equal removed tables in B->A");
            prop_assert_eq!(diff_ab.removed_tables.len(), diff_ba.added_tables.len(),
                          "Removed tables in A->B should equal added tables in B->A");

            // Check that each added table in A->B is a removed table in B->A
            let added_ab: HashSet<_> = diff_ab.added_tables.iter().collect();
            let removed_ba: HashSet<_> = diff_ba.removed_tables.iter().collect();
            prop_assert_eq!(added_ab, removed_ba,
                          "Added tables A->B should equal removed tables B->A");

            let removed_ab: HashSet<_> = diff_ab.removed_tables.iter().collect();
            let added_ba: HashSet<_> = diff_ba.added_tables.iter().collect();
            prop_assert_eq!(removed_ab, added_ba,
                          "Removed tables A->B should equal added tables B->A");
        }

        /// Test that schema formatting handles various edge cases without panicking
        #[test]
        fn prop_schema_formatting_robustness(schema_a in arb_schema(), schema_b in arb_schema()) {
            let comparison = compare_schemas(&schema_a, &schema_b).unwrap();

            // This should not panic regardless of the input
            let formatted = format_comparison(&comparison, "test_a.db", "test_b.db");

            // Basic sanity checks on output format
            prop_assert!(formatted.contains("Schema comparison between"),
                        "Output should contain comparison header");
            prop_assert!(formatted.contains("test_a.db"),
                        "Output should contain first database name");
            prop_assert!(formatted.contains("test_b.db"),
                        "Output should contain second database name");
            prop_assert!(formatted.contains("=") || formatted.is_empty(),
                        "Output should contain separators or be empty for no changes");
        }

        /// Test that schema changes are consistently reported across different operations
        #[test]
        fn prop_consistent_change_reporting(schema_a in arb_schema(), schema_b in arb_schema()) {
            let comparison = compare_schemas(&schema_a, &schema_b).unwrap();

            // If there are no changes, all collections should be empty
            let has_changes = !comparison.added_tables.is_empty() ||
                            !comparison.removed_tables.is_empty() ||
                            !comparison.changed_tables.is_empty() ||
                            !comparison.detailed_diffs.is_empty();

            // If we claim to have changes, there should be details about them
            if has_changes {
                // Either we have changed tables or detailed diffs should exist
                prop_assert!(!comparison.changed_tables.is_empty() ||
                           !comparison.detailed_diffs.is_empty(),
                           "If changes exist, there should be changed tables or detailed diffs");

                // The detailed diffs should match what's reported in summary
                let table_diffs_from_summary =
                    comparison.added_tables.len() +
                    comparison.removed_tables.len() +
                    comparison.changed_tables.len();

                // Should be at least as many detailed diffs as table-level differences
                // (since each table change has at least one detailed diff)
                prop_assert!(comparison.detailed_diffs.len() >= table_diffs_from_summary,
                           "There should be at least one detailed diff per changed table");
            }
        }
    }

    // Additional validation tests

    /// Test edge cases with minimal schemas
    #[test]
    fn test_minimal_schema_comparison() {
        // Empty schemas
        let empty_schema = Schema { tables: std::collections::HashMap::new() };
        let result = compare_schemas(&empty_schema, &empty_schema).unwrap();
        assert!(result.added_tables.is_empty());
        assert!(result.removed_tables.is_empty());

        // Schema with single empty table
        let mut single_table = std::collections::HashMap::new();
        single_table.insert("test".to_string(), Table {
            name: "test".to_string(),
            columns: Vec::new(),
            indexes: Vec::new(),
            foreign_keys: Vec::new(),
        });
        let minimal_schema = Schema { tables: single_table };

        let result = compare_schemas(&empty_schema, &minimal_schema).unwrap();
        assert_eq!(result.added_tables, vec!["test"]);
        assert!(result.detailed_diffs.len() >= 1);
    }

    /// Test that database comparison handles file system operations safely
    #[test]
    fn test_database_comparison_file_operations() {
        let db1 = create_temp_db();
        let db2 = create_temp_db();

        // This should succeed for valid database files
        let result = compare_databases(
            db1.path().to_str().unwrap(),
            db2.path().to_str().unwrap()
        );

        // Both files should be valid databases, so comparison should succeed
        assert!(result.is_ok());
    }

    /// Test schema comparison with real table data from existing test databases
    #[test]
    fn test_schema_comparison_with_real_data() {
        // Create databases with known differences
        let db1 = create_temp_db();
        let conn1 = Connection::open(&db1).unwrap();
        conn1.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL);
             CREATE TABLE posts (id INTEGER PRIMARY KEY, user_id INTEGER);"
        ).unwrap();

        let db2 = create_temp_db();
        let conn2 = Connection::open(&db2).unwrap();
        conn2.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT);
             CREATE TABLE comments (id INTEGER PRIMARY KEY, post_id INTEGER);"
        ).unwrap();

        let result = compare_databases(
            db1.path().to_str().unwrap(),
            db2.path().to_str().unwrap()
        ).unwrap();

        // Expected differences:
        // Added: comments table, email column to users
        // Removed: posts table
        assert!(result.added_tables.contains(&"comments".to_string()));
        assert!(result.removed_tables.contains(&"posts".to_string()));
        assert!(result.changed_tables.contains(&"users".to_string()));
    }

    /// Test round-trip DDL operations: generate diff, apply DDL, compare again
    #[test]
    fn test_ddl_round_trip_consistency() {
        let db_original = create_temp_db();
        let conn_original = Connection::open(&db_original).unwrap();

        // Create initial schema
        conn_original.execute_batch(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                age INTEGER
             );
             CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                title TEXT,
                user_id INTEGER REFERENCES users(id)
             );"
        ).unwrap();

        let db_modified = create_temp_db();
        let conn_modified = Connection::open(&db_modified).unwrap();

        // Apply modifications
        conn_modified.execute_batch(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                age INTEGER,
                email TEXT
             );
             CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                title TEXT,
                content TEXT,
                user_id INTEGER REFERENCES users(id)
             );
             CREATE TABLE tags (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE
             );"
        ).unwrap();

        // Compare original vs modified
        let original_path = db_original.path().to_str().unwrap();
        let modified_path = db_modified.path().to_str().unwrap();

        let first_comparison = compare_databases(original_path, modified_path).unwrap();
        assert!(!first_comparison.detailed_diffs.is_empty());

        // Now try to apply the changes to a third database
        let db_target = create_temp_db();
        let conn_target = Connection::open(&db_target).unwrap();

        // Copy original schema to target
        conn_target.execute_batch(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                age INTEGER
             );
             CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                title TEXT,
                user_id INTEGER REFERENCES users(id)
             );"
        ).unwrap();

        // Apply changes based on diff (this would normally be generated DDL)
        conn_target.execute("ALTER TABLE users ADD COLUMN email TEXT", []).unwrap();
        conn_target.execute("ALTER TABLE posts ADD COLUMN content TEXT", []).unwrap();
        conn_target.execute_batch(
            "CREATE TABLE tags (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE
             );"
        ).unwrap();

        // Compare original with target - should be different (modified)
        let second_comparison = compare_databases(original_path, db_target.path().to_str().unwrap()).unwrap();
        assert!(!second_comparison.detailed_diffs.is_empty());

        // Compare modified with target - should be the same
        let third_comparison = compare_databases(modified_path, db_target.path().to_str().unwrap()).unwrap();
        assert!(third_comparison.detailed_diffs.is_empty(), "After applying DDL changes, schemas should be identical");
    }

    /// Test diff operations with edge cases and boundary conditions
    #[test]
    fn test_diff_operations_edge_cases() {
        let db_empty = create_temp_db();
        let db_with_tables = create_temp_db();

        let conn_with_tables = Connection::open(&db_with_tables).unwrap();
        conn_with_tables.execute_batch(
            "CREATE TABLE test (
                id INTEGER PRIMARY KEY,
                value TEXT,
                flag BOOLEAN DEFAULT FALSE
             );
             CREATE TABLE another (
                pk TEXT PRIMARY KEY,
                data BLOB
             );"
        ).unwrap();

        // Test diff with empty vs populated database
        let comparison = compare_databases(
            db_empty.path().to_str().unwrap(),
            db_with_tables.path().to_str().unwrap()
        ).unwrap();

        assert_eq!(comparison.added_tables.len(), 2);
        assert!(comparison.added_tables.contains(&"test".to_string()));
        assert!(comparison.added_tables.contains(&"another".to_string()));

        // Test with self-comparison (should be empty)
        let self_comparison = compare_databases(
            db_empty.path().to_str().unwrap(),
            db_empty.path().to_str().unwrap()
        ).unwrap();
        assert!(self_comparison.detailed_diffs.is_empty());

        // Test with invalid file paths
        let invalid_result = compare_databases("/nonexistent/path1", "/nonexistent/path2");
        assert!(invalid_result.is_err());
    }

    /// Test property-based edge cases with schema differences
    proptest! {
        /// Test that the detailed diffs count is consistent with summary information
        #[test]
        fn prop_detailed_diffs_count_consistency(schema_a in arb_schema(), schema_b in arb_schema()) {
            let comparison = compare_schemas(&schema_a, &schema_b).unwrap();

            let expected_min_diffs = comparison.added_tables.len() +
                                   comparison.removed_tables.len() +
                                   comparison.changed_tables.len();

            prop_assert!(comparison.detailed_diffs.len() >= expected_min_diffs,
                        "Detailed diffs should be at least as many as changed table summaries. \
                         Expected at least {}, got {}. Added: {}, Removed: {}, Changed: {}",
                        expected_min_diffs, comparison.detailed_diffs.len(),
                        comparison.added_tables.len(), comparison.removed_tables.len(),
                        comparison.changed_tables.len());
        }

        /// Test that every changed table has at least one detailed diff entry
        #[test]
        fn prop_changed_tables_have_detailed_diffs(schema_a in arb_schema(), schema_b in arb_schema()) {
            let comparison = compare_schemas(&schema_a, &schema_b).unwrap();

            for changed_table in &comparison.changed_tables {
                let has_detailed_diff = comparison.detailed_diffs.iter()
                    .any(|diff| diff.table_name == *changed_table);

                prop_assert!(has_detailed_diff,
                           "Table '{}' is marked as changed but has no detailed diffs",
                           changed_table);
            }
        }

        /// Test that schema diff operations handle duplicate table names correctly
        #[test]
        fn prop_no_duplicate_table_names_in_diff(mut schema_a in arb_schema()) {
            // Modify schema_a to have duplicate logic (this tests internal consistency)
            schema_a.tables.clear();

            // Add two tables with different names
            let mut table1 = Table {
                name: "table1".to_string(),
                columns: vec![Column {
                    name: "id".to_string(),
                    type_name: "INTEGER".to_string(),
                    notnull: false,
                    pk: true,
                    dflt_value: None,
                }],
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            };

            let table2 = Table {
                name: "table2".to_string(),
                columns: vec![Column {
                    name: "id".to_string(),
                    type_name: "INTEGER".to_string(),
                    notnull: false,
                    pk: false,
                    dflt_value: None,
                }],
                indexes: Vec::new(),
                foreign_keys: Vec::new(),
            };

            schema_a.tables.insert("table1".to_string(), table1.clone());
            schema_a.tables.insert("table2".to_string(), table2);

            let comparison = compare_schemas(&schema_a, &schema_a).unwrap();

            // Identical schemas should have no differences
            prop_assert!(comparison.detailed_diffs.is_empty(),
                        "Identical schemas should have no diffs");

            // Check that compare_schemas handles the schemas correctly
            let mut schema_b = schema_a.clone();
            if let Some(tbl) = schema_b.tables.get_mut("table1") {
                tbl.columns[0].notnull = true; // Change NOT NULL constraint
            }

            let modified_comparison = compare_schemas(&schema_a, &schema_b).unwrap();
            prop_assert!(!modified_comparison.detailed_diffs.is_empty(),
                        "Modified schemas should show differences");
        }
    }
}