use iced::border;
use iced::gradient;
use iced::widget::{button, container, image, scrollable, slider, text_input, Container};
use iced::{Background, Color, ContentFit, Element, Length, Radians, Shadow, Vector};

use crate::widgets::icons::icon_music;

pub const GLASS_BORDER: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.20,
};

pub const GLASS_SUBTLE_BORDER: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.13,
};

pub const TEXT_PRIMARY: Color = Color {
    r: 252.0 / 255.0,
    g: 252.0 / 255.0,
    b: 255.0 / 255.0,
    a: 0.96,
};

pub const TEXT_SECONDARY: Color = Color {
    r: 235.0 / 255.0,
    g: 236.0 / 255.0,
    b: 244.0 / 255.0,
    a: 0.76,
};

pub const TEXT_MUTED: Color = Color {
    r: 222.0 / 255.0,
    g: 224.0 / 255.0,
    b: 234.0 / 255.0,
    a: 0.54,
};

pub const TEXT_ON_PILL: Color = Color {
    r: 24.0 / 255.0,
    g: 20.0 / 255.0,
    b: 28.0 / 255.0,
    a: 1.0,
};

pub const ICON_LIGHT: Color = Color {
    r: 244.0 / 255.0,
    g: 245.0 / 255.0,
    b: 250.0 / 255.0,
    a: 1.0,
};

pub const ICON_MUTED: Color = Color {
    r: 210.0 / 255.0,
    g: 212.0 / 255.0,
    b: 224.0 / 255.0,
    a: 1.0,
};

pub const ICON_HEART_ACTIVE: Color = Color {
    r: 251.0 / 255.0,
    g: 113.0 / 255.0,
    b: 133.0 / 255.0,
    a: 1.0,
};

fn frosted_surface_gradient(top_alpha: f32, mid_alpha: f32, bot_dark_alpha: f32) -> Background {
    Background::Gradient(gradient::Gradient::Linear(
        gradient::Linear::new(Radians(2.45))
            .add_stop(0.0, Color::from_rgba(1.0, 1.0, 1.0, top_alpha))
            .add_stop(0.55, Color::from_rgba(1.0, 1.0, 1.0, mid_alpha))
            .add_stop(1.0, Color::from_rgba(0.07, 0.05, 0.09, bot_dark_alpha)),
    ))
}

pub fn glass_panel_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(frosted_surface_gradient(0.16, 0.07, 0.22)),
        border: border::Border {
            color: GLASS_BORDER,
            width: 1.0,
            radius: 24.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: Vector::new(0.0, 12.0),
            blur_radius: 28.0,
        },
        snap: false,
    }
}

pub fn glass_elevated_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(frosted_surface_gradient(0.22, 0.11, 0.44)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.24),
            width: 1.0,
            radius: 22.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.42),
            offset: Vector::new(0.0, 14.0),
            blur_radius: 34.0,
        },
        snap: false,
    }
}

pub fn floating_dock_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(frosted_surface_gradient(0.19, 0.09, 0.30)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.24),
            width: 1.0,
            radius: 26.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.36),
            offset: Vector::new(0.0, 14.0),
            blur_radius: 34.0,
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
        button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.14))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.22))),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: TEXT_PRIMARY,
        border: border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 12.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn close_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgba8(225, 62, 74, 0.85))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba8(195, 45, 58, 0.95))),
        _ => None,
    };
    button::Style {
        background: bg,
        text_color: TEXT_PRIMARY,
        border: border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 12.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

/// Borderless sidebar navigation item with a luminous white active pill (zero wireframe boxes).
pub fn sidebar_nav_style(active: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if active {
            let white = match status {
                button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 1.0),
                _ => Color::from_rgba(1.0, 1.0, 1.0, 0.93),
            };
            button::Style {
                background: Some(Background::Color(white)),
                text_color: TEXT_ON_PILL,
                border: border::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 16.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 12.0,
                },
                snap: false,
            }
        } else {
            let bg = match status {
                button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.11))),
                button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.17))),
                _ => None,
            };
            button::Style {
                background: bg,
                text_color: TEXT_SECONDARY,
                border: border::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: 16.0.into(),
                },
                shadow: Shadow::default(),
                snap: false,
            }
        }
    }
}

/// Frosted glass capsule pill; active state flips to crisp solid white with dark text.
pub fn glass_button_style(active: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if active {
            let white = match status {
                button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 1.0),
                button::Status::Pressed => Color::from_rgba(0.92, 0.92, 0.95, 0.96),
                _ => Color::from_rgba(1.0, 1.0, 1.0, 0.93),
            };
            button::Style {
                background: Some(Background::Color(white)),
                text_color: TEXT_ON_PILL,
                border: border::Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.95),
                    width: 1.0,
                    radius: 20.0.into(),
                },
                shadow: Shadow {
                    color: Color::from_rgba(0.0, 0.0, 0.0, 0.18),
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 14.0,
                },
                snap: false,
            }
        } else {
            let alpha = match status {
                button::Status::Hovered => 0.20,
                button::Status::Pressed => 0.26,
                button::Status::Disabled => 0.04,
                button::Status::Active => 0.11,
            };
            button::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, alpha))),
                text_color: if matches!(status, button::Status::Disabled) {
                    TEXT_MUTED
                } else {
                    TEXT_PRIMARY
                },
                border: border::Border {
                    color: GLASS_SUBTLE_BORDER,
                    width: 1.0,
                    radius: 20.0.into(),
                },
                shadow: Shadow::default(),
                snap: false,
            }
        }
    }
}

pub fn primary_pill_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg_color = match status {
        button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 1.0),
        button::Status::Pressed => Color::from_rgba(0.90, 0.90, 0.94, 1.0),
        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.94),
    };
    button::Style {
        background: Some(Background::Color(bg_color)),
        text_color: TEXT_ON_PILL,
        border: border::Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 20.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 16.0,
        },
        snap: false,
    }
}

pub fn play_circle_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg_color = match status {
        button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 1.0),
        button::Status::Pressed => Color::from_rgba(0.88, 0.88, 0.92, 0.98),
        _ => Color::from_rgba(1.0, 1.0, 1.0, 0.94),
    };
    button::Style {
        background: Some(Background::Color(bg_color)),
        text_color: TEXT_ON_PILL,
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.95),
            width: 1.0,
            radius: 24.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.25),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 14.0,
        },
        snap: false,
    }
}

pub fn glass_input_style(_theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let (bg_alpha, border_alpha) = match status {
        text_input::Status::Focused { .. } => (0.16, 0.36),
        text_input::Status::Hovered => (0.14, 0.24),
        _ => (0.10, 0.16),
    };
    text_input::Style {
        background: Background::Color(Color::from_rgba(1.0, 1.0, 1.0, bg_alpha)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, border_alpha),
            width: 1.0,
            radius: 20.0.into(),
        },
        icon: TEXT_SECONDARY,
        placeholder: TEXT_MUTED,
        value: TEXT_PRIMARY,
        selection: Color::from_rgba(1.0, 1.0, 1.0, 0.28),
    }
}

pub fn thin_scrollable_style(_theme: &iced::Theme, status: scrollable::Status) -> scrollable::Style {
    let scroller_alpha = match status {
        scrollable::Status::Hovered { .. } | scrollable::Status::Dragged { .. } => 0.38,
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
            background: Background::Color(Color::from_rgba8(28, 28, 38, 0.85)),
            border: border::color(GLASS_BORDER).width(1.0).rounded(12.0),
            shadow: Shadow::default(),
            icon: TEXT_PRIMARY,
        },
    }
}

pub fn seek_slider_style(_theme: &iced::Theme, status: slider::Status) -> slider::Style {
    let handle_radius = match status {
        slider::Status::Hovered | slider::Status::Dragged => 5.5,
        slider::Status::Active => 3.0,
    };
    slider::Style {
        rail: slider::Rail {
            backgrounds: (
                Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.94)),
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
        container(icon_music((size * 0.42).max(16.0), ICON_MUTED))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size))
            .style(move |_| container::Style {
                background: Some(frosted_surface_gradient(0.18, 0.08, 0.24)),
                border: border::Border {
                    color: GLASS_SUBTLE_BORDER,
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
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.16),
                width: 1.0,
                radius: radius.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.42),
                offset: Vector::new(0.0, 12.0),
                blur_radius: 28.0,
            },
            ..Default::default()
        })
        .into()
}
