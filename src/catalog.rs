pub struct TableInfo {
    pub name: String,
    pub columns: Vec<ColumnInfo>,
}

pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub primary_key: bool,
    pub default_value: Option<String>,
}

/// Simulates discovery of a database schema by returning dummy table information.
/// In a real implementation, this function would connect to a SQLite database,
/// query the sqlite_master table and PRAGMA table_info for each table.
pub fn discover_schema(db_path: &str) -> Result<Vec<TableInfo>, String> {
    if db_path.is_empty() {
        return Err("Empty db_path provided".to_string());
    }

    // Stub: Return dummy table information.
    let dummy_table = TableInfo {
        name: "dummy_table".to_string(),
        columns: vec![
            ColumnInfo {
                name: "id".to_string(),
                data_type: "INTEGER".to_string(),
                not_null: true,
                primary_key: true,
                default_value: None,
            },
            ColumnInfo {
                name: "name".to_string(),
                data_type: "TEXT".to_string(),
                not_null: false,
                primary_key: false,
                default_value: Some("unknown".to_string()),
            },
        ],
    };

    Ok(vec![dummy_table])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_schema_valid_db() {
        let result = discover_schema("dummy.db");
        assert!(result.is_ok());
        let tables = result.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "dummy_table");
        assert_eq!(tables[0].columns.len(), 2);
    }

    #[test]
    fn test_discover_schema_empty_db_path() {
        let result = discover_schema("");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Empty db_path provided");
    }
}
