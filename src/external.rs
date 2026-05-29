//! Wrappers around external binaries: yt-dlp, ffmpeg, whisper. We shell out
//! rather than reimplement YouTube extraction (see RESEARCH.md §2.2).

use crate::error::{Result, YtError};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

/// Locate a binary on PATH or return a MissingTool error with an install hint.
pub fn find(tool: &str, hint: &str) -> Result<PathBuf> {
    which::which(tool)
        .map_err(|_| YtError::MissingTool(format!("`{tool}` not found on PATH. {hint}")))
}

fn ytdlp() -> Result<PathBuf> {
    find(
        "yt-dlp",
        "Install it from https://github.com/yt-dlp/yt-dlp (e.g. `brew install yt-dlp`).",
    )
}

fn ffmpeg() -> Result<PathBuf> {
    find("ffmpeg", "Install it (e.g. `brew install ffmpeg`).")
}

/// Whether to show child progress on our stderr.
fn stderr_for(quiet: bool) -> Stdio {
    if quiet {
        Stdio::null()
    } else {
        Stdio::inherit()
    }
}

/// Run a command, capturing stdout; stderr streams to the user (unless quiet).
async fn run_capture(mut cmd: Command, quiet: bool, what: &str) -> Result<String> {
    cmd.stdout(Stdio::piped()).stderr(stderr_for(quiet));
    let out = cmd
        .output()
        .await
        .map_err(|e| YtError::Other(format!("failed to run {what}: {e}")))?;
    if !out.status.success() {
        return Err(YtError::Other(format!("{what} exited with {}", out.status)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Common yt-dlp args for auth/robustness.
fn common_args(cmd: &mut Command, cookies_from_browser: Option<&str>) {
    if let Some(b) = cookies_from_browser {
        cmd.arg("--cookies-from-browser").arg(b);
    }
    // Prefer the mweb/tv clients which avoid some PO-token requirements.
    cmd.arg("--extractor-args")
        .arg("youtube:player_client=default,mweb");
}

pub struct DownloadOpts<'a> {
    pub video_id: &'a str,
    pub audio_only: bool,
    pub quality: &'a str,
    pub output: Option<&'a str>,
    pub cookies_from_browser: Option<&'a str>,
    pub quiet: bool,
}

/// Download a video (or audio). Returns the final file path.
pub async fn download(opts: DownloadOpts<'_>) -> Result<String> {
    let bin = ytdlp()?;
    let url = format!("https://www.youtube.com/watch?v={}", opts.video_id);
    let template = opts
        .output
        .map(String::from)
        .unwrap_or_else(|| "%(title).80s [%(id)s].%(ext)s".to_string());

    let mut cmd = Command::new(bin);
    common_args(&mut cmd, opts.cookies_from_browser);
    if opts.audio_only {
        cmd.arg("-x");
    } else if opts.quality != "best" {
        // e.g. "720p" -> height cap
        let h = opts.quality.trim_end_matches('p');
        cmd.arg("-f")
            .arg(format!("bv*[height<={h}]+ba/b[height<={h}]"));
    }
    cmd.arg("-o")
        .arg(&template)
        .arg("--no-simulate")
        .arg("--print")
        .arg("after_move:filepath")
        .arg("--no-playlist")
        .arg(&url);

    let path = run_capture(cmd, opts.quiet, "yt-dlp").await?;
    if path.is_empty() {
        return Err(YtError::Other(
            "yt-dlp did not report an output path".into(),
        ));
    }
    Ok(path.lines().last().unwrap_or(&path).to_string())
}

/// Download audio and produce a 16 kHz mono WAV (Whisper-ready). Returns path.
pub async fn download_audio_wav(
    video_id: &str,
    output: Option<&str>,
    cookies_from_browser: Option<&str>,
    quiet: bool,
) -> Result<String> {
    let bin = ytdlp()?;
    ffmpeg()?; // yt-dlp's wav postprocessing needs ffmpeg
    let url = format!("https://www.youtube.com/watch?v={video_id}");
    let template = output
        .map(String::from)
        .unwrap_or_else(|| format!("{video_id}.%(ext)s"));

    let mut cmd = Command::new(bin);
    common_args(&mut cmd, cookies_from_browser);
    cmd.arg("-x")
        .arg("--audio-format")
        .arg("wav")
        .arg("--postprocessor-args")
        .arg("ffmpeg:-ar 16000 -ac 1")
        .arg("-o")
        .arg(&template)
        .arg("--no-simulate")
        .arg("--print")
        .arg("after_move:filepath")
        .arg("--no-playlist")
        .arg(&url);

    let path = run_capture(cmd, quiet, "yt-dlp").await?;
    Ok(path.lines().last().unwrap_or(&path).to_string())
}

/// Extract frames from a local video file at `fps`. Returns the frame count.
pub async fn extract_frames(video_path: &str, fps: &str, out_dir: &str) -> Result<usize> {
    let bin = ffmpeg()?;
    tokio::fs::create_dir_all(out_dir).await?;
    let pattern = format!("{out_dir}/frame_%05d.jpg");
    let mut cmd = Command::new(bin);
    cmd.arg("-i")
        .arg(video_path)
        .arg("-vf")
        .arg(format!("fps={fps}"))
        .arg("-y")
        .arg(&pattern)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = cmd
        .status()
        .await
        .map_err(|e| YtError::Other(format!("failed to run ffmpeg: {e}")))?;
    if !status.success() {
        return Err(YtError::Other(format!("ffmpeg exited with {status}")));
    }
    let mut count = 0;
    let mut rd = tokio::fs::read_dir(out_dir).await?;
    while let Some(e) = rd.next_entry().await? {
        if e.file_name().to_string_lossy().starts_with("frame_") {
            count += 1;
        }
    }
    Ok(count)
}

/// Transcribe an audio file by shelling out to a whisper CLI, if present.
pub async fn whisper_transcribe(audio_path: &str, quiet: bool) -> Result<String> {
    // Try the OpenAI `whisper` CLI, then whisper.cpp's `whisper-cli`.
    if let Ok(bin) = find("whisper", "") {
        let mut cmd = Command::new(bin);
        cmd.arg(audio_path)
            .arg("--model")
            .arg("base")
            .arg("--output_format")
            .arg("txt")
            .arg("--output_dir")
            .arg(".");
        run_capture(cmd, quiet, "whisper").await?;
        let txt = audio_path
            .rsplit_once('.')
            .map(|(s, _)| s)
            .unwrap_or(audio_path);
        let path = format!("{txt}.txt");
        return Ok(tokio::fs::read_to_string(&path).await.unwrap_or_default());
    }
    Err(YtError::MissingTool(
        "no whisper CLI found (install openai-whisper or whisper.cpp) — \
         transcript fallback unavailable"
            .into(),
    ))
}
