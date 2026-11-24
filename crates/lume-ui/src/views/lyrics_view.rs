use std::collections::HashMap;

use iced::alignment::Horizontal;
use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, slider, space, text, Id,
};
use iced::{Alignment, Background, Color, Element, Length, Padding, Shadow, Vector};
use lume_core::{LyricsData, PlaybackStatus, RepeatMode, Track};
use lume_storage::LyricsVisualMode;

use crate::lyrics_anim::{
    active_display_index, bloom_interlude_state, interlude_dot_states, line_visual_style,
    lyrics_bottom_spacer_px, lyrics_top_spacer_px, lyrics_viewport_metrics, resolve_line_words,
    LyricsDisplayItem, LYRICS_INTERLUDE_HEIGHT_PX, LYRICS_ITEM_SPACING_PX, LYRICS_LINE_HEIGHT_MULT,
};
use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::queue_drawer::view_inline_queue;
use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_elevated_style,
    hidden_scrollbar, icon_back, icon_heart, icon_lyrics, icon_more, icon_music, icon_next,
    icon_pause, icon_play, icon_prev, icon_queue, icon_refresh, icon_repeat, icon_shuffle,
    icon_volume, play_circle_button_style, primary_pill_button_style, seek_slider_style,
    thin_scrollable_style, traffic_light_button_style, SpicyLyricLine, ICON_HEART_ACTIVE,
    ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn lyrics_scroll_id() -> Id {
    Id::new("lyrics_scroll")
}

pub const LYRIC_LINE_STRIDE_PX: f32 = 78.0;

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
    pub lyrics_timeline: &'a [LyricsDisplayItem],
    pub lyrics_loading: bool,
    pub lyrics_mode: LyricsVisualMode,
    pub active_scale: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub active_cover: Option<&'a image::Handle>,
    pub art_handles: &'a HashMap<String, image::Handle>,
    pub time_s: f32,
    pub queue_open: bool,
    pub queue_tracks: Vec<&'a Track>,
    pub queue_current_pos: Option<usize>,
    pub status_banner: Option<&'a str>,
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
                text("Library").size(12).font(FONT_MEDIUM)
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
        container(space().width(Length::Fill).height(Length::Fixed(34.0)))
            .width(Length::Fill)
            .height(Length::Fixed(34.0)),
    )
    .on_press(Message::DragWindow);

    let track = props.info.track;
    let lyrics_status_pill = if track.is_none() {
        "Standby".to_string()
    } else if props.lyrics_loading {
        "Searching...".to_string()
    } else if let Some(data) = props.lyrics {
        let provider_label = match data.provider.as_str() {
            "apple" => "Apple Music",
            "musixmatch" => "Musixmatch",
            "qq" => "QQ Music",
            "lyricsplus" => "LyricsPlus",
            "spotify" => "Spotify",
            "netease" => "NetEase",
            "sidecar_ttml" => "TTML",
            "sidecar_lrc" => "Local .lrc",
            "embedded" => "Embedded",
            "lrclib" | "lrclib_checked" | "lrclib_v2" => "LRCLIB",
            other => other,
        };
        let has_native_words = data.has_native_syllables();
        if !data.synced.is_empty() {
            if has_native_words {
                format!("Syllable • {provider_label}")
            } else {
                format!("Bloom • {provider_label}")
            }
        } else if data.plain.is_some() {
            format!("Plain • {provider_label}")
        } else {
            "Unavailable".to_string()
        }
    } else {
        "Unavailable".to_string()
    };

    let top_right = row![
        container(
            text(lyrics_status_pill)
                .size(10)
                .font(FONT_MEDIUM)
                .color(TEXT_SECONDARY),
        )
        .padding([5, 11])
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
            border: iced::border::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                width: 1.0,
                radius: 10.0.into(),
            },
            ..Default::default()
        }),
        button(icon_refresh(13.0, ICON_MUTED))
            .padding([6, 8])
            .style(ghost_button_style)
            .on_press(Message::RetryLyrics),
        button(
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
        .on_press(Message::ToggleQueueDrawer),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let top_bar = container(
        row![top_left, top_center, top_right]
            .spacing(12)
            .padding([6, 20])
            .width(Length::Fill)
            .height(Length::Fixed(46.0))
            .align_y(Alignment::Center),
    )
    .width(Length::Fill);

    let art = props.active_cover.or_else(|| {
        track
            .and_then(|t| t.artwork_ref.as_deref())
            .and_then(|k| props.art_handles.get(k))
    });

    let raw_title = track.map(|t| t.title.as_str()).unwrap_or("Nothing Playing");
    let display_title = marquee_slice(raw_title, 28, props.time_s);
    let artist = track
        .map(|t| t.artist_display())
        .unwrap_or_else(|| "Select a track from your library".to_string());
    let album_label = track
        .and_then(|t| t.album.as_deref())
        .filter(|a| !a.is_empty());
    let source_badge = if props.info.status == PlaybackStatus::Buffering {
        "Buffering..."
    } else {
        props
            .info
            .source_used
            .map(|s| s.badge_label())
            .unwrap_or("Idle")
    };

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

    const ART_SIZE: f32 = 332.0;

    let framed_art = container(cover_art_widget(art, ART_SIZE, 20.0)).style(|_| container::Style {
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.42),
            offset: Vector::new(0.0, 18.0),
            blur_radius: 42.0,
        },
        border: iced::border::rounded(20.0),
        ..Default::default()
    });

    let mut meta_text = column![
        text(display_title)
            .size(21)
            .font(FONT_BOLD)
            .color(TEXT_PRIMARY),
        text(artist)
            .size(14)
            .font(FONT_MEDIUM)
            .color(Color::from_rgba(1.0, 1.0, 1.0, 0.72)),
    ]
    .spacing(3)
    .width(Length::Fill);

    if let Some(album) = album_label {
        meta_text = meta_text.push(
            text(album)
                .size(11)
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.48)),
        );
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
                    icon_heart(true, 17.0, ICON_HEART_ACTIVE)
                } else {
                    icon_heart(false, 17.0, ICON_LIGHT)
                })
                .padding([6, 7])
                .style(ghost_button_style)
                .on_press(Message::ToggleLike(t.clone())),
            )
        } else {
            Element::from(space().width(0))
        },
        if track.is_some() {
            Element::from(
                button(icon_more(16.0, ICON_LIGHT))
                    .padding([6, 7])
                    .style(ghost_button_style)
                    .on_press(Message::TogglePlayerMenu),
            )
        } else {
            Element::from(space().width(0))
        },
    ]
    .spacing(4)
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
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.58)),
            space().width(Length::Fill),
            text(format_timestamp_ms(props.info.duration_ms))
                .size(11)
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.58)),
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
    .spacing(12)
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

    // Frameless Spicy Lyrics cinema left stage: artwork floats directly on the blurred backdrop.
    let left_cinema_stage = container(
        column![
            framed_art,
            space().height(4.0),
            meta_row,
            seek_block,
            transport_row,
            volume_row,
        ]
        .spacing(14)
        .align_x(Alignment::Center),
    )
    .width(Length::Fixed(400.0))
    .height(Length::Fill)
    .padding(Padding {
        top: 12.0,
        right: 12.0,
        bottom: 24.0,
        left: 28.0,
    })
    .align_x(Horizontal::Center)
    .align_y(iced::alignment::Vertical::Center);

    let mut right_col = column![].width(Length::Fill).height(Length::Fill);

    if let Some(toast) = props.status_banner {
        right_col = right_col.push(
            container(
                container(
                    row![
                        text(toast).size(12).color(TEXT_PRIMARY).width(Length::Fill),
                        button(text("Dismiss").size(11))
                            .padding([4, 10])
                            .style(glass_button_style(false, accent))
                            .on_press(Message::DismissToast),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .padding([10, 14])
                .style(glass_elevated_style),
            )
            .padding([4, 24]),
        );
    }

    if props.queue_open {
        right_col = right_col.push(view_inline_queue(
            &props.queue_tracks,
            props.queue_current_pos,
            props.art_handles,
            accent,
        ));
    } else {
        right_col = right_col.push(build_lyrics_column(&props));
    }

    let right_cinema_stage = container(right_col)
        .width(Length::Fill)
        .height(Length::Fill);

    let body = row![left_cinema_stage, right_cinema_stage]
        .spacing(20)
        .padding(Padding {
            top: 0.0,
            right: 20.0,
            bottom: 12.0,
            left: 8.0,
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

fn render_interlude_dots<'a>(
    start_ms: u64,
    end_ms: u64,
    pos_ms: u64,
    is_active: bool,
    fallback_opacity: f32,
    mode: LyricsVisualMode,
) -> Element<'a, Message> {
    let states: [(f32, f32); 3] = if is_active {
        if mode == LyricsVisualMode::SpicySmooth {
            let b = bloom_interlude_state(start_ms, end_ms, pos_ms);
            [
                (b.dot_opacities[0] * b.group_opacity, b.group_scale),
                (b.dot_opacities[1] * b.group_opacity, b.group_scale),
                (b.dot_opacities[2] * b.group_opacity, b.group_scale),
            ]
        } else {
            interlude_dot_states(start_ms, end_ms, pos_ms)
        }
    } else {
        [(fallback_opacity, 0.78); 3]
    };

    let mut dots = row![].spacing(12).align_y(Alignment::Center);
    for (opacity, scale) in states {
        let diameter = (13.0 * scale).clamp(7.0, 19.0);
        let dot_alpha = opacity.clamp(0.0, 1.0);
        let dot = container(space().width(diameter).height(diameter)).style(move |_| {
            container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, dot_alpha))),
                border: iced::border::rounded(diameter * 0.5),
                shadow: if is_active && dot_alpha > 0.55 {
                    Shadow {
                        color: Color::from_rgba(1.0, 1.0, 1.0, (dot_alpha - 0.45) * 0.45),
                        offset: Vector::ZERO,
                        blur_radius: 10.0,
                    }
                } else {
                    Shadow::default()
                },
                ..Default::default()
            }
        });
        dots = dots.push(dot);
    }

    container(dots)
        .height(Length::Fixed(LYRICS_INTERLUDE_HEIGHT_PX))
        .align_y(iced::alignment::Vertical::Center)
        .padding([4, 6])
        .into()
}

fn subtle_lyrics_scrollable_style(
    theme: &iced::Theme,
    status: scrollable::Status,
) -> scrollable::Style {
    thin_scrollable_style(theme, status)
}

fn build_lyrics_column<'a>(props: &LyricsViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;

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
        let phase = props.time_s * 3.4;
        let mut dots = row![].spacing(10).align_y(Alignment::Center);
        for i in 0..3 {
            let wave = ((phase - (i as f32) * 0.65).sin() * 0.5 + 0.5).clamp(0.0, 1.0);
            let alpha = 0.28 + 0.58 * wave;
            let size = 8.0 + 2.5 * wave;
            dots = dots.push(
                container(space().width(size).height(size)).style(move |_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, alpha))),
                    border: iced::border::rounded(size * 0.5),
                    ..Default::default()
                }),
            );
        }

        return container(
            column![
                dots,
                text("Loading lyrics...")
                    .size(14)
                    .font(FONT_MEDIUM)
                    .color(Color::from_rgba(1.0, 1.0, 1.0, 0.62)),
            ]
            .spacing(12)
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
                text("Supports LRCLIB, sidecar .ttml/.lrc files, and embedded audio lyric tags.")
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
        let timeline = props.lyrics_timeline;
        let active_idx = active_display_index(timeline, props.info.position_ms);
        let (_, viewport_h) = lyrics_viewport_metrics(props.window_width, props.window_height);
        let top_spacer = lyrics_top_spacer_px(viewport_h);
        let bottom_spacer = lyrics_bottom_spacer_px(viewport_h);

        let mut lines_col = column![space().height(top_spacer)]
            .spacing(LYRICS_ITEM_SPACING_PX)
            .width(Length::Fill)
            .align_x(Alignment::Start);

        for (display_idx, item) in timeline.iter().enumerate() {
            let distance = match active_idx {
                Some(active) => display_idx as isize - active as isize,
                None => display_idx as isize + 1,
            };
            let style = line_visual_style(distance, props.lyrics_mode);
            let is_active = distance == 0;

            let row_el: Element<'a, Message> = match *item {
                LyricsDisplayItem::Interlude {
                    start_ms,
                    end_ms,
                    opposite_aligned,
                } => {
                    let dot_alpha = if distance < 0 {
                        (style.opacity * 0.55).max(0.08)
                    } else {
                        style.opacity
                    };
                    let dots = render_interlude_dots(
                        start_ms,
                        end_ms,
                        props.info.position_ms,
                        is_active,
                        dot_alpha,
                        props.lyrics_mode,
                    );
                    let h_align = if opposite_aligned {
                        Horizontal::Right
                    } else {
                        Horizontal::Left
                    };
                    mouse_area(container(dots).width(Length::Fill).align_x(h_align))
                        .on_press(Message::SeekTo(start_ms))
                        .into()
                }
                LyricsDisplayItem::Line {
                    line_idx,
                    start_ms,
                    end_ms,
                    is_bg_vocal,
                    opposite_aligned,
                } => {
                    let line = &lyrics.synced[line_idx];
                    let pos_ms = props.info.position_ms;
                    let bloom_fx = props.lyrics_mode == LyricsVisualMode::SpicySmooth;
                    let base_active = if is_bg_vocal { 26.0 } else { 36.0 };
                    let base_idle = if is_bg_vocal { 23.0 } else { 31.0 };

                    // Support overlapping duet / background lines and Bloom's 500ms settling tail
                    // (`RushMs` + Flame settle) so lines never snap abruptly when finishing.
                    let is_overlapping_active =
                        !is_active && pos_ms >= start_ms && pos_ms <= end_ms;
                    let is_settling = !is_active
                        && distance == -1
                        && pos_ms > end_ms
                        && pos_ms <= end_ms.saturating_add(500);

                    let h_align = if opposite_aligned {
                        Horizontal::Right
                    } else {
                        Horizontal::Left
                    };

                    if is_active || is_overlapping_active || is_settling {
                        let resolved_words =
                            resolve_line_words(&lyrics.synced, line_idx, props.info.duration_ms);
                        let settle_opacity = if is_settling {
                            let t = ((pos_ms.saturating_sub(end_ms)) as f32 / 500.0).clamp(0.0, 1.0);
                            1.0 - t * (1.0 - (style.opacity * 0.72).max(0.24))
                        } else if is_overlapping_active {
                            0.94
                        } else {
                            1.0
                        };
                        let active_size = if is_settling {
                            let t = ((pos_ms.saturating_sub(end_ms)) as f32 / 500.0).clamp(0.0, 1.0);
                            base_active + (base_idle * style.scale - base_active) * t
                        } else {
                            base_active
                        };

                        if !resolved_words.is_empty() {
                            container(
                                SpicyLyricLine::new(
                                    resolved_words,
                                    pos_ms,
                                    active_size,
                                    FONT_BOLD,
                                    is_bg_vocal,
                                    opposite_aligned,
                                    bloom_fx,
                                    settle_opacity,
                                )
                                .on_press(Message::SeekTo(start_ms)),
                            )
                            .padding([0, 6])
                            .width(Length::Fill)
                            .into()
                        } else {
                            mouse_area(
                                container(
                                    text(line.text.as_str())
                                        .size(active_size)
                                        .font(FONT_BOLD)
                                        .line_height(LYRICS_LINE_HEIGHT_MULT)
                                        .color(Color::from_rgba(1.0, 1.0, 1.0, settle_opacity))
                                        .align_x(h_align),
                                )
                                .padding([2, 6])
                                .width(Length::Fill)
                                .align_x(h_align),
                            )
                            .on_press(Message::SeekTo(start_ms))
                            .into()
                        }
                    } else {
                        let font_size = base_idle * style.scale;
                        let bg_dim = if is_bg_vocal { 0.78 } else { 1.0 };
                        let alpha = if distance < 0 {
                            (style.opacity * 0.68 * bg_dim).max(0.08)
                        } else {
                            (style.opacity * bg_dim).max(0.08)
                        };
                        mouse_area(
                            container(
                                text(line.text.as_str())
                                    .size(font_size)
                                    .font(FONT_BOLD)
                                    .line_height(LYRICS_LINE_HEIGHT_MULT)
                                    .color(Color::from_rgba(1.0, 1.0, 1.0, alpha))
                                    .align_x(h_align),
                            )
                            .padding([2, 6])
                            .width(Length::Fill)
                            .align_x(h_align),
                        )
                        .on_press(Message::SeekTo(start_ms))
                        .into()
                    }
                }
            };

            lines_col = lines_col.push(row_el);
        }

        lines_col = lines_col.push(space().height(bottom_spacer));

        return container(
            scrollable(lines_col)
                .id(lyrics_scroll_id())
                .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
                .height(Length::Fill)
                .width(Length::Fill)
                .style(subtle_lyrics_scrollable_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([0, 32])
        .into();
    }

    if let Some(plain) = &lyrics.plain {
        return container(
            scrollable(
                container(
                    text(plain.as_str())
                        .size(20)
                        .font(FONT_BOLD)
                        .line_height(1.65)
                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.85))
                        .align_x(Horizontal::Left),
                )
                .width(Length::Fill)
                .padding([28, 36])
                .align_x(Horizontal::Left),
            )
            .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
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
