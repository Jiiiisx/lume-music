use std::collections::HashMap;

use iced::widget::{
    button, column, container, image, mouse_area, row, scrollable, space, text, text_input,
};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{Playlist, PlaylistOrigin, Track, TrackSource};

use crate::views::{ActiveNav, Message, SourceFilter};
use crate::widgets::{
    cover_art_widget, ghost_button_style, glass_button_style, glass_elevated_style,
    glass_input_style, glass_panel, primary_pill_button_style, thin_scrollable_style, GLASS_BORDER,
    TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
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
        .spacing(14)
        .padding([0, 14])
        .width(Length::Fill)
        .height(Length::Fill);

    if props.show_new_playlist_modal {
        let modal = glass_panel(
            container(
                column![
                    text("New Custom Playlist").size(18).color(TEXT_PRIMARY),
                    text("Mix Spotify, YouTube Music, and Local tracks freely.")
                        .size(12)
                        .color(TEXT_SECONDARY),
                    text_input("Playlist name...", props.new_playlist_input)
                        .on_input(Message::NewPlaylistNameChanged)
                        .on_submit(Message::CreatePlaylistSubmitted)
                        .style(glass_input_style)
                        .padding(10),
                    if let Some(err) = props.modal_error {
                        Element::from(text(err).size(12).color(Color::from_rgb8(248, 113, 113)))
                    } else {
                        Element::from(space().height(0))
                    },
                    row![
                        space().width(Length::Fill),
                        button(text("Cancel").size(13))
                            .padding([8, 14])
                            .style(glass_button_style(false))
                            .on_press(Message::CloseModal),
                        button(text("Create").size(13))
                            .padding([8, 16])
                            .style(primary_pill_button_style)
                            .on_press(Message::CreatePlaylistSubmitted),
                    ]
                    .spacing(10),
                ]
                .spacing(12)
                .width(Length::Fixed(360.0)),
            )
            .padding(20)
            .style(glass_elevated_style),
        );
        return iced::widget::stack![
            base,
            container(modal)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.52))),
                    ..Default::default()
                })
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    }

    if props.show_import_url_modal {
        let modal = glass_panel(
            container(
                column![
                    text("Import Public Spotify Playlist URL").size(18).color(TEXT_PRIMARY),
                    text("Paste a public open.spotify.com/playlist/... link (Phase 3 importer hook).")
                        .size(12)
                        .color(TEXT_SECONDARY),
                    text_input(
                        "https://open.spotify.com/playlist/...",
                        props.import_url_input
                    )
                    .on_input(Message::ImportUrlInputChanged)
                    .on_submit(Message::ImportSpotifyUrlSubmitted)
                    .style(glass_input_style)
                    .padding(10),
                    if let Some(err) = props.modal_error {
                        Element::from(text(err).size(12).color(Color::from_rgb8(251, 191, 36)))
                    } else {
                        Element::from(space().height(0))
                    },
                    row![
                        space().width(Length::Fill),
                        button(text("Close").size(13))
                            .padding([8, 14])
                            .style(glass_button_style(false))
                            .on_press(Message::CloseModal),
                        button(text("Import").size(13))
                            .padding([8, 16])
                            .style(primary_pill_button_style)
                            .on_press(Message::ImportSpotifyUrlSubmitted),
                    ]
                    .spacing(10),
                ]
                .spacing(12)
                .width(Length::Fixed(420.0)),
            )
            .padding(20)
            .style(glass_elevated_style),
        );
        return iced::widget::stack![
            base,
            container(modal)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(0.0, 0.0, 0.0, 0.52))),
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
    let nav_btn = |label: &'a str, target: ActiveNav| -> Element<'a, Message> {
        let active = nav == &target;
        button(text(label).size(14))
            .width(Length::Fill)
            .padding([9, 12])
            .style(glass_button_style(active))
            .on_press(Message::SelectNav(target))
            .into()
    };

    let mut playlist_col = column![].spacing(4);
    for pl in playlists {
        let is_active = matches!(nav, ActiveNav::Playlist(id) if id == &pl.id);
        let origin_tag = match &pl.origin {
            PlaylistOrigin::SpotifySynced => "Spotify",
            PlaylistOrigin::SpotifyImported { .. } => "Imported",
            PlaylistOrigin::LocalCustom => "Local",
        };
        let label = format!("{}  ({})", pl.name, pl.track_count);
        let row_btn = button(
            row![
                text(label).size(13).width(Length::Fill),
                text(origin_tag).size(10).color(TEXT_MUTED),
            ]
            .align_y(Alignment::Center)
            .spacing(6),
        )
        .width(Length::Fill)
        .padding([7, 10])
        .style(glass_button_style(is_active))
        .on_press(Message::SelectNav(ActiveNav::Playlist(pl.id.clone())));
        playlist_col = playlist_col.push(row_btn);
    }

    glass_panel(
        column![
            text("LUME MUSIC").size(15).color(TEXT_PRIMARY),
            nav_btn("Search", ActiveNav::Search),
            nav_btn("Liked Songs", ActiveNav::LikedSongs),
            nav_btn("Local Files", ActiveNav::LocalFiles),
            nav_btn("Settings", ActiveNav::Settings),
            container(space().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(GLASS_BORDER)),
                    ..Default::default()
                }),
            row![
                text("PLAYLISTS").size(11).color(TEXT_MUTED),
                space().width(Length::Fill),
                button(text("+ New").size(11))
                    .padding([4, 8])
                    .style(glass_button_style(false))
                    .on_press(Message::OpenNewPlaylistModal),
                button(text("URL").size(11))
                    .padding([4, 8])
                    .style(glass_button_style(false))
                    .on_press(Message::OpenImportUrlModal),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            scrollable(playlist_col)
                .height(Length::Fill)
                .style(thin_scrollable_style),
            container(text("Local Engine Active • Phase 1").size(11).color(TEXT_MUTED))
                .padding([6, 10])
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.05))),
                    border: iced::border::rounded(8.0),
                    ..Default::default()
                }),
        ]
        .spacing(10)
        .padding(14)
        .width(Length::Fixed(248.0))
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .into()
}

fn build_main_content<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let search_bar = row![
        text_input("Search local tracks (press Enter for remote sources)...", props.search_query)
            .on_input(Message::SearchInputChanged)
            .on_submit(Message::SearchSubmitted)
            .style(glass_input_style)
            .padding([10, 14])
            .width(Length::Fill),
    ]
    .spacing(8);

    let filter_chips = row![
        chip_btn("All", SourceFilter::All, props.source_filter),
        chip_btn("Spotify", SourceFilter::Spotify, props.source_filter),
        chip_btn("YouTube Music", SourceFilter::YouTubeMusic, props.source_filter),
        chip_btn("Local", SourceFilter::Local, props.source_filter),
    ]
    .spacing(8);

    let header_title: Element<'a, Message> = match props.nav {
        ActiveNav::Search => text("Unified Search").size(22).color(TEXT_PRIMARY).into(),
        ActiveNav::LikedSongs => text("Liked Songs (Unified)").size(22).color(TEXT_PRIMARY).into(),
        ActiveNav::LocalFiles => text("Local Files").size(22).color(TEXT_PRIMARY).into(),
        ActiveNav::Settings => text("Settings").size(22).color(TEXT_PRIMARY).into(),
        ActiveNav::Playlist(id) => {
            if let Some((renaming_id, draft)) = props.renaming_playlist {
                if renaming_id == id {
                    row![
                        text_input("Playlist name...", draft)
                            .on_input(Message::RenamePlaylistInputChanged)
                            .on_submit(Message::SubmitRenamePlaylist)
                            .style(glass_input_style)
                            .padding([6, 10])
                            .width(Length::Fixed(240.0)),
                        button(text("Save").size(12))
                            .padding([6, 12])
                            .style(primary_pill_button_style)
                            .on_press(Message::SubmitRenamePlaylist),
                        button(text("Cancel").size(12))
                            .padding([6, 10])
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
    let mut track_list = column![].spacing(6);

    if props.visible_tracks.is_empty() {
        let empty_msg = match props.nav {
            ActiveNav::LocalFiles => "No local tracks indexed yet. Add a music folder in Settings.",
            ActiveNav::LikedSongs => "No liked tracks yet. Click the heart on any song to save it here.",
            ActiveNav::Search => "No matching tracks found.",
            ActiveNav::Playlist(_) => "This playlist has no tracks yet.",
            ActiveNav::Settings => "",
        };
        track_list = track_list.push(
            container(text(empty_msg).size(14).color(TEXT_SECONDARY))
                .padding(24)
                .width(Length::Fill),
        );
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

    let mut content_col = column![search_bar, filter_chips, header_title].spacing(12);
    if let Some(toast) = props.status_banner {
        content_col = content_col.push(
            container(
                row![
                    text(toast).size(12).color(TEXT_PRIMARY).width(Length::Fill),
                    button(text("Dismiss").size(11))
                        .padding([3, 8])
                        .style(ghost_button_style)
                        .on_press(Message::DismissToast),
                ]
                .align_y(Alignment::Center),
            )
            .padding([8, 12])
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
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn playlist_header<'a>(props: &BrowseViewProps<'a>, id: &'a str) -> Element<'a, Message> {
    if let Some(pl) = props.playlists.iter().find(|p| p.id == id) {
        let mut hdr = row![text(pl.name.as_str()).size(22).color(TEXT_PRIMARY)]
            .spacing(10)
            .align_y(Alignment::Center);
        if pl.origin.is_editable() {
            hdr = hdr
                .push(
                    button(text("Rename").size(12))
                        .padding([5, 10])
                        .style(glass_button_style(false))
                        .on_press(Message::StartRenamePlaylist(pl.id.clone(), pl.name.clone())),
                )
                .push(
                    button(text("Delete").size(12))
                        .padding([5, 10])
                        .style(glass_button_style(false))
                        .on_press(Message::DeletePlaylist(pl.id.clone())),
                );
        } else {
            hdr = hdr.push(
                container(text("Spotify Synced • Read-only").size(11).color(TEXT_MUTED))
                    .padding([4, 10])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
                        border: iced::border::rounded(10.0),
                        ..Default::default()
                    }),
            );
        }
        hdr.into()
    } else {
        text("Playlist").size(22).color(TEXT_PRIMARY).into()
    }
}

fn chip_btn<'a>(
    label: &'a str,
    target: SourceFilter,
    current: SourceFilter,
) -> Element<'a, Message> {
    button(text(label).size(12))
        .padding([6, 14])
        .style(glass_button_style(target == current))
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

    let main_row = mouse_area(
        container(
            row![
                cover_art_widget(art, 42.0, 6.0),
                column![
                    text(track.title.as_str())
                        .size(14)
                        .color(if is_active {
                            Color::from_rgb8(255, 224, 178)
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
                container(text(source_label(&track.source)).size(11).color(TEXT_MUTED))
                    .padding([2, 8])
                    .style(|_| container::Style {
                        background: Some(Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.06))),
                        border: iced::border::rounded(8.0),
                        ..Default::default()
                    }),
                text(format_duration_ms(track.duration_ms))
                    .size(12)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(46.0)),
                button(text(if track.is_liked { "♥" } else { "♡" }).size(14))
                    .padding([4, 8])
                    .style(ghost_button_style)
                    .on_press(Message::ToggleLike(track.clone())),
                button(text("•••").size(12))
                    .padding([4, 8])
                    .style(ghost_button_style)
                    .on_press(Message::ToggleTrackMenu(track.uid.clone())),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding([8, 12])
        .style(move |_| container::Style {
            background: Some(Background::Color(if is_active {
                Color::from_rgba(1.0, 1.0, 1.0, 0.14)
            } else {
                Color::from_rgba(1.0, 1.0, 1.0, 0.04)
            })),
            border: iced::border::Border {
                color: if is_active {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.24)
                } else {
                    Color::from_rgba(1.0, 1.0, 1.0, 0.06)
                },
                width: 1.0,
                radius: 10.0.into(),
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
        button(text("Play Next").size(11))
            .padding([5, 10])
            .style(glass_button_style(false))
            .on_press(Message::PlayNext(track.clone())),
        button(text("Add to Queue").size(11))
            .padding([5, 10])
            .style(glass_button_style(false))
            .on_press(Message::AddToQueue(track.clone())),
    ]
    .spacing(6);

    for pl in props.playlists.iter().filter(|p| p.origin.is_editable()) {
        actions = actions.push(
            button(text(format!("+ {}", pl.name)).size(11))
                .padding([5, 10])
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
                    button(text("↑").size(11))
                        .padding([5, 8])
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
                    button(text("↓").size(11))
                        .padding([5, 8])
                        .style(glass_button_style(false))
                        .on_press(Message::ReorderPlaylistTrack {
                            playlist_id: pl_id.clone(),
                            from: index,
                            to: index + 1,
                        }),
                );
            }
            actions = actions.push(
                button(text("Remove").size(11))
                    .padding([5, 10])
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
            .padding([6, 12])
            .width(Length::Fill)
            .style(glass_elevated_style)
    ]
    .spacing(4)
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
        assert_eq!(validate_playlist_name("  Night Drive  "), Ok("Night Drive".into()));
    }
}
