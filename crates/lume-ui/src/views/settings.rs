use std::path::Path;

use iced::widget::{button, column, container, row, scrollable, space, text, toggler};
use iced::{Alignment, Background, Color, Element, Length};
use lume_storage::{AppConfig, GlassmorphismMode, LyricsVisualMode};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::Message;
use crate::widgets::{
    glass_button_style, glass_card_style, glass_panel, icon_folder, icon_plus, icon_refresh,
    icon_trash, primary_pill_button_style, thin_scrollable_style, ICON_LIGHT, ICON_MUTED,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn view_settings<'a>(
    config: &'a AppConfig,
    scanning: bool,
    scan_summary: Option<&'a str>,
    yt_dlp_detected: Option<&'a Path>,
    accent: AccentTheme,
) -> Element<'a, Message> {

    let appearance = column![
        text("Appearance & Glassmorphism")
            .size(16)
            .font(FONT_SEMIBOLD)
            .color(TEXT_PRIMARY),
        row![
            button(text("Crystal Clear (Recommended)").size(13).font(FONT_MEDIUM))
                .padding([9, 16])
                .style(glass_button_style(
                    config.glassmorphism_mode == GlassmorphismMode::WindowsAcrylic,
                    accent,
                ))
                .on_press(Message::SetGlassmorphismMode(
                    GlassmorphismMode::WindowsAcrylic
                )),
            button(text("Ultra-Pure Glass").size(13).font(FONT_MEDIUM))
                .padding([9, 16])
                .style(glass_button_style(
                    config.glassmorphism_mode == GlassmorphismMode::PureClear,
                    accent,
                ))
                .on_press(Message::SetGlassmorphismMode(GlassmorphismMode::PureClear)),
            button(text("Album Cinema Glow").size(13).font(FONT_MEDIUM))
                .padding([9, 16])
                .style(glass_button_style(
                    config.glassmorphism_mode == GlassmorphismMode::DynamicAlbumBlur,
                    accent,
                ))
                .on_press(Message::SetGlassmorphismMode(
                    GlassmorphismMode::DynamicAlbumBlur
                )),
        ]
        .spacing(10),
        text("Lyrics Visual Mode")
            .size(15)
            .font(FONT_SEMIBOLD)
            .color(TEXT_PRIMARY),
        row![
            button(text("Spicy Smooth (Spring + Depth)").size(13).font(FONT_MEDIUM))
                .padding([9, 16])
                .style(glass_button_style(
                    config.lyrics_visual_mode == LyricsVisualMode::SpicySmooth,
                    accent,
                ))
                .on_press(Message::SetLyricsVisualMode(LyricsVisualMode::SpicySmooth)),
            button(text("Ultra-Light (Static Minimal)").size(13).font(FONT_MEDIUM))
                .padding([9, 16])
                .style(glass_button_style(
                    config.lyrics_visual_mode == LyricsVisualMode::UltraLight,
                    accent,
                ))
                .on_press(Message::SetLyricsVisualMode(LyricsVisualMode::UltraLight)),
        ]
        .spacing(10),
        toggler(config.reduced_motion)
            .label("Reduced Motion (disables shader & spring frame ticks)")
            .on_toggle(Message::SetReducedMotion)
            .text_size(13),
    ]
    .spacing(12);

    let behavior = column![
        text("Desktop & Playback Behavior")
            .size(16)
            .font(FONT_SEMIBOLD)
            .color(TEXT_PRIMARY),
        toggler(config.prefer_youtube_fallback)
            .label("Prefer YouTube Fallback when playing Spotify tracks (Phase 3)")
            .on_toggle(Message::SetPreferYoutubeFallback)
            .text_size(13),
    ]
    .spacing(12);

    let mut folder_rows = column![].spacing(8);
    if config.local_folders.is_empty() {
        folder_rows = folder_rows.push(
            container(
                text("No local folders added yet. Click \"Add Folder\" to index your music.")
                    .size(13)
                    .color(TEXT_MUTED),
            )
            .padding([12, 16])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.045))),
                border: iced::border::rounded(14.0),
                ..Default::default()
            }),
        );
    } else {
        for folder in &config.local_folders {
            let label = folder.to_string_lossy().into_owned();
            folder_rows = folder_rows.push(
                container(
                    row![
                        icon_folder(16.0, ICON_MUTED),
                        text(label)
                            .size(13)
                            .font(FONT_MEDIUM)
                            .color(TEXT_PRIMARY)
                            .width(Length::Fill),
                        button(
                            row![
                                icon_trash(12.0, ICON_LIGHT),
                                text("Remove").size(12).font(FONT_MEDIUM)
                            ]
                            .spacing(6)
                            .align_y(Alignment::Center),
                        )
                        .padding([6, 12])
                        .style(glass_button_style(false, accent))
                        .on_press(Message::RemoveLocalFolder(folder.clone())),
                    ]
                    .align_y(Alignment::Center)
                    .spacing(10),
                )
                .padding([10, 14])
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.055))),
                    border: iced::border::Border {
                        color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
                        width: 1.0,
                        radius: 14.0.into(),
                    },
                    ..Default::default()
                }),
            );
        }
    }

    let local_library = column![
        row![
            text("Local Audio Folders")
                .size(16)
                .font(FONT_SEMIBOLD)
                .color(TEXT_PRIMARY),
            space().width(Length::Fill),
            button(
                row![
                    icon_plus(13.0, accent.on_primary),
                    text("Add Folder")
                        .size(12)
                        .font(FONT_SEMIBOLD)
                        .color(accent.on_primary)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 16])
            .style(primary_pill_button_style(accent))
            .on_press(Message::PickLocalFolder),
            button(
                row![
                    icon_refresh(13.0, ICON_LIGHT),
                    text(if scanning { "Scanning..." } else { "Rescan All" })
                        .size(12)
                        .font(FONT_MEDIUM),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 16])
            .style(glass_button_style(false, accent))
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
    .spacing(12);

    let yt_dlp_status = if let Some(cfg_path) = &config.yt_dlp_path {
        if cfg_path.is_file() {
            format!("Configured fallback: {}", cfg_path.display())
        } else {
            format!(
                "Configured path missing: {} (will check PATH instead)",
                cfg_path.display()
            )
        }
    } else if let Some(found) = yt_dlp_detected {
        format!("Auto-detected on PATH: {}", found.display())
    } else {
        "Not detected on PATH — RustyPipe works anonymously by default. Install yt-dlp (`winget install yt-dlp.yt-dlp`) or pick yt-dlp.exe for anti-bot fallback.".to_string()
    };

    let mut ytm_actions = row![
        button(
            row![
                icon_folder(13.0, accent.on_primary),
                text("Select yt-dlp.exe")
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .color(accent.on_primary),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([8, 16])
        .style(primary_pill_button_style(accent))
        .on_press(Message::PickYtDlpBinary),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if config.yt_dlp_path.is_some() {
        ytm_actions = ytm_actions.push(
            button(text("Clear").size(12).font(FONT_MEDIUM))
                .padding([8, 14])
                .style(glass_button_style(false, accent))
                .on_press(Message::ClearYtDlpBinary),
        );
    }

    let youtube_section = column![
        row![
            text("YouTube Music & yt-dlp Fallback")
                .size(16)
                .font(FONT_SEMIBOLD)
                .color(TEXT_PRIMARY),
            space().width(Length::Fill),
            ytm_actions,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text(yt_dlp_status).size(12).color(TEXT_SECONDARY),
    ]
    .spacing(10);

    let appearance_card = container(appearance.width(Length::Fill))
        .padding(18)
        .width(Length::Fill)
        .style(glass_card_style);

    let behavior_card = container(behavior.width(Length::Fill))
        .padding(18)
        .width(Length::Fill)
        .style(glass_card_style);

    let youtube_card = container(youtube_section.width(Length::Fill))
        .padding(18)
        .width(Length::Fill)
        .style(glass_card_style);

    let local_library_card = container(local_library.width(Length::Fill))
        .padding(18)
        .width(Length::Fill)
        .style(glass_card_style);

    glass_panel(
        scrollable(
            column![
                text("Settings").size(24).font(FONT_BOLD).color(TEXT_PRIMARY),
                appearance_card,
                behavior_card,
                youtube_card,
                local_library_card,
            ]
            .spacing(16)
            .padding(20),
        )
        .style(thin_scrollable_style)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
