# Lume Music — Phase 1 (Foundation, Local Playback & Glassmorphism UI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a complete, runnable, ultra-lightweight native Rust Windows 11 music player (Phase 1) with SQLite library/custom mixed-source playlists/unified `Liked Songs`, local audio scanning & playback (`symphonia`/`rodio`), resilient LRCLIB cached synced lyrics, `iced 0.14` Browse & Spicy-style Now Playing views, `wgpu` dynamic album gradient shader + Windows 11 Acrylic toggle, SMTC media controls, System Tray, and Smart Render Pause. *(Phases 2 [YouTube Music], 3 [Spotify Premium + URL Importer + Smart Fallback], and 4 [Profiling & Polish] will each have their own follow-up plans once Phase 1 is verified.)*

**Architecture:** Five-crate Cargo workspace (`lume-core`, `lume-storage`, `lume-audio`, `lume-providers`, `lume-ui`) enforcing strict separation between pure domain/queue state, SQLite/disk persistence, a dedicated non-blocking audio worker thread, async media/lyrics providers, and the `iced 0.14` GPU UI shell. All provider traits and database tables (`yt_mappings`, `pending_mutations`, `PlaylistOrigin`) are established in Phase 1 so Phases 2 and 3 plug in without schema or core refactors.

**Tech Stack:** Rust 1.99.1 (Windows 11 x64), `iced 0.14` (`wgpu`, `tokio`, `image`, `advanced`), `window-vibrancy`, `symphonia` + `rodio`/`cpal`, `rusqlite` (bundled, WAL), `lofty` + `walkdir`, `reqwest` + `serde` + `toml`, `souvlaki`, `tray-icon`.

**Spec:** `docs/superpowers/specs/2026-11-22-lume-music-design.md` (visual reference: `docs/superpowers/specs/assets/reference-lyrics-view.jpg`)

## Global Constraints

- **Target platform & toolchain:** Windows 11 x64, Rust `1.99.1` (`cargo` / `rustc`).
- **Performance targets:** `< 100 MB` RAM typical playback, `< 2%` CPU active with animations, `< 0.5%` CPU when minimized to system tray (`0 FPS` render ticks when hidden/minimized).
- **UI thread discipline:** Zero blocking network, SQLite heavy scans, image decoding, or audio decoding on the UI thread.
- **Persistence paths:** SQLite WAL DB at `%LOCALAPPDATA%\LumeMusic\lume.db`; artwork disk cache at `%LOCALAPPDATA%\LumeMusic\cache\art\` capped at ~100 MB LRU; in-memory decoded artwork cache capped at 30 entries; settings TOML at `%APPDATA%\LumeMusic\config.toml`.
- **Visual readability:** Adaptive dark scrim strictly clamped to `[0.38, 0.55]` over dynamic album gradient; frosted glass surfaces use `rgba(20, 20, 26, 0.45)` with `1px` `rgba(255, 255, 255, 0.12)` border; custom frameless titlebar (`─ □ ×`) matching `docs/superpowers/specs/assets/reference-lyrics-view.jpg`.
- **Crate version policy:** Resolve exact latest compatible minor versions via `cargo search` / `cargo check` for Rust 1.99.1 and `iced 0.14` (never pin incompatible transitive `wgpu`/`raw-window-handle` mismatches).

## Review Focus

1. **Malformed & enhanced LRC lyrics input:** `[mm:ss.xxx]` (3-digit ms), `[mm:ss]` (no fraction), repeated timestamps `[00:12.00][01:02.00]Chorus`, out-of-order lines, empty instrumental gaps, malformed brackets, and enhanced word-level `<mm:ss.xx>` tags must parse without panicking, expand repeated timestamps, sort chronologically, and preserve optional `words` (pinned in Task 5).
2. **Messy local audio libraries:** Files with missing/empty tags must fall back to filename stem / `"Unknown Artist"`; corrupt/unreadable audio files must be skipped with a warning without aborting folder scan; deleted files must be pruned on rescan; scanning local folders must never auto-like tracks (pinned in Task 4).
3. **Queue, Shuffle & Previous-track edge cases:** Disabling Shuffle must keep the currently playing track active and restore original order around it; `previous(elapsed_ms)` restarts current track when `elapsed_ms > 3000` and steps back through actual play history when `<= 3000`; `RepeatMode::One` loops current track; deleting the active track advances or stops cleanly (pinned in Task 2).
4. **Bright/corrupt album cover art contrast:** Pure white, 1×1, or corrupted artwork bytes must never panic or cause unreadable white-on-white text — palette extraction must fall back safely and clamp `scrim_alpha` into `[0.38, 0.55]` (pinned in Task 7).
5. **Read-only Spotify mirror protection & cache bounds:** Any mutation (`rename`, `delete`, `add_track`, `remove_track`, `reorder`) on a `SpotifySynced` (`is_editable = false`) playlist must fail with `StorageError::PlaylistReadOnly`; `ArtCache` eviction must strictly enforce the max byte cap without deleting the most recently accessed entry (pinned in Task 3).

---

## File Structure Overview

```text
D:\Project\lume-music\
├── Cargo.toml
├── docs/superpowers/
│   ├── specs/2026-11-22-lume-music-design.md
│   ├── specs/assets/reference-lyrics-view.jpg
│   └── plans/2026-11-22-phase-1-foundation-local-glass-ui.md
└── crates/
    ├── lume-core/
    │   ├── Cargo.toml
    │   └── src/{lib.rs, error.rs, models.rs, traits.rs, queue.rs}
    ├── lume-storage/
    │   ├── Cargo.toml
    │   └── src/{lib.rs, error.rs, schema.sql, db.rs, config.rs, art_cache.rs}
    ├── lume-providers/
    │   ├── Cargo.toml
    │   └── src/{lib.rs, local.rs, lyrics/{mod.rs, lrc.rs, lrclib.rs}}
    ├── lume-audio/
    │   ├── Cargo.toml
    │   └── src/{lib.rs, commands.rs, backend.rs, engine.rs}
    └── lume-ui/
        ├── Cargo.toml
        └── src/
            ├── lib.rs, main.rs, app.rs, lyrics_anim.rs
            ├── theme/{mod.rs, palette.rs, shader.rs, background.wgsl}
            ├── widgets/{mod.rs, glass.rs}
            ├── views/{mod.rs, titlebar.rs, browse.rs, player_bar.rs, queue_drawer.rs, settings.rs, lyrics_view.rs}
            └── platform/{mod.rs, vibrancy.rs, smtc.rs, tray.rs}
```

---

### Task 1: Workspace Scaffold & `lume-core` Domain Models + Provider Traits

**Files:**
- Create: `Cargo.toml`
- Create: `crates/lume-core/Cargo.toml`
- Create: `crates/lume-core/src/lib.rs`
- Create: `crates/lume-core/src/error.rs`
- Create: `crates/lume-core/src/models.rs`
- Create: `crates/lume-core/src/traits.rs`

**Interfaces:**
- Consumes: none (foundation crate)
- Produces:
  - `TrackSource::{Spotify { id: String }, YouTubeMusic { video_id: String }, Local { path: PathBuf }}` with `TrackSource::canonical_uid(&self) -> String`
  - `Track { uid: String, source: TrackSource, title: String, artists: Vec<String>, album: Option<String>, duration_ms: u64, artwork_ref: Option<String>, is_liked: bool }`
  - `PlaylistOrigin::{SpotifySynced, SpotifyImported { source_url: String }, LocalCustom}` with `PlaylistOrigin::is_editable(&self) -> bool`
  - `Playlist { id: String, name: String, origin: PlaylistOrigin, track_count: usize }`
  - `LyricsWord { start_ms: u64, end_ms: u64, text: String }`
  - `LyricsLine { time_ms: u64, text: String, words: Option<Vec<LyricsWord>> }`
  - `LyricsData { synced: Vec<LyricsLine>, plain: Option<String>, provider: String }`
  - `PlaybackSourceUsed::{SpotifyNative, YouTubeFallback, YouTubeDirect, LocalFile}`
  - `RepeatMode::{Off, All, One}`, `PlaybackStatus::{Stopped, Buffering, Playing, Paused}`
  - `ResolvedStream::{LocalFile(PathBuf), HttpUrl { url: String, source: PlaybackSourceUsed }, SpotifyTrack { id: String }}`
  - `CoreError`, `async_trait` `MusicProvider` (`search`, `get_playlists`, `get_playlist_tracks`, `resolve_stream`), `LyricsProvider` (`fetch_lyrics`)

- [ ] **Step 1: Scaffold Cargo workspace and `lume-core` with failing unit tests**

Create root `Cargo.toml` declaring workspace members `crates/lume-core` (remaining crates added in their respective tasks) and shared dependencies (`serde`, `serde_json`, `thiserror`, `async-trait`, `tokio`). Add unit tests in `crates/lume-core/src/models.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn canonical_uid_is_deterministic_per_source() {
        assert_eq!(TrackSource::Spotify { id: "abc".into() }.canonical_uid(), "spotify:abc");
        assert_eq!(TrackSource::YouTubeMusic { video_id: "xyz".into() }.canonical_uid(), "ytm:xyz");
        let local_a = TrackSource::Local { path: PathBuf::from(r"D:\Music\Song.flac") }.canonical_uid();
        let local_b = TrackSource::Local { path: PathBuf::from(r"d:\music\song.flac") }.canonical_uid();
        assert!(local_a.starts_with("local:"));
        assert_eq!(local_a, local_b);
    }

    #[test]
    fn playlist_origin_mutability_matches_spec() {
        assert!(!PlaylistOrigin::SpotifySynced.is_editable());
        assert!(PlaylistOrigin::SpotifyImported { source_url: "https://open.spotify.com/playlist/1".into() }.is_editable());
        assert!(PlaylistOrigin::LocalCustom.is_editable());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lume-core`
Expected: FAIL (unimplemented types/methods).

- [ ] **Step 3: Implement `error.rs`, `models.rs`, `traits.rs`, and `lib.rs` in `crates/lume-core`**

Implement all types in **Interfaces** deriving `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` where appropriate. Normalize Windows local paths (lowercase + forward-slash normalized) before hashing/formatting `local:` UIDs so case differences never create duplicate local tracks.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p lume-core`
Expected: PASS (all tests green, zero warnings).

- [ ] **Step 5: Commit**

Run:
```powershell
git add Cargo.toml Cargo.lock crates/lume-core
git commit -m "feat(core): scaffold workspace and core domain models/traits"
```

---

### Task 2: `lume-core` Unified Queue, Shuffle & Repeat Engine

**Files:**
- Create: `crates/lume-core/src/queue.rs`
- Modify: `crates/lume-core/src/lib.rs`

**Interfaces:**
- Consumes: `Track`, `RepeatMode` from Task 1
- Produces: `QueueSnapshot`, `QueueActionResult::{Play(Track), RestartCurrent, Stopped}`, `QueueEngine` with methods:
  - `new() -> Self`
  - `set_tracks(&mut self, tracks: Vec<Track>, start_index: usize) -> Option<&Track>`
  - `add_to_queue(&mut self, track: Track)`
  - `play_next(&mut self, track: Track)`
  - `remove_at(&mut self, index: usize) -> QueueActionResult`
  - `reorder(&mut self, from: usize, to: usize)`
  - `set_shuffle(&mut self, enabled: bool)`
  - `set_repeat(&mut self, mode: RepeatMode)`
  - `next(&mut self) -> QueueActionResult`
  - `previous(&mut self, elapsed_ms: u64) -> QueueActionResult`
  - `current(&self) -> Option<&Track>`
  - `active_list(&self) -> Vec<&Track>`
  - `to_snapshot(&self) -> QueueSnapshot` / `from_snapshot(QueueSnapshot) -> Self`

- [ ] **Step 1: Write failing tests covering queue, shuffle, repeat, history, and Review Focus #3**

In `crates/lume-core/src/queue.rs`, add tests:
- `mixes_tracks_from_all_sources_and_preserves_order`
- `toggling_shuffle_off_keeps_active_track_and_restores_natural_order`
- `previous_restarts_over_3000ms_and_steps_back_history_under_3000ms`
- `repeat_one_loops_current_and_repeat_all_wraps_around`
- `removing_active_track_advances_or_stops_when_empty`
- `play_next_inserts_immediately_after_current_track`
- `snapshot_round_trip_restores_exact_position_and_modes`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-core queue`
Expected: FAIL.

- [ ] **Step 3: Implement `QueueEngine` in `crates/lume-core/src/queue.rs` and export in `lib.rs`**

Maintain `natural_order: Vec<Track>`, `order: Vec<usize>` (indices into `natural_order`), `current_pos: Option<usize>`, `history: Vec<usize>`, `shuffle: bool`, `repeat: RepeatMode`. When enabling shuffle, place the currently playing track index at position `0` and Fisher-Yates shuffle the remaining indices; when disabling shuffle, restore identity order and set `current_pos` to the active track's natural index.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-core`
Expected: PASS (all queue + model tests green).

- [ ] **Step 5: Commit**

Run:
```powershell
git add crates/lume-core
git commit -m "feat(core): add unified queue, shuffle, repeat, and history engine"
```

---

### Task 3: `lume-storage` SQLite Database, Custom Playlists, Unified `Liked Songs`, Config & Bounded Artwork Cache

**Files:**
- Modify: `Cargo.toml` (add `crates/lume-storage` member)
- Create: `crates/lume-storage/Cargo.toml`
- Create: `crates/lume-storage/src/lib.rs`
- Create: `crates/lume-storage/src/error.rs`
- Create: `crates/lume-storage/src/schema.sql`
- Create: `crates/lume-storage/src/db.rs`
- Create: `crates/lume-storage/src/config.rs`
- Create: `crates/lume-storage/src/art_cache.rs`

**Interfaces:**
- Consumes: `Track`, `TrackSource`, `Playlist`, `PlaylistOrigin`, `LyricsData`, `QueueSnapshot`, `RepeatMode` (Tasks 1–2)
- Produces:
  - `StorageError::{Sql, Io, Serialization, PlaylistReadOnly, NotFound}`
  - `Database::open(path: &Path) -> Result<Self, StorageError>` and `Database::open_in_memory() -> Result<Self, StorageError>`
  - Track & Folder ops: `upsert_track`, `get_track`, `search_local_tracks`, `prune_missing_local_tracks`, `add_local_folder`, `list_local_folders`, `remove_local_folder`
  - Playlist ops: `list_playlists`, `create_playlist`, `rename_playlist`, `delete_playlist`, `get_playlist_tracks`, `add_track_to_playlist`, `remove_track_from_playlist`, `reorder_playlist_track`, `upsert_synced_spotify_playlist`
  - Unified `Liked Songs` ops: `set_local_favorite(track_uid, liked)`, `list_unified_liked_tracks()`, `enqueue_pending_mutation(...)`
  - Lyrics cache ops: `get_cached_lyrics(cache_key, now_epoch_s) -> Result<LyricsCacheLookup, StorageError>` (`Hit(LyricsData)` | `NegativeHit` | `Miss`), `put_cached_lyrics`, `put_negative_lyrics_hit` (24h TTL = `86_400`s)
  - Session ops: `save_playback_session`, `load_playback_session`
  - `GlassmorphismMode::{DynamicAlbumBlur, WindowsAcrylic}`, `LyricsVisualMode::{SpicySmooth, UltraLight}`, `AppConfig` (`load_or_default`, `save`)
  - `ArtCache::new(cache_dir: PathBuf, max_disk_bytes: u64, max_mem_entries: usize)` (`get`, `put`, `evict_if_needed`)

- [ ] **Step 1: Write failing tests covering schema, custom mixed-source playlists, unified likes, lyrics TTL, ArtCache LRU, and Review Focus #5**

Add tests in `crates/lume-storage/src/db.rs`, `config.rs`, and `art_cache.rs`:
- `enables_wal_and_creates_all_spec_tables` (including `tracks`, `playlists`, `playlist_tracks`, `yt_mappings`, `lyrics_cache`, `local_folders`, `local_favorites`, `pending_mutations`, `playback_session`)
- `custom_playlist_supports_mixed_sources_and_reordering`
- `spotify_synced_playlist_rejects_all_mutations_with_read_only_error`
- `unified_liked_songs_combines_spotify_ytm_and_opt_in_local_only`
- `lyrics_cache_respects_24h_negative_ttl_and_infinite_positive_hit`
- `art_cache_evicts_lru_files_when_exceeding_byte_budget`
- `app_config_toml_round_trip_defaults_match_spec` (`DynamicAlbumBlur`, `SpicySmooth`, `close_to_tray = true`)

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-storage`
Expected: FAIL.

- [ ] **Step 3: Implement `schema.sql`, `db.rs`, `config.rs`, `art_cache.rs`, `error.rs`, and `lib.rs`**

Use `rusqlite` with `bundled` feature and `PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;`. Guard every mutating playlist method by verifying `is_editable == 1` and returning `StorageError::PlaylistReadOnly` otherwise. Ensure `ArtCache` tracks last-access timestamp/order for disk eviction (`max_disk_bytes` default `100 * 1024 * 1024`) and enforces `max_mem_entries` default `30`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-storage`
Expected: PASS.

- [ ] **Step 5: Commit**

Run:
```powershell
git add Cargo.toml Cargo.lock crates/lume-storage
git commit -m "feat(storage): add SQLite library, custom playlists, lyrics cache, config, and LRU art cache"
```

---

### Task 4: `lume-providers` Local Audio Scanner (`lofty` + `walkdir`)

**Files:**
- Modify: `Cargo.toml` (add `crates/lume-providers` member)
- Create: `crates/lume-providers/Cargo.toml`
- Create: `crates/lume-providers/src/lib.rs`
- Create: `crates/lume-providers/src/local.rs`

**Interfaces:**
- Consumes: `Track`, `TrackSource::Local`, `MusicProvider`, `ResolvedStream`, `CoreError` (Task 1); `Database`, `ArtCache` (Task 3)
- Produces:
  - `ScanReport { scanned: usize, added_or_updated: usize, skipped_corrupt: usize, pruned: usize }`
  - `LocalProvider` with `scan_folders(&self, folders: &[PathBuf], db: &Database, art_cache: &mut ArtCache) -> Result<ScanReport, CoreError>`, `find_sidecar_lrc(audio_path: &Path) -> Option<PathBuf>`, `read_embedded_lyrics(audio_path: &Path) -> Option<String>`, and `MusicProvider` implementation backed by SQLite local tracks.

- [ ] **Step 1: Write failing tests covering valid scan, missing tags fallback, corrupt skip, deleted prune, opt-in likes (Review Focus #2)**

In `crates/lume-providers/src/local.rs`, use `tempfile::TempDir` to generate a minimal valid PCM WAV header file (untagged) and a corrupt `.mp3` file with garbage bytes:
- `scans_untagged_wav_falling_back_to_filename_and_unknown_artist`
- `skips_corrupt_audio_file_without_aborting_scan`
- `prunes_deleted_local_files_on_incremental_rescan`
- `scanned_local_tracks_default_to_is_liked_false`
- `finds_matching_sidecar_lrc_file`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-providers local`
Expected: FAIL.

- [ ] **Step 3: Implement `LocalProvider` in `crates/lume-providers/src/local.rs`**

Filter supported extensions (`mp3`, `flac`, `ogg`, `wav`, `m4a`, `aac`, `opus`, case-insensitive). Use `lofty::read_from_path` to extract title/artists/album/duration/first picture; save extracted picture bytes to `ArtCache` under the track UID key; skip files that fail to probe and increment `skipped_corrupt`. Always insert newly scanned tracks with `is_liked = false`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-providers local`
Expected: PASS.

- [ ] **Step 5: Commit**

Run:
```powershell
git add Cargo.toml Cargo.lock crates/lume-providers
git commit -m "feat(providers): add local audio scanner with lofty metadata and incremental pruning"
```

---

### Task 5: `lume-providers` LRC Parser & Resilient LRCLIB Lyrics Provider

**Files:**
- Create: `crates/lume-providers/src/lyrics/mod.rs`
- Create: `crates/lume-providers/src/lyrics/lrc.rs`
- Create: `crates/lume-providers/src/lyrics/lrclib.rs`
- Modify: `crates/lume-providers/src/lib.rs`

**Interfaces:**
- Consumes: `Track`, `TrackSource`, `LyricsData`, `LyricsLine`, `LyricsWord`, `CoreError` (Task 1); `Database`, `LyricsCacheLookup` (Task 3); `find_sidecar_lrc`, `read_embedded_lyrics` (Task 4)
- Produces:
  - `parse_lrc(raw: &str, provider: &str) -> LyricsData`
  - `LrclibClient::new(base_url: String)` (`get_exact`, `search_best_match`)
  - `ChainedLyricsProvider::new(lrclib_base_url: String)` with `fetch(&self, track: &Track, db: &Database, force_refresh: bool) -> Result<Option<LyricsData>, CoreError>`

- [ ] **Step 1: Write failing tests covering standard/malformed/enhanced LRC and LRCLIB resolution chain (Review Focus #1)**

In `crates/lume-providers/src/lyrics/lrc.rs` and `lrclib.rs`:
- `parses_standard_two_and_three_digit_ms_and_no_fraction_timestamps`
- `expands_repeated_timestamps_and_sorts_out_of_order_lines`
- `parses_enhanced_word_level_timestamps_into_lyrics_words`
- `ignores_malformed_brackets_without_panicking`
- `chained_provider_returns_sqlite_cache_hit_without_http`
- `chained_provider_respects_24h_negative_cache_unless_force_refresh`
- `lrclib_falls_back_from_exact_get_404_to_search_duration_match` (using a tiny local `tokio::net::TcpListener` mock HTTP server)

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-providers lyrics`
Expected: FAIL.

- [ ] **Step 3: Implement `lrc.rs`, `lrclib.rs`, and `lyrics/mod.rs`**

Parse `[mm:ss]`, `[mm:ss.xx]` (centiseconds × 10), and `[mm:ss.xxx]` (milliseconds); expand multiple leading tags per line; detect inline `<mm:ss.xx> word` spans to populate `words: Some(Vec<LyricsWord>)` setting each word's `end_ms` from the next word's `start_ms` (or next line's `time_ms`). In `LrclibClient`, send a clean `User-Agent: LumeMusic/0.1` header to `https://lrclib.net`, try `/api/get` first, then `/api/search` picking the candidate with synced lyrics closest within ±5 seconds of `track.duration_ms`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-providers`
Expected: PASS.

- [ ] **Step 5: Commit**

Run:
```powershell
git add crates/lume-providers
git commit -m "feat(providers): add LRC/enhanced-LRC parser and cached LRCLIB lyrics chain"
```

---

### Task 6: `lume-audio` Unified Audio Engine (`symphonia` + `rodio`/`cpal`)

**Files:**
- Modify: `Cargo.toml` (add `crates/lume-audio` member)
- Create: `crates/lume-audio/Cargo.toml`
- Create: `crates/lume-audio/src/lib.rs`
- Create: `crates/lume-audio/src/commands.rs`
- Create: `crates/lume-audio/src/backend.rs`
- Create: `crates/lume-audio/src/engine.rs`

**Interfaces:**
- Consumes: `PlaybackStatus`, `PlaybackSourceUsed` (Task 1)
- Produces:
  - `AudioCommand::{LoadLocal { path: PathBuf, track_uid: String, duration_ms: u64 }, Pause, Resume, Seek(u64), SetVolume(f32), SetMute(bool), Stop, Shutdown}`
  - `AudioEvent::{StateChanged(PlaybackStatus), PositionTick { position_ms: u64, duration_ms: u64 }, TrackEnded { track_uid: String }, Error(String)}`
  - `AudioBackend` trait (allows deterministic unit testing without requiring audio hardware in CI, with `RodioAudioBackend` as the real Windows WASAPI implementation)
  - `AudioEngineHandle::spawn() -> Result<(Self, tokio::sync::mpsc::Receiver<AudioEvent>), String>` and `AudioEngineHandle::spawn_with_backend(backend: Box<dyn AudioBackend + Send>)` with `send(&self, cmd: AudioCommand)`

- [ ] **Step 1: Write failing tests for audio worker commands, seek/mute/volume, track-ended, and error resilience**

In `crates/lume-audio/src/engine.rs`, using a `MockAudioBackend` plus a test decoding a generated WAV file through `rodio::Decoder` (`symphonia`):
- `load_pause_resume_seek_stop_emit_expected_events`
- `mute_sets_effective_volume_to_zero_and_unmute_restores_previous_level`
- `missing_or_corrupt_file_emits_error_without_stopping_worker`
- `emits_track_ended_once_when_backend_finishes`
- `decodes_valid_wav_buffer_with_symphonia_decoder`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-audio`
Expected: FAIL.

- [ ] **Step 3: Implement `commands.rs`, `backend.rs`, `engine.rs`, and `lib.rs`**

Enable `rodio` with `symphonia-all` feature (or explicit `symphonia-mp3`, `symphonia-flac`, `symphonia-vorbis`, `symphonia-aac`, `symphonia-isomp4`, `symphonia-wav` + Opus support). Run the worker loop on a dedicated OS thread with a 100 ms tick interval while `Playing` to emit `AudioEvent::PositionTick` and detect `sink.empty()` → `AudioEvent::TrackEnded`. Clamp volume to `[0.0, 1.0]`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-audio`
Expected: PASS.

- [ ] **Step 5: Commit**

Run:
```powershell
git add Cargo.toml Cargo.lock crates/lume-audio
git commit -m "feat(audio): add dedicated symphonia/rodio audio engine worker"
```

---

### Task 7: `lume-ui` Palette Extractor, `wgpu` Dynamic Album Shader & Spicy Lyrics Spring Engine

**Files:**
- Modify: `Cargo.toml` (add `crates/lume-ui` member)
- Create: `crates/lume-ui/Cargo.toml`
- Create: `crates/lume-ui/src/lib.rs`
- Create: `crates/lume-ui/src/theme/mod.rs`
- Create: `crates/lume-ui/src/theme/palette.rs`
- Create: `crates/lume-ui/src/theme/background.wgsl`
- Create: `crates/lume-ui/src/theme/shader.rs`
- Create: `crates/lume-ui/src/lyrics_anim.rs`

**Interfaces:**
- Consumes: `LyricsLine`, `LyricsWord` (Task 1); `GlassmorphismMode`, `LyricsVisualMode` (Task 3)
- Produces:
  - `AlbumPalette { colors: [[f32; 4]; 4], scrim_alpha: f32 }` + `AlbumPalette::default_dark()` + `extract_palette(image_bytes: &[u8]) -> AlbumPalette` + `AlbumPalette::lerp(&self, target: &Self, t: f32) -> Self`
  - `DynamicBackdropProgram { palette: AlbumPalette, time_s: f32, transparent_acrylic_tint: Option<f32> }` implementing `iced::widget::shader::Program<Message>` with `wgpu` fragment shader (`background.wgsl` rendering 4-stop drifting organic mesh gradient + adaptive dark scrim)
  - `LyricsSpringState` with `update(&mut self, dt_s: f32, target_y: f32)`, `scroll_y(&self) -> f32`, `is_settled(&self) -> bool`, `active_line_index(lines: &[LyricsLine], pos_ms: u64) -> Option<usize>`, `line_visual_style(distance_from_active: isize, mode: LyricsVisualMode) -> LineStyle { scale: f32, opacity: f32, glow: f32 }`, `word_progress(word: &LyricsWord, pos_ms: u64) -> f32`

- [ ] **Step 1: Write failing tests covering palette contrast clamping (Review Focus #4) and lyrics spring physics**

In `crates/lume-ui/src/theme/palette.rs` and `crates/lume-ui/src/lyrics_anim.rs`:
- `pure_white_image_clamps_scrim_to_max_0_55`
- `pure_black_image_clamps_scrim_to_min_0_38`
- `corrupt_or_empty_image_bytes_return_safe_default_palette`
- `spring_converges_to_target_and_marks_settled`
- `active_line_index_handles_pre_start_mid_song_and_end`
- `depth_opacity_and_scale_decrease_monotonically_from_active_line`
- `word_progress_clamps_between_zero_and_one`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-ui --lib`
Expected: FAIL.

- [ ] **Step 3: Implement `palette.rs`, `background.wgsl`, `shader.rs`, `lyrics_anim.rs`, and `lib.rs`**

In `palette.rs`, decode via `image` crate, downsample to `32×32`, pick 4 well-separated dominant colors (or fallback to `default_dark()`), compute relative luminance `L = 0.2126*R + 0.7152*G + 0.0722*B`, and set `scrim_alpha = (0.38 + L * 0.17).clamp(0.38, 0.55)`. In `shader.rs`, implement `iced::widget::shader::{Pipeline, Primitive, Program}` backed by `background.wgsl` (smooth animated radial/simplex mesh blending the 4 colors with the scrim applied in fragment space). In `lyrics_anim.rs`, use critically-damped spring integration (`stiffness = 120.0`, `damping = 2.0 * stiffness.sqrt()`) and report `is_settled()` when `|offset - target| < 0.5 && |velocity| < 1.0`.

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-ui --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

Run:
```powershell
git add Cargo.toml Cargo.lock crates/lume-ui
git commit -m "feat(ui): add palette extractor, wgpu dynamic album shader, and lyrics spring engine"
```

---

### Task 8: `lume-ui` Browse / Library View, Custom Titlebar, Queue Drawer & Settings Modal

**Files:**
- Create: `crates/lume-ui/src/widgets/mod.rs`
- Create: `crates/lume-ui/src/widgets/glass.rs`
- Create: `crates/lume-ui/src/views/mod.rs`
- Create: `crates/lume-ui/src/views/titlebar.rs`
- Create: `crates/lume-ui/src/views/browse.rs`
- Create: `crates/lume-ui/src/views/player_bar.rs`
- Create: `crates/lume-ui/src/views/queue_drawer.rs`
- Create: `crates/lume-ui/src/views/settings.rs`
- Modify: `crates/lume-ui/src/lib.rs`

**Interfaces:**
- Consumes: `Track`, `Playlist`, `PlaylistOrigin`, `RepeatMode`, `PlaybackStatus`, `PlaybackSourceUsed` (Task 1); `AppConfig`, `GlassmorphismMode`, `LyricsVisualMode` (Task 3)
- Produces:
  - `SourceFilter::{All, Spotify, YouTubeMusic, Local}`
  - `ActiveNav::{Search, LikedSongs, LocalFiles, Playlist(String), Settings}`
  - `ViewMode::{Browse, NowPlayingLyrics}`
  - `Message` enum covering window chrome (`DragWindow`, `Minimize`, `ToggleMaximize`, `CloseRequested`), navigation, search (`SearchInputChanged`, `SearchSubmitted`, `SourceFilterChanged`), playlist CRUD (`CreatePlaylist`, `RenamePlaylist`, `DeletePlaylist`, `AddTrackToPlaylist`, `RemoveTrackFromPlaylist`, `ImportSpotifyUrlSubmitted`), playback controls, queue drawer, likes, lyrics, and settings toggles.
  - Reusable `glass_panel` helper (`rgba(20,20,26,0.45)`, `1px` `rgba(255,255,255,0.12)` border, rounded corners) and pure view functions `view_titlebar`, `view_browse`, `view_player_bar`, `view_queue_drawer`, `view_settings`, plus pure search-grouping helper `group_tracks_by_source(tracks: &[Track], filter: SourceFilter) -> Vec<(&'static str, Vec<&Track>)>`.

- [ ] **Step 1: Write failing unit tests for search source grouping and playlist validation**

In `crates/lume-ui/src/views/browse.rs`:
- `groups_all_filter_results_by_source_without_dedup`
- `filters_results_to_selected_single_source`
- `rejects_empty_or_whitespace_playlist_name`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-ui --lib views::browse`
Expected: FAIL.

- [ ] **Step 3: Implement glass widgets and Browse / Titlebar / PlayerBar / QueueDrawer / Settings views**

Ensure `titlebar.rs` matches the minimal top-right `─ □ ×` control cluster from `docs/superpowers/specs/assets/reference-lyrics-view.jpg`. In `browse.rs`, include unified Search bar (incremental filter for Local; Enter submits remote searches), source filter pills (`All | Spotify | YouTube Music | Local`), frosted sidebar (Search, unified `Liked Songs`, Local Files, Playlists + **+ New Playlist** + **Import Spotify URL** modal showing clear Phase 3 ready hook), tracklist with `...` context actions (`Add to Queue`, `Play Next`, `Add to Playlist`, `Like/Unlike`), bottom glass player bar (cover art + title/artist clickable to enter Now Playing/Lyrics, shuffle/prev/play-pause/next/repeat, seekbar, lyrics button, queue button, volume/mute), slide-over Queue drawer, and Settings view (Glassmorphism mode toggle, Lyrics mode toggle, Close-to-tray toggle, Local music folders add/remove + Rescan button).

- [ ] **Step 4: Run tests and check compilation**

Run: `cargo test -p lume-ui --lib`
Expected: PASS (zero errors).

- [ ] **Step 5: Commit**

Run:
```powershell
git add crates/lume-ui
git commit -m "feat(ui): add glassmorphism Browse view, custom titlebar, player bar, queue drawer, and settings"
```

---

### Task 9: `lume-ui` Now Playing & Spicy-Style Lyrics Cinema View

**Files:**
- Create: `crates/lume-ui/src/views/lyrics_view.rs`
- Modify: `crates/lume-ui/src/views/mod.rs`

**Interfaces:**
- Consumes: `Track`, `LyricsData`, `PlaybackSourceUsed`, `PlaybackStatus`, `RepeatMode` (Task 1); `LyricsSpringState`, `line_visual_style`, `word_progress` (Task 7); `Message`, `view_titlebar` (Task 8); visual reference `docs/superpowers/specs/assets/reference-lyrics-view.jpg`
- Produces:
  - `format_timestamp_ms(ms: u64) -> String` (`M:SS` matching `1:55` / `3:51` in the reference screenshot)
  - `marquee_slice(text: &str, max_chars: usize, time_s: f32) -> String`
  - `view_now_playing_lyrics(...) -> Element<'_, Message>`

- [ ] **Step 1: Write failing tests for timestamp formatting and marquee scrolling**

In `crates/lume-ui/src/views/lyrics_view.rs`:
- `formats_timestamps_matching_reference_style` (`115_000 -> "1:55"`, `231_000 -> "3:51"`, `5_000 -> "0:05"`)
- `marquee_returns_original_when_within_limit_and_scrolls_when_overflowing`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-ui --lib views::lyrics_view`
Expected: FAIL.

- [ ] **Step 3: Implement `lyrics_view.rs` matching `reference-lyrics-view.jpg`**

Build the 2-column cinema layout over the full-bleed `DynamicBackdropProgram`:
- Top bar: minimal `← Back` + `...` on top-left, frameless `─ □ ×` on top-right.
- Left column: large square album art (~340–400 px responsive) with soft drop shadow, thin interactive seekbar with left elapsed / right total `M:SS` timestamps directly underneath, bold marquee track title, softer artist subtitle, subtle source badge (`Local` / `Spotify` / `YouTube Fallback` / `YouTube`), and compact playback controls (`🔀 ⏮ ⏯ ⏭ 🔁`).
- Right column: synced lyrics list styled via `line_visual_style` (active line brighter/larger with subtle glow accent, surrounding lines faded by distance, smooth spring-interpolated auto-scroll, word-level progress rendering when `line.words` is `Some`), each synced line clickable to emit `Message::SeekTo(line.time_ms)`. Render plain lyrics cleanly if unsynced, or a minimal *"No lyrics found"* panel with a **Retry** button (`Message::RetryLyrics`).

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p lume-ui --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

Run:
```powershell
git add crates/lume-ui
git commit -m "feat(ui): add 2-column Now Playing and Spicy-style synced lyrics cinema view"
```

---

### Task 10: App Wiring, Windows 11 Acrylic (`window-vibrancy`), SMTC (`souvlaki`), System Tray (`tray-icon`) & Smart Render Pause

**Files:**
- Create: `crates/lume-ui/src/platform/mod.rs`
- Create: `crates/lume-ui/src/platform/vibrancy.rs`
- Create: `crates/lume-ui/src/platform/smtc.rs`
- Create: `crates/lume-ui/src/platform/tray.rs`
- Create: `crates/lume-ui/src/app.rs`
- Create: `crates/lume-ui/src/main.rs`
- Modify: `crates/lume-ui/src/lib.rs`

**Interfaces:**
- Consumes: All Phase 1 modules (`lume-core`, `lume-storage`, `lume-audio`, `lume-providers`, `lume-ui` theme & views)
- Produces:
  - Executable binary `lume-music` (`crates/lume-ui/src/main.rs`)
  - `should_tick_frames(window_visible: bool, minimized: bool, reduced_motion: bool, is_playing: bool, transition_active: bool, spring_settled: bool) -> bool`
  - `apply_window_glass_mode(id: window::Id, mode: GlassmorphismMode, view: ViewMode) -> Task<Message>` (calls `window::run_with_handle` → `window_vibrancy::apply_acrylic` / `clear_acrylic` on Windows 11)
  - `SmtcController` (`souvlaki` Windows System Media Transport Controls publishing metadata, artwork, playback status, and forwarding media-key events to `Message`)
  - `TrayController` (`tray-icon` menu: `Show/Hide`, `Play/Pause`, `Next`, `Previous`, `Quit`)

- [ ] **Step 1: Write failing tests for Smart Render Pause gating (`should_tick_frames`)**

In `crates/lume-ui/src/app.rs`:
- `never_ticks_when_hidden_or_minimized_even_while_playing`
- `never_ticks_when_reduced_motion_enabled`
- `stops_ticking_when_paused_and_transitions_and_spring_settled`
- `ticks_when_visible_and_playing_or_palette_transitioning`

- [ ] **Step 2: Run tests to verify failure**

Run: `cargo test -p lume-ui --lib app::tests`
Expected: FAIL.

- [ ] **Step 3: Implement `platform/{vibrancy,smtc,tray}.rs`, `app.rs`, and `main.rs`**

Wire the complete `iced::application` loop:
- Launch with `window::Settings { transparent: true, decorations: false, exit_on_close_request: false, .. }`.
- Initialize `AppConfig`, `Database`, `ArtCache`, `AudioEngineHandle`, `LocalProvider`, `ChainedLyricsProvider`, `SmtcController`, and `TrayController`.
- Restore previous `playback_session` (volume, repeat/shuffle, queue) on startup; save on exit and track/queue mutations.
- Handle `window::Event::CloseRequested`: hide window to tray when `config.close_to_tray` is true, else clean shutdown (`AudioCommand::Shutdown` + save session + exit).
- Gate `iced::window::frames()` strictly behind `should_tick_frames(...)` so hidden/minimized CPU drops to 0 FPS.
- Trigger async palette extraction & lyrics fetch on every track load without blocking the UI thread.

- [ ] **Step 4: Run full workspace test suite and release build verification**

Run: `cargo test --workspace`
Expected: PASS (all unit & integration tests across all 5 crates pass).

Run: `cargo build --release -p lume-ui`
Expected: Clean Windows binary produced at `target\release\lume-ui.exe` (or `lume-music.exe` via `[[bin]] name = "lume-music"`) with zero compilation errors.

- [ ] **Step 5: Commit**

Run:
```powershell
git add crates/lume-ui
git commit -m "feat(ui): wire Lume Music app with Windows 11 Acrylic, SMTC, system tray, and Smart Render Pause"
```
