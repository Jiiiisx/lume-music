use std::time::Duration;

use base64::Engine;
use lume_core::{CoreError, Track, TrackSource};
use serde_json::Value;
use url::Url;

/// Parsed result of importing a public Spotify playlist URL via its embed payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedSpotifyPlaylist {
    pub spotify_id: String,
    pub source_url: String,
    pub name: String,
    pub tracks: Vec<Track>,
}

/// Extracts a Spotify playlist ID from `https://open.spotify.com/playlist/{id}` or `spotify:playlist:{id}`.
pub fn extract_spotify_playlist_id(input: &str) -> Result<String, CoreError> {
    let trimmed = input.trim();
    if let Some(rest) = trimmed.strip_prefix("spotify:playlist:") {
        let id = rest.split(':').next().unwrap_or_default().trim();
        if is_valid_spotify_id(id) {
            return Ok(id.to_string());
        }
    }

    let url = Url::parse(trimmed).map_err(|_| {
        CoreError::InvalidInput(
            "Enter a valid https://open.spotify.com/playlist/... URL.".into(),
        )
    })?;

    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if !host.ends_with("spotify.com") {
        return Err(CoreError::InvalidInput(
            "URL must be a Spotify playlist link (open.spotify.com/playlist/...).".into(),
        ));
    }

    let segments: Vec<&str> = url
        .path_segments()
        .map(|s| s.filter(|p| !p.is_empty()).collect())
        .unwrap_or_default();

    for (idx, seg) in segments.iter().enumerate() {
        if *seg == "playlist" {
            if let Some(id) = segments.get(idx + 1) {
                if is_valid_spotify_id(id) {
                    return Ok((*id).to_string());
                }
            }
        }
    }

    Err(CoreError::InvalidInput(
        "Could not find a Spotify playlist ID in that URL.".into(),
    ))
}

fn is_valid_spotify_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Canonical public playlist URL used as the stable dedup key in SQLite.
pub fn canonical_playlist_url(playlist_id: &str) -> String {
    format!("https://open.spotify.com/playlist/{playlist_id}")
}

/// Fetches a public Spotify playlist via `https://open.spotify.com/embed/playlist/{id}`
/// without requiring any Spotify Web API login or Developer Mode followed-playlist permissions.
pub async fn import_spotify_playlist_url(url: &str) -> Result<ImportedSpotifyPlaylist, CoreError> {
    let playlist_id = extract_spotify_playlist_id(url)?;
    let embed_url = format!("https://open.spotify.com/embed/playlist/{playlist_id}");
    import_spotify_playlist_from_embed_url(&playlist_id, &embed_url).await
}

pub async fn import_spotify_playlist_from_embed_url(
    playlist_id: &str,
    embed_url: &str,
) -> Result<ImportedSpotifyPlaylist, CoreError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36")
        .build()
        .map_err(|e| CoreError::Network(e.to_string()))?;

    let resp = client
        .get(embed_url)
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("failed to fetch Spotify embed page: {e}")))?;

    if !resp.status().is_success() {
        return Err(CoreError::ProviderUnavailable(format!(
            "Spotify returned HTTP {} for playlist {}. Ensure the playlist is public.",
            resp.status(),
            playlist_id
        )));
    }

    let html = resp
        .text()
        .await
        .map_err(|e| CoreError::Network(format!("failed to read Spotify embed HTML: {e}")))?;

    parse_spotify_embed_html(&html, playlist_id)
}

/// Parses Spotify's embed HTML (`__NEXT_DATA__` or base64 `initial-state`) into tracks.
pub fn parse_spotify_embed_html(
    html: &str,
    playlist_id: &str,
) -> Result<ImportedSpotifyPlaylist, CoreError> {
    let json_val = extract_next_data_json(html)
        .or_else(|| extract_initial_state_json(html))
        .ok_or_else(|| {
            CoreError::ProviderUnavailable(
                "Could not locate playlist tracklist in Spotify embed page (playlist may be private)."
                    .into(),
            )
        })?;

    let entity = find_entity_object(&json_val).ok_or_else(|| {
        CoreError::ProviderUnavailable("Spotify embed JSON did not contain playlist data.".into())
    })?;

    let name = entity
        .get("name")
        .or_else(|| entity.get("title"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("Imported Spotify Playlist")
        .to_string();

    let playlist_cover = extract_best_image_url(entity);

    let track_list = entity
        .get("trackList")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CoreError::ProviderUnavailable(
                "Spotify playlist appears empty or private (no trackList found).".into(),
            )
        })?;

    let mut tracks = Vec::with_capacity(track_list.len());
    for item in track_list {
        let Some(track_id) = extract_track_id(item) else {
            continue;
        };
        let title = item
            .get("title")
            .or_else(|| item.get("name"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Unknown Title")
            .to_string();

        let subtitle = item
            .get("subtitle")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let artists: Vec<String> = subtitle
            .split(',')
            .map(|s| s.trim_matches(|c: char| c.is_whitespace() || c == '\u{a0}'))
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect();
        let artists = if artists.is_empty() {
            vec!["Unknown Artist".to_string()]
        } else {
            artists
        };

        let duration_ms = item
            .get("duration")
            .or_else(|| item.get("duration_ms"))
            .and_then(Value::as_u64)
            .unwrap_or(0);

        let artwork_ref = extract_best_image_url(item).or_else(|| playlist_cover.clone());
        let source = TrackSource::Spotify { id: track_id };
        tracks.push(Track {
            uid: source.canonical_uid(),
            source,
            title,
            artists,
            album: None,
            duration_ms,
            artwork_ref,
            is_liked: false,
        });
    }

    if tracks.is_empty() {
        return Err(CoreError::ProviderUnavailable(
            "No playable Spotify tracks found in that playlist.".into(),
        ));
    }

    Ok(ImportedSpotifyPlaylist {
        spotify_id: playlist_id.to_string(),
        source_url: canonical_playlist_url(playlist_id),
        name,
        tracks,
    })
}

fn extract_next_data_json(html: &str) -> Option<Value> {
    let marker = "id=\"__NEXT_DATA__\"";
    let start = html.find(marker)?;
    let after_tag = html[start..].find('>')? + start + 1;
    let end = html[after_tag..].find("</script>")? + after_tag;
    let raw = html[after_tag..end].trim();
    serde_json::from_str(raw).ok()
}

fn extract_initial_state_json(html: &str) -> Option<Value> {
    let marker = "id=\"initial-state\"";
    let start = html.find(marker)?;
    let after_tag = html[start..].find('>')? + start + 1;
    let end = html[after_tag..].find("</script>")? + after_tag;
    let raw = html[after_tag..end].trim();
    if raw.starts_with('{') {
        return serde_json::from_str(raw).ok();
    }
    let decoded = base64::engine::general_purpose::STANDARD.decode(raw).ok()?;
    serde_json::from_slice(&decoded).ok()
}

fn find_entity_object(root: &Value) -> Option<&Value> {
    root.pointer("/props/pageProps/state/data/entity")
        .or_else(|| root.pointer("/data/entity"))
        .or_else(|| root.get("entity"))
}

fn extract_track_id(item: &Value) -> Option<String> {
    if let Some(uri) = item.get("uri").and_then(Value::as_str) {
        if let Some(id) = uri.strip_prefix("spotify:track:") {
            let trimmed = id.trim();
            if is_valid_spotify_id(trimmed) {
                return Some(trimmed.to_string());
            }
        }
    }
    if let Some(id) = item.get("id").and_then(Value::as_str) {
        let trimmed = id.trim();
        if is_valid_spotify_id(trimmed) {
            return Some(trimmed.to_string());
        }
    }
    None
}

fn extract_best_image_url(obj: &Value) -> Option<String> {
    let candidates = obj
        .pointer("/visualIdentity/image")
        .or_else(|| obj.pointer("/coverArt/sources"))
        .or_else(|| obj.get("images"))
        .and_then(Value::as_array)?;

    candidates
        .iter()
        .filter_map(|img| {
            let url = img.get("url").and_then(Value::as_str)?.trim();
            if url.is_empty() {
                return None;
            }
            let w = img
                .get("maxWidth")
                .or_else(|| img.get("width"))
                .and_then(Value::as_u64)
                .unwrap_or(0);
            Some((w, url.to_string()))
        })
        .max_by_key(|(w, _)| *w)
        .map(|(_, url)| url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_playlist_id_from_web_embed_and_uri_formats() {
        assert_eq!(
            extract_spotify_playlist_id(
                "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=abcdef123"
            )
            .unwrap(),
            "37i9dQZF1DXcBWIGoYBM5M"
        );
        assert_eq!(
            extract_spotify_playlist_id(
                "https://open.spotify.com/embed/playlist/37i9dQZF1DXcBWIGoYBM5M"
            )
            .unwrap(),
            "37i9dQZF1DXcBWIGoYBM5M"
        );
        assert_eq!(
            extract_spotify_playlist_id("spotify:playlist:37i9dQZF1DXcBWIGoYBM5M").unwrap(),
            "37i9dQZF1DXcBWIGoYBM5M"
        );
        assert!(extract_spotify_playlist_id("https://example.com/playlist/123").is_err());
        assert!(extract_spotify_playlist_id("https://open.spotify.com/track/123").is_err());
    }

    #[test]
    fn parses_spotify_embed_next_data_into_spotify_tracks() {
        let html = r#"<!doctype html><html><body><script id="__NEXT_DATA__" type="application/json">{
            "props": {
                "pageProps": {
                    "state": {
                        "data": {
                            "entity": {
                                "name": "Late Night Synth",
                                "visualIdentity": {
                                    "image": [
                                        {"url": "https://i.scdn.co/small.jpg", "maxWidth": 300},
                                        {"url": "https://i.scdn.co/large.jpg", "maxWidth": 640}
                                    ]
                                },
                                "trackList": [
                                    {
                                        "uri": "spotify:track:0VjIjW4GlUZAMYd2vXMi3b",
                                        "title": "Blinding Lights",
                                        "subtitle": "The Weeknd",
                                        "duration": 200040
                                    },
                                    {
                                        "uri": "spotify:track:4cOdK2wGLETKBW3PvgPWqT",
                                        "title": "Nightcall",
                                        "subtitle": "Kavinsky,\u00a0Lovefoxxx",
                                        "duration": 258413
                                    }
                                ]
                            }
                        }
                    }
                }
            }
        }</script></body></html>"#;

        let imported = parse_spotify_embed_html(html, "37i9dQZF1DXcBWIGoYBM5M").unwrap();
        assert_eq!(imported.name, "Late Night Synth");
        assert_eq!(
            imported.source_url,
            "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M"
        );
        assert_eq!(imported.tracks.len(), 2);
        assert_eq!(
            imported.tracks[0].uid,
            "spotify:0VjIjW4GlUZAMYd2vXMi3b"
        );
        assert_eq!(
            imported.tracks[0].artwork_ref.as_deref(),
            Some("https://i.scdn.co/large.jpg")
        );
        assert_eq!(imported.tracks[1].artists, vec!["Kavinsky", "Lovefoxxx"]);
        assert_eq!(imported.tracks[1].duration_ms, 258_413);
    }
}
