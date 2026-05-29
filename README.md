# yt

A command-line tool for YouTube — **transcripts**, **video/audio downloads** (for LLM analysis), and **metadata** — designed to be driven equally well by humans and AI agents.

> ⚠️ **v0.0.1 is an early scaffold and is not yet functional.** This release reserves the crate name and publishes the project skeleton. See [`PLAN.md`](PLAN.md) for the roadmap and [`RESEARCH.md`](RESEARCH.md) for the API research behind it.

## Planned features

- `yt transcript <video>` — fetch transcripts of any video (no API key/OAuth required)
- `yt download` / `yt audio <video>` — download video/audio via `yt-dlp` (audio is Whisper-ready)
- `yt info <video>` — metadata (optional Data API key; falls back to oEmbed)
- `yt search` / `channel` / `comments` / `playlist` — Data API queries (key required)
- `yt analyze <video>` — transcript/Whisper + frames bundled as JSON for an LLM
- Agent-friendly: `--format json` on stdout, JSON errors on stderr, stable exit codes

## Credentials

No OAuth. The only credential is an **optional** YouTube Data API key, resolved as: `--api-key` flag → `YT_API_KEY` env var → `~/.yt` config file → none. Transcripts and downloads need no credentials.

## Status

Under active development. Install once functional:

```sh
cargo install yt
```

## License

Licensed under the [MIT License](LICENSE).
