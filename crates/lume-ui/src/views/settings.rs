use iced::widget::{button, column, container, row, scrollable, space, text, toggler};
use iced::{Alignment, Background, Color, Element, Length};
use lume_storage::{AppConfig, GlassmorphismMode, LyricsVisualMode};

use crate::views::Message;
use crate::widgets::{
    glass_button_style, glass_panel, primary_pill_button_style, thin_scrollable_style,
    GLASS_BORDER, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn view_settings<'a>(
    config: &'a AppConfig,
    scanning: bool,
    scan_summary: Option<&'a str>,
) -> Element<'a, Message> {
    let appearance = column![
        text("Appearance & Glassmorphism").size(16).color(TEXT_PRIMARY),
        row![
            button(text("Dynamic Album Blur (Default)").size(13))
                .padding([8, 14])
                .style(glass_button_style(
                    config.glassmorphism_mode == GlassmorphismMode::DynamicAlbumBlur
                ))
                .on_press(Message::SetGlassmorphismMode(
                    GlassmorphismMode::DynamicAlbumBlur
                )),
            button(text("Windows 11 Native Acrylic").size(13))
                .padding([8, 14])
                .style(glass_button_style(
                    config.glassmorphism_mode == GlassmorphismMode::WindowsAcrylic
                ))
                .on_press(Message::SetGlassmorphismMode(
                    GlassmorphismMode::WindowsAcrylic
                )),
        ]
        .spacing(10),
        text("Lyrics Visual Mode").size(15).color(TEXT_PRIMARY),
        row![
            button(text("Spicy Smooth (Spring + Depth)").size(13))
                .padding([8, 14])
                .style(glass_button_style(
                    config.lyrics_visual_mode == LyricsVisualMode::SpicySmooth
                ))
                .on_press(Message::SetLyricsVisualMode(LyricsVisualMode::SpicySmooth)),
            button(text("Ultra-Light (Static Minimal)").size(13))
                .padding([8, 14])
                .style(glass_button_style(
                    config.lyrics_visual_mode == LyricsVisualMode::UltraLight
                ))
                .on_press(Message::SetLyricsVisualMode(LyricsVisualMode::UltraLight)),
        ]
        .spacing(10),
        toggler(config.reduced_motion)
            .label("Reduced Motion (disables shader & spring frame ticks)")
            .on_toggle(Message::SetReducedMotion)
            .text_size(13),
    ]
    .spacing(10);

    let behavior = column![
        text("Desktop & Playback Behavior").size(16).color(TEXT_PRIMARY),
        toggler(config.close_to_tray)
            .label("Close button (×) minimizes to System Tray")
            .on_toggle(Message::SetCloseToTray)
            .text_size(13),
        toggler(config.prefer_youtube_fallback)
            .label("Prefer YouTube Fallback when playing Spotify tracks (Phase 3)")
            .on_toggle(Message::SetPreferYoutubeFallback)
            .text_size(13),
    ]
    .spacing(10);

    let mut folder_rows = column![].spacing(6);
    if config.local_folders.is_empty() {
        folder_rows = folder_rows.push(
            text("No local folders added yet.").size(13).color(TEXT_MUTED),
        );
    } else {
        for folder in &config.local_folders {
            let label = folder.to_string_lossy().into_owned();
            folder_rows = folder_rows.push(
                container(
                    row![
                        text(label).size(13).color(TEXT_PRIMARY).width(Length::Fill),
                        button(text("Remove").size(11))
                            .padding([4, 10])
                            .style(glass_button_style(false))
                            .on_press(Message::RemoveLocalFolder(folder.clone())),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(10),
                )
                .padding([8, 12])
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                    border: iced::border::rounded(8.0),
                    ..Default::default()
                }),
            );
        }
    }

    let local_library = column![
        row![
            text("Local Audio Folders").size(16).color(TEXT_PRIMARY),
            space().width(Length::Fill),
            button(text("+ Add Folder").size(12))
                .padding([7, 14])
                .style(primary_pill_button_style)
                .on_press(Message::PickLocalFolder),
            button(text(if scanning { "Scanning..." } else { "Rescan All" }).size(12))
                .padding([7, 14])
                .style(glass_button_style(false))
                .on_press(Message::RescanLocalFolders),
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        if let Some(summary) = scan_summary {
            Element::from(text(summary).size(12).color(TEXT_SECONDARY))
        } else {
            Element::from(space().height(0))
        },
        folder_rows,
    ]
    .spacing(10);

    let divider = || {
        container(space().height(1))
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(GLASS_BORDER)),
                ..Default::default()
            })
    };

    glass_panel(
        scrollable(
            column![
                text("Settings").size(22).color(TEXT_PRIMARY),
                appearance,
                divider(),
                behavior,
                divider(),
                local_library,
            ]
            .spacing(18)
            .padding(22),
        )
        .style(thin_scrollable_style)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
