# Lume Music — Phase 3 (Spotify Premium, URL Importer & Smart Auto-Fallback) Implementation Plan

**Goal:** Deliver complete Spotify integration in Lume Music — Dual OAuth 2.0 PKCE sessions (Web API + `librespot`), DPAPI-encrypted token storage, Spotify Web API playlist/Liked Songs/search sync & `/me/library` mutations with offline/lapsed-Premium queueing, Public Spotify Playlist URL Importer, native `librespot` playback via mutually-exclusive WASAPI handoff, and seamless `Spotify → YouTube Music` Smart Auto-Fallback with SQLite `yt_mappings` caching and one-click *Re-match*.

**Architecture:**
- **`lume-storage` (`src/tokens.rs`, `src/db.rs`):** Windows DPAPI (`CryptProtectData` / `CryptUnprotectData`) encrypted token store at `%LOCALAPPDATA%\LumeMusic\tokens.dpapi` holding separated Web API and `librespot` OAuth token sets; SQLite helpers for `yt_mappings` CRUD, `pending_mutations` replay queue, `upsert_imported_spotify_playlist`, and `sync_spotify_liked_tracks`.
- **`lume-providers` (`src/spotify/{mod,auth,api,importer,matcher}.rs`):**
  - Dual OAuth 2.0 PKCE loopback flows: personal Developer Client ID on `http://127.0.0.1:43821/callback` for Web API + `librespot` keymaster Client ID (`65b708073fc0480ea92a077233ca87bd`) on `http://127.0.0.1:8898/login` for `streaming` scope, plus token refresh.
  - Spotify Web API client supporting profile/tier detection (`product`), paginated playlists & tracks, Liked Songs, track search, and Feb 2026 `/me/library` Like/Unlike mutations with graceful 403/lapsed-Premium detection and `pending_mutations` flush.
  - Public Spotify Playlist URL Importer fetching `open.spotify.com/embed/playlist/{id}` and parsing `__NEXT_DATA__` / initial-state JSON into editable `SpotifyImported` playlists.
  - Smart Auto-Fallback Matcher (`Spotify → YTM`) scoring YouTube Music search candidates by normalized title/artist similarity, duration proximity (±5s / ±10s), and version-noise penalties (`live`, `cover`, `karaoke`, `slowed`, `reverb`, `8d`, `nightcore`, `instrumental`), cached in `yt_mappings` with `force_rematch` override.
- **`lume-audio` (`src/spotify.rs`, `src/commands.rs`, `src/engine.rs`):** `librespot-core 0.8` + `librespot-playback 0.8` (`rodio-backend`, `native-tls`) integrated into `lume-audio` worker via mutually-exclusive WASAPI handoff with `RodioAudioBackend`, synchronized volume/mute, position tracking, and automatic `AudioEvent::SpotifyFallbackNeeded` emission when Premium is unavailable or track playback fails.
- **`lume-ui` (`src/views/{mod,settings,browse}.rs`, `src/app.rs`):** Settings Spotify card (Client ID input + setup instructions, Web API connect/sync/disconnect, native playback connect/disconnect, live `Prefer YouTube Fallback` toggle), live sidebar connection/fallback badge, functional Import Spotify URL modal, track context menu *Re-match YouTube track* action, parallel Spotify+YTM+Local unified search, automatic Spotify→YTM fallback playback, and Like/Unlike syncing with `pending_mutations` queue.

**Spec:** `docs/superpowers/specs/2026-11-22-lume-music-design.md` (§2, §4.2–§4.6, §5, §6 Phase 3)

---

## Global Constraints & Review Focus

1. **Zero plaintext credentials:** Both OAuth token sets MUST be encrypted via Windows DPAPI (`CryptProtectData` with `CRYPTPROTECT_UI_FORBIDDEN`) before touching disk; never store tokens in `config.toml` or SQLite.
2. **Spotify OAuth 2025/2026 loopback compliance:** Always bind and register `127.0.0.1` (`http://127.0.0.1:43821/callback` and `http://127.0.0.1:8898/login`), never `localhost`.
3. **Lapsed-Premium & 403 resilience:** When Spotify Web API returns HTTP 403 (Dev Mode lapsed Premium) or `librespot` fails/is disconnected, never surface fatal dead-ends — serve cached SQLite playlists/Liked Songs via Smart YTM Auto-Fallback, route search to `Local + YouTube Music`, and queue Like/Unlike actions in `pending_mutations`.
4. **Mutually-exclusive WASAPI handoff:** Starting a `librespot` track MUST stop `RodioAudioBackend` and cancel any active `ActiveHttpStream`; starting a Local/HTTP/yt-dlp track MUST stop `librespot` playback. Volume and mute state MUST stay synchronized across handoffs.
5. **Race-free async state:** Stale Spotify search, sync, import, OAuth, and fallback stream resolutions MUST respect monotonic request IDs and never overwrite newer state.

---

### Task 1: `lume-storage` — DPAPI Token Store & Phase 3 SQLite Helpers

**Files:**
- Modify: `crates/lume-storage/Cargo.toml` (add Windows `windows-sys` with `Win32_Security_Cryptography` & `Win32_Foundation`)
- Create: `crates/lume-storage/src/tokens.rs`
- Modify: `crates/lume-storage/src/db.rs`
- Modify: `crates/lume-storage/src/lib.rs`

**TDD Steps:**
- [ ] Write unit tests in `tokens.rs` verifying DPAPI round-trip encryption/decryption, ciphertext does not contain plaintext tokens, missing file returns empty bundle, and clearing individual sessions works.
- [ ] Write unit tests in `db.rs` for `yt_mappings` put/get/delete, `pending_mutations` enqueue/list/delete/dedup, `upsert_imported_spotify_playlist` (editable `SpotifyImported`), and `sync_spotify_liked_tracks` (preserving local favorites).
- [ ] Implement `tokens.rs` and `db.rs` methods.
- [ ] Run `cargo test -p lume-storage` (verify pass).
- [ ] Commit `feat(storage): add DPAPI token store and Phase 3 Spotify/fallback DB helpers`.

---

### Task 2: `lume-providers` — Spotify Dual OAuth PKCE, Web API Client & Public Playlist URL Importer

**Files:**
- Modify: `Cargo.toml` (add `sha2`, `base64`, `url`, `librespot-core`, `librespot-playback`)
- Modify: `crates/lume-providers/Cargo.toml`
- Create: `crates/lume-providers/src/spotify/mod.rs`
- Create: `crates/lume-providers/src/spotify/auth.rs`
- Create: `crates/lume-providers/src/spotify/api.rs`
- Create: `crates/lume-providers/src/spotify/importer.rs`
- Modify: `crates/lume-providers/src/lib.rs`

**TDD Steps:**
- [ ] Write unit tests in `auth.rs` (PKCE S256 challenge generation, auth URL construction with `127.0.0.1`, callback query parsing & state validation).
- [ ] Write unit tests in `api.rs` against a local mock `tokio::net::TcpListener` testing `/v1/me` tier detection, `/v1/me/playlists` + tracks pagination, `/v1/me/tracks`, `/v1/search`, `/v1/me/library` PUT/DELETE, 403 lapsed-Premium classification, and `pending_mutations` flush.
- [ ] Write unit tests in `importer.rs` for Spotify URL/URI ID extraction and `__NEXT_DATA__` embed HTML parsing into `TrackSource::Spotify` tracks.
- [ ] Implement `auth.rs`, `api.rs`, `importer.rs`.
- [ ] Run `cargo test -p lume-providers spotify` (verify pass).
- [ ] Commit `feat(providers): add Spotify dual PKCE auth, Web API client, and public URL importer`.

---

### Task 3: `lume-providers` — Smart Auto-Fallback Matcher (`Spotify → YTM`)

**Files:**
- Create: `crates/lume-providers/src/spotify/matcher.rs`
- Modify: `crates/lume-providers/src/spotify/mod.rs`
- Modify: `crates/lume-providers/src/lib.rs`

**TDD Steps:**
- [ ] Write unit tests in `matcher.rs`:
  - Strips remaster/deluxe noise from search queries while keeping intentional `(Live)` / `(Acoustic)` tags.
  - Prefers exact title + artist + duration within ±5s over cover/live/slowed/karaoke/nightcore versions.
  - Respects cached `yt_mappings` on normal lookup and skips old video ID on `force_rematch`.
- [ ] Implement `matcher.rs` (`clean_track_title`, `score_ytm_candidate`, `select_best_ytm_match`, `resolve_spotify_to_ytm_video_id`).
- [ ] Run `cargo test -p lume-providers` (verify pass).
- [ ] Commit `feat(providers): add Smart Spotify-to-YouTube Music fallback matcher`.

---

### Task 4: `lume-audio` — `librespot` Native Playback via Mutually-Exclusive WASAPI Handoff

**Files:**
- Modify: `crates/lume-audio/Cargo.toml` (add `librespot-core`, `librespot-playback`)
- Create: `crates/lume-audio/src/spotify.rs`
- Modify: `crates/lume-audio/src/commands.rs`
- Modify: `crates/lume-audio/src/engine.rs`
- Modify: `crates/lume-audio/src/lib.rs`

**TDD Steps:**
- [ ] Add `SpotifyBackend` trait abstraction + `LibrespotBackend` real implementation using `librespot-core 0.8` + `librespot-playback 0.8` (`rodio` sink + `SoftMixer`) so mutually-exclusive handoff, volume/mute sync, seek, pause/resume, end-of-track, and `SpotifyFallbackNeeded` emission are 100% unit-testable in `engine.rs`.
- [ ] Write unit tests in `engine.rs` verifying:
  - Switching Local → Spotify stops Rodio and syncs volume/mute to Spotify.
  - Switching Spotify → Local stops Spotify and resumes on Rodio.
  - Spotify unavailable/Premium error emits `AudioEvent::SpotifyFallbackNeeded` cleanly without killing the worker.
- [ ] Implement `spotify.rs`, extend `commands.rs` and `engine.rs`.
- [ ] Run `cargo test -p lume-audio` (verify pass).
- [ ] Commit `feat(audio): integrate librespot native Spotify playback with mutually-exclusive WASAPI handoff`.

---

### Task 5: `lume-ui` — Spotify Settings, Sync, URL Importer, Unified Search & Smart Auto-Fallback Wiring

**Files:**
- Modify: `crates/lume-ui/src/views/mod.rs`
- Modify: `crates/lume-ui/src/views/settings.rs`
- Modify: `crates/lume-ui/src/views/browse.rs`
- Modify: `crates/lume-ui/src/app.rs`

**Changes:**
- Wire Settings Spotify card (Client ID input + save + redirect URI help, Connect/Disconnect Web API, Sync Library Now, Connect/Disconnect Native Playback, active `Prefer YouTube Fallback` toggle).
- Wire sidebar connection/fallback status pill and playlist origin badges (`Spotify`, `Imported`).
- Wire `ImportSpotifyUrlSubmitted` to real async `import_spotify_playlist_url` → SQLite `upsert_imported_spotify_playlist`.
- Wire parallel Spotify + YTM + Local unified search on Enter / filter switch with graceful 403/lapsed fallback.
- Wire `TrackSource::Spotify` playback: native `librespot` when connected & Premium & fallback not preferred, with automatic transparent fallback to Smart YTM Matcher on `SpotifyFallbackNeeded`, lapsed Premium, or `prefer_youtube_fallback`.
- Wire track context menu *"Re-match YouTube track"* action (`Message::RematchYoutubeFallback(Track)`).
- Wire `ToggleLike` for Spotify tracks to live `/me/library` sync + `pending_mutations` offline/lapsed queue + automatic flush on sync/connect.
- Add unit tests in `lume-ui` covering Spotify fallback routing decisions and status labels.

**TDD Steps:**
- [ ] Write unit tests for Spotify playback routing & status pill logic.
- [ ] Implement UI wiring across `views/mod.rs`, `views/settings.rs`, `views/browse.rs`, `app.rs`.
- [ ] Run `cargo test -p lume-ui` (verify pass).
- [ ] Commit `feat(ui): wire Spotify OAuth, library sync, URL importer, librespot playback, and smart YTM fallback`.

---

### Task 6: Full Workspace Verification & Release Build

- [ ] Run `cargo test --workspace` (all 5 crates green, zero warnings/failures).
- [ ] Run `cargo build --release -p lume-ui` and verify clean `lume-music.exe` build.
- [ ] Verify `git status` is clean and all Phase 3 commits are tidy.
