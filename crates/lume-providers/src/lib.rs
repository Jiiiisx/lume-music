pub mod local;
pub mod lyrics;
pub mod spotify;
pub mod ytm;

pub use local::{
    find_sidecar_lrc, find_sidecar_ttml, read_embedded_lyrics, LocalProvider, ScanReport,
};
pub use lyrics::{
    join_syllable_words_text, parse_lrc, parse_lyricsplus_json, parse_ttml, ChainedLyricsProvider,
    LrclibClient, LyricsPlusClient, DEFAULT_LYRICSPLUS_BASES,
};
pub use spotify::{
    build_authorize_url, build_fallback_search_query, canonical_playlist_url, clean_track_title,
    compute_pkce_s256, ensure_fresh_token, exchange_authorization_code,
    extract_spotify_playlist_id, fetch_spotify_album_embed, fetch_spotify_artist_embed,
    fetch_spotify_chart_tracks, fetch_spotify_track_oembed_cover, import_spotify_playlist_url,
    login_spotify_playback, login_spotify_web_api, needs_spotify_album_artwork,
    parse_exportify_csv, parse_spotify_album_embed_html, parse_spotify_artist_embed_html,
    parse_spotify_embed_html, parse_oauth_callback_request, refresh_spotify_token,
    resolve_spotify_to_ytm_video_id, score_ytm_candidate, select_best_ytm_match,
    upgrade_spotify_album_art_url, FallbackMatch, ImportedSpotifyPlaylist, PkceChallenge,
    SpotifyAccountTier, SpotifyApiError, SpotifyArtistEmbedData, SpotifyArtistMeta,
    SpotifyCatalogSearchResult, SpotifyPlaylistSummary, SpotifyUserProfile, SpotifyWebClient,
    SPOTIFY_ACCOUNTS_TOKEN_URL, SPOTIFY_MOBILE_REDIRECT_URI, SPOTIFY_PLAYBACK_CLIENT_ID,
    SPOTIFY_PLAYBACK_REDIRECT_URI, SPOTIFY_TOP_50_GLOBAL_ID, SPOTIFY_TOP_50_INDONESIA_ID,
    SPOTIFY_WEB_API_REDIRECT_URI, SPOTIFY_WEB_API_SCOPES,
};
pub use ytm::{
    canonical_ytm_playlist_url, detect_yt_dlp_binary, map_track_item, parse_yt_dlp_stdout,
    select_best_audio_stream, upgrade_ytm_cover_url, AudioStreamCandidate, YouTubeMusicProvider,
    YtmCatalogSearchResult, YT_DLP_FORMAT_SELECTOR,
};
