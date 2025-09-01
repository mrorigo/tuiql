use tracing::info;
use tuiql::{config, db, repl};

fn main() {
    // Initialize the logging system using tracing subscriber
    tracing_subscriber::fmt::init();

    info!("Starting tuiql...");

    // Load configuration
    match config::load_or_create_config() {
        Ok(cfg) => {
            info!("Configuration loaded successfully: {:?}", cfg);
            println!("🔧 Configuration loaded from {}", config::get_config_path().display());
        }
        Err(e) => {
            eprintln!("⚠️  Warning: Failed to load configuration: {}", e);
            eprintln!("Using default settings.");
        }
    }

    // Basic startup message
    println!("Welcome to tuiql! A blazing-fast, terminal-native SQLite client.");

    // Parse CLI arguments
    let args: Vec<String> = std::env::args().collect();
    match args.len() {
        1 => {
            println!("No database provided. Running in interactive mode.");
            repl::run_repl();
        }
        _ => {
            let db_path = &args[1];
            println!("Attempting to open database: {}", db_path);
            match db::connect(db_path) {
                Ok(_) => {
                    println!("Successfully connected to database: {}", db_path);
                    println!("Starting interactive mode with connected database.");
                    repl::run_repl();
                }
                Err(e) => {
                    eprintln!("Failed to connect to database: {}", e);
                    println!("Starting interactive mode instead.");
                    repl::run_repl();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tuiql::command_palette::CommandPalette;

    #[test]
    fn test_command_palette_integration() {
        let palette = CommandPalette::new();
        let filtered = palette.filter_commands("open");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "open");
    }
}
