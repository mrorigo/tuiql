// Core infrastructure modules
pub mod core;
pub mod config;

// Feature-specific modules
pub mod catalog;
pub mod command_palette;
pub mod db;
pub mod diff;
pub mod fts5;
pub mod json1;
pub mod json_viewer;
pub mod plan;
pub mod plugins;
pub mod query_editor;
pub mod record_inspector;
pub mod repl;
pub mod results_grid;
pub mod schema_map;
pub mod schema_navigator;
pub mod sql;
pub mod sql_completer;
pub mod storage;

// Test utilities (available only in test builds)
#[cfg(test)]
pub mod test_utils;
