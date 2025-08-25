use tracing::info;
use tracing_subscriber;

mod db;

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
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_basic_math() {
        // A simple sanity test
        assert_eq!(2 + 2, 4);
    }
}
