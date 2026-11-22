use std::collections::HashMap;

use iced::widget::{button, column, container, image, mouse_area, row, slider, space, text};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{PlaybackStatus, RepeatMode};

use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    cover_art_widget, floating_dock_style, ghost_button_style, glass_button_style, icon_heart,
    icon_lyrics, icon_next, icon_pause, icon_play, icon_prev, icon_queue, icon_repeat,
    icon_shuffle, icon_volume, play_circle_button_style, seek_slider_style, ICON_HEART_ACTIVE,
    ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_ON_PILL, TEXT_PRIMARY, TEXT_SECONDARY,
};

fn format_mm_ss(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn view_player_bar<'a>(
    info: NowPlayingInfo<'a>,
    art_handles: &'a HashMap<String, image::Handle>,
    queue_open: bool,
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

    let left_track_info = mouse_area(
        row![
            cover_art_widget(art, 52.0, 12.0),
            column![
                row![
                    text(title).size(14).color(TEXT_PRIMARY),
                    container(text(badge).size(10).color(TEXT_SECONDARY))
                        .padding([2, 8])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
                            border: iced::border::rounded(10.0),
                            ..Default::default()
                        }),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text(artist).size(12).color(TEXT_SECONDARY),
            ]
            .spacing(3),
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics));

    let like_btn: Element<'a, Message> = if let Some(t) = track {
        button(if t.is_liked {
            icon_heart(true, 16.0, ICON_HEART_ACTIVE)
        } else {
            icon_heart(false, 16.0, ICON_MUTED)
        })
        .padding([8, 8])
        .style(ghost_button_style)
        .on_press(Message::ToggleLike(t.clone()))
        .into()
    } else {
        space().width(0).into()
    };

    let is_playing = matches!(
        info.status,
        PlaybackStatus::Playing | PlaybackStatus::Buffering
    );

    let play_icon = if is_playing {
        icon_pause(16.0, TEXT_ON_PILL)
    } else {
        icon_play(16.0, TEXT_ON_PILL)
    };

    let repeat_color = if info.repeat != RepeatMode::Off {
        TEXT_ON_PILL
    } else {
        ICON_MUTED
    };

    let max_dur = info.duration_ms.max(1) as f32;
    let cur_pos = (info.position_ms.min(info.duration_ms.max(1))) as f32;

    let center_controls = column![
        row![
            button(icon_shuffle(
                14.0,
                if info.shuffle { TEXT_ON_PILL } else { ICON_MUTED }
            ))
            .padding([7, 9])
            .style(glass_button_style(info.shuffle))
            .on_press(Message::ToggleShuffle),
            button(icon_prev(15.0, ICON_LIGHT))
                .padding([8, 10])
                .style(ghost_button_style)
                .on_press(Message::PreviousTrack),
            button(
                container(play_icon)
                    .width(Length::Fixed(40.0))
                    .height(Length::Fixed(40.0))
                    .center_x(Length::Fixed(40.0))
                    .center_y(Length::Fixed(40.0)),
            )
            .padding(0)
            .style(play_circle_button_style)
            .on_press(Message::TogglePlayPause),
            button(icon_next(15.0, ICON_LIGHT))
                .padding([8, 10])
                .style(ghost_button_style)
                .on_press(Message::NextTrack),
            button(
                row![
                    icon_repeat(14.0, repeat_color),
                    if info.repeat == RepeatMode::One {
                        Element::from(text("1").size(10).color(repeat_color))
                    } else {
                        Element::from(space().width(0))
                    },
                ]
                .spacing(2)
                .align_y(Alignment::Center),
            )
            .padding([7, 9])
            .style(glass_button_style(info.repeat != RepeatMode::Off))
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
            .style(seek_slider_style)
            .width(Length::Fill),
            text(format_mm_ss(info.duration_ms))
                .size(11)
                .color(TEXT_MUTED),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .width(Length::FillPortion(2));

    let right_controls = row![
        like_btn,
        button(
            row![icon_lyrics(14.0, ICON_LIGHT), text("Lyrics").size(12)]
                .spacing(6)
                .align_y(Alignment::Center),
        )
        .padding([7, 12])
        .style(glass_button_style(false))
        .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics)),
        button(icon_queue(
            15.0,
            if queue_open { TEXT_ON_PILL } else { ICON_LIGHT }
        ))
        .padding([8, 10])
        .style(glass_button_style(queue_open))
        .on_press(Message::ToggleQueueDrawer),
        button(icon_volume(info.muted, 15.0, ICON_LIGHT))
            .padding([8, 8])
            .style(ghost_button_style)
            .on_press(Message::ToggleMute),
        slider(
            0.0..=1.0,
            if info.muted { 0.0 } else { info.volume },
            Message::SetVolume
        )
        .step(0.02_f32)
        .style(seek_slider_style)
        .width(Length::Fixed(88.0)),
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
            .spacing(18)
            .padding([12, 20])
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .style(floating_dock_style)
        .width(Length::Fill),
    )
    .padding(iced::Padding {
        top: 8.0,
        right: 16.0,
        bottom: 14.0,
        left: 16.0,
    })
    .width(Length::Fill)
    .into()
}
