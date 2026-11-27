use std::f32::consts::PI;

use lume_core::{LyricsLine, LyricsWord};
use lume_storage::LyricsVisualMode;

/// Minimum intro silence before the first lyric line to render an instrumental `• • •` interlude.
pub const INTRO_INTERLUDE_MIN_MS: u64 = 4_000;
/// Minimum mid-song gap between two lyric lines to render an instrumental `• • •` interlude.
pub const GAP_INTERLUDE_MIN_MS: u64 = 5_000;
/// Width (in character units) of Spicy Lyrics' Bloom Renderer feathered sweep edge (`--SLR_FadeWidth: 0.882em`).
pub const BLOOM_FADE_HALF_CHARS: f32 = 1.35;
/// Exact Spicy Lyrics Bloom Renderer fade width in `em` (`Mixed.css --SLR_FadeWidth: 0.882em`).
pub const REFRESHED_SWEEP_FADE_EM: f32 = 0.882;
/// Exact Spicy Lyrics Bloom Renderer perceptual timing offset (`LyricsAnimator.ts:1291`).
pub const BLOOM_TIME_OFFSET_MS: f32 = -33.5;
/// Exact Spicy Lyrics Bloom vertical lift amplitude in `em` (`RefreshedLift.Em`).
pub const REFRESHED_LIFT_EM: f32 = 0.0588;
/// Exact Spicy Lyrics Bloom sweep row-end rush duration (`RefreshedSweep.RushMs`).
pub const REFRESHED_SWEEP_RUSH_MS: f32 = 500.0;
/// Minimum sustained syllable/word duration (ms) to trigger letter-by-letter Flame emphasis (`IsLetterCapable.ts`).
pub const FLAME_MIN_DURATION_MS: f32 = 1_000.0;
/// Exact Spicy Lyrics Bloom unlit & lit text tones (tuned so incoming +1 lines at 0.36 opacity
/// never flash darker when transitioning into the active Bloom sweep).
pub const BLOOM_UNLIT_ALPHA: f32 = 0.34;
pub const BLOOM_LIT_ALPHA: f32 = 0.98;

/// CSS `cubic-bezier(x1, y1, x2, y2)` solver ported 1:1 from Spicy Lyrics `CubicBezier.ts`.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let cx = 3.0 * x1;
    let bx = 3.0 * (x2 - x1) - cx;
    let ax = 1.0 - cx - bx;
    let cy = 3.0 * y1;
    let by = 3.0 * (y2 - y1) - cy;
    let ay = 1.0 - cy - by;
    let curve_x = |t: f32| ((ax * t + bx) * t + cx) * t;
    let curve_y = |t: f32| ((ay * t + by) * t + cy) * t;
    let slope_x = |t: f32| (3.0 * ax * t + 2.0 * bx) * t + cx;

    let mut t = x;
    for _ in 0..8 {
        let err = curve_x(t) - x;
        if err.abs() < 1e-6 {
            return curve_y(t);
        }
        let slope = slope_x(t);
        if slope.abs() < 1e-6 {
            break;
        }
        t = (t - err / slope).clamp(0.0, 1.0);
    }

    let mut lo = 0.0_f32;
    let mut hi = 1.0_f32;
    t = x;
    while hi - lo > 1e-6 {
        if curve_x(t) < x {
            lo = t;
        } else {
            hi = t;
        }
        t = (lo + hi) * 0.5;
    }
    curve_y(t)
}

/// Closed-form damped upward lift (`0.0 -> 1.0`) ported 1:1 from Spicy Lyrics `getRefreshedLift`
/// (`Omega = 5.0 rad/s`, `Damping = 0.93`).
pub fn refreshed_lift(elapsed_ms: f32) -> f32 {
    if elapsed_ms <= 0.0 {
        return 0.0;
    }
    const OMEGA: f32 = 5.0;
    const DAMPING: f32 = 0.93;
    let t = elapsed_ms / 1000.0;
    let decay = (-DAMPING * OMEGA * t).exp();
    if decay < 1e-4 {
        return 1.0;
    }
    let wd = OMEGA * (1.0 - DAMPING * DAMPING).sqrt();
    1.0 - decay * ((wd * t).cos() + ((DAMPING * OMEGA) / wd) * (wd * t).sin())
}

/// Exact Spicy Lyrics Bloom `IsLetterCapable` predicate:
/// sustained words held `>= 1000ms` with `2..=7` letters (and non-background) animate letter-by-letter.
pub fn is_letter_capable(letter_len: usize, duration_ms: f32, is_background: bool) -> bool {
    !is_background
        && (2..=7).contains(&letter_len)
        && duration_ms >= FLAME_MIN_DURATION_MS
}

/// A grouped visual word made of 1 or more syllables linked by `is_part_of_word`.
#[derive(Debug, Clone, PartialEq)]
pub struct WordGroupPlan {
    /// Range `[start..end)` into the line's syllable slice.
    pub syllable_range: std::ops::Range<usize>,
    /// Total grapheme/char count across all syllables in this word.
    pub total_letters: usize,
    /// True if this word qualifies for Spicy Lyrics' sustained letter-by-letter Flame animation.
    pub is_flame_split: bool,
    /// Start timestamp of the first syllable in the group.
    pub start_ms: u64,
    /// End timestamp of the last syllable in the group.
    pub end_ms: u64,
}

/// Port of Spicy Lyrics `planWords` (`Syllable.ts:134-175`):
/// groups contiguous `is_part_of_word` syllables into indivisible words so wrapping never splits
/// mid-word and sustained multi-syllable words swell together as a single FlameGroup.
pub fn plan_word_groups(words: &[LyricsWord], is_background: bool) -> Vec<WordGroupPlan> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut end = i;
        while words[end].is_part_of_word && end + 1 < words.len() {
            end += 1;
        }
        let start_ms = words[i].start_ms;
        let end_ms = words[end].end_ms.max(start_ms + 1);
        let duration_ms = end_ms.saturating_sub(start_ms) as f32;
        let total_letters: usize = words[i..=end].iter().map(|w| w.text.chars().count()).sum();
        let is_flame_split = is_letter_capable(total_letters, duration_ms, is_background);
        groups.push(WordGroupPlan {
            syllable_range: i..(end + 1),
            total_letters,
            is_flame_split,
            start_ms,
            end_ms,
        });
        i = end + 1;
    }
    groups
}

/// Per-letter animated transform & glow state for a sustained Flame word (`getRefreshedFlame`).
#[derive(Debug, Clone, PartialEq)]
pub struct FlameLetterState {
    pub scale: f32,
    /// Horizontal push away from word center, in `em`.
    pub x_em: f32,
    /// Vertical lift (negative = upward), in `em`.
    pub y_em: f32,
    /// White glow opacity `[0.0 .. 0.5]`.
    pub glow: f32,
}

/// Port 1:1 of Spicy Lyrics `getRefreshedFlame` (`LyricsAnimator.ts:346-403`).
/// Computes per-letter swell, symmetric sideways push, staggered upward lift, and white glow.
pub fn compute_refreshed_flame(
    letter_widths_em: &[f32],
    start_ms: f32,
    duration_ms: f32,
    pos_ms: f32,
) -> Vec<FlameLetterState> {
    const SCALE_MAX: f32 = 0.14;
    const FROM_MS: f32 = 1000.0;
    const TO_MS: f32 = 2000.0;
    const STAGGER: f32 = 0.4;
    const MAX_STAGGER_MS: f32 = 400.0;
    const MAX_PHASE_MS: f32 = 3000.0;
    const GLOW_ALPHA: f32 = 0.5;

    let n = letter_widths_em.len();
    if n == 0 {
        return Vec::new();
    }

    let duration = duration_ms.max(1.0);
    let phase = duration.min(MAX_PHASE_MS);
    let stagger = ((STAGGER * duration) / (n as f32)).min(MAX_STAGGER_MS);
    let hold_ms = (2.0 * duration) / (n as f32);
    let peak = SCALE_MAX * ((duration / FROM_MS).clamp(1.0, TO_MS / FROM_MS) - 1.0);
    let held = cubic_bezier(0.25, 0.1, 0.25, 1.0, (hold_ms / phase).min(1.0));
    let elapsed = pos_ms - start_ms;

    if elapsed <= 0.0 {
        return vec![
            FlameLetterState {
                scale: 1.0,
                x_em: 0.0,
                y_em: 0.0,
                glow: 0.0,
            };
            n
        ];
    }

    let mut scales = Vec::with_capacity(n);
    let mut glows = Vec::with_capacity(n);
    let mut ys = Vec::with_capacity(n);

    for i in 0..n {
        let at = elapsed - (i as f32) * stagger;
        let amount = if at > 0.0 && at < hold_ms {
            cubic_bezier(0.25, 0.1, 0.25, 1.0, (at / phase).min(1.0))
        } else if at >= hold_ms {
            held * (1.0 - cubic_bezier(0.25, 0.1, 0.25, 1.0, ((at - hold_ms) / phase).min(1.0)))
        } else {
            0.0
        };
        let lift = refreshed_lift(at);
        scales.push(1.0 + peak * amount);
        glows.push(GLOW_ALPHA * amount);
        ys.push(-REFRESHED_LIFT_EM * lift);
    }

    let half: Vec<f32> = scales
        .iter()
        .enumerate()
        .map(|(i, &s)| ((s - 1.0) * letter_widths_em[i]) * 0.5)
        .collect();
    let mut xs = vec![0.0_f32; n];
    let middle = if n.is_multiple_of(2) {
        (n as f32) * 0.5 - 0.5
    } else {
        (n / 2) as f32
    };
    let is_int_mid = (middle - middle.floor()).abs() < 1e-4;
    let left_start: isize = if is_int_mid {
        middle as isize - 1
    } else {
        middle.floor() as isize
    };
    let right_start: usize = if is_int_mid {
        middle as usize + 1
    } else {
        middle.ceil() as usize
    };

    let mut idx = left_start;
    while idx >= 0 {
        let i = idx as usize;
        let mut push = half[i];
        if ((i + 1) as f32) <= middle {
            push += half[i + 1];
        }
        if ((i + 1) as f32) < middle {
            push += xs[i + 1].abs();
        }
        xs[i] = -push * 0.5;
        idx -= 1;
    }
    for i in right_start..n {
        let mut push = half[i];
        if ((i - 1) as f32) >= middle {
            push += half[i - 1];
        }
        if ((i - 1) as f32) > middle {
            push += xs[i - 1].abs();
        }
        xs[i] = push * 0.5;
    }

    (0..n)
        .map(|i| FlameLetterState {
            scale: scales[i],
            x_em: xs[i],
            y_em: ys[i],
            glow: glows[i],
        })
        .collect()
}

/// Measured linear strip geometry for one syllable in Spicy Lyrics' continuous line sweep.
#[derive(Debug, Clone, Copy)]
pub struct SweepSpanMetrics {
    /// Cumulative linear X offset along the line strip (in px).
    pub strip_x: f32,
    /// Box width (in px, including trailing space if not `is_part_of_word`).
    pub width: f32,
    /// Number of letters in the whole word this syllable belongs to.
    pub word_letters: usize,
    /// True if this syllable is part of a Flame-split word.
    pub is_split: bool,
    /// True if this syllable begins a wrapped visual row.
    pub row_start: bool,
    /// True if this syllable ends a wrapped visual row.
    pub row_end: bool,
}

impl SweepSpanMetrics {
    pub fn reach(&self) -> f32 {
        if self.is_split {
            if self.word_letters <= 2 {
                0.25
            } else {
                0.12
            }
        } else if self.row_end {
            1.0
        } else if self.word_letters <= 2 {
            0.5
        } else {
            0.25
        }
    }
}

/// Port 1:1 of Spicy Lyrics `updateLineSweep` (`LyricsAnimator.ts:637-684`).
/// Returns the strip X position of the light-fade center for each syllable (`f32::NEG_INFINITY` if unreached).
pub fn compute_line_sweep_edges(
    spans: &[SweepSpanMetrics],
    words: &[LyricsWord],
    pos_ms: f32,
    fade_px: f32,
) -> Vec<f32> {
    let n = spans.len().min(words.len());
    let mut edges = vec![f32::NEG_INFINITY; n];
    if n == 0 {
        return edges;
    }

    let half = fade_px * 0.5;
    let mut reached = f32::NEG_INFINITY;
    let mut carried = f32::NEG_INFINITY;

    for i in 0..n {
        let span = spans[i];
        let word = &words[i];
        let start = span.strip_x - half;
        let from = if span.row_start {
            start
        } else {
            start.max(carried)
        };
        let to = from.max(span.strip_x + span.width + fade_px * span.reach() - half);
        carried = to;

        let w_start = word.start_ms as f32;
        let w_end = word.end_ms.max(word.start_ms + 1) as f32;
        let mut at = f32::NEG_INFINITY;
        if pos_ms >= w_end {
            at = to;
            let lit = span.strip_x + span.width + half;
            if span.row_end && to < lit {
                let rush = ((pos_ms - w_end) / REFRESHED_SWEEP_RUSH_MS).clamp(0.0, 1.0);
                at = to + (lit - to) * rush;
            }
        } else if pos_ms > w_start {
            let share = ((pos_ms - w_start) / (w_end - w_start)).clamp(0.0, 1.0);
            at = from + (to - from) * share;
        }
        edges[i] = at;
        if at > reached {
            reached = at;
        }
    }

    let mut before = f32::NEG_INFINITY;
    for i in 0..n {
        let span = spans[i];
        if span.row_start {
            before = f32::NEG_INFINITY;
        }
        let word = &words[i];
        let w_start = word.start_ms as f32;
        let w_end = word.end_ms.max(word.start_ms + 1) as f32;
        let at = edges[i];
        if pos_ms >= w_end {
            edges[i] = reached;
        } else if pos_ms <= w_start {
            edges[i] = before;
        }
        if at > before {
            before = at;
        }
    }

    edges
}

/// Bloom interlude state ported 1:1 from Spicy Lyrics `animateRefreshedDot` (`LyricsAnimator.ts:940-1029`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BloomInterludeState {
    pub dot_opacities: [f32; 3],
    pub group_scale: f32,
    pub group_opacity: f32,
}

pub fn bloom_interlude_state(start_ms: u64, end_ms: u64, pos_ms: u64) -> BloomInterludeState {
    const DIM: f32 = 0.18;
    const LIT: f32 = 0.94;
    const ENTER_MS: f32 = 750.0;
    const ENTER_STAGGER_MS: f32 = 50.0;
    const BREATH_MS: f32 = 4000.0;
    const BREATH_SCALE: f32 = 0.2;
    const EXIT_GROW_MS: f32 = 750.0;
    const EXIT_SHRINK_MS: f32 = 250.0;
    const EXIT_SCALE: f32 = 0.5;
    const PRE_HIDDEN_MS: f32 = 250.0;

    let total_ms = end_ms.saturating_sub(start_ms).max(1) as f32;
    let elapsed = (pos_ms as f32) - (start_ms as f32);
    if elapsed < 0.0 {
        return BloomInterludeState {
            dot_opacities: [0.0; 3],
            group_scale: 1.0,
            group_opacity: 0.0,
        };
    }

    let exit_at = (total_ms - PRE_HIDDEN_MS - EXIT_GROW_MS).max(0.0);
    let share = ((exit_at + EXIT_GROW_MS) / 3.0).max(1.0);

    let dot_fill_ease = |x: f32| cubic_bezier(0.0, 0.25, 1.0, 0.58, x);
    let dot_exit_ease = |x: f32| cubic_bezier(0.25, 0.0, 1.0, 0.2, x);
    let dot_breath_ease = |x: f32| cubic_bezier(0.5, 0.02, 0.42, 0.98, x);
    let emp_ease = |x: f32| cubic_bezier(0.25, 0.1, 0.25, 1.0, x);

    let mut dot_opacities = [0.0_f32; 3];
    for (i, slot) in dot_opacities.iter_mut().enumerate() {
        let fill = if i < 2 {
            dot_fill_ease(((elapsed - (i as f32) * share) / share).clamp(0.0, 1.0))
        } else {
            let from = 2.0 * share;
            let room = (exit_at - from).max(1.0);
            let reached = (room / share).min(1.0);
            if elapsed < exit_at {
                reached * dot_fill_ease(((elapsed - from) / room).clamp(0.0, 1.0))
            } else {
                reached + (1.0 - reached) * ((elapsed - exit_at) / EXIT_GROW_MS).clamp(0.0, 1.0)
            }
        };
        let enter_t = ((elapsed - (i as f32) * ENTER_STAGGER_MS) / ENTER_MS).clamp(0.0, 1.0);
        let enter = enter_t * enter_t;
        *slot = (DIM + (LIT - DIM) * fill) * enter;
    }

    let breath_scale = |at: f32| -> f32 {
        let cycles = (exit_at / BREATH_MS).floor();
        if cycles < 1.0 {
            return 1.0;
        }
        let length = exit_at / cycles;
        let x = (at.clamp(0.0, exit_at) % length) / length;
        if x < 0.5 {
            1.0 + BREATH_SCALE * dot_breath_ease(x * 2.0)
        } else {
            1.0 + BREATH_SCALE - BREATH_SCALE * dot_breath_ease((x - 0.5) * 2.0)
        }
    };

    let (group_scale, group_opacity) = if elapsed < exit_at {
        (breath_scale(elapsed), 1.0)
    } else if elapsed < exit_at + EXIT_GROW_MS {
        let from = breath_scale(exit_at);
        let top = 1.0 + BREATH_SCALE;
        let s = from + (top - from) * emp_ease(((elapsed - exit_at) / EXIT_GROW_MS).clamp(0.0, 1.0));
        (s, 1.0)
    } else {
        let x = dot_exit_ease(((elapsed - exit_at - EXIT_GROW_MS) / EXIT_SHRINK_MS).clamp(0.0, 1.0));
        let s = 1.0 + BREATH_SCALE + (EXIT_SCALE - 1.0 - BREATH_SCALE) * x;
        (s, (1.0 - x).clamp(0.0, 1.0))
    };

    BloomInterludeState {
        dot_opacities,
        group_scale,
        group_opacity,
    }
}

/// Shared layout geometry constants kept identical between `lyrics_view` and scroll centering.
pub const LYRICS_ITEM_SPACING_PX: f32 = 20.0;
pub const LYRICS_INTERLUDE_HEIGHT_PX: f32 = 34.0;
pub const LYRICS_LINE_HEIGHT_MULT: f32 = 1.22;
pub const LYRICS_LINE_VPAD_PX: f32 = 10.0;
pub const LYRICS_ACTIVE_FONT_PX: f32 = 36.0;
pub const LYRICS_ACTIVE_BG_FONT_PX: f32 = 26.0;
pub const LYRICS_IDLE_FONT_PX: f32 = 33.0;
pub const LYRICS_IDLE_BG_FONT_PX: f32 = 24.0;

pub fn lyrics_viewport_metrics(window_w: f32, window_h: f32) -> (f32, f32) {
    let viewport_h = (window_h - 66.0).max(240.0);
    let art_size = (window_h * 0.39).min(window_w * 0.30).clamp(200.0, 332.0);
    let left_stage_w = (art_size + 68.0).clamp(264.0, 400.0);
    // Left stage + row gap (20) + body padding (28) + lyrics horiz padding (64) + inner padding (12)
    let text_max_w = (window_w - left_stage_w - 124.0).max(220.0);
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
                    if is_bg_vocal {
                        LYRICS_ACTIVE_BG_FONT_PX
                    } else {
                        LYRICS_ACTIVE_FONT_PX
                    }
                } else {
                    let base = if is_bg_vocal {
                        LYRICS_IDLE_BG_FONT_PX
                    } else {
                        LYRICS_IDLE_FONT_PX
                    };
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

/// Critically-damped spring scroll + continuous visual active-index + subtle active-line breath
/// for silky Apple Music / Spicy Lyrics transitions across both syllable-synced and LRCLIB tracks.
#[derive(Debug, Clone, Copy)]
pub struct LyricsSpringState {
    scroll: SpicySpring,
    visual_idx: SpicySpring,
    active_scale: SpicySpring,
    last_active_idx: Option<usize>,
    has_visual_idx: bool,
}

impl Default for LyricsSpringState {
    fn default() -> Self {
        Self::new()
    }
}

impl LyricsSpringState {
    pub fn new() -> Self {
        Self {
            // Critically-damped Apple Music scroll glide (zero overshoot/bounce, velvet settle)
            scroll: SpicySpring::new(0.0, 1.75, 1.0),
            // Smooth continuous line-focus glide so every line's scale & opacity interpolates at 60 FPS
            visual_idx: SpicySpring::new(0.0, 2.25, 0.98),
            // Subtle active-line breath without jarring jump
            active_scale: SpicySpring::new(1.0, 1.65, 0.85),
            last_active_idx: None,
            has_visual_idx: false,
        }
    }

    pub fn update(&mut self, dt_s: f32, target_y: f32) {
        self.scroll.set_goal(target_y);
        self.scroll.step(dt_s);
        if self.has_visual_idx {
            self.visual_idx.step(dt_s);
        }
        self.active_scale.step(dt_s);
    }

    pub fn notify_active_item(&mut self, item_idx: Option<usize>) {
        if item_idx != self.last_active_idx {
            let prev = self.last_active_idx;
            self.last_active_idx = item_idx;
            if let Some(idx) = item_idx {
                let target = idx as f32;
                // Snap on initial entry or large seeks (> 3 lines); glide smoothly on normal advance.
                let is_large_jump = prev.is_some_and(|p| p.abs_diff(idx) > 3);
                if !self.has_visual_idx || is_large_jump {
                    self.visual_idx.snap_to(target);
                    self.has_visual_idx = true;
                } else {
                    self.visual_idx.set_goal(target);
                }
                self.active_scale.snap_to(0.985);
                self.active_scale.set_goal(1.02);
            }
        }
    }

    pub fn snap_to(&mut self, y: f32) {
        self.scroll.snap_to(y);
        self.visual_idx.snap_to(0.0);
        self.active_scale.snap_to(1.0);
        self.last_active_idx = None;
        self.has_visual_idx = false;
    }

    pub fn scroll_y(&self) -> f32 {
        self.scroll.position()
    }

    pub fn visual_active_idx(&self) -> Option<f32> {
        self.has_visual_idx.then_some(self.visual_idx.position())
    }

    pub fn active_scale(&self) -> f32 {
        self.active_scale.position().clamp(0.96, 1.05)
    }

    pub fn is_settled(&self) -> bool {
        let idx_settled = !self.has_visual_idx
            || ((self.visual_idx.position() - self.visual_idx.goal()).abs() < 0.005
                && self.visual_idx.velocity.abs() < 0.02);
        self.scroll.is_settled()
            && idx_settled
            && (self.active_scale.position() - self.active_scale.goal()).abs() < 0.002
    }
}

/// Display timeline item combining real lyric lines and auto-generated instrumental `• • •` interludes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LyricsDisplayItem {
    Line {
        line_idx: usize,
        start_ms: u64,
        end_ms: u64,
        is_bg_vocal: bool,
        opposite_aligned: bool,
    },
    Interlude {
        start_ms: u64,
        end_ms: u64,
        opposite_aligned: bool,
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
            opposite_aligned: lines[0].opposite_aligned,
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
                        opposite_aligned: next.opposite_aligned,
                    });
                }
            }
            continue;
        }

        let has_exact_end = line.end_ms.is_some()
            || line.words.as_ref().is_some_and(|w| !w.is_empty());
        let mut est_line_end = line
            .end_ms
            .or_else(|| {
                line.words
                    .as_ref()
                    .and_then(|w| w.last().map(|last| last.end_ms))
            })
            .unwrap_or_else(|| {
                let wc = trimmed.split_whitespace().count() as u64;
                line.time_ms.saturating_add((wc * 520).clamp(1_600, 4_800))
            });
        // Line-only lyrics (e.g. LRCLIB) lack explicit end timestamps and never overlap
        // the next line; clamp estimated end to `next.time_ms` so fast long lines never
        // stay falsely active after the next line has already started.
        if !has_exact_end {
            if let Some(next) = lines.get(idx + 1) {
                if next.time_ms > line.time_ms {
                    est_line_end = est_line_end.min(next.time_ms);
                }
            }
        }

        items.push(LyricsDisplayItem::Line {
            line_idx: idx,
            start_ms: line.time_ms,
            end_ms: est_line_end.max(line.time_ms + 1),
            is_bg_vocal: line.is_background || is_background_vocal_text(trimmed),
            opposite_aligned: line.opposite_aligned,
        });

        // Mid-song instrumental break detection between this line and the next non-empty line.
        // Native syllable/TTML lines have exact `end_ms` (use Spicy Lyrics' ~3.5s threshold);
        // line-only LRCLIB fallback lacks exact line end times so require a larger 8.5s gap
        // to avoid false interludes during slow sustained vocals.
        if let Some(next) = lines.get(idx + 1) {
            if !next.text.trim().is_empty() {
                let min_gap = if has_exact_end {
                    3_500
                } else {
                    GAP_INTERLUDE_MIN_MS.max(8_500)
                };
                let gap = next.time_ms.saturating_sub(est_line_end);
                if gap >= min_gap {
                    items.push(LyricsDisplayItem::Interlude {
                        start_ms: est_line_end,
                        end_ms: next.time_ms,
                        opposite_aligned: next.opposite_aligned,
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

/// Continuously interpolated visual depth style for fractional distance from the spring-driven
/// `visual_active_idx`, eliminating 1-frame opacity/scale pops across every visible lyric line.
pub fn line_visual_style_continuous(float_distance: f32, mode: LyricsVisualMode) -> LineStyle {
    let abs_d = float_distance.abs().min(4.0);
    let lo = abs_d.floor() as isize;
    let hi = (lo + 1).min(4);
    let raw_t = (abs_d - lo as f32).clamp(0.0, 1.0);
    // Smoothstep interpolation between depth stops
    let t = raw_t * raw_t * (3.0 - 2.0 * raw_t);
    let a = line_visual_style(lo, mode);
    let b = line_visual_style(hi, mode);
    LineStyle {
        scale: a.scale + (b.scale - a.scale) * t,
        opacity: a.opacity + (b.opacity - a.opacity) * t,
        glow: a.glow + (b.glow - a.glow) * t,
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

fn is_latin_vowel(ch: char) -> bool {
    matches!(
        ch.to_ascii_lowercase(),
        'a' | 'e' | 'i' | 'o' | 'u' | 'y'
    ) || matches!(
        ch,
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å'
            | 'é' | 'è' | 'ê' | 'ë'
            | 'í' | 'ì' | 'î' | 'ï'
            | 'ó' | 'ò' | 'ô' | 'ö' | 'õ'
            | 'ú' | 'ù' | 'û' | 'ü'
            | 'Á' | 'À' | 'Â' | 'Ä' | 'Ã' | 'Å'
            | 'É' | 'È' | 'Ê' | 'Ë'
            | 'Í' | 'Ì' | 'Î' | 'Ï'
            | 'Ó' | 'Ò' | 'Ô' | 'Ö' | 'Õ'
            | 'Ú' | 'Ù' | 'Û' | 'Ü'
    )
}

fn is_onset_digraph(a: char, b: char) -> bool {
    matches!(
        (a.to_ascii_lowercase(), b.to_ascii_lowercase()),
        ('n', 'g')
            | ('n', 'y')
            | ('k', 'h')
            | ('s', 'y')
            | ('c', 'h')
            | ('s', 'h')
            | ('t', 'h')
            | ('p', 'h')
            | ('w', 'h')
            | ('t', 'r')
            | ('p', 'r')
            | ('b', 'r')
            | ('c', 'r')
            | ('d', 'r')
            | ('f', 'r')
            | ('g', 'r')
            | ('p', 'l')
            | ('b', 'l')
            | ('c', 'l')
            | ('f', 'l')
            | ('g', 'l')
            | ('s', 't')
            | ('s', 'p')
            | ('s', 'k')
    )
}

/// Splits long Latin words (`>= 6` alphabetic characters, `>= 2` vowel nuclei) into natural
/// phonetic syllables so line-synced lyrics (LRCLIB / Apple Music Line) ripple upward
/// syllable-by-syllable with `is_part_of_word` just like native Apple Music TTML.
///
/// Lossless invariant: `split_word_into_syllables(word).concat() == word`.
pub fn split_word_into_syllables(word: &str) -> Vec<String> {
    let chars: Vec<char> = word.chars().collect();
    let alpha_count = chars.iter().filter(|c| c.is_alphabetic()).count();
    let all_latin = chars
        .iter()
        .all(|c| !c.is_alphabetic() || c.is_ascii_alphabetic() || is_latin_vowel(*c));
    if alpha_count < 6 || !all_latin {
        return vec![word.to_string()];
    }

    // Locate contiguous vowel nuclei (keeps diphthongs like `ai`, `au`, `ou`, `ea` together).
    let mut nuclei: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_latin_vowel(chars[i]) {
            let start = i;
            while i < chars.len() && is_latin_vowel(chars[i]) {
                i += 1;
            }
            nuclei.push((start, i));
        } else {
            i += 1;
        }
    }

    // Drop silent English trailing `e` (e.g. `teenage`, `someone`) when it's the final alphabetic
    // char preceded by a consonant, so it stays attached to the previous syllable.
    if nuclei.len() >= 2 {
        if let Some(&(last_start, last_end)) = nuclei.last() {
            let is_single_e = last_end == last_start + 1
                && chars[last_start].eq_ignore_ascii_case(&'e');
            let trailing_alpha = chars[last_end..].iter().any(|c| c.is_alphabetic());
            if is_single_e && !trailing_alpha {
                nuclei.pop();
            }
        }
    }

    if nuclei.len() < 2 {
        return vec![word.to_string()];
    }

    let mut cuts: Vec<usize> = Vec::with_capacity(nuclei.len() - 1);
    for pair in nuclei.windows(2) {
        let prev_end = pair[0].1;
        let next_start = pair[1].0;
        let cons_len = next_start.saturating_sub(prev_end);
        let cut = match cons_len {
            0 => next_start,
            1 => prev_end,
            2 => {
                if is_onset_digraph(chars[prev_end], chars[prev_end + 1]) {
                    prev_end
                } else {
                    prev_end + 1
                }
            }
            _ => {
                if next_start >= 2
                    && is_onset_digraph(chars[next_start - 2], chars[next_start - 1])
                {
                    next_start - 2
                } else {
                    next_start - 1
                }
            }
        };
        if cut > cuts.last().copied().unwrap_or(0) && cut < chars.len() {
            cuts.push(cut);
        }
    }

    if cuts.is_empty() {
        return vec![word.to_string()];
    }

    let mut out = Vec::with_capacity(cuts.len() + 1);
    let mut start = 0usize;
    for cut in cuts {
        if cut > start {
            out.push(chars[start..cut].iter().collect());
            start = cut;
        }
    }
    if start < chars.len() {
        out.push(chars[start..].iter().collect());
    }
    if out.is_empty() {
        vec![word.to_string()]
    } else {
        out
    }
}

/// Return real enhanced-LRC / TTML word timings if present, or synthesize Apple-Music-style
/// syllable/word timings across the line's vocal window so line-synced tracks (LRCLIB and
/// line-only Apple Music) deliver feathered Bloom sweep, per-syllable upward float, and
/// sustained-ending Flame emphasis.
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
        .filter(|&t| t > start_ms);

    let vocal_span = if let Some(exact_end) = line.end_ms.filter(|&e| e > start_ms) {
        exact_end.saturating_sub(start_ms).max(400)
    } else {
        let resolved_next = next_ms.unwrap_or_else(|| {
            let fallback = start_ms.saturating_add((tokens.len() as u64 * 620).clamp(1_800, 5_600));
            if track_duration_ms > start_ms {
                track_duration_ms.min(fallback)
            } else {
                fallback
            }
        });
        let raw_span = resolved_next.saturating_sub(start_ms).max(400);
        // Cap synthesized vocal duration so long instrumental breaks don't stretch short lines,
        // while keeping normal 3-6s sung phrases smoothly swept until near the next line.
        let max_vocal_ms = (tokens.len() as u64 * 820).clamp(2_000, 6_400);
        ((raw_span as f32 * 0.90) as u64)
            .min(max_vocal_ms)
            .max(raw_span.min(400))
    };

    let total_tokens = tokens.len();
    let split_tokens: Vec<Vec<String>> = tokens
        .iter()
        .map(|t| split_word_into_syllables(t))
        .collect();

    // Phrase-aware word weights: longer/multi-syllable words take proportionally more time,
    // punctuation adds a subtle breath pause, and the final word of a sustained phrase receives
    // a musical hold boost so sustained endings naturally trigger Spicy Lyrics' Flame glow/swell.
    let weights: Vec<f32> = tokens
        .iter()
        .enumerate()
        .map(|(i, tok)| {
            let alpha_len = tok.chars().filter(|c| c.is_alphabetic()).count().max(1) as f32;
            let syl_count = split_tokens[i].len() as f32;
            let mut w = syl_count * 1.5 + alpha_len * 0.45 + 0.7;
            if tok.ends_with([',', ';', '—', '?', '!']) {
                w += 0.55;
            }
            if i + 1 == total_tokens && total_tokens >= 2 && vocal_span >= 2_400 {
                w *= 1.45;
            }
            w
        })
        .collect();
    let total_weight: f32 = weights.iter().sum::<f32>().max(1.0);

    let total_syls: usize = split_tokens.iter().map(Vec::len).sum();
    let mut out = Vec::with_capacity(total_syls);
    let mut cursor = start_ms as f32;
    let line_end_ms = start_ms.saturating_add(vocal_span);

    for (w_idx, (syls, w_weight)) in split_tokens.into_iter().zip(weights).enumerate() {
        let word_dur = (vocal_span as f32) * (w_weight / total_weight);
        let is_last_word = w_idx + 1 == total_tokens;
        let syl_count = syls.len();

        let syl_weights: Vec<f32> = syls
            .iter()
            .enumerate()
            .map(|(s_idx, s)| {
                let mut sw = (s.chars().count().max(1) as f32) + 0.6;
                if is_last_word && s_idx + 1 == syl_count && syl_count > 1 {
                    sw *= 1.35;
                }
                sw
            })
            .collect();
        let syl_total: f32 = syl_weights.iter().sum::<f32>().max(1.0);

        for (s_idx, (syl_text, sw)) in syls.into_iter().zip(syl_weights).enumerate() {
            let is_last_syl = s_idx + 1 == syl_count;
            let s_start = cursor.round() as u64;
            cursor += word_dur * (sw / syl_total);
            let s_end = if is_last_word && is_last_syl {
                line_end_ms.max(s_start + 1)
            } else {
                (cursor.round() as u64).max(s_start + 1)
            };
            out.push(LyricsWord {
                start_ms: s_start,
                end_ms: s_end,
                text: syl_text,
                is_part_of_word: !is_last_syl,
                is_background: line.is_background,
            });
        }
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
        if w_idx + 1 < words.len() && !w.is_part_of_word {
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

    fn mk_line(time_ms: u64, text: &str) -> LyricsLine {
        LyricsLine {
            time_ms,
            end_ms: None,
            text: text.into(),
            words: None,
            opposite_aligned: false,
            is_background: false,
        }
    }

    fn mk_word(start_ms: u64, end_ms: u64, text: &str, is_part_of_word: bool) -> LyricsWord {
        LyricsWord {
            start_ms,
            end_ms,
            text: text.into(),
            is_part_of_word,
            is_background: false,
        }
    }

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
            mk_line(5_000, "One"),
            mk_line(12_000, "Two"),
            mk_line(20_000, "Three"),
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

        let mid = line_visual_style_continuous(0.5, LyricsVisualMode::SpicySmooth);
        assert!(mid.opacity < s0.opacity && mid.opacity > s1.opacity);
        assert!(mid.scale < s0.scale && mid.scale > s1.scale);

        let mut sp = LyricsSpringState::new();
        sp.notify_active_item(Some(0));
        assert_eq!(sp.visual_active_idx(), Some(0.0));
        sp.notify_active_item(Some(1));
        for _ in 0..10 {
            sp.update(1.0 / 60.0, 80.0);
        }
        let v_mid = sp.visual_active_idx().unwrap();
        assert!(v_mid > 0.1 && v_mid < 0.99, "visual_active_idx should glide smoothly, got {v_mid}");
        for _ in 0..200 {
            sp.update(1.0 / 60.0, 80.0);
        }
        assert!((sp.visual_active_idx().unwrap() - 1.0).abs() < 0.01);
        assert!(sp.is_settled());
    }

    #[test]
    fn word_progress_clamps_between_zero_and_one() {
        let word = mk_word(1000, 2000, "Hello", false);
        assert_eq!(word_progress(&word, 500), 0.0);
        assert!((word_progress(&word, 1500) - 0.5).abs() < 0.001);
        assert_eq!(word_progress(&word, 2500), 1.0);
    }

    #[test]
    fn resolve_line_words_synthesizes_monotonic_spans_and_caps_long_breaks() {
        let lines = vec![
            mk_line(10_000, "And I miss my lover, man"),
            mk_line(40_000, "Next line"),
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
        let lines = vec![mk_line(12_000, "First line"), mk_line(30_000, "(Ooh yeah)")];
        let items = build_display_timeline(&lines);
        assert!(matches!(items[0], LyricsDisplayItem::Interlude { start_ms: 0, end_ms: 12_000, .. }));
        assert!(matches!(items[1], LyricsDisplayItem::Line { line_idx: 0, is_bg_vocal: false, .. }));
        assert!(matches!(items[2], LyricsDisplayItem::Interlude { end_ms: 30_000, .. }));
        assert!(matches!(items[3], LyricsDisplayItem::Line { line_idx: 1, is_bg_vocal: true, .. }));
    }

    #[test]
    fn build_display_timeline_clamps_lrclib_line_end_to_next_line_start() {
        // 8-word fast line (raw estimate 4160ms) followed only 2200ms later by the next line.
        let lines = vec![
            mk_line(16_000, "One second you hate me, then it's false alarm"),
            mk_line(18_200, "That's just what you always do, every time"),
        ];
        let items = build_display_timeline(&lines);
        let first_line = items
            .iter()
            .find(|it| matches!(it, LyricsDisplayItem::Line { line_idx: 0, .. }))
            .unwrap();
        match *first_line {
            LyricsDisplayItem::Line {
                start_ms, end_ms, ..
            } => {
                assert_eq!(start_ms, 16_000);
                assert_eq!(
                    end_ms, 18_200,
                    "LRCLIB line end_ms must never bleed past next line start"
                );
            }
            _ => unreachable!(),
        }
    }

    #[test]
    fn bloom_sweep_runs_reconstructs_full_text_and_respects_part_of_word() {
        let words = vec![
            mk_word(1000, 1400, "Kau", false),
            mk_word(1400, 1800, "da", true),
            mk_word(1800, 2200, "tang", false),
            mk_word(2200, 3200, "te", true),
            mk_word(3200, 4400, "pat", false),
        ];
        let runs = bloom_sweep_runs(&words, 2_000);
        let reconstructed: String = runs.iter().map(|(s, _)| s.as_str()).collect();
        assert_eq!(reconstructed, "Kau datang tepat");
        assert!(runs.first().unwrap().1 > 0.9, "sung prefix must be bright");
        assert!(runs.last().unwrap().1 <= 0.40, "unsung suffix must be dimmed");
    }

    #[test]
    fn plan_word_groups_merges_syllables_and_triggers_flame_on_sustained_words() {
        let words = vec![
            mk_word(14_483, 14_773, "Kau", false),
            mk_word(14_773, 15_214, "da", true),
            mk_word(15_214, 15_401, "tang", false),
            mk_word(17_399, 17_609, "te", true),
            mk_word(17_609, 18_648, "pat", false), // combined "tepat": 5 letters, 1249ms -> Flame!
        ];
        let groups = plan_word_groups(&words, false);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].syllable_range, 0..1);
        assert!(!groups[0].is_flame_split);
        assert_eq!(groups[1].syllable_range, 1..3);
        assert_eq!(groups[1].total_letters, 6);
        assert!(!groups[1].is_flame_split); // 628ms < 1000ms
        assert_eq!(groups[2].syllable_range, 3..5);
        assert_eq!(groups[2].total_letters, 5);
        assert!(groups[2].is_flame_split); // 1249ms >= 1000ms, 5 letters in 2..=7
    }

    #[test]
    fn refreshed_flame_swells_glows_and_pushes_neighbors_symmetrically() {
        let widths = [0.6, 0.55, 0.6, 0.55, 0.45];
        let states = compute_refreshed_flame(&widths, 10_000.0, 1_800.0, 10_600.0);
        assert_eq!(states.len(), 5);
        assert!(states[0].scale > 1.0, "first letter should swell");
        assert!(states[0].glow > 0.0, "active flame letter should glow");
        assert!(states[0].y_em < 0.0, "active flame letter should lift upward");
        assert!(states[0].x_em <= 0.0, "left letters push left");
        assert!(states[4].x_em >= 0.0, "right letters push right");
    }

    #[test]
    fn line_sweep_edges_follow_syllable_timing_and_wait_across_gaps() {
        let words = vec![
            mk_word(1000, 1500, "te", true),
            mk_word(2500, 3500, "pat", false),
        ];
        let spans = vec![
            SweepSpanMetrics {
                strip_x: 0.0,
                width: 40.0,
                word_letters: 5,
                is_split: false,
                row_start: true,
                row_end: false,
            },
            SweepSpanMetrics {
                strip_x: 40.0,
                width: 60.0,
                word_letters: 5,
                is_split: false,
                row_start: false,
                row_end: true,
            },
        ];
        // Mid-gap at 2000ms: first syllable done, second not started -> edge waits near end of "te".
        let gap_edges = compute_line_sweep_edges(&spans, &words, 2000.0, 32.0);
        assert!(gap_edges[0] >= 24.0 && gap_edges[0] <= 56.0);
        // Mid-second syllable at 3000ms: edge moves cleanly into "pat".
        let mid2_edges = compute_line_sweep_edges(&spans, &words, 3000.0, 32.0);
        assert!(mid2_edges[1] > gap_edges[0]);
    }

    #[test]
    fn interlude_dot_states_light_up_sequentially() {
        let early = interlude_dot_states(0, 9_000, 1_500);
        assert!(early[0].0 > early[1].0 && early[1].0 <= 0.30);
        let late = interlude_dot_states(0, 9_000, 8_500);
        assert!(late[0].0 > 0.99 && late[1].0 > 0.99 && late[2].0 > 0.85);

        let bloom_mid = bloom_interlude_state(0, 9_000, 4_500);
        assert!(bloom_mid.dot_opacities[0] > bloom_mid.dot_opacities[2]);
        assert!(bloom_mid.group_opacity > 0.9);
    }

    #[test]
    fn target_scroll_y_tracks_variable_line_heights_and_keeps_active_centered() {
        let lines = vec![
            mk_line(0, "Short"),
            mk_line(
                4_000,
                "A much longer lyric line that will definitely wrap across multiple visual lines on a narrow window",
            ),
            mk_line(20_000, "You found me"),
        ];
        let timeline = build_display_timeline(&lines);
        assert_eq!(timeline.len(), 4);

        let y0 = target_scroll_y_for_item(&timeline, &lines, 0, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);
        let y1 = target_scroll_y_for_item(&timeline, &lines, 1, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);
        let y2 = target_scroll_y_for_item(&timeline, &lines, 2, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);
        let y3 = target_scroll_y_for_item(&timeline, &lines, 3, LyricsVisualMode::SpicySmooth, 1180.0, 760.0);

        assert!(y0 <= 40.0, "first line starts cleanly near center without overshooting: {y0}");
        assert!(y1 > y0 && y2 > y1 && y3 > y2, "offsets strictly increase: {y0}, {y1}, {y2}, {y3}");
        assert!((y2 - y1) > (y3 - y2), "wrapped lyric advances more than interlude");
    }

    #[test]
    fn splits_long_latin_words_into_lossless_syllables() {
        assert_eq!(split_word_into_syllables("sementara"), vec!["se", "men", "ta", "ra"]);
        assert_eq!(split_word_into_syllables("pelangi"), vec!["pe", "la", "ngi"]);
        assert_eq!(split_word_into_syllables("selamanya"), vec!["se", "la", "ma", "nya"]);
        assert_eq!(split_word_into_syllables("daughter"), vec!["daugh", "ter"]);
        assert_eq!(split_word_into_syllables("teenage"), vec!["tee", "nage"]);
        assert_eq!(split_word_into_syllables("lover,"), vec!["lover,"]);
        for sample in ["persinggahanmu", "sementara,", "waiting", "beautiful", "Cinta"] {
            assert_eq!(split_word_into_syllables(sample).concat(), sample);
        }
    }

    #[test]
    fn resolve_line_words_honors_exact_end_ms_and_activates_sustained_flame_on_ending() {
        let mut line = mk_line(30_529, "Menyapa dan pelangi");
        line.end_ms = Some(35_029); // 4.5s exact vocal span
        let words = resolve_line_words(&[line], 0, 220_000);

        assert_eq!(words.first().unwrap().start_ms, 30_529);
        assert_eq!(words.last().unwrap().end_ms, 35_029);

        // Groups reconstruct the 3 original words without splitting mid-word, while animating
        // "Menyapa" and "pelangi" syllable-by-syllable with sustained Flame emphasis on "pelangi".
        let groups = plan_word_groups(&words, false);
        assert_eq!(groups.len(), 3);
        assert!(
            groups[2].is_flame_split,
            "sustained final word 'pelangi' should trigger Flame emphasis (dur={}ms)",
            groups[2].end_ms - groups[2].start_ms
        );
    }
}
