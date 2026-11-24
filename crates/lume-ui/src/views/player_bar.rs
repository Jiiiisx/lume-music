use std::collections::HashMap;

use iced::widget::{button, column, container, image, mouse_area, row, slider, space, text};
use iced::{Alignment, Background, Element, Length};
use lume_core::{PlaybackStatus, RepeatMode};

use crate::theme::{AccentTheme, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    cover_art_widget, floating_dock_style, ghost_button_style, glass_button_style, icon_heart,
    icon_lyrics, icon_next, icon_pause, icon_play, icon_prev, icon_queue, icon_repeat,
    icon_shuffle, icon_volume, play_circle_button_style, seek_slider_style, ICON_HEART_ACTIVE,
    ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

fn format_mm_ss(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn view_player_bar<'a>(
    info: NowPlayingInfo<'a>,
    art_handles: &'a HashMap<String, image::Handle>,
    queue_open: bool,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let track = info.track;
    let art = track
        .and_then(|t| t.artwork_ref.as_deref())
        .and_then(|k| art_handles.get(k));

    let title = track.map(|t| t.title.as_str()).unwrap_or("Nothing Playing");
    let artist = track
        .map(|t| t.artist_display())
        .unwrap_or_else(|| "Select a track to play".to_string());
    let badge = info.source_used.map(|s| s.badge_label()).unwrap_or("Idle");

    let track_click_area = mouse_area(
        row![
            cover_art_widget(art, 48.0, 10.0),
            column![
                text(title).size(13).font(FONT_SEMIBOLD).color(TEXT_PRIMARY),
                text(artist).size(11).color(TEXT_SECONDARY),
            ]
            .spacing(2),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics));

    let like_btn: Element<'a, Message> = if let Some(t) = track {
        button(if t.is_liked {
            icon_heart(true, 15.0, ICON_HEART_ACTIVE)
        } else {
            icon_heart(false, 15.0, ICON_MUTED)
        })
        .padding([7, 7])
        .style(ghost_button_style)
        .on_press(Message::ToggleLike(t.clone()))
        .into()
    } else {
        space().width(0).into()
    };

    let left_track_info = row![track_click_area, like_btn]
        .spacing(8)
        .align_y(Alignment::Center);

    let is_playing = matches!(
        info.status,
        PlaybackStatus::Playing | PlaybackStatus::Buffering
    );

    let play_icon = if is_playing {
        icon_pause(15.0, accent.on_primary)
    } else {
        icon_play(15.0, accent.on_primary)
    };

    let repeat_color = if info.repeat != RepeatMode::Off {
        accent.on_primary
    } else {
        ICON_MUTED
    };

    let max_dur = info.duration_ms.max(1) as f32;
    let cur_pos = (info.position_ms.min(info.duration_ms.max(1))) as f32;

    let center_controls = column![
        row![
            button(icon_shuffle(
                13.0,
                if info.shuffle {
                    accent.on_primary
                } else {
                    ICON_MUTED
                }
            ))
            .padding([6, 8])
            .style(glass_button_style(info.shuffle, accent))
            .on_press(Message::ToggleShuffle),
            button(icon_prev(15.0, ICON_LIGHT))
                .padding([7, 9])
                .style(ghost_button_style)
                .on_press(Message::PreviousTrack),
            button(
                container(play_icon)
                    .width(Length::Fixed(38.0))
                    .height(Length::Fixed(38.0))
                    .center_x(Length::Fixed(38.0))
                    .center_y(Length::Fixed(38.0)),
            )
            .padding(0)
            .style(play_circle_button_style(accent))
            .on_press(Message::TogglePlayPause),
            button(icon_next(15.0, ICON_LIGHT))
                .padding([7, 9])
                .style(ghost_button_style)
                .on_press(Message::NextTrack),
            button(
                row![
                    icon_repeat(13.0, repeat_color),
                    if info.repeat == RepeatMode::One {
                        Element::from(text("1").size(10).color(repeat_color))
                    } else {
                        Element::from(space().width(0))
                    },
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            )
            .padding([6, 8])
            .style(glass_button_style(info.repeat != RepeatMode::Off, accent))
            .on_press(Message::CycleRepeat),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        row![
            text(format_mm_ss(info.position_ms))
                .size(11)
                .color(TEXT_MUTED),
            slider(0.0..=max_dur, cur_pos, |v| {
                Message::SeekSliderChanged(v as u64)
            })
            .step(250.0_f32)
            .style(seek_slider_style(accent))
            .width(Length::Fill),
            text(format_mm_ss(info.duration_ms))
                .size(11)
                .color(TEXT_MUTED),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    ]
    .spacing(5)
    .align_x(Alignment::Center)
    .width(Length::FillPortion(2));

    let right_controls = row![
        container(text(badge).size(10).font(FONT_SEMIBOLD).color(accent.light))
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
        button(
            row![
                icon_lyrics(13.0, ICON_LIGHT),
                text("Lyrics").size(12).font(FONT_MEDIUM)
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([6, 11])
        .style(glass_button_style(false, accent))
        .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics)),
        button(icon_queue(
            14.0,
            if queue_open {
                accent.on_primary
            } else {
                ICON_LIGHT
            }
        ))
        .padding([7, 9])
        .style(glass_button_style(queue_open, accent))
        .on_press(Message::ToggleQueueDrawer),
        button(icon_volume(info.muted, 15.0, ICON_LIGHT))
            .padding([7, 7])
            .style(ghost_button_style)
            .on_press(Message::ToggleMute),
        slider(
            0.0..=1.0,
            if info.muted { 0.0 } else { info.volume },
            Message::SetVolume
        )
        .step(0.02_f32)
        .style(seek_slider_style(accent))
        .width(Length::Fixed(84.0)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    container(
        container(
            row![
                container(left_track_info).width(Length::FillPortion(1)),
                center_controls,
                container(right_controls)
                    .width(Length::FillPortion(1))
                    .align_x(iced::alignment::Horizontal::Right),
            ]
            .spacing(16)
            .padding([10, 18])
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .style(floating_dock_style)
        .width(Length::Fill),
    )
    .padding(iced::Padding {
        top: 8.0,
        right: 16.0,
        bottom: 12.0,
        left: 16.0,
    })
    .width(Length::Fill)
    .into()
}
