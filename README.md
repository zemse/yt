# yt

A command-line tool for YouTube — **transcripts**, **video/audio downloads** (for LLM analysis), and **metadata** — designed to be driven equally well by humans and AI agents.

Every command supports `--format json` (a stable envelope on stdout, errors as JSON on stderr) and predictable exit codes, so agents can call it as reliably as a library. See [`PLAN.md`](PLAN.md) and [`RESEARCH.md`](RESEARCH.md) for design and background.

## Install

```sh
cargo install yt
```

Optional external tools (only needed for the commands that use them):
- **[yt-dlp](https://github.com/yt-dlp/yt-dlp)** — `download`, `audio`, `frames`, `analyze` (`brew install yt-dlp`)
- **ffmpeg** — `audio`, `frames` (`brew install ffmpeg`)
- **whisper** (openai-whisper or whisper.cpp) — `analyze --whisper` fallback

## Commands

| Command | Needs key? | Needs tools? | Description |
|---|---|---|---|
| `yt transcript <video>` | no | no | Transcript via InnerTube, Invidious fallback. Text output shows `[m:ss]` timestamps by default (`--no-timestamps` to hide). `--lang`, `--translate`, `--format text\|json\|srt\|vtt` |
| `yt info <video>` | optional | no | Metadata (Data API with key; oEmbed without) |
| `yt download <video>` | no | yt-dlp | `--audio-only`, `--quality`, `-o`, `--cookies-from-browser` |
| `yt audio <video>` | no | yt-dlp+ffmpeg | 16 kHz mono WAV, Whisper-ready |
| `yt search <query>` | yes | no | `--limit` (costs 100 quota units) |
| `yt channel <id\|@handle>` | yes | no | Channel stats |
| `yt comments <video>` | yes | no | Top comments |
| `yt playlist <id>` | yes | no | Playlist items |
| `yt frames <video>` | no | yt-dlp+ffmpeg | Sample frames at `--fps` |
| `yt analyze <video>` | optional | yt-dlp* | LLM bundle: transcript(+metadata)[+frames] as JSON |
| `yt config set\|get\|path` | — | no | Manage `~/.yt` |

\* `analyze` only needs tools if it has to fall back to Whisper or extract frames.

## Examples

```sh
yt transcript "https://www.youtube.com/watch?v=BgMP3Bx1q10"
yt transcript BgMP3Bx1q10 --format srt > subs.srt
yt info BgMP3Bx1q10 --format json
yt audio BgMP3Bx1q10 -o talk.wav        # 16kHz mono for Whisper
yt analyze BgMP3Bx1q10 --format json > bundle.json
```

## Credentials

No OAuth — ever. The only credential is an **optional** YouTube Data API key, resolved in order:

1. `--api-key <KEY>`
2. `YT_API_KEY` environment variable
3. `~/.yt` config file (`yt config set api-key <KEY>`, written `0600`)

Transcripts and downloads need no credentials. `search`/`channel`/`comments`/`playlist` require a key (Data-API-only features).

## JSON contract

`--format json` prints `{ "data": <result>, "meta": {...} }` to stdout on success, and `{ "error": { "code", "message" } }` to stderr on failure. Error codes: `not_found`, `auth`, `unavailable`, `missing_tool`, `network`, `input`. Exit codes: `0` ok, `1` generic, `2` not found, `3` auth/quota.

## Reliability notes

YouTube enforces PO-tokens and blocks datacenter IPs (see RESEARCH.md). Transcript fetching is most reliable from a residential IP; on failure the tool falls back to Invidious and reports a clear `unavailable` error.

## License

Licensed under the [MIT License](LICENSE).
