#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use iced::{window, Size};
use lume_ui::app::LumeApp;

pub fn main() -> iced::Result {
    let mut win_settings = window::Settings {
        size: Size::new(1180.0, 760.0),
        min_size: Some(Size::new(860.0, 560.0)),
        transparent: true,
        decorations: false,
        exit_on_close_request: false,
        ..Default::default()
    };

    #[cfg(target_os = "windows")]
    {
        win_settings.platform_specific.corner_preference =
            window::settings::platform::CornerPreference::Round;
    }

    iced::application(LumeApp::boot, LumeApp::update, LumeApp::view)
        .title(|state: &LumeApp| state.title())
        .subscription(LumeApp::subscription)
        .theme(LumeApp::theme)
        .style(LumeApp::style)
        .window(win_settings)
        .centered()
        .run()
}
