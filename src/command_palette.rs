tuiql/src/command_palette.rs
// Command Palette Stub Module for TUIQL
//
// This module provides a basic implementation for a command palette,
// which allows users to view available commands, filter them via fuzzy search,
// and execute them. This is a stub implementation that simulates command handling.
//
// The palette maintains a list of commands with descriptions. In a real implementation,
// the command execution may trigger various actions within the application.

#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
}

pub struct CommandPalette {
    commands: Vec<Command>,
}

impl CommandPalette {
    /// Creates a new CommandPalette with an initial list of commands.
    pub fn new() -> Self {
        let commands = vec![
            Command {
                name: "open".to_string(),
                description: "Open a database".to_string(),
            },
            Command {
                name: "attach".to_string(),
                description: "Attach a database".to_string(),
            },
            Command {
                name: "ro".to_string(),
                description: "Toggle read-only mode".to_string(),
            },
            Command {
                name: "rw".to_string(),
                description: "Toggle read-write mode".to_string(),
            },
            Command {
                name: "pragma".to_string(),
                description: "View or set a pragma".to_string(),
            },
            Command {
                name: "plan".to_string(),
                description: "Visualize the query plan".to_string(),
            },
            Command {
                name: "fmt".to_string(),
                description: "Format the current query buffer".to_string(),
            },
            Command {
                name: "export".to_string(),
                description: "Export current result set".to_string(),
            },
            Command {
                name: "find".to_string(),
                description: "Search for text in the database schema or queries".to_string(),
            },
            Command {
                name: "erd".to_string(),
                description: "Show ER-diagram for the schema".to_string(),
            },
            Command {
                name: "hist".to_string(),
                description: "Show command/query history".to_string(),
            },
            Command {
                name: "snip".to_string(),
                description: "Manage query snippets".to_string(),
            },
            Command {
                name: "diff".to_string(),
                description: "Perform a schema diff between databases".to_string(),
            },
        ];
        CommandPalette { commands }
    }

    /// Searches for commands that contain the given query as a substring (case-insensitive)
    /// and returns the filtered list.
    pub fn filter_commands(&self, query: &str) -> Vec<Command> {
        let q = query.to_lowercase();
        self.commands
            .iter()
            .filter(|cmd| cmd.name.to_lowercase().contains(&q) || cmd.description.to_lowercase().contains(&q))
            .cloned()
            .collect()
    }

    /// Executes a command by name.
    /// In this stub implementation, execution simply returns a string message.
    /// Returns an error if the command is not found.
    pub fn execute_command(&self, name: &str) -> Result<String, String> {
        for cmd in &self.commands {
            if cmd.name == name {
                return Ok(format!("Executing command: {} - {}", cmd.name, cmd.description));
            }
        }
        Err(format!("Command '{}' not found", name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_commands() {
        let palette = CommandPalette::new();
        let filtered = palette.filter_commands("open");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "open");
    }

    #[test]
    fn test_execute_existing_command() {
        let palette = CommandPalette::new();
        let result = palette.execute_command("fmt");
        assert!(result.is_ok());
        assert!(result.unwrap().contains("fmt"));
    }

    #[test]
    fn test_execute_nonexistent_command() {
        let palette = CommandPalette::new();
        let result = palette.execute_command("nonexistent");
        assert!(result.is_err());
    }
}
