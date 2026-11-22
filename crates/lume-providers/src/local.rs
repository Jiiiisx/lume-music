use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use lofty::prelude::{Accessor, AudioFile, ItemKey, TaggedFileExt};
use lume_core::{CoreError, MusicProvider, Playlist, ResolvedStream, Track, TrackSource};
use lume_storage::{ArtCache, Database, StorageError};
use walkdir::WalkDir;

const SUPPORTED_EXTENSIONS: &[&str] = &["mp3", "flac", "ogg", "wav", "m4a", "aac", "opus"];

fn map_storage_err(err: StorageError) -> CoreError {
    CoreError::Storage(err.to_string())
}

fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| {
            let lower = ext.to_ascii_lowercase();
            SUPPORTED_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}

fn file_mtime_secs(path: &Path) -> Option<i64> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
}

/// Summary returned after scanning local audio folders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanReport {
    pub scanned: usize,
    pub added_or_updated: usize,
    pub skipped_corrupt: usize,
    pub pruned: usize,
}

/// Local filesystem audio scanner and provider backed by SQLite + `lofty`.
#[derive(Debug, Clone)]
pub struct LocalProvider {
    db_path: PathBuf,
}

impl LocalProvider {
    pub fn new(db_path: PathBuf) -> Self {
        Self { db_path }
    }

    pub fn scan_folders(
        &self,
        folders: &[PathBuf],
        db: &Database,
        art_cache: &mut ArtCache,
    ) -> Result<ScanReport, CoreError> {
        let mut report = ScanReport::default();
        let mut seen_uids = HashSet::new();

        for folder in folders {
            if !folder.exists() {
                continue;
            }
            for entry in WalkDir::new(folder).follow_links(false).into_iter().flatten() {
                let path = entry.path();
                if !path.is_file() || !is_supported_audio_file(path) {
                    continue;
                }

                report.scanned += 1;

                let tagged = match lofty::read_from_path(path) {
                    Ok(t) => t,
                    Err(_) => {
                        report.skipped_corrupt += 1;
                        continue;
                    }
                };

                let source = TrackSource::Local {
                    path: path.to_path_buf(),
                };
                let uid = source.canonical_uid();
                seen_uids.insert(uid.clone());

                let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
                let fallback_title = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("Unknown Track")
                    .to_string();

                let title = tag
                    .and_then(|t| t.title())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(fallback_title);

                let artist = tag
                    .and_then(|t| t.artist())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "Unknown Artist".to_string());

                let album = tag
                    .and_then(|t| t.album())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty());

                let duration_ms = tagged.properties().duration().as_millis() as u64;

                let mut artwork_ref = None;
                if let Some(pic) = tag.and_then(|t| t.pictures().first()) {
                    if art_cache.put(&uid, pic.data()).is_ok() {
                        artwork_ref = Some(uid.clone());
                    }
                }

                let track = Track {
                    uid,
                    source,
                    title,
                    artists: vec![artist],
                    album,
                    duration_ms,
                    artwork_ref,
                    is_liked: false,
                };

                db.upsert_track(&track, file_mtime_secs(path))
                    .map_err(map_storage_err)?;
                report.added_or_updated += 1;
            }
        }

        report.pruned = db
            .prune_missing_local_tracks(&seen_uids)
            .map_err(map_storage_err)?;
        Ok(report)
    }
}

pub fn find_sidecar_lrc(audio_path: &Path) -> Option<PathBuf> {
    let candidate = audio_path.with_extension("lrc");
    candidate.is_file().then_some(candidate)
}

pub fn read_embedded_lyrics(audio_path: &Path) -> Option<String> {
    let tagged = lofty::read_from_path(audio_path).ok()?;
    for tag in tagged.tags() {
        if let Some(text) = tag
            .get_string(ItemKey::Lyrics)
            .or_else(|| tag.get_string(ItemKey::UnsyncLyrics))
        {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }
    None
}

#[async_trait]
impl MusicProvider for LocalProvider {
    async fn search(&self, query: &str, limit: usize) -> Result<Vec<Track>, CoreError> {
        let db_path = self.db_path.clone();
        let q = query.to_string();
        tokio::task::spawn_blocking(move || {
            let db = Database::open(&db_path).map_err(map_storage_err)?;
            db.search_local_tracks(&q, limit).map_err(map_storage_err)
        })
        .await
        .map_err(|e| CoreError::ProviderUnavailable(e.to_string()))?
    }

    async fn get_playlists(&self) -> Result<Vec<Playlist>, CoreError> {
        Ok(Vec::new())
    }

    async fn get_playlist_tracks(&self, _playlist_id: &str) -> Result<Vec<Track>, CoreError> {
        Ok(Vec::new())
    }

    async fn resolve_stream(&self, track: &Track) -> Result<ResolvedStream, CoreError> {
        match &track.source {
            TrackSource::Local { path } => Ok(ResolvedStream::LocalFile(path.clone())),
            _ => Err(CoreError::InvalidInput(
                "LocalProvider can only resolve TrackSource::Local".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_minimal_wav(path: &Path, duration_secs: u32) {
        let sample_rate: u32 = 8000;
        let channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let num_samples = sample_rate * duration_secs;
        let data_size = num_samples * channels as u32 * (bits_per_sample as u32 / 8);
        let riff_size = 36 + data_size;

        let mut buf = Vec::with_capacity((44 + data_size) as usize);
        buf.extend_from_slice(b"RIFF");
        buf.extend_from_slice(&riff_size.to_le_bytes());
        buf.extend_from_slice(b"WAVE");
        buf.extend_from_slice(b"fmt ");
        buf.extend_from_slice(&16u32.to_le_bytes());
        buf.extend_from_slice(&1u16.to_le_bytes()); // PCM
        buf.extend_from_slice(&channels.to_le_bytes());
        buf.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * channels as u32 * (bits_per_sample as u32 / 8);
        buf.extend_from_slice(&byte_rate.to_le_bytes());
        let block_align = channels * (bits_per_sample / 8);
        buf.extend_from_slice(&block_align.to_le_bytes());
        buf.extend_from_slice(&bits_per_sample.to_le_bytes());
        buf.extend_from_slice(b"data");
        buf.extend_from_slice(&data_size.to_le_bytes());
        buf.resize(44 + data_size as usize, 0u8);

        std::fs::write(path, buf).unwrap();
    }

    #[test]
    fn scans_untagged_wav_falling_back_to_filename_and_unknown_artist() {
        let music_dir = TempDir::new().unwrap();
        let art_dir = TempDir::new().unwrap();
        let wav_path = music_dir.path().join("Midnight Drive.wav");
        write_minimal_wav(&wav_path, 1);

        let db = Database::open_in_memory().unwrap();
        let mut art = ArtCache::new(art_dir.path().to_path_buf(), 10_000_000, 10);
        let provider = LocalProvider::new(PathBuf::from(":memory:"));

        let report = provider
            .scan_folders(&[music_dir.path().to_path_buf()], &db, &mut art)
            .unwrap();
        assert_eq!(report.scanned, 1);
        assert_eq!(report.added_or_updated, 1);
        assert_eq!(report.skipped_corrupt, 0);

        let tracks = db.list_local_tracks().unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Midnight Drive");
        assert_eq!(tracks[0].artists, vec!["Unknown Artist".to_string()]);
        assert!(tracks[0].duration_ms >= 900);
    }

    #[test]
    fn skips_corrupt_audio_file_without_aborting_scan() {
        let music_dir = TempDir::new().unwrap();
        let art_dir = TempDir::new().unwrap();
        write_minimal_wav(&music_dir.path().join("Good.wav"), 1);
        std::fs::write(music_dir.path().join("Corrupt.mp3"), b"not real mp3 data").unwrap();

        let db = Database::open_in_memory().unwrap();
        let mut art = ArtCache::new(art_dir.path().to_path_buf(), 10_000_000, 10);
        let provider = LocalProvider::new(PathBuf::from(":memory:"));

        let report = provider
            .scan_folders(&[music_dir.path().to_path_buf()], &db, &mut art)
            .unwrap();
        assert_eq!(report.scanned, 2);
        assert_eq!(report.added_or_updated, 1);
        assert_eq!(report.skipped_corrupt, 1);
        assert_eq!(db.list_local_tracks().unwrap().len(), 1);
    }

    #[test]
    fn prunes_deleted_local_files_on_incremental_rescan() {
        let music_dir = TempDir::new().unwrap();
        let art_dir = TempDir::new().unwrap();
        let a = music_dir.path().join("Keep.wav");
        let b = music_dir.path().join("DeleteMe.wav");
        write_minimal_wav(&a, 1);
        write_minimal_wav(&b, 1);

        let db = Database::open_in_memory().unwrap();
        let mut art = ArtCache::new(art_dir.path().to_path_buf(), 10_000_000, 10);
        let provider = LocalProvider::new(PathBuf::from(":memory:"));

        provider
            .scan_folders(&[music_dir.path().to_path_buf()], &db, &mut art)
            .unwrap();
        assert_eq!(db.list_local_tracks().unwrap().len(), 2);

        std::fs::remove_file(&b).unwrap();
        let report2 = provider
            .scan_folders(&[music_dir.path().to_path_buf()], &db, &mut art)
            .unwrap();
        assert_eq!(report2.pruned, 1);
        let tracks = db.list_local_tracks().unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Keep");
    }

    #[test]
    fn scanned_local_tracks_default_to_is_liked_false() {
        let music_dir = TempDir::new().unwrap();
        let art_dir = TempDir::new().unwrap();
        write_minimal_wav(&music_dir.path().join("Song.wav"), 1);

        let db = Database::open_in_memory().unwrap();
        let mut art = ArtCache::new(art_dir.path().to_path_buf(), 10_000_000, 10);
        let provider = LocalProvider::new(PathBuf::from(":memory:"));

        provider
            .scan_folders(&[music_dir.path().to_path_buf()], &db, &mut art)
            .unwrap();
        let tracks = db.list_local_tracks().unwrap();
        assert!(!tracks[0].is_liked);
        assert!(db.list_unified_liked_tracks().unwrap().is_empty());
    }

    #[test]
    fn finds_matching_sidecar_lrc_file() {
        let dir = TempDir::new().unwrap();
        let audio = dir.path().join("Track.flac");
        let lrc = dir.path().join("Track.lrc");
        std::fs::write(&audio, b"").unwrap();
        assert!(find_sidecar_lrc(&audio).is_none());

        std::fs::write(&lrc, "[00:01.00]Line").unwrap();
        assert_eq!(find_sidecar_lrc(&audio), Some(lrc));
    }
}
