use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::{tree, Tree, Widget};
use iced::advanced::Shell;
use iced::alignment;
use iced::mouse;
use iced::{Color, Element, Event, Font, Length, Pixels, Point, Rectangle, Size};
use lume_core::LyricsWord;

use crate::lyrics_anim::{
    compute_line_sweep_edges, compute_refreshed_flame, plan_word_groups, refreshed_lift,
    SweepSpanMetrics, BLOOM_LIT_ALPHA, BLOOM_TIME_OFFSET_MS, BLOOM_UNLIT_ALPHA,
    LYRICS_LINE_HEIGHT_MULT, REFRESHED_LIFT_EM, REFRESHED_SWEEP_FADE_EM,
};

/// Number of nested cumulative alpha passes across the feathered sweep edge.
/// Nested left-anchored clips completely eliminate adjacent-slice seam/barcode artifacts
/// under fractional Windows DPI scaling (125% / 150%).
const SWEEP_FEATHER_STEPS: usize = 20;

/// Custom GPU-accelerated Spicy Lyrics Bloom renderer widget for an active (or settling)
/// syllable-synced lyric line.
///
/// Faithfully reproduces the reverse-engineered `Spikerko/spicy-lyrics` Bloom mechanics:
/// - True per-syllable timing with indivisible `is_part_of_word` word-groups.
/// - Sub-pixel feathered horizontal light sweep (`--SLR_FadeWidth: 0.882em`) clipped directly
///   across glyphs via `clip_bounds`.
/// - Monotonic per-syllable upward float (`RefreshedLift`: `-0.0588em`, `omega=5`, `damping=0.93`).
/// - Letter-by-letter sustained Flame emphasis (`>= 1000ms`, 2–7 letters) with per-letter
///   swell, symmetric sideways push, staggered lift, and soft white glow halo.
/// - Duet right-alignment (`opposite_aligned`) and background vocal scaling/dimming.
pub struct SpicyLyricLine<Message> {
    words: Vec<LyricsWord>,
    pos_ms: u64,
    font_size: f32,
    font: Font,
    is_bg_vocal: bool,
    opposite_aligned: bool,
    enable_bloom_fx: bool,
    opacity_mult: f32,
    on_press: Option<Message>,
}

impl<Message> SpicyLyricLine<Message> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        words: Vec<LyricsWord>,
        pos_ms: u64,
        font_size: f32,
        font: Font,
        is_bg_vocal: bool,
        opposite_aligned: bool,
        enable_bloom_fx: bool,
        opacity_mult: f32,
    ) -> Self {
        Self {
            words,
            pos_ms,
            font_size,
            font,
            is_bg_vocal,
            opposite_aligned,
            enable_bloom_fx,
            opacity_mult: opacity_mult.clamp(0.0, 1.0),
            on_press: None,
        }
    }

    pub fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }
}

struct CachedLetter<P> {
    ch: String,
    paragraph: P,
    local_x: f32,
    width: f32,
}

struct CachedSyllable<P> {
    word_idx: usize,
    paragraph: P,
    /// Visual coordinates inside the widget bounds.
    x: f32,
    y: f32,
    width: f32,
    /// Width including any trailing inter-word space (for continuous line sweep strip).
    advance_width: f32,
    /// Continuous X offset along the unwrapped sweep strip.
    strip_x: f32,
    row_idx: usize,
    row_start: bool,
    row_end: bool,
    word_letters: usize,
    is_flame_split: bool,
    /// Populated only for `is_flame_split` syllables.
    letters: Vec<CachedLetter<P>>,
}

struct CachedFlameGroup {
    syllable_range: std::ops::Range<usize>,
    start_ms: f32,
    duration_ms: f32,
    letter_widths_em: Vec<f32>,
}

struct SpicyLineState<P> {
    cache_key: (u64, u32, u32, bool, bool),
    total_size: Size,
    line_height_px: f32,
    syllables: Vec<CachedSyllable<P>>,
    flame_groups: Vec<CachedFlameGroup>,
}

impl<P> Default for SpicyLineState<P> {
    fn default() -> Self {
        Self {
            cache_key: (0, 0, 0, false, false),
            total_size: Size::ZERO,
            line_height_px: 40.0,
            syllables: Vec::new(),
            flame_groups: Vec::new(),
        }
    }
}

fn hash_words(words: &[LyricsWord]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for w in words {
        for b in w.text.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h ^= w.start_ms;
        h = h.wrapping_mul(0x100000001b3);
        h ^= w.end_ms;
        h = h.wrapping_mul(0x100000001b3);
        h ^= (w.is_part_of_word as u64) | ((w.is_background as u64) << 1);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn make_single_line_paragraph<P: text::Paragraph<Font = Font>>(
    content: &str,
    font_size: f32,
    font: Font,
) -> P {
    P::with_text(text::Text {
        content,
        bounds: Size::new(f32::INFINITY, f32::INFINITY),
        size: Pixels(font_size),
        line_height: text::LineHeight::Relative(LYRICS_LINE_HEIGHT_MULT),
        font,
        align_x: text::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::None,
    })
}

fn measure_space_width<P: text::Paragraph<Font = Font>>(font_size: f32, font: Font) -> f32 {
    let a = make_single_line_paragraph::<P>("a a", font_size, font).min_width();
    let b = make_single_line_paragraph::<P>("aa", font_size, font).min_width();
    let diff = a - b;
    if diff > font_size * 0.12 && diff < font_size * 0.65 {
        diff
    } else {
        font_size * 0.27
    }
}

impl<P: text::Paragraph<Font = Font>> SpicyLineState<P> {
    fn rebuild_if_needed(
        &mut self,
        words: &[LyricsWord],
        font_size: f32,
        font: Font,
        max_width: f32,
        is_bg_vocal: bool,
        opposite_aligned: bool,
    ) {
        let clamped_w = max_width.max(font_size * 4.0);
        let key = (
            hash_words(words),
            (font_size * 10.0).round() as u32,
            clamped_w.round() as u32,
            is_bg_vocal,
            opposite_aligned,
        );
        if self.cache_key == key && !self.syllables.is_empty() {
            return;
        }
        self.cache_key = key;
        self.syllables.clear();
        self.flame_groups.clear();

        let line_h = font_size * LYRICS_LINE_HEIGHT_MULT;
        self.line_height_px = line_h;

        if words.is_empty() {
            self.total_size = Size::new(clamped_w, line_h);
            return;
        }

        let space_w = measure_space_width::<P>(font_size, font);
        let groups = plan_word_groups(words, is_bg_vocal);

        // Top headroom so upward lift (`-0.0588em`) and Flame letter swell (`+14%`) never clip.
        let top_pad = (font_size * 0.14).ceil();
        let bottom_pad = (font_size * 0.08).ceil();

        let mut cursor_x = 0.0_f32;
        let mut row_idx = 0usize;
        let mut row_widths: Vec<f32> = vec![0.0];

        for (g_idx, group) in groups.iter().enumerate() {
            let mut group_paras: Vec<(usize, P, f32, Vec<CachedLetter<P>>)> =
                Vec::with_capacity(group.syllable_range.len());
            let mut group_w = 0.0_f32;
            let mut flame_widths_em: Vec<f32> = Vec::new();

            for w_idx in group.syllable_range.clone() {
                let w = &words[w_idx];
                let para = make_single_line_paragraph::<P>(&w.text, font_size, font);
                let pw = para.min_width().max(1.0);

                let mut letters = Vec::new();
                if group.is_flame_split {
                    let mut lx = 0.0_f32;
                    for ch in w.text.chars() {
                        let s = ch.to_string();
                        let lp = make_single_line_paragraph::<P>(&s, font_size, font);
                        let lw = lp.min_width().max(font_size * 0.18);
                        let w_em = lw / font_size.max(1.0);
                        flame_widths_em.push(w_em);
                        letters.push(CachedLetter {
                            ch: s,
                            paragraph: lp,
                            local_x: lx,
                            width: lw,
                        });
                        lx += lw;
                    }
                    // Rescale letter advance slightly to match exact shaped syllable width so kerning stays tight.
                    if lx > 0.0 && (lx - pw).abs() > 0.2 {
                        let ratio = pw / lx;
                        let mut acc = 0.0_f32;
                        for lt in &mut letters {
                            lt.local_x = acc;
                            lt.width *= ratio;
                            acc += lt.width;
                        }
                    }
                }

                group_w += pw;
                group_paras.push((w_idx, para, pw, letters));
            }

            // Wrap entire word-group to next row if it overflows `clamped_w`.
            if cursor_x > 0.0 && cursor_x + group_w > clamped_w {
                row_idx += 1;
                cursor_x = 0.0;
                row_widths.push(0.0);
            }

            let syl_start_idx = self.syllables.len();
            let has_trailing_group = g_idx + 1 < groups.len();
            let last_in_group = group_paras.len().saturating_sub(1);

            for (local_i, (w_idx, para, pw, letters)) in group_paras.into_iter().enumerate() {
                let trailing_space = if local_i == last_in_group && has_trailing_group {
                    space_w
                } else {
                    0.0
                };
                self.syllables.push(CachedSyllable {
                    word_idx: w_idx,
                    paragraph: para,
                    x: cursor_x,
                    y: top_pad + (row_idx as f32) * line_h,
                    width: pw,
                    advance_width: pw + trailing_space,
                    strip_x: 0.0,
                    row_idx,
                    row_start: false,
                    row_end: false,
                    word_letters: group.total_letters,
                    is_flame_split: group.is_flame_split,
                    letters,
                });
                cursor_x += pw;
            }

            row_widths[row_idx] = cursor_x;
            if has_trailing_group {
                cursor_x += space_w;
            }

            if group.is_flame_split && !flame_widths_em.is_empty() {
                self.flame_groups.push(CachedFlameGroup {
                    syllable_range: syl_start_idx..self.syllables.len(),
                    start_ms: group.start_ms as f32,
                    duration_ms: group.end_ms.saturating_sub(group.start_ms).max(1) as f32,
                    letter_widths_em: flame_widths_em,
                });
            }
        }

        // Compute `row_start` / `row_end`, linear `strip_x`, and duet right-alignment.
        let mut strip_cursor = 0.0_f32;
        let total_syls = self.syllables.len();
        for i in 0..total_syls {
            let is_start = i == 0 || self.syllables[i].row_idx != self.syllables[i - 1].row_idx;
            let is_end =
                i + 1 == total_syls || self.syllables[i].row_idx != self.syllables[i + 1].row_idx;
            self.syllables[i].row_start = is_start;
            self.syllables[i].row_end = is_end;
            self.syllables[i].strip_x = strip_cursor;
            let adv = if is_end {
                self.syllables[i].width
            } else {
                self.syllables[i].advance_width
            };
            self.syllables[i].advance_width = adv;
            strip_cursor += adv;

            if opposite_aligned {
                let rw = row_widths[self.syllables[i].row_idx];
                self.syllables[i].x += (clamped_w - rw).max(0.0);
            }
        }

        let total_rows = row_widths.len().max(1) as f32;
        self.total_size = Size::new(clamped_w, top_pad + total_rows * line_h + bottom_pad);
    }
}

fn intersect_rect(a: Rectangle, b: Rectangle) -> Option<Rectangle> {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.width).min(b.x + b.width);
    let y2 = (a.y + a.height).min(b.y + b.height);
    if x2 > x1 && y2 > y1 {
        Some(Rectangle {
            x: x1,
            y: y1,
            width: x2 - x1,
            height: y2 - y1,
        })
    } else {
        None
    }
}

enum GlyphSource<'a, P> {
    Paragraph(&'a P),
    ScaledText {
        content: &'a str,
        font_size: f32,
        font: Font,
    },
}

fn fill_glyph<Renderer>(
    renderer: &mut Renderer,
    source: &GlyphSource<'_, Renderer::Paragraph>,
    pos: Point,
    color: Color,
    clip: Rectangle,
) where
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    match source {
        GlyphSource::Paragraph(paragraph) => {
            renderer.fill_paragraph(paragraph, pos, color, clip);
        }
        GlyphSource::ScaledText {
            content,
            font_size,
            font,
        } => {
            renderer.fill_text(
                text::Text {
                    content: (*content).to_string(),
                    bounds: Size::new(f32::INFINITY, f32::INFINITY),
                    size: Pixels(*font_size),
                    line_height: text::LineHeight::Relative(LYRICS_LINE_HEIGHT_MULT),
                    font: *font,
                    align_x: text::Alignment::Left,
                    align_y: alignment::Vertical::Top,
                    shaping: text::Shaping::Advanced,
                    wrapping: text::Wrapping::None,
                },
                pos,
                color,
                clip,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_feathered_glyph<Renderer>(
    renderer: &mut Renderer,
    source: &GlyphSource<'_, Renderer::Paragraph>,
    pos: Point,
    width: f32,
    height: f32,
    edge_local: f32,
    fade_px: f32,
    unlit_alpha: f32,
    lit_alpha: f32,
    glow_strength: f32,
    viewport: &Rectangle,
) where
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    let box_rect = Rectangle {
        x: pos.x - 6.0,
        y: pos.y - 8.0,
        width: width + 12.0,
        height: height + 16.0,
    };
    let Some(base_clip) = intersect_rect(box_rect, *viewport) else {
        return;
    };

    let half_fade = fade_px * 0.5;
    let lit_end = edge_local - half_fade;
    let fade_end = edge_local + half_fade;

    // Fast path 1: completely unlit
    if !edge_local.is_finite() || fade_end <= 0.0 {
        fill_glyph(
            renderer,
            source,
            pos,
            Color::from_rgba(1.0, 1.0, 1.0, unlit_alpha),
            base_clip,
        );
        return;
    }

    // Fast path 2: completely lit
    if lit_end >= width {
        if glow_strength > 0.02 {
            draw_soft_glow(
                renderer,
                source,
                pos,
                glow_strength,
                (height * 0.045).clamp(1.2, 2.8),
                base_clip,
            );
        }
        fill_glyph(
            renderer,
            source,
            pos,
            Color::from_rgba(1.0, 1.0, 1.0, lit_alpha),
            base_clip,
        );
        return;
    }

    // Pass 1: subtle soft white aura behind the currently sweeping syllable.
    if glow_strength > 0.02 {
        let glow_right =
            (pos.x + edge_local.clamp(0.0, width) + 6.0).min(base_clip.x + base_clip.width);
        if glow_right > base_clip.x {
            let glow_clip = Rectangle {
                x: base_clip.x,
                y: base_clip.y,
                width: glow_right - base_clip.x,
                height: base_clip.height,
            };
            draw_soft_glow(
                renderer,
                source,
                pos,
                glow_strength,
                (height * 0.045).clamp(1.2, 2.6),
                glow_clip,
            );
        }
    }

    // Pass 2: full unlit base glyph (ensures zero gaps at any fractional DPI scale factor).
    fill_glyph(
        renderer,
        source,
        pos,
        Color::from_rgba(1.0, 1.0, 1.0, unlit_alpha),
        base_clip,
    );

    // Pass 3: nested left-anchored cumulative alpha passes across `[lit_end .. fade_end]`.
    // Every clip starts at `pos.x - 6.0` and ends at decreasing `cut_x` values moving from
    // `fade_end` toward `lit_end`. Because passes are strictly nested rather than adjacent,
    // fractional DPI scissor rounding can never create bright double-blended seams or dark gaps.
    // Exact `src_over` alpha delta to step from `prev_alpha` to `target_alpha`:
    //   delta = (target - prev) / (1 - prev)
    let mut prev_alpha = unlit_alpha;
    let mut full_width_target: Option<f32> = None;

    for step in 1..=SWEEP_FEATHER_STEPS {
        let t = (step as f32) / (SWEEP_FEATHER_STEPS as f32);
        // Smoothstep ramp from unlit (t=0 at fade_end) to lit (t=1 at lit_end).
        let smooth = t * t * (3.0 - 2.0 * t);
        let target_alpha = unlit_alpha + (lit_alpha - unlit_alpha) * smooth;
        // Cut boundary moves right-to-left from `fade_end` (step 1) to `lit_end` (step N).
        let frac_from_right = (step as f32 - 0.5) / (SWEEP_FEATHER_STEPS as f32);
        let cut_local = fade_end - frac_from_right * fade_px;

        if cut_local <= 0.0 {
            // Remaining cuts are even further left (off the syllable entirely).
            break;
        }

        if cut_local >= width {
            // Merge all passes that cover the full glyph width into a single draw call.
            full_width_target = Some(target_alpha);
            continue;
        }

        // Flush any merged full-width target first before narrowing the nested clip.
        if let Some(full_target) = full_width_target.take() {
            let denom = (1.0 - prev_alpha).max(1e-4);
            let delta = ((full_target - prev_alpha) / denom).clamp(0.0, 1.0);
            if delta > 0.002 {
                fill_glyph(
                    renderer,
                    source,
                    pos,
                    Color::from_rgba(1.0, 1.0, 1.0, delta),
                    base_clip,
                );
                prev_alpha = full_target;
            }
        }

        let denom = (1.0 - prev_alpha).max(1e-4);
        let delta = ((target_alpha - prev_alpha) / denom).clamp(0.0, 1.0);
        if delta <= 0.002 {
            continue;
        }

        let nested_rect = Rectangle {
            x: pos.x - 6.0,
            y: box_rect.y,
            width: cut_local + 6.0,
            height: box_rect.height,
        };
        if let Some(clip) = intersect_rect(nested_rect, base_clip) {
            fill_glyph(
                renderer,
                source,
                pos,
                Color::from_rgba(1.0, 1.0, 1.0, delta),
                clip,
            );
            prev_alpha = target_alpha;
        }
    }

    // Flush if every step covered >= width (edge just finished leaving the right side).
    if let Some(full_target) = full_width_target.take() {
        let denom = (1.0 - prev_alpha).max(1e-4);
        let delta = ((full_target - prev_alpha) / denom).clamp(0.0, 1.0);
        if delta > 0.002 {
            fill_glyph(
                renderer,
                source,
                pos,
                Color::from_rgba(1.0, 1.0, 1.0, delta),
                base_clip,
            );
        }
    }
}

fn draw_soft_glow<Renderer>(
    renderer: &mut Renderer,
    source: &GlyphSource<'_, Renderer::Paragraph>,
    pos: Point,
    strength: f32,
    radius: f32,
    clip: Rectangle,
) where
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    let alpha = (strength * 0.16).clamp(0.0, 0.24);
    if alpha < 0.008 {
        return;
    }
    let color = Color::from_rgba(1.0, 1.0, 1.0, alpha);
    let offsets = [
        (-radius, 0.0),
        (radius, 0.0),
        (0.0, -radius * 0.8),
        (0.0, radius * 0.8),
    ];
    for (dx, dy) in offsets {
        fill_glyph(renderer, source, Point::new(pos.x + dx, pos.y + dy), color, clip);
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for SpicyLyricLine<Message>
where
    Message: Clone,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font>,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SpicyLineState<Renderer::Paragraph>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SpicyLineState::<Renderer::Paragraph>::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree
            .state
            .downcast_mut::<SpicyLineState<Renderer::Paragraph>>();
        let max_w = limits.max().width;
        state.rebuild_if_needed(
            &self.words,
            self.font_size,
            self.font,
            max_w,
            self.is_bg_vocal,
            self.opposite_aligned,
        );
        layout::Node::new(limits.resolve(Length::Fill, Length::Shrink, state.total_size))
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if let Some(msg) = &self.on_press {
            if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
                if cursor.is_over(layout.bounds()) {
                    shell.publish(msg.clone());
                    shell.capture_event();
                }
            }
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.on_press.is_some() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        if intersect_rect(bounds, *viewport).is_none() {
            return;
        }

        let state = tree
            .state
            .downcast_ref::<SpicyLineState<Renderer::Paragraph>>();
        if state.syllables.is_empty() || self.words.is_empty() {
            return;
        }

        let processed_ms = (self.pos_ms as f32) + BLOOM_TIME_OFFSET_MS;
        let fade_px = (self.font_size * REFRESHED_SWEEP_FADE_EM * 1.25).max(14.0);

        let spans: Vec<SweepSpanMetrics> = state
            .syllables
            .iter()
            .map(|s| SweepSpanMetrics {
                strip_x: s.strip_x,
                width: s.advance_width,
                word_letters: s.word_letters,
                is_split: s.is_flame_split,
                row_start: s.row_start,
                row_end: s.row_end,
            })
            .collect();
        let edges = compute_line_sweep_edges(&spans, &self.words, processed_ms, fade_px);

        let bg_dim = if self.is_bg_vocal { 0.82 } else { 1.0 };
        let unlit_alpha = (BLOOM_UNLIT_ALPHA * bg_dim * self.opacity_mult).clamp(0.04, 1.0);
        let lit_alpha = (BLOOM_LIT_ALPHA * bg_dim * self.opacity_mult).clamp(0.08, 1.0);

        // Precompute Flame letter states once per FlameGroup (mirrors Spicy Lyrics `_groupFlames`).
        let flame_states: Vec<Vec<crate::lyrics_anim::FlameLetterState>> = if self.enable_bloom_fx {
            state
                .flame_groups
                .iter()
                .map(|fg| {
                    compute_refreshed_flame(
                        &fg.letter_widths_em,
                        fg.start_ms,
                        fg.duration_ms,
                        processed_ms,
                    )
                })
                .collect()
        } else {
            Vec::new()
        };

        // Monotonic float cap: mirrors `LyricsAnimator.ts:1506` so an out-of-order syllable
        // never lifts ahead of the preceding syllable.
        let mut float_cap = 1.0_f32;

        for (s_idx, syl) in state.syllables.iter().enumerate() {
            let word = &self.words[syl.word_idx];
            let edge_strip = edges.get(s_idx).copied().unwrap_or(f32::NEG_INFINITY);

            if syl.is_flame_split && self.enable_bloom_fx && !syl.letters.is_empty() {
                // Find the owning FlameGroup and letter offset.
                let group_match = state
                    .flame_groups
                    .iter()
                    .enumerate()
                    .find(|(_, fg)| fg.syllable_range.contains(&s_idx));

                let (group_idx, letter_offset) = match group_match {
                    Some((gi, fg)) => {
                        let offset: usize = state.syllables[fg.syllable_range.start..s_idx]
                            .iter()
                            .map(|prev| prev.letters.len())
                            .sum();
                        (Some(gi), offset)
                    }
                    None => (None, 0),
                };

                for (l_idx, lt) in syl.letters.iter().enumerate() {
                    let fl = group_idx
                        .and_then(|gi| flame_states.get(gi))
                        .and_then(|arr| arr.get(letter_offset + l_idx));

                    let (scale, x_em, y_em, glow) = match fl {
                        Some(f) => (f.scale, f.x_em, f.y_em, f.glow),
                        None => (1.0, 0.0, 0.0, 0.0),
                    };

                    let scaled_font = self.font_size * scale;
                    let scale_dx = -((scale - 1.0) * lt.width) * 0.5;
                    let scale_dy = -((scale - 1.0) * state.line_height_px) * 0.45;
                    let draw_x = bounds.x + syl.x + lt.local_x + x_em * self.font_size + scale_dx;
                    let draw_y = bounds.y + syl.y + y_em * self.font_size + scale_dy;
                    let letter_edge_local = edge_strip - (syl.strip_x + lt.local_x);

                    // Compute letter lit tone for Flame glow modulation (`setLetterLit`).
                    let lit_t = if letter_edge_local.is_finite() {
                        ((letter_edge_local + fade_px * 0.5) / (lt.width + fade_px)).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let effective_glow = glow * lit_t * self.opacity_mult;

                    if (scale - 1.0).abs() > 0.004 {
                        // Use owned `fill_text` (`Text::Cached`) for dynamically scaled Flame letters:
                        // `iced_wgpu`'s `fill_paragraph` stores a `Weak<Paragraph>`, so a local
                        // temporary `Paragraph` would be dropped before GPU prepare and vanish!
                        let q_font = (scaled_font * 4.0).round() / 4.0;
                        let sw = (lt.width * scale).max(lt.width);
                        let src = GlyphSource::ScaledText {
                            content: lt.ch.as_str(),
                            font_size: q_font,
                            font: self.font,
                        };
                        draw_feathered_glyph(
                            renderer,
                            &src,
                            Point::new(draw_x, draw_y),
                            sw,
                            q_font * LYRICS_LINE_HEIGHT_MULT,
                            letter_edge_local * scale,
                            fade_px * scale,
                            unlit_alpha,
                            lit_alpha,
                            effective_glow * 1.45,
                            viewport,
                        );
                    } else {
                        let src = GlyphSource::Paragraph(&lt.paragraph);
                        draw_feathered_glyph(
                            renderer,
                            &src,
                            Point::new(draw_x, draw_y),
                            lt.width,
                            state.line_height_px,
                            letter_edge_local,
                            fade_px,
                            unlit_alpha,
                            lit_alpha,
                            effective_glow * 1.45,
                            viewport,
                        );
                    }
                }
            } else {
                let lift_y = if self.enable_bloom_fx {
                    let lift = refreshed_lift(processed_ms - word.start_ms as f32);
                    float_cap = float_cap.min(lift);
                    -REFRESHED_LIFT_EM * float_cap * self.font_size
                } else {
                    0.0
                };

                let edge_local = edge_strip - syl.strip_x;
                let active_glow = if self.enable_bloom_fx
                    && processed_ms >= word.start_ms as f32
                    && processed_ms <= (word.end_ms as f32 + 240.0)
                {
                    let dur = (word.end_ms.saturating_sub(word.start_ms).max(1)) as f32;
                    let p = ((processed_ms - word.start_ms as f32) / dur).clamp(0.0, 1.0);
                    // Smooth bell glow peaking while this syllable is actively sung.
                    (std::f32::consts::PI * p).sin().max(0.0) * 0.45 * self.opacity_mult
                } else {
                    0.0
                };

                let src = GlyphSource::Paragraph(&syl.paragraph);
                draw_feathered_glyph(
                    renderer,
                    &src,
                    Point::new(bounds.x + syl.x, bounds.y + syl.y + lift_y),
                    syl.width,
                    state.line_height_px,
                    edge_local,
                    fade_px,
                    unlit_alpha,
                    lit_alpha,
                    active_glow,
                    viewport,
                );
            }
        }
    }
}

impl<'a, Message, Theme, Renderer> From<SpicyLyricLine<Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Renderer: iced::advanced::Renderer + text::Renderer<Font = Font> + 'a,
{
    fn from(widget: SpicyLyricLine<Message>) -> Self {
        Self::new(widget)
    }
}
