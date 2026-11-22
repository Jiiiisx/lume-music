use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::Track;

use crate::views::Message;
use crate::widgets::{
    ghost_button_style, glass_button_style, glass_elevated_style, thin_scrollable_style,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn view_queue_drawer<'a>(
    queue_tracks: &[&'a Track],
    current_pos: Option<usize>,
) -> Element<'a, Message> {
    let header = row![
        text("Play Queue").size(16).color(TEXT_PRIMARY),
        space().width(Length::Fill),
        button(text("Clear").size(11))
            .padding([4, 10])
            .style(glass_button_style(false))
            .on_press(Message::ClearQueue),
        button(text("×").size(14))
            .padding([3, 9])
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
                            Color::from_rgb8(255, 224, 178)
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
                    button(text("↑").size(11))
                        .padding([4, 6])
                        .style(ghost_button_style)
                        .on_press(Message::QueueMove {
                            from: idx,
                            to: idx - 1,
                        }),
                );
            }
            if idx + 1 < len {
                row_controls = row_controls.push(
                    button(text("↓").size(11))
                        .padding([4, 6])
                        .style(ghost_button_style)
                        .on_press(Message::QueueMove {
                            from: idx,
                            to: idx + 1,
                        }),
                );
            }
            row_controls = row_controls.push(
                button(text("×").size(12))
                    .padding([4, 7])
                    .style(ghost_button_style)
                    .on_press(Message::QueueRemoveAt(idx)),
            );

            list_col = list_col.push(
                container(row_controls)
                    .padding([4, 6])
                    .style(move |_| container::Style {
                        background: Some(Background::Color(if is_current {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.14)
                        } else {
                            Color::from_rgba(1.0, 1.0, 1.0, 0.04)
                        })),
                        border: iced::border::rounded(8.0),
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
        .spacing(12)
        .padding(16)
        .width(Length::Fixed(330.0))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .style(glass_elevated_style)
    .into()
}
