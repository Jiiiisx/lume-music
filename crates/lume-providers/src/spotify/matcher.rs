use std::collections::HashSet;
use std::sync::Mutex;

use lume_core::{CoreError, MusicProvider, Track, TrackSource};
use lume_storage::Database;

use crate::ytm::YouTubeMusicProvider;

/// Minimum confidence required to accept any YouTube fallback candidate.
/// Prevents completely unrelated songs (e.g. wrong artist + partial title overlap) from playing.
pub const MIN_MATCH_CONFIDENCE: f32 = 0.35;

/// Confidence threshold at which a YTM Songs search result is accepted immediately without
/// issuing a secondary YTM Videos search.
const HIGH_CONFIDENCE_THRESHOLD: f32 = 0.78;

/// Cached SQLite `yt_mappings` entries below this confidence are automatically revalidated
/// so stale low-confidence guesses get upgraded when better matching logic is available.
pub const CACHE_REVALIDATE_CONFIDENCE: f32 = 0.65;

/// Slight tie-breaker subtracted from YTM Video candidates when comparing against equally-scored
/// official YTM Song (Art Track) candidates.
const VIDEO_TIEBREAKER_PENALTY: f32 = 0.02;

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
    "piano cover",
    "guitar cover",
    "fingerstyle",
    "8-bit",
    "lofi",
    "lo-fi",
    "remix",
    "acoustic",
    "unplugged",
    "mashup",
    "parody",
    "reaction",
    "tutorial",
    "ringtone",
    "1 hour",
    "bass boosted",
    "chipmunk",
    "off vocal",
    "minus one",
    "music box",
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

const VIDEO_NOISE_PHRASES: &[&str] = &[
    "official music video",
    "official lyric video",
    "official lyrics video",
    "official video",
    "official audio",
    "official visualizer",
    "official mv",
    "music video lirik",
    "video lirik",
    "music video",
    "lyric video",
    "lyrics video",
    "lirik lagu",
    "lirik",
    "lyrics",
    "full audio",
    "original audio",
    "original version",
    "original mix",
    "original",
    "visualizer",
    "audio",
    "m/v",
    "mv",
    "hd",
    "hq",
    "4k",
];

/// Winning YouTube Music match for a Spotify track.
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackMatch {
    pub spotify_track_id: String,
    pub yt_video_id: String,
    pub confidence: f32,
    pub from_cache: bool,
}

/// Returns `true` when a cached `yt_mappings` confidence score is strong enough to trust
/// without re-running YouTube Music search.
pub fn should_use_cached_mapping(confidence: f32) -> bool {
    confidence >= CACHE_REVALIDATE_CONFIDENCE
}

/// Strips non-essential remaster/edition/featuring noise from a track title for cleaner YTM queries,
/// while preserving meaningful version tags (e.g. `Live`, `Acoustic`, `Remix`).
pub fn clean_track_title(title: &str) -> String {
    strip_noise_tags(title, NOISE_PHRASES)
}

fn strip_noise_tags(title: &str, noise_phrases: &[&str]) -> String {
    let mut cleaned = String::with_capacity(title.len());
    let chars: Vec<char> = title.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '(' || ch == '[' {
            let close = if ch == '(' { ')' } else { ']' };
            if let Some(rel_end) = chars[i + 1..].iter().position(|&c| c == close) {
                let inner: String = chars[i + 1..i + 1 + rel_end].iter().collect();
                let inner_lower = inner.to_lowercase();
                let is_noise = inner_lower.starts_with("feat.")
                    || inner_lower.starts_with("feat ")
                    || inner_lower.starts_with("ft.")
                    || inner_lower.starts_with("ft ")
                    || inner_lower.starts_with("with ")
                    || noise_phrases.iter().any(|p| contains_keyword(&inner_lower, p));
                let keeps_meaningful = PENALTY_KEYWORDS
                    .iter()
                    .any(|kw| contains_keyword(&inner_lower, kw));
                if is_noise && !keeps_meaningful {
                    i = i + 1 + rel_end + 1;
                    continue;
                }
            }
        }
        cleaned.push(ch);
        i += 1;
    }

    // Also strip trailing ` - Remastered 2011` / ` - Radio Edit` / ` - Official Video` suffixes.
    if let Some((head, tail)) = cleaned.rsplit_once(" - ") {
        let tail_lower = tail.to_lowercase();
        if noise_phrases.iter().any(|p| contains_keyword(&tail_lower, p))
            && !PENALTY_KEYWORDS
                .iter()
                .any(|kw| contains_keyword(&tail_lower, kw))
        {
            cleaned = head.to_string();
        }
    }

    cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Strips parenthetical/bracketed/dash subtitles to recover the core song title
/// (e.g. `Pelukku Untuk Pelikmu - OST Imperfect...` -> `Pelukku Untuk Pelikmu`).
fn strip_subtitle(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut depth_paren = 0usize;
    let mut depth_brack = 0usize;
    for ch in title.chars() {
        match ch {
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '[' => depth_brack += 1,
            ']' => depth_brack = depth_brack.saturating_sub(1),
            _ if depth_paren == 0 && depth_brack == 0 => out.push(ch),
            _ => {}
        }
    }
    if let Some((head, _)) = out.split_once(" - ") {
        if !head.trim().is_empty() {
            out = head.to_string();
        }
    }
    if let Some((head, _)) = out.split_once(" | ") {
        if !head.trim().is_empty() {
            out = head.to_string();
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Cleans YouTube video upload noise (e.g. `(Official Lyric Video)`, `| Lirik Lagu`) from a
/// candidate title without discarding version keywords like `Live`, `Cover`, or `Karaoke`.
fn clean_candidate_title(title: &str) -> String {
    let mut working = title.to_string();
    // Strip trailing pipe segments like `| Lirik Lagu` or `| Official Music Video` when they are noise.
    if let Some((head, tail)) = working.split_once('|') {
        let tail_lower = tail.to_lowercase();
        let tail_is_noise = VIDEO_NOISE_PHRASES
            .iter()
            .any(|p| contains_keyword(&tail_lower, p))
            || NOISE_PHRASES
                .iter()
                .any(|p| contains_keyword(&tail_lower, p));
        let tail_has_penalty = PENALTY_KEYWORDS
            .iter()
            .any(|kw| contains_keyword(&tail_lower, kw));
        if !head.trim().is_empty() && tail_is_noise && !tail_has_penalty {
            working = head.trim().to_string();
        }
    }
    let stage1 = strip_noise_tags(&working, NOISE_PHRASES);
    strip_noise_tags(&stage1, VIDEO_NOISE_PHRASES)
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

/// Builds a simplified fallback query stripping OST/bracket/dash subtitles when the primary
/// query fails to produce a high-confidence match.
fn build_simplified_fallback_query(track: &Track) -> Option<String> {
    let primary = build_fallback_search_query(track);
    let base_title = strip_subtitle(&clean_track_title(&track.title));
    if base_title.is_empty() {
        return None;
    }
    let artist = track
        .artists
        .first()
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    let simplified = if artist.is_empty() || artist.eq_ignore_ascii_case("unknown artist") {
        base_title
    } else {
        format!("{artist} {base_title}")
    };
    if simplified.eq_ignore_ascii_case(&primary) {
        None
    } else {
        Some(simplified)
    }
}

fn tokenize(s: &str) -> HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|tok| !tok.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn token_jaccard(set_a: &HashSet<String>, set_b: &HashSet<String>) -> f32 {
    if set_a.is_empty() && set_b.is_empty() {
        return 1.0;
    }
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let inter = set_a.intersection(set_b).count() as f32;
    let union = set_a.union(set_b).count() as f32;
    inter / union
}

fn contains_keyword(haystack_lower: &str, kw: &str) -> bool {
    if kw.contains(' ') || kw.contains('-') || kw.contains('/') {
        return haystack_lower.contains(kw);
    }
    haystack_lower
        .split(|c: char| !c.is_alphanumeric())
        .any(|tok| tok == kw)
}

fn is_meaningful_artist(artist: &str) -> bool {
    let trimmed = artist.trim();
    !trimmed.is_empty()
        && !trimmed.eq_ignore_ascii_case("unknown artist")
        && !trimmed.eq_ignore_ascii_case("various artists")
}

/// If a YouTube video title is formatted as `Artist - Title` or `Title - Artist`, strips the
/// artist segment so title similarity evaluates only the song title itself.
fn strip_artist_from_video_title(
    cand_title: &str,
    primary_artist_tokens: &HashSet<String>,
    sp_title_tokens: &HashSet<String>,
) -> String {
    if primary_artist_tokens.is_empty() {
        return cand_title.to_string();
    }
    if let Some((left, right)) = cand_title.split_once(" - ") {
        let left_tokens = tokenize(left);
        let right_tokens = tokenize(right);
        let left_is_artist = !left_tokens.is_empty()
            && primary_artist_tokens.is_subset(&left_tokens)
            && !(sp_title_tokens.is_subset(&left_tokens) && !sp_title_tokens.is_empty());
        let right_is_artist = !right_tokens.is_empty()
            && primary_artist_tokens.is_subset(&right_tokens)
            && !(sp_title_tokens.is_subset(&right_tokens) && !sp_title_tokens.is_empty());

        if left_is_artist && !right_tokens.is_empty() {
            return right.trim().to_string();
        }
        if right_is_artist && !left_tokens.is_empty() {
            return left.trim().to_string();
        }
    }
    cand_title.to_string()
}

/// Scores a YouTube Music candidate against a Spotify track (higher is better, `0.0..=1.0`).
pub fn score_ytm_candidate(spotify_track: &Track, candidate: &Track) -> f32 {
    let sp_title_clean = clean_track_title(&spotify_track.title);
    let cand_title_clean = clean_candidate_title(&candidate.title);

    let sp_title_tokens = tokenize(&sp_title_clean);
    let primary_artist = spotify_track
        .artists
        .first()
        .map(String::as_str)
        .unwrap_or_default()
        .trim();
    let has_real_artist = spotify_track.artists.iter().any(|a| is_meaningful_artist(a));
    let primary_artist_tokens = if is_meaningful_artist(primary_artist) {
        tokenize(primary_artist)
    } else {
        HashSet::new()
    };

    // Strip `Artist - Title` / `Title - Artist` wrapper common on YouTube Video uploads.
    let cand_title_unwrapped =
        strip_artist_from_video_title(&cand_title_clean, &primary_artist_tokens, &sp_title_tokens);
    let cand_title_tokens = tokenize(&cand_title_unwrapped);

    let mut title_sim = token_jaccard(&sp_title_tokens, &cand_title_tokens);
    if !sp_title_tokens.is_empty() && sp_title_tokens == cand_title_tokens {
        title_sim = 1.0;
    } else {
        let sp_base_tokens = tokenize(&strip_subtitle(&sp_title_clean));
        let cand_base_tokens = tokenize(&strip_subtitle(&cand_title_unwrapped));
        if !sp_base_tokens.is_empty() && sp_base_tokens == cand_base_tokens {
            title_sim = title_sim.max(0.88);
        }
    }

    // Artist matching: prefer official channel / YTM artist metadata, allow artist-in-video-title
    // for lyric/audio uploads, and heavily penalize zero-artist-overlap candidates.
    let sp_artist_tokens = tokenize(&spotify_track.artists.join(" "));
    let cand_artist_tokens = tokenize(&candidate.artists.join(" "));
    let mut channel_artist_sim = token_jaccard(&sp_artist_tokens, &cand_artist_tokens);
    if !primary_artist_tokens.is_empty() && primary_artist_tokens.is_subset(&cand_artist_tokens) {
        channel_artist_sim = channel_artist_sim.max(0.92);
    } else if spotify_track.artists.iter().any(|a| {
        let toks = tokenize(a);
        !toks.is_empty() && toks.is_subset(&cand_artist_tokens)
    }) {
        channel_artist_sim = channel_artist_sim.max(0.85);
    }

    let raw_cand_title_tokens = tokenize(&candidate.title);
    let title_mentions_artist = !primary_artist_tokens.is_empty()
        && primary_artist_tokens.is_subset(&raw_cand_title_tokens);

    let channel_matches_album = spotify_track
        .album
        .as_deref()
        .map(|alb| {
            let alb_tokens = tokenize(alb);
            !cand_artist_tokens.is_empty()
                && cand_artist_tokens.len() >= 2
                && cand_artist_tokens.is_subset(&alb_tokens)
        })
        .unwrap_or(false);

    let (artist_sim, artist_mismatch_penalty) = if !has_real_artist {
        (0.5_f32, 0.0_f32)
    } else if channel_artist_sim > 0.0 {
        (channel_artist_sim, 0.0_f32)
    } else if title_mentions_artist {
        // Uploader channel matches the compilation/studio album (e.g. "Lost Ark Studio" for
        // "Lost Ark Studio Compilation, Vol. 8") -> treat as stronger official source than fan reuploads.
        let score = if channel_matches_album { 0.85_f32 } else { 0.68_f32 };
        (score, 0.0_f32)
    } else {
        (0.0_f32, 0.45_f32)
    };

    let title_mismatch_penalty = if !sp_title_tokens.is_empty() && title_sim == 0.0 {
        0.40_f32
    } else {
        0.0_f32
    };

    // Duration proximity score (strong preference within ±5s, steep penalty past ±40s).
    let duration_score = if spotify_track.duration_ms > 0 && candidate.duration_ms > 0 {
        let diff_s = spotify_track.duration_ms.abs_diff(candidate.duration_ms) as f32 / 1000.0;
        if diff_s <= 2.0 {
            1.0
        } else if diff_s <= 5.0 {
            0.92
        } else if diff_s <= 10.0 {
            0.78
        } else if diff_s <= 15.0 {
            0.55
        } else if diff_s <= 25.0 {
            0.20
        } else if diff_s <= 40.0 {
            -0.15
        } else {
            -0.55
        }
    } else {
        0.45
    };

    // Penalty for unintended live/cover/karaoke/slowed/reverb/acoustic/remix/etc. versions.
    let sp_context_lower = match &spotify_track.album {
        Some(alb) if !alb.is_empty() => {
            format!("{} {}", spotify_track.title, alb).to_lowercase()
        }
        _ => spotify_track.title.to_lowercase(),
    };
    let cand_full_lower = candidate.title.to_lowercase();
    let mut version_penalty = 0.0f32;
    for kw in PENALTY_KEYWORDS {
        let in_candidate = contains_keyword(&cand_full_lower, kw);
        let in_source = contains_keyword(&sp_context_lower, kw);
        if in_candidate && !in_source {
            version_penalty += 0.45;
        }
    }

    let raw = (title_sim * 0.46)
        + (artist_sim * 0.30)
        + (duration_score * 0.24)
        - version_penalty
        - artist_mismatch_penalty
        - title_mismatch_penalty;
    raw.clamp(0.0, 1.0)
}

fn select_best_scored_candidate(
    spotify_track: &Track,
    candidates: &[(Track, bool)],
    exclude_video_id: Option<&str>,
) -> Option<FallbackMatch> {
    let spotify_id = match &spotify_track.source {
        TrackSource::Spotify { id } => id.clone(),
        _ => spotify_track.uid.clone(),
    };

    let scored: Vec<(&str, f32)> = candidates
        .iter()
        .filter_map(|(cand, is_video)| {
            let TrackSource::YouTubeMusic { video_id } = &cand.source else {
                return None;
            };
            let base_score = score_ytm_candidate(spotify_track, cand);
            let adjusted = if *is_video {
                (base_score - VIDEO_TIEBREAKER_PENALTY).max(0.0)
            } else {
                base_score
            };
            Some((video_id.as_str(), adjusted))
        })
        .collect();

    // First try excluding any previously rejected video ID (used during "Wrong version? Re-match").
    // Break score ties in favor of earlier search rank (higher YouTube relevance / view rank).
    let best_filtered = scored
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, (vid, score))| exclude_video_id != Some(*vid) && *score >= MIN_MATCH_CONFIDENCE)
        .max_by(|(idx_a, a), (idx_b, b)| a.1.total_cmp(&b.1).then_with(|| idx_b.cmp(idx_a)))
        .map(|(_, pair)| pair);

    let chosen = best_filtered.or_else(|| {
        scored
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, (_, score))| *score >= MIN_MATCH_CONFIDENCE)
            .max_by(|(idx_a, a), (idx_b, b)| a.1.total_cmp(&b.1).then_with(|| idx_b.cmp(idx_a)))
            .map(|(_, pair)| pair)
    })?;

    Some(FallbackMatch {
        spotify_track_id: spotify_id,
        yt_video_id: chosen.0.to_string(),
        confidence: chosen.1,
        from_cache: false,
    })
}

/// Selects the highest-scoring YouTube Music candidate meeting `MIN_MATCH_CONFIDENCE`,
/// optionally excluding a previously rejected video ID.
pub fn select_best_ytm_match(
    spotify_track: &Track,
    candidates: &[Track],
    exclude_video_id: Option<&str>,
) -> Option<FallbackMatch> {
    let tagged: Vec<(Track, bool)> = candidates.iter().cloned().map(|t| (t, false)).collect();
    select_best_scored_candidate(spotify_track, &tagged, exclude_video_id)
}

fn push_unique_candidates(
    pool: &mut Vec<(Track, bool)>,
    seen: &mut HashSet<String>,
    items: Vec<Track>,
    is_video: bool,
) {
    for item in items {
        if let TrackSource::YouTubeMusic { video_id } = &item.source {
            if seen.insert(video_id.clone()) {
                pool.push((item, is_video));
            }
        }
    }
}

/// Resolves a Spotify track to its YouTube Music fallback video ID using SQLite `yt_mappings` cache
/// first (unless `force_rematch` is true or cached confidence is below `CACHE_REVALIDATE_CONFIDENCE`),
/// falling back to a multi-stage live search across YouTube Music Songs and YouTube Music Videos.
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
        if let Some((yt_video_id, confidence)) = &previous_video_id {
            if should_use_cached_mapping(*confidence) {
                return Ok(FallbackMatch {
                    spotify_track_id: spotify_id.clone(),
                    yt_video_id: yt_video_id.clone(),
                    confidence: *confidence,
                    from_cache: true,
                });
            }
        }
    }

    let exclude_id = if force_rematch {
        previous_video_id.as_ref().map(|(id, _)| id.as_str())
    } else {
        None
    };

    let query = build_fallback_search_query(track);
    let mut pool: Vec<(Track, bool)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut primary_err: Option<CoreError> = None;

    // Stage 1: Search YouTube Music Songs (official studio Art Tracks).
    match ytm.search(&query, 10).await {
        Ok(songs) => push_unique_candidates(&mut pool, &mut seen, songs, false),
        Err(e) => primary_err = Some(e),
    }

    let mut best = select_best_scored_candidate(track, &pool, exclude_id);

    // Stage 2: If Songs didn't yield a high-confidence studio match (or user requested Re-match),
    // also search YouTube Music Videos (catches official Lyric Videos / MVs not distributed as Songs).
    let need_video_fallback = force_rematch
        || best
            .as_ref()
            .map(|m| m.confidence < HIGH_CONFIDENCE_THRESHOLD)
            .unwrap_or(true);
    if need_video_fallback {
        match ytm.search_videos(&query, 10).await {
            Ok(videos) => {
                push_unique_candidates(&mut pool, &mut seen, videos, true);
                best = select_best_scored_candidate(track, &pool, exclude_id);
            }
            Err(e) if pool.is_empty() && primary_err.is_none() => {
                primary_err = Some(e);
            }
            Err(_) => {}
        }
    }

    // Stage 3: If still below high confidence and the title has a subtitle (e.g. OST/bracket tag),
    // retry with a simplified base-title query across Songs + Videos.
    let still_low = best
        .as_ref()
        .map(|m| m.confidence < HIGH_CONFIDENCE_THRESHOLD)
        .unwrap_or(true);
    if still_low {
        if let Some(alt_query) = build_simplified_fallback_query(track) {
            let (alt_songs, alt_videos) = tokio::join!(
                ytm.search(&alt_query, 10),
                ytm.search_videos(&alt_query, 10)
            );
            if let Ok(songs) = alt_songs {
                push_unique_candidates(&mut pool, &mut seen, songs, false);
            }
            if let Ok(videos) = alt_videos {
                push_unique_candidates(&mut pool, &mut seen, videos, true);
            }
            best = select_best_scored_candidate(track, &pool, exclude_id);
        }
    }

    let best = match best {
        Some(m) => m,
        None => {
            if let Some(err) = primary_err {
                if pool.is_empty() {
                    return Err(err);
                }
            }
            return Err(CoreError::NotFound(format!(
                "No accurate YouTube Music fallback match found for \"{}\" by {}",
                track.title,
                track.artist_display()
            )));
        }
    };

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

    #[test]
    fn rejects_unrelated_artist_partial_title_match_like_pelangi_vs_pelangi_cinta() {
        let sp = mk_spotify("3ZwqWrRpylU9nfy99qwKdR", "Pelangi", "Hivi!", 217_626);
        let song_candidates = vec![
            mk_ytm("qkC4waEZVt4", "Rahasia Hati", "NIDJI", 251_000),
            mk_ytm("9Xz7Fo46t8s", "Pelangi (Live)", "Hivi!", 266_000),
            mk_ytm(
                "CndSd5u-Xqc",
                "Pelangi Cinta (feat. Afifah Yusuf)",
                "Diskoria",
                322_000,
            ),
        ];

        let diskoria_score = score_ytm_candidate(&sp, &song_candidates[2]);
        assert!(
            diskoria_score < 0.25,
            "unrelated artist/song Diskoria - Pelangi Cinta should score very low, got {diskoria_score}"
        );
        // Must never pick Diskoria - Pelangi Cinta even when no studio YTM Song exists.
        if let Some(m) = select_best_ytm_match(&sp, &song_candidates, None) {
            assert_ne!(
                m.yt_video_id, "CndSd5u-Xqc",
                "must never select unrelated Diskoria track"
            );
        }

        // Same title by a totally different artist (even with identical duration) must be rejected.
        let wrong_artist_same_title = vec![mk_ytm("vid_wrong", "Pelangi", "Koes Plus", 218_000)];
        assert!(
            select_best_ytm_match(&sp, &wrong_artist_same_title, None).is_none(),
            "same title with completely different artist must be rejected"
        );
    }

    #[test]
    fn selects_official_ytm_video_studio_audio_over_live_mv_karaoke_cover_and_fan_uploads() {
        let sp = mk_spotify("3ZwqWrRpylU9nfy99qwKdR", "Pelangi", "Hivi!", 217_626);
        let candidates = vec![
            mk_ytm("9Xz7Fo46t8s", "Pelangi (Live)", "Hivi!", 266_000),
            mk_ytm(
                "CndSd5u-Xqc",
                "Pelangi Cinta (feat. Afifah Yusuf)",
                "Diskoria",
                322_000,
            ),
            mk_ytm("bBjENsfyvFs", "Pelangi", "Hivi!", 222_000),
            mk_ytm("Q6EyNEX6BOc", "Pelangi", "Hivi!", 218_000),
            mk_ytm(
                "Y9UzdUrPd6Y",
                "Pelangi - HIVI! | Lirik Lagu",
                "Indolirik",
                219_000,
            ),
            mk_ytm(
                "-lweUcTlPJc",
                "Pelangi - HIVI! (KARAOKE VERSION)",
                "Karaoke Music",
                214_000,
            ),
            mk_ytm(
                "A1bAjk3FA7s",
                "Pelangi - HiVi | by Nadia & Yoseph (NY Cover)",
                "Nadia Darmawan Official",
                215_000,
            ),
        ];

        let winner = select_best_ytm_match(&sp, &candidates, None).unwrap();
        assert_eq!(winner.yt_video_id, "Q6EyNEX6BOc");
        assert!(
            winner.confidence > 0.90,
            "expected high confidence for exact official lyric video, got {}",
            winner.confidence
        );

        // If no official channel upload exists, a clean fan lyric upload mentioning the artist in title still wins cleanly.
        let fan_only = vec![
            mk_ytm(
                "CndSd5u-Xqc",
                "Pelangi Cinta (feat. Afifah Yusuf)",
                "Diskoria",
                322_000,
            ),
            mk_ytm(
                "Y9UzdUrPd6Y",
                "Pelangi - HIVI! | Lirik Lagu",
                "Indolirik",
                219_000,
            ),
            mk_ytm(
                "-lweUcTlPJc",
                "Pelangi - HIVI! (KARAOKE VERSION)",
                "Karaoke Music",
                214_000,
            ),
        ];
        let fan_winner = select_best_ytm_match(&sp, &fan_only, None).unwrap();
        assert_eq!(fan_winner.yt_video_id, "Y9UzdUrPd6Y");
        assert!(
            fan_winner.confidence > 0.70,
            "expected solid confidence for fan lyric upload, got {}",
            fan_winner.confidence
        );
    }

    #[test]
    fn penalizes_unintended_acoustic_but_honors_intentional_acoustic_version() {
        let sp_studio = mk_spotify("sp_kita", "Kita", "Sheila On 7", 276_000);
        let acoustic = mk_ytm("vid_ac", "Kita (Acoustic)", "Sheila On 7", 211_000);
        let studio = mk_ytm("vid_st", "Kita", "Sheila On 7", 277_000);
        assert!(score_ytm_candidate(&sp_studio, &studio) > score_ytm_candidate(&sp_studio, &acoustic) + 0.5);

        let sp_acoustic = mk_spotify(
            "sp_niki",
            "La La Lost You - Acoustic Version",
            "NIKI",
            208_000,
        );
        let ytm_acoustic = mk_ytm(
            "3s4bzVIAHEU",
            "La La Lost You (Acoustic Version)",
            "NIKI",
            209_000,
        );
        let ytm_studio = mk_ytm("gzwISf_1T4A", "La La Lost You", "NIKI", 200_000);
        let winner = select_best_ytm_match(&sp_acoustic, &[ytm_studio, ytm_acoustic], None).unwrap();
        assert_eq!(winner.yt_video_id, "3s4bzVIAHEU");
    }

    #[test]
    fn revalidates_low_confidence_cached_mappings_and_prefers_songs_over_videos_on_tie() {
        assert!(!should_use_cached_mapping(0.3096));
        assert!(!should_use_cached_mapping(0.64));
        assert!(should_use_cached_mapping(0.65));
        assert!(should_use_cached_mapping(1.0));

        let sp = mk_spotify("sp_song", "Yellow", "Coldplay", 266_773);
        let song = mk_ytm("vid_song", "Yellow", "Coldplay", 267_000);
        let video = mk_ytm("vid_video", "Yellow", "Coldplay", 267_000);
        let winner = select_best_scored_candidate(&sp, &[(video, true), (song, false)], None).unwrap();
        assert_eq!(winner.yt_video_id, "vid_song");
    }

    #[test]
    fn simplifies_ost_and_bracket_subtitles_for_fallback_query() {
        let sp = mk_spotify(
            "sp_ost",
            "Pelukku Untuk Pelikmu - OST Imperfect: Karier, Cinta, & Timbangan",
            "Fiersa Besari",
            249_000,
        );
        assert_eq!(
            build_simplified_fallback_query(&sp).as_deref(),
            Some("Fiersa Besari Pelukku Untuk Pelikmu")
        );
    }

    #[tokio::test]
    #[ignore = "live network match check"]
    async fn live_resolves_hivi_pelangi_to_official_studio_audio() {
        let dir = tempfile::TempDir::new().unwrap();
        let db = Mutex::new(Database::open_in_memory().unwrap());
        // Seed the exact poisoned low-confidence mapping that existed in production.
        db.lock()
            .unwrap()
            .put_yt_mapping("3ZwqWrRpylU9nfy99qwKdR", "CndSd5u-Xqc", 0.3096)
            .unwrap();

        let ytm = YouTubeMusicProvider::new(dir.path().to_path_buf(), None).unwrap();
        let sp = mk_spotify("3ZwqWrRpylU9nfy99qwKdR", "Pelangi", "Hivi!", 217_626);

        let matched = resolve_spotify_to_ytm_video_id(&sp, &db, &ytm, false)
            .await
            .expect("should resolve Hivi! - Pelangi via YTM video fallback");
        println!("Resolved Hivi! - Pelangi => {matched:?}");
        assert!(!matched.from_cache, "stale 0.3096 cache entry must be revalidated");
        assert_ne!(matched.yt_video_id, "CndSd5u-Xqc", "must never pick Diskoria - Pelangi Cinta");
        assert!(
            matched.confidence > 0.85,
            "expected high confidence match, got {}",
            matched.confidence
        );
        // Should pick official HIVI! Pelangi lyric video (Q6EyNEX6BOc) or MV (bBjENsfyvFs).
        assert!(
            matches!(matched.yt_video_id.as_str(), "Q6EyNEX6BOc" | "bBjENsfyvFs"),
            "expected official HIVI! Pelangi upload, got {}",
            matched.yt_video_id
        );

        // Second call must hit SQLite cache immediately with the upgraded video ID.
        let cached = resolve_spotify_to_ytm_video_id(&sp, &db, &ytm, false)
            .await
            .unwrap();
        assert!(cached.from_cache);
        assert_eq!(cached.yt_video_id, matched.yt_video_id);
    }

    #[test]
    fn selects_lost_ark_studio_waiting_room_over_unrelated_topics_and_fan_uploads() {
        let mut sp = mk_spotify(
            "04mAOoQNsXmDJlyupJwmkO",
            "Waiting Room",
            "Phoebe Bridgers",
            393_000,
        );
        sp.album = Some("Lost Ark Studio Compilation, Vol. 8".into());

        let candidates = vec![
            (mk_ytm("iAPYjjQ-Ijs", "Waiting Room", "Wellsgoodness", 600_000), false),
            (mk_ytm("SN4OCw4L3Yc", "Waiting Room (Acoustic)", "Real Friends", 201_000), false),
            (
                mk_ytm(
                    "VUOwg6nhuzA",
                    "Phoebe Bridgers - Waiting Room",
                    "Lost Ark Studio",
                    394_000,
                ),
                true,
            ),
            (
                mk_ytm(
                    "4_60HOTJSCk",
                    "Phoebe Bridgers - Waiting Room (Lyrics)",
                    "April ♡",
                    392_000,
                ),
                true,
            ),
            (
                mk_ytm(
                    "AMGTUg_mECQ",
                    "Phoebe Bridgers - Waiting Room (Live on KEXP)",
                    "KEXP",
                    269_000,
                ),
                true,
            ),
        ];

        let winner = select_best_scored_candidate(&sp, &candidates, None).unwrap();
        assert_eq!(winner.yt_video_id, "VUOwg6nhuzA");
        assert!(
            winner.confidence > 0.90,
            "expected high confidence for Lost Ark Studio official upload, got {}",
            winner.confidence
        );
    }

    #[tokio::test]
    #[ignore = "live network match check"]
    async fn live_resolves_phoebe_bridgers_waiting_room_to_lost_ark_studio() {
        let dir = tempfile::TempDir::new().unwrap();
        let db = Mutex::new(Database::open_in_memory().unwrap());
        db.lock()
            .unwrap()
            .put_yt_mapping("04mAOoQNsXmDJlyupJwmkO", "iAPYjjQ-Ijs", 0.396)
            .unwrap();

        let ytm = YouTubeMusicProvider::new(dir.path().to_path_buf(), None).unwrap();
        let mut sp = mk_spotify(
            "04mAOoQNsXmDJlyupJwmkO",
            "Waiting Room",
            "Phoebe Bridgers",
            393_000,
        );
        sp.album = Some("Lost Ark Studio Compilation, Vol. 8".into());

        let matched = resolve_spotify_to_ytm_video_id(&sp, &db, &ytm, false)
            .await
            .expect("should resolve Phoebe Bridgers - Waiting Room");
        println!("Resolved Phoebe Bridgers - Waiting Room => {matched:?}");
        assert!(!matched.from_cache);
        assert_eq!(matched.yt_video_id, "VUOwg6nhuzA");
        assert!(matched.confidence > 0.85);
    }
}
