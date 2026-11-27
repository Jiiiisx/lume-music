PRAGMA journal_mode = WAL;
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS tracks (
    uid TEXT PRIMARY KEY,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    title TEXT NOT NULL,
    artists_json TEXT NOT NULL,
    album TEXT,
    duration_ms INTEGER NOT NULL,
    artwork_ref TEXT,
    is_liked INTEGER NOT NULL DEFAULT 0,
    file_mtime INTEGER,
    updated_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS playlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    origin TEXT NOT NULL,
    source_url TEXT,
    is_editable INTEGER NOT NULL,
    created_at INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS playlist_tracks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id TEXT NOT NULL,
    track_uid TEXT NOT NULL,
    position INTEGER NOT NULL,
    added_at INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (playlist_id) REFERENCES playlists(id) ON DELETE CASCADE,
    FOREIGN KEY (track_uid) REFERENCES tracks(uid) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order
    ON playlist_tracks(playlist_id, position);

CREATE TABLE IF NOT EXISTS yt_mappings (
    spotify_track_id TEXT PRIMARY KEY,
    yt_video_id TEXT NOT NULL,
    confidence REAL NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS lyrics_cache (
    cache_key TEXT PRIMARY KEY,
    synced_json TEXT,
    plain_text TEXT,
    provider TEXT NOT NULL,
    is_negative INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS local_folders (
    path TEXT PRIMARY KEY,
    added_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS local_favorites (
    track_uid TEXT PRIMARY KEY,
    liked_at INTEGER NOT NULL,
    FOREIGN KEY (track_uid) REFERENCES tracks(uid) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS pending_mutations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    spotify_track_id TEXT NOT NULL,
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS playback_session (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    queue_json TEXT NOT NULL,
    volume REAL NOT NULL,
    muted INTEGER NOT NULL DEFAULT 0,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS play_history (
    track_uid TEXT PRIMARY KEY,
    played_at INTEGER NOT NULL,
    play_count INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY (track_uid) REFERENCES tracks(uid) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_play_history_recent
    ON play_history(played_at DESC);

CREATE TABLE IF NOT EXISTS charts_cache (
    chart_key TEXT PRIMARY KEY,
    tracks_json TEXT NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS offline_tracks (
    track_uid TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    file_size_bytes INTEGER NOT NULL,
    mime_type TEXT NOT NULL,
    downloaded_at INTEGER NOT NULL,
    FOREIGN KEY (track_uid) REFERENCES tracks(uid) ON DELETE CASCADE
);
