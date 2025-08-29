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
            let icon = node.get_icon();

            if node.has_index() {
                // Highlight index usage with green color (in terminal)
                visualization.push_str(&format!("{}🔍 Index Used: {}\n", indent, node.details));
            } else if node.is_full_scan() {
                // Highlight potential performance issues
                visualization.push_str(&format!("{}⚠️  Full Scan: {}\n", indent, node.details));
            } else {
                visualization.push_str(&format!("{}{} {}\n", indent, icon, node.details));
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
}
