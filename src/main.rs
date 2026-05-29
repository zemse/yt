//! `yt` — a CLI for YouTube transcripts, downloads, and metadata.
//!
//! Designed to be driven equally well by humans and AI agents: every command
//! supports `--format json` with a stable envelope and predictable exit codes.

mod cli;
mod commands;
mod config;
mod error;
mod external;
mod id;
mod output;
mod youtube;

use clap::Parser;
use cli::{Cli, Commands};
use error::Result;

fn main() {
    let args = Cli::parse();
    let format = args.format;

    let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
    let result = rt.block_on(run(args));

    if let Err(err) = result {
        output::emit_error(format, &err);
        std::process::exit(err.exit_code());
    }
}

async fn run(args: Cli) -> Result<()> {
    let cfg = config::resolve(args.api_key.clone(), args.invidious_instance.clone());
    let fmt = args.format;
    let quiet = args.quiet;

    match &args.command {
        Commands::Transcript {
            video,
            lang,
            translate,
            timestamps,
        } => commands::transcript(&cfg, fmt, video, lang, translate.as_deref(), *timestamps).await,

        Commands::Info { video } => commands::info(&cfg, fmt, video).await,

        Commands::Download {
            video,
            audio_only,
            quality,
            output,
            cookies_from_browser,
        } => {
            commands::download(
                fmt,
                quiet,
                video,
                *audio_only,
                quality,
                output.as_deref(),
                cookies_from_browser.as_deref(),
            )
            .await
        }

        Commands::Audio {
            video,
            output,
            cookies_from_browser,
        } => {
            commands::audio(
                fmt,
                quiet,
                video,
                output.as_deref(),
                cookies_from_browser.as_deref(),
            )
            .await
        }

        Commands::Search { query, limit } => commands::search(&cfg, fmt, query, *limit).await,

        Commands::Channel { id } => commands::channel(&cfg, fmt, id).await,

        Commands::Comments { video, limit } => commands::comments(&cfg, fmt, video, *limit).await,

        Commands::Playlist { id, limit } => commands::playlist(&cfg, fmt, id, *limit).await,

        Commands::Frames {
            video,
            fps,
            output,
            cookies_from_browser,
        } => {
            commands::frames(
                fmt,
                quiet,
                video,
                fps,
                output,
                cookies_from_browser.as_deref(),
            )
            .await
        }

        Commands::Analyze {
            video,
            frames,
            fps,
            whisper,
            cookies_from_browser,
        } => {
            commands::analyze(
                &cfg,
                fmt,
                quiet,
                video,
                *frames,
                fps,
                *whisper,
                cookies_from_browser.as_deref(),
            )
            .await
        }

        Commands::Config { action } => commands::config_cmd(fmt, action),
    }
}
