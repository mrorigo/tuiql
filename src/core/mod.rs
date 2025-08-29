/// Core Module for TUIQL
///
/// This module contains the fundamental components and utilities that form
/// the backbone of the TUIQL application. It provides shared infrastructure
/// for database operations, error handling, configuration management, and
/// other core functionality.

pub mod db;
pub mod error;

// Re-export commonly used types for convenience
pub use error::{TuiqlError, Result, CommandResult};