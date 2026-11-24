use std::f32::consts::PI;

use lume_core::{LyricsLine, LyricsWord};
use lume_storage::LyricsVisualMode;

/// Minimum intro silence before the first lyric line to render an instrumental `• • •` interlude.
pub const INTRO_INTERLUDE_MIN_MS: u64 = 4_000;
/// Minimum mid-song gap between two lyric lines to render an instrumental `• • •` interlude.
pub const GAP_INTERLUDE_MIN_MS: u64 = 5_000;
/// Width (in character units) of Spicy Lyrics' Bloom Renderer feathered sweep edge (`--SLR_FadeWidth: 0.882em`).
pub const BLOOM_FADE_HALF_CHARS: f32 = 1.35;

/// Shared layout geometry constants kept identical between `lyrics_view` and scroll centering.
pub const LYRICS_ITEM_SPACING_PX: f32 = 20.0;
pub const LYRICS_INTERLUDE_HEIGHT_PX: f32 = 34.0;
pub const LYRICS_LINE_HEIGHT_MULT: f32 = 1.22;
pub const LYRICS_LINE_VPAD_PX: f32 = 4.0;

pub fn lyrics_viewport_metrics(window_w: f32, window_h: f32) -> (f32, f32) {
    let viewport_h = (window_h - 66.0).max(240.0);
    // Left stage (400) + row gap (20) + body padding (28) + lyrics horiz padding (64) + inner line padding (12)
    let text_max_w = (window_w - 524.0).max(220.0);
    (text_max_w, viewport_h)
}

pub fn lyrics_top_spacer_px(viewport_h: f32) -> f32 {
    (viewport_h * 0.36).clamp(110.0, 260.0)
}

pub fn lyrics_bottom_spacer_px(viewport_h: f32) -> f32 {
    (viewport_h * 0.54).clamp(240.0, 440.0)
}

fn char_width_ratio(ch: char) -> f32 {
    match ch {
        ' ' => 0.27,
        'i' | 'l' | 'I' | 'j' | '!' | '.' | ',' | ':' | ';' | '\'' | '|' => 0.31,
        'f' | 'r' | 't' | '(' | ')' | '-' => 0.39,
        'm' | 'w' | 'M' | 'W' => 0.86,
        c if c.is_ascii_uppercase() => 0.68,
        c if !c.is_ascii() => 0.85,
        _ => 0.56,
    }
}

/// Estimate wrapped visual line count for `Plus Jakarta Sans Bold` inside `max_width_px`.
pub fn estimate_wrapped_lines(text: &str, font_size: f32, max_width_px: f32) -> usize {
    let max_w = max_width_px.max(font_size * 4.0);
    let space_w = 0.27 * font_size;
    let mut lines = 1usize;
    let mut current_w = 0.0_f32;

    for word in text.split_whitespace() {
        let word_w: f32 = word.chars().map(|c| char_width_ratio(c) * font_size).sum();
        if current_w == 0.0 {
            if word_w > max_w {
                lines += (word_w / max_w).floor() as usize;
                current_w = word_w % max_w;
            } else {
                current_w = word_w;
            }
        } else if current_w + space_w + word_w <= max_w {
            current_w += space_w + word_w;
        } else {
            lines += 1;
            if word_w > max_w {
                lines += (word_w / max_w).floor() as usize;
                current_w = word_w % max_w;
            } else {
                current_w = word_w;
            }
        }
    }
    lines
}

/// Compute the exact scrollable Y offset that centers `active_idx` at ~42% of the lyrics viewport,
/// accounting for variable line heights, wrapped multi-line verses, and 34px `• • •` interludes.
pub fn target_scroll_y_for_item(
    timeline: &[LyricsDisplayItem],
    lines: &[LyricsLine],
    active_idx: usize,
    mode: LyricsVisualMode,
    window_w: f32,
    window_h: f32,
) -> f32 {
    if timeline.is_empty() || active_idx >= timeline.len() {
        return 0.0;
    }

    let (text_max_w, viewport_h) = lyrics_viewport_metrics(window_w, window_h);
    let top_spacer = lyrics_top_spacer_px(viewport_h);

    // Account for top spacer + spacing between top spacer and first item.
    let mut cursor_y = top_spacer + LYRICS_ITEM_SPACING_PX;
    let mut active_height = LYRICS_INTERLUDE_HEIGHT_PX;

    for (idx, item) in timeline.iter().enumerate() {
        let distance = idx as isize - active_idx as isize;
        let style = line_visual_style(distance, mode);
        let is_active = distance == 0;

        let item_h = match *item {
            LyricsDisplayItem::Interlude { .. } => LYRICS_INTERLUDE_HEIGHT_PX,
            LyricsDisplayItem::Line {
                line_idx,
                is_bg_vocal,
                ..
            } => {
                let font_size = if is_active {
                    let base = if is_bg_vocal { 26.0 } else { 36.0 };
                    if mode == LyricsVisualMode::SpicySmooth {
                        base * 1.045
                    } else {
                        base
                    }
                } else {
                    let base = if is_bg_vocal { 23.0 } else { 31.0 };
                    base * style.scale
                };
                let text = lines
                    .get(line_idx)
                    .map(|l| l.text.as_str())
                    .unwrap_or("");
                let wrapped = estimate_wrapped_lines(text, font_size, text_max_w) as f32;
                wrapped * (font_size * LYRICS_LINE_HEIGHT_MULT) + LYRICS_LINE_VPAD_PX
            }
        };

        if idx < active_idx {
            cursor_y += item_h + LYRICS_ITEM_SPACING_PX;
        } else if idx == active_idx {
            active_height = item_h;
            break;
        }
    }

    let item_center_y = cursor_y + active_height * 0.5;
    (item_center_y - viewport_h * 0.42).max(0.0)
}

/// Visual depth styling computed for each lyric line relative to the active line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineStyle {
    pub scale: f32,
    pub opacity: f32,
    pub glow: f32,
}

/// Closed-form analytical spring ported from Fraktality's `spr` (MIT), as used by Spikerko/spicy-lyrics.
/// Supports underdamped (`d < 1`), critically damped (`d == 1`), and overdamped (`d > 1`) motion
/// with exact sleep detection so Smart Render Pause hits 0 FPS once settled.
#[derive(Debug, Clone, Copy)]
pub struct SpicySpring {
    damping_ratio: f32,
    frequency_hz: f32,
    goal: f32,
    position: f32,
    velocity: f32,
}

impl SpicySpring {
    pub fn new(start: f32, frequency_hz: f32, damping_ratio: f32) -> Self {
        Self {
            damping_ratio,
            frequency_hz,
            goal: start,
            position: start,
            velocity: 0.0,
        }
    }

    pub fn set_goal(&mut self, goal: f32) {
        self.goal = goal;
    }

    pub fn snap_to(&mut self, value: f32) {
        self.goal = value;
        self.position = value;
        self.velocity = 0.0;
    }

    pub fn position(&self) -> f32 {
        self.position
    }

    pub fn goal(&self) -> f32 {
        self.goal
    }

    pub fn step(&mut self, dt_s: f32) -> f32 {
        let dt = dt_s.clamp(0.0, 0.05);
        if dt <= 0.0 {
            return self.position;
        }
        let d = self.damping_ratio;
        let f = self.frequency_hz * 2.0 * PI;
        let g = self.goal;
        let mut p = self.position;
        let mut v = self.velocity;

        if (d - 1.0).abs() < 1e-5 {
            // Critically damped
            let q = (-f * dt).exp();
            let w = dt * q;
            let c0 = q + w * f;
            let c2 = q - w * f;
            let c3 = w * f * f;
            let o = p - g;
            p = o * c0 + v * w + g;
            v = v * c2 - o * c3;
        } else if d < 1.0 {
            // Underdamped (Spicy Lyrics word pop & organic scroll settle)
            let q = (-d * f * dt).exp();
            let c = (1.0 - d * d).sqrt();
            let i = (dt * f * c).cos();
            let j = (dt * f * c).sin();
            let z = if c > 1e-5 { j / c } else { dt * f };
            let y = if f * c > 1e-5 { j / (f * c) } else { dt };
            let o = p - g;
            p = (o * (i + z * d) + v * y) * q + g;
            v = (v * (i - z * d) - o * (z * f)) * q;
        } else {
            // Overdamped
            let c = (d * d - 1.0).sqrt();
            let r1 = -f * (d + c);
            let r2 = -f * (d - c);
            let ec1 = (r1 * dt).exp();
            let ec2 = (r2 * dt).exp();
            let o = p - g;
            let co2 = (v - o * r1) / (2.0 * f * c);
            let co1 = ec1 * (o - co2);
            p = co1 + co2 * ec2 + g;
            v = co1 * r1 + co2 * ec2 * r2;
        }

        self.position = p;
        self.velocity = v;
        p
    }

    pub fn is_settled(&self) -> bool {
        (self.position - self.goal).abs() < 0.5 && self.velocity.abs() < 1.0
    }
}

/// Critically/lightly-damped spring scroll + active-line pop state for synced lyrics.
#[derive(Debug, Clone, Copy)]
pub struct LyricsSpringState {
    scroll: SpicySpring,
    active_scale: SpicySpring,
    last_active_idx: Option<usize>,
}

impl Default for LyricsSpringState {
    fn default() -> Self {
        Self::new()
    }
}

impl LyricsSpringState {
    pub fn new() -> Self {
        Self {
            // Smooth Apple Music / Spicy Lyrics scroll settle
            scroll: SpicySpring::new(0.0, 1.85, 0.92),
            // Exact Spicy Lyrics ScaleFrequency = 0.88, ScaleDamping = 0.64
            active_scale: SpicySpring::new(1.0, 1.35, 0.68),
            last_active_idx: None,
        }
    }

    pub fn update(&mut self, dt_s: f32, target_y: f32) {
        self.scroll.set_goal(target_y);
        self.scroll.step(dt_s);
        self.active_scale.step(dt_s);
    }

    pub fn notify_active_item(&mut self, item_idx: Option<usize>) {
        if item_idx != self.last_active_idx {
            self.last_active_idx = item_idx;
            if item_idx.is_some() {
                // Spicy Lyrics active line subtle pop: 0.95 -> springs to 1.045
                self.active_scale.snap_to(0.95);
                self.active_scale.set_goal(1.045);
            }
        }
    }

    pub fn snap_to(&mut self, y: f32) {
        self.scroll.snap_to(y);
        self.active_scale.snap_to(1.0);
        self.last_active_idx = None;
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll.position()
    }

    pub fn active_scale(&self) -> f32 {
        self.active_scale.position().clamp(0.94, 1.08)
    }

    pub fn is_settled(&self) -> bool {
        self.scroll.is_settled()
            && (self.active_scale.position() - self.active_scale.goal()).abs() < 0.002
    }
}

/// Display timeline item combining real lyric lines and auto-generated instrumental `• • •` interludes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsDisplayItem {
    Line {
        line_idx: usize,
        start_ms: u64,
        is_bg_vocal: bool,
    },
    Interlude {
        start_ms: u64,
        end_ms: u64,
    },
}

impl LyricsDisplayItem {
    pub fn start_ms(&self) -> u64 {
        match self {
            Self::Line { start_ms, .. } => *start_ms,
            Self::Interlude { start_ms, .. } => *start_ms,
        }
    }
}

/// Detect parenthesized background vocal lines like `(Ooh, yeah)` or `(I miss you)`.
pub fn is_background_vocal_text(text: &str) -> bool {
    let t = text.trim();
    t.len() >= 3
        && ((t.starts_with('(') && t.ends_with(')')) || (t.starts_with('[') && t.ends_with(']')))
}

/// Build the full display timeline inserting Spicy-style 3-dot instrumental interludes during
/// long intros (> 4s) and mid-song instrumental breaks (> 5s).
pub fn build_display_timeline(lines: &[LyricsLine]) -> Vec<LyricsDisplayItem> {
    if lines.is_empty() {
        return Vec::new();
    }

    let mut items = Vec::with_capacity(lines.len() + 6);

    // Intro interlude before the first lyric starts.
    if lines[0].time_ms >= INTRO_INTERLUDE_MIN_MS {
        items.push(LyricsDisplayItem::Interlude {
            start_ms: 0,
            end_ms: lines[0].time_ms,
        });
    }

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.text.trim();
        if trimmed.is_empty() {
            // Explicit empty timestamp line in LRC -> treat as interlude until next line if long enough.
            if let Some(next) = lines.get(idx + 1) {
                if next.time_ms.saturating_sub(line.time_ms) >= 2_500 {
                    items.push(LyricsDisplayItem::Interlude {
                        start_ms: line.time_ms,
                        end_ms: next.time_ms,
                    });
                }
            }
            continue;
        }

        items.push(LyricsDisplayItem::Line {
            line_idx: idx,
            start_ms: line.time_ms,
            is_bg_vocal: is_background_vocal_text(trimmed),
        });

        // Mid-song instrumental break detection between this line and the next non-empty line.
        if let Some(next) = lines.get(idx + 1) {
            if !next.text.trim().is_empty() {
                let est_line_end = line
                    .words
                    .as_ref()
                    .and_then(|w| w.last().map(|last| last.end_ms))
                    .unwrap_or_else(|| {
                        let wc = trimmed.split_whitespace().count() as u64;
                        line.time_ms.saturating_add((wc * 520).clamp(1_600, 4_800))
                    });
                let gap = next.time_ms.saturating_sub(est_line_end);
                if gap >= GAP_INTERLUDE_MIN_MS {
                    items.push(LyricsDisplayItem::Interlude {
                        start_ms: est_line_end,
                        end_ms: next.time_ms,
                    });
                }
            }
        }
    }

    items
}

pub fn active_display_index(items: &[LyricsDisplayItem], pos_ms: u64) -> Option<usize> {
    if items.is_empty() {
        return None;
    }
    let next_idx = items.partition_point(|item| item.start_ms() <= pos_ms);
    if next_idx == 0 {
        None
    } else {
        Some(next_idx - 1)
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
                scale: 1.05,
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
                opacity: 0.45,
                glow: 0.0,
            },
            _ => LineStyle {
                scale: 1.0,
                opacity: 0.22,
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

/// Compute Spicy Lyrics' 3 instrumental dot `(opacity, scale)` values across `[start_ms..end_ms]`.
/// Ported from `DotAnimations` (`ScaleRange: 0.75 -> 1.05 -> 1.0`, `OpacityRange: 0.30 -> 1.0`).
pub fn interlude_dot_states(start_ms: u64, end_ms: u64, pos_ms: u64) -> [(f32, f32); 3] {
    let span = end_ms.saturating_sub(start_ms).max(1) as f32;
    let total_prog = ((pos_ms.saturating_sub(start_ms)) as f32 / span).clamp(0.0, 1.0);
    let mut out = [(0.30_f32, 0.75_f32); 3];
    for (i, slot) in out.iter_mut().enumerate() {
        let seg_start = i as f32 / 3.0;
        let seg_end = (i + 1) as f32 / 3.0;
        let p = ((total_prog - seg_start) / (seg_end - seg_start)).clamp(0.0, 1.0);
        // Smooth ease-out opacity (0.28 -> 1.0)
        let eased = 1.0 - (1.0 - p) * (1.0 - p);
        let opacity = 0.28 + 0.72 * eased;
        // Spicy DotAnimations ScaleRange: 0.75 -> peaks at 1.08 around 0.7 -> settles at 1.0
        let scale = if p <= 0.7 {
            let t = p / 0.7;
            0.75 + (1.08 - 0.75) * (1.0 - (1.0 - t) * (1.0 - t))
        } else {
            let t = (p - 0.7) / 0.3;
            1.08 - 0.08 * t
        };
        *slot = (opacity, scale);
    }
    out
}

/// Return real enhanced-LRC / TTML word timings if present, or synthesize smooth proportional
/// per-word timings across the line's vocal window so line-synced tracks (e.g. LRCLIB)
/// still deliver Spicy-Lyrics-style word-by-word Bloom sweep highlighting.
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
    let max_vocal_ms = (tokens.len() as u64 * 560).clamp(1_400, 6_200);
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
    let total_tokens = tokens.len();
    for (i, (tok, w)) in tokens.into_iter().zip(weights).enumerate() {
        let dur = (vocal_span as f32) * (w / total_weight);
        let w_start = cursor.round() as u64;
        cursor += dur;
        let w_end = if i + 1 == total_tokens {
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

/// Compute Spicy Lyrics Bloom Renderer per-character opacity spans along an active line.
///
/// Mirrors `--SLR_FadeWidth: 0.882em`: one continuous feathered sweep edge travels smoothly
/// left-to-right across the entire line (`alpha 1.0` lit -> feathered transition -> `alpha 0.36` unlit).
/// Adjacent characters with nearly identical alpha are merged into compact runs so `rich_text`
/// stays lightweight (< 8 spans per active line).
pub fn bloom_sweep_runs(words: &[LyricsWord], pos_ms: u64) -> Vec<(String, f32)> {
    if words.is_empty() {
        return Vec::new();
    }

    // Build flat character sequence with each character's exact sweep edge progress.
    let mut full_chars: Vec<char> = Vec::new();
    let mut word_char_ranges: Vec<(usize, usize)> = Vec::with_capacity(words.len());
    for (w_idx, w) in words.iter().enumerate() {
        let start = full_chars.len();
        full_chars.extend(w.text.chars());
        let end = full_chars.len();
        word_char_ranges.push((start, end));
        if w_idx + 1 < words.len() {
            full_chars.push(' ');
        }
    }

    let total_chars = full_chars.len() as f32;
    if total_chars <= 0.0 {
        return Vec::new();
    }

    // Map `pos_ms` onto a continuous character cursor `edge_pos` in `[-BLOOM_FADE_HALF_CHARS .. total_chars + BLOOM_FADE_HALF_CHARS]`.
    let edge_pos: f32 = if pos_ms <= words[0].start_ms {
        -BLOOM_FADE_HALF_CHARS
    } else if pos_ms >= words[words.len() - 1].end_ms {
        total_chars + BLOOM_FADE_HALF_CHARS
    } else {
        let mut cursor = -BLOOM_FADE_HALF_CHARS;
        for (i, word) in words.iter().enumerate() {
            let (c_start, c_end) = word_char_ranges[i];
            if pos_ms < word.start_ms {
                cursor = c_start as f32;
                break;
            } else if pos_ms <= word.end_ms {
                let p = word_progress(word, pos_ms);
                // Smooth ease-in-out slightly for organic vocal glide
                cursor = (c_start as f32) + p * ((c_end - c_start) as f32);
                break;
            } else {
                cursor = c_end as f32 + if i + 1 < words.len() { 1.0 } else { 0.0 };
            }
        }
        cursor
    };

    const UNLIT_ALPHA: f32 = 0.36;
    const LIT_ALPHA: f32 = 1.0;

    let mut runs: Vec<(String, f32)> = Vec::with_capacity(8);
    for (ci, ch) in full_chars.into_iter().enumerate() {
        let center = ci as f32 + 0.5;
        // 1.0 when edge_pos is well past this character, 0.0 when edge_pos hasn't reached it yet.
        let t = ((edge_pos - center + BLOOM_FADE_HALF_CHARS) / (2.0 * BLOOM_FADE_HALF_CHARS))
            .clamp(0.0, 1.0);
        // Hermite smoothstep for velvety Bloom gradient edge
        let smooth = t * t * (3.0 - 2.0 * t);
        let alpha = UNLIT_ALPHA + (LIT_ALPHA - UNLIT_ALPHA) * smooth;
        // Quantize to 0.04 steps so lit prefix and unlit suffix collapse into single spans.
        let q_alpha = (alpha * 25.0).round() / 25.0;

        if let Some((last_text, last_alpha)) = runs.last_mut() {
            if (*last_alpha - q_alpha).abs() < 1e-4 {
                last_text.push(ch);
                continue;
            }
        }
        runs.push((ch.to_string(), q_alpha));
    }

    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spring_converges_to_target_and_marks_settled() {
        let mut spring = LyricsSpringState::new();
        for _ in 0..240 {
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
                time_ms: 40_000,
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

    #[test]
    fn build_display_timeline_inserts_intro_and_gap_interludes_and_detects_bg_vocals() {
        let lines = vec![
            LyricsLine {
                time_ms: 12_000,
                text: "First line".into(),
                words: None,
            },
            LyricsLine {
                time_ms: 30_000,
                text: "(Ooh yeah)".into(),
                words: None,
            },
        ];
        let items = build_display_timeline(&lines);
        assert!(matches!(items[0], LyricsDisplayItem::Interlude { start_ms: 0, end_ms: 12_000 }));
        assert!(matches!(items[1], LyricsDisplayItem::Line { line_idx: 0, is_bg_vocal: false, .. }));
        assert!(matches!(items[2], LyricsDisplayItem::Interlude { end_ms: 30_000, .. }));
        assert!(matches!(items[3], LyricsDisplayItem::Line { line_idx: 1, is_bg_vocal: true, .. }));
    }

    #[test]
    fn bloom_sweep_runs_reconstructs_full_text_and_fades_left_to_right() {
        let words = vec![
            LyricsWord {
                start_ms: 1000,
                end_ms: 2000,
                text: "And".into(),
            },
            LyricsWord {
                start_ms: 2000,
                end_ms: 3000,
                text: "I".into(),
            },
            LyricsWord {
                start_ms: 3000,
                end_ms: 4500,
                text: "miss".into(),
            },
            LyricsWord {
                start_ms: 4500,
                end_ms: 6000,
                text: "my".into(),
            },
            LyricsWord {
                start_ms: 6000,
                end_ms: 8000,
                text: "lover".into(),
            },
        ];
        let runs = bloom_sweep_runs(&words, 3_500);
        let reconstructed: String = runs.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(reconstructed, "And I miss my lover");
        assert!(runs.first().unwrap().1 > 0.9, "sung prefix must be bright");
        assert!(runs.last().unwrap().1 <= 0.40, "unsung suffix must be dimmed");
    }

    #[test]
    fn interlude_dot_states_light_up_sequentially() {
        let early = interlude_dot_states(0, 9_000, 1_500);
        assert!(early[0].0 > early[1].0 && early[1].0 <= 0.30);
        let late = interlude_dot_states(0, 9_000, 8_500);
        assert!(late[0].0 > 0.99 && late[1].0 > 0.99 && late[2].0 > 0.85);
    }

    #[test]
    fn target_scroll_y_tracks_variable_line_heights_and_keeps_active_centered() {
        let lines = vec![
            LyricsLine {
                time_ms: 0,
                text: "Short".into(),
                words: None,
            },
            LyricsLine {
                time_ms: 4_000,
                text: "A much longer lyric line that will definitely wrap across multiple visual lines on a narrow window".into(),
                words: None,
            },
            LyricsLine {
                time_ms: 15_000, // creates an interlude before line 2
                text: "You found me".into(),
                words: None,
            },
        ];
        let timeline = build_display_timeline(&lines);
        assert_eq!(timeline.len(), 4);

        let y0 = target_scroll_y_for_item(&timeline, &lines, 0, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);
        let y1 = target_scroll_y_for_item(&timeline, &lines, 1, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);
        let y2 = target_scroll_y_for_item(&timeline, &lines, 2, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);
        let y3 = target_scroll_y_for_item(&timeline, &lines, 3, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);

        assert!(y0 <= 40.0, "first line starts cleanly near center without overshooting: {y0}");
        assert!(y1 > y0 && y2 > y1 && y3 > y2, "offsets strictly increase: {y0}, {y1}, {y2}, {y3}");
        // Wrapped line 1 contributes noticeably more vertical advance than the 34px interlude.
        assert!((y2 - y1) > (y3 - y2), "wrapped lyric advances more than interlude");
    }
}
