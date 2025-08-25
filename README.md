tuiql/README.md
```

# TUIQL: A Terminal-Native SQLite Client

TUIQL is a blazing-fast, terminal-native, keyboard-centric SQLite client designed to make **schema comprehension**, **data navigation**, and **query iteration** effortless. It combines the speed of the `sqlite3` CLI with a delightful, discoverable TUI that scales from quick one-offs to day-long analysis sessions.

---

## Features

### Core Features
- **Schema Map**: Visualize relationships between tables with an ER-like graph, including grouping by schema and highlighting circular references.
- **Query Editor**: Multiline editing, syntax highlighting, advanced linting for dangerous SQL operations, and query formatting.
- **Results Grid**: Virtualized scrolling, sticky headers, and export options (CSV, JSON, Markdown).
- **Record Inspector**: View and edit records with type validation and safeguards, optimized for large records.
- **Plan Visualizer**: Render `EXPLAIN QUERY PLAN` as a tree with cost/loop/row details and index usage highlights.

### Additional Features
- **Command Palette**: Fuzzy search for commands like "Run", "Attach DB", "Export CSV", and more.
- **History & Snippets**: Save, pin, and replay queries with ease.
- **Performance Optimization**: Handles large datasets with virtualized paging and efficient rendering.
- **Extensibility**: Plugin support for custom commands, panels, and exporters.

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
tuiql path/to/database.db
```

### Keybindings
- **Command Palette**: `Ctrl+P`
- **Run Query**: `F5`
- **Toggle Modes**: `E` (Editor), `R` (Results), `S` (Schema Map), `P` (Plan Visualizer)
- **Navigate**: `hjkl` (Vim-style navigation)

### Commands
- `:open <path>`: Open a database.
- `:attach <name> <path>`: Attach another database.
- `:export [csv|json|md]`: Export the current result set.
- `:erd [table]`: Open the schema map and focus on a specific table.

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

### Benchmarking
To evaluate performance:
```bash
cargo bench
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