use iced::border;
use iced::gradient;
use iced::widget::{button, container, image, scrollable, slider, text_input, Container};
use iced::{Background, Color, ContentFit, Element, Length, Radians, Shadow, Vector};

use crate::theme::AccentTheme;
use crate::widgets::icons::icon_music;

/// Pure neutral white specular borders for Ultra-Clear Crystal Glass.
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
    a: 0.10,
};

pub const TEXT_PRIMARY: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.96,
};

pub const TEXT_SECONDARY: Color = Color {
    r: 0.91,
    g: 0.92,
    b: 0.94,
    a: 0.74,
};

pub const TEXT_MUTED: Color = Color {
    r: 0.76,
    g: 0.77,
    b: 0.80,
    a: 0.56,
};

pub const TEXT_ON_ACCENT: Color = Color {
    r: 14.0 / 255.0,
    g: 15.0 / 255.0,
    b: 18.0 / 255.0,
    a: 0.98,
};

pub const ICON_LIGHT: Color = Color {
    r: 0.97,
    g: 0.97,
    b: 0.99,
    a: 0.96,
};

pub const ICON_MUTED: Color = Color {
    r: 0.78,
    g: 0.79,
    b: 0.82,
    a: 0.74,
};

pub const ICON_HEART_ACTIVE: Color = Color {
    r: 251.0 / 255.0,
    g: 113.0 / 255.0,
    b: 133.0 / 255.0,
    a: 1.0,
};

/// Smoked dark obsidian glass surface: keeps subtle top-left glass sheen while providing
/// a dark translucent backing so white text stays crisp and readable over bright wallpapers.
pub fn crystal_surface_gradient(top_alpha: f32, mid_alpha: f32, bot_alpha: f32) -> Background {
    Background::Gradient(gradient::Gradient::Linear(
        gradient::Linear::new(Radians(2.42))
            .add_stop(0.0, Color::from_rgba(0.08, 0.08, 0.11, top_alpha))
            .add_stop(0.45, Color::from_rgba(0.05, 0.05, 0.07, mid_alpha))
            .add_stop(1.0, Color::from_rgba(0.03, 0.03, 0.05, bot_alpha)),
    ))
}

pub fn inner_widget_glass_gradient() -> Background {
    Background::Gradient(gradient::Gradient::Linear(
        gradient::Linear::new(Radians(2.45))
            .add_stop(0.0, Color::from_rgba(0.11, 0.11, 0.14, 0.44))
            .add_stop(0.50, Color::from_rgba(0.07, 0.07, 0.10, 0.48))
            .add_stop(1.0, Color::from_rgba(0.04, 0.04, 0.06, 0.56)),
    ))
}

pub fn glass_sidebar_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(crystal_surface_gradient(0.46, 0.54, 0.62)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.17),
            width: 1.0,
            radius: 20.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.28),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 26.0,
        },
        snap: false,
    }
}

pub fn glass_panel_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(crystal_surface_gradient(0.42, 0.50, 0.58)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.15),
            width: 1.0,
            radius: 22.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.24),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 28.0,
        },
        snap: false,
    }
}

pub fn glass_elevated_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(Background::Gradient(gradient::Gradient::Linear(
            gradient::Linear::new(Radians(2.40))
                .add_stop(0.0, Color::from_rgba(0.12, 0.12, 0.15, 0.80))
                .add_stop(0.45, Color::from_rgba(0.08, 0.08, 0.10, 0.78))
                .add_stop(1.0, Color::from_rgba(0.05, 0.05, 0.07, 0.86)),
        ))),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.20),
            width: 1.0,
            radius: 20.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.42),
            offset: Vector::new(0.0, 16.0),
            blur_radius: 36.0,
        },
        snap: false,
    }
}

pub fn floating_dock_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(crystal_surface_gradient(0.54, 0.62, 0.70)),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.18),
            width: 1.0,
            radius: 22.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.30),
            offset: Vector::new(0.0, 10.0),
            blur_radius: 28.0,
        },
        snap: false,
    }
}

pub fn glass_card_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: Some(inner_widget_glass_gradient()),
        border: border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
            width: 1.0,
            radius: 18.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.22),
            offset: Vector::new(0.0, 8.0),
            blur_radius: 18.0,
        },
        snap: false,
    }
}

pub fn hero_banner_style(accent: AccentTheme) -> impl Fn(&iced::Theme) -> container::Style {
    move |_theme| {
        let top_tint = Color {
            r: accent.light.r * 0.24 + 0.05,
            g: accent.light.g * 0.24 + 0.05,
            b: accent.light.b * 0.24 + 0.07,
            a: 0.46,
        };
        let mid_tint = Color::from_rgba(0.06, 0.06, 0.09, 0.46);
        let bot_tint = Color::from_rgba(0.04, 0.04, 0.06, 0.56);
        container::Style {
            text_color: Some(TEXT_PRIMARY),
            background: Some(Background::Gradient(gradient::Gradient::Linear(
                gradient::Linear::new(Radians(2.28))
                    .add_stop(0.0, top_tint)
                    .add_stop(0.52, mid_tint)
                    .add_stop(1.0, bot_tint),
            ))),
            border: border::Border {
                color: Color {
                    a: 0.20,
                    ..accent.light
                },
                width: 1.0,
                radius: 20.0.into(),
            },
            shadow: Shadow {
                color: Color {
                    a: 0.16,
                    ..accent.glow
                },
                offset: Vector::new(0.0, 8.0),
                blur_radius: 22.0,
            },
            snap: false,
        }
    }
}

pub fn glass_panel<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Container<'a, Message> {
    container(content).style(glass_panel_style)
}

pub fn ghost_button_style(_theme: &iced::Theme, status: button::Status) -> button::Style {
    let bg = match status {
        button::Status::Hovered => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.11))),
        button::Status::Pressed => Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.19))),
        _ => None,
    };
    let border_col = match status {
        button::Status::Hovered => Color::from_rgba(1.0, 1.0, 1.0, 0.16),
        button::Status::Pressed => Color::from_rgba(1.0, 1.0, 1.0, 0.26),
        _ => Color::TRANSPARENT,
    };
    button::Style {
        background: bg,
        text_color: TEXT_PRIMARY,
        border: border::Border {
            color: border_col,
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
    }
}

pub fn traffic_light_button_style(
    base_rgb: (u8, u8, u8),
    hover_rgb: (u8, u8, u8),
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let color = match status {
            button::Status::Hovered => Color::from_rgb8(hover_rgb.0, hover_rgb.1, hover_rgb.2),
            button::Status::Pressed => Color::from_rgba8(base_rgb.0, base_rgb.1, base_rgb.2, 0.8),
            _ => Color::from_rgb8(base_rgb.0, base_rgb.1, base_rgb.2),
        };
        let shadow = if matches!(status, button::Status::Hovered) {
            Shadow {
                color: Color::from_rgba8(base_rgb.0, base_rgb.1, base_rgb.2, 0.55),
                offset: Vector::new(0.0, 0.0),
                blur_radius: 8.0,
            }
        } else {
            Shadow::default()
        };
        button::Style {
            background: Some(Background::Color(color)),
            text_color: Color::TRANSPARENT,
            border: border::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.30),
                width: 0.75,
                radius: 7.0.into(),
            },
            shadow,
            snap: false,
        }
    }
}

pub fn sidebar_nav_style(
    active: bool,
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if active {
            let base_alpha = accent.soft_bg.a;
            let bg = match status {
                button::Status::Hovered => Color {
                    a: (base_alpha + 0.06).min(0.28),
                    ..accent.light
                },
                button::Status::Pressed => Color {
                    a: (base_alpha + 0.11).min(0.34),
                    ..accent.light
                },
                _ => Color {
                    a: base_alpha,
                    ..accent.light
                },
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: TEXT_PRIMARY,
                border: border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: 13.0.into(),
                },
                shadow: Shadow {
                    color: Color {
                        a: 0.14,
                        ..accent.glow
                    },
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 14.0,
                },
                snap: false,
            }
        } else {
            let (bg, border_col) = match status {
                button::Status::Hovered => (
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.075))),
                    Color::from_rgba(1.0, 1.0, 1.0, 0.14),
                ),
                button::Status::Pressed => (
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.13))),
                    Color::from_rgba(1.0, 1.0, 1.0, 0.22),
                ),
                _ => (None, Color::TRANSPARENT),
            };
            button::Style {
                background: bg,
                text_color: TEXT_SECONDARY,
                border: border::Border {
                    color: border_col,
                    width: 1.0,
                    radius: 13.0.into(),
                },
                shadow: Shadow::default(),
                snap: false,
            }
        }
    }
}

pub fn glass_button_style(
    active: bool,
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if active {
            let bg = match status {
                button::Status::Hovered => accent.light,
                button::Status::Pressed => accent.pressed,
                _ => accent.primary,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: accent.on_primary,
                border: border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: 16.0.into(),
                },
                shadow: Shadow {
                    color: accent.glow,
                    offset: Vector::new(0.0, 4.0),
                    blur_radius: 14.0,
                },
                snap: false,
            }
        } else {
            let (bg_color, border_alpha) = match status {
                button::Status::Hovered => (Color::from_rgba(0.14, 0.15, 0.19, 0.62), 0.24),
                button::Status::Pressed => (Color::from_rgba(0.18, 0.19, 0.24, 0.72), 0.32),
                button::Status::Disabled => (Color::from_rgba(0.06, 0.06, 0.09, 0.30), 0.08),
                button::Status::Active => (Color::from_rgba(0.07, 0.08, 0.11, 0.48), 0.15),
            };
            button::Style {
                background: Some(Background::Color(bg_color)),
                text_color: if matches!(status, button::Status::Disabled) {
                    TEXT_MUTED
                } else {
                    TEXT_PRIMARY
                },
                border: border::Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, border_alpha),
                    width: 1.0,
                    radius: 16.0.into(),
                },
                shadow: Shadow::default(),
                snap: false,
            }
        }
    }
}

pub fn primary_pill_button_style(
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let bg_color = match status {
            button::Status::Hovered => accent.light,
            button::Status::Pressed => accent.pressed,
            _ => accent.primary,
        };
        let shadow_blur = if matches!(status, button::Status::Hovered) {
            22.0
        } else {
            16.0
        };
        button::Style {
            background: Some(Background::Color(bg_color)),
            text_color: accent.on_primary,
            border: border::Border {
                color: accent.border,
                width: 1.0,
                radius: 18.0.into(),
            },
            shadow: Shadow {
                color: accent.glow,
                offset: Vector::new(0.0, 5.0),
                blur_radius: shadow_blur,
            },
            snap: false,
        }
    }
}

pub fn play_circle_button_style(
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let bg_color = match status {
            button::Status::Hovered => accent.light,
            button::Status::Pressed => accent.pressed,
            _ => accent.primary,
        };
        let blur = if matches!(status, button::Status::Hovered) {
            22.0
        } else {
            16.0
        };
        button::Style {
            background: Some(Background::Color(bg_color)),
            text_color: accent.on_primary,
            border: border::Border {
                color: accent.border,
                width: 1.0,
                radius: 24.0.into(),
            },
            shadow: Shadow {
                color: accent.glow,
                offset: Vector::new(0.0, 5.0),
                blur_radius: blur,
            },
            snap: false,
        }
    }
}

pub fn track_row_button_style(
    active: bool,
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        if active {
            let bg = match status {
                button::Status::Hovered => Color {
                    a: (accent.soft_bg.a + 0.06).min(0.24),
                    ..accent.light
                },
                button::Status::Pressed => Color {
                    a: (accent.soft_bg.a + 0.11).min(0.30),
                    ..accent.light
                },
                _ => accent.soft_bg,
            };
            button::Style {
                background: Some(Background::Color(bg)),
                text_color: TEXT_PRIMARY,
                border: border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: Shadow::default(),
                snap: false,
            }
        } else {
            let (bg, border_col) = match status {
                button::Status::Hovered => (
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.07))),
                    Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                ),
                button::Status::Pressed => (
                    Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.12))),
                    Color::from_rgba(1.0, 1.0, 1.0, 0.20),
                ),
                _ => (None, Color::TRANSPARENT),
            };
            button::Style {
                background: bg,
                text_color: TEXT_PRIMARY,
                border: border::Border {
                    color: border_col,
                    width: 1.0,
                    radius: 12.0.into(),
                },
                shadow: Shadow::default(),
                snap: false,
            }
        }
    }
}

pub fn glass_input_style(
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, text_input::Status) -> text_input::Style {
    move |_theme, status| {
        let (bg_color, border_col) = match status {
            text_input::Status::Focused { .. } => (
                Color::from_rgba(0.08, 0.09, 0.13, 0.68),
                accent.border,
            ),
            text_input::Status::Hovered => (
                Color::from_rgba(0.07, 0.08, 0.11, 0.58),
                Color::from_rgba(1.0, 1.0, 1.0, 0.24),
            ),
            _ => (
                Color::from_rgba(0.05, 0.06, 0.09, 0.50),
                Color::from_rgba(1.0, 1.0, 1.0, 0.15),
            ),
        };
        text_input::Style {
            background: Background::Color(bg_color),
            border: border::Border {
                color: border_col,
                width: 1.0,
                radius: 16.0.into(),
            },
            icon: TEXT_SECONDARY,
            placeholder: TEXT_MUTED,
            value: TEXT_PRIMARY,
            selection: Color {
                a: 0.30,
                ..accent.light
            },
        }
    }
}

pub fn hidden_scrollbar() -> scrollable::Scrollbar {
    scrollable::Scrollbar::new()
        .width(0)
        .scroller_width(0)
        .margin(0)
}

pub fn thin_scrollable_style(
    _theme: &iced::Theme,
    _status: scrollable::Status,
) -> scrollable::Style {
    let rail = scrollable::Rail {
        background: None,
        border: border::Border::default(),
        scroller: scrollable::Scroller {
            background: Background::Color(Color::TRANSPARENT),
            border: border::Border::default(),
        },
    };
    scrollable::Style {
        container: container::Style::default(),
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        auto_scroll: scrollable::AutoScroll {
            background: Background::Color(Color::from_rgba8(18, 18, 22, 0.84)),
            border: border::color(GLASS_BORDER).width(1.0).rounded(12.0),
            shadow: Shadow::default(),
            icon: TEXT_PRIMARY,
        },
    }
}

pub fn seek_slider_style(
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, slider::Status) -> slider::Style {
    move |_theme, status| {
        let handle_radius = match status {
            slider::Status::Hovered | slider::Status::Dragged => 5.5,
            slider::Status::Active => 3.5,
        };
        slider::Style {
            rail: slider::Rail {
                backgrounds: (
                    Background::Color(accent.primary),
                    Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.18)),
                ),
                width: 3.5,
                border: border::rounded(2.0),
            },
            handle: slider::Handle {
                shape: slider::HandleShape::Circle {
                    radius: handle_radius,
                },
                background: Background::Color(Color::WHITE),
                border_width: 1.0,
                border_color: accent.border,
            },
        }
    }
}

pub fn artist_link_button_style(
    accent: AccentTheme,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme, status| {
        let text_color = match status {
            button::Status::Hovered | button::Status::Pressed => accent.light,
            _ => TEXT_SECONDARY,
        };
        button::Style {
            background: None,
            text_color,
            border: border::Border::default(),
            shadow: Shadow::default(),
            snap: false,
        }
    }
}

pub fn track_row_surface_style(
    active: bool,
    accent: AccentTheme,
) -> impl Fn(&iced::Theme) -> container::Style {
    move |_theme| container::Style {
        text_color: Some(TEXT_PRIMARY),
        background: if active {
            Some(Background::Color(accent.soft_bg))
        } else {
            None
        },
        border: border::Border {
            color: if active {
                accent.border
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: 12.0.into(),
        },
        shadow: Shadow::default(),
        snap: false,
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
                background: Some(inner_widget_glass_gradient()),
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
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.20),
                width: 1.0,
                radius: radius.into(),
            },
            shadow: Shadow {
                color: Color::from_rgba(0.0, 0.0, 0.0, 0.26),
                offset: Vector::new(0.0, 8.0),
                blur_radius: 20.0,
            },
            ..Default::default()
        })
        .into()
}
