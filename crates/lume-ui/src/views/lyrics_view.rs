use std::collections::HashMap;

use iced::widget::{
    button, column, container, image, mouse_area, rich_text, row, scrollable, slider, space, span,
    text, Id,
};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{LyricsData, PlaybackStatus, RepeatMode};
use lume_storage::LyricsVisualMode;

use crate::lyrics_anim::{active_line_index, line_visual_style, word_progress};
use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    close_button_style, cover_art_widget, ghost_button_style, glass_button_style,
    primary_pill_button_style, seek_slider_style, thin_scrollable_style, TEXT_PRIMARY,
    TEXT_SECONDARY,
};

pub fn lyrics_scroll_id() -> Id {
    Id::new("lyrics_scroll")
}

pub const LYRIC_LINE_STRIDE_PX: f32 = 54.0;

pub fn format_timestamp_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

pub fn marquee_slice(text: &str, max_chars: usize, time_s: f32) -> String {
    if max_chars == 0 {
        return String::new();
    }
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    let padded: Vec<char> = text.chars().chain("   •   ".chars()).collect();
    let cycle_len = padded.len();
    let speed_chars_per_sec = 4.0_f32;
    let offset = ((time_s.max(0.0) * speed_chars_per_sec) as usize) % cycle_len;
    (0..max_chars)
        .map(|i| padded[(offset + i) % cycle_len])
        .collect()
}

pub struct LyricsViewProps<'a> {
    pub info: NowPlayingInfo<'a>,
    pub lyrics: Option<&'a LyricsData>,
    pub lyrics_loading: bool,
    pub lyrics_mode: LyricsVisualMode,
    pub art_handles: &'a HashMap<String, image::Handle>,
    pub time_s: f32,
    pub maximized: bool,
}

pub fn view_now_playing_lyrics<'a>(props: LyricsViewProps<'a>) -> Element<'a, Message> {
    let top_left = mouse_area(
        row![
            button(text("← Back").size(13).color(TEXT_SECONDARY))
                .padding([6, 10])
                .style(ghost_button_style)
                .on_press(Message::SetViewMode(ViewMode::Browse)),
            button(text("•••").size(14).color(TEXT_SECONDARY))
                .padding([6, 10])
                .style(ghost_button_style)
                .on_press(Message::ToggleQueueDrawer),
            space().width(Length::Fill),
        ]
        .spacing(4)
        .align_y(Alignment::Center)
        .padding([8, 12]),
    )
    .on_press(Message::DragWindow);

    let max_glyph = if props.maximized { "❐" } else { "□" };
    let top_right = container(
        row![
            button(text("─").size(12).color(TEXT_PRIMARY))
                .padding([6, 12])
                .style(ghost_button_style)
                .on_press(Message::MinimizeWindow),
            button(text(max_glyph).size(12).color(TEXT_PRIMARY))
                .padding([6, 12])
                .style(ghost_button_style)
                .on_press(Message::ToggleMaximizeWindow),
            button(text("×").size(14).color(TEXT_PRIMARY))
                .padding([5, 13])
                .style(close_button_style)
                .on_press(Message::CloseRequested),
        ]
        .spacing(2)
        .align_y(Alignment::Center),
    )
    .padding([2, 4])
    .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.25))),
        ..Default::default()
    });

    let top_bar = row![top_left, top_right]
        .width(Length::Fill)
        .align_y(Alignment::Center);

    let track = props.info.track;
    let art = track
        .and_then(|t| t.artwork_ref.as_deref())
        .and_then(|k| props.art_handles.get(k));

    let raw_title = track.map(|t| t.title.as_str()).unwrap_or("Nothing Playing");
    let display_title = marquee_slice(raw_title, 28, props.time_s);
    let artist = track
        .map(|t| t.artist_display())
        .unwrap_or_else(|| "Select a track from your library".to_string());
    let source_badge = props
        .info
        .source_used
        .map(|s| s.badge_label())
        .unwrap_or("Idle");

    let max_dur = props.info.duration_ms.max(1) as f32;
    let cur_pos = (props.info.position_ms.min(props.info.duration_ms.max(1))) as f32;

    let play_glyph = match props.info.status {
        PlaybackStatus::Playing | PlaybackStatus::Buffering => "⏸",
        PlaybackStatus::Paused | PlaybackStatus::Stopped => "▶",
    };

    let repeat_label = match props.info.repeat {
        RepeatMode::Off => "Repeat",
        RepeatMode::All => "All",
        RepeatMode::One => "One",
    };

    // Left column matching reference-lyrics-view.jpg:
    // Crisp square cover art, thin seekbar + M:SS timestamps right underneath,
    // bold centered marquee title, softer artist subtitle, source badge & compact controls.
    let left_column = container(
        column![
            cover_art_widget(art, 356.0, 8.0),
            column![
                slider(0.0..=max_dur, cur_pos, |v| {
                    Message::SeekSliderChanged(v as u64)
                })
                .step(250.0_f32)
                .style(seek_slider_style)
                .width(Length::Fixed(356.0)),
                row![
                    text(format_timestamp_ms(props.info.position_ms))
                        .size(12)
                        .color(TEXT_SECONDARY),
                    space().width(Length::Fill),
                    text(format_timestamp_ms(props.info.duration_ms))
                        .size(12)
                        .color(TEXT_SECONDARY),
                ]
                .width(Length::Fixed(356.0)),
            ]
            .spacing(4),
            column![
                text(display_title).size(24).color(TEXT_PRIMARY),
                text(artist).size(15).color(TEXT_SECONDARY),
            ]
            .spacing(4)
            .align_x(Alignment::Center)
            .width(Length::Fixed(356.0)),
            row![
                container(text(source_badge).size(11).color(TEXT_SECONDARY))
                    .padding([3, 10])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
                        border: iced::border::rounded(12.0),
                        ..Default::default()
                    }),
                if let Some(t) = track {
                    Element::from(
                        button(text(if t.is_liked { "♥" } else { "♡" }).size(14))
                            .padding([4, 10])
                            .style(ghost_button_style)
                            .on_press(Message::ToggleLike(t.clone())),
                    )
                } else {
                    Element::from(space().width(0))
                }
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            row![
                button(text("Shuffle").size(11))
                    .padding([6, 10])
                    .style(glass_button_style(props.info.shuffle))
                    .on_press(Message::ToggleShuffle),
                button(text("⏮").size(15))
                    .padding([6, 12])
                    .style(ghost_button_style)
                    .on_press(Message::PreviousTrack),
                button(text(play_glyph).size(16))
                    .padding([8, 16])
                    .style(glass_button_style(true))
                    .on_press(Message::TogglePlayPause),
                button(text("⏭").size(15))
                    .padding([6, 12])
                    .style(ghost_button_style)
                    .on_press(Message::NextTrack),
                button(text(repeat_label).size(11))
                    .padding([6, 10])
                    .style(glass_button_style(props.info.repeat != RepeatMode::Off))
                    .on_press(Message::CycleRepeat),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        ]
        .spacing(16)
        .align_x(Alignment::Center),
    )
    .width(Length::FillPortion(5))
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill);

    let right_column = build_lyrics_column(&props);

    let body = row![left_column, right_column]
        .spacing(28)
        .padding([12, 48])
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center);

    column![top_bar, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn build_lyrics_column<'a>(props: &LyricsViewProps<'a>) -> Element<'a, Message> {
    if props.lyrics_loading {
        return container(
            column![
                text("Searching lyrics...").size(22).color(TEXT_PRIMARY),
                text("Checking SQLite cache, local tags & LRCLIB")
                    .size(13)
                    .color(TEXT_SECONDARY),
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        )
        .width(Length::FillPortion(6))
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    }

    let Some(lyrics) = props.lyrics else {
        return container(
            column![
                text("No lyrics found").size(24).color(TEXT_PRIMARY),
                text("No synced or plain lyrics are available for this track yet.")
                    .size(14)
                    .color(TEXT_SECONDARY),
                button(text("↻  Retry").size(13))
                    .padding([9, 18])
                    .style(primary_pill_button_style)
                    .on_press(Message::RetryLyrics),
            ]
            .spacing(14)
            .align_x(Alignment::Center),
        )
        .width(Length::FillPortion(6))
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .into();
    };

    if !lyrics.synced.is_empty() {
        let active_idx = active_line_index(&lyrics.synced, props.info.position_ms);
        let mut lines_col = column![space().height(160.0)].spacing(12).width(Length::Fill);

        for (idx, line) in lyrics.synced.iter().enumerate() {
            let distance = match active_idx {
                Some(active) => idx as isize - active as isize,
                None => idx as isize + 1,
            };
            let style = line_visual_style(distance, props.lyrics_mode);
            let font_size = 24.0 * style.scale;
            let is_active = distance == 0;

            let line_content: Element<'a, Message> = if let Some(words) = &line.words {
                let spans: Vec<iced::widget::text::Span<'a, ()>> = words
                    .iter()
                    .enumerate()
                    .map(|(w_idx, word)| {
                        let prog = if is_active {
                            word_progress(word, props.info.position_ms)
                        } else if distance < 0 {
                            1.0
                        } else {
                            0.0
                        };
                        let alpha = if is_active {
                            0.42 + 0.58 * prog
                        } else {
                            style.opacity
                        };
                        let suffix = if w_idx + 1 < words.len() { " " } else { "" };
                        span(format!("{}{}", word.text, suffix))
                            .size(font_size)
                            .color(Color::from_rgba(1.0, 1.0, 1.0, alpha))
                    })
                    .collect();
                rich_text(spans).into()
            } else {
                let text_label = if line.text.is_empty() {
                    "• • •"
                } else {
                    line.text.as_str()
                };
                let color = if is_active && style.glow > 0.0 {
                    Color::from_rgba(1.0, 0.98, 0.94, style.opacity)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, style.opacity)
                };
                text(text_label).size(font_size).color(color).into()
            };

            let clickable = mouse_area(
                container(line_content)
                    .padding([4, 8])
                    .width(Length::Fill),
            )
            .on_press(Message::SeekTo(line.time_ms));

            lines_col = lines_col.push(clickable);
        }

        lines_col = lines_col.push(space().height(200.0));

        return container(
            scrollable(lines_col)
                .id(lyrics_scroll_id())
                .height(Length::Fill)
                .width(Length::Fill)
                .style(thin_scrollable_style),
        )
        .width(Length::FillPortion(6))
        .height(Length::Fill)
        .padding([12, 8])
        .into();
    }

    if let Some(plain) = &lyrics.plain {
        return container(
            scrollable(
                container(text(plain.as_str()).size(19).color(TEXT_PRIMARY)).padding([24, 12]),
            )
            .height(Length::Fill)
            .style(thin_scrollable_style),
        )
        .width(Length::FillPortion(6))
        .height(Length::Fill)
        .into();
    }

    container(
        column![
            text("No lyrics found").size(24).color(TEXT_PRIMARY),
            button(text("↻  Retry").size(13))
                .padding([9, 18])
                .style(primary_pill_button_style)
                .on_press(Message::RetryLyrics),
        ]
        .spacing(12)
        .align_x(Alignment::Center),
    )
    .width(Length::FillPortion(6))
    .height(Length::Fill)
    .center_x(Length::Fill)
    .center_y(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_timestamps_matching_reference_style() {
        assert_eq!(format_timestamp_ms(115_000), "1:55");
        assert_eq!(format_timestamp_ms(231_000), "3:51");
        assert_eq!(format_timestamp_ms(5_000), "0:05");
    }

    #[test]
    fn marquee_returns_original_when_within_limit_and_scrolls_when_overflowing() {
        assert_eq!(marquee_slice("Short Title", 20, 4.0), "Short Title");

        let long = "Young And Beautiful - DH Orchestral Version";
        let at_start = marquee_slice(long, 20, 0.0);
        let later = marquee_slice(long, 20, 5.0);
        assert_eq!(at_start.chars().count(), 20);
        assert_eq!(later.chars().count(), 20);
        assert_ne!(at_start, later);
    }
}
