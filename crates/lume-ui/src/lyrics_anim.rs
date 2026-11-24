use lume_core::{LyricsLine, LyricsWord};
use lume_storage::LyricsVisualMode;

/// Visual depth styling computed for each lyric line relative to the active line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineStyle {
    pub scale: f32,
    pub opacity: f32,
    pub glow: f32,
}

/// Critically-damped spring scroll state for smooth synced lyrics centering.
#[derive(Debug, Clone, Copy)]
pub struct LyricsSpringState {
    offset_y: f32,
    target_y: f32,
    velocity: f32,
}

impl Default for LyricsSpringState {
    fn default() -> Self {
        Self::new()
    }
}

impl LyricsSpringState {
    pub fn new() -> Self {
        Self {
            offset_y: 0.0,
            target_y: 0.0,
            velocity: 0.0,
        }
    }

    pub fn update(&mut self, dt_s: f32, target_y: f32) {
        self.target_y = target_y;
        let dt = dt_s.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return;
        }
        let stiffness: f32 = 120.0;
        let damping: f32 = 2.0 * stiffness.sqrt();
        let accel = stiffness * (target_y - self.offset_y) - damping * self.velocity;
        self.velocity += accel * dt;
        self.offset_y += self.velocity * dt;
    }

    pub fn snap_to(&mut self, y: f32) {
        self.offset_y = y;
        self.target_y = y;
        self.velocity = 0.0;
    }

    pub fn scroll_y(&self) -> f32 {
        self.offset_y
    }

    pub fn is_settled(&self) -> bool {
        (self.offset_y - self.target_y).abs() < 0.5 && self.velocity.abs() < 1.0
    }
}

pub fn active_line_index(lines: &[LyricsLine], pos_ms: u64) -> Option<usize> {
    if lines.is_empty() {
        return None;
    }
    let next_idx = lines.partition_point(|line| line.time_ms <= pos_ms);
    if next_idx == 0 {
        None
    } else {
        Some(next_idx - 1)
    }
}

pub fn line_visual_style(distance_from_active: isize, mode: LyricsVisualMode) -> LineStyle {
    let dist = distance_from_active.unsigned_abs();
    match mode {
        LyricsVisualMode::SpicySmooth => match dist {
            0 => LineStyle {
                scale: 1.08,
                opacity: 1.0,
                glow: 0.85,
            },
            1 => LineStyle {
                scale: 0.94,
                opacity: 0.36,
                glow: 0.0,
            },
            2 => LineStyle {
                scale: 0.90,
                opacity: 0.24,
                glow: 0.0,
            },
            3 => LineStyle {
                scale: 0.88,
                opacity: 0.16,
                glow: 0.0,
            },
            _ => LineStyle {
                scale: 0.86,
                opacity: 0.10,
                glow: 0.0,
            },
        },
        LyricsVisualMode::UltraLight => match dist {
            0 => LineStyle {
                scale: 1.0,
                opacity: 1.0,
                glow: 0.0,
            },
            1 => LineStyle {
                scale: 1.0,
                opacity: 0.55,
                glow: 0.0,
            },
            _ => LineStyle {
                scale: 1.0,
                opacity: 0.35,
                glow: 0.0,
            },
        },
    }
}

pub fn word_progress(word: &LyricsWord, pos_ms: u64) -> f32 {
    if pos_ms <= word.start_ms {
        return 0.0;
    }
    if pos_ms >= word.end_ms || word.end_ms <= word.start_ms {
        return 1.0;
    }
    let span = (word.end_ms - word.start_ms) as f32;
    ((pos_ms - word.start_ms) as f32 / span).clamp(0.0, 1.0)
}

/// Return real enhanced-LRC word timings if present, or synthesize smooth proportional
/// per-word timings across the line's vocal window so line-synced tracks (e.g. LRCLIB)
/// still deliver Spicy-Lyrics-style word-by-word karaoke highlighting.
pub fn resolve_line_words(
    lines: &[LyricsLine],
    idx: usize,
    track_duration_ms: u64,
) -> Vec<LyricsWord> {
    let Some(line) = lines.get(idx) else {
        return Vec::new();
    };
    if let Some(words) = &line.words {
        if !words.is_empty() {
            return words.clone();
        }
    }

    let tokens: Vec<&str> = line.text.split_whitespace().collect();
    if tokens.is_empty() {
        return Vec::new();
    }

    let start_ms = line.time_ms;
    let next_ms = lines
        .get(idx + 1)
        .map(|l| l.time_ms)
        .filter(|&t| t > start_ms)
        .unwrap_or_else(|| {
            let fallback = start_ms.saturating_add((tokens.len() as u64 * 480).clamp(1_600, 5_000));
            if track_duration_ms > start_ms {
                track_duration_ms.min(fallback)
            } else {
                fallback
            }
        });

    let raw_span = next_ms.saturating_sub(start_ms).max(400);
    // Cap synthesized vocal duration so long instrumental breaks don't stretch short lines.
    let max_vocal_ms = (tokens.len() as u64 * 580).clamp(1_400, 6_500);
    let vocal_span = ((raw_span as f32 * 0.88) as u64)
        .min(max_vocal_ms)
        .max(raw_span.min(400));

    let weights: Vec<f32> = tokens
        .iter()
        .map(|w| (w.chars().count().max(2) as f32) + 1.0)
        .collect();
    let total_weight: f32 = weights.iter().sum::<f32>().max(1.0);

    let mut out = Vec::with_capacity(tokens.len());
    let mut cursor = start_ms as f32;
    for (i, (tok, w)) in tokens.into_iter().zip(weights).enumerate() {
        let dur = (vocal_span as f32) * (w / total_weight);
        let w_start = cursor.round() as u64;
        cursor += dur;
        let w_end = if i + 1 == out.capacity() {
            start_ms.saturating_add(vocal_span).max(w_start + 1)
        } else {
            (cursor.round() as u64).max(w_start + 1)
        };
        out.push(LyricsWord {
            start_ms: w_start,
            end_ms: w_end,
            text: tok.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges_to_target_and_marks_settled() {
        let mut spring = LyricsSpringState::new();
        for _ in 0..180 {
            spring.update(1.0 / 60.0, 240.0);
        }
        assert!((spring.scroll_y() - 240.0).abs() < 0.5);
        assert!(spring.is_settled());
    }

    #[test]
    fn active_line_index_handles_pre_start_mid_song_and_end() {
        let lines = vec![
            LyricsLine {
                time_ms: 5_000,
                text: "One".into(),
                words: None,
            },
            LyricsLine {
                time_ms: 12_000,
                text: "Two".into(),
                words: None,
            },
            LyricsLine {
                time_ms: 20_000,
                text: "Three".into(),
                words: None,
            },
        ];

        assert_eq!(active_line_index(&[], 1_000), None);
        assert_eq!(active_line_index(&lines, 2_000), None);
        assert_eq!(active_line_index(&lines, 5_000), Some(0));
        assert_eq!(active_line_index(&lines, 15_000), Some(1));
        assert_eq!(active_line_index(&lines, 99_000), Some(2));
    }

    #[test]
    fn depth_opacity_and_scale_decrease_monotonically_from_active_line() {
        let s0 = line_visual_style(0, LyricsVisualMode::SpicySmooth);
        let s1 = line_visual_style(1, LyricsVisualMode::SpicySmooth);
        let s2 = line_visual_style(2, LyricsVisualMode::SpicySmooth);
        let s4 = line_visual_style(-4, LyricsVisualMode::SpicySmooth);

        assert!(s0.opacity > s1.opacity && s1.opacity > s2.opacity && s2.opacity >= s4.opacity);
        assert!(s0.scale > s1.scale && s1.scale >= s2.scale && s2.scale >= s4.scale);
        assert!(s0.glow > s1.glow);
    }

    #[test]
    fn word_progress_clamps_between_zero_and_one() {
        let word = LyricsWord {
            start_ms: 1000,
            end_ms: 2000,
            text: "Hello".into(),
        };
        assert_eq!(word_progress(&word, 500), 0.0);
        assert!((word_progress(&word, 1500) - 0.5).abs() < 0.001);
        assert_eq!(word_progress(&word, 2500), 1.0);
    }

    #[test]
    fn resolve_line_words_synthesizes_monotonic_spans_and_caps_long_breaks() {
        let lines = vec![
            LyricsLine {
                time_ms: 10_000,
                text: "And I miss my lover, man".into(),
                words: None,
            },
            LyricsLine {
                time_ms: 40_000, // 30s instrumental gap
                text: "Next line".into(),
                words: None,
            },
        ];
        let words = resolve_line_words(&lines, 0, 217_000);
        assert_eq!(words.len(), 6);
        assert_eq!(words[0].start_ms, 10_000);
        assert_eq!(words[0].text, "And");
        assert_eq!(words[5].text, "man");
        assert!(words[5].end_ms <= 16_500, "should cap long instrumental break");
        for pair in words.windows(2) {
            assert!(pair[0].end_ms <= pair[1].end_ms);
            assert!(pair[0].start_ms < pair[0].end_ms);
        }
    }
}
