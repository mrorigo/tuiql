use crate::core::{Result, TuiqlError};

/// Results Grid Module for TUIQL
///
/// This module provides an implementation for rendering tabular results in the terminal.
/// It includes features like virtualized rendering for large datasets, sticky headers, and export functionality.

use std::collections::BTreeMap;

/// Represents a single cell in the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub content: String,
    pub cell_type: String,
}

/// Represents a row of cells in the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub cells: Vec<Cell>,
    pub row_index: usize,
}

/// Represents the viewport for virtualized scrolling.
#[derive(Debug, Clone)]
pub struct Viewport {
    pub start: usize,
    pub end: usize,
}

impl Viewport {
    pub fn new(start: usize, end: usize) -> Self {
        Viewport { start, end }
    }

    pub fn visible_rows<'a>(&self, rows: &'a [Row]) -> &'a [Row] {
        let start = self.start.min(rows.len());
        let end = self.end.min(rows.len());
        &rows[start..end]
    }

    pub fn scroll_down(&mut self, total_rows: usize) {
        if self.end < total_rows {
            self.start += 1;
            self.end += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.start > 0 {
            self.start -= 1;
            self.end -= 1;
        }
    }
}

/// Represents the entire grid structure.
#[derive(Debug, Clone)]
pub struct ResultsGrid {
    pub headers: Vec<String>,
    pub rows: Vec<Row>,
    pub viewport: Viewport,
}

impl ResultsGrid {
    /// Creates a new, empty ResultsGrid.
    pub fn new() -> Self {
        ResultsGrid {
            headers: Vec::new(),
            rows: Vec::new(),
            viewport: Viewport::new(0, 10), // Default viewport with 10 rows
        }
    }

    /// Sets the headers for the grid.
    pub fn set_headers(&mut self, headers: Vec<String>) {
        self.headers = headers;
    }

    /// Adds a row to the grid. Each row is represented as a vector of strings.
    pub fn add_row(&mut self, row: Vec<String>) {
        let cells = row
            .into_iter()
            .map(|s| Cell {
                content: s,
                cell_type: "text".to_string(),
            })
            .collect();
        self.rows.push(Row {
            cells,
            row_index: self.rows.len(),
        });
    }

    /// Renders the grid as a simple string with headers and rows.
    pub fn render(&self) -> String {
        let mut output = String::new();
        // Render headers if available
        if !self.headers.is_empty() {
            output.push_str(&self.headers.join(" | "));
            output.push('\n');
            let underline: Vec<String> = self
                .headers
                .iter()
                .map(|h| "-".repeat(h.len() + 2))
                .collect();
            output.push_str(&underline.join("-|-"));
            output.push('\n');
        }
        // Render rows
        for row in self.viewport.visible_rows(&self.rows) {
            let row_content: Vec<String> =
                row.cells.iter().map(|cell| cell.content.clone()).collect();
            output.push_str(&row_content.join(" | "));
            output.push('\n');
        }
        output
    }

    /// Exports the grid data to a specified format.
    /// Supported formats: CSV, JSON, Markdown.
    pub fn export(&self, format: &str) -> Result<String> {
        match format.to_lowercase().as_str() {
            "csv" => self.export_to_csv(),
            "json" => self.export_to_json(),
            "markdown" => self.export_to_markdown(),
            _ => Err(TuiqlError::Ui(format!(
                "Unsupported export format: '{}'. Supported formats: csv, json, markdown",
                format
            ))),
        }
    }

    fn export_to_csv(&self) -> Result<String> {
        let mut output = String::new();
        if !self.headers.is_empty() {
            output.push_str(&self.headers.join(","));
            output.push('\n');
        }
        for row in &self.rows {
            let row_content: Vec<String> =
                row.cells.iter().map(|cell| cell.content.clone()).collect();
            output.push_str(&row_content.join(","));
            output.push('\n');
        }
        Ok(output)
    }

    fn export_to_json(&self) -> Result<String> {
        let mut rows = Vec::new();
        for row in &self.rows {
            let mut row_map = BTreeMap::new();
            for (i, cell) in row.cells.iter().enumerate() {
                if let Some(header) = self.headers.get(i) {
                    row_map.insert(header.clone(), cell.content.clone());
                }
            }
            rows.push(row_map);
        }
        // serde_json error will automatically convert due to From trait in TuiqlError
        serde_json::to_string(&rows).map_err(|e| TuiqlError::Json(e))
    }

    fn export_to_markdown(&self) -> Result<String> {
        let mut output = String::new();
        if !self.headers.is_empty() {
            output.push_str(&self.headers.join(" | "));
            output.push('\n');
            let underline: Vec<String> = self
                .headers
                .iter()
                .map(|h| format!("{}", "-".repeat(h.len())))
                .collect();
            output.push_str(&underline.join(" | "));
            output.push('\n');
        }
        for row in &self.rows {
            let row_content: Vec<String> =
                row.cells.iter().map(|cell| cell.content.clone()).collect();
            output.push_str(&row_content.join(" | "));
            output.push('\n');
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty_grid() {
        let grid = ResultsGrid::new();
        assert_eq!(grid.render(), "");
    }

    #[test]
    fn test_render_with_headers_and_rows() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);
        let rendered = grid.render();
        assert!(rendered.contains("ID | Name"));
        assert!(rendered.contains("1 | Alice"));
        assert!(rendered.contains("2 | Bob"));
    }

    #[test]
    fn test_export_to_csv() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);
        let csv = grid.export("csv").unwrap();
        assert!(csv.contains("ID,Name"));
        assert!(csv.contains("1,Alice"));
        assert!(csv.contains("2,Bob"));
    }

    #[test]
    fn test_export_to_json() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);
        let json = grid.export("json").unwrap();
        assert!(json.contains(r#""ID":"1""#));
        assert!(json.contains(r#""Name":"Alice""#));
        assert!(json.contains(r#""ID":"2""#));
        assert!(json.contains(r#""Name":"Bob""#));
    }

    #[test]
    fn test_export_unsupported_format() {
        let grid = ResultsGrid::new();
        let result = grid.export("xml");
        assert!(result.is_err());

        // Check that it's a UI error with the expected message
        if let Err(TuiqlError::Ui(msg)) = result {
            assert!(msg.contains("Unsupported export format"));
            assert!(msg.contains("xml"));
        } else {
            panic!("Expected UI error");
        }
    }
    #[test]
    fn test_export_to_markdown() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);
        let markdown = grid.export("markdown").unwrap();

        // Normalize the markdown output by trimming whitespace and splitting into lines
        let markdown_lines: Vec<&str> = markdown.trim().lines().collect();

        // Define the expected markdown output
        let expected_lines = vec!["ID | Name", "-- | ----", "1 | Alice", "2 | Bob"];

        // Assert that each line matches the expected output
        for (line, expected) in markdown_lines.iter().zip(expected_lines.iter()) {
            assert_eq!(line.trim(), *expected);
        }
    }
}
