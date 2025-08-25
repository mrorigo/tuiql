// Plan Module for SQL Query Visualization
//
// This module parses the output of EXPLAIN QUERY PLAN and generates a structured,
// visual representation of the plan. It highlights index usage and optimizes
// the visualization for better comprehension of query execution.

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
pub fn render_plan(plan: &str) -> String {
    // Parse the plan into structured rows
    let rows: Vec<&str> = plan.lines().collect();
    let mut visualization = String::from("=== Plan Visualization Start ===\n");

    for row in rows {
        if row.contains("USING INDEX") {
            visualization.push_str(&format!("🔍 {}\n", row)); // Highlight index usage
        } else {
            visualization.push_str(&format!("   {}\n", row));
        }
    }

    visualization.push_str("=== Plan Visualization End ===");
    visualization
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_plan_contains_header_and_footer() {
        let plan_with_index = "SCAN TABLE dummy_table USING INDEX idx_dummy";
        let visualization = render_plan(plan_with_index);
        assert!(visualization.contains("=== Plan Visualization Start ==="));
        assert!(visualization.contains("=== Plan Visualization End ==="));
        assert!(visualization.contains("SCAN TABLE dummy_table"));
        assert!(visualization.contains("🔍 SCAN TABLE dummy_table USING INDEX idx_dummy"));
    }
}
