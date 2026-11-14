//! Serialisable command errors.

use serde::Serialize;
use std::fmt;

/// Error returned to the UI. `code` is stable and machine-readable; `message`
/// is short technical copy. Neither ever carries a credential.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("not_found", message)
    }

    pub fn io(context: &str, err: &std::io::Error) -> Self {
        Self::new("io", format!("{context}: {err}"))
    }

    pub fn data(context: &str, err: &serde_json::Error) -> Self {
        Self::new("invalid_data", format!("{context}: {err}"))
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CommandError {}

pub type CommandResult<T> = Result<T, CommandError>;
