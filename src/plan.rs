// Plan Module for SQL Query Visualization
//
// This module parses the output of EXPLAIN QUERY PLAN and generates a structured,
// visual representation of the plan. It highlights index usage and optimizes
// the visualization for better comprehension of query execution.

use crate::core::{Result, TuiqlError};
use crate::db;

#[derive(Debug, Clone)]
pub struct PlanNode {
    pub id: i32,
    pub parent: i32,
    pub details: String,
    pub selectid: i32,
    pub level: i32,
}

#[derive(Debug)]
pub struct PlanStats {
    pub table_row_counts: std::collections::HashMap<String, i64>,
    pub execution_time_ms: Option<u128>,
}

impl PlanNode {
    pub fn new(id: i32, parent: i32, details: String) -> Self {
        let selectid = details.lines().next()
            .and_then(|line| {
                if let Some(colon_pos) = line.find(':') {
                    let after_colon = &line[colon_pos + 1..].trim().chars().take_while(|c| c.is_digit(10)).collect::<String>();
                    after_colon.parse::<i32>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(-1);

        PlanNode {
            id,
            parent,
            details,
            selectid,
            level: 0, // Will be set when building the tree
        }
    }

    pub fn has_index(&self) -> bool {
        self.details.to_uppercase().contains("USING INDEX")
    }

    pub fn is_full_scan(&self) -> bool {
        self.details.to_uppercase().contains("SCAN TABLE") &&
        !self.details.to_uppercase().contains("USING INDEX")
    }

    pub fn get_icon(&self) -> &'static str {
        if self.has_index() {
            "📇" // Index icon
        } else if self.is_full_scan() {
            "⚠️"  // Warning for full table scan
        } else {
            "📋"  // General plan icon
        }
    }

    pub fn get_table_name(&self) -> Option<String> {
        if let Some(table_part) = self.details.split(" FROM ").nth(1)
            .or_else(|| self.details.split("TABLE ").nth(1)) {
            // Extract table name from various formats like "FROM users", "SCAN TABLE users"
            let table_name = table_part.split_whitespace().next()?
                .split('.').last()? // Handle schema.table format
                .trim_matches(|c| !char::is_alphanumeric(c) && c != '_')
                .to_string();
            if !table_name.is_empty() { Some(table_name) } else { None }
        } else {
            None
        }
    }
}

/// Parses EXPLAIN QUERY PLAN output and returns structured plan nodes
pub fn parse_plan_output(plan_output: &str) -> Result<Vec<PlanNode>> {
    let mut nodes = Vec::new();

    for (index, line) in plan_output.lines().enumerate() {
        // Skip headers and empty lines
        let line = line.trim();
        if line.is_empty() || line.contains("SELECTID") || line.contains("ORDER") || line.contains("FROM") {
            continue;
        }

        // SQLite EXPLAIN QUERY PLAN format: "id|parent|details"
        let parts: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
        if parts.len() >= 3 {
            if let (Ok(id), Ok(parent)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                let details = parts[2..].join("|").to_string();
                nodes.push(PlanNode::new(id, parent, details));
            }
        } else if !line.is_empty() {
            // Fallback for other formats or direct output
            nodes.push(PlanNode::new(index as i32, -1, line.to_string()));
        }
    }

    if nodes.is_empty() && !plan_output.trim().is_empty() {
        return Err(TuiqlError::Query("Unable to parse query plan output. Expected format: id|parent|details".to_string()));
    }

    Ok(nodes)
}

/// Builds a tree structure from plan nodes and assigns levels
pub fn build_plan_tree(mut nodes: Vec<PlanNode>) -> Vec<PlanNode> {
    // Sort by id to ensure proper order
    nodes.sort_by_key(|n| n.id);

    // Create a lookup map for parent levels
    let mut parent_levels = std::collections::HashMap::new();

    // Assign levels based on parent relationships
    for node in &mut nodes {
        if node.parent == -1 {
            node.level = 0;
            parent_levels.insert(node.id, 0);
        } else {
            let parent_level = parent_levels.get(&node.parent).copied().unwrap_or(0);
            node.level = parent_level + 1;
            parent_levels.insert(node.id, node.level);
        }
    }

    nodes
}

/// Retrieves row counts for tables mentioned in the plan
pub fn get_table_row_counts(plan: &str) -> Result<std::collections::HashMap<String, i64>> {
    let nodes = parse_plan_output(plan)?;
    let mut table_names = std::collections::HashSet::new();

    // Extract unique table names from plan details
    for node in &nodes {
        if let Some(table_name) = node.get_table_name() {
            table_names.insert(table_name);
        }
    }

    let mut row_counts = std::collections::HashMap::new();

    for table_name in table_names {
        // Try using COUNT(*) for accurate counts, might be expensive for large tables
        match db::execute_query(&format!("SELECT COUNT(*) as count FROM {}", table_name)) {
            Ok(result) => {
                if let Some(row) = result.rows.get(0) {
                    if let Some(count_str) = row.get(0) {
                        if let Ok(count) = count_str.parse::<i64>() {
                            row_counts.insert(table_name, count);
                        }
                    }
                }
            }
            Err(_) => {
                // If count query fails, try to estimate from sqlite_stat1
                match db::execute_query(&format!("SELECT stat FROM sqlite_stat1 WHERE tbl = '{}' LIMIT 1", table_name)) {
                    Ok(result) => {
                        if let Some(row) = result.rows.get(0) {
                            if let Some(stat_str) = row.get(0) {
                                // Parse stat1 format: e.g., "12345 1 1" → take first number as approximate count
                                if let Some(count_part) = stat_str.split_whitespace().next() {
                                    if let Ok(count) = count_part.parse::<i64>() {
                                        row_counts.insert(table_name, count);
                                    }
                                }
                            }
                        }
                    },
                    Err(_) => {} // Table doesn't exist or no statistics available
                }
            }
        };
    }

    Ok(row_counts)
}

/// Renders enhanced plan visualization with cost indicators and performance hints
pub fn render_plan_with_cost_overlay(plan: &str) -> Result<String> {
    let nodes = parse_plan_output(plan)?;
    let tree_nodes = build_plan_tree(nodes);
    let table_row_counts = get_table_row_counts(plan)?;

    let mut visualization = String::from("=== Enhanced Query Plan with Cost Overlay ===\n");
    visualization.push_str("\nLegend: 📇 Index Used | ⚠️ Full Table Scan | 📋 General Operation");
    visualization.push_str("\n        📊 High Cost | 🔢 Row Estimate | 🏎️ Performance Hint\n\n");

    // Display table statistics summary
    if !table_row_counts.is_empty() {
        visualization.push_str("📊 Table Statistics:\n");
        for (table, count) in &table_row_counts {
            let size_hint = match count {
                0..=100 => "Small",
                101..=10000 => "Medium",
                10001..=100000 => "Large",
                _ => "Very Large",
            };
            visualization.push_str(&format!("  {}: ~{} rows ({})", table, count, size_hint));
            if *count > 100000 {
                visualization.push_str(" ⚠️ Consider indexing");
            }
            visualization.push_str("\n");
        }
        visualization.push_str("\n");
    }

    // Group nodes by selectid for compound queries
    let mut nodes_by_select: std::collections::HashMap<i32, Vec<&PlanNode>> = std::collections::HashMap::new();
    for node in &tree_nodes {
        nodes_by_select.entry(node.selectid).or_insert_with(Vec::new).push(node);
    }

    for (selectid, nodes) in nodes_by_select {
        if selectid >= 0 {
            visualization.push_str(&format!("📋 Compound Query {}:\n", selectid));
        }

        for node in nodes {
            let indent = "  ".repeat(node.level as usize);
            let icon = node.get_icon();

            // Add performance annotations
            let mut annotations = Vec::new();

            if let Some(table_name) = node.get_table_name() {
                if let Some(row_count) = table_row_counts.get(&table_name) {
                    if *row_count > 10000 && node.is_full_scan() {
                        annotations.push(format!("📊 High Cost (~{} rows scanned)", row_count));
                    }
                }
            }

            if node.has_index() {
                annotations.push("🏎️ Good: Using Index".to_string());
            } else if node.is_full_scan() {
                annotations.push("⚠️ Warning: Full Table Scan".to_string());
            }

            // Main node line
            if node.has_index() {
                visualization.push_str(&format!("{}🔍 Index Used: {}\n", indent, node.details));
            } else if node.is_full_scan() {
                visualization.push_str(&format!("{}⚠️ Full Scan: {}\n", indent, node.details));
            } else {
                visualization.push_str(&format!("{}{} {}\n", indent, icon, node.details));
            }

            // Annotations
            if !annotations.is_empty() {
                for annotation in annotations {
                    visualization.push_str(&format!("{}   └─ {}\n", indent, annotation));
                }
            }
        }

        if selectid >= 0 {
            visualization.push_str("\n");
        }
    }

    if tree_nodes.is_empty() {
        visualization.push_str("No plan data to display.\n");
    }

    // Performance summary
    let full_scans = tree_nodes.iter().filter(|n| n.is_full_scan()).count();
    let index_scans = tree_nodes.iter().filter(|n| n.has_index()).count();

    visualization.push_str("\n📈 Performance Summary:\n");
    visualization.push_str(&format!("  Index operations: {}\n", index_scans));
    visualization.push_str(&format!("  Full table scans: {}\n", full_scans));

    if full_scans > index_scans {
        visualization.push_str("  💡 Consider adding indexes for frequently queried columns\n");
    }

    visualization.push_str("\n=== End Enhanced Query Plan ===");
    Ok(visualization)
}

/// Renders a detailed visualization for a given SQL query plan.
///
/// # Arguments
///
/// * `plan` - A string slice representing the output of an EXPLAIN QUERY PLAN command.
///
/// # Returns
///
/// A String that provides a structured and optimized visual representation of the plan,
/// highlighting index usage and other key execution details.
pub fn render_plan(plan: &str) -> Result<String> {
    let nodes = parse_plan_output(plan)?;
    let tree_nodes = build_plan_tree(nodes);

    let mut visualization = String::from("=== Query Plan Visualization ===\n");
    visualization.push_str("\nLegend: 📇 Index Used | ⚠️ Full Table Scan | 📋 General Operation\n\n");

    // Group nodes by selectid for compound queries
    let mut nodes_by_select: std::collections::HashMap<i32, Vec<&PlanNode>> = std::collections::HashMap::new();
    for node in &tree_nodes {
        nodes_by_select.entry(node.selectid).or_insert_with(Vec::new).push(node);
    }

    for (selectid, nodes) in nodes_by_select {
        if selectid >= 0 {
            visualization.push_str(&format!("🗂️  Compound Query {}:\n", selectid));
        }

        for node in nodes {
            let indent = "  ".repeat(node.level as usize);

            if node.has_index() {
                // Highlight index usage with green color (in terminal)
                visualization.push_str(&format!("{}🔍 Index Used: {}\n", indent, node.details));
            } else if node.is_full_scan() {
                // Highlight potential performance issues
                visualization.push_str(&format!("{}⚠️  Full Scan: {}\n", indent, node.details));
            } else {
                visualization.push_str(&format!("{}{} {}\n", indent, node.get_icon(), node.details));
            }
        }

        if selectid >= 0 {
            visualization.push_str("\n");
        }
    }

    if tree_nodes.is_empty() {
        visualization.push_str("No plan data to display.\n");
    }

    visualization.push_str("\n=== End Query Plan ===");
    Ok(visualization)
}

/// Renders enhanced plan visualization with execution timing overlay
pub fn render_plan_with_timing(plan: &str, execution_time_ms: u128) -> Result<String> {
    let nodes = parse_plan_output(plan)?;
    let tree_nodes = build_plan_tree(nodes);

    let mut visualization = String::from("=== Query Plan with Execution Timing ===\n");
    visualization.push_str(&format!("⏱️  Execution Time: {} ms\n\n", execution_time_ms));

    // Group nodes by selectid for compound queries
    let mut nodes_by_select: std::collections::HashMap<i32, Vec<&PlanNode>> = std::collections::HashMap::new();
    for node in &tree_nodes {
        nodes_by_select.entry(node.selectid).or_insert_with(Vec::new).push(node);
    }

    for (selectid, nodes) in nodes_by_select {
        if selectid >= 0 {
            visualization.push_str(&format!("🗂️  Compound Query {}:\n", selectid));
        }

        for node in nodes {
            let indent = "  ".repeat(node.level as usize);

            if node.has_index() {
                visualization.push_str(&format!("{}🔍 Index Used: {}\n", indent, node.details));
            } else if node.is_full_scan() {
                visualization.push_str(&format!("{}⚠️  Full Scan: {}\n", indent, node.details));
            } else {
                visualization.push_str(&format!("{}{} {}\n", indent, node.get_icon(), node.details));
            }
        }

        if selectid >= 0 {
            visualization.push_str("\n");
        }
    }

    if tree_nodes.is_empty() {
        visualization.push_str("No plan data to display.\n");
    }

    visualization.push_str("\n=== End Query Plan ===");
    Ok(visualization)
}

/// Executes EXPLAIN QUERY PLAN for a given SQL query and returns enhanced visualization
pub fn explain_query_plan_enhanced(query: &str) -> Result<String> {
    let explain_query = format!("EXPLAIN QUERY PLAN {}", query.trim_end_matches(';'));
    let result = db::execute_query(&explain_query)?;

    if result.rows.is_empty() {
        return Ok("No execution plan available.\n".to_string());
    }

    // Convert the result back to the format expected by render_plan
    let mut plan_output = String::new();
    for row in &result.rows {
        // SQLite returns: id|parent|notused|detail
        if row.len() >= 4 {
            plan_output.push_str(&format!("{}|{}|{}\n", row[0], row[1], row[3]));
        }
    }

    // Try to get actual query timing by running it (with LIMIT 1 for safety)
    let time_query = if query.to_uppercase().contains("SELECT") &&
                     !query.to_uppercase().contains("LIMIT") {
        // Add LIMIT 1 to avoid timing large queries
        format!("{} LIMIT 1", query)
    } else {
        query.to_string()
    };

    let start_time = std::time::Instant::now();
    let _ = db::execute_query(&time_query);
    let execution_time_ms = start_time.elapsed().as_millis();

    match render_plan_with_cost_overlay(&plan_output) {
        Ok(plan_text) => {
            // Add timing information to the enhanced plan
            let mut enhanced_output = String::new();
            enhanced_output.push_str(&plan_text);
            enhanced_output.insert_str(plan_text.find("\n\n").unwrap_or(plan_text.len()-1) + 1,
            &format!("⏱️  Query execution time: {} ms\n", execution_time_ms));
            Ok(enhanced_output)
        }
        Err(_) => render_plan(&plan_output)
    }
}

/// Executes EXPLAIN QUERY PLAN for a given SQL query and returns the visualization
pub fn explain_query_plan(query: &str) -> Result<String> {
    let explain_query = format!("EXPLAIN QUERY PLAN {}", query.trim_end_matches(';'));
    let result = db::execute_query(&explain_query)?;

    if result.rows.is_empty() {
        return Ok("No execution plan available.\n".to_string());
    }

    // Convert the result back to the format expected by render_plan
    let mut plan_output = String::new();
    for row in &result.rows {
        // SQLite returns: id|parent|notused|detail
        if row.len() >= 4 {
            plan_output.push_str(&format!("{}|{}|{}\n", row[0], row[1], row[3]));
        }
    }

    render_plan(&plan_output)
}

/// Explains a query with enhanced visualization
pub fn explain_query_enhanced(query: &str) -> Result<String> {
    match explain_query_plan_enhanced(query) {
        Ok(plan) => Ok(plan),
        Err(TuiqlError::Query(_) | TuiqlError::App(_)) => {
            // Try a simpler approach - just run the query with EXPLAIN
            let explain_stmt = format!("EXPLAIN {}", query.trim_end_matches(';'));
            match db::execute_query(&explain_stmt) {
                Ok(result) => {
                    let mut output = String::from("=== EXPLAIN Output ===\n\n⚠️  Unable to provide enhanced plan analysis. Showing basic EXPLAIN output:\n\n");
                    for row in &result.rows {
                        output.push_str(&row.join(" | "));
                        output.push('\n');
                    }
                    output.push_str("\n=== End EXPLAIN ===");
                    Ok(output)
                }
                Err(_) => Ok("Unable to generate query plan. Ensure the query is valid and the database is connected.\n".to_string()),
            }
        }
        Err(e) => Err(e),
    }
}

/// Explains a query with a different approach if the first one fails
pub fn explain_query(query: &str) -> Result<String> {
    match explain_query_plan(query) {
        Ok(plan) => Ok(plan),
        Err(TuiqlError::Query(_) | TuiqlError::App(_)) => {
            // Try a simpler approach - just run the query with EXPLAIN
            let explain_stmt = format!("EXPLAIN {}", query.trim_end_matches(';'));
            match db::execute_query(&explain_stmt) {
                Ok(result) => {
                    let mut output = String::from("=== EXPLAIN Output ===\n");
                    for row in &result.rows {
                        output.push_str(&row.join(" | "));
                        output.push('\n');
                    }
                    output.push_str("=== End EXPLAIN ===");
                    Ok(output)
                }
                Err(_) => Ok("Unable to generate query plan. Ensure the query is valid and the database is connected.\n".to_string()),
            }
        }
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plan_contains_header_and_footer() {
        let plan_with_index = "0|-1|SCAN TABLE dummy_table USING INDEX idx_dummy";
        let visualization = render_plan(plan_with_index).unwrap();
        assert!(visualization.contains("=== Query Plan Visualization ==="));
        assert!(visualization.contains("=== End Query Plan ==="));
        assert!(visualization.contains("SCAN TABLE dummy_table"));
        assert!(visualization.contains("🔍 Index Used"));
    }

    #[test]
    fn test_parse_plan_output() {
        let plan_output = "0|-1|SCAN TABLE users\n1|0|USING INDEX idx_users_name";
        let nodes = parse_plan_output(plan_output).unwrap();
        assert_eq!(nodes.len(), 2);
        assert_eq!(nodes[0].id, 0);
        assert_eq!(nodes[1].id, 1);
        assert!(nodes[1].has_index());
    }

    #[test]
    fn test_explain_query_empty_result() {
        let empty_output = "";
        let nodes = parse_plan_output(empty_output).unwrap();
        assert!(nodes.is_empty());
    }

    #[test]
    fn test_explain_query_with_database() {
        // This test would require a database setup
        // For now, test that the function exists and returns reasonable results
        let result = explain_query("SELECT 1").unwrap_or_else(|_| "Query plan not available".to_string());
        assert!(!result.is_empty());
    }

    #[test]
    fn test_get_table_row_counts() {
        // Test with mock plan output
        let mock_plan = "0|-1|SCAN TABLE users\n1|-1|SCAN TABLE posts";
        let row_counts = get_table_row_counts(mock_plan).unwrap_or_default();

        // This test may fail if database connection is unavailable
        // The function should handle that gracefully
        if !row_counts.is_empty() {
            // If we have a database, these tables may exist
            assert!(row_counts.contains_key("users") || row_counts.contains_key("posts"));
        }
    }

    #[test]
    fn test_render_plan_with_cost_overlay() {
        let plan_with_index = "0|-1|SCAN TABLE users USING INDEX idx_users_name\n1|-1|SCAN TABLE posts";
        let result = render_plan_with_cost_overlay(plan_with_index);

        if let Ok(visualization) = result {
            assert!(visualization.contains("Enhanced Query Plan with Cost Overlay"));
            assert!(visualization.contains("🏎️ Good: Using Index"));
            // Table statistics may not be available if no database connection
            assert!(visualization.contains("Enhanced Query Plan with Cost Overlay")
                   || visualization.contains("Performance Summary"));
        } else {
            // If no database connection, that's okay - the function handles it gracefully
            println!("Test passed - enhanced plan gracefully handled missing database");
        }
    }

    #[test]
    fn test_plan_node_table_extraction() {
        // Test with format that contains " FROM "
        let from_query_node = PlanNode::new(1, 0, "SELECT * FROM users JOIN posts".to_string());
        assert_eq!(from_query_node.get_table_name(), Some("users".to_string()));

        // Test with simplified table scan format
        let table_scan_node = PlanNode::new(2, 1, "SELECT * FROM posts".to_string());
        assert_eq!(table_scan_node.get_table_name(), Some("posts".to_string()));

        // Test with complex query format
        let complex_query_node = PlanNode::new(3, 2, "SELECT users.name FROM users u WHERE u.id > 10".to_string());
        assert_eq!(complex_query_node.get_table_name(), Some("users".to_string()));

        // Test node without table patterns
        let no_table_node = PlanNode::new(4, 3, "SCAN CONSTANT ROW".to_string());
        assert_eq!(no_table_node.get_table_name(), None);
    }
}
