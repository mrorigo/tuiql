// Record Inspector Stub Module for TUIQL
//
// This module provides a stub implementation for viewing and editing a single record.
// In a complete implementation, this module would support inspecting a record in various
// views (e.g., key-value, JSON, tabular) and allow editing fields with validation.
//
// Currently, a record is represented as a simple key-value map (using Vec<(String, String)>),
// and basic functions are provided to view and edit the record.

use crate::core::{Result, TuiqlError};
use regex::Regex;
use std::collections::BTreeMap;
use crate::json_viewer::JsonTreeViewer;

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    // A record is a mapping from column names to string values.
    pub fields: BTreeMap<String, String>,
}

impl Default for Record {
    fn default() -> Self {
        Self::new()
    }
}

impl Record {
    /// Creates a new, empty record.
    pub fn new() -> Self {
        Record {
            fields: BTreeMap::new(),
        }
    }

    /// Inserts or updates a field with the given key and value.
    pub fn set_field(&mut self, key: &str, value: &str) {
        self.fields.insert(key.to_string(), value.to_string());
    }

    /// Returns a reference to the value of the specified field, if it exists.
    pub fn get_field(&self, key: &str) -> Option<&String> {
        self.fields.get(key)
    }
}

/// The RecordInspector provides functionality to display and edit a single record.
/// It automatically detects JSON fields and provides tree view visualization for them.
pub struct RecordInspector {
    pub record: Record,
    json_viewers: BTreeMap<String, JsonTreeViewer>, // JSON viewers for JSON fields
}

impl Default for RecordInspector {
    fn default() -> Self {
        Self::new()
    }
}

impl RecordInspector {
    /// Creates a new RecordInspector with an empty record.
    pub fn new() -> Self {
        RecordInspector {
            record: Record::new(),
            json_viewers: BTreeMap::new(),
        }
    }

    /// Loads a record into the inspector and automatically detects JSON fields.
    pub fn load_record(&mut self, record: Record) {
        self.record = record;
        self.json_viewers.clear();
        self.detect_and_load_json_fields();
    }

    /// Detects JSON fields in the record and creates JSON viewers for them.
    fn detect_and_load_json_fields(&mut self) {
        for (key, value) in &self.record.fields {
            // Try to parse the value as JSON, but only accept objects or arrays
            if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(value) {
                match json_value {
                    serde_json::Value::Object(_) | serde_json::Value::Array(_) => {
                        // It's a JSON object or array - create a viewer
                        let mut viewer = JsonTreeViewer::new();
                        if viewer.load_json(value).is_ok() {
                            self.json_viewers.insert(key.clone(), viewer);
                        }
                    }
                    _ => {
                        // Primitive values (strings, numbers, booleans, null) are displayed normally
                    }
                }
            }
        }
    }

    /// Returns a string representation of the record in a key-value list format.
    /// JSON fields are displayed with tree visualization.
    pub fn view_record(&self) -> String {
        let mut output = String::new();
        for (key, value) in &self.record.fields {
            if let Some(json_viewer) = self.json_viewers.get(key) {
                // This is a JSON field - display with tree view
                output.push_str(&format!("{}: (JSON Tree)\n{}\n", key, json_viewer.render()));
            } else {
                // Regular field
                output.push_str(&format!("{}: {}\n", key, value));
            }
        }
        output
    }

    /// Validates a field's value based on its key.
    /// For example, an "email" field must contain a valid email address.
    pub fn validate_field(&self, key: &str, value: &str) -> bool {
        match key {
            "email" => {
                let email_regex = Regex::new(r"^[^\s@]+@[^\s@]+\.[^\s@]+$").unwrap();
                email_regex.is_match(value)
            }
            "age" => {
                let age_regex = Regex::new(r"^\\d+$").unwrap(); // Only digits allowed
                age_regex.is_match(value)
            }
            "date" => {
                let date_regex = Regex::new(r"^\\d{4}-\\d{2}-\\d{2}$").unwrap(); // Format: YYYY-MM-DD
                date_regex.is_match(value)
            }
            _ => true,
        }
    }

    /// Previews the record in a JSON-like format.
    pub fn preview_record(&self) -> String {
        let mut output = String::from("{\n");
        for (key, value) in &self.record.fields {
            output.push_str(&format!("  \"{}\": \"{}\",\n", key, value));
        }
        output.pop(); // Remove the last comma
        output.push_str("\n}");
        output
    }

    /// Edits a field in the record with validation. Returns true if the field was successfully updated,
    /// or false if the field did not exist or validation failed.
    pub fn edit_field(&mut self, key: &str, new_value: &str) -> Result<bool> {
        if self.record.fields.contains_key(key) {
            if !self.validate_field(key, new_value) {
                return Err(TuiqlError::Query(format!(
                    "Field '{}' validation failed - value '{}' does not meet requirements for this field type",
                    key, new_value
                )));
            }
            self.record
                .fields
                .insert(key.to_string(), new_value.to_string());
            Ok(true)
        } else {
            Err(TuiqlError::Query(format!(
                "Cannot edit field '{}' - field does not exist in record. Available fields: {}",
                key,
                self.record.fields.keys().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_record_set_and_get() {
        let mut record = Record::new();
        record.set_field("id", "1");
        record.set_field("name", "Alice");
        assert_eq!(record.get_field("id"), Some(&"1".to_string()));
        assert_eq!(record.get_field("name"), Some(&"Alice".to_string()));
        assert_eq!(record.get_field("email"), None);
    }

    #[test]
    fn test_view_record() {
        let mut record = Record::new();
        record.set_field("id", "1");
        record.set_field("name", "Bob");
        let inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };
        let view = inspector.view_record();
        // The output should contain both keys and their corresponding values.
        assert!(view.contains("id: 1"));
        assert!(view.contains("name: Bob"));
    }

    #[test]
    fn test_edit_field_existing_with_validation() {
        let mut record = Record::new();
        record.set_field("name", "Charlie");
        let mut inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };
        let result = inspector.edit_field("name", "Charles");
        assert!(result.unwrap());
        assert_eq!(
            inspector.record.get_field("name"),
            Some(&"Charles".to_string())
        );
    }

    #[test]
    fn test_edit_field_non_existing_or_invalid() {
        let record = Record::new();
        let mut inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };

        // Test invalid field validation - field doesn't exist so it should fail before validation
        let result = inspector.edit_field("email", "invalid-email");
        assert!(result.is_err());
        if let Err(TuiqlError::Query(msg)) = result {
            assert!(msg.contains("Cannot edit field 'email'"));
            assert!(msg.contains("field does not exist"));
        } else {
            panic!("Expected Query error for nonexistent field");
        }

        // Test nonexistent field
        let result = inspector.edit_field("nonexistent", "value");
        assert!(result.is_err());
        if let Err(TuiqlError::Query(msg)) = result {
            assert!(msg.contains("Cannot edit field 'nonexistent'"));
            assert!(msg.contains("field does not exist"));
        } else {
            panic!("Expected Query error for nonexistent field");
        }

        // Test validation failure for existing field
        inspector.record.set_field("email", "test@test.com");
        let result = inspector.edit_field("email", "invalid-email");
        assert!(result.is_err());
        if let Err(TuiqlError::Query(msg)) = result {
            assert!(msg.contains("email"));
            assert!(msg.contains("validation failed"));
        } else {
            panic!("Expected Query error for validation failure");
        }
    }

    #[test]
    fn benchmark_record_inspector_large_record() {
        let mut record = Record::new();
        for i in 0..1000 {
            record.set_field(&format!("field_{}", i), &format!("value_{}", i));
        }
        let inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };

        let start = std::time::Instant::now();
        let view = inspector.view_record();
        let duration = start.elapsed();

        assert!(!view.is_empty());
        println!("Rendering a record with 1000 fields took: {:?}", duration);
        assert!(duration.as_secs_f64() < 0.5, "Rendering took too long!");
    }

    #[test]
    fn test_json_field_display() {
        let mut record = Record::new();
        record.set_field("id", "1");
        record.set_field("name", "Alice");
        record.set_field("data", r#"{"age": 30, "city": "NYC"}"#);

        let mut inspector = RecordInspector::new();
        inspector.load_record(record);

        let view = inspector.view_record();
        assert!(view.contains("id: 1"));
        assert!(view.contains("name: Alice"));
        assert!(view.contains("data: (JSON Tree)"));
        assert!(view.contains("age"));
        assert!(view.contains("city"));
    }

    #[test]
    fn test_invalid_json_field_handling() {
        let mut record = Record::new();
        record.set_field("data", r#"{"invalid": json}"#); // Invalid JSON
        record.set_field("name", "Bob");

        let mut inspector = RecordInspector::new();
        inspector.load_record(record);

        let view = inspector.view_record();
        // Invalid JSON should be displayed as regular text
        assert!(view.contains(r#"data: {"invalid": json}"#));
        assert!(view.contains("name: Bob"));
    }

    #[test]
    fn test_view_record_golden_simple_record() {
        let mut record = Record::new();
        record.set_field("id", "1");
        record.set_field("name", "Alice Johnson");
        record.set_field("email", "alice@example.com");
        record.set_field("active", "true");

        let inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };

        assert_snapshot!("record_inspector_simple_record", inspector.view_record());
    }

    #[test]
    fn test_view_record_golden_with_json_field() {
        let mut record = Record::new();
        record.set_field("id", "1");
        record.set_field("name", "Bob Smith");
        record.set_field("profile", r#"{
            "age": 30,
            "interests": ["coding", "music"],
            "address": {
                "city": "San Francisco",
                "country": "USA"
            }
        }"#);

        let mut inspector = RecordInspector::new();
        inspector.load_record(record);

        assert_snapshot!("record_inspector_with_json_field", inspector.view_record());
    }

    #[test]
    fn test_view_record_golden_mixed_fields() {
        let mut record = Record::new();
        record.set_field("user_id", "123");
        record.set_field("username", "testuser");
        record.set_field("settings", r#"{"theme": "dark", "notifications": true}"#);
        record.set_field("tags", r#"["rust", "developer", "open-source"]"#);
        record.set_field("bio", "A passionate Rust developer");

        let mut inspector = RecordInspector::new();
        inspector.load_record(record);

        assert_snapshot!("record_inspector_mixed_fields", inspector.view_record());
    }

    #[test]
    fn test_view_record_golden_empty_record() {
        let record = Record::new();
        let inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };

        assert_snapshot!("record_inspector_empty_record", inspector.view_record());
    }

    #[test]
    fn test_preview_record_golden() {
        let mut record = Record::new();
        record.set_field("name", "Charlie");
        record.set_field("age", "28");
        record.set_field("city", "London");

        let inspector = RecordInspector {
            record,
            json_viewers: BTreeMap::new(),
        };

        assert_snapshot!("record_inspector_preview", inspector.preview_record());
    }
}
