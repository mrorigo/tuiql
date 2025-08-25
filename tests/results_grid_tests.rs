#[cfg(test)]
mod results_grid_tests {
    use tuiql::results_grid::{Cell, ResultsGrid, Row, Viewport};

    #[test]
    fn test_viewport_scrolling() {
        let mut viewport = Viewport::new(0, 3);
        let rows = vec![
            vec!["Row1".to_string()],
            vec!["Row2".to_string()],
            vec!["Row3".to_string()],
            vec!["Row4".to_string()],
        ];

        // Test initial visible rows
        let row_objects: Vec<Row> = rows
            .iter()
            .map(|row| Row {
                cells: row
                    .iter()
                    .map(|content| Cell {
                        content: content.clone(),
                        cell_type: "text".to_string(),
                    })
                    .collect(),
                row_index: 0,
            })
            .collect();
        assert_eq!(viewport.visible_rows(&row_objects).len(), 3);

        // Scroll down
        viewport.scroll_down(row_objects.len());
        assert_eq!(viewport.start, 1);
        assert_eq!(viewport.end, 4);

        // Scroll up
        viewport.scroll_up();
        assert_eq!(viewport.start, 0);
        assert_eq!(viewport.end, 3);
    }

    #[test]
    fn test_results_grid_rendering() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);

        let rendered = grid.render();
        let expected = "\
ID | Name
-- | ----
1  | Alice
2  | Bob
";
        assert_eq!(
            rendered.replace(" ", "").replace("-", ""),
            expected.replace(" ", "").replace("-", "")
        );
    }

    #[test]
    fn test_results_grid_export_to_csv() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);

        let csv = grid.export("csv").unwrap();
        let expected = "\
ID,Name
1,Alice
2,Bob
";
        assert_eq!(csv, expected);
    }

    #[test]
    fn test_results_grid_export_to_json() {
        let mut grid = ResultsGrid::new();
        grid.set_headers(vec!["ID".to_string(), "Name".to_string()]);
        grid.add_row(vec!["1".to_string(), "Alice".to_string()]);
        grid.add_row(vec!["2".to_string(), "Bob".to_string()]);

        let json = grid.export("json").unwrap();
        let expected = r#"[{"ID":"1","Name":"Alice"},{"ID":"2","Name":"Bob"}]"#;
        assert_eq!(json, expected);
    }
}
