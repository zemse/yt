//! Error type with stable exit codes and a machine-readable error code.

use std::fmt;

/// Stable, agent-facing error categories. The `code()` string is part of the
/// JSON contract; the `exit_code()` is the process exit status.
#[derive(Debug)]
pub enum YtError {
    /// Requested resource (video, caption track, channel…) does not exist.
    NotFound(String),
    /// Missing/invalid credentials or quota exhausted.
    Auth(String),
    /// The resource exists but is not currently obtainable (e.g. transcript
    /// blocked by PO-token enforcement, empty-200 body, no captions).
    Unavailable(String),
    /// A required external binary (yt-dlp, ffmpeg, …) was not found.
    MissingTool(String),
    /// Network / HTTP transport failure.
    Network(String),
    /// Bad user input (unparseable URL/ID, invalid flag combination).
    Input(String),
    /// Anything else.
    Other(String),
}

impl YtError {
    /// Short, stable machine code used in JSON error output.
    pub fn code(&self) -> &'static str {
        match self {
            YtError::NotFound(_) => "not_found",
            YtError::Auth(_) => "auth",
            YtError::Unavailable(_) => "unavailable",
            YtError::MissingTool(_) => "missing_tool",
            YtError::Network(_) => "network",
            YtError::Input(_) => "input",
            YtError::Other(_) => "error",
        }
    }

    /// Process exit code. 0 ok, 1 generic, 2 not-found, 3 auth.
    pub fn exit_code(&self) -> i32 {
        match self {
            YtError::NotFound(_) => 2,
            YtError::Auth(_) => 3,
            _ => 1,
        }
    }

    pub fn message(&self) -> &str {
        match self {
            YtError::NotFound(m)
            | YtError::Auth(m)
            | YtError::Unavailable(m)
            | YtError::MissingTool(m)
            | YtError::Network(m)
            | YtError::Input(m)
            | YtError::Other(m) => m,
        }
    }
}

impl fmt::Display for YtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message())
    }
}

impl std::error::Error for YtError {}

impl From<reqwest::Error> for YtError {
    fn from(e: reqwest::Error) -> Self {
        YtError::Network(e.to_string())
    }
}

impl From<std::io::Error> for YtError {
    fn from(e: std::io::Error) -> Self {
        YtError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, YtError>;
