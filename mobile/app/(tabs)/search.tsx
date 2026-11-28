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

const QUICK_PROMPTS = [
  'Tulus',
  'Bernadya',
  'Mahalini',
  'Hindia',
  'Nadin Amizah',
  'Lofi Chill',
  'Pop Indonesia',
  'Roadtrip Hits',
];

export default function SearchScreen() {
  const router = useRouter();
  const {
    currentTrack,
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

  const runSearch = async (overrideSource?: SourceFilter, overrideQuery?: string) => {
    const trimmed = (overrideQuery ?? query).trim();
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

  const hasResults = tracks.length > 0 || artists.length > 0 || playlists.length > 0;
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
              <Pressable
                onPress={() => {
                  setQuery('');
                  setTracks([]);
                  setArtists([]);
                  setPlaylists([]);
                  setError(null);
                }}
                hitSlop={8}
              >
                <Ionicons name="close-circle" size={18} color={LumeTheme.textMuted} />
              </Pressable>
            )}
          </View>

          <ScrollView
            horizontal
            showsHorizontalScrollIndicator={false}
            contentContainerStyle={styles.chipRow}
          >
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
                  style={[styles.chip, active && styles.chipActive]}
                >
                  <Text style={[styles.chipText, active && styles.chipTextActive]}>{label}</Text>
                </Pressable>
              );
            })}
          </ScrollView>

          <ScrollView
            horizontal
            showsHorizontalScrollIndicator={false}
            contentContainerStyle={styles.chipRow}
          >
            {(['all', 'songs', 'artists', 'playlists'] as CategoryFilter[]).map((cat) => {
              const active = categoryFilter === cat;
              const label =
                cat === 'all'
                  ? 'Semua'
                  : cat === 'songs'
                    ? tracks.length > 0
                      ? `Songs (${tracks.length})`
                      : 'Songs'
                    : cat === 'artists'
                      ? artists.length > 0
                        ? `Artists (${artists.length})`
                        : 'Artists'
                      : playlists.length > 0
                        ? `Playlists (${playlists.length})`
                        : 'Playlists';
              return (
                <Pressable
                  key={cat}
                  onPress={() => setCategoryFilter(cat)}
                  style={[styles.subChip, active && styles.subChipActive]}
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
          <ActivityIndicator size="large" color="#FFFFFF" style={{ marginTop: 48 }} />
        ) : (
          <ScrollView contentContainerStyle={styles.listContent} showsVerticalScrollIndicator={false}>
            {!hasResults && (
              <View style={styles.empty}>
                <View style={styles.emptyIconWrap}>
                  <Ionicons name="compass-outline" size={34} color="#FFFFFF" />
                </View>
                <Text style={styles.emptyHeading}>Jelajahi Musik Favorit</Text>
                <Text style={styles.emptyText}>
                  Cari lagu, artis, album, atau playlist dari YouTube Music, Spotify, dan file lokal HP kamu.
                </Text>

                <View style={styles.promptWrap}>
                  {QUICK_PROMPTS.map((term) => (
                    <Pressable
                      key={term}
                      onPress={() => {
                        setQuery(term);
                        void runSearch(undefined, term);
                      }}
                      style={({ pressed }) => [
                        styles.promptChip,
                        pressed && styles.promptChipPressed,
                      ]}
                    >
                      <Ionicons name="sparkles-outline" size={12} color={LumeTheme.textSecondary} />
                      <Text style={styles.promptText}>{term}</Text>
                    </Pressable>
                  ))}
                </View>
              </View>
            )}

            {showArtists && artists.length > 0 && (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Artists</Text>
                <ScrollView
                  horizontal
                  showsHorizontalScrollIndicator={false}
                  contentContainerStyle={styles.hRow}
                >
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
                          <Ionicons name="person" size={28} color="rgba(255,255,255,0.7)" />
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
                <View style={styles.glassListBox}>
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
              </View>
            )}

            {showPlaylists && playlists.length > 0 && (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Playlists</Text>
                <ScrollView
                  horizontal
                  showsHorizontalScrollIndicator={false}
                  contentContainerStyle={styles.hRow}
                >
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
                          <Ionicons name="musical-notes" size={32} color="rgba(255,255,255,0.7)" />
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
    paddingHorizontal: 18,
    paddingTop: 10,
    paddingBottom: 10,
    gap: 10,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 28,
    fontWeight: '800',
    letterSpacing: -0.6,
  },
  searchBar: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
    borderRadius: 16,
    paddingHorizontal: 14,
    height: 46,
    gap: 10,
  },
  input: {
    flex: 1,
    color: '#FFFFFF',
    fontSize: 15,
  },
  chipRow: {
    gap: 8,
    paddingRight: 18,
  },
  chip: {
    paddingHorizontal: 15,
    paddingVertical: 6.5,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
  },
  chipActive: {
    backgroundColor: '#FFFFFF',
    borderColor: '#FFFFFF',
  },
  chipText: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    fontWeight: '700',
  },
  chipTextActive: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
  },
  subChip: {
    paddingHorizontal: 12,
    paddingVertical: 5.5,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderWidth: 1,
    borderColor: 'transparent',
  },
  subChipActive: {
    backgroundColor: 'rgba(255, 255, 255, 0.20)',
    borderColor: 'rgba(255, 255, 255, 0.28)',
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
    color: '#FECDD3',
    paddingHorizontal: 18,
    marginBottom: 8,
    fontSize: 13,
  },
  listContent: {
    paddingBottom: 170,
  },
  section: {
    marginBottom: 22,
  },
  sectionTitle: {
    color: '#FFFFFF',
    fontSize: 18,
    fontWeight: '800',
    paddingHorizontal: 18,
    marginBottom: 10,
    letterSpacing: -0.2,
  },
  glassListBox: {
    marginHorizontal: 14,
    paddingVertical: 6,
    paddingHorizontal: 4,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.05)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.10)',
  },
  hRow: {
    gap: 12,
    paddingHorizontal: 18,
  },
  artistCard: {
    width: 112,
    alignItems: 'center',
    gap: 6,
    padding: 8,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.11)',
  },
  artistAvatar: {
    width: 88,
    height: 88,
    borderRadius: 44,
  },
  plCard: {
    width: 142,
    gap: 6,
    padding: 8,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.11)',
  },
  plCover: {
    width: 124,
    height: 124,
    borderRadius: 12,
  },
  artFallback: {
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  cardName: {
    color: '#FFFFFF',
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
    paddingHorizontal: 28,
    paddingTop: 34,
    gap: 10,
  },
  emptyIconWrap: {
    width: 64,
    height: 64,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 4,
  },
  emptyHeading: {
    color: '#FFFFFF',
    fontSize: 18,
    fontWeight: '800',
  },
  emptyText: {
    color: LumeTheme.textSecondary,
    fontSize: 13.5,
    textAlign: 'center',
    lineHeight: 20,
  },
  promptWrap: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    justifyContent: 'center',
    gap: 8,
    marginTop: 14,
  },
  promptChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    paddingHorizontal: 13,
    paddingVertical: 8,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.15)',
  },
  promptChipPressed: {
    backgroundColor: 'rgba(255, 255, 255, 0.18)',
  },
  promptText: {
    color: '#FFFFFF',
    fontSize: 12.5,
    fontWeight: '600',
  },
});
