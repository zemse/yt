# YouTube API Research

Research backing a Rust CLI (`yt`) for fetching transcripts, downloading video/audio for LLM analysis, and querying YouTube metadata. Covers official and unofficial APIs, auth requirements, current breakage (2024–2026), and the Rust crate landscape.

> State as of May 2026. YouTube's internal/unofficial surfaces change frequently; treat the "current breakage" sections as perishable.

---

## TL;DR

| Goal | Best path | API key? | Reliability |
|---|---|---|---|
| **Transcript of any video** | Unofficial InnerTube (`youtubei/v1/player`) → `timedtext` baseUrl, OR Invidious `/api/v1/captions/:id` | No | Medium — breaks with PO-token enforcement, datacenter-IP blocks |
| **Transcript of a video you own** | Official Data API `captions.download` | OAuth | High but ownership-locked |
| **Download video/audio file** | Shell out to **`yt-dlp`** (+ ffmpeg) | No | Highest available; self-healing community |
| **Metadata (title, stats, search, comments)** | Official Data API v3 with API key | API key | High, but 10k units/day quota |
| **Quick title/thumbnail, no key** | oEmbed (`/oembed`) | No | High |
| **Latest N channel videos, no key** | RSS (`/feeds/videos.xml`) | No | High |

**Crate name `yt` is AVAILABLE on crates.io** (verified via sparse index `https://index.crates.io/2/yt` → 404/NoSuchKey, May 2026).

**Two hard truths:**
1. The official API **cannot download video files at all**, and **cannot download captions for videos you don't own**. For the user's two headline features (any-video transcript + video download), the official API is a dead end — unofficial methods are mandatory.
2. Since mid-2024 YouTube has rolled out **PO (Proof-of-Origin) tokens**, **SABR streaming**, and **datacenter-IP fingerprinting** that break naive unofficial clients. `yt-dlp` is the only tool that keeps up, because it has a large community patching breakage within hours/days.

---

## 1. Transcripts / Captions

### 1.1 Official Data API v3 — ownership-locked (mostly useless for our case)

- **`captions.list`** — `GET https://www.googleapis.com/youtube/v3/captions?part=snippet&videoId={ID}`
  - **OAuth 2.0 required** (API key alone rejected). Scope `youtube.force-ssl`. Cost 50 units.
  - Returns track *metadata* (id, lang, `trackKind` auto/manual), **not text**.
  - **You must own the video.** 403 otherwise.
- **`captions.download`** — `GET https://www.googleapis.com/youtube/v3/captions/{ID}`
  - OAuth required, **video editor permission required**. Cost ~200 units.
  - Params: `tfmt` (`srt`|`vtt`|`ttml`|`sbv`|`scc`), `tlang` (translate).
  - 403 "permissions not sufficient" for videos you don't own.
- **Verdict:** Only usable for your own channel's videos. **Not viable** for "transcript of any YouTube video." At 200 units/download and a 10k/day quota, also only ~50 downloads/day.

### 1.2 Unofficial InnerTube + timedtext (how `youtube-transcript-api` works)

Two phases:

**Phase 1 — get caption track baseUrls via InnerTube:**
```
POST https://www.youtube.com/youtubei/v1/player
Content-Type: application/json

{ "context": { "client": { "clientName": "ANDROID", "clientVersion": "20.10.38" } },
  "videoId": "VIDEO_ID" }
```
- No OAuth needed for most public videos. A hardcoded `INNERTUBE_API_KEY` may be appended (`?key=...`) but is often optional. `ANDROID`/`TVHTML5` clients trigger fewer bot checks than `WEB`.
- Navigate: `response.captions.playerCaptionsTracklistRenderer.captionTracks[]`. Each entry: `baseUrl`, `languageCode`, `kind` (`"asr"` = auto-generated), `name.simpleText`.

**Phase 2 — fetch transcript text:**
```
{baseUrl}&fmt=json3      # JSON: events[].segs[].utf8, tStartMs, dDurationMs
{baseUrl}&fmt=vtt        # WebVTT
{baseUrl}&tlang=es       # machine-translate to target language
```
Legacy direct form (`https://www.youtube.com/api/timedtext?v=ID&lang=en`) still appears in old tutorials but is superseded — InnerTube hands you a pre-signed `baseUrl`.

**JSON3 shape:**
```json
{ "events": [ { "tStartMs": 1000, "dDurationMs": 3500, "segs": [ { "utf8": "Hello" } ] } ] }
```
Skip events lacking `segs` (timing-only markers).

### 1.3 Invidious / Piped — proxy that returns caption text without OAuth

- Invidious: `GET https://{instance}/api/v1/captions/:id` → WebVTT. No key. This sidesteps the official API's ownership lock.
- Public instances rate-limit, go down, or get YouTube-blocked. Make the instance configurable + implement fallback.

### 1.4 Current breakage (2024–2026)

- **PO tokens on subtitles:** Since ~Aug 2024 the `timedtext` `baseUrl` may carry/require a `&pot=` (Proof-of-Origin) param. Missing it → **HTTP 200 with an empty body** (silent failure — check body length, not just status).
- **Datacenter IPs:** AWS/GCP/Azure ranges get 302→`google.com/sorry` CAPTCHA or 403 within seconds during bulk runs. **Residential IP is the #1 reliability factor.**
- **Cookie auth:** Helps with age-restricted/private but goes stale fast and risks account bans if abused.
- **Soft rate limit:** ~100 req/hr per IP before throttling.

### 1.5 Rust transcript crates

| Crate | Latest | State | Notes |
|---|---|---|---|
| `yt-transcript-rs` | 0.1.8 (2025-06) | **Maintained** | InnerTube-based, cookie + proxy support. Best starting point. |
| `ytt` | 1.1.0 (2025-12) | Active | "Rust impl of YouTube Transcript API"; no repo link. |
| `ydl-lib` | 0.2.0 (2025-08) | Active | Subtitle downloader, SRT/VTT/JSON/XML out. |
| `srv3-ttml` | 0.1.0 (2024-08) | Parser only | Parses YouTube's SRV3/TTML XML. |
| `youtube-transcript` | 0.3.2 (2023) | Stale | HTML-scraping; likely broken. |

---

## 2. Video / Audio Download

### 2.1 Official API — does NOT support it

The Data API is metadata/management only. **No endpoint returns a stream or file.** `videos.list(part=fileDetails)` exposes source-file specs to the *owner* but still no URL. YouTube's only official "download" is DRM-locked offline playback in the mobile app. **Dead end.**

### 2.2 yt-dlp — the de-facto standard (shell out, do NOT reimplement)

`youtube-dl` (2008) → `youtube-dlc` (2020) → **`yt-dlp`** (2021–present, the live fork; original youtube-dl is effectively dead for YouTube). How it extracts:

1. **InnerTube `/youtubei/v1/player`** queried as multiple impersonated clients (`WEB`, `ANDROID`, `IOS`, `MWEB`, `TV`). Multi-client = resilience: if one breaks another works.
2. **Format extraction** from `streamingData.formats` (muxed) + `adaptiveFormats` (separate hi-res video/audio, DASH/HLS).
3. **Signature deciphering** — two JS-obfuscated challenges in `base.js`:
   - `sig` — decrypts the URL signature (reverse/slice/splice ops extracted via AST).
   - `nsig` — throttle key; wrong value → ~50 KB/s throttle. Solved by executing extracted JS in a runtime (**Deno**/Node/Bun/QuickJS via `yt-dlp-ejs`).
   - **Fragile:** new player version → AST patterns break → community patches in hours/days (e.g. player `4fcd6e4a`, Mar 2025, issue #12746).
4. **Merge** separate video+audio with **ffmpeg** (no re-encode).

**Why shell out, not reimplement in Rust:** breakage is constant; yt-dlp has 200+ contributors and a JS-runtime dependency for nsig/sig. A pure-Rust reimplementation would be perpetually broken.

### 2.3 Rust download crates

| Crate | Latest | State | Verdict |
|---|---|---|---|
| **`yt-dlp` (boul2gom)** | 2.7.2 (2026-04) | **Active, ~biweekly** | **Best.** Async wrapper, auto-downloads yt-dlp+ffmpeg binaries, typed structs. |
| `ytdlp_bindings` | active | Niche | Wrapper; `audio-processing` feature vendors ffmpeg. |
| `rusty_ytdl` | 0.7.4 (2024-08) | Fragile | Pure-Rust extraction, no JS runtime / no PO-token path → breaks on player updates. |
| `rustube` | 0.6.0 (2022-10) | **Abandoned** | Don't use. |
| `bgutil-ytdlp-pot-provider` | 0.8.1 (2026-03) | Active | Not a downloader — a Rust **PO-token generator** (BotGuard via `rustypipe-botguard`). HTTP server :4416 or CLI. |

### 2.4 PO Token / SABR / nsig (the 2024–2026 wall)

- **PO Token (Proof of Origin):** attestation from BotGuard (web)/DroidGuard (Android)/iOSGuard. Now **per-video + per-session**, bound to `visitor_data`/`video_id`. Without it: 403 or throttled/absent streams. Enforcement matrix (2025):

  | Client | Video stream (GVS) | Player | Subtitles |
  |---|---|---|---|
  | web | Required | — | Required |
  | mweb | Required | — | — |
  | tv | — | — | — |
  | android/ios | Required | Required | — |

- **SABR** (late 2024): proprietary streaming replacing DASH on `web`; `adaptiveFormats` come back empty (yt-dlp issue #12482). Workaround: use `mweb`/`tv` client.
- **Workarounds:** `mweb`/`tv` client; PO-token provider plugins (`yt-dlp-get-pot`, `bgutil-ytdlp-pot-provider`); `--cookies-from-browser`; keep yt-dlp on nightly; **residential IP** (cloud IPs get 403'd regardless).

### 2.5 Formats for LLM analysis

- **Audio for Whisper** (most relevant): `yt-dlp -x --audio-format wav` then `ffmpeg -ar 16000 -ac 1` (Whisper native 16 kHz mono); or keep Opus (no re-encode). Rust: `whisper-rs`.
- **Skip download if captions exist:** `yt-dlp --write-auto-subs --skip-download` — orders of magnitude faster than Whisper when auto-captions suffice.
- **Frames for vision LLMs:** `ffmpeg -i video.mp4 -vf fps=1 frames/%04d.jpg`.

---

## 3. Metadata via Official Data API v3

### 3.1 Getting a key
Google Cloud Console → Create credentials → API key → enable "YouTube Data API v3" → restrict key. Free tier **10,000 units/day/project**, resets midnight Pacific. Increases require a justification form (not guaranteed).

### 3.2 Quota costs (every call ≥1 unit)

| Method | Units | Key vs OAuth |
|---|---|---|
| `videos.list` | 1 | API key (public) |
| `channels.list` | 1 | API key |
| `playlists.list` / `playlistItems.list` | 1 | API key |
| `comments.list` / `commentThreads.list` | 1 | API key |
| `activities.list` | 1 | API key |
| **`search.list`** | **100** | API key — **most expensive read**; ≤100 searches/day |
| `captions.list` | 50 | OAuth |
| `captions.download` | ~200 | OAuth |
| any insert/update/delete | 50–450 | OAuth |

**Optimization:** prefer ID lookups (`videos.list`, 1u) over `search.list` (100u) whenever the ID is known.

### 3.3 `videos.list` parts (API-key, public, 1u)
`snippet` (title, channel, description, tags, publishedAt, thumbnails), `contentDetails` (ISO-8601 duration, definition, caption flag, regionRestriction), `statistics` (viewCount, likeCount, commentCount — dislikes removed 2021), `status`, `topicDetails`, `liveStreamingDetails`, `player`. Owner-only (OAuth): `fileDetails`, `processingDetails`, `suggestions`, `recordingDetails`.

### 3.4 Key vs OAuth summary
- **API key (public reads):** search, videos, channels, playlists, playlistItems, comments, commentThreads, activities.
- **OAuth required:** captions (even read), `subscriptions mine=true`, ratings, all writes.
- Read scopes: `youtube.readonly`, `youtube.force-ssl` (captions).

---

## 4. No-key Official-ish Endpoints

- **oEmbed:** `https://www.youtube.com/oembed?url=https://www.youtube.com/watch?v={ID}&format=json` → title, author_name/url, thumbnail_url, embed html. No key, CORS-friendly, public only, no stats.
- **RSS/Atom:** channel `https://www.youtube.com/feeds/videos.xml?channel_id={CID}`; playlist `...?playlist_id={PID}`. ~15 latest entries, zero quota.

---

## 5. Invidious / Piped (unofficial proxies, no key)

**Invidious** (`https://{instance}/api/v1/`): `videos/:id` (full metadata + formats + captions list + recommendations), `captions/:id` (WebVTT), `comments/:id`, `channels/:ucid`(+`/videos`,`/playlists`,`/community`,`/search`), `search`, `trending`, `playlists/:plid`, `hashtag/:tag`. Public instances at invidious.io; rate-limited/unstable → make configurable + fallback.

**Piped** (`https://pipedapi.kavin.rocks`): `streams/{id}` (signed stream URLs + metadata + subtitles), `channel/{id}` + `nextpage`, `search`, `playlists/{id}`, `comments/{id}`.

Both give caption text and stream info **without OAuth or API key** — valuable fallback layer, at the cost of depending on third-party uptime.

---

## 6. Existing Rust YouTube crates (general)

| Crate | Latest | Notes |
|---|---|---|
| `google-youtube3` | 7.0.0+20251222 | Full auto-generated Data API v3 bindings (hyper, yup-oauth2). Comprehensive but heavy/generated. |
| `yup-oauth2` | ^12 | Google OAuth2 (device/installed/service-account flows). |
| `rusty_ytdl` | 0.7.4 | Pure-Rust scraping (boa JS engine); fragile. |
| `yt-dlp` (boul2gom) | 2.7.2 | yt-dlp binary wrapper. |

**Gap:** no lightweight, ergonomic, CLI-focused read-only Data API crate. A thin `reqwest` + `serde` client is the pragmatic choice over `google-youtube3` for our needs.

---

## 7. Clap CLI design for humans + AI agents

- `--format text|json` global flag; **machine output on stdout only**, errors as JSON on stderr `{"error","code"}`. Disable color when `--format json` or non-TTY (`NO_COLOR`).
- Stable JSON contract: additive keys OK, renames/removals breaking. Consider NDJSON for large lists (ripgrep `--json` is the canonical example).
- Consistent exit codes (0 ok, 1 generic, 2 not-found, 3 auth). No interactive prompts — config via flags + env (`#[arg(env="YT_API_KEY")]`). Global flags before/after subcommand (`global = true`).

---

## 8. Rate limits, ToS, legal

- Official: 10k units/day; caching captions >24h prohibited; attribution required.
- Unofficial/InnerTube: ~100–200 req/hr/IP soft limit; datacenter IPs banned fast.
- ToS §5.B prohibits scraping/downloading without an explicit download affordance or written permission. yt-dlp the *tool* is legal (German court 2021; GitHub reinstated post-RIAA-DMCA); *use* against copyrighted content is a ToS/DMCA risk. Your own / CC / public-domain content is safe; the CLI should surface these caveats and default to user-supplied auth.

---

## 9. Key sources

- YouTube Data API: captions [list](https://developers.google.com/youtube/v3/docs/captions/list) / [download](https://developers.google.com/youtube/v3/docs/captions/download); [videos.list](https://developers.google.com/youtube/v3/docs/videos/list); [quota costs](https://developers.google.com/youtube/v3/determine_quota_cost)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) · [PO Token Guide](https://github.com/yt-dlp/yt-dlp/wiki/PO-Token-Guide) · [SABR #12482](https://github.com/yt-dlp/yt-dlp/issues/12482)
- [youtube-transcript-api](https://github.com/jdepoix/youtube-transcript-api) · [yt-transcript-rs](https://crates.io/crates/yt-transcript-rs)
- Rust wrappers: [yt-dlp (boul2gom)](https://github.com/boul2gom/yt-dlp) · [rusty_ytdl](https://github.com/Mithronn/rusty_ytdl) · [google-youtube3](https://lib.rs/crates/google-youtube3) · [bgutil-ytdlp-pot-provider](https://github.com/Brainicism/bgutil-ytdlp-pot-provider)
- Proxies: [Invidious API](https://docs.invidious.io/api/) · [Piped API](https://docs.piped.video/docs/api-documentation/)
- [oEmbed](https://oembed.com/) · [Rust CLI machine-readable output](https://rust-cli-recommendations.sunshowers.io/machine-readable-output.html)
- Crate name check: `https://index.crates.io/2/yt` → 404 (available, May 2026)
