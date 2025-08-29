
# Agent Instructions

## Checklist

- [ ] Keep track of your plan, work and progress in docs/PLAN.md and docs/STATUS.md. Document critical decisions and actions.
- [ ] Work independently until the next agent can take over. Do not pause or stop for any reason until one iteration is complete.
- [ ] ALWAYS perform a git commit after every file change (git add -A && git commit -m ""). Use conventional commit messages.
- [ ] Use idiomatic rust, and write tests for everything.

## Documentation

- Product Requirements are in docs/PRD.md
- Current plan is in docs/PLAN.md
- Current status is in docs/STATUS.md

## Tests

- Write tests for all new code and existing code that is modified.
- Run tests before committing changes.

## Error Handling Guidelines

### Standardized TuiqlError System

The TUIQL codebase uses a comprehensive error categorization system to improve maintainability and AI comprehension. **ALWAYS use structured errors instead of `Result<T, String>`.**

### Error Categories

1. **TuiqlError::Database** - SQLite/Database-level errors
   - Connection failures, SQL syntax errors, constraint violations
   - Use: `Err(TuiqlError::Database(rusqlite::Error))`

2. **TuiqlError::Query** - SQL query and data manipulation errors
   - Query validation, execution errors, field validation
   - Examples: Empty queries, validation failures, data type mismatches

3. **TuiqlError::Config** - Configuration loading errors
   - File reading errors, TOML parsing errors, invalid settings
   - Use for configuration file operations

4. **TuiqlError::Schema** - Schema-related operations
   - Schema discovery, diff operations, metadata errors
   - Table/constraint validation errors

5. **TuiqlError::Ui** - User interface/display errors
   - Export format failures, display rendering issues
   - User-facing functionality errors

6. **TuiqlError::Json** - JSON processing errors
   - Parsing failures, serialization errors, malformed data
   - Use: `TuiqlError::Jason(serde_json::Error)`

7. **TuiqlError::Command** - Command execution errors
   - Invalid commands, missing parameters, execution failures
   - REPL command validation

### Migration Pattern

**WHEN MODIFYING FUNCTIONS WITH `Result<T, String>`:**

```rust
// BEFORE (Avoid):
pub fn do_something() -> Result<String, String> {
    return Err("error message".to_string());
}

// AFTER (Required):
use crate::core::{Result, TuiqlError};

pub fn do_something() -> Result<String> {
    return Err(TuiqlError::Category("enhanced error with context and guidance".to_string()));
}
```

### Error Message Guidelines

- **Provide context** about what operation failed
- **Include relevant values** (field names, command names, file paths)
- **Suggest remedies** when possible
- **Use consistent terminology** across similar error types

### Test Updates Required

**WHEN UPDATING ERROR-HANDLING CODE, UPDATE CORRESPONDING TESTS:**

```rust
// BEFORE:
let result = function_call_that_can_fail();
assert!(result.is_err());
assert_eq!(result.unwrap_err(), "old error message");

// AFTER:
let result = function_call_that_can_fail();
assert!(result.is_err());
if let Err(TuiqlError::Category(msg)) = result {
    assert!(msg.contains("expected content"));
    assert!(msg.contains("helpful context"));
} else {
    panic!("Expected Category error");
}
```

## Commit Messages

- Use conventional commit messages.
- Include a description of the changes in the commit message.


## Workflow

- Read the Product Requirements (docs/PRD.md)
- Read the current plan (docs/PLAN.md) and status (docs/STATUS.md)
- Figure out what to do next
- Implement the next step
- Test the implementation
- Update the plan (docs/PLAN.md)
- Update the status (docs/STATUS.md)
- Repeat the workflow until the product is complete.
