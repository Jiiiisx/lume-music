use iced::widget::{button, column, container, row, scrollable, space, text};
use iced::{Alignment, Background, Color, Element, Length};
use lume_core::{
    AlbumSummary, ArtistSummary, CatalogSource, RemotePlaylistSummary, TrackSource,
};

use crate::theme::{AccentTheme, FONT_BOLD, FONT_MEDIUM, FONT_SEMIBOLD};
use crate::views::browse::{track_row, BrowseViewProps};
use crate::views::{
    responsive_grid_columns, responsive_sidebar_width, show_track_album_column,
    show_track_source_column, ActiveNav, Message, SearchTab, SourceFilter,
};
use crate::widgets::{
    cover_art_widget, glass_button_style, glass_card_style, glass_elevated_style, glass_panel,
    hero_banner_style, hidden_scrollbar, icon_artist, icon_back, icon_check, icon_disc, icon_play,
    icon_playlist, icon_plus, primary_pill_button_style, thin_scrollable_style,
    track_row_button_style, GLASS_SUBTLE_BORDER, ICON_LIGHT, TEXT_MUTED, TEXT_PRIMARY,
    TEXT_SECONDARY,
};

fn truncate_text(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let head: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", head.trim_end())
    } else {
        head
    }
}

fn source_pill<'a>(source: CatalogSource, accent: AccentTheme) -> Element<'a, Message> {
    let (bg, border_col, txt_col) = match source {
        CatalogSource::Spotify => (
            Color::from_rgba(29.0 / 255.0, 185.0 / 255.0, 84.0 / 255.0, 0.22),
            Color::from_rgba(29.0 / 255.0, 185.0 / 255.0, 84.0 / 255.0, 0.48),
            Color::from_rgb8(134, 239, 172),
        ),
        CatalogSource::YouTubeMusic => (accent.soft_bg, accent.border, accent.light),
    };
    container(
        text(source.short_badge())
            .size(9)
            .font(FONT_SEMIBOLD)
            .color(txt_col),
    )
    .padding([2, 6])
    .style(move |_| container::Style {
        background: Some(Background::Color(bg)),
        border: iced::border::Border {
            color: border_col,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn artist_avatar_widget<'a>(
    handle: Option<&'a iced::widget::image::Handle>,
    size: f32,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let radius = size * 0.5;
    if handle.is_some() {
        cover_art_widget(handle, size, radius)
    } else {
        container(icon_artist(size * 0.42, accent.light))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size))
            .style(move |_| container::Style {
                background: Some(Background::Color(accent.soft_bg)),
                border: iced::border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

fn album_cover_widget<'a>(
    handle: Option<&'a iced::widget::image::Handle>,
    size: f32,
    radius: f32,
    accent: AccentTheme,
) -> Element<'a, Message> {
    if handle.is_some() {
        cover_art_widget(handle, size, radius)
    } else {
        container(icon_disc(size * 0.36, accent.light))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size))
            .style(move |_| container::Style {
                background: Some(Background::Color(accent.soft_bg)),
                border: iced::border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

fn playlist_cover_widget<'a>(
    handle: Option<&'a iced::widget::image::Handle>,
    size: f32,
    radius: f32,
    accent: AccentTheme,
) -> Element<'a, Message> {
    if handle.is_some() {
        cover_art_widget(handle, size, radius)
    } else {
        container(icon_playlist(size * 0.36, accent.light))
            .width(Length::Fixed(size))
            .height(Length::Fixed(size))
            .center_x(Length::Fixed(size))
            .center_y(Length::Fixed(size))
            .style(move |_| container::Style {
                background: Some(Background::Color(accent.soft_bg)),
                border: iced::border::Border {
                    color: accent.border,
                    width: 1.0,
                    radius: radius.into(),
                },
                ..Default::default()
            })
            .into()
    }
}

pub fn collect_filtered_artists<'a>(props: &BrowseViewProps<'a>) -> Vec<&'a ArtistSummary> {
    let mut out = Vec::new();
    match props.source_filter {
        SourceFilter::All => {
            out.extend(props.spotify_search_artists.iter());
            out.extend(props.ytm_search_artists.iter());
        }
        SourceFilter::Spotify => {
            if props.spotify_web_api_active && !props.spotify_search_artists.is_empty() {
                out.extend(props.spotify_search_artists.iter());
            } else {
                out.extend(props.ytm_search_artists.iter());
            }
        }
        SourceFilter::YouTubeMusic => {
            out.extend(props.ytm_search_artists.iter());
        }
        SourceFilter::Local => {}
    }
    out
}

pub fn collect_filtered_playlists<'a>(
    props: &BrowseViewProps<'a>,
) -> Vec<&'a RemotePlaylistSummary> {
    let mut out = Vec::new();
    match props.source_filter {
        SourceFilter::All => {
            out.extend(props.spotify_search_playlists.iter());
            out.extend(props.ytm_search_playlists.iter());
        }
        SourceFilter::Spotify => {
            if props.spotify_web_api_active && !props.spotify_search_playlists.is_empty() {
                out.extend(props.spotify_search_playlists.iter());
            } else {
                out.extend(props.ytm_search_playlists.iter());
            }
        }
        SourceFilter::YouTubeMusic => {
            out.extend(props.ytm_search_playlists.iter());
        }
        SourceFilter::Local => {}
    }
    out
}

pub fn build_search_tabs_bar<'a>(
    active_tab: SearchTab,
    song_count: usize,
    artist_count: usize,
    playlist_count: usize,
    accent: AccentTheme,
) -> Element<'a, Message> {
    let chip = |tab: SearchTab, count: Option<usize>| -> Element<'a, Message> {
        let active = active_tab == tab;
        let label = match count {
            Some(c) if c > 0 => format!("{} ({c})", tab.label()),
            _ => tab.label().to_string(),
        };
        let col = if active {
            accent.on_primary
        } else {
            TEXT_SECONDARY
        };
        button(
            text(label)
                .size(11)
                .font(if active { FONT_SEMIBOLD } else { FONT_MEDIUM })
                .color(col),
        )
        .padding([6, 13])
        .style(glass_button_style(active, accent))
        .on_press(Message::SearchTabChanged(tab))
        .into()
    };

    row![
        chip(SearchTab::All, None),
        chip(SearchTab::Songs, Some(song_count)),
        chip(SearchTab::Artists, Some(artist_count)),
        chip(SearchTab::Playlists, Some(playlist_count)),
    ]
    .spacing(6)
    .align_y(Alignment::Center)
    .into()
}

pub fn artist_card<'a>(
    props: &BrowseViewProps<'a>,
    artist: &'a ArtistSummary,
) -> Element<'a, Message> {
    let accent = props.accent;
    let handle = artist
        .avatar_url
        .as_deref()
        .and_then(|u| props.art_handles.get(u));
    let sub = artist
        .subtitle
        .as_deref()
        .unwrap_or("Artist");

    let inner = column![
        container(artist_avatar_widget(handle, 96.0, accent))
            .width(Length::Fill)
            .center_x(Length::Fill),
        column![
            row![
                text(truncate_text(&artist.name, 16))
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
                source_pill(artist.source, accent),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            text(truncate_text(sub, 20))
                .size(10)
                .color(TEXT_MUTED),
        ]
        .spacing(3)
        .width(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fixed(132.0));

    button(inner)
        .padding(10)
        .style(track_row_button_style(false, accent))
        .on_press(Message::OpenArtist(artist.clone()))
        .into()
}

pub fn remote_playlist_card<'a>(
    props: &BrowseViewProps<'a>,
    playlist: &'a RemotePlaylistSummary,
) -> Element<'a, Message> {
    let accent = props.accent;
    let handle = playlist
        .cover_url
        .as_deref()
        .and_then(|u| props.art_handles.get(u));
    let sub = match (&playlist.owner, playlist.track_count) {
        (Some(owner), Some(cnt)) => format!("{owner} • {cnt} songs"),
        (Some(owner), None) => owner.clone(),
        (None, Some(cnt)) => format!("{cnt} songs"),
        (None, None) => playlist.source.display_name().to_string(),
    };

    let inner = column![
        playlist_cover_widget(handle, 116.0, 12.0, accent),
        column![
            row![
                text(truncate_text(&playlist.name, 15))
                    .size(12)
                    .font(FONT_SEMIBOLD)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
                source_pill(playlist.source, accent),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            text(truncate_text(&sub, 22))
                .size(10)
                .color(TEXT_MUTED),
        ]
        .spacing(3)
        .width(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fixed(136.0));

    button(inner)
        .padding(10)
        .style(track_row_button_style(false, accent))
        .on_press(Message::OpenRemotePlaylist(playlist.clone()))
        .into()
}

pub fn album_card<'a>(
    props: &BrowseViewProps<'a>,
    album: &'a AlbumSummary,
) -> Element<'a, Message> {
    let accent = props.accent;
    let handle = album
        .cover_url
        .as_deref()
        .and_then(|u| props.art_handles.get(u));
    let sub = match (&album.year, &album.album_type) {
        (Some(y), Some(t)) => format!("{y} • {t}"),
        (Some(y), None) => y.clone(),
        (None, Some(t)) => t.clone(),
        (None, None) => "Album".to_string(),
    };

    let inner = column![
        album_cover_widget(handle, 116.0, 12.0, accent),
        column![
            text(truncate_text(&album.title, 18))
                .size(12)
                .font(FONT_SEMIBOLD)
                .color(TEXT_PRIMARY),
            text(truncate_text(&sub, 20))
                .size(10)
                .color(TEXT_MUTED),
        ]
        .spacing(3)
        .width(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fixed(136.0));

    button(inner)
        .padding(10)
        .style(track_row_button_style(false, accent))
        .on_press(Message::OpenAlbum(album.clone()))
        .into()
}

fn card_grid<'a>(cards: Vec<Element<'a, Message>>, per_row: usize) -> Element<'a, Message> {
    let per_row = per_row.max(1);
    let mut rows_col = column![].spacing(10).width(Length::Fill);
    let mut iter = cards.into_iter();
    loop {
        let mut r = row![].spacing(10);
        let mut count = 0usize;
        for card in iter.by_ref().take(per_row) {
            r = r.push(card);
            count += 1;
        }
        if count == 0 {
            break;
        }
        rows_col = rows_col.push(r);
    }
    rows_col.into()
}

fn catalog_grid_cols(props: &BrowseViewProps<'_>) -> usize {
    responsive_grid_columns(
        props.window_width,
        responsive_sidebar_width(props.window_width),
        136.0,
        10.0,
    )
}

pub fn build_artists_strip<'a>(
    props: &BrowseViewProps<'a>,
    artists: &[&'a ArtistSummary],
) -> Element<'a, Message> {
    let mut strip = row![].spacing(10);
    for artist in artists.iter().take(12) {
        strip = strip.push(artist_card(props, artist));
    }
    let header = row![
        text("Artists")
            .size(14)
            .font(FONT_BOLD)
            .color(TEXT_PRIMARY)
            .width(Length::Fill),
        if artists.len() > 5 {
            Element::from(
                button(text("See all").size(11).font(FONT_MEDIUM).color(props.accent.light))
                    .padding([4, 10])
                    .style(glass_button_style(false, props.accent))
                    .on_press(Message::SearchTabChanged(SearchTab::Artists)),
            )
        } else {
            Element::from(space().width(0))
        },
    ]
    .align_y(Alignment::Center);

    container(
        column![
            header,
            scrollable(strip)
                .direction(scrollable::Direction::Horizontal(hidden_scrollbar()))
                .width(Length::Fill)
                .style(thin_scrollable_style),
        ]
        .spacing(10),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(glass_card_style)
    .into()
}

pub fn build_playlists_strip<'a>(
    props: &BrowseViewProps<'a>,
    playlists: &[&'a RemotePlaylistSummary],
) -> Element<'a, Message> {
    let mut strip = row![].spacing(10);
    for pl in playlists.iter().take(12) {
        strip = strip.push(remote_playlist_card(props, pl));
    }
    let header = row![
        text("Playlists")
            .size(14)
            .font(FONT_BOLD)
            .color(TEXT_PRIMARY)
            .width(Length::Fill),
        if playlists.len() > 5 {
            Element::from(
                button(text("See all").size(11).font(FONT_MEDIUM).color(props.accent.light))
                    .padding([4, 10])
                    .style(glass_button_style(false, props.accent))
                    .on_press(Message::SearchTabChanged(SearchTab::Playlists)),
            )
        } else {
            Element::from(space().width(0))
        },
    ]
    .align_y(Alignment::Center);

    container(
        column![
            header,
            scrollable(strip)
                .direction(scrollable::Direction::Horizontal(hidden_scrollbar()))
                .width(Length::Fill)
                .style(thin_scrollable_style),
        ]
        .spacing(10),
    )
    .padding([12, 14])
    .width(Length::Fill)
    .style(glass_card_style)
    .into()
}

pub fn build_artists_grid_view<'a>(
    props: &BrowseViewProps<'a>,
    artists: &[&'a ArtistSummary],
) -> Element<'a, Message> {
    if artists.is_empty() {
        return container(
            text(if props.searching_remote {
                "Searching artists..."
            } else {
                "No artists found yet — start typing to search Spotify & YouTube Music."
            })
            .size(13)
            .color(TEXT_SECONDARY),
        )
        .padding(32)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into();
    }
    let cards: Vec<Element<'a, Message>> = artists.iter().map(|a| artist_card(props, a)).collect();
    card_grid(cards, catalog_grid_cols(props))
}

pub fn build_playlists_grid_view<'a>(
    props: &BrowseViewProps<'a>,
    playlists: &[&'a RemotePlaylistSummary],
) -> Element<'a, Message> {
    if playlists.is_empty() {
        return container(
            text(if props.searching_remote {
                "Searching playlists..."
            } else {
                "No playlists found yet — start typing to search Spotify & YouTube Music."
            })
            .size(13)
            .color(TEXT_SECONDARY),
        )
        .padding(32)
        .width(Length::Fill)
        .center_x(Length::Fill)
        .into();
    }
    let cards: Vec<Element<'a, Message>> = playlists
        .iter()
        .map(|p| remote_playlist_card(props, p))
        .collect();
    card_grid(cards, catalog_grid_cols(props))
}

fn back_header_row<'a>(
    accent: AccentTheme,
    label: &'static str,
    status_banner: Option<&'a str>,
) -> Element<'a, Message> {
    let mut col = column![
        row![
            button(
                row![
                    icon_back(14.0, ICON_LIGHT),
                    text("Back").size(12).font(FONT_MEDIUM).color(TEXT_PRIMARY),
                ]
                .spacing(5)
                .align_y(Alignment::Center),
            )
            .padding([6, 12])
            .style(glass_button_style(false, accent))
            .on_press(Message::NavigateBack),
            text(label)
                .size(11)
                .font(FONT_SEMIBOLD)
                .color(TEXT_MUTED),
            space().width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center),
    ]
    .spacing(8);

    if let Some(toast) = status_banner {
        col = col.push(
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
            .padding([9, 14])
            .style(glass_elevated_style),
        );
    }

    col.into()
}

pub fn view_artist_profile<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;
    let fallback_name = match props.nav {
        ActiveNav::ArtistProfile { name, .. } => name.as_str(),
        _ => "Artist",
    };

    let mut feed = column![back_header_row(accent, "ARTIST PROFILE", props.status_banner)]
        .spacing(14)
        .width(Length::Fill);

    if props.loading_artist {
        feed = feed.push(
            container(
                column![
                    text(format!("Loading {fallback_name}..."))
                        .size(16)
                        .font(FONT_SEMIBOLD)
                        .color(TEXT_PRIMARY),
                    text("Fetching popular songs, albums, and singles...")
                        .size(12)
                        .color(TEXT_SECONDARY),
                ]
                .spacing(6),
            )
            .padding(24)
            .width(Length::Fill)
            .style(glass_card_style),
        );
    } else if let Some(err) = props.artist_error {
        feed = feed.push(
            container(
                column![
                    text(format!("Could not load {fallback_name}"))
                        .size(15)
                        .font(FONT_BOLD)
                        .color(TEXT_PRIMARY),
                    text(err).size(12).color(Color::from_rgb8(252, 165, 165)),
                ]
                .spacing(6),
            )
            .padding(22)
            .width(Length::Fill)
            .style(glass_card_style),
        );
    } else if let Some(profile) = props.active_artist_profile {
        let avatar_handle = profile
            .avatar_url
            .as_deref()
            .and_then(|u| props.art_handles.get(u));

        let mut meta_col = column![
            row![
                text("ARTIST")
                    .size(10)
                    .font(FONT_SEMIBOLD)
                    .color(accent.light),
                source_pill(profile.source, accent),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            text(profile.name.as_str())
                .size(26)
                .font(FONT_BOLD)
                .color(TEXT_PRIMARY),
        ]
        .spacing(4)
        .width(Length::Fill);

        if let Some(sub) = &profile.subscriber_label {
            meta_col = meta_col.push(text(sub.as_str()).size(12).color(TEXT_SECONDARY));
        }
        if let Some(desc) = profile.description.as_deref().filter(|s| !s.trim().is_empty()) {
            meta_col = meta_col.push(
                text(truncate_text(desc, 160))
                    .size(11)
                    .color(TEXT_MUTED),
            );
        }

        let mut hero_actions = row![].spacing(8).align_y(Alignment::Center);
        if !profile.top_tracks.is_empty() {
            hero_actions = hero_actions.push(
                button(
                    row![
                        icon_play(13.0, accent.on_primary),
                        text("Play Popular")
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(accent.on_primary),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 16])
                .style(primary_pill_button_style(accent))
                .on_press(Message::PlayTrackList {
                    tracks: profile.top_tracks.clone(),
                    start_index: 0,
                }),
            );
        }

        feed = feed.push(
            container(
                row![
                    artist_avatar_widget(avatar_handle, 88.0, accent),
                    meta_col,
                    hero_actions,
                ]
                .spacing(18)
                .align_y(Alignment::Center),
            )
            .padding([18, 22])
            .width(Length::Fill)
            .style(hero_banner_style(accent)),
        );

        // Popular Songs section
        if !profile.top_tracks.is_empty() {
            let show_count = if props.artist_tracks_expanded {
                profile.top_tracks.len()
            } else {
                5.min(profile.top_tracks.len())
            };
            let mut header = row![
                text("Popular Songs")
                    .size(15)
                    .font(FONT_BOLD)
                    .color(TEXT_PRIMARY)
                    .width(Length::Fill),
            ]
            .align_y(Alignment::Center);
            if profile.top_tracks.len() > 5 {
                header = header.push(
                    button(
                        text(if props.artist_tracks_expanded {
                            "Show less".to_string()
                        } else {
                            format!("Show all ({})", profile.top_tracks.len())
                        })
                        .size(11)
                        .font(FONT_MEDIUM)
                        .color(TEXT_PRIMARY),
                    )
                    .padding([5, 12])
                    .style(glass_button_style(false, accent))
                    .on_press(Message::ToggleExpandArtistTracks),
                );
            }

            let mut tracks_col = column![].spacing(3);
            for (idx, track) in profile.top_tracks.iter().take(show_count).enumerate() {
                tracks_col = tracks_col.push(track_row(props, track, idx, idx, &profile.top_tracks));
            }
            feed = feed.push(
                container(column![header, tracks_col].spacing(10))
                    .padding([14, 14])
                    .width(Length::Fill)
                    .style(glass_card_style),
            );
        }

        // Albums section
        if !profile.albums.is_empty() {
            let cards: Vec<Element<'a, Message>> = profile
                .albums
                .iter()
                .map(|alb| album_card(props, alb))
                .collect();
            feed = feed.push(
                container(
                    column![
                        text("Albums")
                            .size(15)
                            .font(FONT_BOLD)
                            .color(TEXT_PRIMARY),
                        card_grid(cards, catalog_grid_cols(props)),
                    ]
                    .spacing(12),
                )
                .padding([14, 14])
                .width(Length::Fill)
                .style(glass_card_style),
            );
        }

        // Singles & EPs section
        if !profile.singles.is_empty() {
            let cards: Vec<Element<'a, Message>> = profile
                .singles
                .iter()
                .map(|alb| album_card(props, alb))
                .collect();
            feed = feed.push(
                container(
                    column![
                        text("Singles & EPs")
                            .size(15)
                            .font(FONT_BOLD)
                            .color(TEXT_PRIMARY),
                        card_grid(cards, catalog_grid_cols(props)),
                    ]
                    .spacing(12),
                )
                .padding([14, 14])
                .width(Length::Fill)
                .style(glass_card_style),
            );
        }

        // Related / Artist Playlists section
        if !profile.playlists.is_empty() {
            let refs: Vec<&RemotePlaylistSummary> = profile.playlists.iter().collect();
            feed = feed.push(build_playlists_strip(props, &refs));
        }
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

pub fn view_album_detail<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;
    let fallback_title = match props.nav {
        ActiveNav::AlbumDetail { title, .. } => title.as_str(),
        _ => "Album",
    };

    let mut feed = column![back_header_row(accent, "ALBUM", props.status_banner)]
        .spacing(14)
        .width(Length::Fill);

    if props.loading_album {
        feed = feed.push(
            container(
                text(format!("Loading album \"{fallback_title}\"..."))
                    .size(14)
                    .font(FONT_MEDIUM)
                    .color(TEXT_SECONDARY),
            )
            .padding(24)
            .width(Length::Fill)
            .style(glass_card_style),
        );
    } else if let Some(err) = props.album_error {
        feed = feed.push(
            container(
                column![
                    text(format!("Could not load \"{fallback_title}\""))
                        .size(15)
                        .font(FONT_BOLD)
                        .color(TEXT_PRIMARY),
                    text(err).size(12).color(Color::from_rgb8(252, 165, 165)),
                ]
                .spacing(6),
            )
            .padding(22)
            .width(Length::Fill)
            .style(glass_card_style),
        );
    } else if let Some(album) = props.active_album_detail {
        let cover_handle = album
            .cover_url
            .as_deref()
            .and_then(|u| props.art_handles.get(u));
        let total_mins: u64 = album
            .tracks
            .iter()
            .map(|t| t.duration_ms / 1000)
            .sum::<u64>()
            / 60;
        let meta_line = match &album.year {
            Some(y) => format!("{y} • {} songs • {total_mins} min", album.tracks.len()),
            None => format!("{} songs • {total_mins} min", album.tracks.len()),
        };

        let artist_btn: Element<'a, Message> = if !album.artist_name.trim().is_empty() {
            let msg = if let Some(aid) = &album.artist_id {
                Message::OpenArtist(ArtistSummary {
                    id: aid.clone(),
                    source: album.source,
                    name: album.artist_name.clone(),
                    avatar_url: None,
                    subtitle: None,
                })
            } else {
                let hint = match album.source {
                    CatalogSource::Spotify => TrackSource::Spotify {
                        id: album.id.clone(),
                    },
                    CatalogSource::YouTubeMusic => TrackSource::YouTubeMusic {
                        video_id: album.id.clone(),
                    },
                };
                Message::OpenArtistByName {
                    name: album.artist_name.clone(),
                    source_hint: hint,
                }
            };
            button(
                text(album.artist_name.as_str())
                    .size(13)
                    .font(FONT_SEMIBOLD)
                    .color(accent.light),
            )
            .padding([2, 0])
            .style(crate::widgets::artist_link_button_style(accent))
            .on_press(msg)
            .into()
        } else {
            space().height(0).into()
        };

        let play_btn: Element<'a, Message> = if !album.tracks.is_empty() {
            button(
                row![
                    icon_play(13.0, accent.on_primary),
                    text("Play All")
                        .size(12)
                        .font(FONT_SEMIBOLD)
                        .color(accent.on_primary),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 16])
            .style(primary_pill_button_style(accent))
            .on_press(Message::PlayTrackList {
                tracks: album.tracks.clone(),
                start_index: 0,
            })
            .into()
        } else {
            space().width(0).into()
        };

        feed = feed.push(
            container(
                row![
                    album_cover_widget(cover_handle, 92.0, 14.0, accent),
                    column![
                        row![
                            text("ALBUM")
                                .size(10)
                                .font(FONT_SEMIBOLD)
                                .color(accent.light),
                            source_pill(album.source, accent),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        text(album.title.as_str())
                            .size(24)
                            .font(FONT_BOLD)
                            .color(TEXT_PRIMARY),
                        artist_btn,
                        text(meta_line).size(12).color(TEXT_SECONDARY),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                    play_btn,
                ]
                .spacing(18)
                .align_y(Alignment::Center),
            )
            .padding([18, 22])
            .width(Length::Fill)
            .style(hero_banner_style(accent)),
        );

        let mut list = column![].spacing(3);
        for (idx, track) in album.tracks.iter().enumerate() {
            list = list.push(track_row(props, track, idx, idx, &album.tracks));
        }
        feed = feed.push(
            container(list)
                .padding([10, 10])
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

pub fn view_remote_playlist<'a>(props: &BrowseViewProps<'a>) -> Element<'a, Message> {
    let accent = props.accent;
    let fallback_name = match props.nav {
        ActiveNav::RemotePlaylist { name, .. } => name.as_str(),
        _ => "Playlist",
    };

    let mut feed = column![back_header_row(
        accent,
        "PLAYLIST PREVIEW",
        props.status_banner
    )]
    .spacing(14)
    .width(Length::Fill);

    if props.loading_remote_playlist {
        feed = feed.push(
            container(
                column![
                    text(format!("Loading playlist \"{fallback_name}\"..."))
                        .size(15)
                        .font(FONT_SEMIBOLD)
                        .color(TEXT_PRIMARY),
                    text("Fetching tracklist for preview...")
                        .size(12)
                        .color(TEXT_SECONDARY),
                ]
                .spacing(6),
            )
            .padding(24)
            .width(Length::Fill)
            .style(glass_card_style),
        );
    } else if let Some(err) = props.remote_playlist_error {
        feed = feed.push(
            container(
                column![
                    text(format!("Could not load \"{fallback_name}\""))
                        .size(15)
                        .font(FONT_BOLD)
                        .color(TEXT_PRIMARY),
                    text(err).size(12).color(Color::from_rgb8(252, 165, 165)),
                ]
                .spacing(6),
            )
            .padding(22)
            .width(Length::Fill)
            .style(glass_card_style),
        );
    } else if let Some(pl) = props.active_remote_playlist {
        let cover_handle = pl
            .cover_url
            .as_deref()
            .and_then(|u| props.art_handles.get(u));
        let total_mins: u64 = pl
            .tracks
            .iter()
            .map(|t| t.duration_ms / 1000)
            .sum::<u64>()
            / 60;
        let owner_part = pl
            .owner
            .as_deref()
            .map(|o| format!("By {o} • "))
            .unwrap_or_default();
        let meta_line = format!("{owner_part}{} songs • {total_mins} min", pl.tracks.len());

        let mut actions = row![].spacing(8).align_y(Alignment::Center);
        if !pl.tracks.is_empty() {
            actions = actions.push(
                button(
                    row![
                        icon_play(13.0, accent.on_primary),
                        text("Play All")
                            .size(12)
                            .font(FONT_SEMIBOLD)
                            .color(accent.on_primary),
                    ]
                    .spacing(6)
                    .align_y(Alignment::Center),
                )
                .padding([8, 16])
                .style(primary_pill_button_style(accent))
                .on_press(Message::PlayTrackList {
                    tracks: pl.tracks.clone(),
                    start_index: 0,
                }),
            );

            let save_label = if props.saving_remote_playlist {
                "Saving..."
            } else if props.remote_playlist_is_saved {
                "Update in Library"
            } else {
                "Save to Library"
            };
            let save_icon = if props.remote_playlist_is_saved {
                icon_check(13.0, accent.light)
            } else {
                icon_plus(13.0, ICON_LIGHT)
            };
            let save_btn = button(
                row![
                    save_icon,
                    text(save_label)
                        .size(12)
                        .font(FONT_MEDIUM)
                        .color(TEXT_PRIMARY),
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
            .padding([8, 14])
            .style(glass_button_style(props.remote_playlist_is_saved, accent));
            let save_btn = if props.saving_remote_playlist {
                save_btn
            } else {
                save_btn.on_press(Message::SaveRemotePlaylistToLibrary)
            };
            actions = actions.push(save_btn);
        }

        feed = feed.push(
            container(
                row![
                    playlist_cover_widget(cover_handle, 92.0, 14.0, accent),
                    column![
                        row![
                            text("REMOTE PLAYLIST")
                                .size(10)
                                .font(FONT_SEMIBOLD)
                                .color(accent.light),
                            source_pill(pl.source, accent),
                        ]
                        .spacing(8)
                        .align_y(Alignment::Center),
                        text(pl.name.as_str())
                            .size(24)
                            .font(FONT_BOLD)
                            .color(TEXT_PRIMARY),
                        text(meta_line).size(12).color(TEXT_SECONDARY),
                    ]
                    .spacing(4)
                    .width(Length::Fill),
                    actions,
                ]
                .spacing(18)
                .align_y(Alignment::Center),
            )
            .padding([18, 22])
            .width(Length::Fill)
            .style(hero_banner_style(accent)),
        );

        if pl.is_truncated_embed {
            feed = feed.push(
                container(
                    text("Showing the first 100 tracks via Spotify public embed (Spotify Development Mode caps third-party playlist API access).")
                        .size(11)
                        .color(TEXT_SECONDARY),
                )
                .padding([8, 14])
                .width(Length::Fill)
                .style(glass_elevated_style),
            );
        }

        let show_album = show_track_album_column(props.window_width);
        let show_source = show_track_source_column(props.window_width);
        let mut hrow = row![
            text("#")
                .size(11)
                .color(TEXT_MUTED)
                .width(Length::Fixed(30.0)),
            space().width(Length::Fixed(40.0)),
            text("TITLE")
                .size(11)
                .color(TEXT_MUTED)
                .width(Length::FillPortion(4)),
        ]
        .spacing(12)
        .padding([4, 12])
        .align_y(Alignment::Center);
        if show_album {
            hrow = hrow.push(
                text("ALBUM")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::FillPortion(3)),
            );
        }
        if show_source {
            hrow = hrow.push(
                text("SOURCE")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(72.0)),
            );
        }
        hrow = hrow
            .push(
                text("TIME")
                    .size(11)
                    .color(TEXT_MUTED)
                    .width(Length::Fixed(48.0)),
            )
            .push(space().width(Length::Fixed(64.0)));

        let header_row = column![
            hrow,
            container(space().height(1))
                .width(Length::Fill)
                .style(|_| container::Style {
                    background: Some(Background::Color(GLASS_SUBTLE_BORDER)),
                    ..Default::default()
                }),
        ]
        .spacing(4);

        let mut list = column![header_row].spacing(3);
        for (idx, track) in pl.tracks.iter().enumerate() {
            list = list.push(track_row(props, track, idx, idx, &pl.tracks));
        }
        feed = feed.push(
            container(list)
                .padding([10, 10])
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
