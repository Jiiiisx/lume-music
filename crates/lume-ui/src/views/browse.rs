use std::collections::HashMap;

use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{Playlist, Track, TrackSource};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{ActiveNav, Message, SourceFilter, ViewMode};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_elevated_style,
    glass_input_style, glass_panel, glass_sidebar_style, hero_banner_style, icon_chevron_down,
    icon_chevron_up, icon_folder, icon_heart, icon_link, icon_lyrics, icon_more, icon_music,
    icon_pause, icon_play, icon_playlist, icon_plus, icon_search, icon_settings, icon_trash,
    primary_pill_button_style, sidebar_nav_style, thin_scrollable_style, track_row_button_style,
    GLASS_SUBTLE_BORDER, ICON_HEART_ACTIVE, ICON_LIGHT, ICON_MUTED, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
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
    pub searching_remote: bool,
    pub playlists: &'a [Playlist],
    pub visible_tracks: &'a [Track],
    pub active_track_uid: Option<&'a str>,
    pub active_track: Option<&'a Track>,
    pub is_playing: bool,
    pub open_track_menu_uid: Option<&'a str>,
    pub renaming_playlist: Option<&'a (String, String)>,
    pub art_handles: &'a HashMap<String, image::Handle>,
    pub show_new_playlist_modal: bool,
    pub new_playlist_input: &'a str,
    pub show_import_url_modal: bool,
    pub import_url_input: &'a str,
    pub modal_error: Option<&'a str>,
    pub status_banner: Option<&'a str>,
    pub accent: AccentTheme,
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
    let sidebar = view_sidebar(props.nav, props.playlists, props.accent);
    let main_area = build_main_content(&props);

    let base = row![sidebar, main_area]
        .spacing(14)
        .padding([2, 16])
        .width(Length::Fill)
        .height(Length::Fill);

    if props.show_new_playlist_modal {
        let modal = container(
            column![
                text("New Custom Playlist")
                    .size(20)
                    .font(FONT_BOLD)
                    .color(TEXT_PRIMARY),
                text("Mix Spotify, YouTube Music, and Local tracks freely.")
                    .size(13)
                    .color(TEXT_SECONDARY),
                text_input("Playlist name...", props.new_playlist_input)
                    .on_input(Message::NewPlaylistNameChanged)
                    .on_submit(Message::CreatePlaylistSubmitted)
                    .style(glass_input_style(props.accent))
                    .padding([11, 16]),
                if let Some(err) = props.modal_error {
                    Element::from(text(err).size(12).color(Color::from_rgb8(252, 165, 165)))
                } else {
                    Element::from(space().height(0))
                },
                row![
                    space().width(Length::Fill),
                    button(text("Cancel").size(13).font(FONT_MEDIUM))
                        .padding([8, 16])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::CloseModal),
                    button(
                        text("Create")
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .color(props.accent.on_primary)
                    )
                    .padding([8, 20])
                    .style(primary_pill_button_style(props.accent))
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
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.44))),
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
                    .font(FONT_BOLD)
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
                .style(glass_input_style(props.accent))
                .padding([11, 16]),
                if let Some(err) = props.modal_error {
                    Element::from(text(err).size(12).color(Color::from_rgb8(253, 224, 71)))
                } else {
                    Element::from(space().height(0))
                },
                row![
                    space().width(Length::Fill),
                    button(text("Close").size(13).font(FONT_MEDIUM))
                        .padding([8, 16])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::CloseModal),
                    button(
                        text("Import")
                            .size(13)
                            .font(FONT_SEMIBOLD)
                            .color(props.accent.on_primary)
                    )
                    .padding([8, 20])
                    .style(primary_pill_button_style(props.accent))
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
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.44))),
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
    accent: AccentTheme,
) -> Element<'a, Message> {
    let nav_item = |label: &'a str,
                    target: ActiveNav,
                    active_icon: Element<'a, Message>,
                    inactive_icon: Element<'a, Message>|
     -> Element<'a, Message> {
        let active = nav == &target;
        let icon = if active { active_icon } else { inactive_icon };
        let text_col = if active { TEXT_PRIMARY } else { TEXT_SECONDARY };
        let font = if active { FONT_SEMIBOLD } else { FONT_MEDIUM };
        let indicator: Element<'a, Message> = if active {
            container(space().width(3).height(16))
                .style(move |_| container::Style {
                    background: Some(Background::Color(accent.light)),
                    border: iced::border::rounded(2.0),
                    ..Default::default()
                })
                .into()
        } else {
            space().width(3).height(1).into()
        };

        button(
            row![indicator, icon, text(label).size(13).font(font).color(text_col)]
                .spacing(9)
                .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([9, 10])
        .style(sidebar_nav_style(active, accent))
        .on_press(Message::SelectNav(target))
        .into()
    };

    let mut playlist_col = column![].spacing(4);
    if playlists.is_empty() {
        playlist_col = playlist_col.push(
            container(text("No playlists yet").size(12).color(TEXT_MUTED)).padding([8, 10]),
        );
    } else {
        for pl in playlists {
            let is_active = matches!(nav, ActiveNav::Playlist(id) if id == &pl.id);
            let icon_col = if is_active { accent.light } else { ICON_MUTED };
            let title_col = if is_active {
                TEXT_PRIMARY
            } else {
                TEXT_SECONDARY
            };
            let count_col = if is_active { accent.light } else { TEXT_MUTED };
            let title_font = if is_active { FONT_SEMIBOLD } else { FONT_MEDIUM };

            let row_btn = button(
                row![
                    icon_playlist(14.0, icon_col),
                    text(pl.name.as_str())
                        .size(12)
                        .font(title_font)
                        .color(title_col)
                        .width(Length::Fill),
                    container(text(format!("{}", pl.track_count)).size(10).color(count_col))
                        .padding([2, 7])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                1.0, 1.0, 1.0, 0.07,
                            ))),
                            border: iced::border::rounded(10.0),
                            ..Default::default()
                        }),
                ]
                .align_y(Alignment::Center)
                .spacing(8),
            )
            .width(Length::Fill)
            .padding([8, 10])
            .style(sidebar_nav_style(is_active, accent))
            .on_press(Message::SelectNav(ActiveNav::Playlist(pl.id.clone())));
            playlist_col = playlist_col.push(row_btn);
        }
    }

    container(
        column![
            container(
                text("LIBRARY")
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(TEXT_MUTED)
            )
            .padding([2, 8]),
            column![
                nav_item(
                    "Library",
                    ActiveNav::LocalFiles,
                    icon_music(15.0, accent.light),
                    icon_music(15.0, ICON_LIGHT),
                ),
                nav_item(
                    "Liked Songs",
                    ActiveNav::LikedSongs,
                    icon_heart(true, 15.0, ICON_HEART_ACTIVE),
                    icon_heart(false, 15.0, ICON_LIGHT),
                ),
                nav_item(
                    "Explore",
                    ActiveNav::Search,
                    icon_search(15.0, accent.light),
                    icon_search(15.0, ICON_LIGHT),
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
                text("PLAYLISTS")
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(TEXT_MUTED),
                space().width(Length::Fill),
                button(icon_plus(12.0, ICON_LIGHT))
                    .padding([4, 7])
                    .style(ghost_button_style)
                    .on_press(Message::OpenNewPlaylistModal),
                button(icon_link(12.0, ICON_LIGHT))
                    .padding([4, 7])
                    .style(ghost_button_style)
                    .on_press(Message::OpenImportUrlModal),
            ]
            .spacing(4)
            .padding([0, 6])
            .align_y(Alignment::Center),
            scrollable(playlist_col)
                .height(Length::Fill)
                .style(thin_scrollable_style),
            container(space().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                    ..Default::default()
                }),
            column![
                button(
                    row![
                        icon_folder(14.0, ICON_LIGHT),
                        text("Add Folder")
                            .size(12)
                            .font(FONT_MEDIUM)
                            .color(TEXT_PRIMARY),
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center),
                )
                .width(Length::Fill)
                .padding([8, 12])
                .style(glass_button_style(false, accent))
                .on_press(Message::PickLocalFolder),
                nav_item(
                    "Settings",
                    ActiveNav::Settings,
                    icon_settings(15.0, accent.light),
                    icon_settings(15.0, ICON_LIGHT),
                ),
            ]
            .spacing(6),
        ]
        .spacing(10)
        .padding(14)
        .width(Length::Fixed(224.0))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .style(glass_sidebar_style)
    .into()
}

fn build_hero_banner<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;
    let total_tracks = props.visible_tracks.len();
    let liked_count = props.visible_tracks.iter().filter(|t| t.is_liked).count();
    let total_mins: u64 = props
        .visible_tracks
        .iter()
        .map(|t| t.duration_ms / 1000)
        .sum::<u64>()
        / 60;

    if let ActiveNav::Playlist(id) = props.nav {
        let pl = props.playlists.iter().find(|p| &p.id == id);
        let pl_name = pl.map(|p| p.name.as_str()).unwrap_or("Playlist");
        let first_art = props
            .visible_tracks
            .first()
            .and_then(|t| t.artwork_ref.as_deref())
            .and_then(|k| props.art_handles.get(k));

        let title_el: Element<'a, Message> = if let Some((renaming_id, draft)) =
            props.renaming_playlist
        {
            if renaming_id == id {
                row![
                    text_input("Playlist name...", draft)
                        .on_input(Message::RenamePlaylistInputChanged)
                        .on_submit(Message::SubmitRenamePlaylist)
                        .style(glass_input_style(props.accent))
                        .padding([7, 14])
                        .width(Length::Fixed(220.0)),
                    button(text("Save").size(12).color(props.accent.on_primary))
                        .padding([7, 14])
                        .style(primary_pill_button_style(props.accent))
                        .on_press(Message::SubmitRenamePlaylist),
                    button(text("Cancel").size(12))
                        .padding([7, 12])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::CancelRenamePlaylist),
                ]
                .spacing(8)
                .align_y(Alignment::Center)
                .into()
            } else {
                text(pl_name).size(24).font(FONT_BOLD).color(TEXT_PRIMARY).into()
            }
        } else {
            text(pl_name).size(24).font(FONT_BOLD).color(TEXT_PRIMARY).into()
        };

        let mut actions = row![].spacing(8).align_y(Alignment::Center);
        if !props.visible_tracks.is_empty() {
            actions = actions.push(
                button(
                    row![
                        icon_play(13.0, props.accent.on_primary),
                        text("Play")
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(props.accent.on_primary),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 16])
                .style(primary_pill_button_style(props.accent))
                .on_press(Message::PlayTrackList {
                    tracks: props.visible_tracks.to_vec(),
                    start_index: 0,
                }),
            );
        }

        if let Some(pl) = pl {
            if pl.origin.is_editable() {
                actions = actions
                    .push(
                        button(text("Rename").size(12).font(FONT_MEDIUM))
                            .padding([7, 13])
                            .style(glass_button_style(false, props.accent))
                            .on_press(Message::StartRenamePlaylist(
                                pl.id.clone(),
                                pl.name.clone(),
                            )),
                    )
                    .push(
                        button(
                            row![
                                icon_trash(12.0, ICON_LIGHT),
                                text("Delete").size(12).font(FONT_MEDIUM)
                            ]
                            .spacing(5)
                            .align_y(Alignment::Center),
                        )
                        .padding([7, 13])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::DeletePlaylist(pl.id.clone())),
                    );
            } else {
                actions = actions.push(
                    container(text("Synced").size(11).color(TEXT_SECONDARY))
                        .padding([5, 12])
                        .style(|_| container::Style {
                            background: Some(Background::Color(Color::from_rgba(
                                1.0, 1.0, 1.0, 0.08,
                            ))),
                            border: iced::border::rounded(14.0),
                            ..Default::default()
                        }),
                );
            }
        }

        let art_tile = if first_art.is_some() {
            cover_art_widget(first_art, 76.0, 14.0)
        } else {
            container(icon_playlist(28.0, accent.light))
                .width(Length::Fixed(76.0))
                .height(Length::Fixed(76.0))
                .center_x(Length::Fixed(76.0))
                .center_y(Length::Fixed(76.0))
                .style(move |_| container::Style {
                    background: Some(Background::Color(accent.soft_bg)),
                    border: iced::border::Border {
                        color: accent.border,
                        width: 1.0,
                        radius: 14.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        };

        return container(
            row![
                art_tile,
                column![
                    text("PLAYLIST")
                        .size(10)
                        .font(FONT_SEMIBOLD)
                        .color(props.accent.light),
                    title_el,
                    text(format!("{total_tracks} songs • {total_mins} min"))
                        .size(12)
                        .color(TEXT_SECONDARY),
                ]
                .spacing(4)
                .width(Length::Fill),
                actions,
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding([16, 20])
        .width(Length::Fill)
        .style(hero_banner_style(props.accent))
        .into();
    }

    let hero_track = props.active_track.or_else(|| props.visible_tracks.first());
    let hero_art = hero_track
        .and_then(|t| t.artwork_ref.as_deref())
        .and_then(|k| props.art_handles.get(k));

    let (overline, title, subtitle) = if let Some(track) = props.active_track {
        let album_part = track
            .album
            .as_deref()
            .filter(|a| !a.is_empty())
            .map(|a| format!(" • {a}"))
            .unwrap_or_default();
        (
            "NOW PLAYING",
            track.title.clone(),
            format!("{}{album_part}", track.artist_display()),
        )
    } else {
        let heading = match props.nav {
            ActiveNav::Search => "Explore",
            ActiveNav::LikedSongs => "Liked Songs",
            ActiveNav::LocalFiles => "Library",
            ActiveNav::Settings => "Settings",
            ActiveNav::Playlist(_) => "Playlist",
        };
        let sub = if total_tracks == 0 {
            match props.nav {
                ActiveNav::LikedSongs => {
                    "Heart any song to collect your favorites here".to_string()
                }
                ActiveNav::Search => "Search across your local library and sources".to_string(),
                _ => "Add a local music folder to start listening".to_string(),
            }
        } else {
            format!("{total_tracks} tracks • {liked_count} favorites • {total_mins} min")
        };
        ("LIBRARY", heading.to_string(), sub)
    };

    let mut actions = row![].spacing(8).align_y(Alignment::Center);
    if props.active_track.is_some() {
        let play_label = if props.is_playing { "Pause" } else { "Play" };
        let play_icon = if props.is_playing {
            icon_pause(13.0, props.accent.on_primary)
        } else {
            icon_play(13.0, props.accent.on_primary)
        };
        actions = actions
            .push(
                button(
                    row![
                        play_icon,
                        text(play_label)
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(props.accent.on_primary)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 16])
                .style(primary_pill_button_style(props.accent))
                .on_press(Message::TogglePlayPause),
            )
            .push(
                button(
                    row![
                        icon_lyrics(13.0, ICON_LIGHT),
                        text("Lyrics").size(12).font(FONT_MEDIUM)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(glass_button_style(false, props.accent))
                .on_press(Message::SetViewMode(ViewMode::NowPlayingLyrics)),
            );
    } else if !props.visible_tracks.is_empty() {
        actions = actions
            .push(
                button(
                    row![
                        icon_play(13.0, props.accent.on_primary),
                        text("Play All")
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(props.accent.on_primary),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 16])
                .style(primary_pill_button_style(props.accent))
                .on_press(Message::PlayTrackList {
                    tracks: props.visible_tracks.to_vec(),
                    start_index: 0,
                }),
            )
            .push(
                button(
                    row![
                        icon_folder(13.0, ICON_LIGHT),
                        text("Add Folder").size(12).font(FONT_MEDIUM)
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 14])
                .style(glass_button_style(false, props.accent))
                .on_press(Message::PickLocalFolder),
            );
    } else {
        actions = actions.push(
            button(
                row![
                    icon_plus(13.0, props.accent.on_primary),
                    text("Add Music Folder")
                        .size(12)
                        .font(FONT_SEMIBOLD)
                        .color(props.accent.on_primary),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 16])
            .style(primary_pill_button_style(props.accent))
            .on_press(Message::PickLocalFolder),
        );
    }

    container(
        row![
            cover_art_widget(hero_art, 76.0, 14.0),
            column![
                text(overline)
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(props.accent.light),
                text(title).size(23).font(FONT_BOLD).color(TEXT_PRIMARY),
                text(subtitle).size(12).color(TEXT_SECONDARY),
            ]
            .spacing(4)
            .width(Length::Fill),
            actions,
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding([16, 20])
    .width(Length::Fill)
    .style(hero_banner_style(props.accent))
    .into()
}

fn build_main_content<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let active_filter_label = match props.source_filter {
        SourceFilter::All => "All Sources",
        SourceFilter::Spotify => "Spotify",
        SourceFilter::YouTubeMusic => "YouTube",
        SourceFilter::Local => "Local",
    };

    let hero = build_hero_banner(props);

    let grouped = group_tracks_by_source(props.visible_tracks, props.source_filter);
    let displayed_count: usize = grouped.iter().map(|(_, items)| items.len()).sum();

    let mut content_col = column![hero].spacing(12);
    if props.searching_remote && matches!(props.nav, ActiveNav::Search) {
        content_col = content_col.push(
            container(
                text(format!("Searching YouTube Music for \"{}\"...", props.search_query.trim()))
                    .size(12)
                    .font(FONT_MEDIUM)
                    .color(props.accent.light),
            )
            .padding([8, 14])
            .style(glass_elevated_style),
        );
    }
    if let Some(toast) = props.status_banner {
        content_col = content_col.push(
            container(
                row![
                    text(toast).size(12).color(TEXT_PRIMARY).width(Length::Fill),
                    button(text("Dismiss").size(11))
                        .padding([4, 10])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::DismissToast),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .style(glass_elevated_style),
        );
    }

    if displayed_count > 0 {
        let table_header = column![
            row![
                text("#")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(30.0)),
                text("TITLE")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::FillPortion(4)),
                text("ALBUM")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::FillPortion(3)),
                text("SOURCE")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(72.0)),
                text("TIME")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(48.0)),
                space().width(Length::Fixed(64.0)),
            ]
            .spacing(12)
            .padding([4, 12])
            .align_y(Alignment::Center),
            container(space().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                    ..Default::default()
                }),
        ]
        .spacing(4);
        content_col = content_col.push(table_header);
    }

    let mut track_list = column![].spacing(4);
    if props.visible_tracks.is_empty() {
        track_list = track_list.push(empty_state_card(props.nav, props.accent));
    } else if displayed_count == 0 {
        track_list = track_list.push(
            container(
                text(format!("No tracks match the {active_filter_label} filter."))
                    .size(13)
                    .color(TEXT_SECONDARY),
            )
            .padding(28)
            .width(Length::Fill)
            .center_x(Length::Fill),
        );
    } else {
        for (section_title, items) in grouped {
            if props.source_filter == SourceFilter::All {
                track_list = track_list.push(
                    container(text(section_title).size(11).color(TEXT_MUTED)).padding([6, 10]),
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

    content_col = content_col.push(
        scrollable(track_list)
            .height(Length::Fill)
            .style(thin_scrollable_style),
    );

    glass_panel(
        content_col
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn empty_state_card<'a>(nav: &'a ActiveNav, accent: AccentTheme) -> Element<'a, Message> {
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
            "Search Local & YouTube Music",
            "Type to filter Local tracks, or press Enter to search YouTube Music anonymously.",
            None,
        ),
        ActiveNav::Playlist(_) => (
            "This playlist is empty",
            "Open any track's ••• menu to add Spotify, YouTube, or Local tracks here.",
            None,
        ),
        ActiveNav::Settings => ("", "", None),
    };

    let mut inner = column![
        container(icon_music(26.0, accent.light))
            .width(Length::Fixed(56.0))
            .height(Length::Fixed(56.0))
            .center_x(Length::Fixed(56.0))
            .center_y(Length::Fixed(56.0))
            .style(move |_| container::Style {
                background: Some(Background::Color(accent.soft_bg)),
                border: iced::border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: 28.0.into(),
                },
                ..Default::default()
            }),
        text(headline).size(18).font(FONT_SEMIBOLD).color(TEXT_PRIMARY),
        text(subtitle).size(13).color(TEXT_SECONDARY),
    ]
    .spacing(10)
    .align_x(Alignment::Center);

    if let Some((btn_label, msg)) = cta {
        inner = inner.push(
            button(
                row![
                    icon_plus(14.0, accent.on_primary),
                    text(btn_label)
                        .size(13)
                        .font(FONT_SEMIBOLD)
                        .color(accent.on_primary)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .padding([10, 20])
            .style(primary_pill_button_style(accent))
            .on_press(msg),
        );
    }

    container(inner)
        .padding(44)
        .width(Length::Fill)
        .center_x(Length::Fill)
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

    let index_cell: Element<'a, Message> = if is_active {
        container(icon_play(12.0, props.accent.primary))
            .width(Length::Fixed(30.0))
            .center_x(Length::Fixed(30.0))
            .into()
    } else {
        container(text(format!("{:02}", index + 1)).size(11).color(TEXT_MUTED))
            .width(Length::Fixed(30.0))
            .center_x(Length::Fixed(30.0))
            .into()
    };

    let play_row_btn = button(
        row![
            index_cell,
            cover_art_widget(art, 40.0, 8.0),
            column![
                text(track.title.as_str())
                    .size(13)
                    .font(if is_active { FONT_SEMIBOLD } else { FONT_MEDIUM })
                    .color(if is_active {
                        props.accent.light
                    } else {
                        TEXT_PRIMARY
                    }),
                text(track.artist_display()).size(11).color(TEXT_SECONDARY),
            ]
            .spacing(2)
            .width(Length::FillPortion(4)),
            text(track.album.as_deref().unwrap_or("—"))
                .size(12)
                .color(TEXT_MUTED)
                .width(Length::FillPortion(3)),
            container(
                text(source_label(&track.source))
                    .size(10)
                    .font(FONT_MEDIUM)
                    .color(TEXT_SECONDARY),
            )
            .width(Length::Fixed(72.0))
            .padding([3, 8])
            .style(|_| container::Style {
                background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.065))),
                border: iced::border::rounded(10.0),
                ..Default::default()
            }),
            text(format_duration_ms(track.duration_ms))
                .size(12)
                .color(if is_active {
                    props.accent.light
                } else {
                    TEXT_SECONDARY
                })
                .width(Length::Fixed(48.0)),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding([7, 10])
    .style(track_row_button_style(is_active, props.accent))
    .on_press(play_msg);

    let main_row = mouse_area(
        row![
            play_row_btn,
            button(heart_icon)
                .padding([6, 7])
                .style(ghost_button_style)
                .on_press(Message::ToggleLike(track.clone())),
            button(icon_more(15.0, ICON_MUTED))
                .padding([6, 7])
                .style(ghost_button_style)
                .on_press(Message::ToggleTrackMenu(track.uid.clone())),
        ]
        .spacing(4)
        .padding([0, 2])
        .align_y(Alignment::Center),
    )
    .on_right_press(Message::ToggleTrackMenu(track.uid.clone()));

    if !menu_open {
        return main_row.into();
    }

    let mut actions = row![
        button(text("Play Next").size(12))
            .padding([6, 12])
            .style(glass_button_style(false, props.accent))
            .on_press(Message::PlayNext(track.clone())),
        button(text("Add to Queue").size(12))
            .padding([6, 12])
            .style(glass_button_style(false, props.accent))
            .on_press(Message::AddToQueue(track.clone())),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    for pl in props.playlists.iter().filter(|p| p.origin.is_editable()) {
        actions = actions.push(
            button(text(format!("+ {}", pl.name)).size(12))
                .padding([6, 12])
                .style(glass_button_style(false, props.accent))
                .on_press(Message::AddTrackToPlaylist {
                    playlist_id: pl.id.clone(),
                    track: track.clone(),
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
                        .style(glass_button_style(false, props.accent))
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
                        .style(glass_button_style(false, props.accent))
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
                .style(glass_button_style(false, props.accent))
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
