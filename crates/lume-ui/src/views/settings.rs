use std::path::Path;

use iced::widget::{button, column, container, row, scrollable, space, text, text_input, toggler};
use iced::{Alignment, Background, Color, Element, Length};
use lume_providers::{SpotifyAccountTier, SPOTIFY_WEB_API_REDIRECT_URI};
use lume_storage::{AppConfig, GlassmorphismMode, LyricsVisualMode};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{Message, SpotifySettingsProps};
use crate::widgets::{
    glass_button_style, glass_card_style, glass_input_style, glass_panel, hidden_scrollbar,
    icon_folder, icon_link, icon_plus, icon_refresh, icon_trash, primary_pill_button_style,
    thin_scrollable_style, ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn view_settings<'a>(
    config: &'a AppConfig,
    scanning: bool,
    scan_summary: Option<&'a str>,
    yt_dlp_detected: Option<&'a Path>,
    spotify: SpotifySettingsProps<'a>,
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
        .spacing(10)
        .wrap(),
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
        .spacing(10)
        .wrap(),
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
            .label("Prefer YouTube Music Fallback when playing Spotify tracks")
            .on_toggle(Message::SetPreferYoutubeFallback)
            .text_size(13),
    ]
    .spacing(12);

    let web_status_text = if spotify.web_api_connected {
        let who = spotify.profile_name.unwrap_or("Connected account");
        let tier_label = if spotify.lapsed_fallback {
            "Lapsed / 403 — serving SQLite cache + YTM Fallback"
        } else {
            match spotify.account_tier {
                SpotifyAccountTier::Premium => "Premium active",
                SpotifyAccountTier::Free => "Free tier — YTM Auto-Fallback active",
                SpotifyAccountTier::Unknown => "Connected",
            }
        };
        if spotify.pending_mutations > 0 {
            format!(
                "Library Web API: {who} ({tier_label}) • {} queued likes",
                spotify.pending_mutations
            )
        } else {
            format!("Library Web API: {who} ({tier_label})")
        }
    } else if spotify.pending_mutations > 0 {
        format!(
            "Library Web API: Not connected • {} queued likes waiting for sign-in",
            spotify.pending_mutations
        )
    } else {
        "Library Web API: Not connected — enter your personal Spotify Developer Client ID below."
            .to_string()
    };

    let playback_status_text = if spotify.playback_connected {
        "Native Playback (librespot): Connected (up to 320 kbps Ogg Vorbis; auto-falls back to YTM if Premium lapses)."
    } else {
        "Native Playback (librespot): Not connected — Spotify tracks will play seamlessly via Smart YouTube Music Fallback."
    };

    let mut web_actions = row![].spacing(8).align_y(Alignment::Center);
    if spotify.web_api_connected {
        web_actions = web_actions
            .push(
                button(
                    row![
                        icon_refresh(12.0, accent.on_primary),
                        text(if spotify.syncing { "Syncing..." } else { "Sync Library" })
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(accent.on_primary),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(primary_pill_button_style(accent))
                .on_press(Message::SyncSpotifyLibrary),
            )
            .push(
                button(text("Disconnect Library").size(12).font(FONT_MEDIUM))
                    .padding([8, 14])
                    .style(glass_button_style(false, accent))
                    .on_press(Message::DisconnectSpotifyWebApi),
            );
    } else {
        web_actions = web_actions.push(
            button(
                row![
                    icon_link(12.0, accent.on_primary),
                    text(if spotify.logging_in_web {
                        "Waiting for Browser..."
                    } else {
                        "Connect Library (Web API)"
                    })
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .color(accent.on_primary),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 16])
            .style(primary_pill_button_style(accent))
            .on_press(Message::ConnectSpotifyWebApi),
        );
    }

    let playback_action = if spotify.playback_connected {
        button(text("Disconnect Playback").size(12).font(FONT_MEDIUM))
            .padding([8, 14])
            .style(glass_button_style(false, accent))
            .on_press(Message::DisconnectSpotifyPlayback)
    } else {
        button(
            text(if spotify.logging_in_playback {
                "Waiting for Browser..."
            } else {
                "Connect Native Playback (librespot)"
            })
            .size(12)
            .font(FONT_MEDIUM),
        )
        .padding([8, 14])
        .style(glass_button_style(false, accent))
        .on_press(Message::ConnectSpotifyPlayback)
    };

    let spotify_section = column![
        text("Spotify Premium & Smart Auto-Fallback")
            .size(16)
            .font(FONT_SEMIBOLD)
            .color(TEXT_PRIMARY),
        text(format!(
            "Create a Spotify Developer app with Redirect URI {SPOTIFY_WEB_API_REDIRECT_URI} (use 127.0.0.1, never localhost), paste your Client ID, and connect. Tokens are encrypted via Windows DPAPI."
        ))
        .size(12)
        .color(TEXT_SECONDARY),
        row![
            text_input(
                "Personal Spotify Developer Client ID...",
                spotify.client_id_input,
            )
            .on_input(Message::SpotifyClientIdChanged)
            .on_submit(Message::SaveSpotifyClientId)
            .style(glass_input_style(accent))
            .padding([9, 14])
            .width(Length::Fill),
            button(
                text("Save ID")
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .color(accent.on_primary)
            )
            .padding([9, 16])
            .style(primary_pill_button_style(accent))
            .on_press(Message::SaveSpotifyClientId),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
        text(web_status_text).size(12).color(TEXT_PRIMARY),
        text(playback_status_text).size(12).color(TEXT_SECONDARY),
        if let Some(summary) = spotify.sync_summary {
            Element::from(text(summary).size(12).color(accent.light))
        } else {
            Element::from(space().height(0))
        },
        row![web_actions, playback_action]
            .spacing(10)
            .align_y(Alignment::Center)
            .wrap(),
    ]
    .spacing(10);

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

    let spotify_card = container(spotify_section.width(Length::Fill))
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
                spotify_card,
                youtube_card,
                local_library_card,
            ]
            .spacing(16)
            .padding(20),
        )
        .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
        .style(thin_scrollable_style)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
