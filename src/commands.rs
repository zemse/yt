//! Command handlers. Each resolves input, calls a data source, and emits
//! output in the requested format.

use crate::cli::ConfigAction;
use crate::config::{self, Config};
use crate::error::Result;
use crate::external::{self, DownloadOpts};
use crate::id::parse_video_id;
use crate::output::{self, OutputFormat};
use crate::youtube::{self, dataapi, fmt_clock, fmt_timestamp, oembed, Transcript};
use serde_json::json;

/// `yt transcript`
pub async fn transcript(
    cfg: &Config,
    fmt: OutputFormat,
    video: &str,
    lang: &str,
    translate: Option<&str>,
    timestamps: bool,
) -> Result<()> {
    let id = parse_video_id(video)?;
    let t = youtube::fetch_transcript(&cfg.invidious_instance, &id, lang, translate).await?;
    match fmt {
        OutputFormat::Json => output::emit(
            fmt,
            &t,
            json!({
                "source": t.source,
                "language": t.language,
                "auto_generated": t.auto_generated,
                "segment_count": t.segments.len(),
            }),
            String::new,
        ),
        OutputFormat::Srt => output::emit_raw(&render_srt(&t)),
        OutputFormat::Vtt => output::emit_raw(&render_vtt(&t)),
        OutputFormat::Text => output::emit_raw(&render_text(&t, timestamps)),
    }
    Ok(())
}

fn render_text(t: &Transcript, timestamps: bool) -> String {
    let mut out = String::new();
    for s in &t.segments {
        if timestamps {
            out.push_str(&format!("[{}] {}\n", fmt_clock(s.start), s.text));
        } else {
            out.push_str(&s.text);
            out.push('\n');
        }
    }
    out
}

fn render_srt(t: &Transcript) -> String {
    let mut out = String::new();
    for (i, s) in t.segments.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n{}\n\n",
            i + 1,
            fmt_timestamp(s.start, true),
            fmt_timestamp(s.start + s.duration, true),
            s.text
        ));
    }
    out
}

fn render_vtt(t: &Transcript) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for s in &t.segments {
        out.push_str(&format!(
            "{} --> {}\n{}\n\n",
            fmt_timestamp(s.start, false),
            fmt_timestamp(s.start + s.duration, false),
            s.text
        ));
    }
    out
}

/// `yt info`
pub async fn info(cfg: &Config, fmt: OutputFormat, video: &str) -> Result<()> {
    let id = parse_video_id(video)?;
    if let Some(key) = cfg.api_key.as_deref() {
        let v = dataapi::get_video(key, &id).await?;
        output::emit(fmt, &v, json!({ "source": "data_api" }), || {
            format!(
                "{}\n  channel:   {}\n  published: {}\n  duration:  {}\n  views:     {}\n  likes:     {}\n  comments:  {}\n",
                v.title,
                v.channel,
                v.published_at,
                v.duration,
                v.view_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                v.like_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
                v.comment_count.map(|n| n.to_string()).unwrap_or_else(|| "—".into()),
            )
        });
    } else {
        let o = oembed::fetch(&id).await?;
        output::emit(fmt, &o, json!({ "source": "oembed" }), || {
            format!(
                "{}\n  channel: {}\n  (set an API key for views/likes/duration)\n",
                o.title, o.author_name
            )
        });
    }
    Ok(())
}

/// `yt download`
pub async fn download(
    fmt: OutputFormat,
    quiet: bool,
    video: &str,
    audio_only: bool,
    quality: &str,
    output_path: Option<&str>,
    cookies: Option<&str>,
) -> Result<()> {
    let id = parse_video_id(video)?;
    let path = external::download(DownloadOpts {
        video_id: &id,
        audio_only,
        quality,
        output: output_path,
        cookies_from_browser: cookies,
        quiet,
    })
    .await?;
    let data = json!({ "path": path, "video_id": id });
    output::emit(fmt, &data, json!({ "tool": "yt-dlp" }), || {
        format!("saved: {path}\n")
    });
    Ok(())
}

/// `yt audio`
pub async fn audio(
    fmt: OutputFormat,
    quiet: bool,
    video: &str,
    output_path: Option<&str>,
    cookies: Option<&str>,
) -> Result<()> {
    let id = parse_video_id(video)?;
    let path = external::download_audio_wav(&id, output_path, cookies, quiet).await?;
    let data = json!({ "path": path, "video_id": id, "format": "wav", "sample_rate": 16000 });
    output::emit(fmt, &data, json!({ "tool": "yt-dlp+ffmpeg" }), || {
        format!("saved 16kHz mono wav: {path}\n")
    });
    Ok(())
}

/// `yt search`
pub async fn search(cfg: &Config, fmt: OutputFormat, query: &str, limit: u32) -> Result<()> {
    let key = config::require_api_key(cfg)?;
    let items = dataapi::search(key, query, limit).await?;
    output::emit(
        fmt,
        &items,
        json!({ "source": "data_api", "quota_cost": 100 }),
        || {
            let mut s = String::new();
            for it in &items {
                s.push_str(&format!("{}  {}  ({})\n", it.id, it.title, it.channel));
            }
            s
        },
    );
    Ok(())
}

/// `yt channel`
pub async fn channel(cfg: &Config, fmt: OutputFormat, id: &str) -> Result<()> {
    let key = config::require_api_key(cfg)?;
    let c = dataapi::get_channel(key, id).await?;
    output::emit(fmt, &c, json!({ "source": "data_api" }), || {
        format!(
            "{}\n  id:          {}\n  subscribers: {}\n  videos:      {}\n  views:       {}\n",
            c.title,
            c.id,
            c.subscriber_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".into()),
            c.video_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".into()),
            c.view_count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "—".into()),
        )
    });
    Ok(())
}

/// `yt comments`
pub async fn comments(cfg: &Config, fmt: OutputFormat, video: &str, limit: u32) -> Result<()> {
    let key = config::require_api_key(cfg)?;
    let id = parse_video_id(video)?;
    let cs = dataapi::comments(key, &id, limit).await?;
    output::emit(fmt, &cs, json!({ "source": "data_api" }), || {
        let mut s = String::new();
        for c in &cs {
            s.push_str(&format!("{} (♥{}): {}\n", c.author, c.like_count, c.text));
        }
        s
    });
    Ok(())
}

/// `yt playlist`
pub async fn playlist(cfg: &Config, fmt: OutputFormat, id: &str, limit: u32) -> Result<()> {
    let key = config::require_api_key(cfg)?;
    let items = dataapi::playlist_items(key, id, limit).await?;
    output::emit(fmt, &items, json!({ "source": "data_api" }), || {
        let mut s = String::new();
        for it in &items {
            s.push_str(&format!(
                "{:>3}. {}  {}\n",
                it.position + 1,
                it.video_id,
                it.title
            ));
        }
        s
    });
    Ok(())
}

/// `yt frames`
pub async fn frames(
    fmt: OutputFormat,
    quiet: bool,
    video: &str,
    fps: &str,
    out_dir: &str,
    cookies: Option<&str>,
) -> Result<()> {
    let id = parse_video_id(video)?;
    let video_path = external::download(DownloadOpts {
        video_id: &id,
        audio_only: false,
        quality: "best",
        output: Some(&format!("{id}.%(ext)s")),
        cookies_from_browser: cookies,
        quiet,
    })
    .await?;
    let count = external::extract_frames(&video_path, fps, out_dir).await?;
    let data = json!({ "video_id": id, "dir": out_dir, "fps": fps, "frame_count": count });
    output::emit(fmt, &data, json!({ "tool": "yt-dlp+ffmpeg" }), || {
        format!("extracted {count} frames at {fps} fps to {out_dir}/\n")
    });
    Ok(())
}

/// `yt analyze` — assemble an LLM-ready bundle.
#[allow(clippy::too_many_arguments)]
pub async fn analyze(
    cfg: &Config,
    fmt: OutputFormat,
    quiet: bool,
    video: &str,
    want_frames: bool,
    fps: &str,
    force_whisper: bool,
    cookies: Option<&str>,
) -> Result<()> {
    let id = parse_video_id(video)?;

    // Metadata (best-effort; don't fail the whole bundle if it's missing).
    let metadata = if let Some(key) = cfg.api_key.as_deref() {
        dataapi::get_video(key, &id).await.ok().map(|v| json!(v))
    } else {
        oembed::fetch(&id).await.ok().map(|o| json!(o))
    };

    // Transcript: prefer captions unless --whisper forces audio transcription.
    let transcript = if force_whisper {
        whisper_block(&id, cookies, quiet).await?
    } else {
        match youtube::fetch_transcript(&cfg.invidious_instance, &id, "en", None).await {
            Ok(t) => json!({
                "source": t.source,
                "language": t.language,
                "auto_generated": t.auto_generated,
                "text": t.full_text(),
                "segments": t.segments,
            }),
            Err(_) => whisper_block(&id, cookies, quiet).await?,
        }
    };

    // Optional frames.
    let frames_block = if want_frames {
        let video_path = external::download(DownloadOpts {
            video_id: &id,
            audio_only: false,
            quality: "best",
            output: Some(&format!("{id}.%(ext)s")),
            cookies_from_browser: cookies,
            quiet,
        })
        .await?;
        let dir = format!("{id}_frames");
        let count = external::extract_frames(&video_path, fps, &dir).await?;
        Some(json!({ "dir": dir, "fps": fps, "frame_count": count }))
    } else {
        None
    };

    let bundle = json!({
        "video_id": id,
        "metadata": metadata,
        "transcript": transcript,
        "frames": frames_block,
    });
    output::emit(fmt, &bundle, json!({ "kind": "analysis_bundle" }), || {
        let words = transcript
            .get("text")
            .and_then(|t| t.as_str())
            .map(|s| s.split_whitespace().count())
            .unwrap_or(0);
        format!("analysis bundle for {id}: transcript ~{words} words\n")
    });
    Ok(())
}

async fn whisper_block(id: &str, cookies: Option<&str>, quiet: bool) -> Result<serde_json::Value> {
    let wav =
        external::download_audio_wav(id, Some(&format!("{id}.%(ext)s")), cookies, quiet).await?;
    let text = external::whisper_transcribe(&wav, quiet).await?;
    Ok(json!({ "source": "whisper", "language": "en", "auto_generated": true, "text": text }))
}

/// `yt config <action>`
pub fn config_cmd(fmt: OutputFormat, action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Set { field, value } => {
            let path = config::set_field(field, value)?;
            let data = json!({ "set": field, "path": path });
            output::emit(fmt, &data, json!({}), || {
                format!("set {field} in {}\n", path.display())
            });
        }
        ConfigAction::Get { field } => {
            let val = config::get_field(field)?;
            let display = val.as_deref().map(|v| {
                if field.contains("key") {
                    config::mask(v)
                } else {
                    v.to_string()
                }
            });
            let data = json!({ "field": field, "value": display });
            output::emit(fmt, &data, json!({}), || match &display {
                Some(v) => format!("{field} = {v}\n"),
                None => format!("{field} is not set\n"),
            });
        }
        ConfigAction::Path => {
            let path = config::config_path();
            let data = json!({ "path": path });
            output::emit(fmt, &data, json!({}), || format!("{}\n", path.display()));
        }
    }
    Ok(())
}
