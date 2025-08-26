tuiql/README.md
```

# TUIQL: A Terminal-Native SQLite Client

> ⚠️ **Project Status**: Early Development - Core features are being implemented

TUIQL is a blazing-fast, terminal-native, keyboard-centric SQLite client designed to make **schema comprehension**, **data navigation**, and **query iteration** effortless. It combines the speed of the `sqlite3` CLI with a delightful, discoverable TUI that scales from quick one-offs to day-long analysis sessions. Focus on writing SQL, not fighting with your tools.

---

## Features

### Core Features
- **Database Operations**: ✅ Connect to SQLite databases via CLI or REPL, with transaction support
- **REPL Interface**: ✅ Interactive command-line interface with:
  - Persistent command history
  - Transaction management (`:begin`, `:commit`, `:rollback`)
  - Transaction status display
  - Database context awareness
- **Schema Navigation**: ✅ Browse tables, columns, indexes with row counts
- **Query Editor**: 🚧 Multiline editing with syntax highlighting (in progress)
- **Results Grid**: ✅ Display query results with column headers
- **Record Inspector**: 🚧 View and edit records (planned)
- **Plan Visualizer**: 🚧 Render `EXPLAIN QUERY PLAN` output (planned)

### Additional Features
- **Command Palette**: ✅ Rich command support (`:open`, `:help`, `:tables`, etc.)
- **Query History**: ✅ Persistent history with success/failure tracking
- **Safety Features**: ✅ Transaction guards and state management
- **Auto-completion**: 🚧 Tab completion for SQL and commands (in progress)
- **Performance**: ✅ Fast response times for common operations
- **Extensibility**: 🚧 Plugin support (planned)

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
Core commands:
- `:help` - List all available commands and their descriptions
- `:open <path>` - Open a database
- `:attach <n> <path>` - Attach another database
- `:tables` - Show database schema information

Transaction management:
- `:begin` - Start a new transaction
- `:commit` - Commit current transaction
- `:rollback` - Rollback current transaction

Database settings:
- `:ro` - Toggle read-only mode
- `:rw` - Toggle read-write mode
- `:pragma <name> [value]` - View or set pragma values

Navigation:
- `:hist` - Show command/query history
- `:quit` - Exit the application

More commands and keybindings will be added as features are implemented.

### Example Session
```
$ tuiql example.db
Welcome to tuiql! A blazing-fast, terminal-native SQLite client.
Attempting to open database: example.db
Successfully connected to database: example.db
Starting interactive mode with connected database.
example.db> :tables
Table: users
  Row Count: 3
  Columns:
    id INTEGER [PK]
    name TEXT [NOT NULL]
    email TEXT
  Indexes:
    - idx_users_email (email)

example.db> :begin
Transaction started
example.db*> SELECT * FROM users;
id | name  | email
---+-------+------------------
1  | Alice | alice@email.com
2  | Bob   | bob@email.com
3  | Carol | carol@email.com

(3 rows)
example.db*> INSERT INTO users (name, email) VALUES ('Dave', 'dave@email.com');
example.db*> SELECT COUNT(*) FROM users;
count(*)
--------
4

(1 row)
example.db*> :rollback
Transaction rolled back
example.db> SELECT COUNT(*) FROM users;
count(*)
--------
3

(1 row)
example.db> :quit
```

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