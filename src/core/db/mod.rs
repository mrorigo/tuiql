/// Database Module
///
/// This module provides the core database functionality for TUIQL,
/// organized into focused submodules for better maintainability and
/// separation of concerns.
///
/// ## Architecture
///
/// The database layer is split into three main concerns:
/// - **Connection Management** (`connection.rs`): Handles database connections, state, and transactions
/// - **Schema Introspection** (`schema.rs`): Provides metadata about database structure
/// - **Query Execution** (`query.rs`): Handles SQL query execution and result formatting
///
/// ## Error Handling
///
/// All database operations use the standardized `TuiqlError` type for consistent error propagation.
///
/// ## Usage
///
/// Most common operations should use the higher-level functions from this module,
/// which automatically handle connection state and error conversion.
pub mod connection;
pub mod query;
pub mod schema;

pub use connection::*;
pub use query::*;
pub use schema::*;