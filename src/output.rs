//! Output rendering: a stable JSON envelope for agents and plain text for humans.

use crate::error::YtError;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Copy, Clone, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text.
    Text,
    /// Machine-readable JSON envelope on stdout.
    Json,
    /// SubRip subtitles (transcript only; other commands fall back to text).
    Srt,
    /// WebVTT subtitles (transcript only; other commands fall back to text).
    Vtt,
}

impl OutputFormat {
    pub fn is_json(self) -> bool {
        self == OutputFormat::Json
    }
}

/// Print a successful result. In JSON mode this wraps `data` in
/// `{ "data": ..., "meta": ... }`; in text mode it invokes `text_render`.
pub fn emit<T: Serialize>(
    format: OutputFormat,
    data: &T,
    meta: Value,
    text_render: impl FnOnce() -> String,
) {
    match format {
        OutputFormat::Json => {
            let envelope = json!({ "data": data, "meta": meta });
            println!(
                "{}",
                serde_json::to_string_pretty(&envelope)
                    .unwrap_or_else(|_| "{\"error\":{\"code\":\"error\"}}".into())
            );
        }
        _ => print!("{}", text_render()),
    }
}

/// Print raw text/bytes verbatim to stdout (used for srt/vtt subtitle output).
pub fn emit_raw(s: &str) {
    print!("{s}");
}

/// Print an error to stderr. JSON mode emits `{ "error": { code, message } }`.
pub fn emit_error(format: OutputFormat, err: &YtError) {
    if format.is_json() {
        let v = json!({ "error": { "code": err.code(), "message": err.message() } });
        eprintln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        eprintln!("error[{}]: {}", err.code(), err.message());
    }
}
