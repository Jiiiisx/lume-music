use std::collections::HashSet;
use std::sync::Mutex;

use lume_core::{CoreError, MusicProvider, Track, TrackSource};
use lume_storage::Database;

use crate::ytm::YouTubeMusicProvider;

const PENALTY_KEYWORDS: &[&str] = &[
    "live",
    "cover",
    "karaoke",
    "slowed",
    "reverb",
    "8d",
    "nightcore",
    "instrumental",
    "sped up",
    "speed up",
    "tribute",
    "piano version",
    "8-bit",
    "lofi",
    "lo-fi",
];

const NOISE_PHRASES: &[&str] = &[
    "remastered",
    "remaster",
    "deluxe edition",
    "deluxe",
    "expanded edition",
    "bonus track",
    "radio edit",
    "album version",
    "single version",
    "explicit",
    "clean version",
    "original motion picture soundtrack",
    "mono",
    "stereo",
];

/// Winning YouTube Music match for a Spotify track.
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackMatch {
    pub spotify_track_id: String,
    pub yt_video_id: String,
    pub confidence: f32,
    pub from_cache: bool,
}

/// Strips non-essential remaster/edition/featuring noise from a track title for cleaner YTM queries,
/// while preserving meaningful version tags (e.g. `Live`, `Acoustic`, `Remix`).
pub fn clean_track_title(title: &str) -> String {
    let mut cleaned = String::with_capacity(title.len());
    let chars: Vec<char> = title.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '(' || ch == '[' {
            let close = if ch == '(' { ')' } else { ']' };
            if let Some(rel_end) = chars[i + 1..].iter().position(|&c| c == close) {
                let inner: String = chars[i + 1..i + 1 + rel_end].iter().collect();
                let inner_lower = inner.to_ascii_lowercase();
                let is_noise = inner_lower.starts_with("feat.")
                    || inner_lower.starts_with("feat ")
                    || inner_lower.starts_with("ft.")
                    || inner_lower.starts_with("ft ")
                    || inner_lower.starts_with("with ")
                    || NOISE_PHRASES.iter().any(|p| inner_lower.contains(p));
                let keeps_meaningful = PENALTY_KEYWORDS
                    .iter()
                    .any(|kw| inner_lower.contains(kw))
                    || inner_lower.contains("acoustic")
                    || inner_lower.contains("remix")
                    || inner_lower.contains("unplugged");
                if is_noise && !keeps_meaningful {
                    i = i + 1 + rel_end + 1;
                    continue;
                }
            }
        }
        cleaned.push(ch);
        i += 1;
    }

    // Also strip trailing ` - Remastered 2011` / ` - Radio Edit` suffixes.
    if let Some((head, tail)) = cleaned.rsplit_once(" - ") {
        let tail_lower = tail.to_ascii_lowercase();
        if NOISE_PHRASES.iter().any(|p| tail_lower.contains(p))
            && !PENALTY_KEYWORDS.iter().any(|kw| tail_lower.contains(kw))
        {
            cleaned = head.to_string();
        }
    }

    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Builds the YouTube Music fallback search query (`"{primary_artist} {clean_title}"`).
pub fn build_fallback_search_query(track: &Track) -> String {
    let artist = track
        .artists
        .first()
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    let title = clean_track_title(&track.title);
    if artist.is_empty() || artist.eq_ignore_ascii_case("unknown artist") {
        title
    } else {
        format!("{artist} {title}")
    }
}

fn tokenize(s: &str) -> HashSet<String> {
    s.to_ascii_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|tok| !tok.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn jaccard_similarity(a: &str, b: &str) -> f32 {
    let set_a = tokenize(a);
    let set_b = tokenize(b);
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let inter = set_a.intersection(&set_b).count() as f32;
    let union = set_a.union(&set_b).count() as f32;
    inter / union
}

fn contains_keyword(haystack_lower: &str, kw: &str) -> bool {
    if kw.contains(' ') || kw.contains('-') {
        return haystack_lower.contains(kw);
    }
    haystack_lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|tok| tok == kw)
}

/// Scores a YouTube Music candidate against a Spotify track (higher is better, roughly `0.0..=1.0`).
pub fn score_ytm_candidate(spotify_track: &Track, candidate: &Track) -> f32 {
    let sp_title_clean = clean_track_title(&spotify_track.title);
    let cand_title_clean = clean_track_title(&candidate.title);

    let mut title_sim = jaccard_similarity(&sp_title_clean, &cand_title_clean);
    let sp_lower = sp_title_clean.to_ascii_lowercase();
    let cand_lower = cand_title_clean.to_ascii_lowercase();
    if !sp_lower.is_empty() && cand_lower == sp_lower {
        title_sim = 1.0;
    } else if !sp_lower.is_empty() && (cand_lower.contains(&sp_lower) || sp_lower.contains(&cand_lower)) {
        title_sim = title_sim.max(0.82);
    }

    let sp_artists = spotify_track.artists.join(" ");
    let cand_artists = candidate.artists.join(" ");
    let mut artist_sim = jaccard_similarity(&sp_artists, &cand_artists);
    if let Some(primary) = spotify_track.artists.first() {
        let p_low = primary.to_ascii_lowercase();
        if !p_low.is_empty() && cand_artists.to_ascii_lowercase().contains(&p_low) {
            artist_sim = artist_sim.max(0.85);
        }
    }

    // Duration proximity score (strong preference within ±5s, acceptable up to ±10s).
    let duration_score = if spotify_track.duration_ms > 0 && candidate.duration_ms > 0 {
        let diff_s = spotify_track.duration_ms.abs_diff(candidate.duration_ms) as f32 / 1000.0;
        if diff_s <= 2.0 {
            1.0
        } else if diff_s <= 5.0 {
            0.92
        } else if diff_s <= 10.0 {
            0.75
        } else if diff_s <= 20.0 {
            0.35
        } else if diff_s <= 35.0 {
            0.05
        } else {
            -0.35
        }
    } else {
        0.5
    };

    // Penalty for unintended live/cover/karaoke/slowed/reverb/8d/nightcore/instrumental versions.
    let sp_full_lower = spotify_track.title.to_ascii_lowercase();
    let cand_full_lower = candidate.title.to_ascii_lowercase();
    let mut version_penalty = 0.0f32;
    for kw in PENALTY_KEYWORDS {
        let in_candidate = contains_keyword(&cand_full_lower, kw);
        let in_source = contains_keyword(&sp_full_lower, kw);
        if in_candidate && !in_source {
            version_penalty += 0.40;
        }
    }

    let raw = (title_sim * 0.48) + (artist_sim * 0.28) + (duration_score * 0.24) - version_penalty;
    raw.clamp(0.0, 1.0)
}

/// Selects the highest-scoring YouTube Music candidate, optionally excluding a previously rejected video ID.
pub fn select_best_ytm_match(
    spotify_track: &Track,
    candidates: &[Track],
    exclude_video_id: Option<&str>,
) -> Option<FallbackMatch> {
    let spotify_id = match &spotify_track.source {
        TrackSource::Spotify { id } => id.clone(),
        _ => spotify_track.uid.clone(),
    };

    let scored: Vec<(&str, f32)> = candidates
        .iter()
        .filter_map(|cand| {
            let TrackSource::YouTubeMusic { video_id } = &cand.source else {
                return None;
            };
            let score = score_ytm_candidate(spotify_track, cand);
            Some((video_id.as_str(), score))
        })
        .collect();

    // First try excluding any previously rejected video ID (used during "Wrong version? Re-match").
    let best_filtered = scored
        .iter()
        .copied()
        .filter(|(vid, _)| exclude_video_id != Some(*vid))
        .max_by(|a, b| a.1.total_cmp(&b.1));

    let chosen = best_filtered.or_else(|| {
        scored
            .iter()
            .copied()
            .max_by(|a, b| a.1.total_cmp(&b.1))
    })?;

    Some(FallbackMatch {
        spotify_track_id: spotify_id,
        yt_video_id: chosen.0.to_string(),
        confidence: chosen.1,
        from_cache: false,
    })
}

/// Resolves a Spotify track to its YouTube Music fallback video ID using SQLite `yt_mappings` cache
/// first (unless `force_rematch` is true), falling back to a live YTM search + scoring pass.
pub async fn resolve_spotify_to_ytm_video_id(
    track: &Track,
    db: &Mutex<Database>,
    ytm: &YouTubeMusicProvider,
    force_rematch: bool,
) -> Result<FallbackMatch, CoreError> {
    let TrackSource::Spotify { id: spotify_id } = &track.source else {
        return Err(CoreError::InvalidInput(
            "Smart YTM fallback matcher only accepts Spotify tracks.".into(),
        ));
    };

    let previous_video_id = db
        .lock()
        .ok()
        .and_then(|d| d.get_yt_mapping(spotify_id).ok().flatten())
        .map(|m| (m.yt_video_id, m.confidence));

    if !force_rematch {
        if let Some((yt_video_id, confidence)) = previous_video_id {
            return Ok(FallbackMatch {
                spotify_track_id: spotify_id.clone(),
                yt_video_id,
                confidence,
                from_cache: true,
            });
        }
    }

    let exclude_id = if force_rematch {
        previous_video_id.as_ref().map(|(id, _)| id.as_str())
    } else {
        None
    };

    let query = build_fallback_search_query(track);
    let candidates = ytm.search(&query, 10).await?;
    let best = select_best_ytm_match(track, &candidates, exclude_id).ok_or_else(|| {
        CoreError::NotFound(format!(
            "No YouTube Music fallback match found for \"{}\" by {}",
            track.title,
            track.artist_display()
        ))
    })?;

    if let Ok(d) = db.lock() {
        let _ = d.put_yt_mapping(spotify_id, &best.yt_video_id, best.confidence);
    }

    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk_spotify(id: &str, title: &str, artist: &str, duration_ms: u64) -> Track {
        let source = TrackSource::Spotify { id: id.into() };
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.into(),
            artists: vec![artist.into()],
            album: Some("Album".into()),
            duration_ms,
            artwork_ref: None,
            is_liked: false,
        }
    }

    fn mk_ytm(video_id: &str, title: &str, artist: &str, duration_ms: u64) -> Track {
        let source = TrackSource::YouTubeMusic {
            video_id: video_id.into(),
        };
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.into(),
            artists: vec![artist.into()],
            album: None,
            duration_ms,
            artwork_ref: None,
            is_liked: false,
        }
    }

    #[test]
    fn cleans_remaster_and_feat_noise_while_keeping_live_tags() {
        assert_eq!(
            clean_track_title("Bohemian Rhapsody (Remastered 2011)"),
            "Bohemian Rhapsody"
        );
        assert_eq!(
            clean_track_title("Hotel California - 2013 Remaster"),
            "Hotel California"
        );
        assert_eq!(
            clean_track_title("Starboy (feat. Daft Punk)"),
            "Starboy"
        );
        assert_eq!(
            clean_track_title("Hotel California (Live On MTV, 1994)"),
            "Hotel California (Live On MTV, 1994)"
        );
    }

    #[test]
    fn prefers_clean_studio_match_over_live_cover_slowed_or_wrong_duration() {
        let sp = mk_spotify(
            "sp_blinding",
            "Blinding Lights (Remastered 2020)",
            "The Weeknd",
            200_000,
        );
        let candidates = vec![
            mk_ytm("vid_slowed", "Blinding Lights (Slowed + Reverb)", "The Weeknd", 235_000),
            mk_ytm("vid_live", "Blinding Lights (Live at Wembley)", "The Weeknd", 215_000),
            mk_ytm("vid_cover", "Blinding Lights (Acoustic Cover)", "Random Singer", 200_000),
            mk_ytm("vid_karaoke", "Blinding Lights (Karaoke Instrumental)", "Karaoke King", 201_000),
            mk_ytm("vid_studio", "Blinding Lights", "The Weeknd", 201_500),
        ];

        let winner = select_best_ytm_match(&sp, &candidates, None).unwrap();
        assert_eq!(winner.yt_video_id, "vid_studio");
        assert!(winner.confidence > 0.85, "expected high confidence, got {}", winner.confidence);
    }

    #[test]
    fn rematch_excludes_previously_cached_video_id_when_alternate_exists() {
        let sp = mk_spotify("sp_1", "Nightcall", "Kavinsky", 258_000);
        let candidates = vec![
            mk_ytm("vid_old", "Nightcall", "Kavinsky", 258_000),
            mk_ytm("vid_alt", "Nightcall (Original)", "Kavinsky", 259_000),
        ];

        let rematch = select_best_ytm_match(&sp, &candidates, Some("vid_old")).unwrap();
        assert_eq!(rematch.yt_video_id, "vid_alt");
    }

    #[test]
    fn honors_intentional_live_version_when_spotify_title_says_live() {
        let sp = mk_spotify("sp_live", "Everlong (Live)", "Foo Fighters", 260_000);
        let studio = mk_ytm("vid_studio", "Everlong", "Foo Fighters", 250_000);
        let live = mk_ytm("vid_live", "Everlong (Live)", "Foo Fighters", 261_000);

        let winner = select_best_ytm_match(&sp, &[studio, live], None).unwrap();
        assert_eq!(winner.yt_video_id, "vid_live");
    }
}
