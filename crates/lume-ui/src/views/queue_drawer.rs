use std::collections::HashMap;

use iced::widget::{button, column, container, image, row, scrollable, space, text};
use iced::{Alignment, Background, Color, Element, Length, Padding};
use lume_core::Track;

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::Message;
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_elevated_style,
    hidden_scrollbar, icon_chevron_down, icon_chevron_up, icon_close, icon_lyrics,
    thin_scrollable_style, track_row_button_style, GLASS_SUBTLE_BORDER, ICON_LIGHT, ICON_MUTED,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

fn format_mm_ss(ms: u64) -> String {
    let total_secs = ms / 1000;
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

fn truncate_text(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", head.trim_end())
    } else {
        head
    }
}

#[allow(clippy::too_many_arguments)]
fn queue_track_row<'a>(
    track: &'a Track,
    idx: usize,
    min_up_idx: usize,
    total_len: usize,
    is_current: bool,
    dimmed: bool,
    art_handles: &'a HashMap<String, image::Handle>,
    accent: AccentTheme,
    art_size: f32,
    max_title_chars: usize,
    max_artist_chars: usize,
    show_reorder: bool,
) -> Element<'a, Message> {
    let art = track
        .artwork_ref
        .as_deref()
        .and_then(|k| art_handles.get(k));

    let title_color = if is_current {
        accent.light
    } else if dimmed {
        Color::from_rgba(1.0, 1.0, 1.0, 0.56)
    } else {
        TEXT_PRIMARY
    };
    let artist_color = if dimmed {
        Color::from_rgba(1.0, 1.0, 1.0, 0.36)
    } else {
        TEXT_SECONDARY
    };
    let dur_color = if dimmed {
        Color::from_rgba(1.0, 1.0, 1.0, 0.34)
    } else {
        TEXT_MUTED
    };

    let main_click = button(
        row![
            cover_art_widget(art, art_size, 8.0),
            column![
                text(truncate_text(&track.title, max_title_chars))
                    .size(13)
                    .font(if is_current { FONT_BOLD } else { FONT_SEMIBOLD })
                    .color(title_color),
                text(truncate_text(&track.artist_display(), max_artist_chars))
                    .size(11)
                    .color(artist_color),
            ]
            .spacing(2)
            .width(Length::Fill),
            text(format_mm_ss(track.duration_ms))
                .size(12)
                .font(FONT_MEDIUM)
                .color(dur_color),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .padding([7, 10])
    .style(track_row_button_style(is_current, accent))
    .on_press(Message::QueueJumpTo(idx));

    let mut action_row = row![main_click].spacing(2).align_y(Alignment::Center);

    if show_reorder && !is_current {
        if idx > min_up_idx {
            action_row = action_row.push(
                button(icon_chevron_up(12.0, ICON_MUTED))
                    .padding([6, 6])
                    .style(ghost_button_style)
                    .on_press(Message::QueueMove {
                        from: idx,
                        to: idx - 1,
                    }),
            );
        }
        if idx + 1 < total_len {
            action_row = action_row.push(
                button(icon_chevron_down(12.0, ICON_MUTED))
                    .padding([6, 6])
                    .style(ghost_button_style)
                    .on_press(Message::QueueMove {
                        from: idx,
                        to: idx + 1,
                    }),
            );
        }
        action_row = action_row.push(
            button(icon_close(11.0, ICON_MUTED))
                .padding([6, 6])
                .style(ghost_button_style)
                .on_press(Message::QueueRemoveAt(idx)),
        );
    }

    action_row.width(Length::Fill).into()
}

/// Frameless cinema "Up Next" queue panel designed to replace the right-hand lyrics column
/// in `ViewMode::NowPlayingLyrics` (matching Spotify Spicy/Beautiful Lyrics extension style).
pub fn view_inline_queue<'a>(
    queue_tracks: &[&'a Track],
    current_pos: Option<usize>,
    art_handles: &'a HashMap<String, image::Handle>,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let total_len = queue_tracks.len();
    let upcoming: Vec<(usize, &'a Track)> = match current_pos {
        Some(cur) if cur + 1 < total_len => queue_tracks
            .iter()
            .enumerate()
            .skip(cur + 1)
            .map(|(i, &t)| (i, t))
            .collect(),
        None => queue_tracks
            .iter()
            .enumerate()
            .map(|(i, &t)| (i, t))
            .collect(),
        _ => Vec::new(),
    };
    let earlier: Vec<(usize, &'a Track)> = match current_pos {
        Some(cur) if cur > 0 => queue_tracks
            .iter()
            .enumerate()
            .take(cur)
            .map(|(i, &t)| (i, t))
            .collect(),
        _ => Vec::new(),
    };

    let subtitle = if upcoming.is_empty() {
        "End of queue".to_string()
    } else if upcoming.len() == 1 {
        "1 song up next".to_string()
    } else {
        format!("{} songs up next", upcoming.len())
    };

    let mut header_actions = row![].spacing(8).align_y(Alignment::Center);
    if !queue_tracks.is_empty() {
        header_actions = header_actions.push(
            button(text("Clear").size(11).font(FONT_MEDIUM).color(TEXT_SECONDARY))
                .padding([6, 12])
                .style(glass_button_style(false, accent))
                .on_press(Message::ClearQueue),
        );
    }
    header_actions = header_actions.push(
        button(
            row![
                icon_lyrics(13.0, ICON_LIGHT),
                text("Lyrics").size(12).font(FONT_MEDIUM).color(TEXT_PRIMARY),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([6, 12])
        .style(glass_button_style(false, accent))
        .on_press(Message::ToggleQueueDrawer),
    );

    let header = row![
        column![
            text("Up Next")
                .size(26)
                .font(FONT_BOLD)
                .color(TEXT_PRIMARY),
            text(subtitle)
                .size(12)
                .font(FONT_MEDIUM)
                .color(Color::from_rgba(1.0, 1.0, 1.0, 0.56)),
        ]
        .spacing(4)
        .width(Length::Fill),
        header_actions,
    ]
    .align_y(Alignment::Center)
    .padding(Padding {
        top: 0.0,
        right: 6.0,
        bottom: 6.0,
        left: 10.0,
    });

    let mut list_col = column![].spacing(3).width(Length::Fill);

    let min_up_idx = current_pos.map(|c| c + 1).unwrap_or(0);

    if upcoming.is_empty() {
        list_col = list_col.push(
            container(
                column![
                    text("Nothing queued up next")
                        .size(15)
                        .font(FONT_SEMIBOLD)
                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.72)),
                    text("Use ••• on any track to choose Play Next or Add to Queue.")
                        .size(12)
                        .color(Color::from_rgba(1.0, 1.0, 1.0, 0.45)),
                ]
                .spacing(6),
            )
            .padding([24, 10]),
        );
    } else {
        for (idx, track) in upcoming {
            list_col = list_col.push(queue_track_row(
                track,
                idx,
                min_up_idx,
                total_len,
                false,
                false,
                art_handles,
                accent,
                44.0,
                44,
                46,
                true,
            ));
        }
    }

    if !earlier.is_empty() {
        list_col = list_col.push(space().height(16.0));
        list_col = list_col.push(
            container(
                text(format!("PLAYED EARLIER • {}", earlier.len()))
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(Color::from_rgba(1.0, 1.0, 1.0, 0.42)),
            )
            .padding([4, 10]),
        );
        for (idx, track) in earlier.into_iter().rev() {
            list_col = list_col.push(queue_track_row(
                track,
                idx,
                0,
                total_len,
                false,
                true,
                art_handles,
                accent,
                40.0,
                44,
                46,
                false,
            ));
        }
    }

    container(
        column![
            header,
            scrollable(list_col)
                .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
                .height(Length::Fill)
                .width(Length::Fill)
                .style(thin_scrollable_style),
        ]
        .spacing(12)
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
        top: 22.0,
        right: 36.0,
        bottom: 18.0,
        left: 24.0,
    })
    .into()
}

/// Sleek smoked-glass Queue Drawer overlay for `ViewMode::Browse` (Home, Library, Playlists, Explore).
pub fn view_queue_drawer<'a>(
    queue_tracks: &[&'a Track],
    current_pos: Option<usize>,
    art_handles: &'a HashMap<String, image::Handle>,
    window_width: f32,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let drawer_width = (window_width * 0.34).clamp(296.0, 380.0);
    let total_len = queue_tracks.len();
    let current_item = current_pos.and_then(|pos| queue_tracks.get(pos).map(|&t| (pos, t)));
    let upcoming: Vec<(usize, &'a Track)> = match current_pos {
        Some(cur) if cur + 1 < total_len => queue_tracks
            .iter()
            .enumerate()
            .skip(cur + 1)
            .map(|(i, &t)| (i, t))
            .collect(),
        None => queue_tracks
            .iter()
            .enumerate()
            .map(|(i, &t)| (i, t))
            .collect(),
        _ => Vec::new(),
    };
    let earlier: Vec<(usize, &'a Track)> = match current_pos {
        Some(cur) if cur > 0 => queue_tracks
            .iter()
            .enumerate()
            .take(cur)
            .map(|(i, &t)| (i, t))
            .collect(),
        _ => Vec::new(),
    };

    let header = row![
        column![
            text("Up Next").size(18).font(FONT_BOLD).color(TEXT_PRIMARY),
            text(if total_len == 0 {
                "Queue is empty".to_string()
            } else {
                format!("{} upcoming • {} total", upcoming.len(), total_len)
            })
            .size(11)
            .color(TEXT_MUTED),
        ]
        .spacing(2)
        .width(Length::Fill),
        if !queue_tracks.is_empty() {
            Element::from(
                button(text("Clear").size(11).font(FONT_MEDIUM).color(TEXT_PRIMARY))
                    .padding([5, 11])
                    .style(glass_button_style(false, accent))
                    .on_press(Message::ClearQueue),
            )
        } else {
            Element::from(space().width(0))
        },
        button(icon_close(14.0, ICON_LIGHT))
            .padding([6, 8])
            .style(ghost_button_style)
            .on_press(Message::ToggleQueueDrawer),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut list_col = column![].spacing(4).width(Length::Fill);

    if queue_tracks.is_empty() {
        list_col = list_col.push(
            container(
                column![
                    text("Your queue is empty")
                        .size(14)
                        .font(FONT_SEMIBOLD)
                        .color(TEXT_SECONDARY),
                    text("Play a song or use ••• → Play Next / Add to Queue.")
                        .size(11)
                        .color(TEXT_MUTED),
                ]
                .spacing(4),
            )
            .padding([24, 10]),
        );
    } else {
        let min_up_idx = current_pos.map(|c| c + 1).unwrap_or(0);
        if let Some((cur_idx, cur_track)) = current_item {
            list_col = list_col.push(
                container(
                    text("NOW PLAYING")
                        .size(10)
                        .font(FONT_SEMIBOLD)
                        .color(accent.light),
                )
                .padding([2, 8]),
            );
            list_col = list_col.push(queue_track_row(
                cur_track,
                cur_idx,
                0,
                total_len,
                true,
                false,
                art_handles,
                accent,
                42.0,
                22,
                24,
                false,
            ));
            list_col = list_col.push(
                container(
                    container(space().height(1))
                        .width(Length::Fill)
                        .style(|_| container::Style {
                            background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                            ..Default::default()
                        }),
                )
                .width(Length::Fill)
                .padding([4, 4]),
            );
        }

        list_col = list_col.push(
            container(
                text(format!("UP NEXT • {}", upcoming.len()))
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(TEXT_MUTED),
            )
            .padding([4, 8]),
        );

        if upcoming.is_empty() {
            list_col = list_col.push(
                container(
                    text("No more tracks queued after this song.")
                        .size(12)
                        .color(TEXT_MUTED),
                )
                .padding([8, 8]),
            );
        } else {
            for (idx, track) in upcoming {
                list_col = list_col.push(queue_track_row(
                    track,
                    idx,
                    min_up_idx,
                    total_len,
                    false,
                    false,
                    art_handles,
                    accent,
                    38.0,
                    20,
                    22,
                    true,
                ));
            }
        }

        if !earlier.is_empty() {
            list_col = list_col.push(space().height(8.0));
            list_col = list_col.push(
                container(
                    text(format!("PLAYED EARLIER • {}", earlier.len()))
                        .size(10)
                        .font(FONT_SEMIBOLD)
                        .color(TEXT_MUTED),
                )
                .padding([4, 8]),
            );
            for (idx, track) in earlier.into_iter().rev() {
                list_col = list_col.push(queue_track_row(
                    track,
                    idx,
                    0,
                    total_len,
                    false,
                    true,
                    art_handles,
                    accent,
                    36.0,
                    22,
                    24,
                    false,
                ));
            }
        }
    }

    container(
        column![
            header,
            scrollable(list_col)
                .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
                .height(Length::Fill)
                .style(thin_scrollable_style),
        ]
        .spacing(12)
        .padding(16)
        .width(Length::Fixed(drawer_width))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .style(glass_elevated_style)
    .into()
}
