import React, { useState } from 'react';
import {
  ActivityIndicator,
  Image,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useRouter } from 'expo-router';
import { LumeCore } from '../../modules/lume-core';
import { AmbientBackdrop } from '../../src/components/AmbientBackdrop';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { ensureFreshSpotifyToken } from '../../src/services/spotifyAuth';
import { LumeTheme } from '../../src/theme';
import type { ArtistSummary, RemotePlaylistSummary, Track } from '../../src/types';

type SourceFilter = 'all' | 'youtube' | 'spotify' | 'local';
type CategoryFilter = 'all' | 'songs' | 'artists' | 'playlists';

export default function SearchScreen() {
  const router = useRouter();
  const {
    currentTrack,
    palette,
    offlineUids,
    downloadingUids,
    playTrack,
    downloadOffline,
    toggleLikeTrack,
  } = usePlayer();

  const [query, setQuery] = useState('');
  const [sourceFilter, setSourceFilter] = useState<SourceFilter>('all');
  const [categoryFilter, setCategoryFilter] = useState<CategoryFilter>('all');

  const [tracks, setTracks] = useState<Track[]>([]);
  const [artists, setArtists] = useState<ArtistSummary[]>([]);
  const [playlists, setPlaylists] = useState<RemotePlaylistSummary[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runSearch = async (overrideSource?: SourceFilter) => {
    const trimmed = query.trim();
    if (!trimmed) return;
    const activeSource = overrideSource ?? sourceFilter;
    setSearching(true);
    setError(null);
    try {
      const nextTracks: Track[] = [];
      const nextArtists: ArtistSummary[] = [];
      const nextPlaylists: RemotePlaylistSummary[] = [];

      if (activeSource === 'all' || activeSource === 'youtube') {
        const ytm = await LumeCore.searchYtm(trimmed, 25);
        nextTracks.push(...ytm.tracks);
        nextArtists.push(...ytm.artists);
        nextPlaylists.push(...ytm.playlists);
      }

      if (activeSource === 'all' || activeSource === 'local') {
        const local = await LumeCore.searchLocalTracks(trimmed);
        nextTracks.push(...local);
      }

      if (activeSource === 'spotify' || activeSource === 'all') {
        try {
          const token = await ensureFreshSpotifyToken();
          const sp = await LumeCore.searchSpotifyCatalog(token.access_token, trimmed, 15);
          nextTracks.push(...sp.tracks);
          nextArtists.push(...sp.artists);
          nextPlaylists.push(...sp.playlists);
        } catch (spErr) {
          if (activeSource === 'spotify') {
            throw spErr;
          }
        }
      }

      setTracks(nextTracks);
      setArtists(nextArtists);
      setPlaylists(nextPlaylists);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSearching(false);
    }
  };

  const showSongs = categoryFilter === 'all' || categoryFilter === 'songs';
  const showArtists = categoryFilter === 'all' || categoryFilter === 'artists';
  const showPlaylists = categoryFilter === 'all' || categoryFilter === 'playlists';

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.container} edges={['top']}>
        <View style={styles.header}>
          <Text style={styles.title}>Explore</Text>
          <View style={styles.searchBar}>
            <Ionicons name="search" size={18} color={LumeTheme.textSecondary} />
            <TextInput
              value={query}
              onChangeText={setQuery}
              onSubmitEditing={() => void runSearch()}
              placeholder="Cari lagu, artis, atau playlist..."
              placeholderTextColor={LumeTheme.textMuted}
              returnKeyType="search"
              autoCorrect={false}
              style={styles.input}
            />
            {query.length > 0 && (
              <Pressable onPress={() => setQuery('')} hitSlop={8}>
                <Ionicons name="close-circle" size={18} color={LumeTheme.textMuted} />
              </Pressable>
            )}
          </View>

          <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.chipRow}>
            {(['all', 'youtube', 'spotify', 'local'] as SourceFilter[]).map((src) => {
              const active = sourceFilter === src;
              const label =
                src === 'all'
                  ? 'All'
                  : src === 'youtube'
                    ? 'YouTube'
                    : src === 'spotify'
                      ? 'Spotify'
                      : 'Local';
              return (
                <Pressable
                  key={src}
                  onPress={() => {
                    setSourceFilter(src);
                    if (query.trim()) void runSearch(src);
                  }}
                  style={[
                    styles.chip,
                    active && { backgroundColor: palette.accent, borderColor: palette.accent },
                  ]}
                >
                  <Text style={[styles.chipText, active && styles.chipTextActive]}>{label}</Text>
                </Pressable>
              );
            })}
          </ScrollView>

          <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.chipRow}>
            {(['all', 'songs', 'artists', 'playlists'] as CategoryFilter[]).map((cat) => {
              const active = categoryFilter === cat;
              const label =
                cat === 'all'
                  ? 'Semua'
                  : cat === 'songs'
                    ? `Songs (${tracks.length})`
                    : cat === 'artists'
                      ? `Artists (${artists.length})`
                      : `Playlists (${playlists.length})`;
              return (
                <Pressable
                  key={cat}
                  onPress={() => setCategoryFilter(cat)}
                  style={[
                    styles.subChip,
                    active && styles.subChipActive,
                  ]}
                >
                  <Text style={[styles.subChipText, active && styles.subChipTextActive]}>
                    {label}
                  </Text>
                </Pressable>
              );
            })}
          </ScrollView>
        </View>

        {error && <Text style={styles.error}>{error}</Text>}

        {searching ? (
          <ActivityIndicator size="large" color={palette.accent} style={{ marginTop: 40 }} />
        ) : (
          <ScrollView contentContainerStyle={styles.listContent}>
            {tracks.length === 0 && artists.length === 0 && playlists.length === 0 && (
              <View style={styles.empty}>
                <Ionicons name="compass-outline" size={40} color={LumeTheme.textMuted} />
                <Text style={styles.emptyText}>
                  Cari lagu, profil artis, album, atau playlist dari YouTube Music, Spotify,
                  maupun file lokal HP kamu.
                </Text>
              </View>
            )}

            {showArtists && artists.length > 0 && (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Artists</Text>
                <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.hRow}>
                  {artists.map((artist) => (
                    <Pressable
                      key={`${artist.source}-${artist.id}`}
                      onPress={() => {
                        if (artist.source === 'YouTubeMusic') {
                          router.push({ pathname: '/artist/[id]', params: { id: artist.id } });
                        }
                      }}
                      style={styles.artistCard}
                    >
                      {artist.avatar_url ? (
                        <Image source={{ uri: artist.avatar_url }} style={styles.artistAvatar} />
                      ) : (
                        <View style={[styles.artistAvatar, styles.artFallback]}>
                          <Ionicons name="person" size={28} color={palette.accent} />
                        </View>
                      )}
                      <Text style={styles.cardName} numberOfLines={1}>
                        {artist.name}
                      </Text>
                      {artist.subtitle && (
                        <Text style={styles.cardSub} numberOfLines={1}>
                          {artist.subtitle}
                        </Text>
                      )}
                    </Pressable>
                  ))}
                </ScrollView>
              </View>
            )}

            {showSongs && tracks.length > 0 && (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Songs</Text>
                {tracks.map((item) => (
                  <TrackRow
                    key={item.uid}
                    track={item}
                    isActive={currentTrack?.uid === item.uid}
                    isOffline={offlineUids.has(item.uid)}
                    isDownloading={downloadingUids.has(item.uid)}
                    onPress={() => void playTrack(item, tracks)}
                    onDownloadPress={() => void downloadOffline(item)}
                    onLikePress={() => void toggleLikeTrack(item)}
                  />
                ))}
              </View>
            )}

            {showPlaylists && playlists.length > 0 && (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Playlists</Text>
                <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.hRow}>
                  {playlists.map((pl) => (
                    <Pressable
                      key={`${pl.source}-${pl.id}`}
                      onPress={() => {
                        if (pl.source === 'YouTubeMusic') {
                          router.push({
                            pathname: '/remote-playlist/[id]',
                            params: { id: pl.id },
                          });
                        }
                      }}
                      style={styles.plCard}
                    >
                      {pl.cover_url ? (
                        <Image source={{ uri: pl.cover_url }} style={styles.plCover} />
                      ) : (
                        <View style={[styles.plCover, styles.artFallback]}>
                          <Ionicons name="musical-notes" size={32} color={palette.accent} />
                        </View>
                      )}
                      <Text style={styles.cardName} numberOfLines={1}>
                        {pl.name}
                      </Text>
                      <Text style={styles.cardSub} numberOfLines={1}>
                        {pl.owner ?? (pl.source === 'Spotify' ? 'Spotify' : 'YouTube Music')}
                      </Text>
                    </Pressable>
                  ))}
                </ScrollView>
              </View>
            )}
          </ScrollView>
        )}
      </SafeAreaView>
    </AmbientBackdrop>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  header: {
    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 10,
    gap: 10,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 27,
    fontWeight: '800',
  },
  searchBar: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: 'rgba(255,255,255,0.09)',
    borderWidth: 1,
    borderColor: LumeTheme.border,
    borderRadius: 14,
    paddingHorizontal: 14,
    height: 44,
    gap: 10,
  },
  input: {
    flex: 1,
    color: LumeTheme.textPrimary,
    fontSize: 15,
  },
  chipRow: {
    gap: 8,
    paddingRight: 16,
  },
  chip: {
    paddingHorizontal: 14,
    paddingVertical: 6,
    borderRadius: 999,
    backgroundColor: 'rgba(255,255,255,0.08)',
    borderWidth: 1,
    borderColor: LumeTheme.border,
  },
  chipText: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    fontWeight: '700',
  },
  chipTextActive: {
    color: '#0A0C12',
  },
  subChip: {
    paddingHorizontal: 12,
    paddingVertical: 5,
    borderRadius: 999,
    backgroundColor: 'rgba(255,255,255,0.06)',
  },
  subChipActive: {
    backgroundColor: 'rgba(255,255,255,0.22)',
  },
  subChipText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontWeight: '600',
  },
  subChipTextActive: {
    color: '#FFFFFF',
    fontWeight: '700',
  },
  error: {
    color: LumeTheme.rose,
    paddingHorizontal: 16,
    marginBottom: 8,
    fontSize: 13,
  },
  listContent: {
    paddingHorizontal: 10,
    paddingBottom: 165,
  },
  section: {
    marginBottom: 20,
  },
  sectionTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 18,
    fontWeight: '700',
    paddingHorizontal: 8,
    marginBottom: 10,
  },
  hRow: {
    gap: 12,
    paddingHorizontal: 8,
  },
  artistCard: {
    width: 112,
    alignItems: 'center',
    gap: 6,
  },
  artistAvatar: {
    width: 96,
    height: 96,
    borderRadius: 48,
  },
  plCard: {
    width: 136,
    gap: 6,
  },
  plCover: {
    width: 136,
    height: 136,
    borderRadius: 14,
  },
  artFallback: {
    backgroundColor: LumeTheme.bgElevated,
    alignItems: 'center',
    justifyContent: 'center',
  },
  cardName: {
    color: LumeTheme.textPrimary,
    fontSize: 13.5,
    fontWeight: '700',
    textAlign: 'center',
  },
  cardSub: {
    color: LumeTheme.textSecondary,
    fontSize: 11.5,
    textAlign: 'center',
  },
  empty: {
    alignItems: 'center',
    paddingHorizontal: 32,
    paddingTop: 56,
    gap: 12,
  },
  emptyText: {
    color: LumeTheme.textSecondary,
    fontSize: 14,
    textAlign: 'center',
    lineHeight: 20,
  },
});
