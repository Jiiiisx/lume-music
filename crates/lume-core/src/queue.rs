use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

use crate::models::{RepeatMode, Track};

/// Outcome of a queue navigation or mutation step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueActionResult {
    Play(Track),
    RestartCurrent,
    Stopped,
}

/// Serializable snapshot of the queue for session persistence across restarts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QueueSnapshot {
    pub natural_order: Vec<Track>,
    pub order: Vec<usize>,
    pub current_pos: Option<usize>,
    pub history: Vec<usize>,
    pub shuffle: bool,
    pub repeat: RepeatMode,
}

/// Unified multi-source playback queue with shuffle, repeat, and play history.
#[derive(Debug, Clone, Default)]
pub struct QueueEngine {
    natural_order: Vec<Track>,
    order: Vec<usize>,
    current_pos: Option<usize>,
    history: Vec<usize>,
    shuffle: bool,
    repeat: RepeatMode,
}

impl QueueEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tracks(&mut self, tracks: Vec<Track>, start_index: usize) -> Option<&Track> {
        self.natural_order = tracks;
        self.history.clear();

        if self.natural_order.is_empty() {
            self.order.clear();
            self.current_pos = None;
            return None;
        }

        let clamped_start = start_index.min(self.natural_order.len() - 1);
        if self.shuffle {
            let mut remaining: Vec<usize> = (0..self.natural_order.len())
                .filter(|&i| i != clamped_start)
                .collect();
            let mut rng = rand::rng();
            remaining.shuffle(&mut rng);
            self.order = std::iter::once(clamped_start).chain(remaining).collect();
            self.current_pos = Some(0);
        } else {
            self.order = (0..self.natural_order.len()).collect();
            self.current_pos = Some(clamped_start);
        }

        self.current()
    }

    pub fn add_to_queue(&mut self, track: Track) {
        let idx = self.natural_order.len();
        self.natural_order.push(track);
        self.order.push(idx);
        if self.current_pos.is_none() {
            self.current_pos = Some(0);
        }
    }

    pub fn play_next(&mut self, track: Track) {
        let idx = self.natural_order.len();
        self.natural_order.push(track);
        match self.current_pos {
            Some(pos) if pos < self.order.len() => {
                self.order.insert(pos + 1, idx);
            }
            _ => {
                self.order.insert(0, idx);
                self.current_pos = Some(0);
            }
        }
    }

    pub fn jump_to(&mut self, index: usize) -> QueueActionResult {
        if index >= self.order.len() {
            return self
                .current()
                .cloned()
                .map(QueueActionResult::Play)
                .unwrap_or(QueueActionResult::Stopped);
        }
        if let Some(cur) = self.current_pos {
            if cur != index {
                if let Some(&nat_idx) = self.order.get(cur) {
                    self.history.push(nat_idx);
                }
            }
        }
        self.current_pos = Some(index);
        self.current()
            .cloned()
            .map(QueueActionResult::Play)
            .unwrap_or(QueueActionResult::Stopped)
    }

    pub fn remove_at(&mut self, index: usize) -> QueueActionResult {
        if index >= self.order.len() {
            return self
                .current()
                .cloned()
                .map(QueueActionResult::Play)
                .unwrap_or(QueueActionResult::Stopped);
        }

        let removed_nat = self.order.remove(index);
        self.natural_order.remove(removed_nat);

        // Shift indices greater than `removed_nat` down by 1.
        for slot in &mut self.order {
            if *slot > removed_nat {
                *slot -= 1;
            }
        }
        self.history.retain(|&idx| idx != removed_nat);
        for hist in &mut self.history {
            if *hist > removed_nat {
                *hist -= 1;
            }
        }

        if self.order.is_empty() {
            self.current_pos = None;
            return QueueActionResult::Stopped;
        }

        match self.current_pos {
            Some(cur) if index < cur => {
                self.current_pos = Some(cur - 1);
            }
            Some(cur) if index == cur => {
                if cur < self.order.len() {
                    self.current_pos = Some(cur);
                } else if self.repeat == RepeatMode::All {
                    self.current_pos = Some(0);
                } else {
                    self.current_pos = None;
                    return QueueActionResult::Stopped;
                }
            }
            _ => {}
        }

        self.current()
            .cloned()
            .map(QueueActionResult::Play)
            .unwrap_or(QueueActionResult::Stopped)
    }

    pub fn reorder(&mut self, from: usize, to: usize) {
        if from == to || from >= self.order.len() || to >= self.order.len() {
            return;
        }
        let item = self.order.remove(from);
        self.order.insert(to, item);

        if let Some(pos) = self.current_pos {
            if pos == from {
                self.current_pos = Some(to);
            } else if from < pos && pos <= to {
                self.current_pos = Some(pos - 1);
            } else if to <= pos && pos < from {
                self.current_pos = Some(pos + 1);
            }
        }
    }

    pub fn set_shuffle(&mut self, enabled: bool) {
        if self.shuffle == enabled {
            return;
        }
        self.shuffle = enabled;
        if self.order.is_empty() {
            return;
        }

        let active_nat = self
            .current_pos
            .and_then(|pos| self.order.get(pos).copied());

        if enabled {
            let mut remaining: Vec<usize> = (0..self.natural_order.len())
                .filter(|&idx| Some(idx) != active_nat)
                .collect();
            let mut rng = rand::rng();
            remaining.shuffle(&mut rng);
            if let Some(active) = active_nat {
                self.order = std::iter::once(active).chain(remaining).collect();
                self.current_pos = Some(0);
            } else {
                self.order = remaining;
            }
        } else {
            self.order = (0..self.natural_order.len()).collect();
            self.current_pos = active_nat;
        }
    }

    pub fn is_shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }

    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat
    }

    pub fn next(&mut self) -> QueueActionResult {
        if self.order.is_empty() {
            self.current_pos = None;
            return QueueActionResult::Stopped;
        }

        let Some(pos) = self.current_pos else {
            return QueueActionResult::Stopped;
        };

        if self.repeat == RepeatMode::One {
            return self
                .current()
                .cloned()
                .map(QueueActionResult::Play)
                .unwrap_or(QueueActionResult::Stopped);
        }

        if let Some(&nat_idx) = self.order.get(pos) {
            self.history.push(nat_idx);
        }

        if pos + 1 < self.order.len() {
            self.current_pos = Some(pos + 1);
            self.current()
                .cloned()
                .map(QueueActionResult::Play)
                .unwrap_or(QueueActionResult::Stopped)
        } else if self.repeat == RepeatMode::All {
            self.current_pos = Some(0);
            self.current()
                .cloned()
                .map(QueueActionResult::Play)
                .unwrap_or(QueueActionResult::Stopped)
        } else {
            self.current_pos = None;
            QueueActionResult::Stopped
        }
    }

    pub fn previous(&mut self, elapsed_ms: u64) -> QueueActionResult {
        if self.order.is_empty() {
            self.current_pos = None;
            return QueueActionResult::Stopped;
        }

        if let Some(pos) = self.current_pos {
            if elapsed_ms > 3000 {
                return QueueActionResult::RestartCurrent;
            }

            while let Some(prev_nat) = self.history.pop() {
                if let Some(prev_pos) = self.order.iter().position(|&idx| idx == prev_nat) {
                    self.current_pos = Some(prev_pos);
                    if let Some(track) = self.current() {
                        return QueueActionResult::Play(track.clone());
                    }
                }
            }

            if pos > 0 {
                self.current_pos = Some(pos - 1);
                return self
                    .current()
                    .cloned()
                    .map(QueueActionResult::Play)
                    .unwrap_or(QueueActionResult::Stopped);
            } else if self.repeat == RepeatMode::All {
                self.current_pos = Some(self.order.len() - 1);
                return self
                    .current()
                    .cloned()
                    .map(QueueActionResult::Play)
                    .unwrap_or(QueueActionResult::Stopped);
            }

            QueueActionResult::RestartCurrent
        } else {
            while let Some(prev_nat) = self.history.pop() {
                if let Some(prev_pos) = self.order.iter().position(|&idx| idx == prev_nat) {
                    self.current_pos = Some(prev_pos);
                    if let Some(track) = self.current() {
                        return QueueActionResult::Play(track.clone());
                    }
                }
            }
            QueueActionResult::Stopped
        }
    }

    pub fn current(&self) -> Option<&Track> {
        self.current_pos
            .and_then(|p| self.order.get(p))
            .and_then(|&idx| self.natural_order.get(idx))
    }

    /// Peeks at the next track that will play after the current track without advancing the queue.
    pub fn peek_next(&self) -> Option<&Track> {
        let pos = self.current_pos?;
        if self.order.is_empty() {
            return None;
        }
        let next_pos = if self.repeat == RepeatMode::One {
            pos
        } else if pos + 1 < self.order.len() {
            pos + 1
        } else if self.repeat == RepeatMode::All {
            0
        } else {
            return None;
        };
        self.order
            .get(next_pos)
            .and_then(|&idx| self.natural_order.get(idx))
    }

    pub fn current_pos(&self) -> Option<usize> {
        self.current_pos
    }

    pub fn active_list(&self) -> Vec<&Track> {
        self.order
            .iter()
            .filter_map(|&idx| self.natural_order.get(idx))
            .collect()
    }

    pub fn clear(&mut self) {
        self.natural_order.clear();
        self.order.clear();
        self.current_pos = None;
        self.history.clear();
    }

    pub fn to_snapshot(&self) -> QueueSnapshot {
        QueueSnapshot {
            natural_order: self.natural_order.clone(),
            order: self.order.clone(),
            current_pos: self.current_pos,
            history: self.history.clone(),
            shuffle: self.shuffle,
            repeat: self.repeat,
        }
    }

    pub fn from_snapshot(snapshot: QueueSnapshot) -> Self {
        let len = snapshot.natural_order.len();
        let mut order: Vec<usize> = snapshot
            .order
            .into_iter()
            .filter(|&idx| idx < len)
            .collect();
        if order.len() != len {
            order = (0..len).collect();
        }
        let current_pos = snapshot.current_pos.filter(|&p| p < order.len());
        let history = snapshot
            .history
            .into_iter()
            .filter(|&idx| idx < len)
            .collect();

        Self {
            natural_order: snapshot.natural_order,
            order,
            current_pos,
            history,
            shuffle: snapshot.shuffle,
            repeat: snapshot.repeat,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::TrackSource;
    use std::path::PathBuf;

    fn make_track(source: TrackSource, title: &str) -> Track {
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.to_string(),
            artists: vec!["Artist".to_string()],
            album: Some("Album".to_string()),
            duration_ms: 180_000,
            artwork_ref: None,
            is_liked: false,
        }
    }

    fn sample_mixed_tracks() -> Vec<Track> {
        vec![
            make_track(TrackSource::Spotify { id: "sp1".into() }, "Spotify One"),
            make_track(
                TrackSource::YouTubeMusic {
                    video_id: "yt1".into(),
                },
                "YouTube One",
            ),
            make_track(
                TrackSource::Local {
                    path: PathBuf::from(r"D:\Music\local1.flac"),
                },
                "Local One",
            ),
            make_track(TrackSource::Spotify { id: "sp2".into() }, "Spotify Two"),
        ]
    }

    #[test]
    fn mixes_tracks_from_all_sources_and_preserves_order() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        let active = engine.set_tracks(tracks.clone(), 1).unwrap();
        assert_eq!(active.uid, tracks[1].uid);

        let uids: Vec<&str> = engine.active_list().iter().map(|t| t.uid.as_str()).collect();
        assert_eq!(
            uids,
            vec![
                tracks[0].uid.as_str(),
                tracks[1].uid.as_str(),
                tracks[2].uid.as_str(),
                tracks[3].uid.as_str(),
            ]
        );
    }

    #[test]
    fn toggling_shuffle_off_keeps_active_track_and_restores_natural_order() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        engine.set_tracks(tracks.clone(), 2);
        assert_eq!(engine.current().unwrap().uid, tracks[2].uid);

        engine.set_shuffle(true);
        assert_eq!(engine.current().unwrap().uid, tracks[2].uid);
        assert_eq!(engine.active_list().len(), 4);

        engine.set_shuffle(false);
        assert_eq!(engine.current().unwrap().uid, tracks[2].uid);
        assert_eq!(engine.current_pos(), Some(2));
        let uids: Vec<&str> = engine.active_list().iter().map(|t| t.uid.as_str()).collect();
        assert_eq!(
            uids,
            vec![
                tracks[0].uid.as_str(),
                tracks[1].uid.as_str(),
                tracks[2].uid.as_str(),
                tracks[3].uid.as_str(),
            ]
        );
    }

    #[test]
    fn previous_restarts_over_3000ms_and_steps_back_history_under_3000ms() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        engine.set_tracks(tracks.clone(), 0);
        assert_eq!(engine.next(), QueueActionResult::Play(tracks[1].clone()));
        assert_eq!(engine.next(), QueueActionResult::Play(tracks[2].clone()));

        assert_eq!(engine.previous(3001), QueueActionResult::RestartCurrent);
        assert_eq!(engine.current().unwrap().uid, tracks[2].uid);

        assert_eq!(
            engine.previous(3000),
            QueueActionResult::Play(tracks[1].clone())
        );
        assert_eq!(
            engine.previous(1500),
            QueueActionResult::Play(tracks[0].clone())
        );
    }

    #[test]
    fn repeat_one_loops_current_and_repeat_all_wraps_around() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        engine.set_tracks(tracks.clone(), 3);

        engine.set_repeat(RepeatMode::One);
        assert_eq!(engine.next(), QueueActionResult::Play(tracks[3].clone()));
        assert_eq!(engine.next(), QueueActionResult::Play(tracks[3].clone()));

        engine.set_repeat(RepeatMode::All);
        assert_eq!(engine.next(), QueueActionResult::Play(tracks[0].clone()));

        engine.set_tracks(tracks.clone(), 3);
        engine.set_repeat(RepeatMode::Off);
        assert_eq!(engine.next(), QueueActionResult::Stopped);
    }

    #[test]
    fn removing_active_track_advances_or_stops_when_empty() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        engine.set_tracks(tracks.clone(), 1);

        assert_eq!(
            engine.remove_at(1),
            QueueActionResult::Play(tracks[2].clone())
        );
        assert_eq!(engine.active_list().len(), 3);

        let mut single = QueueEngine::new();
        single.set_tracks(vec![tracks[0].clone()], 0);
        assert_eq!(single.remove_at(0), QueueActionResult::Stopped);
        assert!(single.active_list().is_empty());
        assert!(single.current().is_none());
    }

    #[test]
    fn play_next_inserts_immediately_after_current_track() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        engine.set_tracks(tracks[0..3].to_vec(), 0);
        engine.play_next(tracks[3].clone());

        assert_eq!(engine.next(), QueueActionResult::Play(tracks[3].clone()));
        assert_eq!(engine.next(), QueueActionResult::Play(tracks[1].clone()));
    }

    #[test]
    fn play_next_and_add_to_queue_initialize_empty_queue() {
        let tracks = sample_mixed_tracks();

        let mut q1 = QueueEngine::new();
        q1.add_to_queue(tracks[0].clone());
        assert_eq!(q1.current().map(|t| t.uid.as_str()), Some(tracks[0].uid.as_str()));

        let mut q2 = QueueEngine::new();
        q2.play_next(tracks[1].clone());
        assert_eq!(q2.current().map(|t| t.uid.as_str()), Some(tracks[1].uid.as_str()));
    }

    #[test]
    fn snapshot_round_trip_restores_exact_position_and_modes() {
        let mut engine = QueueEngine::new();
        let tracks = sample_mixed_tracks();
        engine.set_tracks(tracks.clone(), 1);
        engine.set_repeat(RepeatMode::All);
        engine.set_shuffle(true);
        let next_res = engine.next();
        let expected_uid = match next_res {
            QueueActionResult::Play(t) => t.uid,
            _ => panic!("expected Play"),
        };

        let snapshot = engine.to_snapshot();
        let json = serde_json::to_string(&snapshot).unwrap();
        let decoded: QueueSnapshot = serde_json::from_str(&json).unwrap();
        let mut restored = QueueEngine::from_snapshot(decoded);

        assert_eq!(restored.current().unwrap().uid, expected_uid);
        assert!(restored.is_shuffle());
        assert_eq!(restored.repeat_mode(), RepeatMode::All);
        assert_eq!(
            restored.previous(1000),
            QueueActionResult::Play(tracks[1].clone())
        );
    }
}
