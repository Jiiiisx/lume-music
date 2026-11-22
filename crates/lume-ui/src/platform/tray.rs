use tokio::sync::mpsc::UnboundedSender;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::views::TrayAction;

const ID_SHOW: &str = "lume_tray_show";
const ID_PLAY_PAUSE: &str = "lume_tray_play_pause";
const ID_NEXT: &str = "lume_tray_next";
const ID_PREV: &str = "lume_tray_prev";
const ID_QUIT: &str = "lume_tray_quit";

/// Generates a clean 32×32 RGBA Lume emblem for the Windows notification area.
fn generate_tray_rgba_icon() -> Option<Icon> {
    let size = 32usize;
    let mut rgba = Vec::with_capacity(size * size * 4);
    let center = (size as f32 - 1.0) * 0.5;
    let radius = 14.0_f32;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center;
            let dy = y as f32 - center;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= radius {
                let t = (x as f32 + y as f32) / ((size * 2) as f32);
                let r = (224.0 * (1.0 - t) + 128.0 * t) as u8;
                let g = (76.0 * (1.0 - t) + 92.0 * t) as u8;
                let b = (140.0 * (1.0 - t) + 236.0 * t) as u8;

                // Inner subtle play triangle cutout glow
                let in_tri = dx >= -3.5
                    && dx <= 5.5
                    && dy.abs() <= (5.5 - dx) * 0.62;
                if in_tri {
                    rgba.extend_from_slice(&[250, 250, 255, 255]);
                } else {
                    let edge_alpha = ((radius - dist) * 255.0).clamp(0.0, 255.0) as u8;
                    rgba.extend_from_slice(&[r, g, b, edge_alpha]);
                }
            } else {
                rgba.extend_from_slice(&[0, 0, 0, 0]);
            }
        }
    }

    Icon::from_rgba(rgba, size as u32, size as u32).ok()
}

/// Manages the Windows System Tray icon and context menu using event callbacks (0% polling).
pub struct TrayController {
    _tray: TrayIcon,
}

impl TrayController {
    pub fn new(tx: UnboundedSender<TrayAction>) -> Option<Self> {
        let menu = Menu::new();
        let show = MenuItem::with_id(ID_SHOW, "Show / Hide Lume Music", true, None);
        let play_pause = MenuItem::with_id(ID_PLAY_PAUSE, "Play / Pause", true, None);
        let next = MenuItem::with_id(ID_NEXT, "Next", true, None);
        let prev = MenuItem::with_id(ID_PREV, "Previous", true, None);
        let sep = PredefinedMenuItem::separator();
        let quit = MenuItem::with_id(ID_QUIT, "Quit", true, None);

        let _ = menu.append(&show);
        let _ = menu.append(&sep);
        let _ = menu.append(&play_pause);
        let _ = menu.append(&next);
        let _ = menu.append(&prev);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&quit);

        let icon = generate_tray_rgba_icon()?;
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Lume Music")
            .with_icon(icon)
            .with_menu_on_left_click(false)
            .build()
            .ok()?;

        let tx_click = tx.clone();
        TrayIconEvent::set_event_handler(Some(move |event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => {
                let _ = tx_click.send(TrayAction::ToggleWindow);
            }
            _ => {}
        }));

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = match event.id {
                MenuId(ref id) if id == ID_SHOW => Some(TrayAction::ToggleWindow),
                MenuId(ref id) if id == ID_PLAY_PAUSE => Some(TrayAction::PlayPause),
                MenuId(ref id) if id == ID_NEXT => Some(TrayAction::Next),
                MenuId(ref id) if id == ID_PREV => Some(TrayAction::Previous),
                MenuId(ref id) if id == ID_QUIT => Some(TrayAction::Quit),
                _ => None,
            };
            if let Some(act) = action {
                let _ = tx.send(act);
            }
        }));

        Some(Self { _tray: tray })
    }
}
