# `yt` — CLI Plan

A Rust CLI (clap) for YouTube: transcripts, video/audio download for LLM analysis, and metadata. Designed to be driven equally well by a **human** at a terminal and by an **AI agent** (Claude Code) — every command supports `--format json` with a stable contract, predictable exit codes, and no interactive prompts.

See [RESEARCH.md](RESEARCH.md) for the API findings this plan is built on.

---

## 0. Guiding decisions (from research)

- **Transcripts:** unofficial InnerTube path is the only way to cover *any* video → build on `yt-transcript-rs`, with an **Invidious fallback**. Official `captions.download` (OAuth, owner-only) is out of scope for v1.
- **Download:** **shell out to `yt-dlp` + `ffmpeg`** — never reimplement extraction. Detect/locate binaries; clear error if missing.
- **Metadata:** thin `reqwest` + `serde` client against Data API v3 (skip heavy `google-youtube3`). API key via `--api-key`/`YT_API_KEY`. oEmbed + RSS as keyless fallbacks.
- **Crate name `yt` is available** — publish as `yt`, binary `yt`.
- **Agent-first I/O:** `--format json` (stdout only), errors as JSON on stderr, exit codes, env-var config, no prompts.

---

## 1. Scope

### v1 (MVP)
- `yt transcript <video>` — fetch transcript (InnerTube → Invidious fallback); `--lang`, `--format text|json|srt|vtt`, `--translate`, `--timestamps`.
- `yt download <video>` — via yt-dlp; `--audio-only`, `--format`/`--quality`, `--output`, `--cookies-from-browser`.
- `yt info <video>` — metadata. API key → Data API `videos.list`; no key → oEmbed.
- Global: `--format text|json`, `--quiet`, `--api-key`, `--no-color`, `--version`.

### v2
- `yt search <query>` (Data API `search.list`, warns 100-unit cost), `yt channel <id>`, `yt comments <video>`, `yt playlist <id>`.
- `yt audio <video>` convenience → 16 kHz mono WAV ready for Whisper.
- `yt frames <video> --fps N` (ffmpeg) for vision LLMs.

### v3 (analysis layer — the user's "LLM analysis" goal)
- `yt analyze <video>` pipeline: transcript-or-Whisper → optional frames → emit a single structured JSON bundle for an LLM. Pluggable: prefer existing captions, fall back to audio+Whisper (`whisper-rs`).
- PO-token provider integration (`bgutil-ytdlp-pot-provider`) for hardened download.

---

## 2. Command surface (target)

```
yt transcript <VIDEO>   [--lang en] [--translate es] [--timestamps] [--format text|json|srt|vtt]
yt download   <VIDEO>   [--audio-only] [--quality best|720p|...] [--output PATH] [--cookies-from-browser chrome]
yt audio      <VIDEO>   [--output PATH]            # 16kHz mono wav for Whisper
yt info       <VIDEO>                              # metadata (Data API or oEmbed)
yt search     <QUERY>   [--limit 10]               # v2, Data API (100u)
yt channel    <ID|@handle> [--videos] [--limit]    # v2
yt comments   <VIDEO>   [--limit]                  # v2
yt playlist   <ID>                                 # v2
yt frames     <VIDEO>   [--fps 1] [--output DIR]   # v2
yt analyze    <VIDEO>   [--frames] [--whisper]     # v3

Global: --format text|json | --quiet | --api-key <K> (env YT_API_KEY) | --no-color | -v/--verbose
```

`<VIDEO>` accepts a full URL, a `youtu.be/…` short URL, or a bare 11-char video ID — normalized by a shared `parse_video_id()`.

---

## 3. Architecture

```
yt (bin, clap derive)
├── cli            arg parsing, OutputFormat, global flags, dispatch
├── id             URL/ID parsing & validation
├── output         text vs json rendering; JsonEnvelope { data, error, meta }
├── error          thiserror enum → exit codes (0/1/2/3) + JSON error on stderr
├── transcript     InnerTube client (or yt-transcript-rs) + Invidious fallback; format renderers
├── metadata       Data API v3 reqwest client; oEmbed + RSS fallbacks
├── download       yt-dlp process wrapper (locate binary, build args, stream progress)
├── media          ffmpeg wrapper (audio resample, frame extraction)   # v2+
└── analyze        orchestration: transcript|whisper + frames → bundle  # v3
```

- **Async runtime:** `tokio`. **HTTP:** `reqwest` (rustls). **Serde** for JSON/structs. **`thiserror`** for errors. **clap** derive + `env`.
- **External binaries:** `yt-dlp`, `ffmpeg` located via `which`; actionable error + install hint if absent. Consider the `yt-dlp` (boul2gom) crate for auto-managed binaries — evaluate vs. a hand-rolled `std::process::Command` wrapper (lighter deps).

---

## 4. Agent-friendly I/O contract

- `--format json` → single JSON object on **stdout**; nothing else on stdout.
- JSON envelope: `{ "data": <result>, "meta": { "source": "innertube|invidious|data_api|oembed", ... } }`. Errors: `{ "error": { "code": "not_found|auth|network|unavailable", "message": "..." } }` on **stderr**.
- Exit codes: `0` ok · `1` generic · `2` not found · `3` auth/quota.
- No interactive prompts ever; all input via args/flags/env. Respect `NO_COLOR` and non-TTY (auto-disable color).
- Document the JSON schema in README; additive changes only.

---

## 5. Credentials & configuration

**No OAuth anywhere.** The only credential the tool ever uses is an **optional** YouTube Data API **key** (a plain key from Google Cloud Console, not a consent flow). Transcripts, download, audio, frames, and analyze need **zero credentials**; `info` is richer with a key but falls back to oEmbed without one; v2 `search`/`comments`/`channel`/`playlist` require a key (Data-API-only features).

### Resolution order (first hit wins)
1. `--api-key <KEY>` flag (highest precedence; good for one-off / agent calls).
2. `YT_API_KEY` environment variable.
3. Config file at `$YT_CONFIG` if set, else `~/.yt` (or `~/.config/yt/config.toml` if we prefer XDG — decide at scaffold; default `~/.yt`).
4. None → keyless paths still work; key-requiring commands exit `3` (auth) with a JSON error explaining how to set one.

### Config file `~/.yt`
- TOML, e.g.:
  ```toml
  api_key = "AIza..."
  # optional future keys:
  # invidious_instance = "https://yewtu.be"
  # cookies_from_browser = "chrome"
  ```
- Created/updated by a helper: `yt config set api-key <KEY>` writes `~/.yt` with `0600` perms. `yt config get api-key` / `yt config path` for inspection.
- Never logged or echoed in `--verbose`; masked in any diagnostic output.
- Documented in README with the precedence order above.

### Other (no-key) config
- `--cookies-from-browser` / config `cookies_from_browser` — passed through to yt-dlp for age-restricted/members content (a browser session, not a stored secret).
- `--invidious-instance` / config — override the transcript/metadata fallback proxy.

---

## 6. Key risks & mitigations (from research)

- **PO tokens / datacenter-IP blocks / empty-200 transcript responses** → detect empty body (not just status); surface a clear "unavailable, try residential IP / cookies / PO token" error; Invidious fallback; v3 PO-token provider.
- **yt-dlp player breakage** → shelling out means user can `yt-dlp -U`; document. Pin nothing.
- **Quota burn** → `search` warns about 100-unit cost; prefer ID lookups; cache `info` responses (respect 24h caption cache rule).
- **ToS/legal** → README states intended use (own/CC/public-domain content); no evasion features beyond standard cookies/PO-token that yt-dlp already provides.

---

## 7. Step-by-step milestones

Each step ends with formatting (`cargo fmt`), lint (`cargo clippy`), a green build, and a commit. Steps are ordered so the tool is usable end-to-end as early as possible.

### M1 — Scaffold (foundation)
1. `cargo init yt` (bin), set crate metadata (name `yt`, edition, license, description, repo).
2. Add deps: `clap` (derive, env), `tokio`, `reqwest` (rustls, json), `serde`/`serde_json`, `thiserror`, `anyhow`(bin), `which`, `toml`, `dirs`.
3. `cli` module: `Cli` parser, `Commands` enum, global flags (`--format`, `--quiet`, `--verbose`, `--api-key`, `--no-color`), `--version`.
4. `id` module: `parse_video_id()` handling full URL / `youtu.be` / bare 11-char ID + unit tests.
5. `output` module: `OutputFormat`, `JsonEnvelope { data, meta }`, text vs json rendering; auto-disable color on non-TTY / `NO_COLOR`.
6. `error` module: `thiserror` enum → exit codes (0/1/2/3) and JSON error to stderr.
7. `config` module: resolution order (flag → `YT_API_KEY` → `~/.yt` → none); load TOML; mask secrets.
8. ✅ `yt --version`, `yt --help`, and an `id`-parsing test pass. **Commit.**

### M2 — `yt transcript` (first vertical slice, keyless)
1. `transcript` module: InnerTube client — POST `youtubei/v1/player` (ANDROID context), extract `captionTracks[]`.
2. Fetch `baseUrl&fmt=json3`; parse `events[].segs[].utf8` + timestamps; **detect empty-200 body** → typed `Unavailable` error.
3. Renderers: `text`, `json`, `srt`, `vtt`; flags `--lang`, `--translate` (`&tlang=`), `--timestamps`.
4. Invidious fallback (`/api/v1/captions/:id`) when InnerTube yields nothing; `--invidious-instance` override.
5. Integration test against a known-captioned video (network-gated test).
6. ✅ `yt transcript <url> --format json|srt` works. **Commit.**

### M3 — `yt info` (metadata, key optional)
1. `metadata` module: Data API `videos.list` (`snippet,contentDetails,statistics`) with resolved key.
2. oEmbed fallback (`/oembed`) when no key → title/author/thumbnail.
3. Map both into a unified struct; `meta.source` reflects `data_api` vs `oembed`.
4. ✅ `yt info <url>` works with and without a key. **Commit.**

### M4 — `yt config` (credential UX)
1. `yt config set api-key <KEY>` writes `~/.yt` (`0600`); `get` / `path` subcommands.
2. Key-requiring commands emit a helpful auth error (exit 3) pointing at `yt config set`.
3. ✅ Round-trip set→use without env/flag. **Commit.**

### M5 — `yt download` + `yt audio` (yt-dlp wrapper)
1. `download` module: locate `yt-dlp` + `ffmpeg` via `which`; actionable error + install hint if missing.
2. Build args for `--audio-only`, `--quality`, `--output`, `--cookies-from-browser`; stream progress to stderr (suppressed by `--quiet`).
3. `yt audio`: extract bestaudio → ffmpeg `-ar 16000 -ac 1` WAV (Whisper-ready).
4. ✅ Download a short public video + audio-only extract. **Commit.**

### M6 — v2 metadata & media commands
1. `search` (Data API `search.list`, key required; warn 100-unit cost), `channel`, `comments`, `playlist`.
2. `frames <video> --fps N` via ffmpeg for vision LLMs.
3. ✅ Each command works in `text` and `json`. **Commit per command.**

### M7 — v3 `analyze` pipeline
1. Orchestrate: prefer existing transcript → else download audio + Whisper (`whisper-rs`); optional `--frames`.
2. Emit one structured JSON bundle (transcript + metadata + frame paths) for an LLM.
3. PO-token provider integration (`bgutil-ytdlp-pot-provider`) for hardened download.
4. ✅ `yt analyze <url> --format json` produces a complete bundle. **Commit.**

### M8 — Polish & release
1. README: install, command table, **JSON schema**, credentials/precedence, ToS notes, yt-dlp/ffmpeg prerequisites.
2. CI (build + fmt + clippy + tests), `--help` examples, error-code docs.
3. `cargo publish` as `yt` (name confirmed available). **Tag release.**

---

## 8. Initial dependencies

`clap` (derive, env), `tokio`, `reqwest` (rustls, json), `serde`/`serde_json`, `thiserror`, `which`. Evaluate: `yt-transcript-rs` (transcripts), `yt-dlp` boul2gom (download), `whisper-rs` (v3). Keep the dependency tree lean per project conventions; justify each add.

---

## 9. Next steps

1. Confirm scope: is v1 (transcript + download + info) the right first cut, or include `search` immediately?
2. Decide download integration: **`yt-dlp` boul2gom crate** (auto-manages binaries, heavier) vs. **hand-rolled `Command` wrapper** (lean, user installs yt-dlp). Recommendation: start hand-rolled for control, revisit.
3. Scaffold the crate (Milestone 1) and wire `yt transcript` end-to-end as the first vertical slice.
