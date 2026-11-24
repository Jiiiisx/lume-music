use lume_core::{LyricsData, LyricsLine, LyricsWord};

/// Parse a TTML timestamp (`HH:MM:SS.mmm`, `MM:SS.mmm`, `SS.mmm`, or `12.5s`) into milliseconds.
pub fn parse_ttml_time_ms(raw: &str) -> Option<u64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(secs_str) = trimmed.strip_suffix('s') {
        let secs: f64 = secs_str.trim().parse().ok()?;
        if secs < 0.0 {
            return None;
        }
        return Some((secs * 1000.0).round() as u64);
    }

    let parts: Vec<&str> = trimmed.split(':').collect();
    match parts.as_slice() {
        [secs] => {
            let s: f64 = secs.parse().ok()?;
            (s >= 0.0).then_some((s * 1000.0).round() as u64)
        }
        [mins, secs] => {
            let m: u64 = mins.parse().ok()?;
            let s: f64 = secs.parse().ok()?;
            (s >= 0.0).then_some(m * 60_000 + (s * 1000.0).round() as u64)
        }
        [hours, mins, secs] => {
            let h: u64 = hours.parse().ok()?;
            let m: u64 = mins.parse().ok()?;
            let s: f64 = secs.parse().ok()?;
            (s >= 0.0).then_some(h * 3_600_000 + m * 60_000 + (s * 1000.0).round() as u64)
        }
        _ => None,
    }
}

fn unescape_xml(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
}

fn extract_attr(attrs: &str, name: &str) -> Option<String> {
    let needle = format!("{name}=");
    let idx = attrs.find(&needle)?;
    let rest = &attrs[idx + needle.len()..];
    let mut chars = rest.chars();
    let quote = chars.next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let inner = &rest[1..];
    let end = inner.find(quote)?;
    Some(inner[..end].to_string())
}

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Parse Apple Music / Spicy Lyrics `.ttml` XML (both line-synced `<p>` and word/syllable-synced
/// `<span begin="..." end="...">`) into `LyricsData`.
pub fn parse_ttml(xml: &str, provider: &str) -> LyricsData {
    let mut synced = Vec::new();
    let mut plain_lines = Vec::new();
    let mut cursor = 0usize;

    while let Some(rel_p) = xml[cursor..].find("<p") {
        let p_start = cursor + rel_p;
        // Guard against `<param` etc.
        let after_p = xml.as_bytes().get(p_start + 2).copied().unwrap_or(b' ');
        if !after_p.is_ascii_whitespace() && after_p != b'>' {
            cursor = p_start + 2;
            continue;
        }
        let Some(rel_gt) = xml[p_start..].find('>') else {
            break;
        };
        let open_end = p_start + rel_gt;
        let p_attrs = &xml[p_start + 2..open_end];
        let Some(rel_close) = xml[open_end + 1..].find("</p>") else {
            break;
        };
        let p_inner = &xml[open_end + 1..open_end + 1 + rel_close];
        cursor = open_end + 1 + rel_close + 4;

        let p_begin = extract_attr(p_attrs, "begin").and_then(|v| parse_ttml_time_ms(&v));
        let p_end = extract_attr(p_attrs, "end").and_then(|v| parse_ttml_time_ms(&v));
        let p_agent = extract_attr(p_attrs, "ttm:agent")
            .or_else(|| extract_attr(p_attrs, "agent"))
            .unwrap_or_default();
        let opposite_aligned = matches!(p_agent.trim(), "v2" | "v3" | "v4");
        let p_is_bg = extract_attr(p_attrs, "ttm:role")
            .is_some_and(|r| r.eq_ignore_ascii_case("x-bg"));

        // Parse nested `<span begin="..." end="...">text</span>` syllable/word elements.
        let mut words: Vec<LyricsWord> = Vec::new();
        let mut s_cursor = 0usize;
        while let Some(rel_s) = p_inner[s_cursor..].find("<span") {
            let gap_before = &p_inner[s_cursor..s_cursor + rel_s];
            let has_space_before = gap_before.chars().any(|c| c.is_whitespace());
            if !words.is_empty() && !has_space_before {
                if let Some(prev) = words.last_mut() {
                    prev.is_part_of_word = true;
                }
            }

            let s_start = s_cursor + rel_s;
            let Some(rel_s_gt) = p_inner[s_start..].find('>') else {
                break;
            };
            let s_open_end = s_start + rel_s_gt;
            let s_attrs = &p_inner[s_start + 5..s_open_end];
            let Some(rel_s_close) = p_inner[s_open_end + 1..].find("</span>") else {
                break;
            };
            let s_body = &p_inner[s_open_end + 1..s_open_end + 1 + rel_s_close];
            s_cursor = s_open_end + 1 + rel_s_close + 7;

            let w_start = extract_attr(s_attrs, "begin").and_then(|v| parse_ttml_time_ms(&v));
            let w_end = extract_attr(s_attrs, "end").and_then(|v| parse_ttml_time_ms(&v));
            let is_bg_span = p_is_bg
                || extract_attr(s_attrs, "ttm:role")
                    .is_some_and(|r| r.eq_ignore_ascii_case("x-bg"));
            let raw_text = unescape_xml(&strip_tags(s_body));
            let text = raw_text.trim().to_string();
            if let (Some(start_ms), Some(end_ms)) = (w_start, w_end) {
                if !text.is_empty() {
                    words.push(LyricsWord {
                        start_ms,
                        end_ms: end_ms.max(start_ms + 1),
                        text,
                        is_part_of_word: false,
                        is_background: is_bg_span,
                    });
                }
            }
        }

        let line_text = if !words.is_empty() {
            let mut joined = String::new();
            for (idx, w) in words.iter().enumerate() {
                joined.push_str(&w.text);
                if idx + 1 < words.len() && !w.is_part_of_word {
                    joined.push(' ');
                }
            }
            joined
        } else {
            unescape_xml(&strip_tags(p_inner))
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        };

        if line_text.is_empty() {
            continue;
        }
        plain_lines.push(line_text.clone());

        let time_ms = p_begin.or_else(|| words.first().map(|w| w.start_ms));
        if let Some(time_ms) = time_ms {
            let end_ms = p_end.or_else(|| words.last().map(|w| w.end_ms));
            synced.push(LyricsLine {
                time_ms,
                end_ms,
                text: line_text,
                words: (!words.is_empty()).then_some(words),
                opposite_aligned,
                is_background: p_is_bg,
            });
        }
    }

    synced.sort_by_key(|l| l.time_ms);

    LyricsData {
        synced,
        plain: (!plain_lines.is_empty()).then(|| plain_lines.join("\n")),
        provider: provider.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ttml_word_and_syllable_timestamps() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:49.500" end="00:53.200">
        <span begin="00:49.500" end="00:50.100">And</span>
        <span begin="00:50.100" end="00:50.600">I</span>
        <span begin="00:50.600" end="00:51.400">miss</span>
        <span begin="00:51.400" end="00:52.000">my</span>
        <span begin="00:52.000" end="00:53.200">lover, man</span>
      </p>
      <p begin="1:02.000" end="1:05.500">I&apos;m Mr. Loverman</p>
    </div>
  </body>
</tt>"#;

        let parsed = parse_ttml(xml, "sidecar_ttml");
        assert_eq!(parsed.synced.len(), 2);
        assert_eq!(parsed.synced[0].time_ms, 49_500);
        assert_eq!(parsed.synced[0].text, "And I miss my lover, man");
        let words = parsed.synced[0].words.as_ref().expect("word timings");
        assert_eq!(words.len(), 5);
        assert_eq!(words[0].text, "And");
        assert_eq!(words[0].start_ms, 49_500);
        assert_eq!(words[0].end_ms, 50_100);
        assert_eq!(parsed.synced[1].time_ms, 62_000);
        assert_eq!(parsed.synced[1].text, "I'm Mr. Loverman");
        assert!(parsed.synced[1].words.is_none());
    }

    #[test]
    fn parses_ttml_time_formats() {
        assert_eq!(parse_ttml_time_ms("01:02:03.450"), Some(3_723_450));
        assert_eq!(parse_ttml_time_ms("02:15.5"), Some(135_500));
        assert_eq!(parse_ttml_time_ms("12.345s"), Some(12_345));
        assert_eq!(parse_ttml_time_ms("bad"), None);
    }
}
