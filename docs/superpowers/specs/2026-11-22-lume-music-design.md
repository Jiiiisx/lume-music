# Lume Music — Design Specification

- **Date:** 2026-11-22
- **Status:** Draft (pending user approval)
- **Working Project Name:** `lume-music` *(Designer's call — can be renamed before scaffolding)*
- **Target Platform:** Windows 11 x64 (verified host: Windows 11 Pro Build 26200, Rust 1.99.1, AMD Radeon Graphics, ~13.7 GB usable RAM)

---

## 1. Intent & The Why

The user's current Spotify Desktop + Spicetify setup (`Hazy` glassmorphism theme + `Spicy Lyrics` animated lyrics extension) consumes ~1.08 GB of RAM and ~13.6% CPU across 9 Chromium/CEF processes. Additionally, third-party Spicetify lyrics servers periodically fail with *"Lyrics are temporarily unavailable"*, and whenever Spotify Premium temporarily lapses between renewals, playback breaks completely.

**Lume Music** is a lightweight, native Rust desktop music player built to replace Spotify Desktop for everyday listening while preserving the two things the user loves most:
1. A refined **glassmorphism & dynamic album-art visual experience** (Browse mode with frosted glass panels + a clean 2-column Now Playing & Synced Lyrics cinema view).
2. **Seamless multi-source playback**: full Spotify Premium playback when active, automatic fallback to anonymous YouTube Music when Spotify Premium lapses (using the user's exact same Spotify playlists), direct YouTube Music search/playback, and local audio files.

---

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- **Ultra-lightweight footprint:** Target **< 100 MB RAM** during normal playback, **< 2% CPU** while actively visible with animations, and **< 0.5% CPU** when minimized to the system tray.
- **Full multi-source playback in one unified UI & Queue:**
  - **Spotify Premium:** Native playback via `librespot` (up to 320 kbps Ogg Vorbis) + Spotify Web API (OAuth 2.0 Authorization Code with PKCE) for reading Playlists, Liked Songs, Albums, Search, and syncing Like/Unlike actions.
  - **YouTube Music (anonymous / no Google login):** Direct search and audio streaming via `rustypipe`, with automatic fallback to an external `yt-dlp` binary if YouTube rotates PO tokens / anti-bot enforcement.
  - **Local Audio Files:** Scan user-selected folders for MP3, FLAC, OGG, WAV, M4A/AAC, and Opus tracks with embedded metadata and cover art via `lofty`.
- **Smart Auto-Fallback (Spotify → YouTube Music):**
  - Spotify playlists and Liked Songs are cached in a local SQLite database and remain browsable even when Spotify Premium is inactive or offline.
  - If `librespot` cannot play a Spotify track (Premium expired, `AudioKeyError`, session unavailable, or manual fallback toggle enabled), the app automatically matches the track on YouTube Music, streams the audio, and caches the `spotify_track_id → yt_video_id` mapping in SQLite so subsequent plays are instant.
- **Complete core playback controls:** Play/Pause, Next/Previous, interactive Seek bar with elapsed/total timestamps, Shuffle (preserving back-navigation history), Repeat (`Off` / `All` / `One`), unified cross-source Queue (view, add to queue, play next, remove, reorder), Volume slider + mute, Like/Save.
- **Resilient Synced Lyrics:** Multi-provider fallback chain (SQLite cache → embedded/sidecar `.lrc` for local tracks → **LRCLIB** exact match → LRCLIB fuzzy search) so previously played tracks load instantaneously and are resilient against remote server outages. Supports synced line highlighting, smooth auto-scroll, click-a-line-to-seek *(Designer's call)*, and graceful fallback to plain text lyrics.
- **Hybrid Glassmorphism UI (2 primary modes):**
  - **Browse / Library Mode:** Frameless window with custom titlebar (`─ □ ×`), search bar, frosted glass sidebar (Search, Liked Songs, Local Files, Playlists, active source/fallback badge), source filter chips (`All | Spotify | YouTube Music | Local`), scrollable tracklist, bottom glass player bar, and slide-over Queue drawer.
  - **Now Playing & Lyrics Mode:** 1-click transition to a minimal 2-column cinema layout (Left: large crisp album cover, seekbar with timestamps, marquee-scrolling track title, artist, source badge, core playback controls; Right: smooth-scrolling synced lyrics) over an animated dynamic album-color blur/mesh background.
  - **Toggleable Glassmorphism Style:** Default is **Dynamic Album Blur** (`wgpu` fragment shader derived from the active cover art) with frosted translucent panels; Settings provides a switch to enable **Windows 11 Native Acrylic** (`window-vibrancy`) in Browse mode.
- **Native Windows 11 Desktop Polish:** Global media keys and Windows volume/lockscreen flyout (SMTC via `souvlaki`), System Tray icon with playback controls, and **Smart Render Pause** (completely halts shader and lyrics animation ticks when minimized or hidden).

### Non-Goals (Out of Scope for v1)
- No Notch Desktop bridge (`notch-bridge.js` is retired by the user).
- No podcasts, audiobooks, Spotify Canvas video loops, friend activity/social feeds, or algorithmic Home/Discover Weekly recommendation engines.
- No Google/YouTube account sign-in or private YouTube playlist sync.
- No offline audio downloading/ripping of Spotify or YouTube tracks (only metadata, artwork thumbnails, lyrics, and track-ID match mappings are persisted locally).
- No Spotify Connect remote-receiver mode in v1 (playback is driven locally from Lume Music; Connect receiver can be added later) *(Designer's call)*.
- No Linux/macOS packaging priority in v1 (keep Rust core portable, but design and verify specifically for Windows 11 x64).

### Anti-Goals (Failure Conditions)
- Exceeding ~150 MB RAM during typical listening or keeping GPU/CPU active while minimized to tray.
- Blocking or stuttering the UI thread during network requests, YouTube stream resolution, Spotify OAuth, or lyrics fetching.
- Low-contrast unreadable white text over bright album art or bright Windows desktop wallpapers (must apply an adaptive dark scrim / contrast floor behind text and glass surfaces).
- Total playback failure when Spotify Premium expires or when YouTube rotates anti-bot tokens.

---

## 3. Architectural Approaches Considered

Before finalizing the design, three native Rust approaches were evaluated (Electron/CEF and WebView-based stacks were rejected for RAM overhead; `egui` was rejected for aesthetics/typography; `GPUI` was rejected due to rapidly shifting Windows APIs):

| Approach | Summary | Trade-offs |
|---|---|---|
| **Approach A (Selected)** | **`iced 0.14` (`wgpu` shader) + `window-vibrancy` + Pure-Rust Audio (`librespot` + `symphonia`/`rodio`) + `rustypipe` (with `yt-dlp` fallback)** | **Pros:** ~45–90 MB RAM, full custom `wgpu` shader flexibility for Spicy Lyrics-grade dynamic backgrounds, single native binary with optional `yt-dlp` escape hatch for YouTube PO-token changes. **Cons:** More explicit Rust UI code than CSS; requires careful seekable buffering for HTTP streams with `symphonia`. |
| **Approach B** | `iced 0.14` (`wgpu`) + `libmpv` for YTM/Local audio + `librespot` for Spotify | **Pros:** Battle-tested HTTP streaming/seeking out of the box. **Cons:** Requires bundling external `libmpv-2.dll` and complex Windows linking setup. |
| **Approach C** | `Slint` declarative UI + same Pure-Rust backend as Approach A | **Pros:** Faster CSS-like UI authoring. **Cons:** More limited custom animated shader backgrounds compared to `iced`'s `wgpu` pipeline; GPLv3 copyleft license for free use. |

**Why Approach A was picked:** The user prioritized native Rust lightweightness plus rich animated glassmorphism matching Spicy Lyrics/Hazy without external heavy runtime DLLs.

---

## 4. Technical Architecture

### 4.1 Cargo Workspace Layout

```text
lume-music/
├── Cargo.toml                  # Workspace root
├── docs/superpowers/specs/     # Design specifications
└── crates/
    ├── lume-core/              # Domain types, Provider traits, Queue/Shuffle, Smart Fallback matcher
    ├── lume-storage/           # SQLite cache/library, artwork disk cache, config.toml, secure token store
    ├── lume-audio/             # Unified AudioEngine (symphonia + cpal/rodio + librespot bridge)
    ├── lume-providers/         # Spotify (PKCE + Web API + librespot), YTM (rustypipe + yt-dlp), Local (lofty), Lyrics (LRCLIB)
    └── lume-ui/                # Binary crate: iced 0.14 app, wgpu shader, Acrylic toggle, SMTC, Tray
```

### 4.2 Core Domain & Traits (`lume-core`)

- **`TrackSource`:** `Spotify { id: String } | YouTubeMusic { video_id: String } | Local { path: PathBuf }`
- **`Track`:** `uid` (canonical internal ID), `source`, `title`, `artists: Vec<String>`, `album: Option<String>`, `duration_ms: u64`, `artwork_ref` (URL or local path/embedded key), `is_liked: bool`.
- **`PlaybackSourceUsed`:** Tracks whether the active song is actually streaming via `SpotifyNative`, `YouTubeFallback`, `YouTubeDirect`, or `LocalFile` (surfaced cleanly in the UI badge).
- **`MusicProvider` (async trait):**
  - `search(&self, query: &str, limit: usize) -> Result<Vec<Track>>`
  - `get_playlists(&self) -> Result<Vec<Playlist>>` (optional/empty for anonymous YTM)
  - `get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>>`
  - `resolve_stream(&self, track: &Track) -> Result<ResolvedStream>`
- **Smart Fallback Matcher (`Spotify → YTM`):**
  - Triggered when `librespot` is unauthenticated, reports Premium required / unavailable, encounters a playback error, or when user enables *"Prefer YouTube Fallback"* in Settings.
  - First checks SQLite `yt_mappings` table by `spotify_track_id`.
  - On cache miss: searches YouTube Music using `"{primary_artist} {clean_title}"`, scores top candidates by:
    1. Normalized title & artist similarity (stripping noise like `(Remastered 2011)` unless meaningful).
    2. Duration proximity (strong preference within ±5 seconds, acceptable up to ±10 seconds).
    3. Penalty for unintended `live`, `cover`, `karaoke`, `slowed`, `reverb`, `8d`, `nightcore`, or `instrumental` unless the original Spotify title contains that keyword.
  - Stores winning `yt_video_id` + confidence score in `yt_mappings` (with a UI action *"Wrong version? Re-match"* to override/clear a bad match *(Designer's call)*).
- **Queue & Shuffle Engine:**
  - Supports mixing Spotify, YouTube Music, and Local tracks in one unified queue.
  - Maintains original order + shuffled order separately so toggling Shuffle off restores natural order without losing the current track, and Previous (`⏮`) steps cleanly backward through played history.

### 4.3 Audio Engine (`lume-audio`)

Runs on a dedicated background worker (Tokio runtime + audio thread) controlled via bounded command/event channels (`AudioCommand` → `AudioEngine` → `AudioEvent`), ensuring the UI thread never blocks.

- **Unified Output & Device Management:**
  - Local files and YouTube Music streams decode via **`symphonia`** (MP3, FLAC, Vorbis, Opus, AAC/M4A, WAV) and output through **`rodio`/`cpal`** (Windows WASAPI shared mode).
  - **Solving `symphonia` HTTP seeking cleanly *(Designer's call)*:** When streaming a YouTube Music URL, an async downloader task streams chunks into a single temporary seekable buffer file in `%LOCALAPPDATA%\LumeMusic\cache\stream.tmp` (overwritten/deleted on track change and exit) while `symphonia` reads progressively from it. This guarantees instant, stutter-free seeking across the whole track while using < 5 MB RAM and without permanently saving copyrighted audio.
  - **Spotify (`librespot-playback`) Integration:** Preferred design is a custom `librespot` `Sink` adapter pushing decoded PCM into the same `cpal`/`rodio` output mixer so volume, mute, and device changes are 100% unified; if implementation reveals buffering friction, clean mutually-exclusive handoff (stopping `rodio` sink before `librespot` starts and vice versa) is acceptable *(Left to builder)*.
- **Position Ticks:** Emits playback position updates at ~100 ms intervals while playing (sufficient for smooth synced lyrics highlighting & seekbar without spamming UI state).

### 4.4 Providers & Authentication (`lume-providers`)

1. **Spotify Provider:**
   - **Authentication:** User enters their personal Spotify Developer App **Client ID** in Settings once (with concise in-app setup instructions noting redirect URI `http://127.0.0.1:43821/callback` — using `127.0.0.1`, never `localhost`, per 2025/2026 Spotify OAuth rules). App opens the system browser for **OAuth 2.0 Authorization Code with PKCE** and captures the callback on a short-lived local loopback listener.
   - **Token Storage:** Refresh/access tokens stored securely via Windows Credential Manager (`keyring` crate) or DPAPI-encrypted local file *(Left to builder; never plaintext)*.
   - **Web API:** Reads user profile (including Premium vs. Free `product` tier), Playlists, Liked Songs, Search, and Like/Unlike mutations. Syncs results into SQLite so browsing works instantly from cache even offline or rate-limited.
   - **`librespot` Session:** Uses `librespot-core` + `librespot-oauth` tokens for native audio streaming when user has Premium. Detects Free tier / session failures cleanly and delegates to Smart Fallback without surfacing fatal errors.
2. **YouTube Music Provider (Anonymous):**
   - Uses `rustypipe` (no login/cookies) for fast InnerTube music search, metadata, thumbnails, and audio stream URL extraction (trying compatible clients such as `android`/`tv` or `rustypipe-botguard` if available).
   - **Resilience Fallback:** If `rustypipe` receives HTTP 403 / PO-token rejection on stream resolution, automatically tries running `yt-dlp -f bestaudio -g --no-playlist <url>` (if `yt-dlp.exe` is detected on `PATH` or configured in Settings). If neither succeeds, shows a clear non-blocking status toast explaining how to install/update `yt-dlp`.
3. **Local Files Provider:**
   - Scans user-configured directories (`walkdir`), parses tags & embedded artwork (`lofty`), and indexes tracks into SQLite with modification-time incremental rescans.
4. **Lyrics Provider:**
   - Resolution order:
     1. Local SQLite `lyrics_cache` (instant).
     2. For local files: sidecar `.lrc` file next to the audio file or embedded `USLT`/`SYLT` lyric tags.
     3. **LRCLIB API** (`https://lrclib.net/api/get` using track name, artist, album, duration → fallback to `/api/search` best duration match).
   - Stores synced LRC (parsed into `Vec<LyricsLine { time_ms, text }>`) and/or plain lyrics in SQLite. Negative lookups cached with 24h TTL; manual **Retry / Refresh Lyrics** button bypasses cache.

### 4.5 Storage & Caching (`lume-storage`)

- **Database:** `%LOCALAPPDATA%\LumeMusic\lume.db` (`rusqlite` with bundled SQLite, WAL mode).
  - Core tables: `tracks`, `playlists`, `playlist_tracks`, `yt_mappings`, `lyrics_cache`, `local_folders`, `local_favorites`, `playback_session` (remembers last queue/track/volume/mode across restarts *(Designer's call)*).
- **Artwork Cache:** Bounded on-disk thumbnail cache at `%LOCALAPPDATA%\LumeMusic\cache\art\` (max ~100 MB LRU eviction) and bounded in-memory GPU/decoded cover cache (max 30 images) to keep RAM strictly bounded.
- **Settings File:** `%APPDATA%\LumeMusic\config.toml` (glassmorphism mode, blur/animation intensity, close-to-tray behavior, fallback preference, Spotify Client ID, optional `yt-dlp` path, local folders).

### 4.6 Glassmorphism UI & Windows Integration (`lume-ui`)

- **Window & Custom Chrome:**
  - `iced 0.14` with `window::Settings { transparent: true, decorations: false, .. }`.
  - Custom top bar with drag handle, window resize edges, and clean `─ □ ×` buttons matching the user's screenshot aesthetic.
- **Dynamic Album-Color Shader (`wgpu`):**
  - On track change, extracts 3–4 dominant colors from the cover art (on a background task) and interpolates smoothly (crossfade ~600 ms) into a custom `wgpu` fragment shader rendering a subtle, organic moving gradient/blurred-art backdrop.
  - Applies an **adaptive dark contrast scrim** (`rgba(0,0,0,0.38–0.55)` depending on cover luminance) so foreground white text and glass borders always meet comfortable readability.
  - **Hybrid Visual Toggle (Settings):**
    - *Dynamic Album Blur (Default):* Opaque window backdrop filled by the dynamic shader in both Browse and Lyrics modes; sidebar, tracklist surface, and player bar are frosted translucent panels (`rgba(20, 20, 26, 0.45)` + `1px` light border `rgba(255, 255, 255, 0.12)`).
    - *Windows 11 Acrylic Mode:* Enables `window-vibrancy` (`apply_acrylic`) so the Browse mode window subtly blurs desktop content behind it while blending a low-opacity tint of the active track's accent color; switching to Now Playing / Lyrics mode transitions cleanly to the rich full-bleed album gradient for maximum cinema readability.
- **Smart Render Pause (Crucial for CPU target):**
  - Animation subscription (`iced::window::frames()`) runs **only** when `window_visible && !minimized && !reduced_motion && (is_playing || transition_active)`.
  - When minimized to system tray or hidden, frame ticks drop to **0 FPS** — only the audio engine and SMTC remain active.
- **Two Primary Views:**
  - **Browse / Library View:** Top unified search bar + source filter pills (`All | Spotify | YouTube Music | Local`), frosted left sidebar (Search, Liked Songs, Local Files, Playlists, Settings, Connection/Fallback status pill), scrollable track list, bottom glass player bar, slide-over Queue drawer.
  - **Now Playing & Lyrics View:** 1-click toggle from the lyrics icon or album thumbnail in the player bar. Two-column layout matching the user's reference screenshot:
    - Left column: Large cover art with soft drop shadow, interactive seekbar with timestamps, marquee scrolling title when overflowing, artist name, subtle source badge (`Spotify` / `YouTube Fallback` / `YouTube` / `Local`), and compact playback controls.
    - Right column: Synced lyrics list where the active line is brighter/larger and smoothly auto-scrolls to vertical center; clicking any synced line seeks playback to that timestamp. Displays plain lyrics or a clean minimal *"No lyrics found"* state with a **Retry** button if unavailable.
- **Windows Integration:**
  - **`souvlaki` (SMTC):** Publishes active track title, artist, album, cover art, and play/pause state to Windows System Media Transport Controls; responds to keyboard Play/Pause/Next/Prev keys.
  - **`tray-icon`:** System tray icon with context menu (`Show/Hide`, `Play/Pause`, `Next`, `Previous`, `Quit`). Closing the window (`×`) minimizes to tray by default (configurable in Settings *(Designer's call)*).

---

## 5. Designer's Calls & Builder Discretion

### Calls Made in This Design (user can override any before approval)
1. **Working project name:** `lume-music` inside `C:\Users\HYPE AMD\.gemini\antigravity\scratch\lume-music`.
2. **Temporary file buffer for YTM seeking:** Stream active YouTube audio through a single temporary seekable file deleted on track switch/exit so `symphonia` seeking is instant without bloating RAM or saving permanent downloads.
3. **Click-lyric-to-seek:** Clicking a synced lyric line seeks the track to that line's timestamp.
4. **Close button (`×`) behavior:** Minimizes to system tray by default (toggleable in Settings to exit completely).
5. **Session restore:** Remembers last volume, shuffle/repeat mode, and queue across app restarts.
6. **Wrong YTM fallback override:** Includes a quick *"Re-match YouTube track"* action on fallback tracks if automated matching picks a cover/live version.
7. **No Spotify Connect remote receiver in v1:** Focuses on local playback initiated inside the app first.

### Left to the Builder
- Exact crate minor versions compatible with Rust 1.99.1 and `iced 0.14` (`librespot`, `rustypipe`, `symphonia`, `rodio`/`cpal`, `rusqlite`, `lofty`, `souvlaki`, `tray-icon`, `window-vibrancy`).
- Choice between custom `librespot` PCM `Sink` into unified `rodio`/`cpal` vs. mutually-exclusive WASAPI device handoff (provided volume/mute consistency is maintained).
- Choice between `keyring` (Windows Credential Manager) and Windows DPAPI for encrypted OAuth token storage.
- Exact visual spacing, font family selection (e.g., Windows `Segoe UI Variable` / bundled sans-serif), and color extraction algorithm (k-means / median-cut).

---

## 6. High-Level Phased Roadmap (for Implementation Planning)

1. **Phase 1 — Foundation, Local Playback & Glassmorphism UI:** Workspace setup, SQLite storage, Local Files scanner (`lofty`), unified `symphonia`/`rodio` audio engine, `iced 0.14` Browse + Now Playing/Lyrics views, `wgpu` dynamic album shader + Acrylic toggle, LRCLIB cached lyrics, SMTC + Tray + Smart Render Pause.
2. **Phase 2 — Anonymous YouTube Music Provider:** `rustypipe` search/metadata/stream + seekable temp-file audio buffering + automatic `yt-dlp` fallback.
3. **Phase 3 — Spotify Premium & Smart Auto-Fallback:** Spotify OAuth PKCE loopback flow, Web API playlist/Liked Songs/search sync into SQLite, `librespot` native playback, and seamless `Spotify → YTM` auto-fallback matcher + `yt_mappings` cache.
4. **Phase 4 — Profiling & Polish:** Verify RAM (`< 100 MB`), active CPU (`< 2%`), tray CPU (`< 0.5%`), edge-case hardening (expired Premium, 403 YTM, offline cache, bright-cover legibility).
