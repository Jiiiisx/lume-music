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
  - **Spotify Premium:** Native playback via `librespot` (up to 320 kbps Ogg Vorbis) + Spotify Web API (OAuth 2.0 Authorization Code with PKCE) for reading Playlists, Liked Songs, Search, and syncing Like/Unlike actions via Spotify's February 2026 `/me/library` endpoints. Album names and cover art are displayed on tracks only in v1 (full Saved Albums pages are out of scope) *(Designer's call)*.
  - **YouTube Music (anonymous / no Google login):** Direct search and audio streaming via `rustypipe`, with automatic fallback to an external `yt-dlp` binary if YouTube rotates PO tokens / anti-bot enforcement.
  - **Local Audio Files:** Scan user-selected folders for MP3, FLAC, OGG, WAV, M4A/AAC, and Opus tracks with embedded metadata and cover art via `lofty`.
- **Smart Auto-Fallback (Spotify → YouTube Music):**
  - Spotify playlists and Liked Songs are cached in a local SQLite database and remain browsable even when Spotify Premium is inactive or offline.
  - Under Spotify's February 2026 Development Mode rules, when the app owner's Premium lapses, **both** `librespot` playback and the Developer Mode Web API stop working. During a lapse (or offline):
    - Previously synced playlists and Liked Songs are served directly from SQLite and play automatically via YouTube Music fallback (`spotify_track_id → yt_video_id` cached in SQLite for instant subsequent plays).
    - Unified Search automatically routes to Local SQLite + YouTube Music.
    - Like/Unlike mutations are saved locally in SQLite and queued in a `pending_mutations` table to sync to Spotify `/me/library` once Premium resumes *(Designer's call)*.
  - **Playlist Import by URL *(Designer's call)*:** Followed, editorial, or external Spotify playlists (restricted under Feb 2026 Dev Mode Web API) are added on demand via **Import Playlist URL** — pasting a public Spotify playlist link extracts its tracklist into SQLite as an `imported` playlist, playable via `librespot` when Premium is active or automatically via YouTube Music fallback.
- **Unified `Liked Songs` & Custom Mixed-Source Playlists:**
  - `Liked Songs` is a unified cross-source collection combining Spotify Liked Songs (synced with `/me/library` or queued in `pending_mutations`), liked YouTube Music tracks, and opt-in liked Local audio files (local tracks are only added when explicitly liked by the user). Liking a fallback-played Spotify track likes the canonical Spotify track.
  - Users can **create, rename, delete, and customize mixed-source playlists** inside Lume Music — freely adding, removing, and reordering tracks from Spotify, YouTube Music, and Local files. Synced Spotify playlists remain read-only mirrors *(Designer's call)*; imported and custom local playlists are editable locally.
- **Complete core playback controls:** Play/Pause, Next/Previous, interactive Seek bar with elapsed/total timestamps, Shuffle (preserving back-navigation history), Repeat (`Off` / `All` / `One`), unified cross-source Queue (view, add to queue, play next, remove, reorder), Volume slider + mute, Like/Save.
- **Resilient Synced Lyrics (Spicy Lyrics-Inspired Lightweight Presentation) *(Designer's call)*:** Multi-provider fallback chain (SQLite cache → embedded/sidecar `.lrc` for local tracks → **LRCLIB** exact match → LRCLIB fuzzy search) so previously played tracks load instantaneously and stay resilient against remote server outages. Renders with critically-damped spring auto-scroll to vertical center, depth-based opacity/scale hierarchy (crisp bright active line with subtle glow, dimmed distant lines), progressive word-level karaoke highlight when enhanced/word-synced timestamps exist (gracefully falling back to smooth active-line emphasis/progress sweep for standard line-synced LRCLIB tracks), click-a-line-to-seek *(Designer's call)*, graceful plain-text fallback, and a Settings toggle (*Spicy Smooth* default vs. *Ultra-Light*).
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
- **`PlaylistOrigin` & `Playlist` Mutability:**
  - `PlaylistOrigin`: `SpotifySynced | SpotifyImported { source_url: String } | LocalCustom`.
  - `LocalCustom` and `SpotifyImported` playlists support full local CRUD (`create_playlist`, `rename_playlist`, `delete_playlist`, `add_track_to_playlist`, `remove_track_from_playlist`, `reorder_playlist_track`) across any `TrackSource`; `SpotifySynced` playlists remain read-only mirrors *(Designer's call)*.
- **`LyricsWord` & `LyricsLine`:**
  - `LyricsWord { start_ms: u64, end_ms: u64, text: String }`
  - `LyricsLine { time_ms: u64, text: String, words: Option<Vec<LyricsWord>> }` (natively supports both standard line-synced LRC and word-level enhanced LRC without changing architecture later).
- **`PlaybackSourceUsed`:** Tracks whether the active song is actually streaming via `SpotifyNative`, `YouTubeFallback`, `YouTubeDirect`, or `LocalFile` (surfaced cleanly in the UI badge).
- **`MusicProvider` (async trait):**
  - `search(&self, query: &str, limit: usize) -> Result<Vec<Track>>`
  - `get_playlists(&self) -> Result<Vec<Playlist>>` (optional/empty for anonymous YTM)
  - `get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>>`
  - `resolve_stream(&self, track: &Track) -> Result<ResolvedStream>`
- **Unified Search Behavior *(Designer's call)*:**
  - Search returns `Track` items only (`Vec<Track>`) in v1.
  - Local SQLite library filters incrementally while typing; remote searches (Spotify Web API / YouTube Music) trigger on **Enter** to avoid API rate limits and unnecessary background work.
  - When the `All` filter pill is active, available providers are queried in parallel and rendered in clearly grouped source sections (`Spotify`, `YouTube Music`, `Local`) without lossy cross-source deduplication.
  - When Spotify Web API is unavailable or Premium has lapsed, `All` seamlessly queries `Local + YouTube Music`.
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

1. **Spotify Provider (Dual OAuth PKCE Architecture) *(Designer's call)*:**
   - Custom Developer Dashboard Client IDs generally cannot authorize `librespot`'s internal `streaming` / `AudioKey` scope, while `librespot` alone does not cover the official Web API routes. Spotify therefore uses **two cleanly separated OAuth 2.0 Authorization Code with PKCE sessions**:
     1. **Playback Session (`librespot-oauth`):** Uses `librespot`'s built-in desktop/keymaster Client ID (`65b708073fc0480ea92a077233ca87bd` on `http://127.0.0.1:8898/login`) required to obtain the `streaming` scope for `librespot-core` / `AudioKey` native audio playback when Premium is active.
     2. **Library Web API Session (User's Personal Developer App Client ID):** User enters their personal Spotify Developer Client ID in Settings once (with concise in-app setup instructions noting redirect URI `http://127.0.0.1:43821/callback`, using `127.0.0.1`, never `localhost`, per 2025/2026 Spotify OAuth rules). Captures the PKCE callback on a short-lived local loopback listener to read user profile (including Premium vs. Free `product` tier), Playlists, Liked Songs, Search, and Like/Unlike mutations via Spotify's February 2026 unified `/me/library` endpoints (`PUT /me/library`, `DELETE /me/library`, `GET /me/library/contains`).
   - **Token Storage:** Both token sets (access + refresh tokens) are stored securely via Windows Credential Manager (`keyring` crate) or DPAPI-encrypted local file *(Left to builder; never plaintext)*.
   - **Resilience & Cache:** Web API results sync into SQLite so browsing works instantaneously from cache when offline or during a Premium lapse. Free-tier / lapsed sessions or `403` responses trigger clean fallback to SQLite + YouTube Music without surfacing fatal errors.
   - **External / Editorial Playlist URL Importer *(Designer's call)*:** Accepts a public Spotify playlist URL (`open.spotify.com/playlist/{id}`), extracts public track metadata (e.g., via Spotify's public embed page/JSON payload without requiring Dev Mode followed-playlist Web API permissions), saves tracks into SQLite as an editable `SpotifyImported` playlist, and plays them via `librespot` or Smart YTM Fallback.
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
   - Parser supports both standard `[mm:ss.xx]` LRC and enhanced word-level `<mm:ss.xx>` LRC tags when available from local files or future/upstream providers, storing `Vec<LyricsLine>` (with optional `Vec<LyricsWord>`) and/or plain lyrics in SQLite while working seamlessly with LRCLIB line-synced timestamps out of the box. Negative lookups cached with 24h TTL; manual **Retry / Refresh Lyrics** button bypasses cache.

### 4.5 Storage & Caching (`lume-storage`)

- **Database:** `%LOCALAPPDATA%\LumeMusic\lume.db` (`rusqlite` with bundled SQLite, WAL mode).
  - Core tables: `tracks`, `playlists` (includes `origin` [`spotify_synced` | `spotify_imported` | `local_custom`], `source_url`, `is_editable`), `playlist_tracks`, `yt_mappings`, `lyrics_cache`, `local_folders`, `local_favorites` (keyed by canonical `track_uid` for YouTube Music and opt-in Local favorites, merged with Spotify-liked tracks in the unified `Liked Songs` view), `pending_mutations` (queued Spotify Like/Unlike actions while offline or during a Premium lapse), `playback_session` (remembers last queue/track/volume/mode across restarts *(Designer's call)*).
- **Artwork Cache:** Bounded on-disk thumbnail cache at `%LOCALAPPDATA%\LumeMusic\cache\art\` (max ~100 MB LRU eviction) and bounded in-memory GPU/decoded cover cache (max 30 images) to keep RAM strictly bounded.
- **Settings File:** `%APPDATA%\LumeMusic\config.toml` (glassmorphism mode, `lyrics_visual_mode` [`spicy_smooth` | `ultra_light`], blur/animation intensity, close-to-tray behavior, fallback preference, Spotify Client ID, optional `yt-dlp` path, local folders).

### 4.6 Glassmorphism UI & Windows Integration (`lume-ui`)

- **Window & Custom Chrome:**
  - `iced 0.14` with `window::Settings { transparent: true, decorations: false, .. }`.
  - Custom top bar with drag handle, window resize edges, and clean `─ □ ×` buttons matching the user's reference screenshot ([reference-lyrics-view.jpg](assets/reference-lyrics-view.jpg)).
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
  - **Browse / Library View:** Top unified search bar + source filter pills (`All | Spotify | YouTube Music | Local`), frosted left sidebar (Search, unified cross-source `Liked Songs`, Local Files, Playlists with **+ New Playlist** and **Import Spotify URL** actions separating/badging synced Spotify, imported, and custom local playlists, Settings, Connection/Fallback status pill), scrollable track list with right-click / `...` context menu (`Add to Queue`, `Play Next`, `Add to Playlist`, `Like/Unlike`, `Re-match YouTube track` for fallback tracks), bottom glass player bar, and slide-over Queue drawer.
  - **Now Playing & Lyrics View:** 1-click toggle from the lyrics icon or album thumbnail in the player bar. Two-column cinema layout matching the user's saved reference screenshot ([reference-lyrics-view.jpg](assets/reference-lyrics-view.jpg)):
    - **Observed reference aesthetic:** Full-bleed warm/rich dynamic album-color gradient backdrop, minimal top-left `...` menu trigger and top-right frameless `─ □ ×` window control group.
    - **Left column:** Large crisp square album cover art with soft drop shadow, thin interactive seekbar with left/right `MM:SS` timestamps, bold marquee-scrolling track title when overflowing, softer artist subtitle, subtle source badge (`Spotify` / `YouTube Fallback` / `YouTube` / `Local`), and compact playback controls.
    - **Right column (Spicy Lyrics-Inspired Lightweight Rendering) *(Designer's call)*:** Driven by a lightweight critically-damped spring scroll integrator (stops ticking when settled or minimized) and depth-based scale/opacity gradient across surrounding lines (delivering Spicy Lyrics visual depth without heavy Chromium per-node CSS blur passes). The active line renders with high-contrast emphasis + subtle glow accent and progressive word-level karaoke highlight when `words` timestamps are present (or smooth active-line emphasis/progress sweep for line-synced LRC); clicking any synced line seeks playback to that timestamp. Displays plain lyrics or a clean minimal *"No lyrics found"* state with a **Retry** button if unavailable. Settings offers *Spicy Smooth* (default: spring scroll + depth fade + animated `wgpu` gradient) vs. *Ultra-Light* (static gradient + minimal animations), strictly respecting the `< 100 MB` RAM and `< 2%` active / `< 0.5%` tray CPU budget.
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
8. **Dual Spotify OAuth PKCE sessions + Feb 2026 `/me/library` API:** Uses `librespot`'s built-in keymaster Client ID (`65b708073fc0480ea92a077233ca87bd`) for native `streaming` playback and the user's personal Developer Client ID for Web API library reads and `/me/library` Like/Unlike writes.
9. **Lapsed-Premium resilience:** During a Premium lapse, serves cached playlists/Liked Songs from SQLite via YTM fallback, routes Unified Search to `Local + YouTube Music`, and queues Like/Unlike actions in `pending_mutations` for sync upon renewal.
10. **Tracks-only Unified Search on Enter:** Filters local library while typing, executes remote provider searches on Enter, and displays `All` results grouped by provider without lossy cross-source deduplication.
11. **Display-only Album metadata in v1:** Shows album names and cover art on tracks without dedicated Saved Albums syncing or Album detail pages.
12. **Public Spotify playlist URL importer via embed metadata:** External/followed Spotify playlists are imported via public playlist URL parsing (avoiding Feb 2026 Dev Mode followed-playlist API blocks) and stored locally as editable playlists.
13. **Read-only Spotify mirrors vs. editable local/imported playlists:** Synced Spotify playlists stay read-only to avoid unexpected remote mutations/403s, while custom local and imported playlists have full local mixed-source CRUD.
14. **Lightweight Spicy Lyrics visual engine:** Achieves Spicy Lyrics aesthetics using native spring scroll physics + depth opacity/scale hierarchy + optional word-level highlight over the `wgpu` gradient shader (avoiding heavy per-line GPU blur passes) with an *Ultra-Light* fallback toggle.

### Left to the Builder
- Exact crate minor versions compatible with Rust 1.99.1 and `iced 0.14` (`librespot`, `rustypipe`, `symphonia`, `rodio`/`cpal`, `rusqlite`, `lofty`, `souvlaki`, `tray-icon`, `window-vibrancy`).
- Choice between custom `librespot` PCM `Sink` into unified `rodio`/`cpal` vs. mutually-exclusive WASAPI device handoff (provided volume/mute consistency is maintained).
- Choice between `keyring` (Windows Credential Manager) and Windows DPAPI for encrypted OAuth token storage.
- Exact visual spacing, font family selection (e.g., Windows `Segoe UI Variable` / bundled sans-serif), and color extraction algorithm (k-means / median-cut).

---

## 6. High-Level Phased Roadmap (for Implementation Planning)

1. **Phase 1 — Foundation, Local Playback & Glassmorphism UI:** Workspace setup, SQLite storage (including custom mixed-source playlist CRUD & unified `Liked Songs`), Local Files scanner (`lofty`), unified `symphonia`/`rodio` audio engine, `iced 0.14` Browse + Now Playing/Lyrics views (with lightweight Spicy Lyrics spring/depth engine), `wgpu` dynamic album shader + Acrylic toggle, LRCLIB cached lyrics, SMTC + Tray + Smart Render Pause.
2. **Phase 2 — Anonymous YouTube Music Provider:** `rustypipe` search/metadata/stream + seekable temp-file audio buffering + automatic `yt-dlp` fallback + cross-source YTM likes & playlist mixing.
3. **Phase 3 — Spotify Premium, URL Importer & Smart Auto-Fallback:** Dual Spotify OAuth PKCE loopback flows, Web API playlist/Liked Songs (`/me/library`)/search sync into SQLite, public Spotify Playlist URL Importer, `librespot` native playback, and seamless `Spotify → YTM` auto-fallback matcher + `yt_mappings` cache + `pending_mutations` queue.
4. **Phase 4 — Profiling & Polish:** Verify RAM (`< 100 MB`), active CPU (`< 2%`), tray CPU (`< 0.5%`), edge-case hardening (expired Premium, 403 YTM, offline cache, bright-cover legibility).
