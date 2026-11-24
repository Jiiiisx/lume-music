use lume_core::{LyricsData, LyricsLine, LyricsWord};

fn parse_timestamp_str(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    let (min_str, sec_part) = trimmed.split_once(':')?;
    if min_str.is_empty() || !min_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let minutes: u64 = min_str.parse().ok()?;

    let (seconds, frac_ms) = if let Some((sec_str, frac_str)) = sec_part.split_once('.') {
        if sec_str.is_empty()
            || !sec_str.chars().all(|c| c.is_ascii_digit())
            || frac_str.is_empty()
            || !frac_str.chars().all(|c| c.is_ascii_digit())
        {
            return None;
        }
        let sec: u64 = sec_str.parse().ok()?;
        let frac: u64 = match frac_str.len() {
            1 => frac_str.parse::<u64>().ok()? * 100,
            2 => frac_str.parse::<u64>().ok()? * 10,
            _ => frac_str[..3].parse::<u64>().ok()?,
        };
        (sec, frac)
    } else {
        if sec_part.is_empty() || !sec_part.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        let sec: u64 = sec_part.parse().ok()?;
        (sec, 0)
    };

    Some(minutes * 60_000 + seconds * 1_000 + frac_ms)
}

fn parse_inline_words(body: &str) -> (String, Option<Vec<LyricsWord>>) {
    let mut clean = String::with_capacity(body.len());
    let mut raw_words: Vec<(u64, String)> = Vec::new();
    let mut current_start: Option<u64> = None;
    let mut current_buf = String::new();
    let mut idx = 0;

    while idx < body.len() {
        let slice = &body[idx..];
        if slice.starts_with('<') {
            if let Some(close_rel) = slice.find('>') {
                let inner = &slice[1..close_rel];
                if let Some(ts) = parse_timestamp_str(inner) {
                    if let Some(prev_ts) = current_start.take() {
                        let w = current_buf.trim().to_string();
                        if !w.is_empty() {
                            raw_words.push((prev_ts, w));
                        }
                        current_buf.clear();
                    } else if !current_buf.is_empty() {
                        current_buf.clear();
                    }
                    current_start = Some(ts);
                    idx += close_rel + 1;
                    continue;
                }
            }
        }

        let Some(ch) = slice.chars().next() else {
            break;
        };
        clean.push(ch);
        if current_start.is_some() {
            current_buf.push(ch);
        }
        idx += ch.len_utf8();
    }

    if let Some(last_ts) = current_start {
        let w = current_buf.trim().to_string();
        if !w.is_empty() {
            raw_words.push((last_ts, w));
        }
    }

    let clean_trimmed = clean.trim().to_string();
    if raw_words.is_empty() {
        (clean_trimmed, None)
    } else {
        let words = raw_words
            .into_iter()
            .map(|(start_ms, text)| LyricsWord {
                start_ms,
                end_ms: start_ms,
                text,
                is_part_of_word: false,
                is_background: false,
            })
            .collect();
        (clean_trimmed, Some(words))
    }
}

/// Parses standard `[mm:ss.xx]` / `[mm:ss.xxx]` / `[mm:ss]` LRC and enhanced `<mm:ss.xx>` word-synced LRC.
pub fn parse_lrc(raw: &str, provider: &str) -> LyricsData {
    let cleaned_raw = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut synced: Vec<LyricsLine> = Vec::new();
    let mut plain_lines: Vec<String> = Vec::new();

    for raw_line in cleaned_raw.lines() {
        let mut rest = raw_line.trim();
        if rest.is_empty() {
            continue;
        }

        let mut timestamps = Vec::new();
        while rest.starts_with('[') {
            let Some(close_idx) = rest.find(']') else {
                break;
            };
            let inner = &rest[1..close_idx];
            if let Some(ts) = parse_timestamp_str(inner) {
                timestamps.push(ts);
                rest = &rest[close_idx + 1..];
            } else {
                break;
            }
        }

        if timestamps.is_empty() {
            if !rest.starts_with('[') {
                plain_lines.push(rest.to_string());
            }
            continue;
        }

        let (text, words) = parse_inline_words(rest.trim());
        if !text.is_empty() {
            plain_lines.push(text.clone());
        }

        for ts in timestamps {
            synced.push(LyricsLine {
                time_ms: ts,
                end_ms: None,
                text: text.clone(),
                words: words.clone(),
                opposite_aligned: false,
                is_background: false,
            });
        }
    }

    synced.sort_by_key(|line| line.time_ms);

    let line_times: Vec<u64> = synced.iter().map(|l| l.time_ms).collect();
    for (idx, line) in synced.iter_mut().enumerate() {
        let next_line_ms = line_times.get(idx + 1).copied();
        if let Some(words) = &mut line.words {
            let len = words.len();
            for w_idx in 0..len {
                let next_word_ms = (w_idx + 1 < len).then(|| words[w_idx + 1].start_ms);
                let end = next_word_ms
                    .or(next_line_ms)
                    .unwrap_or(words[w_idx].start_ms + 800);
                words[w_idx].end_ms = end.max(words[w_idx].start_ms);
            }
        }
    }

    let plain = if plain_lines.is_empty() {
        None
    } else {
        Some(plain_lines.join("\n"))
    };

    LyricsData {
        synced,
        plain,
        provider: provider.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_two_and_three_digit_ms_and_no_fraction_timestamps() {
        let raw = "[00:05]First\n[00:12.34]Second\n[01:02.345]Third";
        let data = parse_lrc(raw, "test");
        assert_eq!(data.synced.len(), 3);
        assert_eq!(data.synced[0].time_ms, 5_000);
        assert_eq!(data.synced[0].text, "First");
        assert_eq!(data.synced[1].time_ms, 12_340);
        assert_eq!(data.synced[1].text, "Second");
        assert_eq!(data.synced[2].time_ms, 62_345);
        assert_eq!(data.synced[2].text, "Third");
    }

    #[test]
    fn expands_repeated_timestamps_and_sorts_out_of_order_lines() {
        let raw = "[01:02.00][00:12.00]Chorus\n[00:05.00]Intro\n[00:30.00]";
        let data = parse_lrc(raw, "test");
        assert_eq!(data.synced.len(), 4);
        assert_eq!(data.synced[0].time_ms, 5_000);
        assert_eq!(data.synced[0].text, "Intro");
        assert_eq!(data.synced[1].time_ms, 12_000);
        assert_eq!(data.synced[1].text, "Chorus");
        assert_eq!(data.synced[2].time_ms, 30_000);
        assert_eq!(data.synced[2].text, "");
        assert_eq!(data.synced[3].time_ms, 62_000);
        assert_eq!(data.synced[3].text, "Chorus");
    }

    #[test]
    fn parses_enhanced_word_level_timestamps_into_lyrics_words() {
        let raw = "[00:10.00]<00:10.00>Hello <00:10.50>world\n[00:12.00]Next";
        let data = parse_lrc(raw, "enhanced");
        assert_eq!(data.synced.len(), 2);
        assert_eq!(data.synced[0].text, "Hello world");
        let words = data.synced[0].words.as_ref().unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Hello");
        assert_eq!(words[0].start_ms, 10_000);
        assert_eq!(words[0].end_ms, 10_500);
        assert_eq!(words[1].text, "world");
        assert_eq!(words[1].start_ms, 10_500);
        assert_eq!(words[1].end_ms, 12_000);
        assert!(data.synced[1].words.is_none());
    }

    #[test]
    fn ignores_malformed_brackets_without_panicking() {
        let raw = "[ar:Artist]\n[broken\n[99:xx]Invalid\n[00:02.50]Valid [unclosed\nplain text line";
        let data = parse_lrc(raw, "test");
        assert_eq!(data.synced.len(), 1);
        assert_eq!(data.synced[0].time_ms, 2_500);
        assert_eq!(data.synced[0].text, "Valid [unclosed");
    }
}
