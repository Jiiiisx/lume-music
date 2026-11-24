pub mod local;
pub mod lyrics;
pub mod spotify;
pub mod ytm;

pub use local::{
    find_sidecar_lrc, find_sidecar_ttml, read_embedded_lyrics, LocalProvider, ScanReport,
};
pub use lyrics::{parse_lrc, parse_ttml, ChainedLyricsProvider, LrclibClient};
pub use spotify::{
    build_authorize_url, build_fallback_search_query, canonical_playlist_url, clean_track_title,
    compute_pkce_s256, ensure_fresh_token, extract_spotify_playlist_id,
    fetch_spotify_track_oembed_cover, import_spotify_playlist_url, login_spotify_playback,
    login_spotify_web_api, needs_spotify_album_artwork, parse_oauth_callback_request,
    parse_spotify_embed_html, refresh_spotify_token, resolve_spotify_to_ytm_video_id,
    score_ytm_candidate, select_best_ytm_match, upgrade_spotify_album_art_url, FallbackMatch,
    ImportedSpotifyPlaylist, PkceChallenge, SpotifyAccountTier, SpotifyApiError,
    SpotifyPlaylistSummary, SpotifyUserProfile, SpotifyWebClient, SPOTIFY_PLAYBACK_CLIENT_ID,
    SPOTIFY_PLAYBACK_REDIRECT_URI, SPOTIFY_WEB_API_REDIRECT_URI,
};
pub use ytm::{
    detect_yt_dlp_binary, map_track_item, parse_yt_dlp_stdout, select_best_audio_stream,
    upgrade_ytm_cover_url, AudioStreamCandidate, YouTubeMusicProvider, YT_DLP_FORMAT_SELECTOR,
};
