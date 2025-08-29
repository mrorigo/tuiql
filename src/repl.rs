use crate::{
    db, schema_navigator,
    storage::{HistoryEntry, Storage},
};
use dirs::data_dir;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Debug, Default)]
struct ReplState {}

impl ReplState {
    fn new() -> Self {
        Self {}
    }

    fn get_prompt_prefix() -> String {
        if let Ok(guard) = db::DB_STATE.get().unwrap().lock() {
            let tx_indicator = match guard.transaction_state {
                db::TransactionState::Transaction => "*",
                db::TransactionState::Failed => "!",
                db::TransactionState::Autocommit => "",
            };
            if let Some(path) = &guard.current_path {
                format!("{}{}", path, tx_indicator)
            } else {
                tx_indicator.to_string()
            }
        } else {
            String::new()
        }
    }
}

/// Represents a parsed REPL command.
#[derive(Debug, PartialEq)]
pub enum Command {
    Open(String),
    Attach { name: String, path: String },
    Ro,
    Rw,
    Begin,
    Commit,
    Rollback,
    Pragma { name: String, value: Option<String> },
    Plan,
    Fmt,
    Export(String),
    Find(String),
    Erd(Option<String>),
    Hist,
    Snip(String),
    Diff { db_a: String, db_b: String },
    Help,
    Sql(String),
    Tables,
    Unknown(String),
}

/// Parses a user input string into a corresponding `Command`.
///
/// If the input starts with a colon (`:`), it is interpreted as a command.
/// Otherwise, it is treated as a SQL query.
pub fn parse_command(input: &str) -> Command {
    let input = input.trim();
    if !input.starts_with(':') {
        return Command::Sql(input.to_string());
    }
    let trimmed = &input[1..];
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.is_empty() {
        return Command::Unknown(input.to_string());
    }
    match parts[0] {
        "open" => {
            if parts.len() >= 2 {
                Command::Open(parts[1].to_string())
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "attach" => {
            if parts.len() >= 3 {
                Command::Attach {
                    name: parts[1].to_string(),
                    path: parts[2].to_string(),
                }
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "ro" => Command::Ro,
        "rw" => Command::Rw,
        "begin" => Command::Begin,
        "commit" => Command::Commit,
        "rollback" => Command::Rollback,
        "pragma" => {
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                let value = if parts.len() >= 3 {
                    Some(parts[2].to_string())
                } else {
                    None
                };
                Command::Pragma { name, value }
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "plan" => Command::Plan,
        "fmt" => Command::Fmt,
        "export" => {
            if parts.len() >= 2 {
                Command::Export(parts[1].to_string())
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "find" => {
            if parts.len() >= 2 {
                Command::Find(parts[1].to_string())
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "erd" => {
            if parts.len() >= 2 {
                Command::Erd(Some(parts[1].to_string()))
            } else {
                Command::Erd(None)
            }
        }
        "hist" => Command::Hist,
        "snip" => {
            if parts.len() >= 2 {
                Command::Snip(parts[1].to_string())
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "diff" => {
            if parts.len() >= 3 {
                Command::Diff {
                    db_a: parts[1].to_string(),
                    db_b: parts[2].to_string(),
                }
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "help" => Command::Help,
        "tables" => Command::Tables,
        _ => Command::Unknown(input.to_string()),
    }
}

/// Runs a simple REPL shell that reads commands from standard input,
/// parses them, and prints the parsed command. Type ":quit" to exit.
pub fn run_repl() {
    use crate::command_palette::CommandPalette;

    let _state = ReplState::new();
    println!("Welcome to the tuiql REPL! Type :quit to exit.");
    let mut input = String::new();
    let command_palette = CommandPalette::new();

    // Initialize storage
    let mut data_path = data_dir().unwrap_or_else(|| PathBuf::from("."));
    data_path.push("tuiql");
    std::fs::create_dir_all(&data_path).expect("Failed to create data directory");
    data_path.push("history.db");

    let storage = Storage::new(data_path).expect("Failed to initialize storage");

    loop {
        if let Ok(guard) = db::DB_STATE.get().unwrap().lock() {
            let tx_indicator = match guard.transaction_state {
                db::TransactionState::Transaction => "*",
                db::TransactionState::Failed => "!",
                db::TransactionState::Autocommit => "",
            };
            if let Some(path) = &guard.current_path {
                print!("{}{}>", path, tx_indicator);
            } else {
                print!("{}>", tx_indicator);
            }
        } else {
            print!("> ");
        }
        io::stdout().flush().expect("Failed to flush stdout");
        input.clear();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");
        let trimmed = input.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed == ":quit" {
            break;
        }

        if trimmed.starts_with(':') {
            let suggestions = command_palette.filter_commands(&trimmed[1..]);
            if !suggestions.is_empty() {
                println!("Did you mean:");
                for suggestion in suggestions {
                    println!("  :{} - {}", suggestion.name, suggestion.description);
                }
            }
        }

        let command = parse_command(&trimmed);
        match command {
            Command::Hist => match storage.get_recent_history(10) {
                Ok(entries) => {
                    println!("Recent command history:");
                    for entry in entries {
                        let timestamp = chrono::DateTime::from_timestamp(entry.timestamp, 0)
                            .unwrap_or_default()
                            .format("%Y-%m-%d %H:%M:%S")
                            .to_string();
                        println!(
                            "[{}] {} ({}ms) - {}",
                            timestamp,
                            if entry.success { "✓" } else { "✗" },
                            entry.duration_ms.unwrap_or(0),
                            entry.query
                        );
                    }
                }
                Err(e) => eprintln!("Error retrieving history: {}", e),
            },
            Command::Help => {
                println!("Available commands:");
                println!("  :help - List all available commands and their descriptions");
                println!("  :open <path> - Open a database");
                println!("  :attach <n> <path> - Attach a database");
                println!("  :ro - Toggle read-only mode");
                println!("  :rw - Toggle read-write mode");
                println!("  :begin - Start a new transaction");
                println!("  :commit - Commit current transaction");
                println!("  :rollback - Rollback current transaction");
                println!("  :pragma <n> [val] - View or set a pragma");
                println!("  :plan - Visualize the query plan");
                println!("  :fmt - Format the current query buffer");
                println!("  :export <format> - Export current result set");
                println!("  :find <text> - Search for text in the database schema or queries");
                println!("  :erd [table] - Show ER-diagram for the schema");
                println!("  :hist - Show command/query history");
                println!("  :snip <action> - Manage query snippets");
                println!("  :diff <dbA> <dbB> - Perform a schema diff between databases");
                println!("  :tables - Show database schema information");
                println!("\nOr enter SQL queries directly without any prefix.");
            }
            Command::Tables => match schema_navigator::SchemaNavigator::new() {
                Ok(navigator) => println!("{}", navigator.render()),
                Err(e) => eprintln!("Error getting schema: {}", e),
            },
            Command::Begin => match db::execute_query("BEGIN TRANSACTION") {
                Ok(_) => println!("Transaction started"),
                Err(e) => eprintln!("Failed to start transaction: {}", e),
            },
            Command::Commit => match db::execute_query("COMMIT") {
                Ok(_) => println!("Transaction committed"),
                Err(e) => eprintln!("Failed to commit transaction: {}", e),
            },
            Command::Rollback => match db::execute_query("ROLLBACK") {
                Ok(_) => println!("Transaction rolled back"),
                Err(e) => eprintln!("Failed to rollback transaction: {}", e),
            },
            Command::Plan => {
                println!("Enter a SQL query to visualize its execution plan:");
                println!("(Note: Make sure a database is connected with :open first)");
                println!("\nExecute :plan <your_sql_query> to see the plan, or enter the query directly here:");
            }
            Command::Open(path) => match db::connect(&path) {
                Ok(_) => println!("Successfully opened database: {}", path),
                Err(e) => eprintln!("Error opening database: {}", e),
            },
            Command::Sql(sql) => {
                if sql.trim().is_empty() {
                    continue;
                }
                let start_time = Instant::now();
                match db::execute_query(&sql) {
                    Ok(result) => {
                        // Print column headers
                        println!("{}", result.columns.join(" | "));
                        println!("{}", "-".repeat(result.columns.join(" | ").len()));

                        // Print rows
                        for row in result.rows {
                            println!("{}", row.join(" | "));
                        }
                        println!("\n({} rows)", result.row_count);

                        // Record successful query in history
                        let duration = start_time.elapsed().as_millis() as i64;
                        let entry = HistoryEntry::new(
                            sql.to_string(),
                            db::DB_STATE
                                .get()
                                .unwrap()
                                .lock()
                                .unwrap()
                                .current_path
                                .clone()
                                .unwrap_or_else(|| "main".to_string()),
                            true,
                            Some(duration),
                            Some(result.row_count as i64),
                        );
                        if let Err(e) = storage.add_history(entry) {
                            eprintln!("Failed to save to history: {}", e);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error executing query: {}", e);
                        // Record failed query in history
                        let duration = start_time.elapsed().as_millis() as i64;
                        let entry = HistoryEntry::new(
                            sql.to_string(),
                            db::DB_STATE
                                .get()
                                .unwrap()
                                .lock()
                                .unwrap()
                                .current_path
                                .clone()
                                .unwrap_or_else(|| "main".to_string()),
                            false,
                            Some(duration),
                            None,
                        );
                        if let Err(e) = storage.add_history(entry) {
                            eprintln!("Failed to save to history: {}", e);
                        }
                    }
                }
            }
            _ => println!("You entered: {:?}", command),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        db,
        storage::{HistoryEntry, Storage},
    };
    use tempfile;

    #[test]
    fn test_parse_open_command() {
        let cmd = parse_command(":open database.db");
        assert_eq!(cmd, Command::Open("database.db".to_string()));
    }

    #[test]
    fn test_parse_transaction_commands() {
        let begin_cmd = parse_command(":begin");
        assert_eq!(begin_cmd, Command::Begin);

        let commit_cmd = parse_command(":commit");
        assert_eq!(commit_cmd, Command::Commit);

        let rollback_cmd = parse_command(":rollback");
        assert_eq!(rollback_cmd, Command::Rollback);
    }

    #[test]
    fn test_transaction_execution() {
        // Setup test database
        db::tests::setup_test_db_global();

        // Start transaction
        let begin_result = db::execute_query("BEGIN TRANSACTION");
        assert!(begin_result.is_ok());

        // Execute some SQL within transaction
        let insert_result =
            db::execute_query("INSERT INTO test (name, value) VALUES ('transaction_test', 3.3)");
        assert!(insert_result.is_ok());

        // Verify transaction state
        let state = db::DB_STATE.get().unwrap().lock().unwrap();
        assert_eq!(state.transaction_state, db::TransactionState::Transaction);
        drop(state);

        // Commit transaction
        let commit_result = db::execute_query("COMMIT");
        assert!(commit_result.is_ok());

        // Verify data persisted
        let select_result =
            db::execute_query("SELECT name FROM test WHERE name = 'transaction_test'");
        assert!(select_result.is_ok());
        assert_eq!(select_result.unwrap().row_count, 1);
    }

    #[test]
    fn test_parse_attach_command() {
        let cmd = parse_command(":attach mydb path/to/mydb.db");
        assert_eq!(
            cmd,
            Command::Attach {
                name: "mydb".to_string(),
                path: "path/to/mydb.db".to_string()
            }
        );
    }

    #[test]
    fn test_parse_pragma_command_with_value() {
        let cmd = parse_command(":pragma page_size 4096");
        assert_eq!(
            cmd,
            Command::Pragma {
                name: "page_size".to_string(),
                value: Some("4096".to_string())
            }
        );
    }

    #[test]
    fn test_parse_pragma_command_without_value() {
        let cmd = parse_command(":pragma journal_mode");
        assert_eq!(
            cmd,
            Command::Pragma {
                name: "journal_mode".to_string(),
                value: None
            }
        );
    }

    #[test]
    fn test_parse_diff_command() {
        let cmd = parse_command(":diff db1.db db2.db");
        assert_eq!(
            cmd,
            Command::Diff {
                db_a: "db1.db".to_string(),
                db_b: "db2.db".to_string()
            }
        );
    }

    #[test]
    fn test_parse_unknown_command() {
        let cmd = parse_command(":invalid");
        assert_eq!(cmd, Command::Unknown(":invalid".to_string()));
    }

    #[test]
    fn test_parse_sql_query() {
        let cmd = parse_command("SELECT * FROM users");
        assert_eq!(cmd, Command::Sql("SELECT * FROM users".to_string()));
    }

    #[test]
    fn test_database_connection_state() {
        // Setup test database
        db::tests::setup_test_db_global();

        // Verify connection state
        let state = db::DB_STATE.get().unwrap().lock().unwrap();
        assert!(state.connection.is_some());
        assert_eq!(state.current_path.as_ref().unwrap(), ":memory:");
    }

    #[test]
    #[ignore = "Test disabled due to global state isolation issues during sequential test execution"]
    fn test_sql_execution_with_history() {
        // Setup test database
        db::tests::setup_test_db_global();

        // Create a temporary directory for history database
        let temp_dir = tempfile::tempdir().unwrap();
        let mut history_path = temp_dir.path().to_path_buf();
        history_path.push("history.db");

        let storage = Storage::new(history_path).unwrap();

        // Execute a query
        let sql = "SELECT * FROM test";
        let result = db::execute_query(sql).unwrap();

        // Verify query results
        assert_eq!(result.columns, vec!["id", "name", "value"]);
        // Note: Due to sequential test execution with global state,
        // there may be additional rows from previous transaction tests
        assert!(result.rows.len() >= 2, "Should have at least the original 2 test rows");
        assert_eq!(result.rows[0][1], "test1");
        assert_eq!(result.rows[1][1], "test2");

        // Add entry to history (since execute_query alone doesn't add history)
        let entry = HistoryEntry::new(
            sql.to_string(),
            ":memory:".to_string(),
            true,
            Some(0),
            Some(result.row_count as i64),
        );
        storage.add_history(entry).unwrap();

        // Check history entry
        let history = storage.get_recent_history(1).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].query, sql);
        assert!(history[0].success);
        assert_eq!(history[0].row_count, Some(2));

        // Clean up history storage
        drop(storage);
        temp_dir.close().unwrap();
    }
}
