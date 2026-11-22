use std::collections::HashMap;

use iced::widget::{button, column, container, image, mouse_area, row, slider, space, text};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{PlaybackStatus, RepeatMode};

use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_panel, seek_slider_style,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
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
    let badge = info
        .source_used
        .map(|s| s.badge_label())
        .unwrap_or("Idle");

    let left_track_info = mouse_area(
        row![
            cover_art_widget(art, 50.0, 8.0),
            column![
                row![
                    text(title).size(14).color(TEXT_PRIMARY),
                    container(text(badge).size(10).color(TEXT_MUTED))
                        .padding([1, 6])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
                            border: iced::border::rounded(6.0),
                            ..Default::default()
                        }),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                text(artist).size(12).color(TEXT_SECONDARY),
            ]
            .spacing(3),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics));

    let like_btn: Element<'a, Message> = if let Some(t) = track {
        button(text(if t.is_liked { "♥" } else { "♡" }).size(15))
            .padding([6, 8])
            .style(ghost_button_style)
            .on_press(Message::ToggleLike(t.clone()))
            .into()
    } else {
        space().width(0).into()
    };

    let play_glyph = match info.status {
        PlaybackStatus::Playing | PlaybackStatus::Buffering => "⏸",
        PlaybackStatus::Paused | PlaybackStatus::Stopped => "▶",
    };

    let repeat_label = match info.repeat {
        RepeatMode::Off => "Repeat",
        RepeatMode::All => "Repeat All",
        RepeatMode::One => "Repeat 1",
    };

    let max_dur = info.duration_ms.max(1) as f32;
    let cur_pos = (info.position_ms.min(info.duration_ms.max(1))) as f32;

    let center_controls = column![
        row![
            button(text("Shuffle").size(11))
                .padding([5, 10])
                .style(glass_button_style(info.shuffle))
                .on_press(Message::ToggleShuffle),
            button(text("⏮").size(14))
                .padding([6, 10])
                .style(ghost_button_style)
                .on_press(Message::PreviousTrack),
            button(text(play_glyph).size(15))
                .padding([7, 14])
                .style(glass_button_style(true))
                .on_press(Message::TogglePlayPause),
            button(text("⏭").size(14))
                .padding([6, 10])
                .style(ghost_button_style)
                .on_press(Message::NextTrack),
            button(text(repeat_label).size(11))
                .padding([5, 10])
                .style(glass_button_style(info.repeat != RepeatMode::Off))
                .on_press(Message::CycleRepeat),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        row![
            text(format_mm_ss(info.position_ms))
                .size(11)
                .color(TEXT_MUTED),
            slider(0.0..=max_dur, cur_pos, |v| Message::SeekSliderChanged(v as u64))
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
        button(text("Lyrics").size(12))
            .padding([6, 10])
            .style(glass_button_style(false))
            .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics)),
        button(text("Queue").size(12))
            .padding([6, 10])
            .style(glass_button_style(queue_open))
            .on_press(Message::ToggleQueueDrawer),
        button(text(if info.muted { "Muted" } else { "Vol" }).size(11))
            .padding([6, 8])
            .style(glass_button_style(info.muted))
            .on_press(Message::ToggleMute),
        slider(
            0.0..=1.0,
            if info.muted { 0.0 } else { info.volume },
            Message::SetVolume
        )
        .step(0.02_f32)
        .style(seek_slider_style)
        .width(Length::Fixed(86.0)),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    container(
        glass_panel(
            row![
                container(left_track_info).width(Length::FillPortion(1)),
                center_controls,
                container(right_controls)
                    .width(Length::FillPortion(1))
                    .align_x(iced::alignment::Horizontal::Right),
            ]
            .spacing(16)
            .padding([10, 16])
            .align_y(Alignment::Center)
            .width(Length::Fill),
        )
        .width(Length::Fill),
    )
    .padding([10, 14])
    .width(Length::Fill)
    .into()
}
