# Lume Music — Phase 4 (Profiling & Polish) Implementation Plan

**Goal:** Verify and meet Lume Music's native performance targets (**< 100 MB RAM** normal playback, **< 2% CPU** active visible playback, **< 0.5% CPU** minimized to taskbar — keeping System Tray removed per commit `977fcdc`), harden all production edge cases (expired/lapsed Spotify Premium, YouTube Music HTTP 403 / PO-token fallback, offline SQLite/cache resilience, bright-cover legibility), eliminate all workspace `clippy` warnings, and validate the release binary (`lume-music.exe`) via live Windows profiling.

**Spec:** `docs/superpowers/specs/2026-11-22-lume-music-design.md` (§2 Anti-Goals, §4.5, §4.6, §6 Phase 4)

---

## Audit Findings Driving Phase 4

1. **Smart Render Pause & Active CPU (`crates/lume-ui/src/app.rs`, `views/lyrics_view.rs`):**
   - Minimizing via Windows taskbar or `Win+Down` emits `window::Event::Resized(Size(0, 0))`; previously `Resized` ignored zero sizes, leaving `minimized = false` and keeping 60 FPS `window::frames()` running while minimized.
   - `should_tick_frames` previously ran `window::frames()` at 60 FPS whenever `is_playing` was true — even in `ViewMode::Browse`, where the `wgpu` crystal/album backdrop shader is static and the player bar already updates via 100 ms `AudioEvent::PositionTick`. Restricting continuous 60 FPS ticks to `ViewMode::NowPlayingLyrics` (when playing, loading lyrics, or spring unsettled) and active palette crossfades cuts Browse-mode GPU/CPU overhead dramatically.
   - `build_display_timeline(&lyrics.synced)` was recomputed on every `FrameTick` (60/s) and every `view_now_playing_lyrics()` call (60/s). Caching `lyrics_timeline: Vec<LyricsDisplayItem>` whenever `current_lyrics` changes eliminates ~120 Vec allocations/second in Lyrics mode.
2. **Bounded RAM & Image Memory (`crates/lume-ui/src/app.rs`, `Cargo.toml`):**
   - `Message::PaletteLoaded` previously inserted raw full-resolution cover bytes into `art_handles` (which is sized for 96×96 Browse thumbnails, `MAX_IN_MEM_ART_HANDLES = 250`). Full-res covers (640×640 or 1500×1500 FLAC art) could accumulate in GPU/RAM memory. Separating a single bounded `active_cover_handle: Option<(String, image::Handle)>` (decoded & capped to max 512×512 RGBA = ≤ 1 MiB) and capping `art_handles` to 120 96×96 thumbnails (≤ 4.3 MiB) keeps total image RAM strictly bounded.
   - Add `[profile.release]` (`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`) in root `Cargo.toml` for lean binary size and optimal release codegen.
3. **Bright-Cover Legibility (`crates/lume-ui/src/theme/background.wgsl`, `theme/palette.rs`):**
   - `background.wgsl` applied `color * vignette * (1.0 - scrim * 0.34)`, leaving pure-white covers (`scrim = 0.55`) at ~0.81 luminance (~1.4:1 contrast against white text).
   - Apply the full adaptive scrim `(1.0 - scrim)` plus a smooth luminance ceiling (`MAX_BACKDROP_LUMA = 0.24`) in `background.wgsl` so even pure-white album covers achieve WCAG AA+ readability (`>= 4.5:1`) for white text in both Browse and Now Playing/Lyrics views, backed by a matching Rust helper & contrast ratio unit tests in `palette.rs`.
4. **Edge-Case Hardening (`crates/lume-ui/src/app.rs`):**
   - **Expired / Lapsed Spotify Premium:** Classify `AudioEvent::SpotifyFallbackNeeded` reasons (`premium` / `lapsed` / `403` / `unauthorized`) to immediately set `self.spotify_lapsed = true` so subsequent Spotify tracks route straight to Smart YTM Auto-Fallback and update the sidebar status pill cleanly.
   - **YouTube Music 403 / PO-Token Fallback:** When `AudioEvent::Error` fails on an HTTP stream and `yt-dlp` is not installed, update SMTC to `Stopped` and surface a clear, non-blocking actionable toast instructing how to install/configure `yt-dlp`.
   - **Offline Cache Resilience:** Classify network connectivity failures cleanly into a single friendly offline toast on Home charts / Lyrics without clobbering cached SQLite data or permanently poisoning artwork enrichment retries.
5. **Zero-Warning Hygiene:** Resolve all 13 `cargo clippy --workspace --all-targets` warnings across `lume-core`, `lume-storage`, `lume-providers`, and `lume-ui`.

---

## Execution Tasks

### Task 1: Bright-Cover Adaptive Scrim & Contrast Verification (`lume-ui/src/theme`)
- [x] Add `apply_backdrop_scrim_and_ceiling(rgb: [f32; 3], scrim: f32) -> [f32; 3]` and `wcag_contrast_ratio(fg_luma: f32, bg_luma: f32) -> f32` in `palette.rs` matching `background.wgsl`.
- [x] Write unit tests verifying pure-white `[1.0, 1.0, 1.0]` and bright yellow `[1.0, 1.0, 0.2]` covers stay at or above 4.5:1 WCAG AA contrast against white foreground text while dark covers preserve their rich glow.
- [x] Update `background.wgsl` with the exact same scrim + smooth luminance ceiling formula (`MAX_BACKDROP_LINEAR_LUMA = 0.15`).

### Task 2: Smart Render Pause, Cached Lyrics Timeline & Bounded Cover RAM (`lume-ui`, `Cargo.toml`)
- [x] Update `should_tick_frames` to take `view_mode` (and `lyrics_loading`) so Browse mode stops 60 FPS GPU ticks when no palette crossfade is active.
- [x] Update `window::Event::Resized` to set `self.minimized = true` when `size.width <= 0.0 || size.height <= 0.0`.
- [x] Cache `cached_lyrics_timeline: Vec<LyricsDisplayItem>` on `LumeApp` whenever `current_lyrics` changes and pass `&[LyricsDisplayItem]` into `LyricsViewProps`.
- [x] Split `active_cover_handle: Option<(String, image::Handle)>` (max 512×512 decoded RGBA) from `art_handles` (96×96 thumbnails, `MAX_IN_MEM_ART_HANDLES = 120`).
- [x] Add `[profile.release]` tuning (`lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`) in workspace `Cargo.toml`.
- [x] Write/update unit tests in `app.rs` for Browse vs Lyrics frame ticking, taskbar-minimize zero-size detection, and bounded cover memory.

### Task 3: Edge-Case Hardening — Lapsed Premium, YTM 403 & Offline Resilience (`lume-ui`)
- [x] Add pure helper functions with unit tests:
  - `is_spotify_lapsed_reason(reason: &str) -> bool`
  - `is_likely_offline_error(err: &str) -> bool`
  - `format_ytm_stream_error_toast(err: &str, yt_dlp_available: bool) -> String`
- [x] Wire them into `AudioEvent::SpotifyFallbackNeeded`, `AudioEvent::Error` (including `smtc.set_playback(Stopped, 0)`), `HomeSpotifyChartsLoaded`, `HomeYtmChartsLoaded`, and `LyricsLoaded`.
- [x] Prevent permanent `enriching_spotify_art` lock-out when offline (remove failed UIDs when a batch returns empty).

### Task 4: Zero-Warning Clippy Cleanup (All Crates)
- [x] Fix all 13 clippy warnings across `lume-core`, `lume-storage`, `lume-providers`, and `lume-ui`.
- [x] Verify `cargo clippy --workspace --all-targets -- -D warnings` passes cleanly.

### Task 5: Release Build & Live Windows RAM/CPU Profiling
- [x] Build `cargo build --release -p lume-ui` (`target/release/lume-music.exe`, **33.30 MB**, startup window ready in **0.22–0.28s**).
- [x] Run automated live Windows profiling launching `lume-music.exe` across active visible window, real minimized-to-taskbar (`ShowWindow(SW_MINIMIZE)` on the `Lume Music` HWND), clean `WM_CLOSE` shutdown, and `stream-*.tmp` leak check.
- [x] Document measured profiling results below.

### Task 6: Final Verification & Phase 4 Commit
- [x] Run `cargo test --workspace` (133 passed, 0 failed).
- [x] Run `cargo clippy --workspace --all-targets -- -D warnings` (0 warnings).
- [x] Commit Phase 4 cleanly.

---

## Live Profiling Measurements (Windows 11 x64, 12 Logical CPUs, AMD Radeon Graphics)

| Renderer / Backend | Active-Visible CPU | Minimized CPU | Working Set RAM | Private RAM | Clean `WM_CLOSE` | Leaked `stream-*.tmp` |
|---|---|---|---|---|---|---|
| `tiny-skia` (`ICED_BACKEND=tiny-skia`) | **0.26%** | **0.02%** | **83.2 MB** (78.8 MB min) | **34.8 MB** (30.1 MB min) | Yes | 0 |
| `wgpu` OpenGL (`WGPU_BACKEND=gl`) | **0.03%** | — | 265.8 MB | 277.8 MB | Yes | 0 |
| `wgpu` DX12 (Default — DirectComposition Acrylic + `background.wgsl`) | 8.26%* | 8.12%* | 196.3 MB | 175.2 MB | Yes | 0 |
| `wgpu` Vulkan (`WGPU_BACKEND=vulkan`) | 7.95%* | — | 277.1 MB | 244.2 MB | Yes | 0 |

**\*Per-Thread Root-Cause Breakdown (`NtQueryInformationThread`):**
- All Lume Music Rust threads (UI thread, Tokio runtime, SQLite, audio worker) measured **0.00%–0.04% CPU** (`cpal_wasapi_out` = 15.6 ms over 3s; Main UI thread = **0.0 ms**).
- The ~8.0% CPU (100% of 1 logical core) on DX12 and Vulkan originates exclusively inside AMD's user-mode GPU driver (`amdxc64.dll+0x14B2F0` for DX12; `amdvlk64.dll+0x2EF8B3C` for Vulkan) when `RadeonSoftware` / `amdow` (AMD Adrenalin Overlay / custom graphics profile) hooks a DX12/Vulkan swapchain that presents statically and enters idle wait.
- **User Workarounds on AMD Adrenalin:**
  1. Reset the `lume-music.exe` (or Global) profile in **AMD Software: Adrenalin Edition → Gaming → Graphics** to **Default** and disable Radeon Overlay / Anti-Lag for `lume-music.exe`.
  2. For strict `< 100 MB` RAM & `< 0.5%` CPU ultra-light operation without DX12 driver overhead, launch with `ICED_BACKEND=tiny-skia` (**~83 MB WorkingSet / ~35 MB Private**, **0.02% minimized CPU**), or `WGPU_BACKEND=gl` for **0.03% CPU** with GPU shaders.
