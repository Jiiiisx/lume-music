use std::collections::HashMap;

use iced::widget::{button, column, container, image, mouse_area, row, scrollable, slider, space, text};
use iced::{Alignment, Background, Element, Length};
use lume_core::{PlaybackStatus, Playlist, RepeatMode, Track, TrackSource};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{Message, NowPlayingInfo, ViewMode};
use crate::widgets::{
    cover_art_widget, floating_dock_style, ghost_button_style, glass_button_style,
    glass_elevated_style, hidden_scrollbar, icon_heart, icon_lyrics, icon_more, icon_next,
    icon_pause, icon_play, icon_playlist, icon_plus, icon_prev, icon_queue, icon_refresh,
    icon_repeat, icon_shuffle, icon_volume, play_circle_button_style, seek_slider_style,
    thin_scrollable_style, GLASS_SUBTLE_BORDER, ICON_HEART_ACTIVE, ICON_LIGHT, ICON_MUTED,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

fn format_mm_ss(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

fn truncate_single_line(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let head: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{}…", head.trim_end())
}

pub fn view_player_bar<'a>(
    info: NowPlayingInfo<'a>,
    active_cover: Option<&'a image::Handle>,
    art_handles: &'a HashMap<String, image::Handle>,
    queue_open: bool,
    player_menu_open: bool,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let track = info.track;
    let art = active_cover.or_else(|| {
        track
            .and_then(|t| t.artwork_ref.as_deref())
            .and_then(|k| art_handles.get(k))
    });

    let title = track.map(|t| t.title.as_str()).unwrap_or("Nothing Playing");
    let artist = track
        .map(|t| t.artist_display())
        .unwrap_or_else(|| "Select a track to play".to_string());
    let badge = if info.status == PlaybackStatus::Buffering {
        "Buffering..."
    } else {
        info.source_used.map(|s| s.badge_label()).unwrap_or("Idle")
    };

    let title_display = truncate_single_line(title, 26);
    let artist_display = truncate_single_line(&artist, 28);

    let track_click_area = container(
        mouse_area(
            row![
                cover_art_widget(art, 48.0, 10.0),
                container(
                    column![
                        text(title_display)
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .color(TEXT_PRIMARY)
                            .wrapping(text::Wrapping::None),
                        text(artist_display)
                            .size(11)
                            .color(TEXT_SECONDARY)
                            .wrapping(text::Wrapping::None),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                )
                .width(Length::Fill)
                .clip(true),
            ]
            .spacing(12)
            .width(Length::Fill)
            .align_y(Alignment::Center),
        )
        .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics)),
    )
    .width(Length::Fill)
    .clip(true);

    let like_btn: Element<'a, Message> = if let Some(t) = track {
        button(
            container(if t.is_liked {
                icon_heart(true, 15.0, ICON_HEART_ACTIVE)
            } else {
                icon_heart(false, 15.0, ICON_MUTED)
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
        )
        .padding(0)
        .width(Length::Fixed(30.0))
        .height(Length::Fixed(30.0))
        .style(ghost_button_style)
        .on_press(Message::ToggleLike(t.clone()))
        .into()
    } else {
        space().width(0).into()
    };

    let more_btn: Element<'a, Message> = if track.is_some() {
        button(
            container(icon_more(
                15.0,
                if player_menu_open {
                    accent.on_primary
                } else {
                    ICON_MUTED
                },
            ))
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
        )
        .padding(0)
        .width(Length::Fixed(30.0))
        .height(Length::Fixed(30.0))
        .style(if player_menu_open {
            glass_button_style(true, accent)
        } else {
            glass_button_style(false, accent)
        })
        .on_press(Message::TogglePlayerMenu)
        .into()
    } else {
        space().width(0).into()
    };

    let left_track_info = row![track_click_area, like_btn, more_btn]
        .spacing(6)
        .width(Length::Fill)
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

/// Floating smoked-glass action menu for the currently playing track (opened via `•••` on the
/// player bar or Now Playing lyrics view).
pub fn view_player_menu_popover<'a>(
    track: &'a Track,
    playlists: &'a [Playlist],
    accent: AccentTheme,
) -> Element<'a, Message> {
    let mut items = column![
        column![
            text(track.title.as_str())
                .size(13)
                .font(FONT_BOLD)
                .color(TEXT_PRIMARY),
            text(track.artist_display())
                .size(11)
                .color(TEXT_SECONDARY),
        ]
        .spacing(2)
        .padding([2, 6]),
        container(space().height(1))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                ..Default::default()
            }),
        button(
            row![
                icon_play(12.0, ICON_LIGHT),
                text("Play Next").size(12).font(FONT_MEDIUM).color(TEXT_PRIMARY),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(ghost_button_style)
        .on_press(Message::PlayNext(track.clone())),
        button(
            row![
                icon_queue(13.0, ICON_LIGHT),
                text("Add to Queue")
                    .size(12)
                    .font(FONT_MEDIUM)
                    .color(TEXT_PRIMARY),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(ghost_button_style)
        .on_press(Message::AddToQueue(track.clone())),
    ]
    .spacing(4)
    .width(Length::Fixed(248.0));

    if matches!(track.source, TrackSource::Spotify { .. }) {
        items = items.push(
            button(
                row![
                    icon_refresh(12.0, accent.light),
                    text("Wrong version? Re-match")
                        .size(12)
                        .font(FONT_MEDIUM)
                        .color(accent.light),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([7, 10])
            .style(ghost_button_style)
            .on_press(Message::RematchYoutubeFallback(track.clone())),
        );
    }

    items = items.push(
        container(space().height(1))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                ..Default::default()
            }),
    );

    items = items.push(
        container(
            text("ADD TO PLAYLIST")
                .size(10)
                .font(FONT_SEMIBOLD)
                .color(TEXT_MUTED),
        )
        .padding([4, 8]),
    );

    let editable_playlists: Vec<&Playlist> = playlists
        .iter()
        .filter(|pl| pl.origin.is_editable())
        .collect();

    if editable_playlists.is_empty() {
        items = items.push(
            container(
                text("No custom playlists yet")
                    .size(11)
                    .color(TEXT_MUTED),
            )
            .padding([4, 8]),
        );
    } else {
        let mut pl_col = column![].spacing(2);
        for pl in editable_playlists {
            pl_col = pl_col.push(
                button(
                    row![
                        icon_playlist(13.0, accent.light),
                        text(pl.name.as_str())
                            .size(12)
                            .font(FONT_MEDIUM)
                            .color(TEXT_PRIMARY)
                            .width(Length::Fill),
                        text(format!("{}", pl.track_count))
                            .size(10)
                            .color(TEXT_MUTED),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([7, 10])
                .style(ghost_button_style)
                .on_press(Message::AddTrackToPlaylist {
                    playlist_id: pl.id.clone(),
                    track: track.clone(),
                }),
            );
        }
        items = items.push(
            scrollable(pl_col)
                .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
                .height(Length::Shrink)
                .style(thin_scrollable_style),
        );
    }

    items = items.push(
        button(
            row![
                icon_plus(12.0, accent.light),
                text("New Playlist")
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .color(accent.light),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(ghost_button_style)
        .on_press(Message::OpenNewPlaylistModal),
    );

    container(items)
        .padding(10)
        .max_height(380.0)
        .style(glass_elevated_style)
        .into()
}
