# TUIQL User Guide

Welcome to TUIQL, a terminal-native SQLite client focused on efficiency and ease of use. TUIQL provides powerful schema analysis, full-text search capabilities, and comprehensive query optimization tools for professional SQLite development and data exploration.

**Key Features:**
- 🔧 Configuration system with XDG Base Directory support and automatic defaults
- 🔍 Comprehensive schema visualization with ER diagrams
- 🔍 Full-text search with FTS5 support and advanced ranking
- 📊 Query plan analysis and optimization insights
- 🎯 Intelligent SQL auto-completion with Tab completion
- 🚀 Professional reedline interface with Ctrl+R history search
- 📋 Database schema exploration with table relationships
- ⚡ Real-time query performance analysis
- 🔗 Transaction management and safety features
- 🧩 Plugin system for extending functionality with external tools

This guide will help you unlock the full potential of TUIQL's features for effective SQLite development and data analysis.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Basic Usage](#basic-usage)
3. [REPL Commands](#repl-commands)
4. [Working with Databases](#working-with-databases)
   - [Database Modes](#database-modes)
   - [Schema Exploration](#schema-exploration)
   - [Full-Text Search (FTS5)](#full-text-search-fts5)
5. [Extensibility](#extensibility)
   - [Plugin System](#plugin-system)
6. [Configuration](#configuration)
7. [Tips and Best Practices](#tips-and-best-practices)
8. [Troubleshooting](#troubleshooting)
9. [Future Features](#future-features-m2-development-in-progress)

## Getting Started

### Installation

TUIQL requires Rust and SQLite to be installed on your system. To install TUIQL:

1. Ensure you have Rust installed (via [rustup](https://rustup.rs/))
2. Clone the repository
3. Build the project:
   ```bash
   cargo build --release
   ```
4. The binary will be available at `./target/release/tuiql`

### First Run

You can start TUIQL in two ways:

1. With a database path:
   ```bash
   tuiql path/to/database.db
   ```

2. In interactive mode:
   ```bash
   tuiql
   ```

## Basic Usage

### Reedline Interface Features

TUIQL uses a professional reedline-powered interface with advanced editing capabilities:

#### Keyboard Shortcuts
- **Ctrl+R**: Reverse search through command history
- **Tab**: Intelligent SQL auto-completion (keywords, tables, columns)
- **↑/↓**: Navigate through command history
- **→/←/Home/End**: Standard line editing navigation
- **Ctrl+D**: Exit TUIQL (or type `:quit`)

#### Persistent History
TUIQL automatically maintains command history in `~/.tuiql/repl_history.txt`:
- History persists between sessions
- Search through your command history with Ctrl+R
- Automatic storage of successful and failed queries
- Performance metrics tracked for each query

### Connecting to a Database

There are two ways to connect to a database:

1. **Command Line**: Launch TUIQL with the database path as an argument:
   ```bash
   tuiql path/to/database.db
   ```

2. **REPL Command**: Use the `:open` command in interactive mode:
   ```sql
   :open path/to/database.db
   ```

If the connection is successful, you'll see a confirmation message.

### Viewing Help

To see all available commands and their descriptions, use the `:help` command:
```sql
:help
```

## REPL Commands

TUIQL provides several commands that start with a colon (`:`). Here's the complete list:

### Core Database Operations
- `:help` - Display a list of available commands
- `:open <path>` - Open a database file at specified path
- `:quit` - Exit TUIQL
- `:tables` - Display database schema information with row counts
- `:hist` - Show command and query history

### Query Analysis & Optimization
- `:plan` - Visualize SQL query execution plans (type query after command)
- `:erd` - Display comprehensive Entity-Relationship diagram for database schema

### Advanced Features (Available Now)
- `:fts5 <command>` - Full-text search management and operations

### Transaction Management
- `:begin` - Start a database transaction
- `:commit` - Commit current transaction
- `:rollback` - Rollback current transaction

### Session Management
- `:ro` - Toggle read-only mode
- `:rw` - Toggle read-write mode

### Advanced/Coming Soon
- `:attach <name> <path>` - Attach additional database (coming soon)
- `:pragma <name> [value]` - View or set SQLite pragmas (coming soon)
- `:fmt` - Format SQL queries (coming soon)
- `:export <format>` - Export result sets (coming soon)
- `:find <text>` - Search database schema (coming soon)
- `:snip <action>` - Query snippet management (coming soon)
- `:diff <dbA> <dbB>` - Compare database schemas (coming soon)
- `:plugin <name> [args]` - Execute external plugin with arguments

## Working with Databases

### Database Modes

TUIQL supports both read-only and read-write modes:

- Use `:ro` to enable read-only mode (prevents accidental modifications)
- Use `:rw` to enable read-write mode (required for INSERT/UPDATE/DELETE)
- Use `:begin`, `:commit`, and `:rollback` to manage transactions

### Schema Exploration

TUIQL provides powerful tools for understanding your database structure:

#### Schema Overview
```sql
:tables   -- Show all tables with row counts and basic schema info
```

#### Entity-Relationship Diagrams
```sql
:erd      -- Generate comprehensive ER diagram with relationships
```
The ER diagram shows:
- 📋 All tables with their columns and types
- 🔑 Primary key indicators
- 🔗 Foreign key relationships showing parent-child connections
- ↙️ Reference counters showing how many tables reference each one
- 📝 Column type information
- ⚠️ Circular reference warnings

### Full-Text Search (FTS5)

TUIQL includes comprehensive support for SQLite's FTS5 (Full-Text Search version 5) for natural language searching:

#### Getting Started with FTS5
```sql
-- First, explore the database and see what FTS5 tables exist
:fts5 list

-- Get help with usage examples
:fts5 help
```

#### Creating FTS5 Tables
```sql
-- Create a simple FTS5 table
CREATE VIRTUAL TABLE posts_fts USING fts5(title, content);

-- Create with custom tokenizer
CREATE VIRTUAL TABLE docs_fts USING fts5(title, body, tokenize=porter);

-- Index from existing table
INSERT INTO posts_fts SELECT id, title, content FROM posts WHERE content IS NOT NULL;
```

#### Searching with FTS5
```sql
-- Basic phrase search
SELECT * FROM posts_fts WHERE posts_fts MATCH 'database optimization';

-- Proximity search (words within 10 terms of each other)
SELECT * FROM posts_fts WHERE posts_fts MATCH 'database NEAR optimization';

-- Boolean operators
SELECT * FROM posts_fts WHERE posts_fts MATCH 'database OR mysql';
SELECT * FROM posts_fts WHERE posts_fts MATCH 'database AND NOT tutorial';

-- Ranked results (higher rank = better match)
SELECT title, rank FROM posts_fts WHERE posts_fts MATCH 'database' ORDER BY rank DESC;

-- Search with highlighting
SELECT highlight(posts_fts, 0, '<b>', '</b>') as highlighted_title
FROM posts_fts WHERE posts_fts MATCH 'database';
```

#### Advanced FTS5 Features
- **Tokenizers**: `porter` (stemming), `unicode61`, `trigram` (character-level)
- **Ranking**: BM25 algorithm for relevance scoring
- **Highlighting**: Mark search term occurrences in results
- **Phrase Queries**: Exact phrase matching with quotes
- **Prefix Search**: `*` wildcard for prefix matching
- **Boolean Logic**: AND, OR, NOT, AND_NOT operators

### Attaching Databases

You can attach additional databases using the `:attach` command (coming soon):
```sql
:attach my_other_db path/to/other.db
```
Overlay the attached database onto the primary database, allowing cross-database queries and references.
## Extensibility

### Plugin System

TUIQL supports a flexible plugin system that allows you to extend its functionality with external tools and scripts. Plugins are executable programs that can be easily integrated into your workflow through the `:plugin` command.

#### Setting Up Plugins

To use plugins, first configure them in your TUIQL configuration file:

```toml
[plugins]
enabled = [
  { name = "data_exporter", path = "/usr/local/bin/export.sh", description = "Export data in custom format" },
  { name = "backup_tool", path = "~/scripts/backup.py", description = "Create database backups" }
]
```

Each plugin is defined with:
- `name`: Short name used to invoke the plugin
- `path`: Path to the executable script or program
- `description`: Optional description shown in help

#### Using Plugins

Once configured, invoke plugins using the `:plugin` command:

```sql
:plugin data_exporter csv users
:plugin backup_tool --path /backup/location
```

#### Plugin Requirements

For a script to work as a TUIQL plugin:

1. **Executable**: The file must be executable (`chmod +x /path/to/plugin`)
2. **Path Resolution**: Use absolute paths or `~` for home directory
3. **Arguments**: Plugins receive command-line arguments as provided
4. **Output**: Plugin output is displayed in the TUIQL interface
5. **Exit Codes**: Non-zero exit codes are treated as errors

#### Example Plugin Script

Here's a simple bash plugin example:

```bash
#!/bin/bash
# Export plugin that takes table name and format as arguments

TABLE_NAME=$1
FORMAT=$2

echo "Exporting table $TABLE_NAME in $FORMAT format..."

# Export logic here...
echo "Export completed!"
```


## Tips and Best Practices

### General Usage
1. **Always check connection status**: After opening a database, verify that the connection was successful.
2. **Use read-only mode**: When you only need to query data, use `:ro` to prevent accidental modifications.
3. **Command history**: Use the ↑/↓ arrow keys to navigate through previous commands.
4. **Reverse history search**: Press Ctrl+R to search through your command history interactively.
5. **Tab completion**: SQL queries support intelligent completion - press Tab for contextual suggestions (keywords, table names, column names).
6. **Persistent sessions**: Your command history and performance metrics are automatically saved to `~/.tuiql/` and persist between sessions.

### Schema Exploration
1. **Start with schema overview**: Use `:tables` to get a quick understanding of your database structure.
2. **Explore relationships**: Use `:erd` to understand how tables connect via foreign keys.
3. **Use FTS5 for text content**: Set up full-text search for efficient content queries.

### Query Development
1. **Analyze performance**: Use `:plan` followed by a SQL query to see execution details.
2. **Leverage FTS5**: For text-heavy applications, FTS5 can dramatically improve search performance.
3. **Consider tokenizers**: Choose appropriate tokenizers (porter for stemming, trigram for Asian languages).

### Advanced Tips
1. **FTS5 ranking**: Use rank in ORDER BY clauses for better search result ordering.
2. **FTS5 highlighting**: Use highlight() function to show search term relevance in results.
3. **ER diagram insights**: Pay attention to tables with no relationships (isolated data) or circular references.

## Troubleshooting

### Common Issues

1. **Connection Failed**
   - Verify the database path is correct
   - Check file permissions
   - Ensure the file is a valid SQLite database

2. **Permission Denied**
   - Check file system permissions
   - Try running in read-only mode (`:ro`)

3. **Database is Locked**
   - Check if another process is using the database
   - Verify the database file isn't open in another application

### Getting Help

If you encounter issues:

1. Use `:help` to see available commands
2. Check the error messages for specific details
3. Consult the project's GitHub issues
4. Report new issues with detailed reproduction steps

## Future Features (M3 Development in Progress)

### Recently Completed (M2 Features)
- ✅ **Configuration System**: TOML configuration with XDG Base Directory support, automatic config creation, and application-wide settings
- ✅ **Schema Map Visualization**: Complete ER diagram generation with foreign key analysis
- ✅ **JSON1 Helper**: SQLite's built-in JSON functions for structured data handling
- ✅ **FTS5 Full-Text Search**: Comprehensive text search with BM25 ranking and highlighting
- ✅ **Database Diff**: Compare and merge schema differences between databases
- ✅ **Advanced Query Analysis**: Interactive query plan visualization
- ✅ **Enhanced REPL**: Intelligent completions and comprehensive help system
- ✅ **Reedline Professional Interface**: Full terminal editing with Ctrl+R history search, persistent storage, and advanced keyboard navigation
- ✅ **Plugin System**: External executable plugin integration for custom functionality

### Now Available (M2 Features)
- ✅ **Cancellable Queries**: Interrupt long-running database operations
- ✅ **Property Tests**: Comprehensive DDL validation framework

### In Development (M3 Features)
- **Advanced Query Editor**: Syntax highlighting, error detection, and formatting
- **Results Grid**: Virtualized scrolling for large datasets with sticky headers
- **Record Inspector**: Enhanced data viewing and editing capabilities
- **Export Options**: CSV, JSON, Markdown, and XML export formats
- **Query Snippets**: Save and manage frequently used query templates
- **Multi-Database Support**: Attach and query across multiple databases
- **Performance Monitoring**: Query timing and optimization suggestions

### Recent Achievements (Now Available)

🚀 **M2 Complete**: Advanced Features Milestone Achieved
🧩 **Plugin System**: External executable integration for custom tools
🚀 **Schema Map Visualization**: Complete ER diagram generation with foreign key analysis
🔍 **FTS5 Full-Text Search**: Comprehensive text search with BM25 ranking and highlighting
📊 **Advanced Query Analysis**: Interactive query plan visualization
🎯 **Enhanced REPL**: Intelligent completions and comprehensive help system
🚶 **Reedline Professional Interface**: Full terminal editing with Ctrl+R history search, persistent storage, and advanced keyboard navigation
⚖️ **Property Tests**: Comprehensive DDL validation framework
🔄 **Cancellable Queries**: Interrupt long-running database operations

## Configuration

TUIQL supports user configuration via a TOML file located in the XDG Base Directory specification path (`$XDG_CONFIG_HOME/tuiql/config.toml`, typically `~/.config/tuiql/config.toml`). If no configuration file exists, TUIQL automatically creates one with sensible defaults.

### Configuration Sections

The configuration file supports the following sections:

```toml
[ui]
theme = "dark"              # Interface theme (currently unused, future feature)
show_status_tips = true     # Show status line tips (currently unused, future feature)

[keys]
run = "F5"                  # Key binding for running queries (currently unused, future feature)
run_selection = "S-F5"      # Key binding for running selection (currently unused, future feature)
vim_mode = true             # Enable Vim-style key bindings (currently unused, future feature)

[sqlite]
load_extensions = []        # List of SQLite extensions to load on startup (currently unused, future feature)
page_size_hint = 4096       # Page size hint for SQLite connections (currently unused, future feature)

[plugins]
enabled = [
  { name = "my_plugin", path = "/path/to/plugin", description = "My custom plugin" },
  { name = "another_plugin", path = "~/scripts/another.sh", description = "Another plugin" }
]
```

### Editing Configuration

1. **Manual Editing**: Edit `~/.config/tuiql/config.toml` directly with your preferred text editor
2. **Automatic Creation**: If the file doesn't exist, TUIQL will create it with default values on first startup
3. **Runtime Update**: Currently changes require restarting TUIQL (this will be improved in future versions)

Stay tuned for ongoing development updates! New features are being added regularly.