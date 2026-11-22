use iced::widget::{button, container, mouse_area, row, space, text};
use iced::{Alignment, Background, Color, Element, Length};

use crate::views::Message;
use crate::widgets::{
    close_button_style, ghost_button_style, icon_close, icon_maximize, icon_minimize, ICON_LIGHT,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn view_titlebar<'a>(
    left_label: &'a str,
    maximized: bool,
    source_status_badge: Option<&'a str>,
) -> Element<'a, Message> {
    let accent_dot = container(space().width(8).height(8)).style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgb8(245, 166, 118))),
        border: iced::border::rounded(4.0),
        ..Default::default()
    });

    let drag_region = mouse_area(
        row![
            accent_dot,
            text(left_label).size(13).color(TEXT_PRIMARY),
            if let Some(badge) = source_status_badge {
                Element::from(
                    container(text(badge).size(11).color(TEXT_SECONDARY))
                        .padding([3, 10])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
                            border: iced::border::Border {
                                color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
                                width: 1.0,
                                radius: 12.0.into(),
                            },
                            ..Default::default()
                        }),
                )
            } else {
                Element::from(space().width(0))
            },
            space().width(Length::Fill),
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .padding([8, 18]),
    )
    .on_press(Message::DragWindow);

    let window_controls = container(
        row![
            button(icon_minimize(13.0, ICON_LIGHT))
                .padding([7, 11])
                .style(ghost_button_style)
                .on_press(Message::MinimizeWindow),
            button(icon_maximize(maximized, 13.0, ICON_LIGHT))
                .padding([7, 11])
                .style(ghost_button_style)
                .on_press(Message::ToggleMaximizeWindow),
            button(icon_close(13.0, ICON_LIGHT))
                .padding([7, 12])
                .style(close_button_style)
                .on_press(Message::CloseRequested),
        ]
        .spacing(2)
        .align_y(Alignment::Center),
    )
    .padding(2)
    .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
        border: iced::border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
            width: 1.0,
            radius: 14.0.into(),
        },
        ..Default::default()
    });

    container(
        row![drag_region, window_controls]
            .padding(iced::Padding {
                top: 0.0,
                right: 14.0,
                bottom: 0.0,
                left: 0.0,
            })
            .width(Length::Fill)
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .height(Length::Fixed(46.0))
    .into()
}
