pub mod api;
pub mod auth;
pub mod importer;
pub mod matcher;

pub use api::{
    SpotifyAccountTier, SpotifyApiError, SpotifyPlaylistSummary, SpotifyUserProfile,
    SpotifyWebClient,
};
pub use auth::{
    build_authorize_url, compute_pkce_s256, ensure_fresh_token, login_spotify_playback,
    login_spotify_web_api, parse_oauth_callback_request, refresh_spotify_token, PkceChallenge,
    SPOTIFY_PLAYBACK_CLIENT_ID, SPOTIFY_PLAYBACK_REDIRECT_URI, SPOTIFY_WEB_API_REDIRECT_URI,
};
pub use importer::{
    canonical_playlist_url, extract_spotify_playlist_id, fetch_spotify_chart_tracks,
    fetch_spotify_track_oembed_cover, import_spotify_playlist_from_embed_url,
    import_spotify_playlist_url, needs_spotify_album_artwork, parse_exportify_csv,
    parse_spotify_embed_html, upgrade_spotify_album_art_url, ImportedSpotifyPlaylist,
    SPOTIFY_TOP_50_GLOBAL_ID, SPOTIFY_TOP_50_INDONESIA_ID,
};
pub use matcher::{
    build_fallback_search_query, clean_track_title, resolve_spotify_to_ytm_video_id,
    score_ytm_candidate, select_best_ytm_match, FallbackMatch,
};
