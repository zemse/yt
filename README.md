# yt

A small command-line tool to get a **YouTube video's transcript** — no API key, no OAuth. Also does metadata, downloads, and an LLM-analysis bundle (see `yt help`).

## Install

```sh
cargo install yt
```

## Get a transcript

```sh
yt transcript "https://www.youtube.com/watch?v=mqbyysExjfU"
```

```
[0:01] Good evening, YouTube. This is Ramesh
[0:03] Sheriff with another video log.
[0:05] Okay, guys. After
...
```

Accepts a full URL, a `youtu.be/…` link, or a bare video ID. Timestamps are shown by default — add `--no-timestamps` for plain text.

A few handy variants:

```sh
yt transcript "<url>" --no-timestamps        # plain text, no timestamps
yt transcript "<url>" --format srt > subs.srt # subtitle file
yt transcript "<url>" --format json           # machine-readable (start/duration per line)
yt transcript "<url>" --lang es               # pick a caption language
```

> Tip: always quote the URL (the `?`/`&` confuse the shell otherwise).

## Other features

`yt` can also fetch metadata, download video/audio, extract frames, and build an LLM-ready bundle. Run:

```sh
yt help
yt help transcript    # detailed flags for any subcommand
```

Quick tour: `yt info` (metadata), `yt download` / `yt audio` (needs [yt-dlp](https://github.com/yt-dlp/yt-dlp) + ffmpeg), `yt analyze` (transcript + metadata as JSON), `yt search` / `comments` / `channel` / `playlist` (need a YouTube Data API key via `yt config set api-key <KEY>`).

Every command supports `--format json` (result on stdout, errors on stderr) for scripting and agents.

## License

[MIT](LICENSE)
