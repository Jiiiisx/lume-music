use iced::Task;
use lume_storage::GlassmorphismMode;

use crate::views::{Message, ViewMode};

pub fn apply_window_glass_mode(
    id: iced::window::Id,
    mode: GlassmorphismMode,
    view: ViewMode,
) -> Task<Message> {
    let enable_acrylic = mode == GlassmorphismMode::WindowsAcrylic && view == ViewMode::Browse;
    iced::window::run(id, move |window| {
        #[cfg(target_os = "windows")]
        {
            if enable_acrylic {
                let _ = window_vibrancy::apply_acrylic(window, Some((18, 18, 26, 175)));
            } else {
                let _ = window_vibrancy::clear_acrylic(window);
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (window, enable_acrylic);
    })
    .discard()
}
