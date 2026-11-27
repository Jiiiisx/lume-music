# Lume Music Mobile (iOS Standalone)

Standalone iOS client for **Lume Music**, reusing the shared Rust core (`lume-core`, `lume-storage`, `lume-providers`) via `crates/lume-ffi` (`UniFFI`) and a thin Swift Expo Module (`modules/lume-core`), with native `AVPlayer` background playback (`expo-audio`) and Lock Screen / Bluetooth remote controls.

## Features Built In
- **Native iOS Background Audio (`AVPlayer`):** Plays with screen locked, supports Lock Screen metadata & steering wheel / Bluetooth Next & Previous commands (`MPRemoteCommandCenter`), and uses a 30s forward buffer (`preferredForwardBufferDuration`) for highway driving.
- **Offline Driving Mode:** Download YouTube Music and Spotify-matched tracks (`.m4a` + lyrics) locally into SQLite + App Container storage. Dedicated **Driving Mode** screen with oversized controls and full-screen swipe gestures.
- **YouTube Music + Spotify Sync:** Anonymous YTM search & cached weekly charts (Indonesia / Global), public Spotify playlist URL import (no login needed), and Spotify OAuth 2.0 PKCE (`lumemusic://callback`) with iOS Keychain token storage (`expo-secure-store`) + Smart YTM auto-fallback.
- **Synced Lyrics:** Multi-provider chain (`SQLite -> LyricsPlus syllable-synced -> LRCLIB`) with active word highlight, auto-scroll, and tap-to-seek.

## Building `.ipa` from Windows (No Mac Needed)
1. Push this repository to GitHub (keep repository **Public** so `macos-latest` Actions runners are free).
2. Open **GitHub → Actions → `iOS Build (Lume Music)` → Run workflow**:
   - Choose **`release`** for a fully standalone `.ipa` (works anywhere offline/driving without laptop).
   - Choose **`development`** for an Expo Dev Client `.ipa` (supports 1-second Wi-Fi Fast Refresh from Windows via `npm run start:lan`).
3. Download the `LumeMusic-iOS-*.ipa` artifact when the workflow finishes (~10–15 min on first run, faster with Rust cache).
4. Sideload the `.ipa` to your iPhone using **AltServer** (`Shift + Click -> Sideload .ipa...`) or **SideStore**.

## Spotify Mobile Setup (Optional)
In [Spotify Developer Dashboard](https://developer.spotify.com/dashboard), add this Redirect URI to your app (alongside desktop `http://127.0.0.1:43821/callback`):
```text
lumemusic://callback
```

## Local Verification (Windows)
```powershell
cd mobile
npm run typecheck
npm run start:lan   # For Wi-Fi Fast Refresh with the development .ipa
```
