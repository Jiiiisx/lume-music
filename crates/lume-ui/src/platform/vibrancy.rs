use iced::Task;
use lume_storage::GlassmorphismMode;

use crate::views::Message;

pub fn apply_window_glass_mode(
    id: iced::window::Id,
    mode: GlassmorphismMode,
    focused: bool,
) -> Task<Message> {
    iced::window::run(id, move |window| {
        #[cfg(target_os = "windows")]
        {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            use windows::Win32::Foundation::HWND;

            let hwnd = HasWindowHandle::window_handle(window)
                .ok()
                .and_then(|handle| match handle.as_raw() {
                    RawWindowHandle::Win32(win32) => {
                        Some(HWND(win32.hwnd.get() as *mut core::ffi::c_void))
                    }
                    _ => None,
                });

            if let Some(hwnd) = hwnd {
                win_backdrop::apply(hwnd, mode, focused);
            }
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (window, mode, focused);
    })
    .discard()
}

#[cfg(target_os = "windows")]
mod win_backdrop {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use lume_storage::GlassmorphismMode;
    use windows::core::Interface;
    use windows::Foundation::Numerics::Vector2;
    use windows::System::{DispatcherQueue, DispatcherQueueController};
    use windows::UI::Color;
    use windows::UI::Composition::Desktop::DesktopWindowTarget;
    use windows::UI::Composition::{
        CompositionColorBrush, Compositor, ContainerVisual, SpriteVisual, Visual,
    };
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMSBT_NONE, DWMWA_SYSTEMBACKDROP_TYPE,
        DWMWA_USE_HOSTBACKDROPBRUSH, DWMWA_USE_IMMERSIVE_DARK_MODE,
    };
    use windows::Win32::System::WinRT::Composition::ICompositorDesktopInterop;
    use windows::Win32::System::WinRT::{
        CreateDispatcherQueueController, DispatcherQueueOptions, DQTAT_COM_NONE,
        DQTYPE_THREAD_CURRENT,
    };

    struct WindowBackdropEntry {
        _target: DesktopWindowTarget,
        _root: ContainerVisual,
        blur_visual: SpriteVisual,
        tint_brush: CompositionColorBrush,
        _tint_visual: SpriteVisual,
    }

    #[derive(Default)]
    struct ThreadBackdropManager {
        _dq_controller: Option<DispatcherQueueController>,
        compositor: Option<Compositor>,
        windows: HashMap<usize, WindowBackdropEntry>,
    }

    thread_local! {
        static MANAGER: RefCell<ThreadBackdropManager> = RefCell::new(ThreadBackdropManager::default());
    }

    fn focused_tint(mode: GlassmorphismMode) -> Color {
        match mode {
            GlassmorphismMode::PureClear => Color {
                A: 4,
                R: 16,
                G: 16,
                B: 18,
            },
            GlassmorphismMode::WindowsAcrylic => Color {
                A: 12,
                R: 16,
                G: 16,
                B: 18,
            },
            GlassmorphismMode::DynamicAlbumBlur => Color {
                A: 24,
                R: 14,
                G: 14,
                B: 16,
            },
        }
    }

    fn unfocused_tint() -> Color {
        Color {
            A: 64,
            R: 16,
            G: 16,
            B: 18,
        }
    }

    fn create_entry(
        compositor: &Compositor,
        hwnd: HWND,
        initial_tint: Color,
    ) -> windows::core::Result<WindowBackdropEntry> {
        unsafe {
            let enable: u32 = 1;
            let backdrop_none: i32 = DWMSBT_NONE.0;
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&raw const enable).cast(),
                std::mem::size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_HOSTBACKDROPBRUSH,
                (&raw const enable).cast(),
                std::mem::size_of::<u32>() as u32,
            );
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_SYSTEMBACKDROP_TYPE,
                (&raw const backdrop_none).cast(),
                std::mem::size_of::<i32>() as u32,
            );
        }

        let interop: ICompositorDesktopInterop = compositor.cast()?;
        let target = unsafe { interop.CreateDesktopWindowTarget(hwnd, false)? };

        let root = compositor.CreateContainerVisual()?;
        root.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;

        let blur_visual = compositor.CreateSpriteVisual()?;
        blur_visual.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        let host_brush = compositor.CreateHostBackdropBrush()?;
        blur_visual.SetBrush(&host_brush)?;

        let tint_brush = compositor.CreateColorBrushWithColor(initial_tint)?;
        let tint_visual = compositor.CreateSpriteVisual()?;
        tint_visual.SetRelativeSizeAdjustment(Vector2 { X: 1.0, Y: 1.0 })?;
        tint_visual.SetBrush(&tint_brush)?;

        let children = root.Children()?;
        children.InsertAtTop(&blur_visual.cast::<Visual>()?)?;
        children.InsertAtTop(&tint_visual.cast::<Visual>()?)?;

        target.SetRoot(&root.cast::<Visual>()?)?;

        Ok(WindowBackdropEntry {
            _target: target,
            _root: root,
            blur_visual,
            tint_brush,
            _tint_visual: tint_visual,
        })
    }

    pub fn apply(hwnd: HWND, mode: GlassmorphismMode, focused: bool) {
        MANAGER.with(|cell| {
            let mut mgr = cell.borrow_mut();

            if mgr._dq_controller.is_none() && DispatcherQueue::GetForCurrentThread().is_err() {
                let options = DispatcherQueueOptions {
                    dwSize: std::mem::size_of::<DispatcherQueueOptions>() as u32,
                    threadType: DQTYPE_THREAD_CURRENT,
                    apartmentType: DQTAT_COM_NONE,
                };
                if let Ok(controller) = unsafe { CreateDispatcherQueueController(options) } {
                    mgr._dq_controller = Some(controller);
                }
            }

            if mgr.compositor.is_none() {
                if let Ok(comp) = Compositor::new() {
                    mgr.compositor = Some(comp);
                }
            }

            let Some(compositor) = mgr.compositor.as_ref().cloned() else {
                return;
            };

            let hwnd_key = hwnd.0 as usize;
            if !mgr.windows.contains_key(&hwnd_key) {
                let init_color = if focused {
                    focused_tint(mode)
                } else {
                    unfocused_tint()
                };
                if let Ok(entry) = create_entry(&compositor, hwnd, init_color) {
                    mgr.windows.insert(hwnd_key, entry);
                }
            }

            if let Some(entry) = mgr.windows.get(&hwnd_key) {
                if focused {
                    let _ = entry.blur_visual.SetIsVisible(true);
                    let _ = entry.tint_brush.SetColor(focused_tint(mode));
                } else {
                    let _ = entry.blur_visual.SetIsVisible(false);
                    let _ = entry.tint_brush.SetColor(unfocused_tint());
                }
            }
        });
    }
}
