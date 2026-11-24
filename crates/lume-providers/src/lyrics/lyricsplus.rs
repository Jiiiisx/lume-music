use std::time::Duration;

use lume_core::{CoreError, LyricsData, LyricsLine, LyricsWord, Track};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use crate::spotify::clean_track_title;

pub const DEFAULT_LYRICSPLUS_BASES: &[&str] = &[
    "https://lyricsplus.prjktla.my.id",
    "https://lyricsplus.binimum.org",
];

const LYRICSPLUS_SOURCES: &str = "apple,lyricsplus,musixmatch-word,musixmatch,spotify";

#[derive(Debug, Deserialize)]
struct LyricsPlusResponse {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    metadata: LyricsPlusMetadata,
    #[serde(default)]
    lyrics: Vec<LyricsPlusLine>,
}

#[derive(Debug, Default, Deserialize)]
struct LyricsPlusMetadata {
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LyricsPlusLine {
    #[serde(default)]
    time: u64,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    text: String,
    #[serde(default)]
    syllabus: Vec<LyricsPlusSyllable>,
    #[serde(default)]
    element: Option<LyricsPlusElement>,
}

#[derive(Debug, Deserialize)]
struct LyricsPlusSyllable {
    #[serde(default)]
    time: u64,
    #[serde(default)]
    duration: u64,
    #[serde(default)]
    text: String,
    #[serde(rename = "isBackground", default)]
    is_background: bool,
}

#[derive(Debug, Deserialize)]
struct LyricsPlusElement {
    #[serde(default)]
    singer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SonglistSearchResponse {
    #[serde(default)]
    results: Vec<SonglistCandidate>,
}

#[derive(Debug, Deserialize)]
struct SonglistCandidate {
    #[serde(default)]
    title: String,
    #[serde(default)]
    artist: String,
    #[serde(default)]
    album: String,
    #[serde(rename = "durationMs", default)]
    duration_ms: u64,
}

fn normalize_provider_name(source: Option<&str>) -> String {
    let raw = source.unwrap_or("lyricsplus").trim().to_lowercase();
    if raw.contains("apple") {
        "apple".to_string()
    } else if raw.contains("musixmatch") {
        "musixmatch".to_string()
    } else if raw.contains("qq") {
        "qq".to_string()
    } else if raw.contains("spotify") {
        "spotify".to_string()
    } else if raw.contains("netease") {
        "netease".to_string()
    } else if raw.is_empty() {
        "lyricsplus".to_string()
    } else {
        raw
    }
}

/// Convert raw `syllabus` entries into `LyricsWord` items preserving Spicy Lyrics' `is_part_of_word`.
///
/// Handles both Apple Music TTML style (trailing space inside syllable text: `"da"` + `"tang "`)
/// and QQ Music style (separate whitespace-only tokens: `"Abel"`, `" "`, `"The"`).
fn build_syllable_words(raw_syllables: &[&LyricsPlusSyllable]) -> Vec<LyricsWord> {
    let mut words: Vec<LyricsWord> = Vec::with_capacity(raw_syllables.len());

    for syl in raw_syllables {
        let raw = syl.text.as_str();
        // Whitespace-only separator token (e.g. QQ Music) ends the previous word cleanly.
        if raw.trim().is_empty() {
            if let Some(prev) = words.last_mut() {
                prev.is_part_of_word = false;
            }
            continue;
        }

        let has_leading_space = raw.chars().next().is_some_and(|c| c.is_whitespace());
        let has_trailing_space = raw.chars().last().is_some_and(|c| c.is_whitespace());
        if has_leading_space {
            if let Some(prev) = words.last_mut() {
                prev.is_part_of_word = false;
            }
        }

        let text = raw.trim().to_string();
        if text.is_empty() {
            continue;
        }

        let start_ms = syl.time;
        let end_ms = if syl.duration > 0 {
            start_ms.saturating_add(syl.duration)
        } else {
            start_ms.saturating_add(1)
        };

        words.push(LyricsWord {
            start_ms,
            end_ms: end_ms.max(start_ms + 1),
            text,
            // If no trailing whitespace, tentatively joins directly into the next syllable;
            // the last word of the line is cleared below, and any leading-space or
            // whitespace-only next token also clears this flag.
            is_part_of_word: !has_trailing_space,
            is_background: syl.is_background,
        });
    }

    // Never leave `is_part_of_word` dangling on the last syllable of a line.
    if let Some(last) = words.last_mut() {
        last.is_part_of_word = false;
    }

    // Ensure monotonic non-zero durations without overlaps breaking progress.
    for i in 0..words.len() {
        if words[i].end_ms <= words[i].start_ms {
            let next_start = words
                .get(i + 1)
                .map(|w| w.start_ms)
                .filter(|&s| s > words[i].start_ms);
            words[i].end_ms = next_start.unwrap_or(words[i].start_ms + 120);
        }
    }

    words
}

/// Reconstruct clean display text from syllable words respecting `is_part_of_word`.
pub fn join_syllable_words_text(words: &[LyricsWord]) -> String {
    let mut out = String::new();
    for (idx, w) in words.iter().enumerate() {
        out.push_str(&w.text);
        if idx + 1 < words.len() && !w.is_part_of_word {
            out.push(' ');
        }
    }
    out
}

/// Parse a LyricsPlus `/v2/lyrics/get` JSON payload into Lume's unified `LyricsData`.
pub fn parse_lyricsplus_json(raw_json: &str) -> Result<Option<LyricsData>, CoreError> {
    let resp: LyricsPlusResponse = serde_json::from_str(raw_json)
        .map_err(|e| CoreError::Network(format!("LyricsPlus JSON parse error: {e}")))?;

    if resp.lyrics.is_empty() {
        return Ok(None);
    }

    let provider = normalize_provider_name(resp.metadata.source.as_deref());

    // Detect duet lines: if there are multiple distinct individual singers (e.g. `v1` and `v2`),
    // mark secondary singers as `opposite_aligned` (right-aligned, matching Spicy Lyrics).
    let mut primary_singer: Option<String> = None;
    let mut has_duet = false;
    for line in &resp.lyrics {
        if let Some(singer) = line.element.as_ref().and_then(|e| e.singer.as_deref()) {
            let s = singer.trim();
            if s.is_empty() || s == "v1000" || s.eq_ignore_ascii_case("group") {
                continue;
            }
            match &primary_singer {
                None => primary_singer = Some(s.to_string()),
                Some(first) if first != s => {
                    has_duet = true;
                    break;
                }
                _ => {}
            }
        }
    }

    let mut synced: Vec<LyricsLine> = Vec::with_capacity(resp.lyrics.len() + 8);
    let mut plain_lines: Vec<String> = Vec::with_capacity(resp.lyrics.len());

    let is_word_synced = resp.kind.eq_ignore_ascii_case("Word")
        || resp.kind.eq_ignore_ascii_case("Syllable")
        || resp.lyrics.iter().any(|l| !l.syllabus.is_empty());

    for line in &resp.lyrics {
        let singer = line
            .element
            .as_ref()
            .and_then(|e| e.singer.as_deref())
            .map(str::trim);
        let opposite_aligned = has_duet
            && matches!(
                (primary_singer.as_deref(), singer),
                (Some(first), Some(cur)) if !cur.is_empty() && cur != first && cur != "v1000"
            );

        if is_word_synced && !line.syllabus.is_empty() {
            let lead_syls: Vec<&LyricsPlusSyllable> =
                line.syllabus.iter().filter(|s| !s.is_background).collect();
            let bg_syls: Vec<&LyricsPlusSyllable> =
                line.syllabus.iter().filter(|s| s.is_background).collect();

            if !lead_syls.is_empty() {
                let words = build_syllable_words(&lead_syls);
                if !words.is_empty() {
                    let start_ms = words.first().map(|w| w.start_ms).unwrap_or(line.time);
                    let end_ms = words
                        .last()
                        .map(|w| w.end_ms)
                        .or_else(|| (line.duration > 0).then_some(line.time + line.duration));
                    let text = join_syllable_words_text(&words);
                    if !text.trim().is_empty() {
                        plain_lines.push(text.clone());
                        synced.push(LyricsLine {
                            time_ms: start_ms,
                            end_ms,
                            text,
                            words: Some(words),
                            opposite_aligned,
                            is_background: false,
                        });
                    }
                }
            }

            if !bg_syls.is_empty() {
                let bg_words = build_syllable_words(&bg_syls);
                if !bg_words.is_empty() {
                    let start_ms = bg_words.first().map(|w| w.start_ms).unwrap_or(line.time);
                    let end_ms = bg_words.last().map(|w| w.end_ms);
                    let text = join_syllable_words_text(&bg_words);
                    if !text.trim().is_empty() {
                        plain_lines.push(text.clone());
                        synced.push(LyricsLine {
                            time_ms: start_ms,
                            end_ms,
                            text,
                            words: Some(bg_words),
                            opposite_aligned,
                            is_background: true,
                        });
                    }
                }
            }
        } else {
            let text = line.text.trim().to_string();
            if text.is_empty() {
                continue;
            }
            let end_ms = (line.duration > 0).then_some(line.time.saturating_add(line.duration));
            plain_lines.push(text.clone());
            synced.push(LyricsLine {
                time_ms: line.time,
                end_ms,
                text,
                words: None,
                opposite_aligned,
                is_background: false,
            });
        }
    }

    if synced.is_empty() {
        return Ok(None);
    }

    synced.sort_by_key(|l| (l.time_ms, l.is_background));

    Ok(Some(LyricsData {
        synced,
        plain: (!plain_lines.is_empty()).then(|| plain_lines.join("\n")),
        provider,
    }))
}

/// HTTP client for public LyricsPlus v2 syllable-synced lyrics endpoints (Apple Music TTML,
/// Musixmatch Word, QQ Music, LyricsPlus UGC).
#[derive(Debug, Clone)]
pub struct LyricsPlusClient {
    base_urls: Vec<String>,
    http: Client,
}

impl LyricsPlusClient {
    pub fn new(base_urls: Vec<String>) -> Self {
        let http = Client::builder()
            .user_agent("LumeMusic/1.0")
            .timeout(Duration::from_secs(9))
            .build()
            .unwrap_or_else(|_| Client::new());
        let cleaned = base_urls
            .into_iter()
            .map(|u| u.trim_end_matches('/').to_string())
            .filter(|u| !u.is_empty())
            .collect();
        Self {
            base_urls: cleaned,
            http,
        }
    }

    pub fn default_public() -> Self {
        Self::new(
            DEFAULT_LYRICSPLUS_BASES
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
        )
    }

    async fn query_v2(
        &self,
        base: &str,
        title: &str,
        artist: &str,
        album: Option<&str>,
        duration_s: Option<u64>,
    ) -> Result<Option<LyricsData>, CoreError> {
        let mut params: Vec<(&str, String)> = vec![
            ("title", title.to_string()),
            ("artist", artist.to_string()),
            ("source", LYRICSPLUS_SOURCES.to_string()),
        ];
        if let Some(alb) = album.filter(|a| !a.trim().is_empty()) {
            params.push(("album", alb.to_string()));
        }
        if let Some(dur) = duration_s.filter(|&d| d > 0) {
            params.push(("duration", dur.to_string()));
        }

        let url = format!("{base}/v2/lyrics/get");
        let resp = self
            .http
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| CoreError::Network(e.to_string()))?;

        match resp.status() {
            StatusCode::NOT_FOUND => return Ok(None),
            status if !status.is_success() => {
                return Err(CoreError::Network(format!(
                    "LyricsPlus {base} status {status}"
                )));
            }
            _ => {}
        }

        let body = resp
            .text()
            .await
            .map_err(|e| CoreError::Network(e.to_string()))?;
        parse_lyricsplus_json(&body)
    }

    async fn search_songlist(
        &self,
        base: &str,
        query: &str,
        target_duration_ms: u64,
    ) -> Option<SonglistCandidate> {
        let url = format!("{base}/v1/songlist/search");
        let resp = self
            .http
            .get(&url)
            .query(&[("q", query)])
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let parsed: SonglistSearchResponse = resp.json().await.ok()?;
        if parsed.results.is_empty() {
            return None;
        }
        if target_duration_ms == 0 {
            return parsed.results.into_iter().next();
        }
        parsed
            .results
            .into_iter()
            .filter(|c| {
                c.duration_ms == 0
                    || (c.duration_ms as i64 - target_duration_ms as i64).abs() <= 8_000
            })
            .min_by_key(|c| {
                if c.duration_ms == 0 {
                    5_000
                } else {
                    (c.duration_ms as i64 - target_duration_ms as i64).unsigned_abs()
                }
            })
    }

    /// Fetch syllable-synced lyrics for a track across configured LyricsPlus mirrors.
    /// Returns `Ok(None)` cleanly if no mirror has the track (allowing fallback to LRCLIB).
    pub async fn fetch(&self, track: &Track) -> Result<Option<LyricsData>, CoreError> {
        let primary_artist = track
            .artists
            .first()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown Artist");
        let full_artist = track.artist_display();
        let cleaned_title = clean_track_title(&track.title);
        let duration_s = (track.duration_ms > 0)
            .then_some((track.duration_ms as f64 / 1000.0).round() as u64);

        let mut last_err: Option<CoreError> = None;
        let mut best_line_only: Option<LyricsData> = None;

        for base in &self.base_urls {
            // Attempt 1: exact track metadata (title + primary artist + duration + album).
            let mut attempt1_had_album = track.album.as_deref().is_some_and(|a| !a.is_empty());
            match self
                .query_v2(
                    base,
                    &track.title,
                    primary_artist,
                    track.album.as_deref(),
                    duration_s,
                )
                .await
            {
                Ok(Some(data)) if data.has_native_syllables() => return Ok(Some(data)),
                Ok(Some(line_only)) => {
                    if best_line_only.is_none() {
                        best_line_only = Some(line_only);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    last_err = Some(err);
                    continue;
                }
            }

            // Attempt 2: cleaned title + WITHOUT album (important: Apple Music frequently maps
            // album-pinned requests to an older line-only edition while album-less lookup returns
            // the full per-syllable Word/TTML release, e.g. HiVi! - Pelangi).
            if cleaned_title != track.title
                || attempt1_had_album
                || primary_artist != full_artist
            {
                attempt1_had_album = false;
                match self
                    .query_v2(base, &cleaned_title, primary_artist, None, duration_s)
                    .await
                {
                    Ok(Some(data)) if data.has_native_syllables() => return Ok(Some(data)),
                    Ok(Some(line_only)) if best_line_only.is_none() => {
                        best_line_only = Some(line_only);
                    }
                    _ => {}
                }
            }
            let _ = attempt1_had_album;

            // Attempt 3: catalog fuzzy search (`/v1/songlist/search`) to resolve punctuation/edition
            // mismatches and locate a syllable-synced release when earlier attempts were missing or line-only.
            let search_q = format!("{cleaned_title} {primary_artist}");
            if let Some(cand) = self
                .search_songlist(base, &search_q, track.duration_ms)
                .await
            {
                let cand_dur = (cand.duration_ms > 0)
                    .then_some((cand.duration_ms as f64 / 1000.0).round() as u64)
                    .or(duration_s);
                let cand_album = (!cand.album.trim().is_empty()).then_some(cand.album.as_str());
                match self
                    .query_v2(base, &cand.title, &cand.artist, cand_album, cand_dur)
                    .await
                {
                    Ok(Some(data)) if data.has_native_syllables() => return Ok(Some(data)),
                    Ok(Some(line_only)) if best_line_only.is_none() => {
                        best_line_only = Some(line_only);
                    }
                    _ => {}
                }
            }

            if best_line_only.is_some() {
                return Ok(best_line_only);
            }
        }

        if best_line_only.is_some() {
            return Ok(best_line_only);
        }

        if let Some(err) = last_err {
            Err(err)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_apple_style_syllables_with_part_of_word_and_duets() {
        let json = r#"{
            "type": "Word",
            "metadata": { "source": "Apple" },
            "lyrics": [
                {
                    "time": 14483,
                    "duration": 4165,
                    "text": "Kau datang di saat yang tepat",
                    "syllabus": [
                        {"time": 14483, "duration": 290, "text": "Kau "},
                        {"time": 14773, "duration": 441, "text": "da"},
                        {"time": 15214, "duration": 187, "text": "tang "},
                        {"time": 17399, "duration": 210, "text": "te"},
                        {"time": 17609, "duration": 1039, "text": "pat"}
                    ],
                    "element": { "singer": "v1" }
                },
                {
                    "time": 20000,
                    "duration": 2000,
                    "text": "Ooh",
                    "syllabus": [
                        {"time": 20000, "duration": 900, "text": "Ooh"},
                        {"time": 20200, "duration": 800, "text": "(yeah)", "isBackground": true}
                    ],
                    "element": { "singer": "v2" }
                }
            ]
        }"#;

        let data = parse_lyricsplus_json(json).unwrap().unwrap();
        assert_eq!(data.provider, "apple");
        assert!(data.has_native_syllables());
        assert_eq!(data.synced.len(), 3);

        let l0 = &data.synced[0];
        assert_eq!(l0.text, "Kau datang tepat");
        assert!(!l0.opposite_aligned);
        let words = l0.words.as_ref().unwrap();
        assert_eq!(words.len(), 5);
        assert!(!words[0].is_part_of_word); // "Kau "
        assert!(words[1].is_part_of_word); // "da"
        assert!(!words[2].is_part_of_word); // "tang "
        assert!(words[3].is_part_of_word); // "te"
        assert!(!words[4].is_part_of_word); // "pat" (last syllable in line)
        assert_eq!(words[4].end_ms - words[4].start_ms, 1039);

        let l1 = &data.synced[1];
        assert!(l1.opposite_aligned);
        assert!(!l1.is_background);

        let l2 = &data.synced[2];
        assert!(l2.opposite_aligned);
        assert!(l2.is_background);
        assert_eq!(l2.text, "(yeah)");
    }

    #[test]
    fn handles_qq_music_separate_whitespace_tokens_cleanly() {
        let json = r#"{
            "type": "Word",
            "metadata": { "source": "QQ Music" },
            "lyrics": [
                {
                    "time": 1000,
                    "duration": 2000,
                    "text": "Hello world",
                    "syllabus": [
                        {"time": 1000, "duration": 300, "text": "Hel"},
                        {"time": 1300, "duration": 300, "text": "lo"},
                        {"time": 1600, "duration": 100, "text": " "},
                        {"time": 1700, "duration": 500, "text": "world"}
                    ]
                }
            ]
        }"#;

        let data = parse_lyricsplus_json(json).unwrap().unwrap();
        assert_eq!(data.provider, "qq");
        let l0 = &data.synced[0];
        assert_eq!(l0.text, "Hello world");
        let words = l0.words.as_ref().unwrap();
        assert_eq!(words.len(), 3);
        assert!(words[0].is_part_of_word); // "Hel" -> "lo"
        assert!(!words[1].is_part_of_word); // "lo" followed by " "
        assert!(!words[2].is_part_of_word); // "world"
    }

    #[tokio::test]
    async fn retries_without_album_when_album_lookup_returns_line_only() {
        use lume_core::TrackSource;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server = tokio::spawn(async move {
            for _ in 0..2 {
                let (mut sock, _) = listener.accept().await.unwrap();
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).await.unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);
                let body = if req.contains("album=") {
                    r#"{"type":"Line","metadata":{"source":"Apple"},"lyrics":[{"time":8453,"duration":6792,"text":"Kuingin cinta"}]}"#
                } else {
                    r#"{"type":"Word","metadata":{"source":"Apple"},"lyrics":[{"time":9725,"duration":6946,"text":"Kuingin cinta","syllabus":[{"time":9725,"duration":656,"text":"Ku"},{"time":10381,"duration":674,"text":"i"},{"time":11055,"duration":568,"text":"ngin "},{"time":11623,"duration":638,"text":"cin"},{"time":12261,"duration":576,"text":"ta"}]}]}"#
                };
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                sock.write_all(resp.as_bytes()).await.unwrap();
            }
        });

        let client = LyricsPlusClient::new(vec![format!("http://{addr}")]);
        let track = Track {
            uid: "spotify:pelangi".into(),
            source: TrackSource::Spotify {
                id: "pelangi".into(),
            },
            title: "Pelangi".into(),
            artists: vec!["Hivi!".into()],
            album: Some("Kereta Kencan".into()),
            duration_ms: 217_626,
            artwork_ref: None,
            is_liked: false,
        };

        let fetched = client.fetch(&track).await.unwrap().unwrap();
        assert_eq!(fetched.provider, "apple");
        assert!(
            fetched.has_native_syllables(),
            "must upgrade from line-only album response to album-less native syllables"
        );
        server.await.unwrap();
    }

    #[tokio::test]
    #[ignore = "live network lyricsplus check"]
    async fn live_fetches_apple_syllables_for_hivi_pelangi() {
        use lume_core::TrackSource;

        let client = LyricsPlusClient::default_public();
        let track = Track {
            uid: "spotify:3ZwqWrRpylU9nfy99qwKdR".into(),
            source: TrackSource::Spotify {
                id: "3ZwqWrRpylU9nfy99qwKdR".into(),
            },
            title: "Pelangi".into(),
            artists: vec!["Hivi!".into()],
            album: Some("Kereta Kencan".into()),
            duration_ms: 217_626,
            artwork_ref: None,
            is_liked: false,
        };

        let fetched = client.fetch(&track).await.unwrap().unwrap();
        println!(
            "Pelangi lyrics => provider={}, lines={}, has_native_syllables={}",
            fetched.provider,
            fetched.synced.len(),
            fetched.has_native_syllables()
        );
        assert_eq!(fetched.provider, "apple");
        assert!(fetched.has_native_syllables());
    }
}
