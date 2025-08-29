# Project Plan

This document outlines the strategy and high-level plan for the TUIQL project. It serves as a roadmap to guide development, testing, and deployment processes.

## Objectives

- Create a blazing-fast, terminal-native SQLite client ✅
- Focus on schema comprehension, data navigation, and query iteration 🚧
- Achieve < 2s time-to-first-result on commodity laptops ✅
- Maintain 90% keyboard accessibility with median 3 keystrokes to execute last query 🚧
- Ensure < 10ms TUI frame budget with 30+ FPS on 100k row scrolling 🚧
- Achieve P99 zero panics across 10k sessions 🚧

## Architecture

```
crates/
  app/                 # main binary, arg parsing, config
  core/                # domain: catalog cache, query exec, diff, plan
  tui/                 # ratatui widgets, layout, theming
  repl/                # reedline integration, command palette
  sql/                 # sqlparser wrappers, formatting, lint rules
  storage/             # exports, history, snippets, keybindings
  drivers/sqlite/      # rusqlite wrapper, pragmas, vtab helpers
  plugins/             # extension points & dynamic registration
```

## Milestones

### M0: Basic Functionality (Current Sprint)
- [x] Repository setup and basic documentation
- [x] Initial directory structure
- [x] Basic SQLite connection handling
- [x] REPL with command support
- [x] SQL execution and result display
- [x] Basic navigation tree
- [x] Simple history tracking
- [x] Unit tests for core functionality

### M1: Enhanced Query Support (Current Sprint) - MAJOR PROGRESS
- [x] Schema cache implementation
- [x] Transaction management (BEGIN/COMMIT/ROLLBACK)
- [x] Query auto-completion - COMPLETED with SQL completer
- [x] Basic record inspector - STUB IMPLEMENTED
- [ ] Export functionality (CSV/JSON/MD)
- [x] Basic query plan visualization - ENHANCED with tree structure
- [x] Integration tests for query features - MAJOR IMPROVEMENT

### M2: Advanced Features
- [x] Schema map visualization ✅ COMPLETED
- [x] JSON1 helper implementation ✅ COMPLETED
- [ ] FTS5 helper implementation
- [ ] Database diff functionality
- [ ] Cancellable query support
- [ ] Configuration system
- [ ] Property tests for DDL operations

### M3: Polish & Extensions
- [ ] Plugin system implementation
- [ ] Plan cost overlay visualization
- [ ] Dangerous operation linting
- [ ] ER diagram auto-layout
- [ ] Performance optimization
- [ ] Cross-platform testing
- [ ] Documentation completion

## Development Process

### Testing Strategy
- **Unit Tests**: Core components (catalog, plan tree, diff engine)
- **Golden Tests**: Widget render snapshots with ratatui harness
- **Integration Tests**: Sample database operations
- **Property Tests**: DDL operations (diff → apply → compare)
- **Performance Tests**: Benchmark against 1M row/100 table datasets

### Development Guidelines
- Use idiomatic Rust with focus on safety and performance
- Write tests for all new code and modified existing code
- Follow conventional commit format
- Document all public APIs and user-facing features
- Benchmark performance-critical code paths

### Safety & Performance
- Auto-open databases in autocommit mode
- Implement safe edit mode with WHERE clause guards
- Support read-only mode per connection
- Use virtual scrolling for large datasets
- Implement cancellable long-running queries

## Current Sprint Tasks

### In Progress
- Implement transaction management (BEGIN/COMMIT/ROLLBACK)
- Add auto-completion for SQL queries
- Design and implement record inspector
- Add export functionality

### Next Up
- FTS5 helper implementation - SQLite full-text search capabilities
- JSON1 helper implementation - Built-in JSON functions for SQLite
- Database diff functionality - Schema comparison between databases
- Cancellable query support - Interrupting long-running queries
- Configuration system - User preferences and persistent settings
- Property tests for DDL operations - Robust DDL testing framework

### Completed
- [x] Repository setup and documentation
- [x] Project structure and architecture
- [x] SQLite connection handling
- [x] Basic REPL implementation
- [x] Command parsing and execution
- [x] Help command system
- [x] SQL query execution and result display
- [x] Core functionality tests
- [x] SQL Query Auto-Completion System Implementation
- [x] Enhanced Query Plan Visualization with Tree Structure
- [x] Test Suite Improvements and Concurrency Issue Resolution
- [x] Major M1 Feature Completion
- [x] Schema Map Visualization (M2) - Complete ER diagram implementation with foreign key relationships
- [x] JSON1 Helper Implementation (M2) - Complete JSON1 extension helpers with query builders, REPL integration, and comprehensive tests

This plan will evolve as the project grows. Continuous improvements and adaptations will be made to meet the project's objectives and timelines.