use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LauncherError {
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("unsupported installation: {0}")]
    Unsupported(String),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("invalid metadata: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, LauncherError>;
