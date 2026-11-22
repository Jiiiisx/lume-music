use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::Track;

use crate::views::Message;
use crate::widgets::{
    ghost_button_style, glass_button_style, glass_elevated_style, icon_chevron_down,
    icon_chevron_up, icon_close, thin_scrollable_style, ICON_LIGHT, ICON_MUTED, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn view_queue_drawer<'a>(
    queue_tracks: &[&'a Track],
    current_pos: Option<usize>,
) -> Element<'a, Message> {
    let header = row![
        text("Play Queue").size(17).color(TEXT_PRIMARY),
        space().width(Length::Fill),
        button(text("Clear").size(11))
            .padding([5, 12])
            .style(glass_button_style(false))
            .on_press(Message::ClearQueue),
        button(icon_close(14.0, ICON_LIGHT))
            .padding([6, 8])
            .style(ghost_button_style)
            .on_press(Message::ToggleQueueDrawer),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut list_col = column![].spacing(6);
    if queue_tracks.is_empty() {
        list_col = list_col.push(
            container(text("Queue is empty.").size(13).color(TEXT_MUTED)).padding(16),
        );
    } else {
        let len = queue_tracks.len();
        for (idx, track) in queue_tracks.iter().enumerate() {
            let is_current = current_pos == Some(idx);
            let mut row_controls = row![
                button(
                    column![
                        text(track.title.as_str()).size(13).color(if is_current {
                            Color::from_rgb8(255, 224, 184)
                        } else {
                            TEXT_PRIMARY
                        }),
                        text(track.artist_display()).size(11).color(TEXT_SECONDARY),
                    ]
                    .spacing(2),
                )
                .width(Length::Fill)
                .padding([6, 8])
                .style(ghost_button_style)
                .on_press(Message::QueueJumpTo(idx)),
            ]
            .spacing(4)
            .align_y(Alignment::Center);

            if idx > 0 {
                row_controls = row_controls.push(
                    button(icon_chevron_up(13.0, ICON_MUTED))
                        .padding([5, 6])
                        .style(ghost_button_style)
                        .on_press(Message::QueueMove {
                            from: idx,
                            to: idx - 1,
                        }),
                );
            }
            if idx + 1 < len {
                row_controls = row_controls.push(
                    button(icon_chevron_down(13.0, ICON_MUTED))
                        .padding([5, 6])
                        .style(ghost_button_style)
                        .on_press(Message::QueueMove {
                            from: idx,
                            to: idx + 1,
                        }),
                );
            }
            row_controls = row_controls.push(
                button(icon_close(12.0, ICON_MUTED))
                    .padding([5, 7])
                    .style(ghost_button_style)
                    .on_press(Message::QueueRemoveAt(idx)),
            );

            list_col = list_col.push(
                container(row_controls)
                    .padding([6, 8])
                    .style(move |_| container::Style {
                        background: Some(Background::Color(if is_current {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.16)
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                        })),
                        border: iced::border::Border {
                            color: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                            width: 1.0,
                            radius: 14.0.into(),
                        },
                        ..Default::default()
                    }),
            );
        }
    }

    container(
        column![
            header,
            scrollable(list_col)
                .height(Length::Fill)
                .style(thin_scrollable_style),
        ]
        .spacing(14)
        .padding(18)
        .width(Length::Fixed(340.0))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .style(glass_elevated_style)
    .into()
}
