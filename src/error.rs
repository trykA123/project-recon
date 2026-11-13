//! Defines typed failures shared by SQLite operations and the CLI JSON boundary.

use serde_json::{Value, json};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid input: {0}")]
    Invalid(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Source changed: {0}")]
    SourceChanged(String),
    #[error("The knowledge map changed; prepare a new Librarian request")]
    StaleMap,
    #[error("Invalid Librarian route: {0}")]
    InvalidRoute(String),
    #[error("Essential response exceeds the requested output budget")]
    Budget,
    #[error("Database operation failed: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("File operation failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid JSON input: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Maps a typed failure to a stable machine-readable protocol code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) | Self::Json(_) => "invalid_input",
            Self::NotFound(_) => "not_found",
            Self::Conflict(_) => "conflict",
            Self::SourceChanged(_) => "source_changed",
            Self::StaleMap => "stale_map",
            Self::InvalidRoute(_) => "invalid_route",
            Self::Budget => "budget_exceeded",
            Self::Database(_) => "storage_error",
            Self::Io(_) => "io_error",
        }
    }

    /// Serializes one failure without exposing a backtrace or request payload.
    pub fn envelope(&self) -> Value {
        json!({"schemaVersion":1,"ok":false,"error":{"code":self.code(),"message":self.to_string()}})
    }
}

/// Wraps successful command data in the versioned CLI protocol.
pub fn envelope(data: Value) -> Value {
    json!({"schemaVersion":1,"ok":true,"data":data})
}
