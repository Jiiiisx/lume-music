use iced::border;
use iced::widget::{button, container, image, scrollable, slider, text, text_input, Container};
use iced::{Background, Color, ContentFit, Element, Length, Shadow, Vector};

pub const GLASS_PANEL_BG: Color = Color {
    r: 20.0 / 255.0,
    g: 20.0 / 255.0,
    b: 26.0 / 255.0,
    a: 0.45,
};

pub const GLASS_ELEVATED_BG: Color = Color {
    r: 28.0 / 255.0,
    g: 29.0 / 255.0,
    b: 38.0 / 255.0,
    a: 0.72,
};

pub const GLASS_BORDER: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.12,
};

pub const GLASS_HIGHLIGHT: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.14,
};

pub const TEXT_PRIMARY: Color = Color {
    r: 247.0 / 255.0,
    g: 248.0 / 255.0,
    b: 252.0 / 255.0,
    a: 0.96,
};

pub const TEXT_SECONDARY: Color = Color {
    r: 220.0 / 255.0,
    g: 224.0 / 255.0,
    b: 235.0 / 255.0,
    a: 0.72,
};

pub const TEXT_MUTED: Color = Color {
    r: 195.0 / 255.0,
    g: 200.0 / 255.0,
    b: 214.0 / 255.0,
    a: 0.46,
};

pub fn glass_panel_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(Background::Color(GLASS_PANEL_BG)),
        border: border::Border {
            color: GLASS_BORDER,
            width: 1.0,
            radius: 14.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 18.0,
        },
        snap: false,
    }
}

pub fn glass_elevated_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(Background::Color(GLASS_ELEVATED_BG)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
            width: 1.0,
            radius: 14.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.45),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 28.0,
        },
        snap: false,
    }
}

pub fn glass_panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    container(content).style(glass_panel_style)
}

pub fn ghost_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.18))),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: TEXT_PRIMARY,
        border: border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 8.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn close_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgba8(220, 56, 68, 0.85))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba8(190, 42, 54, 0.95))),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: TEXT_PRIMARY,
        border: border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 6.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn glass_button_style(active: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let base_alpha = if active { 0.22 } else { 0.08 };
        let alpha = match status {
            button::Status::Hovered => base_alpha + 0.08,
            button::Status::Pressed => base_alpha + 0.14,
            button::Status::Disabled => 0.03,
            button::Status::Active => base_alpha,
        };
        let border_color = if active {
            Color::from_rgba(1.0, 1.0, 1.0, 0.28)
        } else {
            GLASS_BORDER
        };
        button::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, alpha))),
            text_color: if matches!(status, button::Status::Disabled) {
                TEXT_MUTED
            } else {
                TEXT_PRIMARY
            },
            border: border::Border {
                color: border_color,
                width: 1.0,
                radius: 10.0.into(),
            },
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

pub fn primary_pill_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg_color = match status {
        button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 1.0),
        button::Status::Pressed => Color::from_rgba(0.88, 0.88, 0.92, 1.0),
        _ => Color::from_rgba(0.95, 0.95, 0.98, 0.94),
    };
    button::Style {
        background: Some(Background::Color(bg_color)),
        text_color: Color::from_rgb8(18, 18, 24),
        border: border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 18.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn glass_input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let border_alpha = match status {
        text_input::Status::Focused { .. } => 0.36,
        text_input::Status::Hovered => 0.22,
        _ => 0.12,
    };
    text_input::Style {
        background: Background::Color(Color::from_rgba8(14, 14, 20, 0.55)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, border_alpha),
            width: 1.0,
            radius: 10.0.into(),
        },
        icon: TEXT_SECONDARY,
        placeholder: TEXT_MUTED,
        value: TEXT_PRIMARY,
        selection: Color::from_rgba(1.0, 1.0, 1.0, 0.25),
    }
}

pub fn thin_scrollable_style(_theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
    let scroller_alpha = match status {
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => 0.35,
        _ => 0.16,
    };
    let rail = scrollable::Rail {
        background: None,
        border: border::Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, scroller_alpha)),
            border: border::rounded(4.0),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(GLASS_ELEVATED_BG),
            border: border::color(GLASS_BORDER).width(1.0).rounded(8.0),
            shadow: Shadow::default(),
            icon: TEXT_PRIMARY,
        },
    }
}

pub fn seek_slider_style(_theme: &iced::Theme, status: slider::Status) -> slider::Style {
    let handle_radius = match status {
        slider::Status::Hovered | slider::Status::Dragged => 6.0,
        slider::Status::Active => 0.0,
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.92)),
                Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22)),
            ),
            width: 4.0,
            border: border::rounded(2.0),
        },
        handle: slider::Handle {
            shape: slider::HandleShape::Circle {
                radius: handle_radius,
            },
            background: Background::Color(Color::WHITE),
            border_width: 0.0,
            border_color: Color::TRANSPARENT,
        },
    }
}

pub fn cover_art_widget<'a, Message: 'a>(
    handle: Option<&image::Handle>,
    size: f32,
    radius: f32,
) -> Element<'a, Message> {
    let inner: Element<'a, Message> = if let Some(h) = handle {
        image(h.clone())
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .content_fit(ContentFit::Cover)
            .into()
    } else {
        container(text("♫").size((size * 0.34).max(14.0)).color(TEXT_MUTED))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size))
            .style(move |_| container::Style {
                background: Some(Background::Color(Color::from_rgba8(24, 24, 34, 0.72))),
                border: border::Border {
                    color: GLASS_BORDER,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            })
            .into()
    };

    container(inner)
        .width(Length::Fixed(size))
        .height(Length::Fixed(size))
        .style(move |_| container::Style {
            border: border::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.10),
                width: 1.0,
                radius: radius.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.48),
                offset: Vector::new(0.0, 12.0),
                blur_radius: 28.0,
            },
            ..Default::default()
        })
        .into()
}
