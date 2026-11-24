use std::collections::HashMap;

use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{Playlist, Track, TrackSource};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::{
    ActiveNav, ChartRegion, Message, PlaylistSort, PlaylistSortField, SortDirection, SourceFilter,
    ViewMode,
};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_card_style,
    glass_elevated_style, glass_input_style, glass_panel, glass_sidebar_style, hero_banner_style,
    hidden_scrollbar, icon_chevron_down, icon_chevron_up, icon_clock, icon_folder, icon_heart,
    icon_home, icon_link, icon_lyrics, icon_more, icon_music, icon_pause, icon_play, icon_playlist,
    icon_plus, icon_refresh, icon_search, icon_settings, icon_trash, icon_trending,
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

pub fn matches_source_filter(source: &TrackSource, filter: SourceFilter) -> bool {
    match filter {
        SourceFilter::All => true,
        SourceFilter::Spotify => matches!(source, TrackSource::Spotify { .. }),
        SourceFilter::YouTubeMusic => matches!(source, TrackSource::YouTubeMusic { .. }),
        SourceFilter::Local => matches!(source, TrackSource::Local { .. }),
    }
}

/// Sorts playlist tracks according to `sort` while preserving each track's original DB `position`
/// so delete/reorder mutations always target the exact underlying playlist row.
pub fn sort_playlist_tracks<'a>(
    tracks: &'a [Track],
    sort: PlaylistSort,
) -> Vec<(usize, &'a Track)> {
    let mut indexed: Vec<(usize, &'a Track)> = tracks.iter().enumerate().collect();
    let desc = sort.direction == SortDirection::Desc;

    match sort.field {
        PlaylistSortField::Custom | PlaylistSortField::RecentlyAdded => {
            if desc {
                indexed.reverse();
            }
        }
        PlaylistSortField::Title => {
            indexed.sort_by(|(ia, a), (ib, b)| {
                let ord = a
                    .title
                    .to_ascii_lowercase()
                    .cmp(&b.title.to_ascii_lowercase())
                    .then_with(|| {
                        a.artist_display()
                            .to_ascii_lowercase()
                            .cmp(&b.artist_display().to_ascii_lowercase())
                    })
                    .then_with(|| ia.cmp(ib));
                if desc { ord.reverse() } else { ord }
            });
        }
        PlaylistSortField::Artist => {
            indexed.sort_by(|(ia, a), (ib, b)| {
                let ord = a
                    .artist_display()
                    .to_ascii_lowercase()
                    .cmp(&b.artist_display().to_ascii_lowercase())
                    .then_with(|| {
                        a.title
                            .to_ascii_lowercase()
                            .cmp(&b.title.to_ascii_lowercase())
                    })
                    .then_with(|| ia.cmp(ib));
                if desc { ord.reverse() } else { ord }
            });
        }
        PlaylistSortField::Album => {
            indexed.sort_by(|(ia, a), (ib, b)| {
                let alb_a = a.album.as_deref().map(str::trim).unwrap_or("");
                let alb_b = b.album.as_deref().map(str::trim).unwrap_or("");
                // Keep empty albums at the bottom regardless of Asc/Desc.
                match (alb_a.is_empty(), alb_b.is_empty()) {
                    (true, false) => std::cmp::Ordering::Greater,
                    (false, true) => std::cmp::Ordering::Less,
                    _ => {
                        let ord = alb_a
                            .to_ascii_lowercase()
                            .cmp(&alb_b.to_ascii_lowercase())
                            .then_with(|| {
                                a.title
                                    .to_ascii_lowercase()
                                    .cmp(&b.title.to_ascii_lowercase())
                            })
                            .then_with(|| ia.cmp(ib));
                        if desc { ord.reverse() } else { ord }
                    }
                }
            });
        }
        PlaylistSortField::Duration => {
            indexed.sort_by(|(ia, a), (ib, b)| {
                let ord = a
                    .duration_ms
                    .cmp(&b.duration_ms)
                    .then_with(|| {
                        a.title
                            .to_ascii_lowercase()
                            .cmp(&b.title.to_ascii_lowercase())
                    })
                    .then_with(|| ia.cmp(ib));
                if desc { ord.reverse() } else { ord }
            });
        }
    }

    indexed
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
    pub playlist_sort: PlaylistSort,
    pub searching_remote: bool,
    pub playlists: &'a [Playlist],
    pub visible_tracks: &'a [Track],
    pub recent_tracks: &'a [Track],
    pub home_region: ChartRegion,
    pub home_spotify_trending: &'a [Track],
    pub home_ytm_trending: &'a [Track],
    pub home_loading_spotify: bool,
    pub home_loading_ytm: bool,
    pub home_spotify_expanded: bool,
    pub home_ytm_expanded: bool,
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
    pub importing_url: bool,
    pub connection_status: &'a str,
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
    let sidebar = view_sidebar(
        props.nav,
        props.playlists,
        props.connection_status,
        props.accent,
    );
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
        let import_btn = button(
            text(if props.importing_url {
                "Importing..."
            } else {
                "Import"
            })
            .size(13)
            .font(FONT_SEMIBOLD)
            .color(props.accent.on_primary),
        )
        .padding([8, 20])
        .style(primary_pill_button_style(props.accent));
        let import_btn = if props.importing_url {
            import_btn
        } else {
            import_btn.on_press(Message::ImportSpotifyUrlSubmitted)
        };

        let csv_btn = button(
            row![
                icon_folder(13.0, ICON_LIGHT),
                text("Import Exportify .csv")
                    .size(12)
                    .font(FONT_MEDIUM),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .padding([8, 14])
        .style(glass_button_style(false, props.accent));
        let csv_btn = if props.importing_url {
            csv_btn
        } else {
            csv_btn.on_press(Message::PickExportifyCsv)
        };

        let modal = container(
            column![
                text("Import Spotify Playlist")
                    .size(20)
                    .font(FONT_BOLD)
                    .color(TEXT_PRIMARY),
                text("Paste a public open.spotify.com/playlist/... link (full tracklist if Spotify Web API is connected in Settings; Spotify caps anonymous public links at 100 tracks) — or import an Exportify .csv for unlimited tracks without login.")
                    .size(12)
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
                    csv_btn,
                    space().width(Length::Fill),
                    button(text("Close").size(13).font(FONT_MEDIUM))
                        .padding([8, 16])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::CloseModal),
                    import_btn,
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            ]
            .spacing(14)
            .width(Length::Fixed(460.0)),
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
    connection_status: &'a str,
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

            let origin_tag = match &pl.origin {
                lume_core::PlaylistOrigin::SpotifySynced => Some("SP"),
                lume_core::PlaylistOrigin::SpotifyImported { .. } => Some("IMP"),
                lume_core::PlaylistOrigin::LocalCustom => None,
            };

            let mut pl_row = row![
                icon_playlist(14.0, icon_col),
                text(pl.name.as_str())
                    .size(12)
                    .font(title_font)
                    .color(title_col)
                    .width(Length::Fill),
            ]
            .align_y(Alignment::Center)
            .spacing(6);

            if let Some(tag) = origin_tag {
                pl_row = pl_row.push(
                    container(text(tag).size(9).font(FONT_SEMIBOLD).color(accent.light))
                        .padding([1, 5])
                        .style(move |_| container::Style {
                            background: Some(Background::Color(accent.soft_bg)),
                            border: iced::border::rounded(6.0),
                            ..Default::default()
                        }),
                );
            }

            pl_row = pl_row.push(
                container(text(format!("{}", pl.track_count)).size(10).color(count_col))
                    .padding([2, 7])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(
                            1.0, 1.0, 1.0, 0.07,
                        ))),
                        border: iced::border::rounded(10.0),
                        ..Default::default()
                    }),
            );

            let row_btn = button(pl_row)
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
                    "Home",
                    ActiveNav::Home,
                    icon_home(15.0, accent.light),
                    icon_home(15.0, ICON_LIGHT),
                ),
                nav_item(
                    "Local Files",
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
                .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
                .height(Length::Fill)
                .style(thin_scrollable_style),
            container(space().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                    ..Default::default()
                }),
            column![
                container(
                    text(connection_status)
                        .size(11)
                        .font(FONT_MEDIUM)
                        .color(accent.light),
                )
                .width(Length::Fill)
                .padding([6, 10])
                .style(move |_| container::Style {
                    background: Some(Background::Color(accent.soft_bg)),
                    border: iced::border::Border {
                        color: accent.border,
                        width: 1.0,
                        radius: 10.0.into(),
                    },
                    ..Default::default()
                }),
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

fn build_hero_banner<'a>(
    props: &BrowseViewProps<'a>,
    playlist_play_order: Option<&[Track]>,
) -> Element<'a, Message> {
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
        let hero_play_tracks = playlist_play_order.unwrap_or(props.visible_tracks);
        if !hero_play_tracks.is_empty() {
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
                    tracks: hero_play_tracks.to_vec(),
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

        let pl_overline = match pl.map(|p| &p.origin) {
            Some(lume_core::PlaylistOrigin::SpotifySynced) => "SPOTIFY SYNCED PLAYLIST (READ-ONLY)",
            Some(lume_core::PlaylistOrigin::SpotifyImported { .. }) => "IMPORTED SPOTIFY PLAYLIST",
            _ => "CUSTOM PLAYLIST",
        };

        return container(
            row![
                art_tile,
                column![
                    text(pl_overline)
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
            ActiveNav::Home => "Home",
            ActiveNav::Search => "Explore",
            ActiveNav::LikedSongs => "Liked Songs",
            ActiveNav::LocalFiles => "Local Files",
            ActiveNav::Settings => "Settings",
            ActiveNav::Playlist(_) => "Playlist",
        };
        let sub = if total_tracks == 0 {
            match props.nav {
                ActiveNav::Home => "Listening history & live charts".to_string(),
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

fn build_playlist_sort_bar<'a>(sort: PlaylistSort, accent: AccentTheme) -> Element<'a, Message> {
    let sort_chip = |label: &'static str, field: PlaylistSortField| -> Element<'a, Message> {
        let active = sort.field == field;
        let text_col = if active {
            accent.on_primary
        } else {
            TEXT_SECONDARY
        };
        let mut inner = row![text(label).size(11).font(FONT_MEDIUM).color(text_col)]
            .spacing(4)
            .align_y(Alignment::Center);
        if active {
            let chev = if sort.direction == SortDirection::Asc {
                icon_chevron_up(11.0, accent.on_primary)
            } else {
                icon_chevron_down(11.0, accent.on_primary)
            };
            inner = inner.push(chev);
        }
        button(inner)
            .padding([5, 11])
            .style(glass_button_style(active, accent))
            .on_press(Message::SetPlaylistSort(field))
            .into()
    };

    let dir_icon = if sort.direction == SortDirection::Asc {
        icon_chevron_up(12.0, ICON_LIGHT)
    } else {
        icon_chevron_down(12.0, ICON_LIGHT)
    };
    let dir_label = if sort.direction == SortDirection::Asc {
        "Asc"
    } else {
        "Desc"
    };

    row![
        text("Sort:")
            .size(11)
            .font(FONT_SEMIBOLD)
            .color(TEXT_MUTED),
        sort_chip("Custom", PlaylistSortField::Custom),
        sort_chip("Recently Added", PlaylistSortField::RecentlyAdded),
        sort_chip("Title", PlaylistSortField::Title),
        sort_chip("Artist", PlaylistSortField::Artist),
        sort_chip("Album", PlaylistSortField::Album),
        sort_chip("Duration", PlaylistSortField::Duration),
        space().width(Length::Fill),
        button(
            row![
                dir_icon,
                text(dir_label).size(11).font(FONT_MEDIUM).color(TEXT_PRIMARY),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
        )
        .padding([5, 10])
        .style(glass_button_style(false, accent))
        .on_press(Message::TogglePlaylistSortDirection),
    ]
    .spacing(6)
    .padding([0, 6])
    .align_y(Alignment::Center)
    .into()
}

fn truncate_label(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let taken: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", taken.trim_end())
    } else {
        taken
    }
}

fn home_history_card<'a>(
    props: &BrowseViewProps<'a>,
    track: &'a Track,
    idx: usize,
) -> Element<'a, Message> {
    let accent = props.accent;
    let is_active = props.active_track_uid == Some(track.uid.as_str());
    let art = track
        .artwork_ref
        .as_deref()
        .and_then(|k| props.art_handles.get(k));
    let title_col = if is_active { accent.light } else { TEXT_PRIMARY };

    let card_inner = column![
        cover_art_widget(art, 112.0, 12.0),
        column![
            text(truncate_label(&track.title, 18))
                .size(12)
                .font(FONT_SEMIBOLD)
                .color(title_col),
            text(truncate_label(&track.artist_display(), 20))
                .size(11)
                .color(TEXT_SECONDARY),
        ]
        .spacing(2)
        .width(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fixed(128.0));

    button(card_inner)
        .padding(10)
        .style(track_row_button_style(is_active, accent))
        .on_press(Message::PlayTrackList {
            tracks: props.recent_tracks.to_vec(),
            start_index: idx,
        })
        .into()
}

fn build_home_chart_section<'a>(
    props: &BrowseViewProps<'a>,
    title: String,
    subtitle: &'static str,
    tracks: &'a [Track],
    loading: bool,
    expanded: bool,
    toggle_msg: Message,
) -> Element<'a, Message> {
    let accent = props.accent;
    let visible_limit = if expanded { tracks.len() } else { 8.min(tracks.len()) };

    let mut header_actions = row![].spacing(8).align_y(Alignment::Center);
    if !tracks.is_empty() {
        header_actions = header_actions.push(
            button(
                row![
                    icon_play(11.0, accent.on_primary),
                    text("Play All")
                        .size(11)
                        .font(FONT_SEMIBOLD)
                        .color(accent.on_primary),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .padding([6, 13])
            .style(primary_pill_button_style(accent))
            .on_press(Message::PlayTrackList {
                tracks: tracks.to_vec(),
                start_index: 0,
            }),
        );
        if tracks.len() > 8 {
            header_actions = header_actions.push(
                button(
                    text(if expanded {
                        "Show less".to_string()
                    } else {
                        format!("Show all ({})", tracks.len())
                    })
                    .size(11)
                    .font(FONT_MEDIUM)
                    .color(TEXT_PRIMARY),
                )
                .padding([6, 12])
                .style(glass_button_style(false, accent))
                .on_press(toggle_msg),
            );
        }
    }

    let header = row![
        icon_trending(15.0, accent.light),
        column![
            text(title).size(15).font(FONT_BOLD).color(TEXT_PRIMARY),
            text(subtitle).size(11).color(TEXT_MUTED),
        ]
        .spacing(1)
        .width(Length::Fill),
        header_actions,
    ]
    .spacing(10)
    .align_y(Alignment::Center);

    let mut body = column![header].spacing(10);
    if loading && tracks.is_empty() {
        body = body.push(
            container(
                text("Loading live chart...")
                    .size(12)
                    .font(FONT_MEDIUM)
                    .color(TEXT_MUTED),
            )
            .padding([14, 6]),
        );
    } else if tracks.is_empty() {
        body = body.push(
            container(
                text("Chart unavailable right now — click refresh to retry.")
                    .size(12)
                    .color(TEXT_MUTED),
            )
            .padding([12, 6]),
        );
    } else {
        let mut rows_col = column![].spacing(3);
        for (idx, track) in tracks.iter().take(visible_limit).enumerate() {
            rows_col = rows_col.push(track_row(props, track, idx, idx, tracks));
        }
        body = body.push(rows_col);
    }

    container(body)
        .padding([14, 14])
        .width(Length::Fill)
        .style(glass_card_style)
        .into()
}

fn build_home_content<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;

    let region_pill = |region: ChartRegion| -> Element<'a, Message> {
        let active = props.home_region == region;
        button(
            text(region.label())
                .size(11)
                .font(if active { FONT_SEMIBOLD } else { FONT_MEDIUM })
                .color(if active {
                    accent.on_primary
                } else {
                    TEXT_SECONDARY
                }),
        )
        .padding([6, 13])
        .style(glass_button_style(active, accent))
        .on_press(Message::SelectHomeRegion(region))
        .into()
    };

    let home_header = row![
        column![
            text("Home").size(22).font(FONT_BOLD).color(TEXT_PRIMARY),
            text("Listening history, playlists & live charts")
                .size(12)
                .color(TEXT_SECONDARY),
        ]
        .spacing(2)
        .width(Length::Fill),
        row![
            region_pill(ChartRegion::Indonesia),
            region_pill(ChartRegion::Global),
            button(icon_refresh(13.0, ICON_LIGHT))
                .padding([7, 9])
                .style(glass_button_style(false, accent))
                .on_press(Message::RefreshHomeCharts),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    ]
    .align_y(Alignment::Center)
    .padding([2, 4]);

    let mut feed = column![home_header].spacing(14).width(Length::Fill);

    if let Some(toast) = props.status_banner {
        feed = feed.push(
            container(
                row![
                    text(toast).size(12).color(TEXT_PRIMARY).width(Length::Fill),
                    button(text("Dismiss").size(11))
                        .padding([4, 10])
                        .style(glass_button_style(false, accent))
                        .on_press(Message::DismissToast),
                ]
                .align_y(Alignment::Center),
            )
            .padding([10, 14])
            .style(glass_elevated_style),
        );
    }

    if props.active_track.is_some() {
        feed = feed.push(build_hero_banner(props, None));
    }

    // Quick Access: Your Playlists cards
    if !props.playlists.is_empty() {
        let mut pl_row = row![].spacing(10).width(Length::Fill);
        for pl in props.playlists.iter().take(4) {
            let origin_tag = match &pl.origin {
                lume_core::PlaylistOrigin::SpotifySynced => "Spotify",
                lume_core::PlaylistOrigin::SpotifyImported { .. } => "Imported",
                lume_core::PlaylistOrigin::LocalCustom => "Custom",
            };
            let card = button(
                row![
                    container(icon_playlist(16.0, accent.light))
                        .width(Length::Fixed(36.0))
                        .height(Length::Fixed(36.0))
                        .center_x(Length::Fixed(36.0))
                        .center_y(Length::Fixed(36.0))
                        .style(move |_| container::Style {
                            background: Some(Background::Color(accent.soft_bg)),
                            border: iced::border::rounded(10.0),
                            ..Default::default()
                        }),
                    column![
                        text(truncate_label(&pl.name, 16))
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(TEXT_PRIMARY),
                        text(format!("{} songs • {origin_tag}", pl.track_count))
                            .size(10)
                            .color(TEXT_MUTED),
                    ]
                    .spacing(2)
                    .width(Length::Fill),
                ]
                .spacing(10)
                .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .padding([10, 12])
            .style(glass_button_style(false, accent))
            .on_press(Message::SelectNav(ActiveNav::Playlist(pl.id.clone())));
            pl_row = pl_row.push(card);
        }

        feed = feed.push(
            column![
                text("YOUR PLAYLISTS")
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(TEXT_MUTED),
                pl_row,
            ]
            .spacing(8),
        );
    }

    // Recently Played (Listening History) card strip
    let recent_filtered: Vec<(usize, &Track)> = props
        .recent_tracks
        .iter()
        .enumerate()
        .filter(|(_, t)| matches_source_filter(&t.source, props.source_filter))
        .collect();

    let mut recent_header = row![
        icon_clock(15.0, accent.light),
        text("Recently Played")
            .size(15)
            .font(FONT_BOLD)
            .color(TEXT_PRIMARY)
            .width(Length::Fill),
    ]
    .spacing(8)
    .align_y(Alignment::Center);

    if !recent_filtered.is_empty() {
        recent_header = recent_header.push(
            text(format!("{} tracks", recent_filtered.len()))
                .size(11)
                .color(TEXT_MUTED),
        );
    }

    let recent_body: Element<'a, Message> = if recent_filtered.is_empty() {
        container(
            text("Play any song from your playlists, charts, or search to build your listening history.")
                .size(12)
                .color(TEXT_MUTED),
        )
        .padding([12, 6])
        .into()
    } else {
        let mut cards_row = row![].spacing(10);
        for (orig_idx, track) in recent_filtered.into_iter().take(10) {
            cards_row = cards_row.push(home_history_card(props, track, orig_idx));
        }
        scrollable(cards_row)
            .direction(scrollable::Direction::Horizontal(hidden_scrollbar()))
            .style(thin_scrollable_style)
            .width(Length::Fill)
            .into()
    };

    feed = feed.push(
        container(column![recent_header, recent_body].spacing(10))
            .padding([14, 14])
            .width(Length::Fill)
            .style(glass_card_style),
    );

    // Trending on Spotify
    if matches!(props.source_filter, SourceFilter::All | SourceFilter::Spotify) {
        let sp_title = format!("Trending on Spotify • {}", props.home_region.label());
        let sp_sub = match props.home_region {
            ChartRegion::Indonesia => "Top 50 - Indonesia • Plays via Smart YouTube Fallback",
            ChartRegion::Global => "Top 50 - Global • Plays via Smart YouTube Fallback",
        };
        feed = feed.push(build_home_chart_section(
            props,
            sp_title,
            sp_sub,
            props.home_spotify_trending,
            props.home_loading_spotify,
            props.home_spotify_expanded,
            Message::ToggleExpandSpotifyTrending,
        ));
    }

    // Trending on YouTube Music
    if matches!(
        props.source_filter,
        SourceFilter::All | SourceFilter::YouTubeMusic
    ) {
        let ytm_title = format!("Trending on YouTube Music • {}", props.home_region.label());
        let ytm_sub = match props.home_region {
            ChartRegion::Indonesia => "YouTube Music Charts Indonesia • Direct stream",
            ChartRegion::Global => "YouTube Music Charts Global • Direct stream",
        };
        feed = feed.push(build_home_chart_section(
            props,
            ytm_title,
            ytm_sub,
            props.home_ytm_trending,
            props.home_loading_ytm,
            props.home_ytm_expanded,
            Message::ToggleExpandYtmTrending,
        ));
    }

    if props.source_filter == SourceFilter::Local {
        feed = feed.push(
            container(
                row![
                    column![
                        text("Local Music Library")
                            .size(14)
                            .font(FONT_BOLD)
                            .color(TEXT_PRIMARY),
                        text("Browse your local MP3, FLAC, WAV, and M4A folders in Local Files.")
                            .size(12)
                            .color(TEXT_SECONDARY),
                    ]
                    .spacing(3)
                    .width(Length::Fill),
                    button(
                        text("Open Local Files")
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(accent.on_primary),
                    )
                    .padding([8, 16])
                    .style(primary_pill_button_style(accent))
                    .on_press(Message::SelectNav(ActiveNav::LocalFiles)),
                ]
                .spacing(12)
                .align_y(Alignment::Center),
            )
            .padding([16, 18])
            .width(Length::Fill)
            .style(glass_card_style),
        );
    }

    glass_panel(
        scrollable(feed.padding([0, 4]))
            .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
            .height(Length::Fill)
            .width(Length::Fill)
            .style(thin_scrollable_style),
    )
    .padding(16)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn build_main_content<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    if matches!(props.nav, ActiveNav::Home) {
        return build_home_content(props);
    }

    let active_filter_label = match props.source_filter {
        SourceFilter::All => "All Sources",
        SourceFilter::Spotify => "Spotify",
        SourceFilter::YouTubeMusic => "YouTube",
        SourceFilter::Local => "Local",
    };

    let is_playlist_view = matches!(props.nav, ActiveNav::Playlist(_));
    let sorted_playlist_entries: Vec<(usize, &Track)> = if is_playlist_view {
        sort_playlist_tracks(props.visible_tracks, props.playlist_sort)
            .into_iter()
            .filter(|(_, t)| matches_source_filter(&t.source, props.source_filter))
            .collect()
    } else {
        Vec::new()
    };
    let sorted_playlist_tracks: Vec<Track> = if is_playlist_view {
        sorted_playlist_entries
            .iter()
            .map(|(_, t)| (*t).clone())
            .collect()
    } else {
        Vec::new()
    };

    let hero = build_hero_banner(
        props,
        is_playlist_view.then_some(sorted_playlist_tracks.as_slice()),
    );

    let grouped = if is_playlist_view {
        Vec::new()
    } else {
        group_tracks_by_source(props.visible_tracks, props.source_filter)
    };
    let displayed_count: usize = if is_playlist_view {
        sorted_playlist_entries.len()
    } else {
        grouped.iter().map(|(_, items)| items.len()).sum()
    };

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

    if is_playlist_view && !props.visible_tracks.is_empty() {
        content_col = content_col.push(build_playlist_sort_bar(props.playlist_sort, props.accent));
    }

    if displayed_count > 0 {
        let header_row: Element<'a, Message> = if is_playlist_view {
            let col_btn = |label: &'static str, field: PlaylistSortField, width: Length| {
                let active = props.playlist_sort.field == field;
                let col = if active { props.accent.light } else { TEXT_MUTED };
                let mut inner = row![text(label).size(11).color(col)]
                    .spacing(3)
                    .align_y(Alignment::Center);
                if active {
                    inner = inner.push(if props.playlist_sort.direction == SortDirection::Asc {
                        icon_chevron_up(10.0, props.accent.light)
                    } else {
                        icon_chevron_down(10.0, props.accent.light)
                    });
                }
                button(inner)
                    .padding([2, 0])
                    .width(width)
                    .style(ghost_button_style)
                    .on_press(Message::SetPlaylistSort(field))
            };
            row![
                col_btn("#", PlaylistSortField::Custom, Length::Fixed(30.0)),
                col_btn("TITLE", PlaylistSortField::Title, Length::FillPortion(4)),
                col_btn("ALBUM", PlaylistSortField::Album, Length::FillPortion(3)),
                text("SOURCE")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(72.0)),
                col_btn("TIME", PlaylistSortField::Duration, Length::Fixed(48.0)),
                space().width(Length::Fixed(64.0)),
            ]
            .spacing(12)
            .padding([4, 12])
            .align_y(Alignment::Center)
            .into()
        } else {
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
            .align_y(Alignment::Center)
            .into()
        };

        let table_header = column![
            header_row,
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
    } else if is_playlist_view {
        for (display_idx, (orig_pos, track)) in sorted_playlist_entries.into_iter().enumerate() {
            track_list = track_list.push(track_row(
                props,
                track,
                display_idx,
                orig_pos,
                &sorted_playlist_tracks,
            ));
        }
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
                track_list = track_list.push(track_row(
                    props,
                    track,
                    global_idx,
                    global_idx,
                    props.visible_tracks,
                ));
            }
        }
    }

    content_col = content_col.push(
        scrollable(track_list)
            .direction(scrollable::Direction::Vertical(hidden_scrollbar()))
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
        ActiveNav::Settings | ActiveNav::Home => ("", "", None),
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
    orig_playlist_pos: usize,
    play_tracks: &[Track],
) -> Element<'a, Message> {
    let is_active = props.active_track_uid == Some(track.uid.as_str());
    let menu_open = props.open_track_menu_uid == Some(track.uid.as_str());
    let art = track
        .artwork_ref
        .as_deref()
        .and_then(|k| props.art_handles.get(k));

    let play_msg = Message::PlayTrackList {
        tracks: play_tracks.to_vec(),
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

    if matches!(track.source, TrackSource::Spotify { .. }) {
        actions = actions.push(
            button(text("Re-match YouTube").size(12))
                .padding([6, 12])
                .style(glass_button_style(false, props.accent))
                .on_press(Message::RematchYoutubeFallback(track.clone())),
        );
    }

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
            let allow_manual_reorder = props.playlist_sort == PlaylistSort::default()
                && props.source_filter == SourceFilter::All;
            if allow_manual_reorder && orig_playlist_pos > 0 {
                actions = actions.push(
                    button(icon_chevron_up(14.0, ICON_LIGHT))
                        .padding([6, 8])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::ReorderPlaylistTrack {
                            playlist_id: pl_id.clone(),
                            from: orig_playlist_pos,
                            to: orig_playlist_pos - 1,
                        }),
                );
            }
            if allow_manual_reorder && orig_playlist_pos + 1 < props.visible_tracks.len() {
                actions = actions.push(
                    button(icon_chevron_down(14.0, ICON_LIGHT))
                        .padding([6, 8])
                        .style(glass_button_style(false, props.accent))
                        .on_press(Message::ReorderPlaylistTrack {
                            playlist_id: pl_id.clone(),
                            from: orig_playlist_pos,
                            to: orig_playlist_pos + 1,
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
                    position: orig_playlist_pos,
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

    #[test]
    fn sorts_playlist_by_recently_added_title_artist_album_and_duration_preserving_orig_pos() {
        let mut t0 = mk_track(TrackSource::Spotify { id: "s0".into() }, "Yellow");
        t0.artists = vec!["Coldplay".into()];
        t0.album = Some("Parachutes".into());
        t0.duration_ms = 266_000;

        let mut t1 = mk_track(TrackSource::Spotify { id: "s1".into() }, "drunk text");
        t1.artists = vec!["Henry Moodie".into()];
        t1.album = None;
        t1.duration_ms = 187_000;

        let mut t2 = mk_track(TrackSource::Spotify { id: "s2".into() }, "Blinding Lights");
        t2.artists = vec!["The Weeknd".into()];
        t2.album = Some("After Hours".into());
        t2.duration_ms = 200_000;

        let tracks = vec![t0, t1, t2];

        // RecentlyAdded defaults to Desc (newest / last added first).
        let mut sort = PlaylistSort::default();
        sort.select(PlaylistSortField::RecentlyAdded);
        assert_eq!(sort.direction, SortDirection::Desc);
        let recent: Vec<usize> = sort_playlist_tracks(&tracks, sort)
            .into_iter()
            .map(|(orig, _)| orig)
            .collect();
        assert_eq!(recent, vec![2, 1, 0]);

        // Title Asc (case-insensitive) then click again flips to Desc.
        sort.select(PlaylistSortField::Title);
        assert_eq!(sort.direction, SortDirection::Asc);
        let by_title: Vec<usize> = sort_playlist_tracks(&tracks, sort)
            .into_iter()
            .map(|(orig, _)| orig)
            .collect();
        assert_eq!(by_title, vec![2, 1, 0]); // Blinding Lights, drunk text, Yellow

        sort.select(PlaylistSortField::Title);
        assert_eq!(sort.direction, SortDirection::Desc);
        let by_title_desc: Vec<usize> = sort_playlist_tracks(&tracks, sort)
            .into_iter()
            .map(|(orig, _)| orig)
            .collect();
        assert_eq!(by_title_desc, vec![0, 1, 2]);

        // Album Asc keeps empty album (`t1`) at the bottom.
        sort.select(PlaylistSortField::Album);
        let by_album: Vec<usize> = sort_playlist_tracks(&tracks, sort)
            .into_iter()
            .map(|(orig, _)| orig)
            .collect();
        assert_eq!(by_album, vec![2, 0, 1]); // After Hours, Parachutes, None

        // Duration Asc.
        sort.select(PlaylistSortField::Duration);
        let by_dur: Vec<usize> = sort_playlist_tracks(&tracks, sort)
            .into_iter()
            .map(|(orig, _)| orig)
            .collect();
        assert_eq!(by_dur, vec![1, 2, 0]); // 187s, 200s, 266s
    }
}
