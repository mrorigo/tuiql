tuiql/README.md
```

# TUIQL: A Terminal-Native SQLite Client

> ⚠️ **Project Status**: Early Development - Core features are being implemented

TUIQL is a blazing-fast, terminal-native, keyboard-centric SQLite client designed to make **schema comprehension**, **data navigation**, and **query iteration** effortless. It combines the speed of the `sqlite3` CLI with a delightful, discoverable TUI that scales from quick one-offs to day-long analysis sessions.

---

## Features

### Core Features (🚧 In Development)
- **Basic SQLite Operations**: ✅ Connect to SQLite databases via CLI or REPL
- **REPL Interface**: ✅ Interactive command-line interface with command history
- **Schema Map**: 🚧 Visualize relationships between tables (coming soon)
- **Query Editor**: 🚧 Multiline editing with syntax highlighting (in progress)
- **Results Grid**: 🚧 Display query results with pagination
- **Record Inspector**: 🚧 View and edit records (planned)
- **Plan Visualizer**: 🚧 Render `EXPLAIN QUERY PLAN` output (planned)

### Additional Features (🚧 Planned)
- **Command Palette**: ✅ Basic command support (`:open`, `:help`, etc.)
- **Command Auto-completion**: ✅ Tab completion for commands
- **History & Snippets**: 🚧 Save and replay queries (planned)
- **Performance Optimization**: 🚧 Handle large datasets efficiently (planned)
- **Extensibility**: 🚧 Plugin support (future enhancement)

---

## Installation

### Prerequisites
- **Rust**: Ensure you have Rust installed. If not, install it via [rustup](https://rustup.rs/).
- **SQLite**: TUIQL uses SQLite as its database engine.

### Steps
1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/tuiql.git
   cd tuiql
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Run the binary:
   ```bash
   ./target/release/tuiql
   ```

---

## Usage

### Opening a Database
```bash
# Open directly with a database path
tuiql path/to/database.db

# Or start in interactive mode and connect later
tuiql
> :open path/to/database.db
```

### Available Commands
- `:help` - List all available commands and their descriptions
- `:open <path>` - Open a database
- `:attach <name> <path>` - Attach another database
- `:ro` - Toggle read-only mode
- `:rw` - Toggle read-write mode
- `:quit` - Exit the application

More commands and keybindings will be added as features are implemented.

---

## Development

### Project Structure
- **`src/`**: Core application code.
- **`docs/`**: Documentation files.
- **`tests/`**: Integration and unit tests.

### Running Tests
```bash
cargo test
```

---

## Contributing

We welcome contributions! Please follow these steps:
1. Fork the repository.
2. Create a feature branch:
   ```bash
   git checkout -b feature/your-feature
   ```
3. Commit your changes:
   ```bash
   git commit -m "feat: add your feature"
   ```
4. Push and create a pull request.

---

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---

## Acknowledgments

- Built with ❤️ using Rust and SQLite.
- Inspired by tools like `sqlite3`, `litecli`, and `DB Browser for SQLite`.

For questions or feedback, please open an issue or reach out to the maintainers.