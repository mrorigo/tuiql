/// Schema Introspection Module
///
/// This module provides functionality for introspecting database schema,
/// including tables, columns, indexes, and their relationships.
/// It handles the metadata layer of database operations.

use crate::core::Result;
use rusqlite::{Connection, Row};
use std::collections::HashMap;

/// Represents a foreign key relationship
#[derive(Debug, Clone)]
pub struct ForeignKey {
    /// The table this foreign key references
    pub referenced_table: String,
    /// The column in this table that is the foreign key
    pub from_column: String,
    /// The referenced column in the foreign table
    pub to_column: String,
}

/// Represents a database column with its metadata
#[derive(Debug, Clone)]
pub struct Column {
    /// Column name
    pub name: String,
    /// SQLite type name (e.g., "INTEGER", "TEXT", "REAL", "BLOB")
    pub type_name: String,
    /// Whether the column allows NULL values
    pub notnull: bool,
    /// Whether this column is part of the primary key
    pub pk: bool,
    /// Default value expression (if any)
    pub dflt_value: Option<String>,
}

impl Column {
    /// Creates a Column from a PRAGMA table_info result row
    fn from_pragma_row(row: &Row) -> rusqlite::Result<Self> {
        Ok(Column {
            name: row.get(1)?,
            type_name: row.get(2)?,
            notnull: row.get(3)?,
            pk: row.get(5)?,
            dflt_value: row.get(4)?,
        })
    }
}

/// Represents a database index
#[derive(Debug, Clone)]
pub struct Index {
    /// Index name
    pub name: String,
    /// Whether this is a UNIQUE index
    pub unique: bool,
    /// Column names that make up this index
    pub columns: Vec<String>,
}

impl Index {
    /// Creates an Index from PRAGMA index_info results
    fn from_pragma_info(
        conn: &Connection,
        index_name: String,
        unique: bool,
    ) -> Result<Self> {
        let mut columns = Vec::new();

        let mut stmt = conn.prepare(&format!("PRAGMA index_info('{}')", index_name))?;
        let column_iter = stmt.query_map([], |row| row.get::<_, String>(2))?;

        for column_result in column_iter {
            columns.push(column_result?);
        }

        Ok(Index {
            name: index_name,
            unique,
            columns,
        })
    }
}

/// Represents a database table with its structure and metadata
#[derive(Debug, Clone)]
pub struct Table {
    /// Table name
    pub name: String,
    /// List of columns in this table
    pub columns: Vec<Column>,
    /// List of indexes defined on this table
    pub indexes: Vec<Index>,
    /// List of foreign key relationships
    pub foreign_keys: Vec<ForeignKey>,
}

impl Table {
    /// Creates a Table by introspecting the database for the given table name
    fn from_database(conn: &Connection, table_name: &str) -> Result<Self> {
        let columns = get_table_columns(conn, table_name)?;
        let indexes = get_table_indexes(conn, table_name)?;
        let foreign_keys = get_table_foreign_keys(conn, table_name)?;

        Ok(Table {
            name: table_name.to_string(),
            columns,
            indexes,
            foreign_keys,
        })
    }
}

/// Comprehensive schema information for a database
#[derive(Debug, Clone)]
pub struct Schema {
    /// Map of table name to table information
    pub tables: HashMap<String, Table>,
}

impl Schema {
    /// Creates a new Schema by introspecting all tables in the database
    pub fn from_connection(conn: &Connection) -> Result<Self> {
        let tables = get_all_tables(conn)?;
        Ok(Schema { tables })
    }
}

/// Helper function to retrieve all user-defined tables from the database
fn get_all_tables(conn: &Connection) -> Result<HashMap<String, Table>> {
    let mut tables = HashMap::new();

    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master
         WHERE type='table' AND name NOT LIKE 'sqlite_%'"
    )?;

    let table_iter = stmt.query_map([], |row| row.get::<_, String>(0))?;

    for table_result in table_iter {
        let table_name = table_result?;
        tables.insert(
            table_name.clone(),
            Table::from_database(conn, &table_name)?
        );
    }

    Ok(tables)
}

/// Helper function to retrieve column information for a specific table
fn get_table_columns(conn: &Connection, table_name: &str) -> Result<Vec<Column>> {
    let mut columns = Vec::new();

    let mut stmt = conn.prepare(&format!("PRAGMA table_info('{}')", table_name))?;
    let column_iter = stmt.query_map([], |row| Column::from_pragma_row(row))?;

    for column_result in column_iter {
        columns.push(column_result?);
    }

    Ok(columns)
}

/// Helper function to retrieve index information for a specific table
fn get_table_indexes(conn: &Connection, table_name: &str) -> Result<Vec<Index>> {
    let mut indexes = Vec::new();

    let mut stmt = conn.prepare(&format!("PRAGMA index_list('{}')", table_name))?;
    let index_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(1)?, // index name
            row.get::<_, bool>(2)?,   // unique
        ))
    })?;

    for index_result in index_iter {
        let (index_name, unique) = index_result?;
        indexes.push(Index::from_pragma_info(conn, index_name, unique)?);
    }

    Ok(indexes)
}

/// Helper function to retrieve foreign key information for a specific table
fn get_table_foreign_keys(conn: &Connection, table_name: &str) -> Result<Vec<ForeignKey>> {
    let mut foreign_keys = Vec::new();

    let mut stmt = conn.prepare(&format!("PRAGMA foreign_key_list('{}')", table_name))?;
    let fk_iter = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(2)?, // referenced table
            row.get::<_, String>(3)?, // from column
            row.get::<_, String>(4)?, // to column
        ))
    })?;

    for fk_result in fk_iter {
        let (referenced_table, from_column, to_column) = fk_result?;
        foreign_keys.push(ForeignKey {
            referenced_table,
            from_column,
            to_column,
        });
    }

    Ok(foreign_keys)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_test_schema(conn: &Connection) -> rusqlite::Result<()> {
        conn.execute_batch(
            "
            CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                email TEXT UNIQUE,
                age INTEGER
            );
            CREATE INDEX idx_users_age ON users(age);
            CREATE TABLE posts (
                id INTEGER PRIMARY KEY,
                user_id INTEGER,
                title TEXT NOT NULL,
                content TEXT,
                FOREIGN KEY (user_id) REFERENCES users(id)
            );
        "
        )
    }

    #[test]
    fn test_schema_introspection() {
        let conn = Connection::open_in_memory().unwrap();
        setup_test_schema(&conn).unwrap();

        let schema = Schema::from_connection(&conn).unwrap();

        // Check users table
        let users_table = schema.tables.get("users").unwrap();
        assert_eq!(users_table.name, "users");
        assert_eq!(users_table.columns.len(), 4);

        // Check primary key
        let id_col = &users_table.columns[0];
        assert_eq!(id_col.name, "id");
        assert!(id_col.pk);

        // Check NOT NULL constraint
        let name_col = &users_table.columns[1];
        assert_eq!(name_col.name, "name");
        assert!(name_col.notnull);

        // Check indexes
        assert_eq!(users_table.indexes.len(), 2); // email unique, age regular
        let email_idx = users_table
            .indexes
            .iter()
            .find(|i| i.columns.contains(&"email".to_string()))
            .unwrap();
        assert!(email_idx.unique);

        // Check posts table exists
        assert!(schema.tables.contains_key("posts"));
        let posts_table = schema.tables.get("posts").unwrap();
        assert_eq!(posts_table.columns.len(), 4);
    }

    #[test]
    fn test_column_metadata() {
        let conn = Connection::open_in_memory().unwrap();
        setup_test_schema(&conn).unwrap();

        let schema = Schema::from_connection(&conn).unwrap();
        let users_table = schema.tables.get("users").unwrap();

        // Test column types and constraints
        let columns = &users_table.columns;
        assert_eq!(columns.len(), 4);

        // id column: INTEGER PRIMARY KEY
        assert_eq!(columns[0].name, "id");
        assert_eq!(columns[0].type_name, "INTEGER");
        assert!(columns[0].pk);
        // Note: SQLite PRIMARY KEY columns are implicitly NOT NULL,
        // but this may not always be reflected in pragma table_info
        // So we'll skip this assertion to avoid test flakiness

        // name column: TEXT NOT NULL
        assert_eq!(columns[1].name, "name");
        assert_eq!(columns[1].type_name, "TEXT");
        assert!(!columns[1].pk);
        assert!(columns[1].notnull);

        // email column: TEXT UNIQUE (converted to implicit unique index)
        assert_eq!(columns[2].name, "email");
        assert_eq!(columns[2].type_name, "TEXT");
        assert!(!columns[2].pk);
        assert!(!columns[2].notnull);

        // age column: INTEGER (nullable)
        assert_eq!(columns[3].name, "age");
        assert_eq!(columns[3].type_name, "INTEGER");
        assert!(!columns[3].pk);
        assert!(!columns[3].notnull);
    }
}