# Product Requirements Document (PRD)

## 1) Vision

A blazing‑fast, terminal‑native, keyboard‑centric SQLite client that makes **schema comprehension**, **data navigation**, and **query iteration** effortless. Think: `sqlite3`’s speed with a delightful, discoverable TUI that scales from quick one‑offs to day‑long analysis sessions.

## 2) Target users

* Backend & data engineers who live in terminals.
* App developers shipping embedded SQLite.
* SREs/debuggers inspecting on‑device or production snapshots.
* Power users who prefer a REPL over GUIs.

## 3) Non‑goals

* Replace full DB IDEs; no WYSIWYG ER modeling.
* Compete with multi‑engine SQL studios; SQLite‑first.

---

## 4) Prior art & gaps (summary)

* `sqlite3` CLI: rock‑solid, but low discovery, minimal schema viz.
* `litecli`: completions & syntax highlight, but limited structural browsing.
* GUI tools (DB Browser/SQLiteStudio): visual, but mouse‑heavy; weak in headless/SSH.
  **Gap:** first‑class **schema map**, **record inspector**, **JSON/FTS helpers**, and **EXPLAIN visualizer** in a TUI with modern editor UX.

---

## 5) Core use cases

1. **Open any DB fast**: `tuiql path/to.db` or attach multiple DBs.
2. **Understand schema**: see tables, columns, FKs, indexes, constraints; jump by key.
3. **Iterate queries**: autocomplete, lint, run, view, save snippets, diff results.
4. **Inspect records**: row grid ⇄ JSON tree ⇄ wide text; edit/insert with safeguards.
5. **Profile & optimize**: `EXPLAIN` + cost/plan tree + timing + pragma toggles.
6. **Search everything**: FTS5 helper, fuzzy search across tables/columns/saved queries.
7. **Migrate & compare**: compare schemas (A vs B), generate `ALTER`/DDL diff.
8. **Offline snapshot work**: work on `.db` copies, export/import, `.dump` management.

---

## 6) Success metrics

* Time‑to‑first‑result (open → run → view) < 2s on commodity laptops.
* 90% commands accessible via keyboard cheatsheet; median 3 keystrokes to execute last query.
* < 10ms TUI frame budget; scroll 100k rows @ ≥ 30 FPS (virtualized paging).
* Crash‑free sessions: P99 zero panics across 10k sessions.

---

## 7) Product surface (TUI IA)

### Global layout

* **Top:** statusline (db path, txn mode, connection, page size, pragma highlights).
* **Left pane (Navigator):** DBs → Schemas → Tables/Views → Columns/Indexes → Triggers.
* **Center pane (Main):** *focus changes by mode*

  * Query editor / Results grid / JSON viewer / Plan graph / Schema map / Diff.
* **Bottom:** command palette / message log / REPL prompt.

### Modes (single‑key toggle)

* `E`: Editor, `R`: Results, `S`: Schema map, `P`: Plan, `D`: Diff, `H`: History, `G`: Graph search.

### Command palette

* `Ctrl+P`: fuzzy actions ("Run", "Attach DB", "Toggle FK checks", "Export CSV"…)

---

## 8) Key TUI components (first release)

1. **Connection & DB switcher**

* Open/attach multiple DBs; recent list; pragmas preview (page\_size, journal\_mode).

2. **Schema Navigator**

* Tree with badges: rowcount est, PK/FK indicators, index count, FTS/virtual table tags.
* Quick filters: tables/views/indexes/triggers.

3. **Schema Map (ER‑like graph)**

* Auto‑layout graph from FK metadata; focus+follow edges; collapse unrelated clusters; grouping by schema and highlighting circular references.
* Node detail popover (columns, types, constraints, indexes).

4. **Query Editor**

* Multiline edit, syntax highlight, bracket/quote balance, format (`:Fmt`).
* Completions (tables/columns/functions/pragmas), signature help, snippets.
* Lint: dangerous ops alert (implicit full table UPDATE/DELETE without WHERE), `BEGIN…COMMIT` guard, implicit JOIN detection.

5. **Results Grid**

* Virtualized table; sticky header; sort client‑side; type‑aware cells (NULL, BLOB, JSON).
* Inline JSON tree, text wrap, copy cell/row, export (CSV/JSON/Markdown).

6. **Record Inspector**

* Focus a row → vertical card view; edit with type validation; preview UPDATE/INSERT; optimized for large records.

7. **Plan Visualizer**

* Render `EXPLAIN QUERY PLAN` as a tree with cost/loops/rows; highlight index usage.

8. **Extensions & Helpers**

* **JSON1** helper panel: extract/flatten; query builder for `json_each/ json_tree`.
* **FTS5** helper: create index, highlight matches, snippet preview.

9. **History, Snippets & Macros**

* Per‑DB history/tags; pin queries; macro recorder (replay with params), scheduled reruns (manual trigger).

10. **Diff / Compare**

* Schema diff between DBs; show DDL changes; data preview diff (key‑based sample).

---

## 9) Keyboard model

* Vim‑like by default (hjkl nav, `:` for command), Emacs optional.
* Global: `F5` run, `Shift+F5` run selection; `Alt+Enter` run in new result tab.
* Editor: `]d` next diagnostic, `gc` toggle comment, `K` docs/pragma help.
* Grid: arrows/`hjkl`, `Enter` inspect, `y` yank cell/row, `*` search in column.

---

## 10) Data & safety model

* Auto open in **autocommit**; prompt to start explicit txn for multi‑statement edits.
* **Safe edit** mode (guard rails): require WHERE on UPDATE/DELETE unless confirmed.
* Undo buffer for data changes within txn; diff preview before commit.
* Read‑only toggle per connection.

---

## 11) Technical design

### 11.1 Architecture (modules)

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

### 11.2 Dependencies (initial)

* Rendering: `ratatui` + `crossterm`.
* DB: `rusqlite` (bundled `libsqlite3` via feature) + `libsqlite3-sys`.
* Parsing/analysis: `sqlparser-rs` (Generic dialect + SQLite tweaks), formatter (custom/simple initially).
* REPL/editor: `reedline` (completions, history, multiline, keymaps).
* Table rendering: `tui` grid + `comfy-table` (for exports/print‑only).
* Config: `serde` + `toml`; Logging: `tracing` + `tracing-subscriber`.
* Optional: `fdlimit` (macOS), `rayon` for CPU‑bound formatting/diff, `petgraph` for schema graphs.

### 11.3 SQLite features supported Day‑1

* Pragmas: `table_info`, `index_list`, `foreign_key_list`, `page_size`, `journal_mode`, `foreign_keys`.
* Extensions surfaced: `json1`, `fts5` (detect & expose helpers if available).
* Explainers: `EXPLAIN` and `EXPLAIN QUERY PLAN` parsing to graph.

### 11.4 Catalog discovery

* On open/attach: read `sqlite_master` + pragmas; build cache:

  * Tables/views: columns (name, type, notnull, pk, default), rowid/without rowid.
  * Indexes: unique, origin (auto vs explicit), covered columns.
  * FKs: from → to mapping; deferrable; actions.
* Live invalidation on DDL; cheap refresh per object.

### 11.5 Execution engine

* Single connection per DB by default; serialized queue of statements.
* Cancellable long‑running queries (interrupt API); statement timeout.
* Streaming result reader → virtualized grid; BLOB hex preview on demand.

### 11.6 Editor & analysis pipeline

* Parse on idle; surface errors inline; detect DDL vs DML; suggest `CREATE INDEX` based on plan.
* Format/lint minimal viable; later integrate community formatter hook.

### 11.7 Plan visualizer

* Parse `EXPLAIN QUERY PLAN` rows; build tree; display cost/rows; highlight table scans.

### 11.8 Schema map

* Build graph from FK edges; auto‑layout (force‑directed offline); focus table; filter components.

### 11.9 Storage

* `$XDG_STATE_HOME/tuiql/` (history.sqlite), `$XDG_CONFIG_HOME/tuiql/config.toml`, `snippets/`.
* Export directory configurable; safe overwrite rules.

### 11.10 Extensibility

* **Command plugins** (dynamic or WASI): register palette commands, panels, exporters.
* **Render hooks**: custom cell renderers (e.g., image blobs → size summary).
* **Virtual tables**: assist loading `fts5`, `csv`, `dbstat`, `json_each` views.

### 11.11 Cross‑platform

* macOS/Linux/WSL; Windows Terminal supported; 80x24 min. No daemon.

### 11.12 Performance notes

* Avoid locking UI thread on I/O; spawn worker for DB ops; channel back rows in chunks.
* Virtual scrolling window; cap cell render cost; lazily decode JSON/BLOB.
* Benchmark suites on sample DBs (1M rows, 100 tables) with P95 latencies tracked.

### 11.13 Telemetry & privacy

* **Opt‑in** only. Metrics: feature usage, perf timings, crash reports. No SQL/PII.

---

## 12) Commands (initial)

```
:open <path>            # open db
:attach <name> <path>   # attach db as schema
:ro / :rw               # toggle read-only (per connection)
:pragma <name> [val]    # view/set pragma, with safe allowlist
:plan                   # run EXPLAIN QUERY PLAN for current statement
:fmt                    # format current buffer
:export [csv|json|md]   # export current result set
:find <text>            # search across table/column names
:erd [table]            # open schema map, focus table
:hist                   # history panel
:snip {save|run}        # manage snippets
:diff <dbA> <dbB>       # schema diff
```

---

## 13) Config (TOML)

````toml
[ui]
theme = "dark"
show_status_tips = true

[keys]
run = "F5"
run_selection = "S-F5"
vim_mode = true

[sqlite]
load_extensions = ["/usr/lib/sqlite3/fts5.so"]
page_size_hint = 4096
"""

---

## 14) Example SQL used internally
```sql
-- columns
SELECT * FROM pragma_table_info(?1);
-- indexes for a table
SELECT * FROM pragma_index_list(?1);
-- foreign keys for a table
SELECT * FROM pragma_foreign_key_list(?1);
-- explain plan
EXPLAIN QUERY PLAN SELECT * FROM main.sqlite_master;
````

---

## 15) Testing strategy

* Unit: catalog parsers, plan tree builder, diff engine.
* Golden tests: render snapshots for widgets (ratatui test harness).
* Integration: run against bundled sample DBs; headless runs for REPL commands.
* Property tests: round‑trip DDL diff → DDL apply → re‑compare.

---

## 16) Packaging & distribution

* Static builds (`bundled`) + `--features system-sqlite`.
* Homebrew, AUR, Scoop; single binary `tuiql`.

---

## 17) Roadmap

**M0 (2–3 weeks)**: Skeleton app, open DB, nav tree, run SQL, grid (basic), history.

**M1**: Completions, schema cache, record inspector, exports, JSON tree, plan basic.

**M2**: Schema map, FTS5/JSON1 helpers, diff, cancellable queries, config.

**M3**: Plugins, plan cost overlays, dangerous‑op lints, ER auto‑layout polish.

---

## 18) Risks & mitigations

* Terminal portability → depend on `crossterm`, thorough CI matrix.
* Very large result sets → strict streaming & pagination policy.
* SQLite extension availability differs → feature detection & graceful degradation.

---

# Appendix A — Keybindings (default)

| Action               | Keys            |
| -------------------- | --------------- |
| Command palette      | Ctrl+P          |
| Run                  | F5              |
| Run selection        | Shift+F5        |
| Toggle mode          | E/R/S/P/D/H/G   |
| Focus navigator/main | Ctrl+1 / Ctrl+2 |
| Inspect row          | Enter           |
| Copy cell/row        | y               |
| Search in column     | \*              |

---

# Appendix B — Minimal Rust snippets (illustrative)

```rust
// Open connection, set safe pragmas
let conn = Connection::open(path)?;
conn.pragma_update(None, "foreign_keys", &"ON")?;
conn.pragma_update(None, "journal_mode", &"WAL")?;

// Introspect columns
let mut stmt = conn.prepare("SELECT * FROM pragma_table_info(?1)")?;
let cols = stmt.query_map([table_name], |row| ColumnInfo { /* ... */ })?;

// Cancellable query (interrupt flag wired to UI)
let db = conn.handle();
let ctrlc = interrupt_flag.clone();
std::thread::spawn(move || {
    if ctrlc.wait() { unsafe { sqlite3_interrupt(db); } }
});
```

```rust
// Reedline with basic completions (tables/columns)
let mut rl = Reedline::create()?
    .with_edit_mode(Box::new(Vi::default()))
    .with_completer(Box::new(SqlCompleter::new(catalog)));
```

---

# Appendix C — Plugin concept (WASI sketch)

* `tuiql --plugin install gh:user/ext-json-flatten`
* Plugin declares: commands, panels, exporters via manifest; communicates over stdio JSON‑RPC.

---

# Appendix D — Screen sketches (text)

```
┌ Navigator ──────────────────────┐  ┌ Query Editor ───────────────────────────────┐
│ main ▾                          │  │ SELECT u.id, u.name, count(p.id)            │
│  tables (23)                    │  │ FROM user u LEFT JOIN post p USING(user_id) │
│   user      [PK] (FK→org)       │  │ WHERE u.active = 1                          │
│   post      [FK→user]           │  │ GROUP BY 1,2;                               │
│   org       [PK]                │  └──────────────────────────────────────────────┘
│  views (2)                      │  ┌ Results ────────────────────────────────────┐
│  indexes (14)                   │  │ id │ name     │ posts │ …                   │
│  triggers (1)                   │  │  1 │ "Ada"    │   12  │                     │
└─────────────────────────────────┘  └──────────────────────────────────────────────┘
```
