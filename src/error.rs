use std::io;
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PatrolError {
    #[error("root directory does not exist: {0}")]
    MissingRoot(PathBuf),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("io error: {0}")]
    Io(#[from] io::Error),

    #[error("walkdir error: {0}")]
    Walkdir(#[from] walkdir::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("git command failed for repo {repo}: {message}")]
    GitCommand { repo: String, message: String },

    #[error("timeout running git command for repo {repo}: {command}")]
    GitTimeout { repo: String, command: String },

    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("fancy-regex error: {0}")]
    FancyRegex(#[from] fancy_regex::Error),
}

pub type Result<T> = std::result::Result<T, PatrolError>;
