use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use lume_core::{LyricsData, LyricsLine, Playlist, PlaylistOrigin, QueueSnapshot, Track, TrackSource};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use crate::error::StorageError;

pub const NEGATIVE_LYRICS_TTL_S: i64 = 86_400;

/// Result of querying `lyrics_cache`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsCacheLookup {
    Hit(LyricsData),
    NegativeHit,
    Miss,
}

/// Restored playback session state from SQLite.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackSession {
    pub queue: QueueSnapshot,
    pub volume: f32,
    pub muted: bool,
}

/// Thread-local / owned SQLite persistence layer for Lume Music.
pub struct Database {
    conn: Connection,
}

fn now_epoch_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn source_parts(source: &TrackSource) -> (&'static str, String) {
    match source {
        TrackSource::Spotify { id } => ("spotify", id.clone()),
        TrackSource::YouTubeMusic { video_id } => ("ytm", video_id.clone()),
        TrackSource::Local { path } => ("local", path.to_string_lossy().into_owned()),
    }
}

fn parse_track_source(source_type: &str, source_id: &str) -> TrackSource {
    match source_type {
        "spotify" => TrackSource::Spotify {
            id: source_id.to_string(),
        },
        "ytm" => TrackSource::YouTubeMusic {
            video_id: source_id.to_string(),
        },
        _ => TrackSource::Local {
            path: PathBuf::from(source_id),
        },
    }
}

fn parse_playlist_origin(origin: &str, source_url: Option<String>) -> PlaylistOrigin {
    match origin {
        "spotify_synced" => PlaylistOrigin::SpotifySynced,
        "spotify_imported" => PlaylistOrigin::SpotifyImported {
            source_url: source_url.unwrap_or_default(),
        },
        _ => PlaylistOrigin::LocalCustom,
    }
}

fn row_to_track(row: &rusqlite::Row<'_>) -> rusqlite::Result<Track> {
    let uid: String = row.get(0)?;
    let source_type: String = row.get(1)?;
    let source_id: String = row.get(2)?;
    let title: String = row.get(3)?;
    let artists_json: String = row.get(4)?;
    let album: Option<String> = row.get(5)?;
    let duration_ms: i64 = row.get(6)?;
    let artwork_ref: Option<String> = row.get(7)?;
    let is_liked: i64 = row.get(8)?;

    let artists: Vec<String> = serde_json::from_str(&artists_json).unwrap_or_default();

    Ok(Track {
        uid,
        source: parse_track_source(&source_type, &source_id),
        title,
        artists,
        album,
        duration_ms: duration_ms.max(0) as u64,
        artwork_ref,
        is_liked: is_liked != 0,
    })
}

impl Database {
    pub fn default_db_path() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("LumeMusic").join("lume.db")
    }

    pub fn open(path: &Path) -> Result<Self, StorageError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self { conn })
    }

    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(include_str!("schema.sql"))?;
        Ok(Self { conn })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    pub fn upsert_track(&self, track: &Track, file_mtime: Option<i64>) -> Result<(), StorageError> {
        let (source_type, source_id) = source_parts(&track.source);
        let artists_json = serde_json::to_string(&track.artists)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        let now = now_epoch_s();

        self.conn.execute(
            r#"
            INSERT INTO tracks (
                uid, source_type, source_id, title, artists_json, album,
                duration_ms, artwork_ref, is_liked, file_mtime, updated_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(uid) DO UPDATE SET
                source_type = excluded.source_type,
                source_id = excluded.source_id,
                title = excluded.title,
                artists_json = excluded.artists_json,
                album = excluded.album,
                duration_ms = excluded.duration_ms,
                artwork_ref = excluded.artwork_ref,
                is_liked = CASE
                    WHEN excluded.source_type = 'spotify' THEN excluded.is_liked
                    ELSE tracks.is_liked
                END,
                file_mtime = COALESCE(excluded.file_mtime, tracks.file_mtime),
                updated_at = excluded.updated_at
            "#,
            params![
                track.uid,
                source_type,
                source_id,
                track.title,
                artists_json,
                track.album,
                track.duration_ms as i64,
                track.artwork_ref,
                if track.is_liked { 1 } else { 0 },
                file_mtime,
                now,
            ],
        )?;
        Ok(())
    }

    pub fn get_track(&self, uid: &str) -> Result<Option<Track>, StorageError> {
        let track = self
            .conn
            .query_row(
                r#"
                SELECT uid, source_type, source_id, title, artists_json, album,
                       duration_ms, artwork_ref, is_liked
                FROM tracks
                WHERE uid = ?1
                "#,
                [uid],
                row_to_track,
            )
            .optional()?;
        Ok(track)
    }

    pub fn list_local_tracks(&self) -> Result<Vec<Track>, StorageError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT uid, source_type, source_id, title, artists_json, album,
                   duration_ms, artwork_ref, is_liked
            FROM tracks
            WHERE source_type = 'local'
            ORDER BY title COLLATE NOCASE ASC
            "#,
        )?;
        let rows = stmt.query_map([], row_to_track)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn search_local_tracks(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Track>, StorageError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            let mut stmt = self.conn.prepare(
                r#"
                SELECT uid, source_type, source_id, title, artists_json, album,
                       duration_ms, artwork_ref, is_liked
                FROM tracks
                WHERE source_type = 'local'
                ORDER BY title COLLATE NOCASE ASC
                LIMIT ?1
                "#,
            )?;
            let rows = stmt.query_map([limit as i64], row_to_track)?;
            let mut out = Vec::new();
            for r in rows {
                out.push(r?);
            }
            return Ok(out);
        }

        let pattern = format!("%{}%", trimmed.to_lowercase());
        let mut stmt = self.conn.prepare(
            r#"
            SELECT uid, source_type, source_id, title, artists_json, album,
                   duration_ms, artwork_ref, is_liked
            FROM tracks
            WHERE source_type = 'local'
              AND (
                  LOWER(title) LIKE ?1
                  OR LOWER(artists_json) LIKE ?1
                  OR LOWER(COALESCE(album, '')) LIKE ?1
              )
            ORDER BY title COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], row_to_track)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn search_cached_tracks(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Track>, StorageError> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.list_local_tracks();
        }
        let pattern = format!("%{}%", trimmed.to_lowercase());
        let mut stmt = self.conn.prepare(
            r#"
            SELECT uid, source_type, source_id, title, artists_json, album,
                   duration_ms, artwork_ref, is_liked
            FROM tracks
            WHERE LOWER(title) LIKE ?1
               OR LOWER(artists_json) LIKE ?1
               OR LOWER(COALESCE(album, '')) LIKE ?1
            ORDER BY title COLLATE NOCASE ASC
            LIMIT ?2
            "#,
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], row_to_track)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn prune_missing_local_tracks(
        &self,
        existing_uids: &HashSet<String>,
    ) -> Result<usize, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT uid FROM tracks WHERE source_type = 'local'")?;
        let existing_db_uids: Vec<String> = stmt
            .query_map([], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(stmt);

        let mut pruned = 0usize;
        for uid in existing_db_uids {
            if !existing_uids.contains(&uid) {
                self.conn
                    .execute("DELETE FROM tracks WHERE uid = ?1", [&uid])?;
                pruned += 1;
            }
        }
        Ok(pruned)
    }

    pub fn add_local_folder(&self, path: &Path) -> Result<(), StorageError> {
        let path_str = path.to_string_lossy().into_owned();
        self.conn.execute(
            "INSERT OR IGNORE INTO local_folders (path, added_at) VALUES (?1, ?2)",
            params![path_str, now_epoch_s()],
        )?;
        Ok(())
    }

    pub fn list_local_folders(&self) -> Result<Vec<PathBuf>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path FROM local_folders ORDER BY added_at ASC")?;
        let rows = stmt.query_map([], |r| {
            let p: String = r.get(0)?;
            Ok(PathBuf::from(p))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn remove_local_folder(&self, path: &Path) -> Result<(), StorageError> {
        let path_str = path.to_string_lossy().into_owned();
        self.conn
            .execute("DELETE FROM local_folders WHERE path = ?1", [path_str])?;
        Ok(())
    }

    fn ensure_playlist_editable(&self, playlist_id: &str) -> Result<(), StorageError> {
        let editable: Option<i64> = self
            .conn
            .query_row(
                "SELECT is_editable FROM playlists WHERE id = ?1",
                [playlist_id],
                |r| r.get(0),
            )
            .optional()?;

        match editable {
            None => Err(StorageError::NotFound(format!("playlist {playlist_id}"))),
            Some(0) => Err(StorageError::PlaylistReadOnly(playlist_id.to_string())),
            Some(_) => Ok(()),
        }
    }

    pub fn list_playlists(&self) -> Result<Vec<Playlist>, StorageError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT p.id, p.name, p.origin, p.source_url, COUNT(pt.id) AS track_count
            FROM playlists p
            LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
            GROUP BY p.id
            ORDER BY p.created_at ASC, p.name COLLATE NOCASE ASC
            "#,
        )?;
        let rows = stmt.query_map([], |r| {
            let id: String = r.get(0)?;
            let name: String = r.get(1)?;
            let origin: String = r.get(2)?;
            let source_url: Option<String> = r.get(3)?;
            let track_count: i64 = r.get(4)?;
            Ok(Playlist {
                id,
                name,
                origin: parse_playlist_origin(&origin, source_url),
                track_count: track_count.max(0) as usize,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn create_playlist(&self, name: &str) -> Result<Playlist, StorageError> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            return Err(StorageError::Serialization(
                "playlist name cannot be empty".into(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = now_epoch_s();
        self.conn.execute(
            r#"
            INSERT INTO playlists (id, name, origin, source_url, is_editable, created_at, updated_at)
            VALUES (?1, ?2, 'local_custom', NULL, 1, ?3, ?3)
            "#,
            params![id, trimmed, now],
        )?;
        Ok(Playlist {
            id,
            name: trimmed.to_string(),
            origin: PlaylistOrigin::LocalCustom,
            track_count: 0,
        })
    }

    pub fn rename_playlist(&self, playlist_id: &str, new_name: &str) -> Result<(), StorageError> {
        self.ensure_playlist_editable(playlist_id)?;
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err(StorageError::Serialization(
                "playlist name cannot be empty".into(),
            ));
        }
        self.conn.execute(
            "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![trimmed, now_epoch_s(), playlist_id],
        )?;
        Ok(())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), StorageError> {
        self.ensure_playlist_editable(playlist_id)?;
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", [playlist_id])?;
        Ok(())
    }

    pub fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<Track>, StorageError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.uid, t.source_type, t.source_id, t.title, t.artists_json, t.album,
                   t.duration_ms, t.artwork_ref, t.is_liked
            FROM playlist_tracks pt
            INNER JOIN tracks t ON t.uid = pt.track_uid
            WHERE pt.playlist_id = ?1
            ORDER BY pt.position ASC
            "#,
        )?;
        let rows = stmt.query_map([playlist_id], row_to_track)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn add_track_to_playlist(
        &self,
        playlist_id: &str,
        track_uid: &str,
    ) -> Result<(), StorageError> {
        self.ensure_playlist_editable(playlist_id)?;
        let next_pos: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_tracks WHERE playlist_id = ?1",
            [playlist_id],
            |r| r.get(0),
        )?;
        self.conn.execute(
            r#"
            INSERT INTO playlist_tracks (playlist_id, track_uid, position, added_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![playlist_id, track_uid, next_pos, now_epoch_s()],
        )?;
        Ok(())
    }

    pub fn remove_track_from_playlist(
        &self,
        playlist_id: &str,
        position: usize,
    ) -> Result<(), StorageError> {
        self.ensure_playlist_editable(playlist_id)?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND position = ?2",
            params![playlist_id, position as i64],
        )?;
        tx.execute(
            "UPDATE playlist_tracks SET position = position - 1 WHERE playlist_id = ?1 AND position > ?2",
            params![playlist_id, position as i64],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn reorder_playlist_track(
        &self,
        playlist_id: &str,
        from_pos: usize,
        to_pos: usize,
    ) -> Result<(), StorageError> {
        self.ensure_playlist_editable(playlist_id)?;
        if from_pos == to_pos {
            return Ok(());
        }
        let mut row_ids: Vec<i64> = {
            let mut stmt = self.conn.prepare(
                "SELECT id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position ASC",
            )?;
            let rows = stmt.query_map([playlist_id], |r| r.get(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        if from_pos >= row_ids.len() || to_pos >= row_ids.len() {
            return Err(StorageError::NotFound(format!(
                "playlist position out of bounds ({from_pos} -> {to_pos})"
            )));
        }

        let moved = row_ids.remove(from_pos);
        row_ids.insert(to_pos, moved);

        let tx = self.conn.unchecked_transaction()?;
        for (pos, row_id) in row_ids.into_iter().enumerate() {
            tx.execute(
                "UPDATE playlist_tracks SET position = ?1 WHERE id = ?2",
                params![pos as i64, row_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn upsert_synced_spotify_playlist(
        &self,
        id: &str,
        name: &str,
        tracks: &[Track],
    ) -> Result<(), StorageError> {
        for track in tracks {
            self.upsert_track(track, None)?;
        }
        let now = now_epoch_s();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            r#"
            INSERT INTO playlists (id, name, origin, source_url, is_editable, created_at, updated_at)
            VALUES (?1, ?2, 'spotify_synced', NULL, 0, ?3, ?3)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                origin = 'spotify_synced',
                is_editable = 0,
                updated_at = excluded.updated_at
            "#,
            params![id, name, now],
        )?;
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", [id])?;
        for (pos, track) in tracks.iter().enumerate() {
            tx.execute(
                r#"
                INSERT INTO playlist_tracks (playlist_id, track_uid, position, added_at)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![id, track.uid, pos as i64, now],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn set_local_favorite(&self, track_uid: &str, liked: bool) -> Result<(), StorageError> {
        let updated = self.conn.execute(
            "UPDATE tracks SET is_liked = ?1 WHERE uid = ?2",
            params![if liked { 1 } else { 0 }, track_uid],
        )?;
        if updated == 0 {
            return Err(StorageError::NotFound(format!("track {track_uid}")));
        }

        if liked {
            self.conn.execute(
                "INSERT OR REPLACE INTO local_favorites (track_uid, liked_at) VALUES (?1, ?2)",
                params![track_uid, now_epoch_s()],
            )?;
        } else {
            self.conn.execute(
                "DELETE FROM local_favorites WHERE track_uid = ?1",
                [track_uid],
            )?;
        }
        Ok(())
    }

    pub fn list_unified_liked_tracks(&self) -> Result<Vec<Track>, StorageError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT t.uid, t.source_type, t.source_id, t.title, t.artists_json, t.album,
                   t.duration_ms, t.artwork_ref, 1 AS is_liked
            FROM tracks t
            LEFT JOIN local_favorites lf ON lf.track_uid = t.uid
            WHERE t.is_liked = 1 OR lf.track_uid IS NOT NULL
            ORDER BY COALESCE(lf.liked_at, t.updated_at) DESC, t.title COLLATE NOCASE ASC
            "#,
        )?;
        let rows = stmt.query_map([], row_to_track)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn enqueue_pending_mutation(
        &self,
        action: &str,
        spotify_track_id: &str,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO pending_mutations (action, spotify_track_id, created_at) VALUES (?1, ?2, ?3)",
            params![action, spotify_track_id, now_epoch_s()],
        )?;
        Ok(())
    }

    pub fn get_cached_lyrics(
        &self,
        cache_key: &str,
        now_epoch_s: i64,
    ) -> Result<LyricsCacheLookup, StorageError> {
        let row: Option<(Option<String>, Option<String>, String, i64, i64)> = self
            .conn
            .query_row(
                r#"
                SELECT synced_json, plain_text, provider, is_negative, updated_at
                FROM lyrics_cache
                WHERE cache_key = ?1
                "#,
                [cache_key],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .optional()?;

        let Some((synced_json, plain_text, provider, is_negative, updated_at)) = row else {
            return Ok(LyricsCacheLookup::Miss);
        };

        if is_negative != 0 {
            if now_epoch_s - updated_at > NEGATIVE_LYRICS_TTL_S {
                return Ok(LyricsCacheLookup::Miss);
            }
            return Ok(LyricsCacheLookup::NegativeHit);
        }

        let synced: Vec<LyricsLine> = match synced_json {
            Some(raw) => {
                serde_json::from_str(&raw).map_err(|e| StorageError::Serialization(e.to_string()))?
            }
            None => Vec::new(),
        };

        Ok(LyricsCacheLookup::Hit(LyricsData {
            synced,
            plain: plain_text,
            provider,
        }))
    }

    pub fn put_cached_lyrics(
        &self,
        cache_key: &str,
        lyrics: &LyricsData,
        now_epoch_s: i64,
    ) -> Result<(), StorageError> {
        let synced_json = serde_json::to_string(&lyrics.synced)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.conn.execute(
            r#"
            INSERT INTO lyrics_cache (cache_key, synced_json, plain_text, provider, is_negative, updated_at)
            VALUES (?1, ?2, ?3, ?4, 0, ?5)
            ON CONFLICT(cache_key) DO UPDATE SET
                synced_json = excluded.synced_json,
                plain_text = excluded.plain_text,
                provider = excluded.provider,
                is_negative = 0,
                updated_at = excluded.updated_at
            "#,
            params![cache_key, synced_json, lyrics.plain, lyrics.provider, now_epoch_s],
        )?;
        Ok(())
    }

    pub fn put_negative_lyrics_hit(
        &self,
        cache_key: &str,
        now_epoch_s: i64,
    ) -> Result<(), StorageError> {
        self.conn.execute(
            r#"
            INSERT INTO lyrics_cache (cache_key, synced_json, plain_text, provider, is_negative, updated_at)
            VALUES (?1, NULL, NULL, 'none', 1, ?2)
            ON CONFLICT(cache_key) DO UPDATE SET
                synced_json = NULL,
                plain_text = NULL,
                provider = 'none',
                is_negative = 1,
                updated_at = excluded.updated_at
            "#,
            params![cache_key, now_epoch_s],
        )?;
        Ok(())
    }

    pub fn save_playback_session(
        &self,
        queue: &QueueSnapshot,
        volume: f32,
        muted: bool,
    ) -> Result<(), StorageError> {
        let queue_json = serde_json::to_string(queue)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;
        self.conn.execute(
            r#"
            INSERT INTO playback_session (id, queue_json, volume, muted, updated_at)
            VALUES (1, ?1, ?2, ?3, ?4)
            ON CONFLICT(id) DO UPDATE SET
                queue_json = excluded.queue_json,
                volume = excluded.volume,
                muted = excluded.muted,
                updated_at = excluded.updated_at
            "#,
            params![
                queue_json,
                volume as f64,
                if muted { 1 } else { 0 },
                now_epoch_s(),
            ],
        )?;
        Ok(())
    }

    pub fn load_playback_session(&self) -> Result<Option<PlaybackSession>, StorageError> {
        let row: Option<(String, f64, i64)> = self
            .conn
            .query_row(
                "SELECT queue_json, volume, muted FROM playback_session WHERE id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        let Some((queue_json, volume, muted)) = row else {
            return Ok(None);
        };

        let queue: QueueSnapshot = serde_json::from_str(&queue_json)
            .map_err(|e| StorageError::Serialization(e.to_string()))?;

        Ok(Some(PlaybackSession {
            queue,
            volume: volume as f32,
            muted: muted != 0,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lume_core::{LyricsLine, TrackSource};

    fn track_sp(id: &str, title: &str, liked: bool) -> Track {
        let source = TrackSource::Spotify { id: id.to_string() };
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.to_string(),
            artists: vec!["Spotify Artist".into()],
            album: Some("Album".into()),
            duration_ms: 200_000,
            artwork_ref: None,
            is_liked: liked,
        }
    }

    fn track_ytm(id: &str, title: &str) -> Track {
        let source = TrackSource::YouTubeMusic {
            video_id: id.to_string(),
        };
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.to_string(),
            artists: vec!["YTM Artist".into()],
            album: None,
            duration_ms: 190_000,
            artwork_ref: None,
            is_liked: false,
        }
    }

    fn track_local(path: &str, title: &str) -> Track {
        let source = TrackSource::Local {
            path: PathBuf::from(path),
        };
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.to_string(),
            artists: vec!["Local Artist".into()],
            album: Some("Local Album".into()),
            duration_ms: 210_000,
            artwork_ref: None,
            is_liked: false,
        }
    }

    #[test]
    fn enables_wal_and_creates_all_spec_tables() {
        let dir = tempfile::TempDir::new().unwrap();
        let db = Database::open(&dir.path().join("lume.db")).unwrap();
        let mode: String = db
            .connection()
            .query_row("PRAGMA journal_mode;", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_lowercase(), "wal");

        let expected_tables = [
            "tracks",
            "playlists",
            "playlist_tracks",
            "yt_mappings",
            "lyrics_cache",
            "local_folders",
            "local_favorites",
            "pending_mutations",
            "playback_session",
        ];
        for tbl in expected_tables {
            let exists: i64 = db
                .connection()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [tbl],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "missing table {tbl}");
        }
    }

    #[test]
    fn custom_playlist_supports_mixed_sources_and_reordering() {
        let db = Database::open_in_memory().unwrap();
        let sp = track_sp("sp1", "SP Song", false);
        let yt = track_ytm("yt1", "YTM Song");
        let loc = track_local(r"D:\Music\loc.flac", "Local Song");
        db.upsert_track(&sp, None).unwrap();
        db.upsert_track(&yt, None).unwrap();
        db.upsert_track(&loc, Some(10)).unwrap();

        let pl = db.create_playlist("Mixed Roadtrip").unwrap();
        db.add_track_to_playlist(&pl.id, &sp.uid).unwrap();
        db.add_track_to_playlist(&pl.id, &yt.uid).unwrap();
        db.add_track_to_playlist(&pl.id, &loc.uid).unwrap();

        db.reorder_playlist_track(&pl.id, 2, 0).unwrap();
        let ordered = db.get_playlist_tracks(&pl.id).unwrap();
        assert_eq!(
            ordered.iter().map(|t| t.uid.as_str()).collect::<Vec<_>>(),
            vec![loc.uid.as_str(), sp.uid.as_str(), yt.uid.as_str()]
        );

        db.remove_track_from_playlist(&pl.id, 1).unwrap();
        let after_remove = db.get_playlist_tracks(&pl.id).unwrap();
        assert_eq!(after_remove.len(), 2);
        assert_eq!(after_remove[0].uid, loc.uid);
        assert_eq!(after_remove[1].uid, yt.uid);
    }

    #[test]
    fn spotify_synced_playlist_rejects_all_mutations_with_read_only_error() {
        let db = Database::open_in_memory().unwrap();
        let sp = track_sp("sp1", "SP Song", false);
        let yt = track_ytm("yt1", "YTM Song");
        db.upsert_track(&yt, None).unwrap();
        db.upsert_synced_spotify_playlist("sp_pl_1", "Daily Mix", &[sp.clone()])
            .unwrap();

        assert!(matches!(
            db.rename_playlist("sp_pl_1", "Edited"),
            Err(StorageError::PlaylistReadOnly(_))
        ));
        assert!(matches!(
            db.delete_playlist("sp_pl_1"),
            Err(StorageError::PlaylistReadOnly(_))
        ));
        assert!(matches!(
            db.add_track_to_playlist("sp_pl_1", &yt.uid),
            Err(StorageError::PlaylistReadOnly(_))
        ));
        assert!(matches!(
            db.remove_track_from_playlist("sp_pl_1", 0),
            Err(StorageError::PlaylistReadOnly(_))
        ));
        assert!(matches!(
            db.reorder_playlist_track("sp_pl_1", 0, 0),
            Err(StorageError::PlaylistReadOnly(_))
        ));
    }

    #[test]
    fn unified_liked_songs_combines_spotify_ytm_and_opt_in_local_only() {
        let db = Database::open_in_memory().unwrap();
        let sp_liked = track_sp("sp_like", "Liked SP", true);
        let sp_unliked = track_sp("sp_plain", "Plain SP", false);
        let ytm = track_ytm("yt_like", "Liked YTM");
        let loc_opt_in = track_local(r"D:\Music\fav.mp3", "Liked Local");
        let loc_not_liked = track_local(r"D:\Music\plain.mp3", "Plain Local");

        for t in [&sp_liked, &sp_unliked, &ytm, &loc_opt_in, &loc_not_liked] {
            db.upsert_track(t, None).unwrap();
        }
        db.set_local_favorite(&ytm.uid, true).unwrap();
        db.set_local_favorite(&loc_opt_in.uid, true).unwrap();

        let liked = db.list_unified_liked_tracks().unwrap();
        let liked_uids: HashSet<&str> = liked.iter().map(|t| t.uid.as_str()).collect();
        assert_eq!(liked_uids.len(), 3);
        assert!(liked_uids.contains(sp_liked.uid.as_str()));
        assert!(liked_uids.contains(ytm.uid.as_str()));
        assert!(liked_uids.contains(loc_opt_in.uid.as_str()));
        assert!(!liked_uids.contains(loc_not_liked.uid.as_str()));
    }

    #[test]
    fn lyrics_cache_respects_24h_negative_ttl_and_infinite_positive_hit() {
        let db = Database::open_in_memory().unwrap();
        let lyrics = LyricsData {
            synced: vec![LyricsLine {
                time_ms: 1000,
                text: "Hello".into(),
                words: None,
            }],
            plain: Some("Hello".into()),
            provider: "lrclib".into(),
        };

        db.put_cached_lyrics("track:pos", &lyrics, 1_000).unwrap();
        assert_eq!(
            db.get_cached_lyrics("track:pos", 1_000 + 10_000_000)
                .unwrap(),
            LyricsCacheLookup::Hit(lyrics)
        );

        db.put_negative_lyrics_hit("track:neg", 1_000).unwrap();
        assert_eq!(
            db.get_cached_lyrics("track:neg", 1_000 + 3600).unwrap(),
            LyricsCacheLookup::NegativeHit
        );
        assert_eq!(
            db.get_cached_lyrics("track:neg", 1_000 + NEGATIVE_LYRICS_TTL_S + 1)
                .unwrap(),
            LyricsCacheLookup::Miss
        );
    }
}
