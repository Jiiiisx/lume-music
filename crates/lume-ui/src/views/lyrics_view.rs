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
    close_button_style, cover_art_widget, ghost_button_style, glass_button_style, icon_back,
    icon_close, icon_heart, icon_maximize, icon_minimize, icon_more, icon_next, icon_pause,
    icon_play, icon_prev, icon_refresh, icon_repeat, icon_shuffle, play_circle_button_style,
    primary_pill_button_style, seek_slider_style, thin_scrollable_style, ICON_HEART_ACTIVE,
    ICON_LIGHT, ICON_MUTED, TEXT_ON_PILL, TEXT_PRIMARY, TEXT_SECONDARY,
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
            button(
                row![icon_back(14.0, ICON_LIGHT), text("Library").size(13)]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .padding([6, 12])
            .style(glass_button_style(false))
            .on_press(Message::SetViewMode(ViewMode::Browse)),
            button(icon_more(16.0, ICON_LIGHT))
                .padding([7, 10])
                .style(glass_button_style(false))
                .on_press(Message::ToggleQueueDrawer),
            space().width(Length::Fill),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .padding([10, 14]),
    )
    .on_press(Message::DragWindow);

    let top_right = container(
        row![
            button(icon_minimize(13.0, ICON_LIGHT))
                .padding([7, 11])
                .style(ghost_button_style)
                .on_press(Message::MinimizeWindow),
            button(icon_maximize(props.maximized, 13.0, ICON_LIGHT))
                .padding([7, 11])
                .style(ghost_button_style)
                .on_press(Message::ToggleMaximizeWindow),
            button(icon_close(13.0, ICON_LIGHT))
                .padding([7, 12])
                .style(close_button_style)
                .on_press(Message::CloseRequested),
        ]
        .spacing(2)
        .align_y(Alignment::Center),
    )
    .padding(2)
    .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
        border: iced::border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    });

    let top_bar = row![top_left, top_right]
        .padding(iced::Padding {
            top: 0.0,
            right: 14.0,
            bottom: 0.0,
            left: 0.0,
        })
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
    let source_badge = props.info.source_used.map(|s| s.badge_label()).unwrap_or("Idle");

    let max_dur = props.info.duration_ms.max(1) as f32;
    let cur_pos = (props.info.position_ms.min(props.info.duration_ms.max(1))) as f32;

    let is_playing = matches!(
        props.info.status,
        PlaybackStatus::Playing | PlaybackStatus::Buffering
    );
    let play_icon = if is_playing {
        icon_pause(18.0, TEXT_ON_PILL)
    } else {
        icon_play(18.0, TEXT_ON_PILL)
    };
    let repeat_color = if props.info.repeat != RepeatMode::Off {
        TEXT_ON_PILL
    } else {
        ICON_MUTED
    };

    let left_column = container(
        column![
            cover_art_widget(art, 356.0, 14.0),
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
                    .padding([4, 12])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.14))),
                        border: iced::border::rounded(14.0),
                        ..Default::default()
                    }),
                if let Some(t) = track {
                    Element::from(
                        button(if t.is_liked {
                            icon_heart(true, 16.0, ICON_HEART_ACTIVE)
                        } else {
                            icon_heart(false, 16.0, ICON_LIGHT)
                        })
                        .padding([6, 10])
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
                button(icon_shuffle(
                    15.0,
                    if props.info.shuffle {
                        TEXT_ON_PILL
                    } else {
                        ICON_MUTED
                    }
                ))
                .padding([8, 10])
                .style(glass_button_style(props.info.shuffle))
                .on_press(Message::ToggleShuffle),
                button(icon_prev(17.0, ICON_LIGHT))
                    .padding([9, 12])
                    .style(ghost_button_style)
                    .on_press(Message::PreviousTrack),
                button(
                    container(play_icon)
                        .width(Length::Fixed(46.0))
                        .height(Length::Fixed(46.0))
                        .center_x(Length::Fixed(46.0))
                        .center_y(Length::Fixed(46.0)),
                )
                .padding(0)
                .style(play_circle_button_style)
                .on_press(Message::TogglePlayPause),
                button(icon_next(17.0, ICON_LIGHT))
                    .padding([9, 12])
                    .style(ghost_button_style)
                    .on_press(Message::NextTrack),
                button(
                    row![
                        icon_repeat(15.0, repeat_color),
                        if props.info.repeat == RepeatMode::One {
                            Element::from(text("1").size(10).color(repeat_color))
                        } else {
                            Element::from(space().width(0))
                        },
                    ]
                    .spacing(2)
                    .align_y(Alignment::Center),
                )
                .padding([8, 10])
                .style(glass_button_style(props.info.repeat != RepeatMode::Off))
                .on_press(Message::CycleRepeat),
            ]
            .spacing(10)
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
                button(
                    row![icon_refresh(14.0, TEXT_ON_PILL), text("Try again").size(13)]
                        .spacing(8)
                        .align_y(Alignment::Center),
                )
                .padding([10, 20])
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
            button(
                row![icon_refresh(14.0, TEXT_ON_PILL), text("Try again").size(13)]
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .padding([10, 20])
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
