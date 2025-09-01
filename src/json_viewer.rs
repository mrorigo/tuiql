/// JSON Tree Viewer Module for TUIQL
///
/// This module provides functionality to parse and display JSON data in a structured
/// tree format within the terminal. It supports nested objects and arrays, and provides
/// collapsible nodes for better navigation of complex JSON structures.
///
/// Key features:
/// - Parse JSON strings into tree structures
/// - Render tree with collapsible/expandable nodes
/// - Handle different JSON value types (objects, arrays, primitives)
/// - Integration with TUI for keyboard navigation
use crate::core::{Result, TuiqlError};

/// Represents different types of JSON values for display purposes.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValueType {
    Object,
    Array,
    String,
    Number,
    Boolean,
    Null,
}

/// A node in the JSON tree structure.
#[derive(Debug, Clone)]
pub struct JsonNode {
    pub key: String,             // The key (for objects) or index (for arrays)
    pub value: JsonValue,        // The actual value
    pub children: Vec<JsonNode>, // Child nodes (empty for primitives)
    pub expanded: bool,          // Whether this node is expanded (for collapsible nodes)
    pub depth: usize,            // How deep in the tree this node is
}

/// The different types of values a JSON node can hold.
#[derive(Debug, Clone)]
pub enum JsonValue {
    Object(serde_json::Map<String, serde_json::Value>),
    Array(Vec<serde_json::Value>),
    String(String),
    Number(serde_json::Number),
    Bool(bool),
    Null,
}

impl JsonValue {
    /// Get the type of this JSON value.
    pub fn value_type(&self) -> JsonValueType {
        match self {
            JsonValue::Object(_) => JsonValueType::Object,
            JsonValue::Array(_) => JsonValueType::Array,
            JsonValue::String(_) => JsonValueType::String,
            JsonValue::Number(_) => JsonValueType::Number,
            JsonValue::Bool(_) => JsonValueType::Boolean,
            JsonValue::Null => JsonValueType::Null,
        }
    }

    /// Get a display string for this value.
    pub fn display_value(&self) -> String {
        match self {
            JsonValue::Object(map) => format!("{{{} object}}", map.len()),
            JsonValue::Array(arr) => format!("[{} items]", arr.len()),
            JsonValue::String(s) => format!("\"{}\"", s),
            JsonValue::Number(n) => n.to_string(),
            JsonValue::Bool(b) => b.to_string(),
            JsonValue::Null => "null".to_string(),
        }
    }

    /// Check if this value has children (i.e., is collapsible).
    pub fn has_children(&self) -> bool {
        match self {
            JsonValue::Object(map) => !map.is_empty(),
            JsonValue::Array(arr) => !arr.is_empty(),
            _ => false,
        }
    }
}

/// The main JSON tree viewer component.
pub struct JsonTreeViewer {
    pub root: Option<JsonNode>,
    pub focused_path: Vec<String>, // Path to the currently focused node
}

impl Default for JsonTreeViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonTreeViewer {
    /// Create a new empty JSON tree viewer.
    pub fn new() -> Self {
        JsonTreeViewer {
            root: None,
            focused_path: Vec::new(),
        }
    }

    /// Load JSON data from a string.
    pub fn load_json(&mut self, json_str: &str) -> Result<()> {
        let parsed: serde_json::Value = serde_json::from_str(json_str).map_err(TuiqlError::Json)?;

        self.root = Some(self.build_tree_from_value("root".to_string(), parsed, 0));
        self.focused_path.clear();
        Ok(())
    }

    /// Build a tree structure from a serde_json Value.
    #[allow(clippy::only_used_in_recursion)]
    fn build_tree_from_value(
        &self,
        key: String,
        value: serde_json::Value,
        depth: usize,
    ) -> JsonNode {
        let (json_value, children) = match &value {
            serde_json::Value::Object(map) => {
                let obj_map = map.clone();
                let mut child_nodes = Vec::new();
                for (k, v) in &obj_map {
                    child_nodes.push(self.build_tree_from_value(k.clone(), v.clone(), depth + 1));
                }
                (JsonValue::Object(obj_map), child_nodes)
            }
            serde_json::Value::Array(arr) => {
                let arr_vec = arr.clone();
                let mut child_nodes = Vec::new();
                for (i, v) in arr_vec.iter().enumerate() {
                    child_nodes.push(self.build_tree_from_value(
                        i.to_string(),
                        v.clone(),
                        depth + 1,
                    ));
                }
                (JsonValue::Array(arr_vec), child_nodes)
            }
            serde_json::Value::String(s) => (JsonValue::String(s.clone()), Vec::new()),
            serde_json::Value::Number(n) => (JsonValue::Number(n.clone()), Vec::new()),
            serde_json::Value::Bool(b) => (JsonValue::Bool(*b), Vec::new()),
            serde_json::Value::Null => (JsonValue::Null, Vec::new()),
        };

        JsonNode {
            key,
            value: json_value,
            children,
            expanded: depth < 2, // Auto-expand first two levels
            depth,
        }
    }

    /// Render the JSON tree as a formatted string.
    pub fn render(&self) -> String {
        let mut output = String::new();
        if let Some(ref root) = self.root {
            self.render_node(&mut output, root, &Vec::new());
        } else {
            output.push_str("No JSON data loaded");
        }
        output
    }

    /// Render a single node and its children.
    fn render_node(&self, output: &mut String, node: &JsonNode, path: &[String]) {
        // Add indentation based on depth
        let indent = "  ".repeat(node.depth);
        let prefix = self.get_node_prefix(node);

        // Add focus indicator if this node is focused
        let focus_marker = if path == self.focused_path.as_slice() {
            "▶ "
        } else {
            "  "
        };

        // Format the node line
        output.push_str(&format!(
            "{}{}{} {}: {}\n",
            indent,
            focus_marker,
            prefix,
            node.key,
            node.value.display_value()
        ));

        // Render children if expanded
        if node.expanded && node.value.has_children() {
            for child in &node.children {
                let mut child_path = path.to_vec();
                child_path.push(child.key.clone());
                self.render_node(output, child, &child_path);
            }
        }
    }

    /// Get the appropriate prefix icon for a node type.
    fn get_node_prefix(&self, node: &JsonNode) -> &'static str {
        if node.value.has_children() {
            if node.expanded {
                "📂"
            } else {
                "📁"
            }
        } else {
            match node.value.value_type() {
                JsonValueType::String => "\"",
                JsonValueType::Number => "#",
                JsonValueType::Boolean => "✓",
                JsonValueType::Null => "∅",
                JsonValueType::Object | JsonValueType::Array => "📄", // Shouldn't happen for primitive types
            }
        }
    }

    /// Toggle the expanded state of a node at the given path.
    pub fn toggle_expanded(&mut self, path: &[String]) {
        if let Some(ref mut root) = self.root {
            toggle_node_expanded(root, path);
        }
    }
    /// Set the focused node path.
    pub fn set_focus(&mut self, path: Vec<String>) {
        self.focused_path = path;
    }

    /// Get a flat list of all expandable nodes for navigation.
    pub fn get_expandable_nodes(&self) -> Vec<Vec<String>> {
        let mut nodes = Vec::new();
        if let Some(ref root) = self.root {
            self.collect_expandable_nodes(root, &Vec::new(), &mut nodes);
        }
        nodes
    }

    /// Extract a sub-value from the JSON tree at the given path.
    pub fn extract_value(&self, path: &[String]) -> Option<&JsonValue> {
        if let Some(ref root) = self.root {
            return self.find_value_at_path(root, path);
        }
        None
    }

    /// Recursively collect all expandable nodes.
    #[allow(clippy::only_used_in_recursion)]
    fn collect_expandable_nodes(
        &self,
        node: &JsonNode,
        current_path: &[String],
        nodes: &mut Vec<Vec<String>>,
    ) {
        if node.value.has_children() {
            let mut path = current_path.to_vec();
            path.push(node.key.clone());
            nodes.push(path.clone());

            if node.expanded {
                for child in &node.children {
                    self.collect_expandable_nodes(child, &path, nodes);
                }
            }
        }
    }

    /// Recursively find a value at the given path.
    #[allow(clippy::only_used_in_recursion)]
    fn find_value_at_path<'a>(
        &self,
        node: &'a JsonNode,
        target_path: &[String],
    ) -> Option<&'a JsonValue> {
        if target_path.is_empty() {
            return Some(&node.value);
        }

        let current_key = &target_path[0];
        let remaining_path = &target_path[1..];

        for child in &node.children {
            if &child.key == current_key {
                return self.find_value_at_path(child, remaining_path);
            }
        }
        None
    }
}

/// Recursively find and toggle a node at the given path.
/// This is a standalone function to avoid borrowing issues within the impl.
fn toggle_node_expanded(node: &mut JsonNode, target_path: &[String]) {
    if target_path.is_empty() {
        if node.value.has_children() {
            node.expanded = !node.expanded;
        }
        return;
    }

    let current_key = &target_path[0];
    let remaining_path = &target_path[1..];

    for child in &mut node.children {
        if &child.key == current_key {
            toggle_node_expanded(child, remaining_path);
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_snapshot;

    #[test]
    fn test_load_simple_json() {
        let mut viewer = JsonTreeViewer::new();
        let json = r#"{"name": "Alice", "age": 30, "active": true}"#;

        assert!(viewer.load_json(json).is_ok());
        assert!(viewer.root.is_some());

        let rendered = viewer.render();
        assert!(rendered.contains("name"));
        assert!(rendered.contains("Alice"));
        assert!(rendered.contains("age"));
        assert!(rendered.contains("30"));
        assert!(rendered.contains("active"));
        assert!(rendered.contains("true"));
    }

    #[test]
    fn test_load_nested_json() {
        let mut viewer = JsonTreeViewer::new();
        let json = r#"{
            "user": {
                "name": "Bob",
                "profile": {
                    "hobbies": ["reading", "coding"],
                    "address": {
                        "city": "NYC",
                        "zip": "10001"
                    }
                }
            },
            "tags": ["developer", "rust"]
        }"#;

        assert!(viewer.load_json(json).is_ok());
        let rendered = viewer.render();
        println!("Rendered output:\n{}", rendered); // Debug output
        assert!(rendered.contains("user"));
        // User object has name, profile - so {2 object}
        assert!(rendered.contains("{2 object}"));
        assert!(rendered.contains("tags"));
        assert!(rendered.contains("[2 items]"));
    }

    #[test]
    fn test_toggle_expanded() {
        let mut viewer = JsonTreeViewer::new();
        let json = r#"{"root": {"child": "value"}}"#;

        viewer.load_json(json).unwrap();
        let expandable = viewer.get_expandable_nodes();

        // Should have at least the root object
        assert!(!expandable.is_empty());

        // Find the path to root object and toggle it
        let root_path = expandable
            .into_iter()
            .find(|path| path.len() == 1 && path[0] == "root");
        if let Some(path) = root_path {
            viewer.toggle_expanded(&path);
            // Note: In a real TUI implementation, we would re-render to verify the change
        }
    }

    #[test]
    fn test_invalid_json() {
        let mut viewer = JsonTreeViewer::new();
        let invalid_json = r#"{"invalid": json}"#;

        assert!(viewer.load_json(invalid_json).is_err());
        assert!(viewer.root.is_none());
    }

    #[test]
    fn test_empty_viewer() {
        let viewer = JsonTreeViewer::new();
        let rendered = viewer.render();
        assert_eq!(rendered, "No JSON data loaded");
    }

    #[test]
    fn test_extract_value() {
        let mut viewer = JsonTreeViewer::new();
        let json = r#"{"users": [{"name": "Alice"}]}"#;

        viewer.load_json(json).unwrap();

        // Extract the users array
        let users_value = viewer.extract_value(&["users".to_string()]);
        assert!(users_value.is_some());

        if let Some(JsonValue::Array(arr)) = users_value {
            assert_eq!(arr.len(), 1);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_render_golden_simple_object() {
        let mut viewer = JsonTreeViewer::new();
        viewer
            .load_json(r#"{"name": "Alice", "age": 30, "active": true, "score": null}"#)
            .unwrap();
        assert_snapshot!("json_viewer_simple_object", viewer.render());
    }

    #[test]
    fn test_render_golden_nested_structure() {
        let mut viewer = JsonTreeViewer::new();
        viewer
            .load_json(
                r#"{
            "company": {
                "name": "TechCorp",
                "departments": [
                    {"name": "Engineering", "employees": 25},
                    {"name": "Sales", "employees": 15}
                ],
                "location": {"city": "San Francisco", "state": "CA"}
            },
            "tags": ["startup", "tech", "innovative"]
        }"#,
            )
            .unwrap();
        assert_snapshot!("json_viewer_nested_structure", viewer.render());
    }

    #[test]
    fn test_render_golden_array() {
        let mut viewer = JsonTreeViewer::new();
        viewer
            .load_json(
                r#"[
            {"id": 1, "product": "Laptop", "price": 999.99},
            {"id": 2, "product": "Mouse", "price": 29.99},
            {"id": 3, "product": "Keyboard", "price": 79.99}
        ]"#,
            )
            .unwrap();
        assert_snapshot!("json_viewer_array", viewer.render());
    }

    #[test]
    fn test_render_golden_empty_viewer() {
        let viewer = JsonTreeViewer::new();
        assert_snapshot!("json_viewer_empty", viewer.render());
    }

    #[test]
    fn test_render_golden_with_focus() {
        let mut viewer = JsonTreeViewer::new();
        viewer
            .load_json(r#"{"root": {"child1": "value1", "child2": {"grandchild": "value2"}}}"#)
            .unwrap();
        viewer.set_focus(vec![
            "root".to_string(),
            "child2".to_string(),
            "grandchild".to_string(),
        ]);
        assert_snapshot!("json_viewer_with_focus", viewer.render());
    }
}
