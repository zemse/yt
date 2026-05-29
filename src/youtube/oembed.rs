//! oEmbed: keyless title/author/thumbnail lookup.

use super::http_client;
use crate::error::{Result, YtError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct OEmbed {
    pub title: String,
    pub author_name: String,
    pub author_url: String,
    pub thumbnail_url: String,
    #[serde(default)]
    pub provider_name: String,
}

pub async fn fetch(video_id: &str) -> Result<OEmbed> {
    let client = http_client();
    let url = format!(
        "https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v={video_id}&format=json"
    );
    let resp = client.get(&url).send().await?;
    if resp.status() == reqwest::StatusCode::NOT_FOUND
        || resp.status() == reqwest::StatusCode::UNAUTHORIZED
    {
        return Err(YtError::NotFound(format!(
            "video {video_id} not found (or private/embedding disabled)"
        )));
    }
    if !resp.status().is_success() {
        return Err(YtError::Network(format!(
            "oembed returned HTTP {}",
            resp.status()
        )));
    }
    resp.json::<OEmbed>()
        .await
        .map_err(|e| YtError::Other(format!("parse oembed: {e}")))
}
