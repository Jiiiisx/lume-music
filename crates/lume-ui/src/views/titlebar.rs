use iced::widget::{button, container, mouse_area, row, space, text, text_input};
use iced::{Alignment, Background, Color, Element, Length};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{responsive_sidebar_width, Message, SourceFilter};
use crate::widgets::{
    ghost_button_style, glass_button_style, icon_close, icon_queue, icon_search,
    traffic_light_button_style, ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_PRIMARY,
};

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

fn source_chip<'a>(
    label: &'a str,
    target: SourceFilter,
    current: SourceFilter,
    compact: bool,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let active = target == current;
    let col = if active {
        accent.on_primary
    } else {
        TEXT_PRIMARY
    };
    let font = if active { FONT_SEMIBOLD } else { FONT_MEDIUM };
    button(
        text(label)
            .size(11)
            .font(font)
            .color(col)
            .wrapping(text::Wrapping::None),
    )
    .padding(if compact { [4, 8] } else { [5, 12] })
    .style(glass_button_style(active, accent))
    .on_press(Message::SourceFilterChanged(target))
    .into()
}

pub fn view_titlebar<'a>(
    source_status_badge: Option<&'a str>,
    search_query: &'a str,
    source_filter: SourceFilter,
    queue_open: bool,
    window_width: f32,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let compact = window_width < 980.0;
    let sidebar_w = responsive_sidebar_width(window_width);

    let traffic_lights = row![
        traffic_light_btn((255, 95, 87), (255, 123, 115), Message::CloseRequested),
        traffic_light_btn((254, 188, 46), (255, 208, 91), Message::MinimizeWindow),
        traffic_light_btn((40, 200, 64), (74, 222, 94), Message::ToggleMaximizeWindow),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    let mut brand_row = row![text("Lume")
        .size(15)
        .font(FONT_BOLD)
        .color(TEXT_PRIMARY)
        .wrapping(text::Wrapping::None)]
    .spacing(8)
    .align_y(Alignment::Center);

    if let Some(badge) = source_status_badge.filter(|_| !compact) {
        brand_row = brand_row.push(
            container(
                text(badge)
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(accent.light)
                    .wrapping(text::Wrapping::None),
            )
            .padding([3, 7])
            .style(move |_| container::Style {
                background: Some(Background::Color(accent.soft_bg)),
                border: iced::border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: 10.0.into(),
                },
                ..Default::default()
            }),
        );
    }

    let drag_brand = mouse_area(
        container(brand_row)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(Alignment::Center),
    )
    .on_press(Message::DragWindow);

    let left_section = row![traffic_lights, drag_brand]
        .spacing(12)
        .align_y(Alignment::Center)
        .width(Length::Fixed(sidebar_w))
        .height(Length::Fill);

    let mut search_row = row![
        icon_search(14.0, ICON_MUTED),
        text_input("Search songs, artists, playlists...", search_query)
            .on_input(Message::SearchInputChanged)
            .on_submit(Message::SearchSubmitted)
            .size(12)
            .style(move |_, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: iced::border::Border::default(),
                icon: TEXT_PRIMARY,
                placeholder: TEXT_MUTED,
                value: TEXT_PRIMARY,
                selection: Color {
                    a: 0.30,
                    ..accent.light
                },
            })
            .padding([5, 4])
            .width(Length::Fill),
    ]
    .spacing(6)
    .padding([1, 10])
    .align_y(Alignment::Center);

    if !search_query.is_empty() {
        search_row = search_row.push(
            button(icon_close(12.0, ICON_MUTED))
                .padding([3, 4])
                .style(ghost_button_style)
                .on_press(Message::SearchInputChanged(String::new())),
        );
    }

    let search_width = (window_width * 0.28).clamp(180.0, 360.0);
    let center_search = container(search_row)
        .width(Length::Fixed(search_width))
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
            border: iced::border::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
                width: 1.0,
                radius: 16.0.into(),
            },
            ..Default::default()
        });

    let source_chips = row![
        source_chip("All", SourceFilter::All, source_filter, compact, accent),
        source_chip("Spotify", SourceFilter::Spotify, source_filter, compact, accent),
        source_chip(
            "YouTube",
            SourceFilter::YouTubeMusic,
            source_filter,
            compact,
            accent,
        ),
        source_chip("Local", SourceFilter::Local, source_filter, compact, accent),
    ]
    .spacing(if compact { 4 } else { 5 })
    .align_y(Alignment::Center);

    let queue_content: Element<'a, Message> = if compact {
        icon_queue(
            14.0,
            if queue_open {
                accent.on_primary
            } else {
                ICON_LIGHT
            },
        )
    } else {
        row![
            icon_queue(
                14.0,
                if queue_open {
                    accent.on_primary
                } else {
                    ICON_LIGHT
                }
            ),
            text("Queue")
                .size(11)
                .font(if queue_open { FONT_SEMIBOLD } else { FONT_MEDIUM })
                .color(if queue_open {
                    accent.on_primary
                } else {
                    TEXT_PRIMARY
                })
                .wrapping(text::Wrapping::None),
        ]
        .spacing(6)
        .align_y(Alignment::Center)
        .into()
    };

    let queue_btn = button(queue_content)
        .padding(if compact { [5, 9] } else { [5, 12] })
        .style(glass_button_style(queue_open, accent))
        .on_press(Message::ToggleQueueDrawer);

    let right_section = row![
        mouse_area(space().width(Length::Fill).height(Length::Fixed(34.0)))
            .on_press(Message::DragWindow),
        source_chips,
        queue_btn,
    ]
    .spacing(if compact { 6 } else { 10 })
    .align_y(Alignment::Center)
    .width(Length::Fill);

    container(
        row![left_section, center_search, right_section]
            .spacing(if compact { 10 } else { 14 })
            .padding([6, 16])
            .width(Length::Fill)
            .height(Length::Fixed(46.0))
            .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .into()
}
