//! Parse a YouTube video ID from a full URL, short URL, or bare ID.

use crate::error::{Result, YtError};

/// Extract an 11-character video ID from any of:
/// - `https://www.youtube.com/watch?v=ID`
/// - `https://youtu.be/ID`
/// - `https://www.youtube.com/shorts/ID` / `/embed/ID` / `/live/ID`
/// - a bare `ID`
pub fn parse_video_id(input: &str) -> Result<String> {
    let s = input.trim();

    // Bare ID (no scheme, no slash, no query).
    if is_video_id(s) {
        return Ok(s.to_string());
    }

    // Strip scheme for easier matching.
    let rest = s
        .strip_prefix("https://")
        .or_else(|| s.strip_prefix("http://"))
        .unwrap_or(s);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);

    // youtu.be/ID
    if let Some(after) = rest.strip_prefix("youtu.be/") {
        return take_id(after);
    }

    if rest.starts_with("youtube.com/") || rest.starts_with("m.youtube.com/") {
        let path = rest.split_once('/').map(|x| x.1).unwrap_or("");

        // watch?v=ID
        if let Some(q) = path.strip_prefix("watch") {
            if let Some(v) = query_param(q, "v") {
                return take_id(&v);
            }
        }
        // /shorts/ID, /embed/ID, /live/ID, /v/ID
        for prefix in ["shorts/", "embed/", "live/", "v/"] {
            if let Some(after) = path.strip_prefix(prefix) {
                return take_id(after);
            }
        }
    }

    Err(YtError::Input(format!(
        "could not parse a YouTube video ID from {input:?}"
    )))
}

/// True if `s` looks like a canonical 11-char video ID.
fn is_video_id(s: &str) -> bool {
    s.len() == 11
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Take the leading ID from a path/query fragment, dropping any trailing
/// `?`, `&`, `/`, or `#`.
fn take_id(s: &str) -> Result<String> {
    let id: String = s
        .chars()
        .take_while(|&c| c != '?' && c != '&' && c != '/' && c != '#')
        .collect();
    if is_video_id(&id) {
        Ok(id)
    } else {
        Err(YtError::Input(format!("invalid video ID: {id:?}")))
    }
}

/// Find `key` in a `?a=b&c=d` style query string.
fn query_param(query: &str, key: &str) -> Option<String> {
    let q = query.strip_prefix('?').unwrap_or(query);
    q.split('&').find_map(|pair| {
        let (k, v) = pair.split_once('=')?;
        (k == key).then(|| v.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_id() {
        assert_eq!(parse_video_id("BgMP3Bx1q10").unwrap(), "BgMP3Bx1q10");
    }

    #[test]
    fn watch_url() {
        assert_eq!(
            parse_video_id("https://www.youtube.com/watch?v=BgMP3Bx1q10").unwrap(),
            "BgMP3Bx1q10"
        );
    }

    #[test]
    fn watch_url_extra_params() {
        assert_eq!(
            parse_video_id("https://youtube.com/watch?v=BgMP3Bx1q10&t=42s&list=PL").unwrap(),
            "BgMP3Bx1q10"
        );
    }

    #[test]
    fn short_url() {
        assert_eq!(
            parse_video_id("https://youtu.be/BgMP3Bx1q10?si=abc").unwrap(),
            "BgMP3Bx1q10"
        );
    }

    #[test]
    fn shorts_and_embed() {
        assert_eq!(
            parse_video_id("https://www.youtube.com/shorts/BgMP3Bx1q10").unwrap(),
            "BgMP3Bx1q10"
        );
        assert_eq!(
            parse_video_id("http://youtube.com/embed/BgMP3Bx1q10").unwrap(),
            "BgMP3Bx1q10"
        );
    }

    #[test]
    fn rejects_junk() {
        assert!(parse_video_id("not a url").is_err());
        assert!(parse_video_id("https://example.com/watch?v=BgMP3Bx1q10").is_err());
    }
}
