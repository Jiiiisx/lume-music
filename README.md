<div align="center">
  <img src="crates/lume-ui/assets/icons/lume-music-256.png" alt="Lume Music Logo" width="112" height="112" />
  <h1>Lume Music</h1>
  <p><strong>A lightweight, native Rust desktop music player for Windows 11 with glassmorphism UI, syllable-synced lyrics, and unified multi-source playback.</strong></p>

  <p>
    <img src="https://img.shields.io/badge/Rust-2021-orange?logo=rust&logoColor=white" alt="Rust" />
    <img src="https://img.shields.io/badge/Platform-Windows%2011%20x64-0078D6?logo=windows11&logoColor=white" alt="Windows 11" />
    <img src="https://img.shields.io/badge/UI-iced%200.14%20%2B%20wgpu-4F46E5" alt="iced 0.14 + wgpu" />
    <img src="https://img.shields.io/badge/License-MIT-green" alt="MIT License" />
    <img src="https://img.shields.io/badge/Release-v1.0.0-blue" alt="v1.0.0" />
  </p>
</div>

---

## Why Lume Music?

Heavy Electron/CEF desktop music clients routinely consume **1–2 GB of RAM** across nearly a dozen background processes and break completely whenever a Spotify Premium subscription temporarily lapses or third-party lyrics plugins go down.

**Lume Music** is built natively in Rust (`iced 0.14`, `wgpu`, `symphonia`/`rodio`, `librespot`, `rustypipe`, `rusqlite`) to keep the visual polish and animated lyrics experience you love at a fraction of the memory footprint and with seamless multi-source resilience.

---

## Features

### Unified Multi-Source Playback & Queue
- **Spotify Premium (Native):** Direct Ogg Vorbis playback via `librespot` + Spotify Web API sync (Dual OAuth 2.0 PKCE with Windows **DPAPI-encrypted** token storage never stored in plaintext).
- **Smart Auto-Fallback (Spotify → YouTube Music):** Playlists and Liked Songs are cached in local SQLite. If Spotify Premium lapses, Web API returns `403`, or *"Prefer YouTube Fallback"* is enabled, tracks automatically match and stream via YouTube Music (`yt_mappings` cached in SQLite, plus one-click **Re-match YouTube track** override). Like/Unlike mutations queue in `pending_mutations` for sync upon renewal.
- **Public Playlist & CSV Import:** Import public/editorial Spotify playlists via URL (embed metadata parser) or Exportify `.csv` files into locally editable playlists.
- **Anonymous YouTube Music (No Google Login):** Fast InnerTube search, trending charts (Indonesia & Global), chunked `Range` seekable temp-file streaming (`%LOCALAPPDATA%\LumeMusic\cache\stream-*.tmp`, deleted automatically on track change/exit), and automatic `yt-dlp` fallback when YouTube enforces PO-token `403` blocks.
- **Local Audio Library:** Incremental folder scanning via `walkdir` + `lofty` supporting MP3, FLAC, OGG, WAV, M4A/AAC, and Opus with embedded metadata and cover art.
- **Unified Library:** Cross-source **Liked Songs**, custom mixed-source playlists (CRUD + reordering + persistent sorting by Custom, Recently Added, Title, Artist, Album, Duration), **Home** view with play history & charts, and a unified cross-source **Queue** with Shuffle & Repeat (`Off` / `All` / `One`).

### Visuals & Animated Synced Lyrics
- **Hybrid Glassmorphism UI:** Frameless custom window with three visual modes **Dynamic Album Blur** (custom `wgpu` fragment shader with WCAG AA luminance floor for bright album covers), **Windows 11 Native Acrylic** (`HostBackdropBrush` via DirectComposition), and **PureClear**.
- **2-Column Now Playing & Lyrics Cinema View:**
  - Multi-provider resilient lyrics chain: SQLite cache → local sidecar `.ttml`/`.lrc` & embedded tags → **LyricsPlus** (Apple Music / Musixmatch / QQ syllable-synced) → **LRCLIB**.
  - Custom `SpicyLyricLine` bloom/character-sweep highlight, interlude indicator dots, critically-damped spring auto-scroll, click-any-line-to-seek, and *Spicy Smooth* vs. *Ultra-Light* visual toggle.
- **Native Windows 11 Integration:** System Media Transport Controls (**SMTC** via `souvlaki`) for media keys & Windows lockscreen/volume flyout, plus **Smart Render Pause** that halts 60 FPS frame ticks when minimized or in static Browse mode.

---

## Architecture

Lume Music is structured as a 5-crate Cargo workspace:

```text
lume-music/
├── Cargo.toml                  # Workspace manifest & thin-LTO release profile
├── crates/
│   ├── lume-core/              # Domain models, MusicProvider trait, Queue/Shuffle/Repeat engine
│   ├── lume-storage/           # SQLite (WAL mode), LRU artwork disk cache, config.toml, Windows DPAPI token store
│   ├── lume-audio/             # Background AudioEngine worker (symphonia/rodio + librespot WASAPI handoff + seekable HTTP/yt-dlp buffer)
│   ├── lume-providers/         # Spotify (Dual PKCE, Web API, URL/CSV importer, Smart YTM Matcher), YTM (rustypipe + yt-dlp), Local (lofty), Lyrics (LyricsPlus + LRCLIB + TTML/LRC)
│   └── lume-ui/                # Binary crate (lume-music.exe): iced 0.14 UI, wgpu shader, DirectComposition Acrylic, SMTC, Win32 icon resource
├── patches/                    # Local iced_wgpu & iced_winit patches for true Windows 11 DirectComposition transparency
├── scripts/                    # Windows install & uninstall PowerShell scripts
└── docs/superpowers/           # Design specification & Phase 1–4 implementation plans
```

---

## Installation (Windows 11 x64)

### Option 1: Download Prebuilt Binary
1. Download `lume-music.exe` from the [Releases](https://github.com/Jiiiisx/lume-music/releases) page.
2. Run it directly, or place it in `%LOCALAPPDATA%\Programs\LumeMusic\` and pin it to your Start Menu / Taskbar.

### Option 2: Build & Install from Source (Recommended)
**Prerequisites:**
- Windows 11 x64
- [Rust toolchain](https://rustup.rs/) (MSVC target)
- Optional (recommended for YouTube `403` PO-token fallback):
  ```powershell
  winget install yt-dlp.yt-dlp
  ```

**Clone, build release, and install with Start Menu & Desktop shortcuts:**
```powershell
git clone https://github.com/Jiiiisx/lume-music.git
cd lume-music
powershell -ExecutionPolicy Bypass -File scripts\install-windows.ps1 -Build
```

This installs `lume-music.exe` into `%LOCALAPPDATA%\Programs\LumeMusic\`, registers **Lume Music** in Windows Start Menu, Desktop, and **Settings → Installed Apps**.

To uninstall later:
```powershell
powershell -ExecutionPolicy Bypass -File scripts\uninstall-windows.ps1
# Add -PurgeUserData to also remove local SQLite cache and config.toml
```

---

## Spotify Setup (Optional)

Lume Music works out of the box with **Local Files** and **YouTube Music** with zero login required. To connect your Spotify library and/or Premium playback:

1. Open [Spotify Developer Dashboard](https://developer.spotify.com/dashboard) and create an app.
2. Add the Redirect URI (must use `127.0.0.1`, never `localhost`):
   ```text
   http://127.0.0.1:43821/callback
   ```
3. In Lume Music → **Settings → Spotify**, paste your **Client ID**, save, and click **Connect Web API** (and optionally **Connect Native Playback** for `librespot` 320 kbps Premium streaming).
4. To import public or editorial Spotify playlists without Web API restrictions, click the **Import URL** button in the sidebar and paste any `open.spotify.com/playlist/...` link or select an Exportify `.csv` file.

---

## Performance & Renderer Modes

Measured on Windows 11 x64 (release build, `33.4 MB` binary, window ready in `~0.25s`):

| Mode / Launch Variable | Active CPU | Minimized CPU | Working Set RAM | Private RAM | Notes |
|---|---|---|---|---|---|
| Default (`DX12 wgpu`) | `< 0.1%` app* | `< 0.1%` app* | ~196 MB | ~175 MB | Full DirectComposition Acrylic + `wgpu` blurred album-art shader |
| `ICED_BACKEND=tiny-skia` | **0.26%** | **0.02%** | **~83 MB** | **~35 MB** | Ultra-lightweight CPU renderer (no `wgpu` shader overhead) |
| `WGPU_BACKEND=gl` | **0.03%** | — | ~266 MB | ~278 MB | GPU shader via OpenGL without DX12 driver polling |

> **\*AMD Radeon / Adrenalin Note:** On some AMD Adrenalin driver setups with Radeon Overlay (`amdow`) or custom game profiles enabled, `amdxc64.dll` may spawn a busy-wait driver thread (~1 core) on idle DX12 swapchains even while Lume Music's own Rust threads sit at `0.00%–0.04%` CPU. Resetting the `lume-music.exe` profile in **AMD Software: Adrenalin Edition → Gaming → Graphics** to **Default** (or launching with `WGPU_BACKEND=gl` / `ICED_BACKEND=tiny-skia`) avoids this driver quirk.

---

## Development & Verification

```powershell
# Run full workspace unit & integration tests (133 tests across 5 crates)
cargo test --workspace

# Run strict clippy lints (zero warnings)
cargo clippy --workspace --all-targets -- -D warnings

# Build optimized release binary (thin-LTO, stripped)
cargo build --release -p lume-ui
```

---

## Data Locations

- **Database & Caches:** `%LOCALAPPDATA%\LumeMusic\` (`lume.db`, `tokens.dpapi`, `cache\art\`, temporary `cache\stream-*.tmp`)
- **User Configuration:** `%APPDATA%\LumeMusic\config.toml`

---

## License

This project is licensed under the [MIT License](LICENSE).
