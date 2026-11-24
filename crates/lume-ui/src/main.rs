#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use iced::{window, Size};
use lume_ui::app::LumeApp;
use lume_ui::theme::{
    FONT_REGULAR, PLUS_JAKARTA_BOLD, PLUS_JAKARTA_MEDIUM, PLUS_JAKARTA_REGULAR,
    PLUS_JAKARTA_SEMIBOLD,
};

const APP_ICON_PNG: &[u8] = include_bytes!("../assets/icons/lume-music-64.png");

fn load_window_icon() -> Option<window::Icon> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    window::icon::from_rgba(img.into_raw(), w, h).ok()
}

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
        icon: load_window_icon(),
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
        .font(PLUS_JAKARTA_REGULAR)
        .font(PLUS_JAKARTA_MEDIUM)
        .font(PLUS_JAKARTA_SEMIBOLD)
        .font(PLUS_JAKARTA_BOLD)
        .default_font(FONT_REGULAR)
        .window(win_settings)
        .centered()
        .run()
}
