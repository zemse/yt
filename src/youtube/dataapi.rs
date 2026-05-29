//! Official YouTube Data API v3 client (read-only, API key).

use super::http_client;
use crate::error::{Result, YtError};
use serde::Serialize;
use serde_json::Value;

const BASE: &str = "https://www.googleapis.com/youtube/v3";

#[derive(Debug, Serialize)]
pub struct VideoMeta {
    pub id: String,
    pub title: String,
    pub channel: String,
    pub channel_id: String,
    pub published_at: String,
    pub duration: String,
    pub view_count: Option<u64>,
    pub like_count: Option<u64>,
    pub comment_count: Option<u64>,
    pub description: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SearchItem {
    pub kind: String,
    pub id: String,
    pub title: String,
    pub channel: String,
    pub published_at: String,
}

#[derive(Debug, Serialize)]
pub struct ChannelMeta {
    pub id: String,
    pub title: String,
    pub subscriber_count: Option<u64>,
    pub video_count: Option<u64>,
    pub view_count: Option<u64>,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct Comment {
    pub author: String,
    pub text: String,
    pub like_count: u64,
    pub published_at: String,
}

#[derive(Debug, Serialize)]
pub struct PlaylistItem {
    pub position: u64,
    pub video_id: String,
    pub title: String,
    pub channel: String,
}

async fn get(client: &reqwest::Client, path: &str, params: &[(&str, &str)]) -> Result<Value> {
    let resp = client
        .get(format!("{BASE}/{path}"))
        .query(params)
        .send()
        .await?;
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(body);
    }
    let msg = body
        .pointer("/error/message")
        .and_then(|v| v.as_str())
        .unwrap_or("Data API request failed")
        .to_string();
    Err(match status.as_u16() {
        400 | 401 | 403 => YtError::Auth(msg),
        404 => YtError::NotFound(msg),
        _ => YtError::Network(format!("Data API HTTP {status}: {msg}")),
    })
}

fn num(v: &Value, ptr: &str) -> Option<u64> {
    v.pointer(ptr)
        .and_then(|x| x.as_str())
        .and_then(|s| s.parse().ok())
}

fn str_at(v: &Value, ptr: &str) -> String {
    v.pointer(ptr)
        .and_then(|x| x.as_str())
        .unwrap_or("")
        .to_string()
}

pub async fn get_video(key: &str, id: &str) -> Result<VideoMeta> {
    let client = http_client();
    let body = get(
        &client,
        "videos",
        &[
            ("part", "snippet,contentDetails,statistics"),
            ("id", id),
            ("key", key),
        ],
    )
    .await?;
    let item = body
        .pointer("/items/0")
        .ok_or_else(|| YtError::NotFound(format!("video {id} not found")))?;
    Ok(VideoMeta {
        id: id.to_string(),
        title: str_at(item, "/snippet/title"),
        channel: str_at(item, "/snippet/channelTitle"),
        channel_id: str_at(item, "/snippet/channelId"),
        published_at: str_at(item, "/snippet/publishedAt"),
        duration: str_at(item, "/contentDetails/duration"),
        view_count: num(item, "/statistics/viewCount"),
        like_count: num(item, "/statistics/likeCount"),
        comment_count: num(item, "/statistics/commentCount"),
        description: str_at(item, "/snippet/description"),
        tags: item
            .pointer("/snippet/tags")
            .and_then(|t| t.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

pub async fn search(key: &str, query: &str, limit: u32) -> Result<Vec<SearchItem>> {
    let client = http_client();
    let max = limit.min(50).to_string();
    let body = get(
        &client,
        "search",
        &[
            ("part", "snippet"),
            ("q", query),
            ("type", "video"),
            ("maxResults", &max),
            ("key", key),
        ],
    )
    .await?;
    let items = body
        .get("items")
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items
        .iter()
        .map(|it| SearchItem {
            kind: str_at(it, "/id/kind"),
            id: str_at(it, "/id/videoId"),
            title: str_at(it, "/snippet/title"),
            channel: str_at(it, "/snippet/channelTitle"),
            published_at: str_at(it, "/snippet/publishedAt"),
        })
        .collect())
}

pub async fn get_channel(key: &str, id: &str) -> Result<ChannelMeta> {
    let client = http_client();
    // Accept either a UC… id or an @handle.
    let param: (&str, &str) = if let Some(handle) = id.strip_prefix('@') {
        ("forHandle", handle)
    } else if id.starts_with('@') {
        ("forHandle", id)
    } else {
        ("id", id)
    };
    let body = get(
        &client,
        "channels",
        &[
            ("part", "snippet,statistics"),
            (param.0, param.1),
            ("key", key),
        ],
    )
    .await?;
    let item = body
        .pointer("/items/0")
        .ok_or_else(|| YtError::NotFound(format!("channel {id} not found")))?;
    Ok(ChannelMeta {
        id: str_at(item, "/id"),
        title: str_at(item, "/snippet/title"),
        subscriber_count: num(item, "/statistics/subscriberCount"),
        video_count: num(item, "/statistics/videoCount"),
        view_count: num(item, "/statistics/viewCount"),
        description: str_at(item, "/snippet/description"),
    })
}

pub async fn comments(key: &str, video_id: &str, limit: u32) -> Result<Vec<Comment>> {
    let client = http_client();
    let max = limit.min(100).to_string();
    let body = get(
        &client,
        "commentThreads",
        &[
            ("part", "snippet"),
            ("videoId", video_id),
            ("maxResults", &max),
            ("order", "relevance"),
            ("key", key),
        ],
    )
    .await?;
    let items = body
        .get("items")
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items
        .iter()
        .map(|it| {
            let top = "/snippet/topLevelComment/snippet";
            Comment {
                author: str_at(it, &format!("{top}/authorDisplayName")),
                text: str_at(it, &format!("{top}/textDisplay")),
                like_count: num(it, &format!("{top}/likeCount")).unwrap_or(0),
                published_at: str_at(it, &format!("{top}/publishedAt")),
            }
        })
        .collect())
}

pub async fn playlist_items(key: &str, playlist_id: &str, limit: u32) -> Result<Vec<PlaylistItem>> {
    let client = http_client();
    let max = limit.min(50).to_string();
    let body = get(
        &client,
        "playlistItems",
        &[
            ("part", "snippet"),
            ("playlistId", playlist_id),
            ("maxResults", &max),
            ("key", key),
        ],
    )
    .await?;
    let items = body
        .get("items")
        .and_then(|i| i.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items
        .iter()
        .map(|it| PlaylistItem {
            position: num(it, "/snippet/position").unwrap_or(0),
            video_id: str_at(it, "/snippet/resourceId/videoId"),
            title: str_at(it, "/snippet/title"),
            channel: str_at(it, "/snippet/videoOwnerChannelTitle"),
        })
        .collect())
}
