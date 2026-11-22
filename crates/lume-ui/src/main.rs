#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use iced::{window, Size};
use lume_ui::app::LumeApp;

pub fn main() -> iced::Result {
    // Force DX12 + DirectComposition (`DxgiFromVisual`) on Windows 11 so per-pixel
    // window transparency and native Acrylic blur behind the window actually compose.
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("WGPU_BACKEND").is_none() {
            std::env::set_var("WGPU_BACKEND", "dx12");
        }
        if std::env::var_os("WGPU_DX12_PRESENTATION_SYSTEM").is_none() {
            std::env::set_var("WGPU_DX12_PRESENTATION_SYSTEM", "DxgiFromVisual");
        }
    }

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
