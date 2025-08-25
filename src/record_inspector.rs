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
        let inspector = RecordInspector { record };
        let view = inspector.view_record();
        // The output should contain both keys and their corresponding values.
        assert!(view.contains("id: 1"));
        assert!(view.contains("name: Bob"));
    }

    #[test]
    fn test_edit_field_existing_with_validation() {
        let mut record = Record::new();
        record.set_field("name", "Charlie");
        let mut inspector = RecordInspector { record };
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
        let mut inspector = RecordInspector { record };
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
}
