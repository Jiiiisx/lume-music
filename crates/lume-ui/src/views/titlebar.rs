use iced::widget::{button, container, mouse_area, row, space, text};
use iced::{Alignment, Background, Color, Element, Length};

use crate::views::Message;
use crate::widgets::{close_button_style, ghost_button_style, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY};

pub fn view_titlebar<'a>(
    left_label: &'a str,
    maximized: bool,
    source_status_badge: Option<&'a str>,
) -> Element<'a, Message> {
    let drag_region = mouse_area(
        row![
            text(left_label).size(13).color(TEXT_SECONDARY),
            space().width(10),
            if let Some(badge) = source_status_badge {
                Element::from(
                    container(text(badge).size(11).color(TEXT_MUTED))
                        .padding([2, 8])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
                            border: iced::border::rounded(10.0),
                            ..Default::default()
                        }),
                )
            } else {
                Element::from(space().width(0))
            },
            space().width(Length::Fill),
        ]
        .align_y(Alignment::Center)
        .padding([8, 14]),
    )
    .on_press(Message::DragWindow);

    let max_glyph = if maximized { "❐" } else { "□" };

    let window_controls = row![
        button(text("─").size(12).color(TEXT_PRIMARY))
            .padding([6, 12])
            .style(ghost_button_style)
            .on_press(Message::MinimizeWindow),
        button(text(max_glyph).size(12).color(TEXT_PRIMARY))
            .padding([6, 12])
            .style(ghost_button_style)
            .on_press(Message::ToggleMaximizeWindow),
        button(text("×").size(14).color(TEXT_PRIMARY))
            .padding([5, 13])
            .style(close_button_style)
            .on_press(Message::CloseRequested),
    ]
    .spacing(2)
    .align_y(Alignment::Center);

    container(
        row![drag_region, window_controls]
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(38.0))
    .into()
}
