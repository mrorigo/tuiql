// Cargo.toml
[package]
name = "tuiql"
version = "0.1.0"
edition = "2021"
authors = ["TuiQL Team"]
description = "A blazing-fast, terminal-native, keyboard-centric SQLite client"
license = "MIT"
repository = "https://github.com/tuiql/tuiql"

[[bin]]
name = "tuiql"
path = "src/main.rs"

[dependencies]
# TUI and terminal handling
ratatui = "0.24"
crossterm = "0.27"

# Database
rusqlite = { version = "0.30", features = ["bundled", "modern_sqlite"] }

# CLI and configuration
clap = { version = "4.0", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
toml = "0.8"

# Text editing and completion
reedline = "0.27"

# SQL parsing and formatting
sqlparser = "0.43"

# Async runtime and utilities
tokio = { version = "1.0", features = ["full"] }
futures = "0.3"

# Error handling and logging
anyhow = "1.0"
thiserror = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"

# Utilities
chrono = { version = "0.4", features = ["serde"] }
dirs = "5.0"
uuid = { version = "1.0", features = ["v4"] }

# JSON handling
serde_json = "1.0"

---

// src/main.rs
use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{error, info};

mod app;
mod config;
mod core;
mod tui;
mod sql;
mod storage;

use app::TuiqlApp;
use config::Config;

#[derive(Parser)]
#[command(name = "tuiql")]
#[command(about = "A blazing-fast, terminal-native, keyboard-centric SQLite client")]
struct Args {
    /// SQLite database file to open
    database: Option<PathBuf>,

    /// Open in read-only mode
    #[arg(short, long)]
    readonly: bool,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(format!("tuiql={}", log_level))
        .init();

    info!("Starting TuiQL SQLite Client");

    // Load configuration
    let config = Config::load(args.config.as_deref())?;

    // Create and run the application
    let mut app = TuiqlApp::new(config, args.database, args.readonly)?;

    if let Err(e) = app.run().await {
        error!("Application error: {}", e);
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

---

// src/config.rs
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ui: UiConfig,
    pub keys: KeyConfig,
    pub sqlite: SqliteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    pub theme: String,
    pub show_status_tips: bool,
    pub page_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyConfig {
    pub run: String,
    pub run_selection: String,
    pub vim_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqliteConfig {
    pub load_extensions: Vec<String>,
    pub page_size_hint: u32,
    pub foreign_keys: bool,
    pub journal_mode: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ui: UiConfig {
                theme: "dark".to_string(),
                show_status_tips: true,
                page_size: 100,
            },
            keys: KeyConfig {
                run: "F5".to_string(),
                run_selection: "S-F5".to_string(),
                vim_mode: true,
            },
            sqlite: SqliteConfig {
                load_extensions: vec![],
                page_size_hint: 4096,
                foreign_keys: true,
                journal_mode: "WAL".to_string(),
            },
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let config_path = match path {
            Some(p) => p.to_path_buf(),
            None => Self::default_config_path()?,
        };

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            let config = Config::default();
            if let Some(parent) = config_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(&config)?;
            std::fs::write(&config_path, content)?;
            Ok(config)
        }
    }

    fn default_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine config directory"))?;
        Ok(config_dir.join("tuiql").join("config.toml"))
    }
}

---

// src/core/mod.rs
pub mod database;
pub mod catalog;
pub mod query;
pub mod plan;

pub use database::*;
pub use catalog::*;
pub use query::*;
pub use plan::*;

---

// src/core/database.rs
use anyhow::{Context, Result};
use rusqlite::{Connection, OpenFlags, Row};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::config::SqliteConfig;

#[derive(Debug, Clone)]
pub struct DatabaseConnection {
    pub name: String,
    pub path: String,
    pub readonly: bool,
    connection: Arc<Mutex<Connection>>,
}

impl DatabaseConnection {
    pub fn open<P: AsRef<Path>>(
        name: String,
        path: P,
        readonly: bool,
        config: &SqliteConfig,
    ) -> Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        let mut flags = OpenFlags::SQLITE_OPEN_URI;
        if readonly {
            flags |= OpenFlags::SQLITE_OPEN_READ_ONLY;
        } else {
            flags |= OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
        }

        let conn = Connection::open_with_flags(&path_str, flags)
            .with_context(|| format!("Failed to open database: {}", path_str))?;

        // Set pragmas
        if config.foreign_keys {
            conn.pragma_update(None, "foreign_keys", "ON")?;
        }

        conn.pragma_update(None, "journal_mode", &config.journal_mode)?;
        conn.pragma_update(None, "page_size", config.page_size_hint)?;

        info!("Opened database: {} (readonly: {})", path_str, readonly);

        Ok(Self {
            name,
            path: path_str,
            readonly,
            connection: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn execute_query(&self, sql: &str) -> Result<QueryResult> {
        let conn = self.connection.lock().unwrap();

        debug!("Executing query: {}", sql);

        let start = std::time::Instant::now();

        // Check if it's a SELECT query
        let trimmed = sql.trim().to_uppercase();
        if trimmed.starts_with("SELECT") || trimmed.starts_with("WITH") {
            let mut stmt = conn.prepare(sql)?;
            let column_names: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(|s| s.to_string())
                .collect();

            let rows = stmt.query_map([], |row| {
                let mut values = Vec::new();
                for i in 0..column_names.len() {
                    let value = match row.get::<_, rusqlite::types::Value>(i) {
                        Ok(v) => format_sqlite_value(v),
                        Err(_) => "NULL".to_string(),
                    };
                    values.push(value);
                }
                Ok(values)
            })?;

            let mut data = Vec::new();
            for row in rows {
                data.push(row?);
            }

            let duration = start.elapsed();
            Ok(QueryResult::Select {
                columns: column_names,
                rows: data,
                duration,
            })
        } else {
            // Execute non-SELECT statement
            let changes = conn.execute(sql, [])?;
            let duration = start.elapsed();
            Ok(QueryResult::Execute { changes, duration })
        }
    }

    pub fn get_tables(&self) -> Result<Vec<TableInfo>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT name, type, sql FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name"
        )?;

        let tables = stmt.query_map([], |row| {
            Ok(TableInfo {
                name: row.get(0)?,
                table_type: row.get::<_, String>(1)?,
                sql: row.get::<_, Option<String>>(2)?,
                columns: Vec::new(), // Will be populated separately
            })
        })?;

        let mut result = Vec::new();
        for table in tables {
            let mut table_info = table?;
            // Get column information
            table_info.columns = self.get_table_columns(&table_info.name)?;
            result.push(table_info);
        }

        Ok(result)
    }

    pub fn get_table_columns(&self, table_name: &str) -> Result<Vec<ColumnInfo>> {
        let conn = self.connection.lock().unwrap();
        let mut stmt = conn.prepare("SELECT * FROM pragma_table_info(?1)")?;

        let columns = stmt.query_map([table_name], |row| {
            Ok(ColumnInfo {
                name: row.get(1)?,
                data_type: row.get(2)?,
                not_null: row.get::<_, i32>(3)? != 0,
                default_value: row.get(4)?,
                primary_key: row.get::<_, i32>(5)? != 0,
            })
        })?;

        let mut result = Vec::new();
        for column in columns {
            result.push(column?);
        }

        Ok(result)
    }

    pub fn explain_query(&self, sql: &str) -> Result<Vec<ExplainRow>> {
        let conn = self.connection.lock().unwrap();
        let explain_sql = format!("EXPLAIN QUERY PLAN {}", sql);
        let mut stmt = conn.prepare(&explain_sql)?;

        let rows = stmt.query_map([], |row| {
            Ok(ExplainRow {
                id: row.get(0)?,
                parent: row.get(1)?,
                detail: row.get(3)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }

        Ok(result)
    }
}

fn format_sqlite_value(value: rusqlite::types::Value) -> String {
    match value {
        rusqlite::types::Value::Null => "NULL".to_string(),
        rusqlite::types::Value::Integer(i) => i.to_string(),
        rusqlite::types::Value::Real(f) => f.to_string(),
        rusqlite::types::Value::Text(s) => s,
        rusqlite::types::Value::Blob(b) => {
            if b.len() <= 16 {
                format!("BLOB({}): {:02x?}", b.len(), b)
            } else {
                format!("BLOB({}) bytes", b.len())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct TableInfo {
    pub name: String,
    pub table_type: String,
    pub sql: Option<String>,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Debug, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub not_null: bool,
    pub default_value: Option<String>,
    pub primary_key: bool,
}

#[derive(Debug, Clone)]
pub enum QueryResult {
    Select {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        duration: std::time::Duration,
    },
    Execute {
        changes: usize,
        duration: std::time::Duration,
    },
}

#[derive(Debug, Clone)]
pub struct ExplainRow {
    pub id: i32,
    pub parent: i32,
    pub detail: String,
}

---

// src/core/catalog.rs
use anyhow::Result;
use std::collections::HashMap;
use crate::core::database::{DatabaseConnection, TableInfo};

pub struct Catalog {
    databases: HashMap<String, DatabaseConnection>,
    current_db: Option<String>,
    tables: HashMap<String, Vec<TableInfo>>,
}

impl Catalog {
    pub fn new() -> Self {
        Self {
            databases: HashMap::new(),
            current_db: None,
            tables: HashMap::new(),
        }
    }

    pub fn add_database(&mut self, db: DatabaseConnection) -> Result<()> {
        let name = db.name.clone();

        // Load table information
        let tables = db.get_tables()?;
        self.tables.insert(name.clone(), tables);

        self.databases.insert(name.clone(), db);

        // Set as current if it's the first database
        if self.current_db.is_none() {
            self.current_db = Some(name);
        }

        Ok(())
    }

    pub fn get_current_database(&self) -> Option<&DatabaseConnection> {
        self.current_db
            .as_ref()
            .and_then(|name| self.databases.get(name))
    }

    pub fn get_tables(&self, db_name: Option<&str>) -> Option<&Vec<TableInfo>> {
        let name = db_name.or(self.current_db.as_ref())?;
        self.tables.get(name)
    }

    pub fn get_table_names(&self, db_name: Option<&str>) -> Vec<String> {
        self.get_tables(db_name)
            .map(|tables| tables.iter().map(|t| t.name.clone()).collect())
            .unwrap_or_default()
    }

    pub fn refresh_tables(&mut self, db_name: &str) -> Result<()> {
        if let Some(db) = self.databases.get(db_name) {
            let tables = db.get_tables()?;
            self.tables.insert(db_name.to_string(), tables);
        }
        Ok(())
    }
}

---

// src/tui/mod.rs
pub mod app;
pub mod components;
pub mod events;

pub use app::*;
pub use components::*;
pub use events::*;

---

// src/tui/app.rs
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Table, Row, Cell},
    Frame, Terminal,
};
use std::io;
use tokio::sync::mpsc;

use crate::core::{Catalog, QueryResult};

#[derive(Debug, Clone, PartialEq)]
pub enum AppMode {
    Editor,
    Results,
    Schema,
    Plan,
    Diff,
    History,
    Graph,
}

pub struct TuiApp {
    catalog: Catalog,
    current_mode: AppMode,
    query_editor: QueryEditor,
    results_view: ResultsView,
    schema_view: SchemaView,
    status_message: String,
    show_help: bool,
}

impl TuiApp {
    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            current_mode: AppMode::Editor,
            query_editor: QueryEditor::new(),
            results_view: ResultsView::new(),
            schema_view: SchemaView::new(),
            status_message: "Ready".to_string(),
            show_help: false,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| self.render(f))?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        break;
                    }
                    KeyCode::F(5) => {
                        self.execute_query().await;
                    }
                    KeyCode::Char('e') => {
                        self.current_mode = AppMode::Editor;
                    }
                    KeyCode::Char('r') => {
                        self.current_mode = AppMode::Results;
                    }
                    KeyCode::Char('s') => {
                        self.current_mode = AppMode::Schema;
                    }
                    KeyCode::Char('h') => {
                        self.show_help = !self.show_help;
                    }
                    _ => {
                        match self.current_mode {
                            AppMode::Editor => self.query_editor.handle_input(key),
                            AppMode::Results => self.results_view.handle_input(key),
                            AppMode::Schema => self.schema_view.handle_input(key),
                            _ => {}
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn render<B: Backend>(&mut self, frame: &mut Frame<B>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Status bar
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Command line
            ])
            .split(frame.size());

        // Status bar
        self.render_status_bar(frame, chunks[0]);

        // Main content
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(30), Constraint::Min(0)])
            .split(chunks[1]);

        // Schema navigator
        self.render_schema_navigator(frame, main_chunks[0]);

        // Main pane based on mode
        match self.current_mode {
            AppMode::Editor => self.render_query_editor(frame, main_chunks[1]),
            AppMode::Results => self.render_results_view(frame, main_chunks[1]),
            AppMode::Schema => self.render_schema_view(frame, main_chunks[1]),
            _ => self.render_placeholder(frame, main_chunks[1], &format!("{:?} Mode", self.current_mode)),
        }

        // Command line
        self.render_command_line(frame, chunks[2]);

        // Help overlay
        if self.show_help {
            self.render_help_overlay(frame);
        }
    }

    fn render_status_bar<B: Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let db_name = self.catalog
            .get_current_database()
            .map(|db| db.name.as_str())
            .unwrap_or("No DB");

        let status_text = format!("{} | Mode: {:?} | {}", db_name, self.current_mode, self.status_message);
        let paragraph = Paragraph::new(status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(paragraph, area);
    }

    fn render_schema_navigator<B: Backend>(&mut self, frame: &mut Frame<B>, area: Rect) {
        let tables = self.catalog.get_tables(None).cloned().unwrap_or_default();

        let items: Vec<ListItem> = tables
            .iter()
            .map(|table| {
                let style = if table.table_type == "table" {
                    Style::default().fg(Color::White)
                } else {
                    Style::default().fg(Color::Yellow)
                };

                let line = Line::from(vec![
                    Span::styled(&table.name, style),
                    Span::raw(format!(" ({})", table.columns.len())),
                ]);

                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Schema"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(list, area, &mut self.schema_view.list_state);
    }

    fn render_query_editor<B: Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let paragraph = Paragraph::new(self.query_editor.content.clone())
            .block(Block::default().borders(Borders::ALL).title("Query Editor"))
            .wrap(ratatui::widgets::Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }

    fn render_results_view<B: Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        match &self.results_view.current_result {
            Some(QueryResult::Select { columns, rows, duration }) => {
                let header_cells = columns.iter().map(|h| Cell::from(h.clone()));
                let header = Row::new(header_cells).style(Style::default().fg(Color::Yellow));

                let data_rows = rows.iter().take(100).map(|row| {
                    let cells = row.iter().map(|c| Cell::from(c.clone()));
                    Row::new(cells)
                });

                let table = Table::new(data_rows)
                    .header(header)
                    .block(Block::default().borders(Borders::ALL).title(format!(
                        "Results ({} rows, {:.2}ms)",
                        rows.len(),
                        duration.as_secs_f64() * 1000.0
                    )))
                    .widths(&vec![Constraint::Length(15); columns.len()]);

                frame.render_widget(table, area);
            }
            Some(QueryResult::Execute { changes, duration }) => {
                let text = format!(
                    "Query executed successfully.\nRows affected: {}\nTime: {:.2}ms",
                    changes,
                    duration.as_secs_f64() * 1000.0
                );
                let paragraph = Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).title("Results"));
                frame.render_widget(paragraph, area);
            }
            None => {
                let paragraph = Paragraph::new("No results to display")
                    .block(Block::default().borders(Borders::ALL).title("Results"));
                frame.render_widget(paragraph, area);
            }
        }
    }

    fn render_schema_view<B: Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let paragraph = Paragraph::new("Schema map visualization would go here")
            .block(Block::default().borders(Borders::ALL).title("Schema Map"));
        frame.render_widget(paragraph, area);
    }

    fn render_placeholder<B: Backend>(&self, frame: &mut Frame<B>, area: Rect, title: &str) {
        let paragraph = Paragraph::new(format!("{} implementation coming soon...", title))
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(paragraph, area);
    }

    fn render_command_line<B: Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let help_text = "F5: Run | E: Editor | R: Results | S: Schema | H: Help | Ctrl+Q: Quit";
        let paragraph = Paragraph::new(help_text)
            .style(Style::default().fg(Color::Gray));
        frame.render_widget(paragraph, area);
    }

    fn render_help_overlay<B: Backend>(&self, frame: &mut Frame<B>) {
        let help_text = vec![
            "TuiQL Help",
            "",
            "Global Commands:",
            "  F5          - Execute query",
            "  Ctrl+Q      - Quit application",
            "  H           - Toggle this help",
            "",
            "Mode Switching:",
            "  E           - Query Editor",
            "  R           - Results View",
            "  S           - Schema View",
            "",
            "Navigation:",
            "  Arrow keys  - Navigate UI",
            "  Tab         - Switch panes",
            "",
            "Press H to close this help.",
        ];

        let area = centered_rect(60, 70, frame.size());

        frame.render_widget(Clear, area);

        let help_widget = Paragraph::new(help_text.join("\n"))
            .block(Block::default().borders(Borders::ALL).title("Help"))
            .wrap(ratatui::widgets::Wrap { trim: false });

        frame.render_widget(help_widget, area);
    }

    async fn execute_query(&mut self) {
        if let Some(db) = self.catalog.get_current_database() {
            match db.execute_query(&self.query_editor.content) {
                Ok(result) => {
                    self.results_view.current_result = Some(result);
                    self.current_mode = AppMode::Results;
                    self.status_message = "Query executed successfully".to_string();
                }
                Err(e) => {
                    self.status_message = format!("Query error: {}", e);
                }
            }
        } else {
            self.status_message = "No database connected".to_string();
        }
    }
}

pub struct QueryEditor {
    pub content: String,
    cursor_position: usize,
}

impl QueryEditor {
    pub fn new() -> Self {
        Self {
            content: "SELECT * FROM sqlite_master;".to_string(),
            cursor_position: 0,
        }
    }

    pub fn handle_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char(c) => {
                self.content.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    self.content.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Enter => {
                self.content.insert(self.cursor_position, '\n');
                self.cursor_position += 1;
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.content.len() {
                    self.cursor_position += 1;
                }
            }
            _ => {}
        }
    }
}

pub struct ResultsView {
    pub current_result: Option<QueryResult>,
}

impl ResultsView {
    pub fn new() -> Self {
        Self {
            current_result: None,
        }
    }

    pub fn handle_input(&mut self, _key: crossterm::event::KeyEvent) {
        // Handle results view navigation
    }
}

pub struct SchemaView {
    pub list_state: ListState,
}

impl SchemaView {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            list_state: state,
        }
    }

    pub fn handle_input(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Up => {
                if let Some(selected) = self.list_state.selected() {
                    if selected > 0 {
                        self.list_state.select(Some(selected - 1));
                    }
                }
            }
            KeyCode::Down => {
                if let Some(selected) = self.list_state.selected() {
                    self.list_state.select(Some(selected + 1));
                }
            }
            _ => {}
        }
    }
}

// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100
