use crate::{db, schema_navigator};
use std::io::{self, Write};

/// Represents a parsed REPL command.
#[derive(Debug, PartialEq)]
pub enum Command {
    Open(String),
    Attach { name: String, path: String },
    Ro,
    Rw,
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

    println!("Welcome to the tuiql REPL! Type :quit to exit.");
    let mut input = String::new();
    let command_palette = CommandPalette::new();

    loop {
        print!("> ");
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
            Command::Help => {
                println!("Available commands:");
                println!("  :help - List all available commands and their descriptions");
                println!("  :open <path> - Open a database");
                println!("  :attach <name> <path> - Attach a database");
                println!("  :ro - Toggle read-only mode");
                println!("  :rw - Toggle read-write mode");
                println!("  :pragma <name> [val] - View or set a pragma");
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
            Command::Open(path) => match db::connect(&path) {
                Ok(_) => println!("Successfully opened database: {}", path),
                Err(e) => eprintln!("Error opening database: {}", e),
            },
            Command::Sql(sql) => {
                if sql.trim().is_empty() {
                    continue;
                }
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
                    }
                    Err(e) => eprintln!("Error executing query: {}", e),
                }
            }
            _ => println!("You entered: {:?}", command),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_open_command() {
        let cmd = parse_command(":open database.db");
        assert_eq!(cmd, Command::Open("database.db".to_string()));
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
}
