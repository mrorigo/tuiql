// Record Inspector Stub Module for TUIQL
//
// This module provides a stub implementation for viewing and editing a single record.
// In a complete implementation, this module would support inspecting a record in various
// views (e.g., key-value, JSON, tabular) and allow editing fields with validation.
//
// Currently, a record is represented as a simple key-value map (using Vec<(String, String)>),
// and basic functions are provided to view and edit the record.

use regex::Regex;
use std::collections::HashMap;
use crate::json_viewer::JsonTreeViewer;

#[derive(Debug, Clone, PartialEq)]
pub struct Record {
    // A record is a mapping from column names to string values.
    pub fields: HashMap<String, String>,
}

impl Record {
    /// Creates a new, empty record.
    pub fn new() -> Self {
        Record {
            fields: HashMap::new(),
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
    json_viewers: HashMap<String, JsonTreeViewer>, // JSON viewers for JSON fields
}

impl RecordInspector {
    /// Creates a new RecordInspector with an empty record.
    pub fn new() -> Self {
        RecordInspector {
            record: Record::new(),
            json_viewers: HashMap::new(),
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
    pub fn edit_field(&mut self, key: &str, new_value: &str) -> Result<bool, String> {
        if self.record.fields.contains_key(key) {
            if !self.validate_field(key, new_value) {
                return Err(format!("Validation failed for field '{}'", key));
            }
            self.record
                .fields
                .insert(key.to_string(), new_value.to_string());
            Ok(true)
        } else {
            Err(format!("Validation failed for field '{}'", key))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            json_viewers: HashMap::new(),
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
            json_viewers: HashMap::new(),
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
            json_viewers: HashMap::new(),
        };
        let result = inspector.edit_field("email", "invalid-email");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Validation failed for field 'email'");

        let result = inspector.edit_field("nonexistent", "value");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err(),
            "Validation failed for field 'nonexistent'"
        );
    }

    #[test]
    fn benchmark_record_inspector_large_record() {
        let mut record = Record::new();
        for i in 0..1000 {
            record.set_field(&format!("field_{}", i), &format!("value_{}", i));
        }
        let inspector = RecordInspector {
            record,
            json_viewers: HashMap::new(),
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
        assert!(view.contains("data: {\"invalid\": json}"));
        assert!(view.contains("name: Bob"));
    }
}
