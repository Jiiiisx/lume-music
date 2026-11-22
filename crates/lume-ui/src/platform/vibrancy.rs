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
            use raw_window_handle::RawWindowHandle;
            use windows_sys::Win32::Foundation::HWND;
            use windows_sys::Win32::Graphics::Dwm::{
                DwmExtendFrameIntoClientArea, DwmSetWindowAttribute, DWMWA_USE_IMMERSIVE_DARK_MODE,
            };
            use windows_sys::Win32::UI::Controls::MARGINS;

            let hwnd = window
                .window_handle()
                .ok()
                .and_then(|handle| match handle.as_raw() {
                    RawWindowHandle::Win32(win32) => Some(win32.hwnd.get() as HWND),
                    _ => None,
                });

            if enable_acrylic {
                if let Some(hwnd) = hwnd {
                    let margins = MARGINS {
                        cxLeftWidth: -1,
                        cxRightWidth: -1,
                        cyTopHeight: -1,
                        cyBottomHeight: -1,
                    };
                    let dark_mode: u32 = 1;
                    unsafe {
                        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
                        let _ = DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_USE_IMMERSIVE_DARK_MODE as u32,
                            (&dark_mode as *const u32).cast(),
                            std::mem::size_of::<u32>() as u32,
                        );
                    }
                }
                let _ = window_vibrancy::apply_acrylic(window, Some((18, 20, 28, 80)));
            } else {
                let _ = window_vibrancy::clear_acrylic(window);
                if let Some(hwnd) = hwnd {
                    let margins = MARGINS {
                        cxLeftWidth: 0,
                        cxRightWidth: 0,
                        cyTopHeight: 0,
                        cyBottomHeight: 0,
                    };
                    unsafe {
                        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
                    }
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (window, enable_acrylic);
    })
    .discard()
}
