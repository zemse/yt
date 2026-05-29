//! Invidious fallback for transcripts (returns WebVTT, no API key/OAuth).

use super::{http_client, Segment, Transcript};
use crate::error::{Result, YtError};
use serde::Deserialize;

#[derive(Deserialize)]
struct CaptionList {
    captions: Vec<CaptionEntry>,
}

#[derive(Deserialize)]
struct CaptionEntry {
    #[serde(default)]
    #[serde(rename = "languageCode")]
    language_code: String,
    url: String,
}

/// Fetch a transcript via an Invidious instance.
pub async fn fetch(instance: &str, video_id: &str, lang: &str) -> Result<Transcript> {
    let client = http_client();
    let base = instance.trim_end_matches('/');

    let list_url = format!("{base}/api/v1/captions/{video_id}");
    let resp = client.get(&list_url).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(YtError::NotFound(format!(
            "Invidious has no captions for {video_id}"
        )));
    }
    if !resp.status().is_success() {
        return Err(YtError::Network(format!(
            "Invidious caption list returned HTTP {}",
            resp.status()
        )));
    }
    let list: CaptionList = resp
        .json()
        .await
        .map_err(|e| YtError::Other(format!("parse Invidious caption list: {e}")))?;

    if list.captions.is_empty() {
        return Err(YtError::Unavailable(format!(
            "no captions available for {video_id}"
        )));
    }

    let entry = list
        .captions
        .iter()
        .find(|c| c.language_code == lang)
        .unwrap_or(&list.captions[0]);
    let track_lang = if entry.language_code.is_empty() {
        lang.to_string()
    } else {
        entry.language_code.clone()
    };

    // entry.url is a path relative to the instance.
    let vtt_url = if entry.url.starts_with("http") {
        entry.url.clone()
    } else {
        format!("{base}{}", entry.url)
    };
    let vtt = client.get(&vtt_url).send().await?.text().await?;
    let segments = parse_vtt(&vtt);
    if segments.is_empty() {
        return Err(YtError::Unavailable(
            "Invidious returned an empty transcript".into(),
        ));
    }

    Ok(Transcript {
        video_id: video_id.to_string(),
        language: track_lang,
        auto_generated: false,
        source: "invidious".to_string(),
        segments,
    })
}

/// Minimal WebVTT parser → segments.
fn parse_vtt(vtt: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut lines = vtt.lines().peekable();
    while let Some(line) = lines.next() {
        let Some((start_s, end_s)) = line.split_once("-->") else {
            continue;
        };
        let Some(start) = parse_ts(start_s.trim()) else {
            continue;
        };
        // End may have trailing cue settings ("00:00:03.500 align:start").
        let end = end_s
            .split_whitespace()
            .next()
            .and_then(parse_ts)
            .unwrap_or(start);

        let mut text_parts = Vec::new();
        while let Some(peek) = lines.peek() {
            if peek.trim().is_empty() {
                break;
            }
            text_parts.push(lines.next().unwrap().trim().to_string());
        }
        let text = strip_tags(&text_parts.join(" "));
        if !text.is_empty() {
            segments.push(Segment {
                start,
                duration: (end - start).max(0.0),
                text,
            });
        }
    }
    segments
}

/// Parse `HH:MM:SS.mmm` or `MM:SS.mmm` into seconds.
fn parse_ts(s: &str) -> Option<f64> {
    let s = s.trim();
    let (hms, ms) = s.split_once('.').unwrap_or((s, "0"));
    let parts: Vec<&str> = hms.split(':').collect();
    let (h, m, sec) = match parts.as_slice() {
        [h, m, s] => (h.parse::<f64>().ok()?, m.parse().ok()?, s.parse().ok()?),
        [m, s] => (0.0, m.parse::<f64>().ok()?, s.parse::<f64>().ok()?),
        _ => return None,
    };
    let frac: f64 = format!("0.{ms}").parse().unwrap_or(0.0);
    Some(h * 3600.0 + m * 60.0 + sec + frac)
}

/// Remove simple `<...>` markup that some VTT tracks include.
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}
