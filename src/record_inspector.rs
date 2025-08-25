// Record Inspector Stub Module for TUIQL
//
// This module provides a stub implementation for viewing and editing a single record.
// In a complete implementation, this module would support inspecting a record in various
// views (e.g., key-value, JSON, tabular) and allow editing fields with validation.
//
// Currently, a record is represented as a simple key-value map (using Vec<(String, String)>),
// and basic functions are provided to view and edit the record.

use std::collections::HashMap;

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
pub struct RecordInspector {
    pub record: Record,
}

impl RecordInspector {
    /// Creates a new RecordInspector with an empty record.
    pub fn new() -> Self {
        RecordInspector {
            record: Record::new(),
        }
    }

    /// Loads a record into the inspector.
    pub fn load_record(&mut self, record: Record) {
        self.record = record;
    }

    /// Returns a string representation of the record in a key-value list format.
    pub fn view_record(&self) -> String {
        let mut output = String::new();
        for (key, value) in &self.record.fields {
            output.push_str(&format!("{}: {}\n", key, value));
        }
        output
    }

    /// Edits a field in the record. Returns true if the field was successfully updated,
    /// or false if the field did not exist.
    pub fn edit_field(&mut self, key: &str, new_value: &str) -> bool {
        if self.record.fields.contains_key(key) {
            self.record
                .fields
                .insert(key.to_string(), new_value.to_string());
            true
        } else {
            false
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
        let inspector = RecordInspector { record };
        let view = inspector.view_record();
        // The output should contain both keys and their corresponding values.
        assert!(view.contains("id: 1"));
        assert!(view.contains("name: Bob"));
    }

    #[test]
    fn test_edit_field_existing() {
        let mut record = Record::new();
        record.set_field("name", "Charlie");
        let mut inspector = RecordInspector { record };
        let result = inspector.edit_field("name", "Charles");
        assert!(result);
        assert_eq!(
            inspector.record.get_field("name"),
            Some(&"Charles".to_string())
        );
    }

    #[test]
    fn test_edit_field_non_existing() {
        let record = Record::new();
        let mut inspector = RecordInspector { record };
        let result = inspector.edit_field("email", "charles@example.com");
        assert!(!result);
    }
}
