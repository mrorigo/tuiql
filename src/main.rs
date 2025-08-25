use tracing::info;
use tracing_subscriber;

mod command_palette;
mod db;
mod plan;
mod query_editor;
mod record_inspector;
mod repl;
mod schema_map;
mod schema_navigator;

fn main() {
    // Initialize the logging system using tracing subscriber
    tracing_subscriber::fmt::init();

    info!("Starting tuiql...");

    // Basic startup message
    println!("Welcome to tuiql! A blazing-fast, terminal-native SQLite client.");

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let db_path = &args[1];
        println!("Attempting to open database: {}", db_path);
        // Use the new db::connect function for SQLite connection stub
        if let Err(e) = db::connect(db_path) {
            eprintln!("Failed to connect to database: {}", e);
        }
    } else {
        println!("No database provided. Running in interactive mode.");
        repl::run_repl();
    }
}

#[cfg(test)]
mod tests {
    use super::command_palette::CommandPalette;

    #[test]
    fn test_basic_math() {
        // A simple sanity test
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn test_command_palette_integration() {
        let palette = CommandPalette::new();
        let filtered = palette.filter_commands("open");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "open");
    }
}
