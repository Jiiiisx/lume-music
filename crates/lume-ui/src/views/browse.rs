use std::collections::HashMap;

use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{Playlist, PlaylistOrigin, Track, TrackSource};

use crate::views::{ActiveNav, Message, SourceFilter};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_elevated_style,
    glass_input_style, glass_panel, icon_chevron_down, icon_chevron_up, icon_folder, icon_heart,
    icon_link, icon_more, icon_music, icon_playlist, icon_plus, icon_search, icon_settings,
    icon_trash, primary_pill_button_style, sidebar_nav_style, thin_scrollable_style,
    GLASS_SUBTLE_BORDER, ICON_HEART_ACTIVE, ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_ON_PILL,
    TEXT_PRIMARY, TEXT_SECONDARY,
};

pub fn group_tracks_by_source<'a>(
    tracks: &'a [Track],
    filter: SourceFilter,
) -> Vec<(&'static str, Vec<&'a Track>)> {
    let mut spotify = Vec::new();
    let mut youtube = Vec::new();
    let mut local = Vec::new();

    for track in tracks {
        match &track.source {
            TrackSource::Spotify { .. } => spotify.push(track),
            TrackSource::YouTubeMusic { .. } => youtube.push(track),
            TrackSource::Local { .. } => local.push(track),
        }
    }

    let mut groups = Vec::new();
    match filter {
        SourceFilter::All => {
            if !spotify.is_empty() {
                groups.push(("Spotify", spotify));
            }
            if !youtube.is_empty() {
                groups.push(("YouTube Music", youtube));
            }
            if !local.is_empty() {
                groups.push(("Local", local));
            }
        }
        SourceFilter::Spotify => {
            if !spotify.is_empty() {
                groups.push(("Spotify", spotify));
            }
        }
        SourceFilter::YouTubeMusic => {
            if !youtube.is_empty() {
                groups.push(("YouTube Music", youtube));
            }
        }
        SourceFilter::Local => {
            if !local.is_empty() {
                groups.push(("Local", local));
            }
        }
    }
    groups
}

pub fn validate_playlist_name(raw: &str) -> Result<String, &'static str> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Err("Playlist name cannot be empty.")
    } else {
        Ok(trimmed.to_string())
    }
}

pub struct BrowseViewProps<'a> {
    pub nav: &'a ActiveNav,
    pub search_query: &'a str,
    pub source_filter: SourceFilter,
    pub playlists: &'a [Playlist],
    pub visible_tracks: &'a [Track],
    pub active_track_uid: Option<&'a str>,
    pub open_track_menu_uid: Option<&'a str>,
    pub renaming_playlist: Option<&'a (String, String)>,
    pub art_handles: &'a HashMap<String, image::Handle>,
    pub show_new_playlist_modal: bool,
    pub new_playlist_input: &'a str,
    pub show_import_url_modal: bool,
    pub import_url_input: &'a str,
    pub modal_error: Option<&'a str>,
    pub status_banner: Option<&'a str>,
}

fn source_label(source: &TrackSource) -> &'static str {
    match source {
        TrackSource::Spotify { .. } => "Spotify",
        TrackSource::YouTubeMusic { .. } => "YouTube",
        TrackSource::Local { .. } => "Local",
    }
}

fn format_duration_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

pub fn view_browse<'a>(props: BrowseViewProps<'a>) -> Element<'a, Message> {
    let sidebar = view_sidebar(props.nav, props.playlists);
    let main_area = build_main_content(&props);

    let base = row![sidebar, main_area]
        .spacing(16)
        .padding([2, 16])
        .width(Length::Fill)
        .height(Length::Fill);

    if props.show_new_playlist_modal {
        let modal = container(
            column![
                text("New Custom Playlist").size(20).color(TEXT_PRIMARY),
                text("Mix Spotify, YouTube Music, and Local tracks freely.")
                    .size(13)
                    .color(TEXT_SECONDARY),
                text_input("Playlist name...", props.new_playlist_input)
                    .on_input(Message::NewPlaylistNameChanged)
                    .on_submit(Message::CreatePlaylistSubmitted)
                    .style(glass_input_style)
                    .padding([11, 16]),
                if let Some(err) = props.modal_error {
                    Element::from(text(err).size(12).color(Color::from_rgb8(252, 165, 165)))
                } else {
                    Element::from(space().height(0))
                },
                row![
                    space().width(Length::Fill),
                    button(text("Cancel").size(13))
                        .padding([8, 16])
                        .style(glass_button_style(false))
                        .on_press(Message::CloseModal),
                    button(text("Create").size(13))
                        .padding([8, 20])
                        .style(primary_pill_button_style)
                        .on_press(Message::CreatePlaylistSubmitted),
                ]
                .spacing(10),
            ]
            .spacing(14)
            .width(Length::Fixed(370.0)),
        )
        .padding(24)
        .style(glass_elevated_style);

        return iced::widget::stack![
            base,
            container(modal)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
                    ..Default::default()
                })
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    if props.show_import_url_modal {
        let modal = container(
            column![
                text("Import Public Spotify Playlist")
                    .size(20)
                    .color(TEXT_PRIMARY),
                text("Paste a public open.spotify.com/playlist/... link (Phase 3 importer hook).")
                    .size(13)
                    .color(TEXT_SECONDARY),
                text_input(
                    "https://open.spotify.com/playlist/...",
                    props.import_url_input
                )
                .on_input(Message::ImportUrlInputChanged)
                .on_submit(Message::ImportSpotifyUrlSubmitted)
                .style(glass_input_style)
                .padding([11, 16]),
                if let Some(err) = props.modal_error {
                    Element::from(text(err).size(12).color(Color::from_rgb8(253, 224, 71)))
                } else {
                    Element::from(space().height(0))
                },
                row![
                    space().width(Length::Fill),
                    button(text("Close").size(13))
                        .padding([8, 16])
                        .style(glass_button_style(false))
                        .on_press(Message::CloseModal),
                    button(text("Import").size(13))
                        .padding([8, 20])
                        .style(primary_pill_button_style)
                        .on_press(Message::ImportSpotifyUrlSubmitted),
                ]
                .spacing(10),
            ]
            .spacing(14)
            .width(Length::Fixed(420.0)),
        )
        .padding(24)
        .style(glass_elevated_style);

        return iced::widget::stack![
            base,
            container(modal)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.45))),
                    ..Default::default()
                })
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    base.into()
}

pub fn view_sidebar<'a>(
    nav: &'a ActiveNav,
    playlists: &'a [Playlist],
) -> Element<'a, Message> {
    let nav_item = |label: &'a str, target: ActiveNav, active_icon: Element<'a, Message>, inactive_icon: Element<'a, Message>| -> Element<'a, Message> {
        let active = nav == &target;
        let icon = if active { active_icon } else { inactive_icon };
        let text_col = if active { TEXT_ON_PILL } else { TEXT_SECONDARY };
        button(
            row![icon, text(label).size(14).color(text_col)]
                .spacing(12)
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([10, 14])
        .style(sidebar_nav_style(active))
        .on_press(Message::SelectNav(target))
        .into()
    };

    let mut playlist_col = column![].spacing(4);
    if playlists.is_empty() {
        playlist_col = playlist_col.push(
            container(
                text("No playlists yet")
                    .size(12)
                    .color(TEXT_MUTED),
            )
            .padding([8, 12]),
        );
    } else {
        for pl in playlists {
            let is_active = matches!(nav, ActiveNav::Playlist(id) if id == &pl.id);
            let origin_tag = match &pl.origin {
                PlaylistOrigin::SpotifySynced => "Spotify",
                PlaylistOrigin::SpotifyImported { .. } => "Imported",
                PlaylistOrigin::LocalCustom => "Local",
            };
            let icon_col = if is_active { TEXT_ON_PILL } else { ICON_MUTED };
            let title_col = if is_active { TEXT_ON_PILL } else { TEXT_PRIMARY };
            let meta_col = if is_active {
                Color::from_rgba8(24, 20, 28, 0.65)
            } else {
                TEXT_MUTED
            };

            let row_btn = button(
                row![
                    icon_playlist(15.0, icon_col),
                    text(pl.name.as_str())
                        .size(13)
                        .color(title_col)
                        .width(Length::Fill),
                    text(format!("{} • {}", pl.track_count, origin_tag))
                        .size(10)
                        .color(meta_col),
                ]
                .align_y(Alignment::Center)
                .spacing(9),
            )
            .width(Length::Fill)
            .padding([8, 12])
            .style(sidebar_nav_style(is_active))
            .on_press(Message::SelectNav(ActiveNav::Playlist(pl.id.clone())));
            playlist_col = playlist_col.push(row_btn);
        }
    }

    glass_panel(
        column![
            container(
                column![
                    text("Lume").size(22).color(TEXT_PRIMARY),
                    text("Glass Audio Workspace").size(11).color(TEXT_MUTED),
                ]
                .spacing(2),
            )
            .padding(iced::Padding {
                top: 4.0,
                right: 8.0,
                bottom: 8.0,
                left: 8.0,
            }),
            column![
                nav_item(
                    "Search",
                    ActiveNav::Search,
                    icon_search(16.0, TEXT_ON_PILL),
                    icon_search(16.0, ICON_LIGHT),
                ),
                nav_item(
                    "Liked Songs",
                    ActiveNav::LikedSongs,
                    icon_heart(true, 16.0, ICON_HEART_ACTIVE),
                    icon_heart(false, 16.0, ICON_LIGHT),
                ),
                nav_item(
                    "Local Files",
                    ActiveNav::LocalFiles,
                    icon_folder(16.0, TEXT_ON_PILL),
                    icon_folder(16.0, ICON_LIGHT),
                ),
                nav_item(
                    "Settings",
                    ActiveNav::Settings,
                    icon_settings(16.0, TEXT_ON_PILL),
                    icon_settings(16.0, ICON_LIGHT),
                ),
            ]
            .spacing(4),
            container(space().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                    ..Default::default()
                }),
            row![
                text("PLAYLISTS").size(11).color(TEXT_MUTED),
                space().width(Length::Fill),
                button(icon_plus(13.0, ICON_LIGHT))
                    .padding([6, 8])
                    .style(glass_button_style(false))
                    .on_press(Message::OpenNewPlaylistModal),
                button(icon_link(13.0, ICON_LIGHT))
                    .padding([6, 8])
                    .style(glass_button_style(false))
                    .on_press(Message::OpenImportUrlModal),
            ]
            .spacing(6)
            .padding([0, 4])
            .align_y(Alignment::Center),
            scrollable(playlist_col)
                .height(Length::Fill)
                .style(thin_scrollable_style),
            container(
                row![
                    container(space().width(6).height(6)).style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgb8(110, 231, 183))),
                        border: iced::border::rounded(3.0),
                        ..Default::default()
                    }),
                    text("Local Engine Ready").size(11).color(TEXT_SECONDARY),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
                border: iced::border::Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.12),
                    width: 1.0,
                    radius: 14.0.into(),
                },
                ..Default::default()
            }),
        ]
        .spacing(12)
        .padding(16)
        .width(Length::Fixed(248.0))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

fn build_main_content<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let search_bar = container(
        row![
            icon_search(16.0, ICON_MUTED),
            text_input(
                "Search local library (press Enter for remote sources)...",
                props.search_query
            )
            .on_input(Message::SearchInputChanged)
            .on_submit(Message::SearchSubmitted)
            .style(|_, _| text_input::Style {
                background: Background::Color(Color::TRANSPARENT),
                border: iced::border::Border::default(),
                icon: TEXT_SECONDARY,
                placeholder: TEXT_MUTED,
                value: TEXT_PRIMARY,
                selection: Color::from_rgba(1.0, 1.0, 1.0, 0.25),
            })
            .padding([8, 6])
            .width(Length::Fill),
        ]
        .spacing(8)
        .padding([2, 14])
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .style(|_| container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.11))),
        border: iced::border::Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.18),
            width: 1.0,
            radius: 22.0.into(),
        },
        ..Default::default()
    });

    let filter_chips = row![
        chip_btn("All", SourceFilter::All, props.source_filter),
        chip_btn("Spotify", SourceFilter::Spotify, props.source_filter),
        chip_btn("YouTube Music", SourceFilter::YouTubeMusic, props.source_filter),
        chip_btn("Local", SourceFilter::Local, props.source_filter),
    ]
    .spacing(8);

    let header_title: Element<'a, Message> = match props.nav {
        ActiveNav::Search => text("Unified Search").size(24).color(TEXT_PRIMARY).into(),
        ActiveNav::LikedSongs => text("Liked Songs").size(24).color(TEXT_PRIMARY).into(),
        ActiveNav::LocalFiles => text("Local Files").size(24).color(TEXT_PRIMARY).into(),
        ActiveNav::Settings => text("Settings").size(24).color(TEXT_PRIMARY).into(),
        ActiveNav::Playlist(id) => {
            if let Some((renaming_id, draft)) = props.renaming_playlist {
                if renaming_id == id {
                    row![
                        text_input("Playlist name...", draft)
                            .on_input(Message::RenamePlaylistInputChanged)
                            .on_submit(Message::SubmitRenamePlaylist)
                            .style(glass_input_style)
                            .padding([7, 14])
                            .width(Length::Fixed(240.0)),
                        button(text("Save").size(12))
                            .padding([7, 14])
                            .style(primary_pill_button_style)
                            .on_press(Message::SubmitRenamePlaylist),
                        button(text("Cancel").size(12))
                            .padding([7, 12])
                            .style(glass_button_style(false))
                            .on_press(Message::CancelRenamePlaylist),
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center)
                    .into()
                } else {
                    playlist_header(props, id)
                }
            } else {
                playlist_header(props, id)
            }
        }
    };

    let grouped = group_tracks_by_source(props.visible_tracks, props.source_filter);
    let mut track_list = column![].spacing(8);

    if props.visible_tracks.is_empty() {
        track_list = track_list.push(empty_state_card(props.nav));
    } else {
        for (section_title, items) in grouped {
            if props.source_filter == SourceFilter::All {
                track_list = track_list.push(
                    container(text(section_title).size(12).color(TEXT_MUTED)).padding([6, 4]),
                );
            }
            for track in items {
                let global_idx = props
                    .visible_tracks
                    .iter()
                    .position(|t| t.uid == track.uid)
                    .unwrap_or(0);
                track_list = track_list.push(track_row(props, track, global_idx));
            }
        }
    }

    let mut content_col = column![search_bar, filter_chips, header_title].spacing(14);
    if let Some(toast) = props.status_banner {
        content_col = content_col.push(
            container(
                row![
                    text(toast).size(12).color(TEXT_PRIMARY).width(Length::Fill),
                    button(text("Dismiss").size(11))
                        .padding([4, 10])
                        .style(glass_button_style(false))
                        .on_press(Message::DismissToast),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .style(glass_elevated_style),
        );
    }

    content_col = content_col.push(
        scrollable(track_list)
            .height(Length::Fill)
            .style(thin_scrollable_style),
    );

    glass_panel(
        content_col
            .padding(20)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn empty_state_card<'a>(nav: &'a ActiveNav) -> Element<'a, Message> {
    let (headline, subtitle, cta): (&str, &str, Option<(&str, Message)>) = match nav {
        ActiveNav::LocalFiles => (
            "Your local music library is empty",
            "Select a folder with MP3, FLAC, WAV, OGG, AAC, or Opus files to begin.",
            Some(("Add Music Folder", Message::PickLocalFolder)),
        ),
        ActiveNav::LikedSongs => (
            "No liked songs yet",
            "Tap the heart icon on any track to gather your favorites across sources.",
            None,
        ),
        ActiveNav::Search => (
            "Search across your workspace",
            "Type to filter local tracks live, or add a music folder first.",
            Some(("Add Music Folder", Message::PickLocalFolder)),
        ),
        ActiveNav::Playlist(_) => (
            "This playlist is empty",
            "Open any track's ••• menu to add Spotify, YouTube, or Local tracks here.",
            None,
        ),
        ActiveNav::Settings => ("", "", None),
    };

    let mut inner = column![
        container(icon_music(26.0, ICON_LIGHT))
            .width(Length::Fixed(56.0))
            .height(Length::Fixed(56.0))
            .center_x(Length::Fixed(56.0))
            .center_y(Length::Fixed(56.0))
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.14))),
                border: iced::border::Border {
                    color: Color::from_rgba(1.0, 1.0, 1.0, 0.24),
                    width: 1.0,
                    radius: 28.0.into(),
                },
                ..Default::default()
            }),
        text(headline).size(17).color(TEXT_PRIMARY),
        text(subtitle).size(13).color(TEXT_SECONDARY),
    ]
    .spacing(10)
    .align_x(Alignment::Center);

    if let Some((btn_label, msg)) = cta {
        inner = inner.push(
            button(
                row![icon_plus(14.0, TEXT_ON_PILL), text(btn_label).size(13)]
                    .spacing(8)
                    .align_y(Alignment::Center),
            )
            .padding([10, 20])
            .style(primary_pill_button_style)
            .on_press(msg),
        );
    }

    container(inner)
        .padding(32)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
            border: iced::border::Border {
                color: Color::from_rgba(1.0, 1.0, 1.0, 0.14),
                width: 1.0,
                radius: 20.0.into(),
            },
            ..Default::default()
        })
        .into()
}

fn playlist_header<'a>(props: &BrowseViewProps<'a>, id: &'a str) -> Element<'a, Message> {
    if let Some(pl) = props.playlists.iter().find(|p| p.id == id) {
        let mut hdr = row![text(pl.name.as_str()).size(24).color(TEXT_PRIMARY)]
            .spacing(10)
            .align_y(Alignment::Center);
        if pl.origin.is_editable() {
            hdr = hdr
                .push(
                    button(text("Rename").size(12))
                        .padding([6, 12])
                        .style(glass_button_style(false))
                        .on_press(Message::StartRenamePlaylist(pl.id.clone(), pl.name.clone())),
                )
                .push(
                    button(
                        row![icon_trash(12.0, ICON_LIGHT), text("Delete").size(12)]
                            .spacing(5)
                            .align_y(Alignment::Center),
                    )
                    .padding([6, 12])
                    .style(glass_button_style(false))
                    .on_press(Message::DeletePlaylist(pl.id.clone())),
                );
        } else {
            hdr = hdr.push(
                container(text("Spotify Synced • Read-only").size(11).color(TEXT_SECONDARY))
                    .padding([4, 12])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.10))),
                        border: iced::border::rounded(14.0),
                        ..Default::default()
                    }),
            );
        }
        hdr.into()
    } else {
        text("Playlist").size(24).color(TEXT_PRIMARY).into()
    }
}

fn chip_btn<'a>(
    label: &'a str,
    target: SourceFilter,
    current: SourceFilter,
) -> Element<'a, Message> {
    let active = target == current;
    let col = if active { TEXT_ON_PILL } else { TEXT_PRIMARY };
    button(text(label).size(12).color(col))
        .padding([7, 16])
        .style(glass_button_style(active))
        .on_press(Message::SourceFilterChanged(target))
        .into()
}

fn track_row<'a>(
    props: &BrowseViewProps<'a>,
    track: &'a Track,
    index: usize,
) -> Element<'a, Message> {
    let is_active = props.active_track_uid == Some(track.uid.as_str());
    let menu_open = props.open_track_menu_uid == Some(track.uid.as_str());
    let art = track
        .artwork_ref
        .as_deref()
        .and_then(|k| props.art_handles.get(k));

    let play_msg = Message::PlayTrackList {
        tracks: props.visible_tracks.to_vec(),
        start_index: index,
    };

    let heart_icon = if track.is_liked {
        icon_heart(true, 15.0, ICON_HEART_ACTIVE)
    } else {
        icon_heart(false, 15.0, ICON_MUTED)
    };

    let main_row = mouse_area(
        container(
            row![
                cover_art_widget(art, 44.0, 10.0),
                column![
                    text(track.title.as_str())
                        .size(14)
                        .color(if is_active {
                            Color::from_rgb8(255, 224, 184)
                        } else {
                            TEXT_PRIMARY
                        }),
                    text(track.artist_display()).size(12).color(TEXT_SECONDARY),
                ]
                .spacing(2)
                .width(Length::FillPortion(3)),
                text(track.album.as_deref().unwrap_or(""))
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(Length::FillPortion(2)),
                container(text(source_label(&track.source)).size(11).color(TEXT_SECONDARY))
                    .padding([3, 10])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.10))),
                        border: iced::border::rounded(12.0),
                        ..Default::default()
                    }),
                text(format_duration_ms(track.duration_ms))
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(44.0)),
                button(heart_icon)
                    .padding([6, 8])
                    .style(ghost_button_style)
                    .on_press(Message::ToggleLike(track.clone())),
                button(icon_more(16.0, ICON_MUTED))
                    .padding([6, 8])
                    .style(ghost_button_style)
                    .on_press(Message::ToggleTrackMenu(track.uid.clone())),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([10, 14])
        .style(move |_| container::Style {
            background: Some(Background::Color(if is_active {
                Color::from_rgba(1.0, 1.0, 1.0, 0.18)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.07)
            })),
            border: iced::border::Border {
                color: if is_active {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.30)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.12)
                },
                width: 1.0,
                radius: 16.0.into(),
            },
            ..Default::default()
        }),
    )
    .on_press(play_msg)
    .on_right_press(Message::ToggleTrackMenu(track.uid.clone()));

    if !menu_open {
        return main_row.into();
    }

    let mut actions = row![
        button(text("Play Next").size(12))
            .padding([6, 12])
            .style(glass_button_style(false))
            .on_press(Message::PlayNext(track.clone())),
        button(text("Add to Queue").size(12))
            .padding([6, 12])
            .style(glass_button_style(false))
            .on_press(Message::AddToQueue(track.clone())),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    for pl in props.playlists.iter().filter(|p| p.origin.is_editable()) {
        actions = actions.push(
            button(text(format!("+ {}", pl.name)).size(12))
                .padding([6, 12])
                .style(glass_button_style(false))
                .on_press(Message::AddTrackToPlaylist {
                    playlist_id: pl.id.clone(),
                    track_uid: track.uid.clone(),
                }),
        );
    }

    if let ActiveNav::Playlist(pl_id) = props.nav {
        if props
            .playlists
            .iter()
            .any(|p| &p.id == pl_id && p.origin.is_editable())
        {
            if index > 0 {
                actions = actions.push(
                    button(icon_chevron_up(14.0, ICON_LIGHT))
                        .padding([6, 8])
                        .style(glass_button_style(false))
                        .on_press(Message::ReorderPlaylistTrack {
                            playlist_id: pl_id.clone(),
                            from: index,
                            to: index - 1,
                        }),
                );
            }
            if index + 1 < props.visible_tracks.len() {
                actions = actions.push(
                    button(icon_chevron_down(14.0, ICON_LIGHT))
                        .padding([6, 8])
                        .style(glass_button_style(false))
                        .on_press(Message::ReorderPlaylistTrack {
                            playlist_id: pl_id.clone(),
                            from: index,
                            to: index + 1,
                        }),
                );
            }
            actions = actions.push(
                button(
                    row![icon_trash(12.0, ICON_LIGHT), text("Remove").size(12)]
                        .spacing(5)
                        .align_y(Alignment::Center),
                )
                .padding([6, 12])
                .style(glass_button_style(false))
                .on_press(Message::RemoveTrackFromPlaylist {
                    playlist_id: pl_id.clone(),
                    position: index,
                }),
            );
        }
    }

    column![
        main_row,
        container(actions)
            .padding([8, 14])
            .width(Length::Fill)
            .style(glass_elevated_style)
    ]
    .spacing(6)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn mk_track(source: TrackSource, title: &str) -> Track {
        Track {
            uid: source.canonical_uid(),
            source,
            title: title.into(),
            artists: vec!["Artist".into()],
            album: None,
            duration_ms: 180_000,
            artwork_ref: None,
            is_liked: false,
        }
    }

    #[test]
    fn groups_all_filter_results_by_source_without_dedup() {
        let tracks = vec![
            mk_track(TrackSource::Spotify { id: "s1".into() }, "Same Name"),
            mk_track(
                TrackSource::YouTubeMusic {
                    video_id: "y1".into(),
                },
                "Same Name",
            ),
            mk_track(
                TrackSource::Local {
                    path: PathBuf::from(r"D:\Music\same.flac"),
                },
                "Same Name",
            ),
        ];

        let grouped = group_tracks_by_source(&tracks, SourceFilter::All);
        assert_eq!(grouped.len(), 3);
        assert_eq!(grouped[0].0, "Spotify");
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[1].0, "YouTube Music");
        assert_eq!(grouped[1].1.len(), 1);
        assert_eq!(grouped[2].0, "Local");
        assert_eq!(grouped[2].1.len(), 1);
    }

    #[test]
    fn filters_results_to_selected_single_source() {
        let tracks = vec![
            mk_track(TrackSource::Spotify { id: "s1".into() }, "SP"),
            mk_track(
                TrackSource::Local {
                    path: PathBuf::from(r"D:\Music\loc.flac"),
                },
                "LOC",
            ),
        ];

        let grouped = group_tracks_by_source(&tracks, SourceFilter::Local);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].0, "Local");
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[0].1[0].title, "LOC");
    }

    #[test]
    fn rejects_empty_or_whitespace_playlist_name() {
        assert!(validate_playlist_name("").is_err());
        assert!(validate_playlist_name("   \t\n  ").is_err());
        assert_eq!(
            validate_playlist_name("  Night Drive  "),
            Ok("Night Drive".into())
        );
    }
}
