# Lume Music — Phase 2 (Anonymous YouTube Music Provider & Seekable Streaming) Implementation Plan

**Goal:** Add full anonymous YouTube Music search, audio streaming, seekable temp-file buffering, automatic `yt-dlp` fallback, remote cover-art caching, and cross-source likes & mixed-source playlist support to Lume Music without requiring any Google login or saving permanent copyrighted audio files.

**Architecture:**
- **`lume-providers` (`src/ytm.rs`):** `YouTubeMusicProvider` implementing `MusicProvider` via `rustypipe 0.11` (`music_search_tracks` + `player_from_clients`) with M4A/AAC (`AudioFormat::M4a` + `AudioCodec::Mp4a`) stream selection for native `symphonia`/`rodio` decoding, optional HTTP 403 probe, and automatic `yt-dlp` subprocess fallback (`-f bestaudio[ext=m4a]/bestaudio[acodec^=mp4a]/bestaudio -g --no-playlist`).
- **`lume-audio` (`src/stream.rs`, `src/commands.rs`, `src/backend.rs`, `src/engine.rs`):** `AudioCommand::LoadHttpStream` downloads audio progressively via `reqwest` into a single temporary seekable buffer file under `%LOCALAPPDATA%\LumeMusic\cache\stream-*.tmp` wrapped by `ProgressiveStreamReader` (`Read + Seek + Send + Sync`), handed to `rodio::Decoder::builder().with_seekable(true)`. Stale/active temp files are cancelled and deleted on track change, `Stop`, `Shutdown`, and startup.
- **`lume-ui` (`src/app.rs`, `src/views/mod.rs`, `src/views/browse.rs`, `src/views/settings.rs`):** Wire Enter-triggered YTM search alongside incremental Local filtering, async stream resolution with stale-request cancellation (`play_request_id`), HTTP thumbnail/cover caching into `ArtCache` (bounded 100 MB disk / 30 RAM LRU), SQLite upsert before liking or adding YTM tracks to mixed-source playlists, and `yt-dlp` binary status + file picker in Settings.

**Spec:** `docs/superpowers/specs/2026-11-22-lume-music-design.md` (§2, §4.3, §4.4, §4.5, §6 Phase 2)

---

## Global Constraints & Review Focus

1. **`symphonia`/`rodio` codec compatibility:** `rodio 0.23` (`symphonia-all`) decodes M4A/AAC (`mp4a`), MP3, FLAC, Vorbis, and WAV, but does **not** decode WebM/Opus. Stream selection in both `rustypipe` and `yt-dlp` MUST prefer `M4a/Mp4a` highest bitrate and reject Opus-only candidates so playback never fails silently with `UnrecognizedFormat`.
2. **Temp buffer lifecycle & Windows file sharing:** Active HTTP buffer files under `%LOCALAPPDATA%\LumeMusic\cache\` must be cleaned up on track switch, `Stop`, `Shutdown`, and startup. Cancelling an in-flight download must immediately wake any blocked `ProgressiveStreamReader::read()` via `Condvar` so the audio thread never deadlocks or leaks handles.
3. **SQLite foreign keys for cross-source YTM tracks:** YTM tracks from Search are not yet in `tracks` until interacted with. `ToggleLike` and `AddTrackToPlaylist` MUST `upsert_track` before touching `local_favorites` or `playlist_tracks`.
4. **Remote artwork cache key consistency:** YTM tracks store `artwork_ref = Some("https://...")` (distinct from `track.uid = "ytm:<id>"`). `PaletteLoaded` and `ThumbnailsLoaded` must insert into `art_handles` keyed by `artwork_ref` and persist downloaded bytes through `ArtCache` so RAM stays bounded (`<= 30` handles) and repeated views load from disk.
5. **Race-free playback & search:** Rapid track switching or repeated Enter searches must discard stale async completions via monotonic request IDs (`play_request_id`, `search_request_id`).

---

### Task 1: `lume-providers` Anonymous YouTube Music Provider (`rustypipe` + `yt-dlp` Fallback)

**Files:**
- Modify: `Cargo.toml` (add `rustypipe` workspace dependency)
- Modify: `crates/lume-providers/Cargo.toml`
- Create: `crates/lume-providers/src/ytm.rs`
- Modify: `crates/lume-providers/src/lib.rs`

**Interfaces:**
- `AudioStreamCandidate { url: String, bitrate: u32, is_m4a_aac: bool, is_opus: bool, is_original_track: bool, size: u64 }`
- `select_best_audio_stream(candidates: &[AudioStreamCandidate]) -> Result<&AudioStreamCandidate, CoreError>`
- `upgrade_ytm_cover_url(url: &str) -> String`
- `detect_yt_dlp_binary(configured: Option<&Path>) -> Option<PathBuf>`
- `parse_yt_dlp_stdout(stdout: &str) -> Option<String>`
- `YouTubeMusicProvider::new(cache_dir: PathBuf, yt_dlp_path: Option<PathBuf>) -> Result<Self, CoreError>`
  - Implements `MusicProvider` (`search`, `get_playlists` -> empty, `get_playlist_tracks` -> empty, `resolve_stream`)
  - `resolve_stream_with_yt_dlp(&self, video_id: &str) -> Result<ResolvedStream, CoreError>`

**TDD Steps:**
- [ ] Add `rustypipe` dependency (`default-features = false, features = ["native-tls"]`) and write failing unit tests in `crates/lume-providers/src/ytm.rs` for stream selection (M4A/AAC preferred over Opus, highest bitrate wins, original preferred over dubbed, empty/Opus-only rejected), cover URL normalization, `Track` mapping, `yt-dlp` stdout parsing, and missing-binary actionable error.
- [ ] Run `cargo test -p lume-providers ytm` (verify fail).
- [ ] Implement `YouTubeMusicProvider` with `rustypipe` search (`music_search_tracks`), multi-client player resolution (`[Tv, Android, Ios, Desktop]`), optional HTTP 403 probe (`Range: bytes=0-1`), and silent Windows `yt-dlp` subprocess fallback (`CREATE_NO_WINDOW`).
- [ ] Run `cargo test -p lume-providers` (verify pass).
- [ ] Commit `feat(providers): add anonymous YouTube Music provider with rustypipe and yt-dlp fallback`.

---

### Task 2: `lume-audio` Seekable Temp-File HTTP Stream Buffering

**Files:**
- Modify: `crates/lume-audio/Cargo.toml` (add `reqwest`, `dirs`)
- Create: `crates/lume-audio/src/stream.rs`
- Modify: `crates/lume-audio/src/commands.rs`
- Modify: `crates/lume-audio/src/backend.rs`
- Modify: `crates/lume-audio/src/engine.rs`
- Modify: `crates/lume-audio/src/lib.rs`

**Interfaces:**
- `AudioCommand::LoadHttpStream { url: String, track_uid: String, duration_ms: u64, mime_hint: Option<String> }`
- `ProgressiveStreamReader` (`Read + Seek + Send + Sync`) backed by `stream-<gen>.tmp` + shared `StreamBufferState` (`Mutex` + `Condvar` + `AtomicBool` cancel flag)
- `HttpStreamSession` managing Tokio download task cancellation & temp-file removal on drop
- `AudioBackend::load_stream(&mut self, reader: ProgressiveStreamReader, byte_len: Option<u64>, hint: Option<&str>) -> Result<Option<u64>, String>`
- `cleanup_stale_stream_files(cache_dir: &Path)` called at engine startup & shutdown

**TDD Steps:**
- [ ] Write failing unit/integration tests in `crates/lume-audio/src/stream.rs` and `src/engine.rs` using a local `tokio::net::TcpListener` serving generated WAV bytes:
  - `progressive_reader_waits_for_chunks_and_supports_seeking`
  - `cancelling_stream_wakes_blocked_reader_and_deletes_temp_file`
  - `load_http_stream_emits_buffering_then_playing_and_cleans_up_on_stop`
  - `http_403_stream_emits_error_without_stopping_worker`
- [ ] Run `cargo test -p lume-audio` (verify fail).
- [ ] Implement `stream.rs`, extend `AudioCommand`, `AudioBackend`, `RodioAudioBackend`, and `engine.rs` (running a lightweight dedicated single-thread Tokio runtime inside the audio worker for HTTP chunk downloading, emitting `Buffering` → `Playing`, and ensuring zero leaked `stream-*.tmp` files).
- [ ] Run `cargo test -p lume-audio` (verify pass).
- [ ] Commit `feat(audio): add seekable temp-file HTTP stream buffering for YouTube Music`.

---

### Task 3: `lume-ui` Wiring — YTM Search, Playback, Remote Artwork Cache, Cross-Source Likes/Playlists & Settings

**Files:**
- Modify: `crates/lume-ui/Cargo.toml` (add `reqwest`)
- Modify: `crates/lume-ui/src/views/mod.rs`
- Modify: `crates/lume-ui/src/views/browse.rs`
- Modify: `crates/lume-ui/src/views/settings.rs`
- Modify: `crates/lume-ui/src/app.rs`

**Changes:**
- Add `Message` variants: `YtmSearchFinished`, `StreamResolved`, `PickYtDlpBinary`, `YtDlpPicked`, `ClearYtDlpBinary`; update `AddTrackToPlaylist` to carry `track: Track` and `PaletteLoaded` to carry `art_key: Option<String>`.
- On `Message::SearchSubmitted`: keep instant Local SQLite matches and trigger async `YouTubeMusicProvider::search` when `source_filter` is `All | YouTubeMusic`, hydrating `is_liked` from SQLite `local_favorites` and preloading remote thumbnails.
- On `start_playing_track` for `TrackSource::YouTubeMusic`: set `PlaybackStatus::Buffering`, upsert track in SQLite, resolve stream asynchronously via `YouTubeMusicProvider`, and dispatch `AudioCommand::LoadHttpStream` (ignoring stale `play_request_id`).
- Upgrade `load_palette_task` and `preload_thumbnails_task` to fetch uncached `http(s)://` artwork via `reqwest`, persist into `ArtCache`, and key `art_handles` by `artwork_ref`.
- Fix `ToggleLike` and `AddTrackToPlaylist` to `db.upsert_track(&track, None)` before foreign-key writes so YTM tracks can be liked and mixed into custom playlists seamlessly.
- Add **YouTube Music & yt-dlp Fallback** card in `views/settings.rs` showing auto-detection status plus `Browse yt-dlp.exe` / `Clear` actions.
- Add unit tests in `lume-ui` verifying search result merging, stale request gating, and `is_remote_art_url` helper.

**TDD Steps:**
- [ ] Write unit tests for search merge/hydration and remote art detection.
- [ ] Implement UI wiring across `views/mod.rs`, `views/browse.rs`, `views/settings.rs`, and `app.rs`.
- [ ] Run `cargo test -p lume-ui` (verify pass).
- [ ] Commit `feat(ui): wire anonymous YouTube Music search, streaming, remote artwork cache, and yt-dlp settings`.

---

### Task 4: Full Workspace Verification & Release Build

- [ ] Run `cargo test --workspace` (all 5 crates green, zero warnings/failures).
- [ ] Run `cargo build --release -p lume-ui` and verify clean `lume-music.exe` build.
- [ ] Verify `git status` is clean and all Phase 2 commits are tidy.
