//! One error type for the whole crate, built with [`thiserror`].

use std::path::PathBuf;

use thiserror::Error;

/// Everything that can go wrong in `chrono-tasker`.
///
/// Each variant carries just enough data to produce a helpful message.
#[derive(Debug, Error)]
pub enum TaskError {
    #[error(
        "could not understand the date '{0}' \
         (try 2026-12-25, today, tomorrow, +3d, +2w, +1m, or a weekday like fri)"
    )]
    InvalidDate(String),

    #[error("no task with id {0}")]
    NotFound(u32),

    #[error("task {0} is already done")]
    AlreadyDone(u32),

    #[error("the task title cannot be empty")]
    EmptyTitle,

    #[error("unknown priority '{0}' (use low, medium or high)")]
    InvalidPriority(String),

    #[error("unknown repeat rule '{0}' (use daily, weekly or monthly)")]
    InvalidRecurrence(String),

    #[error("could not access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not read {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

/// Shorthand used across the crate.
pub type Result<T> = std::result::Result<T, TaskError>;
