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

## 5. Key risks & mitigations (from research)

- **PO tokens / datacenter-IP blocks / empty-200 transcript responses** → detect empty body (not just status); surface a clear "unavailable, try residential IP / cookies / PO token" error; Invidious fallback; v3 PO-token provider.
- **yt-dlp player breakage** → shelling out means user can `yt-dlp -U`; document. Pin nothing.
- **Quota burn** → `search` warns about 100-unit cost; prefer ID lookups; cache `info` responses (respect 24h caption cache rule).
- **ToS/legal** → README states intended use (own/CC/public-domain content); no evasion features beyond standard cookies/PO-token that yt-dlp already provides.

---

## 6. Milestones

1. **Scaffold** — `cargo init yt`, clap skeleton, global flags, `id` + `output` + `error` modules, `--version`. Commit.
2. **`transcript`** — InnerTube fetch + parse (json3), text/json/srt/vtt renderers, `--lang`/`--translate`, Invidious fallback. Tests on a known-captioned video.
3. **`info`** — Data API `videos.list` with key; oEmbed fallback without key.
4. **`download` + `audio`** — yt-dlp wrapper, binary detection, progress passthrough, `--audio-only`, ffmpeg 16 kHz resample.
5. **v2** — `search`/`channel`/`comments`/`playlist`/`frames`.
6. **v3** — `analyze` pipeline + Whisper + PO-token provider.
7. **Polish & release** — README (JSON schema, install, ToS notes), CI, `cargo fmt`/`clippy`, publish to crates.io as `yt`.

---

## 7. Initial dependencies

`clap` (derive, env), `tokio`, `reqwest` (rustls, json), `serde`/`serde_json`, `thiserror`, `which`. Evaluate: `yt-transcript-rs` (transcripts), `yt-dlp` boul2gom (download), `whisper-rs` (v3). Keep the dependency tree lean per project conventions; justify each add.

---

## 8. Next steps

1. Confirm scope: is v1 (transcript + download + info) the right first cut, or include `search` immediately?
2. Decide download integration: **`yt-dlp` boul2gom crate** (auto-manages binaries, heavier) vs. **hand-rolled `Command` wrapper** (lean, user installs yt-dlp). Recommendation: start hand-rolled for control, revisit.
3. Scaffold the crate (Milestone 1) and wire `yt transcript` end-to-end as the first vertical slice.
