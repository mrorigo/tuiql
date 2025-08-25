// Results Grid Stub Module for TUIQL
//
// This module provides a basic implementation for rendering tabular results in the terminal.
// In a complete version, this module will handle virtualized rendering for large datasets,
// sticky headers, sorting, and cell type formatting.

/// Represents a single cell in the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Cell {
    pub content: String,
}

/// Represents a row of cells in the grid.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub cells: Vec<Cell>,
}

/// Represents the entire grid structure.
#[derive(Debug, Clone)]
pub struct ResultsGrid {
    pub headers: Vec<String>,
    pub rows: Vec<Row>,
}

impl ResultsGrid {
    /// Creates a new, empty ResultsGrid.
    pub fn new() -> Self {
        ResultsGrid {
            headers: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Sets the headers for the grid.
    pub fn set_headers(&mut self, headers: Vec<String>) {
        self.headers = headers;
    }

    /// Adds a row to the grid. Each row is represented as a vector of strings.
    pub fn add_row(&mut self, row: Vec<String>) {
        let cells = row.into_iter().map(|s| Cell { content: s }).collect();
        self.rows.push(Row { cells });
    }

    /// Renders the grid as a simple string with headers and rows.
    /// This is a placeholder implementation which creates a plain text table.
    pub fn render(&self) -> String {
        let mut output = String::new();
        // Render headers if available
        if !self.headers.is_empty() {
            output.push_str(&self.headers.join(" | "));
            output.push('\n');
            // Underline headers
            let underline: Vec<String> = self.headers.iter().map(|h| "-".repeat(h.len())).collect();
            output.push_str(&underline.join("-|-"));
            output.push('\n');
        }
        // Render rows
        for row in &self.rows {
            let row_content: Vec<String> =
                row.cells.iter().map(|cell| cell.content.clone()).collect();
            output.push_str(&row_content.join(" | "));
            output.push('\n');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_grid_render() {
        let grid = ResultsGrid::new();
        let rendered = grid.render();
        // With no headers or rows, the rendered grid should be empty.
        assert_eq!(rendered, "");
    }

    #[test]
    fn test_grid_with_headers_and_rows() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec![
            "ID".to_string(),
            "Name".to_string(),
            "Email".to_string(),
        ]);
        grid.add_row(vec![
            "1".to_string(),
            "Alice".to_string(),
            "alice@example.com".to_string(),
        ]);
        grid.add_row(vec![
            "2".to_string(),
            "Bob".to_string(),
            "bob@example.com".to_string(),
        ]);
        let rendered = grid.render();
        // Check that the output contains the headers and row data.
        assert!(rendered.contains("ID | Name | Email"));
        assert!(rendered.contains("Alice | alice@example.com"));
        assert!(rendered.contains("Bob | bob@example.com"));
    }
}
