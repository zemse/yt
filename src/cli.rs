//! Command-line interface definition (clap derive).

use crate::output::OutputFormat;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "yt",
    version,
    about = "YouTube CLI: transcripts, downloads, and metadata — for humans and AI agents",
    propagate_version = true
)]
pub struct Cli {
    /// Output format. `srt`/`vtt` apply to `transcript` only.
    #[arg(long, value_enum, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Suppress progress/diagnostic output on stderr.
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Disable ANSI color (also respects the NO_COLOR env var).
    #[arg(long, global = true)]
    pub no_color: bool,

    /// YouTube Data API key (overrides YT_API_KEY and ~/.yt).
    #[arg(long, global = true, env = "YT_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// Invidious instance base URL used as a fallback.
    #[arg(long, global = true)]
    pub invidious_instance: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Fetch a video's transcript (no API key required).
    Transcript {
        /// Video URL, youtu.be link, or bare 11-char ID.
        video: String,
        /// Preferred caption language (ISO-639-1, e.g. en).
        #[arg(long, default_value = "en")]
        lang: String,
        /// Machine-translate the transcript to this language code.
        #[arg(long)]
        translate: Option<String>,
        /// Omit per-segment timestamps from text output (shown by default).
        #[arg(long = "no-timestamps", action = clap::ArgAction::SetFalse, default_value_t = true)]
        timestamps: bool,
    },

    /// Show video metadata (Data API if a key is set, else oEmbed).
    Info { video: String },

    /// Download a video via yt-dlp.
    Download {
        video: String,
        /// Download audio only.
        #[arg(long)]
        audio_only: bool,
        /// Quality selector passed to yt-dlp (e.g. best, 720p).
        #[arg(long, default_value = "best")]
        quality: String,
        /// Output file path/template.
        #[arg(long, short)]
        output: Option<String>,
        /// Use cookies from an installed browser (chrome, firefox, …).
        #[arg(long)]
        cookies_from_browser: Option<String>,
    },

    /// Download audio and convert to 16 kHz mono WAV (Whisper-ready).
    Audio {
        video: String,
        #[arg(long, short)]
        output: Option<String>,
        #[arg(long)]
        cookies_from_browser: Option<String>,
    },

    /// Search YouTube (Data API; costs 100 quota units; key required).
    Search {
        query: String,
        #[arg(long, default_value_t = 10)]
        limit: u32,
    },

    /// Show channel info (Data API; key required).
    Channel {
        /// Channel ID (UC…) or @handle.
        id: String,
    },

    /// List comments on a video (Data API; key required).
    Comments {
        video: String,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },

    /// List items in a playlist (Data API; key required).
    Playlist {
        id: String,
        #[arg(long, default_value_t = 50)]
        limit: u32,
    },

    /// Extract frames from a video (downloads then runs ffmpeg).
    Frames {
        video: String,
        /// Frames per second to sample.
        #[arg(long, default_value = "1")]
        fps: String,
        /// Output directory.
        #[arg(long, short, default_value = "frames")]
        output: String,
        #[arg(long)]
        cookies_from_browser: Option<String>,
    },

    /// Build an LLM-ready bundle: transcript (or Whisper) + metadata [+ frames].
    Analyze {
        video: String,
        /// Also extract frames.
        #[arg(long)]
        frames: bool,
        /// Frames per second when --frames is set.
        #[arg(long, default_value = "0.2")]
        fps: String,
        /// Force audio download + Whisper even if a transcript exists.
        #[arg(long)]
        whisper: bool,
        #[arg(long)]
        cookies_from_browser: Option<String>,
    },

    /// Manage the ~/.yt config file.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
    /// Set a config field (api-key, invidious-instance, cookies-from-browser).
    Set { field: String, value: String },
    /// Get a config field (secrets are masked).
    Get { field: String },
    /// Print the config file path.
    Path,
}
