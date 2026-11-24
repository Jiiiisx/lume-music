use std::collections::HashMap;

use iced::alignment::Horizontal;
use iced::widget::{
    button, column, container, image, mouse_area, rich_text, row, scrollable, slider, space, span,
    text, Id,
};
use iced::{Alignment, Background, Color, Element, Length, Padding, Shadow, Vector};
use lume_core::{LyricsData, PlaybackStatus, RepeatMode};
use lume_storage::LyricsVisualMode;

use crate::lyrics_anim::{active_line_index, line_visual_style, word_progress};
use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_panel_style,
    glass_sidebar_style, icon_back, icon_heart, icon_lyrics, icon_music, icon_next, icon_pause,
    icon_play, icon_prev, icon_queue, icon_refresh, icon_repeat, icon_shuffle, icon_volume,
    play_circle_button_style, primary_pill_button_style, seek_slider_style, thin_scrollable_style,
    traffic_light_button_style, GLASS_SUBTLE_BORDER, ICON_HEART_ACTIVE, ICON_LIGHT, ICON_MUTED,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn lyrics_scroll_id() -> Id {
    Id::new("lyrics_scroll")
}

pub const LYRIC_LINE_STRIDE_PX: f32 = 58.0;

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
    pub queue_open: bool,
    pub accent: AccentTheme,
}

fn traffic_light_btn<'a>(
    base: (u8, u8, u8),
    hover: (u8, u8, u8),
    msg: Message,
) -> Element<'a, Message> {
    button(space().width(13).height(13))
        .padding(0)
        .width(Length::Fixed(13.0))
        .height(Length::Fixed(13.0))
        .style(traffic_light_button_style(base, hover))
        .on_press(msg)
        .into()
}

pub fn view_now_playing_lyrics<'a>(props: LyricsViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;

    let traffic_lights = row![
        traffic_light_btn((255, 95, 87), (255, 123, 115), Message::CloseRequested),
        traffic_light_btn((254, 188, 46), (255, 208, 91), Message::MinimizeWindow),
        traffic_light_btn((40, 200, 64), (74, 222, 94), Message::ToggleMaximizeWindow),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let top_left = row![
        traffic_lights,
        button(
            row![
                icon_back(14.0, ICON_LIGHT),
                text("Back to Library").size(12).font(FONT_MEDIUM)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .style(glass_button_style(false, accent))
        .on_press(Message::SetViewMode(ViewMode::Browse)),
    ]
    .spacing(14)
    .align_y(Alignment::Center);

    let top_center = mouse_area(
        container(
            text("NOW PLAYING")
                .size(11)
                .font(FONT_SEMIBOLD)
                .color(TEXT_MUTED),
        )
        .width(Length::Fill)
        .height(Length::Fixed(34.0))
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center),
    )
    .on_press(Message::DragWindow);

    let top_right = button(
        row![
            icon_queue(
                14.0,
                if props.queue_open {
                    accent.on_primary
                } else {
                    ICON_LIGHT
                }
            ),
            text("Queue")
                .size(12)
                .font(if props.queue_open {
                    FONT_SEMIBOLD
                } else {
                    FONT_MEDIUM
                })
                .color(if props.queue_open {
                    accent.on_primary
                } else {
                    TEXT_PRIMARY
                }),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 12])
    .style(glass_button_style(props.queue_open, accent))
    .on_press(Message::ToggleQueueDrawer);

    let top_bar = container(
        row![top_left, top_center, top_right]
            .spacing(12)
            .padding([6, 18])
            .width(Length::Fill)
            .height(Length::Fixed(46.0))
            .align_y(Alignment::Center),
    )
    .width(Length::Fill);

    let track = props.info.track;
    let art = track
        .and_then(|t| t.artwork_ref.as_deref())
        .and_then(|k| props.art_handles.get(k));

    let raw_title = track.map(|t| t.title.as_str()).unwrap_or("Nothing Playing");
    let display_title = marquee_slice(raw_title, 26, props.time_s);
    let artist = track
        .map(|t| t.artist_display())
        .unwrap_or_else(|| "Select a track from your library".to_string());
    let album_label = track
        .and_then(|t| t.album.as_deref())
        .filter(|a| !a.is_empty());
    let source_badge = props.info.source_used.map(|s| s.badge_label()).unwrap_or("Idle");

    let max_dur = props.info.duration_ms.max(1) as f32;
    let cur_pos = (props.info.position_ms.min(props.info.duration_ms.max(1))) as f32;

    let is_playing = matches!(
        props.info.status,
        PlaybackStatus::Playing | PlaybackStatus::Buffering
    );
    let play_icon = if is_playing {
        icon_pause(18.0, accent.on_primary)
    } else {
        icon_play(18.0, accent.on_primary)
    };
    let repeat_color = if props.info.repeat != RepeatMode::Off {
        accent.on_primary
    } else {
        ICON_MUTED
    };

    const ART_SIZE: f32 = 276.0;

    let mut meta_text = column![
        text(display_title)
            .size(19)
            .font(FONT_BOLD)
            .color(TEXT_PRIMARY),
        text(artist)
            .size(13)
            .font(FONT_MEDIUM)
            .color(TEXT_SECONDARY),
    ]
    .spacing(3)
    .width(Length::Fill);

    if let Some(album) = album_label {
        meta_text = meta_text.push(text(album).size(11).color(TEXT_MUTED));
    }

    let meta_actions = row![
        container(
            text(source_badge)
                .size(10)
                .font(FONT_SEMIBOLD)
                .color(accent.light),
        )
        .padding([3, 9])
        .style(move |_| container::Style {
            background: Some(Background::Color(accent.soft_bg)),
            border: iced::border::Border {
                color: accent.border,
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        }),
        if let Some(t) = track {
            Element::from(
                button(if t.is_liked {
                    icon_heart(true, 16.0, ICON_HEART_ACTIVE)
                } else {
                    icon_heart(false, 16.0, ICON_LIGHT)
                })
                .padding([6, 7])
                .style(ghost_button_style)
                .on_press(Message::ToggleLike(t.clone())),
            )
        } else {
            Element::from(space().width(0))
        },
    ]
    .spacing(6)
    .align_y(Alignment::Center);

    let meta_row = row![meta_text, meta_actions]
        .spacing(10)
        .width(Length::Fixed(ART_SIZE))
        .align_y(Alignment::Center);

    let seek_block = column![
        slider(0.0..=max_dur, cur_pos, |v| {
            Message::SeekSliderChanged(v as u64)
        })
        .step(250.0_f32)
        .style(seek_slider_style(accent))
        .width(Length::Fixed(ART_SIZE)),
        row![
            text(format_timestamp_ms(props.info.position_ms))
                .size(11)
                .color(TEXT_MUTED),
            space().width(Length::Fill),
            text(format_timestamp_ms(props.info.duration_ms))
                .size(11)
                .color(TEXT_MUTED),
        ]
        .width(Length::Fixed(ART_SIZE)),
    ]
    .spacing(4);

    let transport_row = row![
        button(icon_shuffle(
            14.0,
            if props.info.shuffle {
                accent.on_primary
            } else {
                ICON_MUTED
            }
        ))
        .padding([8, 10])
        .style(glass_button_style(props.info.shuffle, accent))
        .on_press(Message::ToggleShuffle),
        button(icon_prev(16.0, ICON_LIGHT))
            .padding([9, 11])
            .style(ghost_button_style)
            .on_press(Message::PreviousTrack),
        button(
            container(play_icon)
                .width(Length::Fixed(48.0))
                .height(Length::Fixed(48.0))
                .align_x(Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center),
        )
        .padding(0)
        .style(play_circle_button_style(accent))
        .on_press(Message::TogglePlayPause),
        button(icon_next(16.0, ICON_LIGHT))
            .padding([9, 11])
            .style(ghost_button_style)
            .on_press(Message::NextTrack),
        button(
            row![
                icon_repeat(14.0, repeat_color),
                if props.info.repeat == RepeatMode::One {
                    Element::from(text("1").size(10).font(FONT_SEMIBOLD).color(repeat_color))
                } else {
                    Element::from(space().width(0))
                },
            ]
            .spacing(2)
            .align_y(Alignment::Center),
        )
        .padding([8, 10])
        .style(glass_button_style(
            props.info.repeat != RepeatMode::Off,
            accent,
        ))
        .on_press(Message::CycleRepeat),
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let volume_row = row![
        button(icon_volume(props.info.muted, 14.0, ICON_MUTED))
            .padding([5, 6])
            .style(ghost_button_style)
            .on_press(Message::ToggleMute),
        slider(
            0.0..=1.0,
            if props.info.muted {
                0.0
            } else {
                props.info.volume
            },
            Message::SetVolume,
        )
        .step(0.02_f32)
        .style(seek_slider_style(accent))
        .width(Length::Fill),
    ]
    .spacing(10)
    .width(Length::Fixed(ART_SIZE))
    .align_y(Alignment::Center);

    // Fixed 364px Apple-Music-style Crystal Glass card so artwork stays a crisp 1:1 square
    // and all 5 transport controls never clip or squash.
    let left_player_card = container(
        column![
            cover_art_widget(art, ART_SIZE, 20.0),
            meta_row,
            seek_block,
            transport_row,
            volume_row,
        ]
        .spacing(16)
        .align_x(Alignment::Center),
    )
    .width(Length::Fixed(364.0))
    .height(Length::Fill)
    .padding(24)
    .align_x(Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center)
    .style(glass_sidebar_style);

    let lyrics_status_pill = if track.is_none() {
        "Standby".to_string()
    } else if props.lyrics_loading {
        "Searching...".to_string()
    } else if let Some(data) = props.lyrics {
        let provider_label = match data.provider.as_str() {
            "sidecar_lrc" => "Local .lrc",
            "embedded" => "Embedded",
            "lrclib" => "LRCLIB",
            other => other,
        };
        if !data.synced.is_empty() {
            format!("Synced • {provider_label}")
        } else if data.plain.is_some() {
            format!("Plain • {provider_label}")
        } else {
            "Unavailable".to_string()
        }
    } else {
        "Unavailable".to_string()
    };

    let lyrics_header = column![
        row![
            icon_lyrics(14.0, ICON_MUTED),
            text("LYRICS")
                .size(11)
                .font(FONT_SEMIBOLD)
                .color(TEXT_MUTED),
            space().width(Length::Fill),
            container(
                text(lyrics_status_pill)
                    .size(10)
                    .font(FONT_MEDIUM)
                    .color(TEXT_SECONDARY),
            )
            .padding([3, 10])
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
                border: iced::border::Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }),
        ]
        .spacing(8)
        .padding([14, 22])
        .align_y(Alignment::Center),
        container(space().height(1))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                ..Default::default()
            }),
    ];

    let right_lyrics_panel = container(
        column![lyrics_header, build_lyrics_column(&props)]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(glass_panel_style);

    let body = row![left_player_card, right_lyrics_panel]
        .spacing(16)
        .padding(Padding {
            top: 4.0,
            right: 16.0,
            bottom: 14.0,
            left: 16.0,
        })
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Alignment::Center);

    column![top_bar, body]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn empty_medallion<'a>(
    icon: Element<'a, Message>,
    accent: AccentTheme,
) -> Element<'a, Message> {
    container(icon)
        .width(Length::Fixed(64.0))
        .height(Length::Fixed(64.0))
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_| container::Style {
            background: Some(Background::Color(accent.soft_bg)),
            border: iced::border::Border {
                color: accent.border,
                width: 1.0,
                radius: 32.0.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.20),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 20.0,
            },
            ..Default::default()
        })
        .into()
}

fn build_lyrics_column<'a>(props: &LyricsViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;

    // Smart Empty State #1: No track currently loaded/playing.
    let Some(active_track) = props.info.track else {
        return container(
            column![
                empty_medallion(icon_music(26.0, accent.light), accent),
                text("Nothing playing yet")
                    .size(21)
                    .font(FONT_BOLD)
                    .color(TEXT_PRIMARY),
                text("Pick a song from your Library to follow live synced lyrics here.")
                    .size(13)
                    .color(TEXT_SECONDARY)
                    .align_x(Horizontal::Center),
                button(
                    row![
                        icon_back(13.0, accent.on_primary),
                        text("Browse Library")
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .color(accent.on_primary),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([10, 22])
                .style(primary_pill_button_style(accent))
                .on_press(Message::SetViewMode(ViewMode::Browse)),
            ]
            .spacing(12)
            .max_width(380.0)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(32)
        .into();
    };

    if props.lyrics_loading {
        return container(
            column![
                empty_medallion(icon_lyrics(26.0, accent.light), accent),
                text("Searching lyrics...")
                    .size(21)
                    .font(FONT_BOLD)
                    .color(TEXT_PRIMARY),
                text("Checking SQLite cache, local .lrc files & LRCLIB")
                    .size(13)
                    .color(TEXT_SECONDARY)
                    .align_x(Horizontal::Center),
            ]
            .spacing(10)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(32)
        .into();
    }

    let track_context = format!("{} — {}", active_track.title, active_track.artist_display());

    // Smart Empty State #2: Track is active, but no lyrics were found.
    let render_no_lyrics = || -> Element<'a, Message> {
        container(
            column![
                empty_medallion(icon_lyrics(26.0, ICON_MUTED), accent),
                text("No lyrics found")
                    .size(21)
                    .font(FONT_BOLD)
                    .color(TEXT_PRIMARY),
                text(track_context.clone())
                    .size(13)
                    .font(FONT_MEDIUM)
                    .color(TEXT_SECONDARY)
                    .align_x(Horizontal::Center),
                text("Supports LRCLIB, sidecar .lrc files, and embedded audio lyric tags.")
                    .size(12)
                    .color(TEXT_MUTED)
                    .align_x(Horizontal::Center),
                button(
                    row![
                        icon_refresh(14.0, accent.on_primary),
                        text("Try again")
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .color(accent.on_primary),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .padding([10, 22])
                .style(primary_pill_button_style(accent))
                .on_press(Message::RetryLyrics),
            ]
            .spacing(11)
            .max_width(420.0)
            .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .padding(32)
        .into()
    };

    let Some(lyrics) = props.lyrics else {
        return render_no_lyrics();
    };

    if !lyrics.synced.is_empty() {
        let active_idx = active_line_index(&lyrics.synced, props.info.position_ms);
        let mut lines_col = column![space().height(140.0)]
            .spacing(10)
            .width(Length::Fill)
            .align_x(Alignment::Center);

        let active_tint = Color {
            r: (0.88 + accent.light.r * 0.12).clamp(0.0, 1.0),
            g: (0.88 + accent.light.g * 0.12).clamp(0.0, 1.0),
            b: (0.88 + accent.light.b * 0.12).clamp(0.0, 1.0),
            a: 1.0,
        };

        for (idx, line) in lyrics.synced.iter().enumerate() {
            let distance = match active_idx {
                Some(active) => idx as isize - active as isize,
                None => idx as isize + 1,
            };
            let style = line_visual_style(distance, props.lyrics_mode);
            let font_size = 22.0 * style.scale;
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
                            0.48 + 0.52 * prog
                        } else {
                            style.opacity
                        };
                        let col = if is_active && prog > 0.45 {
                            Color {
                                a: alpha,
                                ..active_tint
                            }
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, alpha)
                        };
                        let suffix = if w_idx + 1 < words.len() { " " } else { "" };
                        span(format!("{}{}", word.text, suffix))
                            .size(font_size)
                            .font(if is_active { FONT_BOLD } else { FONT_MEDIUM })
                            .color(col)
                    })
                    .collect();
                rich_text(spans).align_x(Horizontal::Center).into()
            } else {
                let text_label = if line.text.is_empty() {
                    "•  •  •"
                } else {
                    line.text.as_str()
                };
                let color = if is_active {
                    Color {
                        a: style.opacity,
                        ..active_tint
                    }
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, style.opacity)
                };
                text(text_label)
                    .size(font_size)
                    .font(if is_active { FONT_BOLD } else { FONT_MEDIUM })
                    .color(color)
                    .align_x(Horizontal::Center)
                    .into()
            };

            // Centered Pill Highlight: active line wrapped in a crisp frosted glass pill.
            let line_box = if is_active {
                container(line_content)
                    .padding([12, 24])
                    .max_width(640.0)
                    .align_x(Horizontal::Center)
                    .style(move |_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.095))),
                        border: iced::border::Border {
                            color: Color {
                                a: 0.26,
                                ..accent.light
                            },
                            width: 1.0,
                            radius: 18.0.into(),
                        },
                        shadow: Shadow {
                            color: Color {
                                a: 0.16,
                                ..accent.glow
                            },
                            offset: Vector::new(0.0, 6.0),
                            blur_radius: 22.0,
                        },
                        ..Default::default()
                    })
            } else {
                container(line_content)
                    .padding([6, 20])
                    .max_width(620.0)
                    .align_x(Horizontal::Center)
            };

            let clickable = mouse_area(
                container(line_box)
                    .width(Length::Fill)
                    .align_x(Horizontal::Center),
            )
            .on_press(Message::SeekTo(line.time_ms));

            lines_col = lines_col.push(clickable);
        }

        lines_col = lines_col.push(space().height(180.0));

        return container(
            scrollable(lines_col)
                .id(lyrics_scroll_id())
                .height(Length::Fill)
                .width(Length::Fill)
                .style(thin_scrollable_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([8, 24])
        .into();
    }

    if let Some(plain) = &lyrics.plain {
        return container(
            scrollable(
                container(
                    text(plain.as_str())
                        .size(18)
                        .font(FONT_MEDIUM)
                        .line_height(1.7)
                        .color(TEXT_PRIMARY)
                        .align_x(Horizontal::Center),
                )
                .width(Length::Fill)
                .padding([28, 36])
                .align_x(Horizontal::Center),
            )
            .height(Length::Fill)
            .width(Length::Fill)
            .style(thin_scrollable_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    render_no_lyrics()
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
