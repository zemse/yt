//! YouTube data sources: unofficial InnerTube, Invidious fallback, official
//! Data API v3, and oEmbed.

pub mod dataapi;
pub mod innertube;
pub mod invidious;
pub mod oembed;

use crate::error::Result;
use serde::Serialize;

/// Fetch a transcript via InnerTube, falling back to Invidious on failure.
pub async fn fetch_transcript(
    invidious_instance: &str,
    video_id: &str,
    lang: &str,
    translate: Option<&str>,
) -> Result<Transcript> {
    match innertube::fetch(video_id, lang, translate).await {
        Ok(t) => Ok(t),
        Err(primary) => match invidious::fetch(invidious_instance, video_id, lang).await {
            Ok(t) => Ok(t),
            // Surface the original (usually more informative) error.
            Err(_) => Err(primary),
        },
    }
}

/// A single timed transcript segment.
#[derive(Debug, Clone, Serialize)]
pub struct Segment {
    /// Start time in seconds.
    pub start: f64,
    /// Duration in seconds.
    pub duration: f64,
    pub text: String,
}

/// A fetched transcript plus provenance.
#[derive(Debug, Clone, Serialize)]
pub struct Transcript {
    pub video_id: String,
    pub language: String,
    /// True if these are auto-generated (ASR) captions.
    pub auto_generated: bool,
    /// Which backend produced this ("innertube" | "invidious").
    pub source: String,
    pub segments: Vec<Segment>,
}

impl Transcript {
    /// Concatenate all segment text into a single string.
    pub fn full_text(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
    (KHTML, like Gecko) Chrome/124.0 Safari/537.36";

/// Shared HTTP client with a browser-like user agent.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(UA)
        .build()
        .expect("failed to build reqwest client")
}

/// Render seconds as `HH:MM:SS,mmm` (SRT) or `HH:MM:SS.mmm` (VTT).
pub fn fmt_timestamp(seconds: f64, comma: bool) -> String {
    let total_ms = (seconds * 1000.0).round() as u64;
    let ms = total_ms % 1000;
    let s = (total_ms / 1000) % 60;
    let m = (total_ms / 60_000) % 60;
    let h = total_ms / 3_600_000;
    let sep = if comma { ',' } else { '.' };
    format!("{h:02}:{m:02}:{s:02}{sep}{ms:03}")
}
