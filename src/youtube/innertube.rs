//! Unofficial transcript fetching via the InnerTube `player` endpoint +
//! `timedtext` baseUrl. No API key or OAuth required.

use super::{http_client, Segment, Transcript};
use crate::error::{Result, YtError};
use serde_json::{json, Value};

const PLAYER_URL: &str = "https://www.youtube.com/youtubei/v1/player";
// Well-known public web INNERTUBE key; often optional but harmless to send.
const INNERTUBE_KEY: &str = "AIzaSyAO_FJ2SlqU8Q4STEHLGCilw_Y9_11qcW8";

/// (clientName, clientVersion). Tried in order; ANDROID/TV trip fewer bot checks.
const CLIENTS: &[(&str, &str)] = &[
    ("ANDROID", "20.10.38"),
    ("WEB", "2.20240726.00.00"),
    ("TVHTML5_SIMPLY_EMBEDDED_PLAYER", "2.0"),
];

/// Fetch a transcript for `video_id` in `lang`, optionally machine-translated
/// to `translate`.
pub async fn fetch(video_id: &str, lang: &str, translate: Option<&str>) -> Result<Transcript> {
    let client = http_client();
    let mut last_err =
        YtError::Unavailable(format!("no caption tracks found for video {video_id}"));

    for (name, version) in CLIENTS {
        let player = match request_player(&client, video_id, name, version).await {
            Ok(p) => p,
            Err(e) => {
                last_err = e;
                continue;
            }
        };

        if let Some(reason) = playability_error(&player) {
            // A hard playability error won't change across clients.
            return Err(reason);
        }

        let tracks = caption_tracks(&player);
        if tracks.is_empty() {
            continue;
        }

        let (base_url, track_lang, auto) = match pick_track(&tracks, lang) {
            Some(t) => t,
            None => continue,
        };

        match fetch_timedtext(&client, &base_url, translate).await {
            Ok(segments) if !segments.is_empty() => {
                return Ok(Transcript {
                    video_id: video_id.to_string(),
                    language: translate.unwrap_or(&track_lang).to_string(),
                    auto_generated: auto,
                    source: "innertube".to_string(),
                    segments,
                });
            }
            Ok(_) => {
                last_err = YtError::Unavailable(
                    "timedtext returned an empty body (likely PO-token enforcement); \
                     try a residential IP, cookies, or the Invidious fallback"
                        .into(),
                );
            }
            Err(e) => last_err = e,
        }
    }

    Err(last_err)
}

async fn request_player(
    client: &reqwest::Client,
    video_id: &str,
    client_name: &str,
    client_version: &str,
) -> Result<Value> {
    let body = json!({
        "context": { "client": {
            "clientName": client_name,
            "clientVersion": client_version,
            "hl": "en",
            "androidSdkVersion": 30,
        }},
        "videoId": video_id,
    });
    let resp = client
        .post(format!("{PLAYER_URL}?key={INNERTUBE_KEY}"))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(YtError::Network(format!(
            "innertube player returned HTTP {}",
            resp.status()
        )));
    }
    Ok(resp.json::<Value>().await?)
}

/// Returns a hard error if the video itself is unavailable/private.
fn playability_error(player: &Value) -> Option<YtError> {
    let status = player.pointer("/playabilityStatus/status")?.as_str()?;
    match status {
        "OK" => None,
        "ERROR" | "LOGIN_REQUIRED" | "UNPLAYABLE" => {
            let reason = player
                .pointer("/playabilityStatus/reason")
                .and_then(|v| v.as_str())
                .unwrap_or(status);
            Some(if status == "ERROR" {
                YtError::NotFound(reason.to_string())
            } else {
                YtError::Unavailable(reason.to_string())
            })
        }
        _ => None,
    }
}

fn caption_tracks(player: &Value) -> Vec<Value> {
    player
        .pointer("/captions/playerCaptionsTracklistRenderer/captionTracks")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// Pick the best track for `lang`: exact manual > exact auto > any. Returns
/// (baseUrl, languageCode, auto_generated).
fn pick_track(tracks: &[Value], lang: &str) -> Option<(String, String, bool)> {
    let get = |t: &Value| {
        let url = t.get("baseUrl")?.as_str()?.to_string();
        let lc = t
            .get("languageCode")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let auto = t.get("kind").and_then(|v| v.as_str()) == Some("asr");
        Some((url, lc, auto))
    };

    // Exact language, prefer manual (non-asr) first.
    let mut exact: Vec<(String, String, bool)> = tracks
        .iter()
        .filter_map(get)
        .filter(|(_, lc, _)| lc == lang)
        .collect();
    exact.sort_by_key(|(_, _, auto)| *auto); // false (manual) first
    if let Some(t) = exact.into_iter().next() {
        return Some(t);
    }
    // Otherwise the first available track.
    tracks.iter().find_map(get)
}

async fn fetch_timedtext(
    client: &reqwest::Client,
    base_url: &str,
    translate: Option<&str>,
) -> Result<Vec<Segment>> {
    // Strip any existing &fmt= and force json3.
    let mut url = strip_param(base_url, "fmt");
    url.push_str("&fmt=json3");
    if let Some(t) = translate {
        url = strip_param(&url, "tlang");
        url.push_str(&format!("&tlang={t}"));
    }

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(YtError::Network(format!(
            "timedtext returned HTTP {}",
            resp.status()
        )));
    }
    let text = resp.text().await?;
    if text.trim().is_empty() {
        return Ok(Vec::new()); // caller treats empty as PO-token failure
    }
    parse_json3(&text)
}

fn parse_json3(body: &str) -> Result<Vec<Segment>> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| YtError::Other(format!("parse json3 transcript: {e}")))?;
    let events = v
        .get("events")
        .and_then(|e| e.as_array())
        .ok_or_else(|| YtError::Unavailable("transcript had no events".into()))?;

    let mut segments = Vec::new();
    for ev in events {
        let segs = match ev.get("segs").and_then(|s| s.as_array()) {
            Some(s) => s,
            None => continue, // timing/window markers
        };
        let text: String = segs
            .iter()
            .filter_map(|s| s.get("utf8").and_then(|u| u.as_str()))
            .collect();
        let text = text.trim().to_string();
        if text.is_empty() {
            continue;
        }
        let start = ev.get("tStartMs").and_then(|t| t.as_f64()).unwrap_or(0.0) / 1000.0;
        let duration = ev
            .get("dDurationMs")
            .and_then(|t| t.as_f64())
            .unwrap_or(0.0)
            / 1000.0;
        segments.push(Segment {
            start,
            duration,
            text,
        });
    }
    Ok(segments)
}

/// Remove a `&key=...` (or `?key=...`) query parameter from a URL.
fn strip_param(url: &str, key: &str) -> String {
    let (base, query) = match url.split_once('?') {
        Some(x) => x,
        None => return url.to_string(),
    };
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let k = pair.split_once('=').map(|(k, _)| k).unwrap_or(pair);
            k != key
        })
        .collect();
    if kept.is_empty() {
        base.to_string()
    } else {
        format!("{base}?{}", kept.join("&"))
    }
}
