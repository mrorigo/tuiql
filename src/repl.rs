use crate::{
    db, schema_navigator, schema_map,
    storage::{HistoryEntry, Storage},
    plan, fts5, json1, sql_completer::SqlCompleter, diff,
    results_grid::ResultsGrid,
    plugins::PluginManager,
    query_editor::QueryEditor,
};
use crate::config::load_or_create_config;
use std::sync::mpsc;
use reedline::{
    Completer, History, Span, Suggestion,
    Reedline, Signal, FileBackedHistory, DefaultPrompt,
};
use std::io::{self, Write};
use std::sync::Arc;
use std::path::PathBuf;
use std::time::Instant;


#[derive(Debug, Default)]
struct ReplState {
    /// Stores the last query result for export functionality
    pub last_result_grid: Option<ResultsGrid>,
}

impl ReplState {
    pub fn store_result(&mut self, result: &db::QueryResult) {
        let mut grid = ResultsGrid::new();
        grid.set_headers(result.columns.clone());
        for row in &result.rows {
            grid.add_row(row.clone());
        }
        self.last_result_grid = Some(grid);
    }

    pub fn get_last_result(&self) -> Option<&ResultsGrid> {
        self.last_result_grid.as_ref()
    }
    fn new() -> Self {
        Self {
            last_result_grid: None,
        }
    }

    #[allow(dead_code)]
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

/// Reedline-compatible completer that wraps SqlCompleter
pub struct ReedlineCompleter {
    sql_completer: SqlCompleter,
    query_buffer: String,
}

impl ReedlineCompleter {
    /// Creates a new ReedlineCompleter with an underlying SqlCompleter
    pub fn new() -> Self {
        ReedlineCompleter {
            sql_completer: SqlCompleter::new(),
            query_buffer: String::new(),
        }
    }

    /// Updates the query buffer with current input (useful for multiline support)
    pub fn set_query_buffer(&mut self, buffer: String) {
        self.query_buffer = buffer;
    }
}

impl Completer for ReedlineCompleter {
    fn complete(
        &mut self,
        line: &str,
        position: usize
    ) -> Vec<Suggestion> {
        // Try to update schema for better completions
        let _ = self.sql_completer.update_schema();

        // Get suggestions from SqlCompleter
        match self.sql_completer.complete(line, position) {
            Ok(suggestions) => {
                suggestions.into_iter()
                    .map(|s| Suggestion {
                        value: s.clone(),
                        description: Some("SQL completion".to_string()),
                        extra: None,
                        span: Span::new(position, position),
                        append_whitespace: s.ends_with(' ') || matches!(s.to_uppercase().as_str(), "SELECT" | "FROM" | "WHERE" | "JOIN" | "ON" | "ORDER" | "BY" | "LIMIT" | "GROUP" | "HAVING"),
                    })
                    .collect()
            }
            Err(_) => Vec::new(),
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
    PlanEnhanced,
    Fmt,
    Export { format: String, filename: Option<String> },
    Find(String),
    Erd(Option<String>),
    Fts5(Option<String>),
    Json1(Option<String>),
    Hist,
    Snip(String),
    Diff { db_a: String, db_b: String },
    Plugin { name: String, args: Vec<String> },
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
        "plan_enhanced" => Command::PlanEnhanced,
        "fmt" => Command::Fmt,
        "export" => {
            if parts.len() >= 2 {
                let format = parts[1].to_string();
                let filename = if parts.len() >= 3 {
                    Some(parts[2].to_string())
                } else {
                    None
                };
                Command::Export { format, filename }
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
        "fts5" => {
            if parts.len() >= 2 {
                Command::Fts5(Some(parts[1].to_string()))
            } else {
                Command::Fts5(None)
            }
        }
        "json1" => {
            if parts.len() >= 2 {
                Command::Json1(Some(parts[1].to_string()))
            } else {
                Command::Json1(None)
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
        "plugin" => {
            if parts.len() >= 2 {
                let name = parts[1].to_string();
                let args: Vec<String> = parts[2..].iter().map(|s| s.to_string()).collect();
                Command::Plugin { name, args }
            } else {
                Command::Unknown(input.to_string())
            }
        }
        "help" => Command::Help,
        "tables" => Command::Tables,
        _ => Command::Unknown(input.to_string()),
    }
}

/// Enhanced REPL shell with readline support, persistent history, and auto-completion
pub fn run_repl() {
    use crate::command_palette::CommandPalette;

    println!("🏗️  Welcome to the tuiql REPL! Type :quit to exit.");
    println!("Enhanced with reedline support and persistent history.\n");

    let mut state = ReplState::new();
    let command_palette = CommandPalette::new();

    // Load configuration for plugins
    let config = load_or_create_config().map_err(|e| {
        eprintln!("Failed to load configuration: {}", e);
        e
    }).expect("Failed to load configuration");

    let mut plugin_manager = PluginManager::new();
    if let Some(ref plugins_config) = config.plugins {
        if let Err(e) = plugin_manager.load_plugins(&plugins_config.enabled) {
            eprintln!("Warning: Failed to load plugins: {}", e);
        }
    }

    // Initialize storage with cross-platform compatibility
    let home_dir = std::env::var("HOME").ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("Warning: HOME environment variable not set, using .");
            PathBuf::from(".")
        });
    let mut tuiql_dir = home_dir.clone();
    tuiql_dir.push(".tuiql");

    // Create the directory first
    if let Err(err) = std::fs::create_dir_all(&tuiql_dir) {
        eprintln!("Warning: Failed to create data directory '{}': {}, using in-memory storage", tuiql_dir.display(), err);
    }

    // Create separate paths for storage and history
    let mut storage_path = tuiql_dir.clone();
    storage_path.push("storage.db");

    let storage = Storage::new(storage_path).unwrap_or_else(|err| {
        eprintln!("Warning: Failed to initialize persistent storage: {}, using in-memory storage", err);
        Storage::new(PathBuf::from(":memory:")).expect("Failed to create in-memory storage")
    });

    // Initialize completer
    let completer = ReedlineCompleter::new();

    // Initialize reedline with history and completion
    let mut line_editor = Reedline::create()
        .with_completer(Box::new(completer));

    // Configure reedline history storage to use the same ~/.tuiql directory
    let mut history_path = tuiql_dir;
    history_path.push("repl_history.txt");
    let history_result = FileBackedHistory::with_file(1000, history_path).ok();
    if history_result.is_none() {
        println!("Note: Using in-memory history (persistent history unavailable)");
    }

    if let Some(history) = history_result {
        line_editor = line_editor.with_history(Box::new(history) as Box<dyn History>);
    } else {
        println!("Note: Using in-memory history (persistent history unavailable)");
    }

    println!("🔧 Reedline enabled: Ctrl+R history search, arrow keys navigation");
    println!("Use Ctrl+D or :quit to exit\n");

    // Track whether we're currently executing a query for cancellation support
    let executing_query = Arc::new(std::sync::Mutex::new(false));
    let executing_query_clone = executing_query.clone();

    // Global cancellation channel for interrupt communication
    let (global_cancel_tx, global_cancel_rx): (mpsc::Sender<()>, mpsc::Receiver<()>) = mpsc::channel();
    let global_cancel_tx = Arc::new(std::sync::Mutex::new(Some(global_cancel_tx)));
    let global_cancel_rx = Arc::new(std::sync::Mutex::new(Some(global_cancel_rx)));

    loop {
        // Read line with reedline
        match line_editor.read_line(&DefaultPrompt) {
            Ok(Signal::Success(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if trimmed == ":quit" {
                    break;
                }

                // Handle command suggestions
                if trimmed.starts_with(':') {
                    let suggestions = command_palette.filter_commands(&trimmed[1..]);
                    if !suggestions.is_empty() {
                        println!("Did you mean:");
                        for suggestion in suggestions {
                            println!("  :{} - {}", suggestion.name, suggestion.description);
                        }
                    }
                }

                // Parse and execute command
                let command = parse_command(&trimmed);
                let executing_query = executing_query_clone.clone();
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
                println!("  :attach <n> <path> - 🔗 Attach a database (coming soon!)");
                println!("  :ro - 🔒 Toggle read-only mode (coming soon!)");
                println!("  :rw - 🔓 Toggle read-write mode (coming soon!)");
                println!("  :begin - Start a new transaction");
                println!("  :commit - Commit current transaction");
                println!("  :rollback - Rollback current transaction");
                println!("  :pragma <n> [val] - ⚙️ View or set SQLite pragmas (coming soon!)");
                println!("  :plan - Visualize the query plan");
                println!("  :plan_enhanced - 🔬 Enhanced query plan with cost overlay and performance data");
                println!("  :fmt - 🛠️ Format the current query buffer (coming soon!)");
                println!("  :export <format> [<file>] - 📤 Export current result set (supported: csv, json, markdown)");
                println!("  :find <text> - 🔍 Search for text in the database schema or queries (coming soon!)");
                println!("  :erd [table] - 📊 Show ER-diagram for the schema");
                println!("  :fts5 [cmd] - 🔍 FTS5 full-text search helper");
                println!("  :json1 [cmd] - 🎯 JSON1 extension helper");
                println!("  :hist - Show command/query history");
                println!("  :plugin <name> [args] - 🧩 Execute a configured plugin");
                println!("  :snip <action> - 💾 Manage query snippets (coming soon!)");
                println!("  :diff <dbA> <dbB> - 🔄 Perform a schema diff between databases (coming soon!)");
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
                loop {
                    print!("Query: ");
                    io::stdout().flush().expect("Failed to flush stdout");
                    let mut input = String::new();
                    io::stdin()
                        .read_line(&mut input)
                        .expect("Failed to read input");
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.eq_ignore_ascii_case("quit") || trimmed.eq_ignore_ascii_case("exit") {
                        break;
                    }
                    match plan::explain_query(&trimmed) {
                        Ok(plan_output) => println!("{}", plan_output),
                        Err(e) => eprintln!("Error generating plan: {}", e),
                    }
                }
            }
            Command::PlanEnhanced => {
                println!("🔬 Enhanced Query Plan Analyzer with Cost Overlay");
                println!("Enter a SQL query to analyze its execution plan with performance data:");
                println!("(Note: Make sure a database is connected with :open first)");
                loop {
                    print!("Enhanced Query: ");
                    io::stdout().flush().expect("Failed to flush stdout");
                    let mut input = String::new();
                    io::stdin()
                        .read_line(&mut input)
                        .expect("Failed to read input");
                    let trimmed = input.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    if trimmed.eq_ignore_ascii_case("quit") || trimmed.eq_ignore_ascii_case("exit") {
                        break;
                    }
                    println!("\nAnalyzing query execution plan with cost overlay...");
                    println!("This may take a moment as it executes the query to gather timing data.");
                    match plan::explain_query_enhanced(&trimmed) {
                        Ok(plan_output) => println!("{}", plan_output),
                        Err(e) => eprintln!("Error generating enhanced plan: {}", e),
                    }
                }
            }
            Command::Open(path) => match db::connect(&path) {
                Ok(_) => println!("Successfully opened database: {}", path),
                Err(e) => eprintln!("Error opening database: {}", e),
            },
            Command::Sql(sql) => {
                if sql.trim().is_empty() {
                    continue;
                }

                // LINT: Check for dangerous operations before execution
                let mut editor = QueryEditor::new();
                editor.set_query(&sql);
                if let Err(lint_err) = editor.lint_query() {
                    eprintln!("⚠️  Linting Warning: {}", lint_err);
                    println!("Do you want to continue anyway? (y/N): ");

                    // Read user confirmation
                    let mut confirmation = String::new();
                    if std::io::stdin().read_line(&mut confirmation).is_err() {
                        continue; // Failed to read input, skip execution
                    }

                    if !confirmation.trim().to_lowercase().starts_with('y') {
                        println!("Query execution cancelled.");
                        continue;
                    }
                }

                let start_time = Instant::now();

                // Mark that we're executing a query
                *executing_query.lock().unwrap() = true;

                println!("Executing query... (Interrupt will be handled by Ctrl+C in input mode)");

                // Extract the global cancellation receiver for this command
                let global_rx_clone = global_cancel_rx.clone();
                let exec_flag = executing_query.clone();

                // Use the cancellable query function with a callback that monitors cancellation
                let cancellation_monitor = move |interrupt_handle: rusqlite::InterruptHandle| {
                    std::thread::spawn(move || {
                        // Wait for cancellation signal from global Ctrl+C handler
                        let rx_opt = global_rx_clone.lock().unwrap();
                        if let Some(rx) = rx_opt.as_ref() {
                            if let Ok(()) = rx.recv() {
                                // Interruption requested
                                interrupt_handle.interrupt();
                                *exec_flag.lock().unwrap() = false;
                            }
                        }
                    });
                };

                // Execute query with cancellation support
                let result = db::execute_cancellable_query(&sql, cancellation_monitor);

                // Mark query execution as complete
                *executing_query.lock().unwrap() = false;

                match result {
                    Ok(result) => {
                        // Store result in ReplState for export functionality
                        state.store_result(&result);

                        // Print column headers
                        println!("{}", result.columns.join(" | "));
                        println!("{}", "-".repeat(result.columns.join(" | ").len()));

                        // Print rows
                        for row in result.rows {
                            println!("{}", row.join(" | "));
                        }
                        println!("\n({} rows)", result.row_count);
                        println!("💡 Tip: Use ':export <format>' to export results to CSV, JSON, or Markdown");

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
            Command::Fmt => {
                println!("🛠️  SQL Formatting is coming soon!");
                println!("This feature will automatically format your SQL queries for better readability.");
            }
            Command::Export { format, filename } => {
                if let Some(grid) = state.get_last_result() {
                    match grid.export(&format) {
                        Ok(exported_data) => {
                            if let Some(fname) = filename {
                                match std::fs::write(&fname, &exported_data) {
                                    Ok(_) => {
                                        println!("📤 Exported results in {} format to file: {}", format.to_uppercase(), fname);
                                        // Show file size
                                        if let Ok(metadata) = std::fs::metadata(&fname) {
                                            println!("   File size: {} bytes", metadata.len());
                                        }
                                    }
                                    Err(e) => {
                                        eprintln!("❌ Failed to write to file '{}': {}", fname, e);
                                    }
                                }
                            } else {
                                println!("📤 Exported results in {} format:", format.to_uppercase());
                                println!("{}", exported_data);
                                println!("\n💡 To export to a file, use: :export {} <filename>", format);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ Export failed: {}", e);
                        }
                    }
                } else {
                    println!("❌ No query results available for export.");
                    println!("Run a SQL query first to generate results for export.");
                }
            }
            Command::Find(search_term) => {
                println!("🔍 Search functionality is coming soon!");
                println!("This will search your database schema and queries for: {:?}", search_term);
            }
            Command::Erd(_table) => {
                match schema_map::generate_schema_map() {
                    Ok(schema_map) => {
                        let diagram = schema_map::render_schema_map(&schema_map);
                        println!("{}", diagram);
                    }
                    Err(e) => {
                        println!("❌ Error generating schema map: {}", e);
                        println!("Make sure you have connected to a database with :open first.");
                    }
                }
            }
            Command::Fts5(action) => {
                match action.as_deref() {
                    Some("help") | None => {
                        println!("{}", fts5::fts5_help());
                    },
                    Some("list") => {
                        match fts5::list_fts5_tables() {
                            Ok(_) => {},
                            Err(e) => println!("❌ Error listing FTS5 tables: {}", e),
                        }
                    },
                    Some(cmd) => {
                        match fts5::execute_fts5_command(cmd) {
                            Ok(_) => {},
                            Err(e) => println!("❌ Error executing FTS5 command: {}", e),
                        }
                    }
                }
            }
            Command::Json1(action) => {
                match action {
                    Some(subcommand) if !subcommand.is_empty() => {
                        match json1::execute_json1_command(&subcommand) {
                            Ok(_) => {},
                            Err(e) => println!("❌ Error executing JSON1 command: {}", e),
                        }
                    }
                    _ => {
                        println!("{}", json1::json1_help());
                    }
                }
            }
            Command::Snip(action) => {
                println!("💾 Query snippets functionality is coming soon!");
                println!("This will manage saved query snippets. Action: {:?}", action);
            }
            Command::Diff { db_a, db_b } => {
                match diff::compare_databases(&db_a, &db_b) {
                    Ok(comparison) => {
                        let output = diff::format_comparison(&comparison, &db_a, &db_b);
                        println!("{}", output);
                    }
                    Err(e) => {
                        println!("❌ Error performing schema diff: {}", e);
                        println!("Make sure both database files exist and are valid SQLite databases.");
                        println!("Usage: :diff <database1> <database2>");
                    }
                }
            }
            Command::Pragma { name, value } => {
                println!("⚙️  Pragma functionality is coming soon!");
                println!("This will view/set SQLite pragmas. Name: {}, Value: {:?}", name, value);
            }
            Command::Ro => {
                println!("🔒 Read-only mode functionality is coming soon!");
                println!("This will set the database to read-only mode.");
            }
            Command::Rw => {
                println!("🔓 Read-write mode functionality is coming soon!");
                println!("This will set the database to read-write mode.");
            }
            Command::Attach { name, path } => {
                println!("🔗 Database attach functionality is coming soon!");
                println!("This will attach database '{}' at path '{}'", name, path);
            }
            Command::Plugin { name, args } => {
                match plugin_manager.execute_plugin(&name, &args) {
                    Ok(output) => {
                        println!("Plugin '{}' output:", name);
                        println!("{}", output);
                    }
                    Err(e) => {
                        println!("❌ Failed to execute plugin '{}': {}", name, e);
                        println!("Make sure the plugin is configured in your config.toml and the executable exists.");
                        println!("To list available plugins, check your ~/.config/tuiql/config.toml file.");
                    }
                }
            }
            Command::Unknown(command_str) => {
                println!("❓ Unknown command: '{}'", command_str);
                println!("Type ':help' to see available commands.");
            }
            }
        }
        Ok(Signal::CtrlC) => {
            // Check if we're currently executing a query
            if *executing_query.lock().unwrap() {
                eprintln!("\nQuery execution cancelled by user (Ctrl+C)");
                *executing_query.lock().unwrap() = false;
                // Send cancellation signal to interrupt query execution
                if let Ok(mut tx) = global_cancel_tx.lock() {
                    if let Some(tx) = tx.take() {
                        let _ = tx.send(());
                    }
                }
            } else {
                eprintln!("");
            }
            continue;
        }
        Ok(Signal::CtrlD) => {
            println!("\nGoodbye!");
            break;
        }
        Err(e) => {
            eprintln!("Error reading input: {}", e);
            continue;
        }
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
    use std::io::Write;

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
    fn test_parse_plugin_command_with_args() {
        let cmd = parse_command(":plugin myplugin arg1 arg2");
        assert_eq!(
            cmd,
            Command::Plugin {
                name: "myplugin".to_string(),
                args: vec!["arg1".to_string(), "arg2".to_string()]
            }
        );
    }

    #[test]
    fn test_parse_plugin_command_no_args() {
        let cmd = parse_command(":plugin simple");
        assert_eq!(
            cmd,
            Command::Plugin {
                name: "simple".to_string(),
                args: vec![]
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
    fn test_repl_state_store_and_get_result() {
        let mut state = ReplState::new();

        // Initially no result stored
        assert!(state.get_last_result().is_none());

        // Create a sample query result
        let query_result = db::QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
            ],
            row_count: 2,
        };

        // Store the result
        state.store_result(&query_result);

        // Verify result is stored and can be retrieved
        let stored_grid = state.get_last_result().unwrap();
        assert_eq!(stored_grid.headers, vec!["id".to_string(), "name".to_string()]);
        assert_eq!(stored_grid.rows.len(), 2);
        assert_eq!(stored_grid.rows[0].cells[1].content, "Alice");
        assert_eq!(stored_grid.rows[1].cells[0].content, "2");
    }

    #[test]
    fn test_export_command_with_results() {
        let mut state = ReplState::new();

        // Create and store a sample result
        let query_result = db::QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
            ],
            row_count: 2,
        };
        state.store_result(&query_result);

        // Test CSV export
        let csv_grid = state.get_last_result().unwrap();
        let csv_export = csv_grid.export("csv").unwrap();
        assert!(csv_export.contains("id,name"));
        assert!(csv_export.contains("1,Alice"));
        assert!(csv_export.contains("2,Bob"));

        // Test JSON export
        let json_export = csv_grid.export("json").unwrap();
        assert!(json_export.contains(r#""id":"1""#));
        assert!(json_export.contains(r#""name":"Alice""#));
        assert!(json_export.contains(r#""id":"2""#));

        // Test Markdown export - verify table content, not exact header format
        let md_export = csv_grid.export("markdown").unwrap();
        assert!(md_export.contains("1 | Alice"));
        assert!(md_export.contains("2 | Bob"));
        assert!(md_export.contains("|")); // Contains pipe separators
    }

    #[test]
    fn test_export_command_no_results() {
        let state = ReplState::new();

        // Test with no stored result
        assert!(state.get_last_result().is_none());
    }

    #[test]
    fn test_parse_export_command_with_filename() {
        let cmd = parse_command(":export csv results.csv");
        assert_eq!(
            cmd,
            Command::Export {
                format: "csv".to_string(),
                filename: Some("results.csv".to_string())
            }
        );
    }

    #[test]
    fn test_parse_export_command_without_filename() {
        let cmd = parse_command(":export json");
        assert_eq!(
            cmd,
            Command::Export {
                format: "json".to_string(),
                filename: None
            }
        );
    }

    #[test]
    fn test_export_to_file() -> std::io::Result<()> {
        // Create a temporary file for testing
        use std::io::Write;
        let mut tmpfile = tempfile::NamedTempFile::new()?;
        let filename = tmpfile.path().to_str().unwrap().to_string();

        let mut state = ReplState::new();

        // Create and store a sample result
        let query_result = db::QueryResult {
            columns: vec!["id".to_string(), "name".to_string()],
            rows: vec![
                vec!["1".to_string(), "Alice".to_string()],
                vec!["2".to_string(), "Bob".to_string()],
            ],
            row_count: 2,
        };
        state.store_result(&query_result);

        // Test file export functionality (simulate what the command handler does)
        if let Some(grid) = state.get_last_result() {
            let csv_export = grid.export("csv").unwrap();
            std::fs::write(&filename, &csv_export)?;
        }

        // Verify the file was written correctly
        let file_contents = std::fs::read_to_string(&filename)?;
        assert!(file_contents.contains("id,name"));
        assert!(file_contents.contains("1,Alice"));
        assert!(file_contents.contains("2,Bob"));

        Ok(())
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
