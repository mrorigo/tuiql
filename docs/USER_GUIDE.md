# TUIQL User Guide

Welcome to TUIQL, a terminal-native SQLite client focused on efficiency and ease of use. This guide will help you understand how to use TUIQL effectively.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Basic Usage](#basic-usage)
3. [REPL Commands](#repl-commands)
4. [Working with Databases](#working-with-databases)
5. [Tips and Best Practices](#tips-and-best-practices)
6. [Troubleshooting](#troubleshooting)

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

TUIQL provides several commands that start with a colon (`:`). Here are the currently available commands:

- `:help` - Display a list of available commands
- `:open <path>` - Open a database file
- `:attach <name> <path>` - Attach another database with the given name
- `:ro` - Toggle read-only mode
- `:rw` - Toggle read-write mode
- `:pragma <name> [value]` - View or set a pragma
- `:plan` - Visualize the query plan
- `:fmt` - Format the current query buffer
- `:export <format>` - Export the current result set
- `:find <text>` - Search in the database schema or queries
- `:erd [table]` - Show ER-diagram for the schema
- `:hist` - Show command/query history
- `:snip <action>` - Manage query snippets
- `:diff <dbA> <dbB>` - Compare schemas of two databases
- `:quit` - Exit TUIQL

## Working with Databases

### Database Modes

TUIQL supports both read-only and read-write modes:

- Use `:ro` to enable read-only mode
- Use `:rw` to enable read-write mode

### Attaching Databases

You can attach additional databases using the `:attach` command:
```sql
:attach my_other_db path/to/other.db
```

## Tips and Best Practices

1. **Always check connection status**: After opening a database, verify that the connection was successful.
2. **Use read-only mode**: When you only need to query data, use `:ro` to prevent accidental modifications.
3. **Command history**: Use the up and down arrow keys to navigate through previous commands.
4. **Tab completion**: Commands support tab completion - press Tab to see available options.

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

## Future Features

The following features are planned for future releases:

- Schema visualization with ER diagrams
- Advanced query editor with syntax highlighting
- Results grid with virtualized scrolling
- Record inspector for viewing and editing data
- Query plan visualization
- Export options (CSV, JSON, Markdown)

Stay tuned for updates!