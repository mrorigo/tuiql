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
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

---

// src/tui/components.rs
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Table, Row, Cell, Gauge},
    Frame,
};

pub struct StatusBar {
    pub database_name: String,
    pub current_mode: String,
    pub message: String,
    pub readonly: bool,
}

impl StatusBar {
    pub fn new() -> Self {
        Self {
            database_name: "No DB".to_string(),
            current_mode: "Editor".to_string(),
            message: "Ready".to_string(),
            readonly: false,
        }
    }

    pub fn render<B: ratatui::backend::Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let readonly_indicator = if self.readonly { " [RO]" } else { "" };
        let status_text = format!(
            "{}{} | Mode: {} | {}",
            self.database_name, readonly_indicator, self.current_mode, self.message
        );

        let paragraph = Paragraph::new(status_text)
            .style(Style::default().bg(Color::DarkGray).fg(Color::White));
        frame.render_widget(paragraph, area);
    }
}

pub struct Navigator {
    pub list_state: ListState,
    pub items: Vec<NavigatorItem>,
    pub expanded: std::collections::HashSet<String>,
}

#[derive(Debug, Clone)]
pub struct NavigatorItem {
    pub id: String,
    pub name: String,
    pub item_type: NavigatorItemType,
    pub level: usize,
    pub parent: Option<String>,
    pub metadata: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigatorItemType {
    Database,
    TablesGroup,
    Table,
    ViewsGroup,
    View,
    IndexesGroup,
    Index,
    TriggersGroup,
    Trigger,
    Column,
}

impl Navigator {
    pub fn new() -> Self {
        Self {
            list_state: ListState::default(),
            items: Vec::new(),
            expanded: std::collections::HashSet::new(),
        }
    }

    pub fn build_tree(&mut self, tables: &[crate::core::TableInfo]) {
        self.items.clear();

        // Add tables group
        self.items.push(NavigatorItem {
            id: "tables_group".to_string(),
            name: format!("Tables ({})", tables.iter().filter(|t| t.table_type == "table").count()),
            item_type: NavigatorItemType::TablesGroup,
            level: 0,
            parent: None,
            metadata: None,
        });

        // Add tables
        if self.expanded.contains("tables_group") {
            for table in tables.iter().filter(|t| t.table_type == "table") {
                self.items.push(NavigatorItem {
                    id: format!("table_{}", table.name),
                    name: table.name.clone(),
                    item_type: NavigatorItemType::Table,
                    level: 1,
                    parent: Some("tables_group".to_string()),
                    metadata: Some(format!("{} cols", table.columns.len())),
                });

                // Add columns if table is expanded
                if self.expanded.contains(&format!("table_{}", table.name)) {
                    for column in &table.columns {
                        let mut col_info = column.data_type.clone();
                        if column.primary_key {
                            col_info.push_str(" [PK]");
                        }
                        if column.not_null {
                            col_info.push_str(" NOT NULL");
                        }

                        self.items.push(NavigatorItem {
                            id: format!("column_{}_{}", table.name, column.name),
                            name: column.name.clone(),
                            item_type: NavigatorItemType::Column,
                            level: 2,
                            parent: Some(format!("table_{}", table.name)),
                            metadata: Some(col_info),
                        });
                    }
                }
            }
        }

        // Add views group
        let view_count = tables.iter().filter(|t| t.table_type == "view").count();
        if view_count > 0 {
            self.items.push(NavigatorItem {
                id: "views_group".to_string(),
                name: format!("Views ({})", view_count),
                item_type: NavigatorItemType::ViewsGroup,
                level: 0,
                parent: None,
                metadata: None,
            });

            if self.expanded.contains("views_group") {
                for view in tables.iter().filter(|t| t.table_type == "view") {
                    self.items.push(NavigatorItem {
                        id: format!("view_{}", view.name),
                        name: view.name.clone(),
                        item_type: NavigatorItemType::View,
                        level: 1,
                        parent: Some("views_group".to_string()),
                        metadata: Some(format!("{} cols", view.columns.len())),
                    });
                }
            }
        }
    }

    pub fn render<B: ratatui::backend::Backend>(&mut self, frame: &mut Frame<B>, area: Rect) {
        let items: Vec<ListItem> = self.items
            .iter()
            .map(|item| {
                let indent = "  ".repeat(item.level);
                let icon = match item.item_type {
                    NavigatorItemType::TablesGroup | NavigatorItemType::ViewsGroup => {
                        if self.expanded.contains(&item.id) { "▼ " } else { "► " }
                    }
                    NavigatorItemType::Table => {
                        if self.expanded.contains(&item.id) { "▼ 📋 " } else { "► 📋 " }
                    }
                    NavigatorItemType::View => "👁 ",
                    NavigatorItemType::Column => "  📄 ",
                    _ => "  ",
                };

                let mut spans = vec![
                    Span::raw(indent),
                    Span::styled(icon, Style::default().fg(Color::Cyan)),
                    Span::raw(&item.name),
                ];

                if let Some(metadata) = &item.metadata {
                    spans.push(Span::styled(
                        format!(" {}", metadata),
                        Style::default().fg(Color::Gray),
                    ));
                }

                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Navigator"))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
            .highlight_symbol("► ");

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    pub fn toggle_expand(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(item) = self.items.get(selected) {
                if self.expanded.contains(&item.id) {
                    self.expanded.remove(&item.id);
                } else {
                    self.expanded.insert(item.id.clone());
                }
            }
        }
    }

    pub fn move_up(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected > 0 {
                self.list_state.select(Some(selected - 1));
            }
        } else if !self.items.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn move_down(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if selected < self.items.len().saturating_sub(1) {
                self.list_state.select(Some(selected + 1));
            }
        } else if !self.items.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn get_selected_item(&self) -> Option<&NavigatorItem> {
        self.list_state.selected().and_then(|i| self.items.get(i))
    }
}

pub struct QueryPlanViewer {
    pub plan: Vec<crate::core::ExplainRow>,
}

impl QueryPlanViewer {
    pub fn new() -> Self {
        Self { plan: Vec::new() }
    }

    pub fn set_plan(&mut self, plan: Vec<crate::core::ExplainRow>) {
        self.plan = plan;
    }

    pub fn render<B: ratatui::backend::Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        if self.plan.is_empty() {
            let paragraph = Paragraph::new("No query plan available. Run a SELECT query first.")
                .block(Block::default().borders(Borders::ALL).title("Query Plan"));
            frame.render_widget(paragraph, area);
            return;
        }

        let plan_text: Vec<String> = self.plan
            .iter()
            .map(|row| {
                let indent = "  ".repeat(row.id as usize);
                format!("{}{}", indent, row.detail)
            })
            .collect();

        let paragraph = Paragraph::new(plan_text.join("\n"))
            .block(Block::default().borders(Borders::ALL).title("Query Plan"))
            .wrap(ratatui::widgets::Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}

pub struct JsonViewer {
    pub content: String,
    pub formatted: bool,
}

impl JsonViewer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            formatted: false,
        }
    }

    pub fn set_content(&mut self, content: String) {
        self.content = content;
        self.formatted = false;
    }

    pub fn format_json(&mut self) -> Result<(), serde_json::Error> {
        if !self.formatted {
            let parsed: serde_json::Value = serde_json::from_str(&self.content)?;
            self.content = serde_json::to_string_pretty(&parsed)?;
            self.formatted = true;
        }
        Ok(())
    }

    pub fn render<B: ratatui::backend::Backend>(&self, frame: &mut Frame<B>, area: Rect) {
        let paragraph = Paragraph::new(self.content.clone())
            .block(Block::default().borders(Borders::ALL).title("JSON View"))
            .wrap(ratatui::widgets::Wrap { trim: false });

        frame.render_widget(paragraph, area);
    }
}

---

// src/tui/events.rs
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
    Query(String),
    QueryResult(crate::core::QueryResult),
    Error(String),
}

pub struct EventHandler {
    sender: mpsc::UnboundedSender<AppEvent>,
    receiver: mpsc::UnboundedReceiver<AppEvent>,
    last_tick: Instant,
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();

        Self {
            sender,
            receiver,
            last_tick: Instant::now(),
            tick_rate,
        }
    }

    pub fn sender(&self) -> mpsc::UnboundedSender<AppEvent> {
        self.sender.clone()
    }

    pub async fn next_event(&mut self) -> Option<AppEvent> {
        let timeout = self.tick_rate.saturating_sub(self.last_tick.elapsed());

        if crossterm::event::poll(Duration::from_millis(0)).unwrap_or(false) {
            match crossterm::event::read() {
                Ok(Event::Key(key)) => return Some(AppEvent::Key(key)),
                Ok(Event::Resize(w, h)) => return Some(AppEvent::Resize(w, h)),
                _ => {}
            }
        }

        if let Ok(event) = tokio::time::timeout(timeout, self.receiver.recv()).await {
            return event;
        }

        if self.last_tick.elapsed() >= self.tick_rate {
            self.last_tick = Instant::now();
            return Some(AppEvent::Tick);
        }

        None
    }
}

pub fn should_quit(event: &AppEvent) -> bool {
    matches!(
        event,
        AppEvent::Key(KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::CONTROL,
            ..
        }) | AppEvent::Key(KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            ..
        })
    )
}

---

// src/app.rs
use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::{interval, Interval};
use tracing::{debug, error, info};

use crate::config::Config;
use crate::core::{Catalog, DatabaseConnection};
use crate::tui::{events::*, TuiApp};

pub struct TuiqlApp {
    config: Config,
    catalog: Catalog,
    event_handler: EventHandler,
}

impl TuiqlApp {
    pub fn new(config: Config, database_path: Option<PathBuf>, readonly: bool) -> Result<Self> {
        let mut catalog = Catalog::new();

        // Open database if provided
        if let Some(path) = database_path {
            let db = DatabaseConnection::open(
                "main".to_string(),
                &path,
                readonly,
                &config.sqlite,
            )?;
            catalog.add_database(db)?;
        }

        let event_handler = EventHandler::new(Duration::from_millis(250));

        Ok(Self {
            config,
            catalog,
            event_handler,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Create TUI app
        let mut tui_app = TuiApp::new(self.catalog.clone());

        let result = self.run_event_loop(&mut terminal, &mut tui_app).await;

        // Cleanup terminal
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;

        result
    }

    async fn run_event_loop<B: ratatui::backend::Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        tui_app: &mut TuiApp,
    ) -> Result<()> {
        let mut should_quit = false;

        while !should_quit {
            // Draw UI
            terminal.draw(|frame| tui_app.render(frame))?;

            // Handle events
            if let Some(event) = self.event_handler.next_event().await {
                match event {
                    AppEvent::Key(key) => {
                        if crate::tui::events::should_quit(&AppEvent::Key(key)) {
                            should_quit = true;
                        } else {
                            tui_app.handle_key_event(key, &mut self.catalog).await?;
                        }
                    }
                    AppEvent::Resize(w, h) => {
                        terminal.resize(ratatui::layout::Rect::new(0, 0, w, h))?;
                    }
                    AppEvent::Tick => {
                        // Handle periodic updates
                    }
                    AppEvent::QueryResult(result) => {
                        tui_app.set_query_result(result);
                    }
                    AppEvent::Error(error) => {
                        tui_app.set_error_message(error);
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

---

// src/sql/mod.rs
pub mod parser;
pub mod formatter;
pub mod completion;

pub use parser::*;
pub use formatter::*;
pub use completion::*;

---

// src/sql/parser.rs
use sqlparser::ast::{Statement, Query, Select, TableFactor};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;
use anyhow::{Result, anyhow};

pub struct SqlAnalyzer {
    dialect: SQLiteDialect,
}

impl SqlAnalyzer {
    pub fn new() -> Self {
        Self {
            dialect: SQLiteDialect {},
        }
    }

    pub fn parse(&self, sql: &str) -> Result<Vec<Statement>> {
        let mut parser = Parser::new(&self.dialect).try_with_sql(sql)?;
        parser.parse_statements().map_err(|e| anyhow!("Parse error: {}", e))
    }

    pub fn is_select_query(&self, sql: &str) -> bool {
        match self.parse(sql) {
            Ok(statements) => {
                statements.iter().any(|stmt| matches!(stmt, Statement::Query(_)))
            }
            Err(_) => false,
        }
    }

    pub fn extract_table_names(&self, sql: &str) -> Vec<String> {
        match self.parse(sql) {
            Ok(statements) => {
                let mut table_names = Vec::new();
                for stmt in statements {
                    self.extract_tables_from_statement(&stmt, &mut table_names);
                }
                table_names.sort();
                table_names.dedup();
                table_names
            }
            Err(_) => Vec::new(),
        }
    }

    fn extract_tables_from_statement(&self, stmt: &Statement, table_names: &mut Vec<String>) {
        match stmt {
            Statement::Query(query) => {
                self.extract_tables_from_query(query, table_names);
            }
            Statement::Insert { table_name, .. } => {
                table_names.push(table_name.to_string());
            }
            Statement::Update { table, .. } => {
                if let TableFactor::Table { name, .. } = &table.relation {
                    table_names.push(name.to_string());
                }
            }
            Statement::Delete { tables, .. } => {
                for table in tables {
                    if let TableFactor::Table { name, .. } = &table.relation {
                        table_names.push(name.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    fn extract_tables_from_query(&self, query: &Query, table_names: &mut Vec<String>) {
        if let sqlparser::ast::SetExpr::Select(select) = &*query.body {
            self.extract_tables_from_select(select, table_names);
        }
    }

    fn extract_tables_from_select(&self, select: &Select, table_names: &mut Vec<String>) {
        for table in &select.from {
            if let TableFactor::Table { name, .. } = &table.relation {
                table_names.push(name.to_string());
            }

            for join in &table.joins {
                if let TableFactor::Table { name, .. } = &join.relation {
                    table_names.push(name.to_string());
                }
            }
        }
    }

    pub fn validate_query(&self, sql: &str) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        match self.parse(sql) {
            Ok(statements) => {
                for stmt in statements {
                    match stmt {
                        Statement::Update { selection: None, .. } => {
                            warnings.push("UPDATE without WHERE clause - affects all rows!".to_string());
                        }
                        Statement::Delete { selection: None, .. } => {
                            warnings.push("DELETE without WHERE clause - deletes all rows!".to_string());
                        }
                        _ => {}
                    }
                }
                Ok(warnings)
            }
            Err(e) => Err(e),
        }
    }
}

---

// src/sql/completion.rs
use crate::core::{Catalog, TableInfo, ColumnInfo};
use std::collections::HashSet;

pub struct SqlCompleter {
    keywords: HashSet<String>,
    functions: HashSet<String>,
    pragmas: HashSet<String>,
}

impl SqlCompleter {
    pub fn new() -> Self {
        let mut keywords = HashSet::new();
        // Add SQLite keywords
        for keyword in &[
            "SELECT", "FROM", "WHERE", "INSERT", "UPDATE", "DELETE", "CREATE", "DROP",
            "ALTER", "TABLE", "INDEX", "VIEW", "TRIGGER", "BEGIN", "COMMIT", "ROLLBACK",
            "DISTINCT", "ORDER", "GROUP", "HAVING", "LIMIT", "OFFSET", "JOIN", "INNER",
            "LEFT", "RIGHT", "OUTER", "ON", "USING", "UNION", "INTERSECT", "EXCEPT",
            "AS", "AND", "OR", "NOT", "NULL", "IS", "LIKE", "GLOB", "MATCH", "REGEXP",
            "BETWEEN", "IN", "EXISTS", "CASE", "WHEN", "THEN", "ELSE", "END", "CAST",
            "PRIMARY", "KEY", "FOREIGN", "REFERENCES", "UNIQUE", "CHECK", "DEFAULT",
            "AUTOINCREMENT", "COLLATE", "ASC", "DESC", "IF", "NOT", "EXISTS",
        ] {
            keywords.insert(keyword.to_string());
        }

        let mut functions = HashSet::new();
        // Add SQLite functions
        for func in &[
            "COUNT", "SUM", "AVG", "MIN", "MAX", "LENGTH", "SUBSTR", "UPPER", "LOWER",
            "TRIM", "LTRIM", "RTRIM", "REPLACE", "ROUND", "ABS", "RANDOM", "DATE",
            "TIME", "DATETIME", "STRFTIME", "JULIANDAY", "JSON_EXTRACT", "JSON_ARRAY",
            "JSON_OBJECT", "COALESCE", "NULLIF", "TYPEOF", "LAST_INSERT_ROWID",
        ] {
            functions.insert(func.to_string());
        }

        let mut pragmas = HashSet::new();
        // Add SQLite pragmas
        for pragma in &[
            "table_info", "index_list", "foreign_key_list", "database_list",
            "table_list", "foreign_keys", "journal_mode", "page_size",
            "cache_size", "synchronous", "auto_vacuum", "encoding",
        ] {
            pragmas.insert(pragma.to_string());
        }

        Self {
            keywords,
            functions,
            pragmas,
        }
    }

    pub fn get_completions(&self, input: &str, catalog: &Catalog) -> Vec<String> {
        let mut completions = Vec::new();
        let input_upper = input.to_uppercase();

        // Add matching keywords
        for keyword in &self.keywords {
            if keyword.starts_with(&input_upper) {
                completions.push(keyword.clone());
            }
        }

        // Add matching functions
        for function in &self.functions {
            if function.starts_with(&input_upper) {
                completions.push(format!("{}()", function));
            }
        }

        // Add table names
        for table_name in catalog.get_table_names(None) {
            if table_name.to_uppercase().starts_with(&input_upper) {
                completions.push(table_name);
            }
        }

        // Add column names if we can determine the context
        if let Some(tables) = catalog.get_tables(None) {
            for table in tables {
                for column in &table.columns {
                    if column.name.to_uppercase().starts_with(&input_upper) {
                        completions.push(column.name.clone());
                    }
                }
            }
        }

        // Add pragmas if input starts with PRAGMA
        if input_upper.starts_with("PRAGMA ") {
            let pragma_part = &input_upper[7..];
            for pragma in &self.pragmas {
                if pragma.starts_with(pragma_part) {
                    completions.push(format!("PRAGMA {}", pragma));
                }
            }
        }

        completions.sort();
        completions.dedup();
        completions
    }
}

---

// src/storage/mod.rs
pub mod history;
pub mod snippets;
pub mod exports;

pub use history::*;
pub use snippets::*;
pub use exports::*;

---

// src/storage/history.rs
use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryHistoryEntry {
    pub id: i64,
    pub database_name: String,
    pub query: String,
    pub executed_at: DateTime<Utc>,
    pub duration_ms: f64,
    pub success: bool,
    pub error_message: Option<String>,
}

pub struct QueryHistory {
    connection: Connection,
}

impl QueryHistory {
    pub fn new() -> Result<Self> {
        let history_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?
            .join("tuiql");

        std::fs::create_dir_all(&history_dir)?;
        let db_path = history_dir.join("history.sqlite");

        let connection = Connection::open(db_path)?;

        // Create tables if they don't exist
        connection.execute(
            "CREATE TABLE IF NOT EXISTS query_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                database_name TEXT NOT NULL,
                query TEXT NOT NULL,
                executed_at TEXT NOT NULL,
                duration_ms REAL NOT NULL,
                success INTEGER NOT NULL,
                error_message TEXT
            )",
            [],
        )?;

        connection.execute(
            "CREATE INDEX IF NOT EXISTS idx_history_database_time
             ON query_history(database_name, executed_at DESC)",
            [],
        )?;

        Ok(Self { connection })
    }

    pub fn add_entry(&mut self, entry: QueryHistoryEntry) -> Result<i64> {
        let id = self.connection.execute(
            "INSERT INTO query_history
             (database_name, query, executed_at, duration_ms, success, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                entry.database_name,
                entry.query,
                entry.executed_at.to_rfc3339(),
                entry.duration_ms,
                if entry.success { 1 } else { 0 },
                entry.error_message,
            ],
        )?;

        Ok(self.connection.last_insert_rowid())
    }

    pub fn get_recent(&self, database_name: Option<&str>, limit: usize) -> Result<Vec<QueryHistoryEntry>> {
        let (sql, params): (String, Vec<&dyn rusqlite::ToSql>) = match database_name {
            Some(db) => (
                "SELECT id, database_name, query, executed_at, duration_ms, success, error_message
                 FROM query_history WHERE database_name = ?1
                 ORDER BY executed_at DESC LIMIT ?2".to_string(),
                vec![&db, &(limit as i64)],
            ),
            None => (
                "SELECT id, database_name, query, executed_at, duration_ms, success, error_message
                 FROM query_history ORDER BY executed_at DESC LIMIT ?1".to_string(),
                vec![&(limit as i64)],
            ),
        };

        let mut stmt = self.connection.prepare(&sql)?;
        let entries = stmt.query_map(&params[..], |row| {
            Ok(QueryHistoryEntry {
                id: row.get(0)?,
                database_name: row.get(1)?,
                query: row.get(2)?,
                executed_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(3)?)
                    .unwrap()
                    .with_timezone(&Utc),
                duration_ms: row.get(4)?,
                success: row.get::<_, i32>(5)? != 0,
                error_message: row.get(6)?,
            })
        })?;

        let mut result = Vec::new();
        for entry in entries {
            result.push(entry?);
        }

        Ok(result)
    }

    pub fn
