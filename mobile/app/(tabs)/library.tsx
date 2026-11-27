import React, { useCallback, useEffect, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { LumeCore } from '../../modules/lume-core';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import {
  clearSpotifySession,
  connectSpotifyMobile,
  loadSpotifyClientId,
  loadSpotifyTokenSet,
  syncSpotifyNow,
} from '../../src/services/spotifyAuth';
import { LumeTheme } from '../../src/theme';
import type { Playlist, Track } from '../../src/types';

export default function LibraryScreen() {
  const {
    currentTrack,
    offlineUids,
    downloadingUids,
    playTrack,
    downloadOffline,
    toggleLikeTrack,
  } = usePlayer();

  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [likedTracks, setLikedTracks] = useState<Track[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [selectedTracks, setSelectedTracks] = useState<Track[]>([]);
  const [showLiked, setShowLiked] = useState(false);

  const [clientId, setClientId] = useState('');
  const [spotifyConnected, setSpotifyConnected] = useState(false);
  const [importUrl, setImportUrl] = useState('');
  const [newPlaylistName, setNewPlaylistName] = useState('');
  const [busyMessage, setBusyMessage] = useState<string | null>(null);

  const refreshLibrary = useCallback(async () => {
    const [cid, token] = await Promise.all([
      loadSpotifyClientId(),
      loadSpotifyTokenSet(),
    ]);
    setClientId(cid);
    setSpotifyConnected(token !== null);

    if (!LumeCore.isAvailable()) return;
    const [pls, liked] = await Promise.all([
      LumeCore.listPlaylists(),
      LumeCore.listLikedTracks(),
    ]);
    setPlaylists(pls);
    setLikedTracks(liked);
  }, []);

  useEffect(() => {
    void refreshLibrary();
  }, [refreshLibrary]);

  const openPlaylist = async (pl: Playlist) => {
    const tracks = await LumeCore.getPlaylistTracks(pl.id);
    setSelectedPlaylist(pl);
    setSelectedTracks(tracks);
    setShowLiked(false);
  };

  const handleConnectSpotify = async () => {
    setBusyMessage('Menghubungkan ke Spotify...');
    try {
      await connectSpotifyMobile(clientId);
      setSpotifyConnected(true);
      const summary = await syncSpotifyNow();
      await refreshLibrary();
      Alert.alert(
        'Spotify Tersinkronisasi',
        `Halo ${summary.display_name}! ${summary.liked_count} Liked Songs & ${summary.playlist_count} playlist disinkronkan (auto-fallback YTM saat diputar).`
      );
    } catch (err) {
      Alert.alert('Spotify', err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMessage(null);
    }
  };

  const handleSyncSpotify = async () => {
    setBusyMessage('Menyinkronkan playlist Spotify...');
    try {
      const summary = await syncSpotifyNow();
      await refreshLibrary();
      Alert.alert(
        'Sync Selesai',
        `${summary.liked_count} Liked Songs & ${summary.playlist_count} playlist diperbarui.`
      );
    } catch (err) {
      Alert.alert('Sync Gagal', err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMessage(null);
    }
  };

  const handleImportUrl = async () => {
    const trimmed = importUrl.trim();
    if (!trimmed) return;
    setBusyMessage('Mengimpor playlist Spotify publik...');
    try {
      const pl = await LumeCore.importSpotifyPlaylistUrl(trimmed);
      setImportUrl('');
      await refreshLibrary();
      Alert.alert('Import Berhasil', `Playlist "${pl.name}" (${pl.track_count} lagu) tersimpan.`);
    } catch (err) {
      Alert.alert('Import Gagal', err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMessage(null);
    }
  };

  const handleCreatePlaylist = async () => {
    const trimmed = newPlaylistName.trim();
    if (!trimmed) return;
    await LumeCore.createPlaylist(trimmed);
    setNewPlaylistName('');
    await refreshLibrary();
  };

  const handleDownloadPlaylist = async (tracks: Track[]) => {
    setBusyMessage(`Mengunduh ${tracks.length} lagu ke Offline...`);
    let ok = 0;
    for (const track of tracks) {
      if (offlineUids.has(track.uid)) {
        ok++;
        continue;
      }
      try {
        await downloadOffline(track);
        ok++;
      } catch {
        // Continue downloading remaining tracks.
      }
    }
    setBusyMessage(null);
    Alert.alert('Offline Download', `${ok} / ${tracks.length} lagu siap diputar tanpa sinyal.`);
  };

  if (selectedPlaylist || showLiked) {
    const activeTitle = showLiked ? 'Liked Songs' : selectedPlaylist?.name ?? '';
    const activeTracks = showLiked ? likedTracks : selectedTracks;

    return (
      <SafeAreaView style={styles.container} edges={['top']}>
        <View style={styles.detailHeader}>
          <Pressable
            onPress={() => {
              setSelectedPlaylist(null);
              setShowLiked(false);
            }}
            style={styles.backBtn}
          >
            <Ionicons name="chevron-back" size={22} color={LumeTheme.textPrimary} />
            <Text style={styles.backText}>Library</Text>
          </Pressable>

          {activeTracks.length > 0 && (
            <Pressable
              onPress={() => void handleDownloadPlaylist(activeTracks)}
              style={styles.downloadAllBtn}
            >
              <Ionicons name="cloud-download" size={16} color="#FFFFFF" />
              <Text style={styles.downloadAllText}>Download Semua</Text>
            </Pressable>
          )}
        </View>

        <Text style={styles.detailTitle}>{activeTitle}</Text>
        {busyMessage && (
          <View style={styles.busyRow}>
            <ActivityIndicator size="small" color={LumeTheme.accentBright} />
            <Text style={styles.busyText}>{busyMessage}</Text>
          </View>
        )}

        <ScrollView contentContainerStyle={styles.listContent}>
          {activeTracks.map((track) => (
            <TrackRow
              key={track.uid}
              track={track}
              isActive={currentTrack?.uid === track.uid}
              isOffline={offlineUids.has(track.uid)}
              isDownloading={downloadingUids.has(track.uid)}
              onPress={() => void playTrack(track, activeTracks)}
              onDownloadPress={() => void downloadOffline(track)}
              onLikePress={() => void toggleLikeTrack(track)}
            />
          ))}
        </ScrollView>
      </SafeAreaView>
    );
  }

  return (
    <SafeAreaView style={styles.container} edges={['top']}>
      <ScrollView contentContainerStyle={styles.scrollContent}>
        <Text style={styles.title}>Library</Text>

        {busyMessage && (
          <View style={styles.busyRow}>
            <ActivityIndicator size="small" color={LumeTheme.accentBright} />
            <Text style={styles.busyText}>{busyMessage}</Text>
          </View>
        )}

        <Pressable
          onPress={() => setShowLiked(true)}
          style={styles.likedCard}
        >
          <View style={styles.likedIcon}>
            <Ionicons name="heart" size={22} color="#FFFFFF" />
          </View>
          <View style={{ flex: 1 }}>
            <Text style={styles.cardTitle}>Liked Songs</Text>
            <Text style={styles.cardSub}>{likedTracks.length} lagu tersimpan</Text>
          </View>
          <Ionicons name="chevron-forward" size={18} color={LumeTheme.textMuted} />
        </Pressable>

        <View style={styles.section}>
          <Text style={styles.sectionTitle}>Playlist Kamu</Text>
          <View style={styles.inlineForm}>
            <TextInput
              value={newPlaylistName}
              onChangeText={setNewPlaylistName}
              placeholder="Nama playlist baru..."
              placeholderTextColor={LumeTheme.textMuted}
              style={styles.input}
            />
            <Pressable
              onPress={() => void handleCreatePlaylist()}
              style={styles.primaryBtn}
            >
              <Text style={styles.btnText}>Buat</Text>
            </Pressable>
          </View>

          {playlists.map((pl) => (
            <Pressable
              key={pl.id}
              onPress={() => void openPlaylist(pl)}
              style={styles.playlistRow}
            >
              <View style={styles.plIcon}>
                <Ionicons
                  name="musical-notes"
                  size={18}
                  color={
                    pl.origin === 'SpotifySynced'
                      ? LumeTheme.spotify
                      : LumeTheme.accentBright
                  }
                />
              </View>
              <View style={{ flex: 1 }}>
                <Text style={styles.cardTitle}>{pl.name}</Text>
                <Text style={styles.cardSub}>
                  {pl.track_count} lagu •{' '}
                  {pl.origin === 'SpotifySynced' ? 'Spotify Sync' : 'Lokal / Import'}
                </Text>
              </View>
              <Ionicons name="chevron-forward" size={18} color={LumeTheme.textMuted} />
            </Pressable>
          ))}
        </View>

        <View style={styles.cardPanel}>
          <Text style={styles.panelTitle}>Import Playlist Spotify Publik (Tanpa Login)</Text>
          <View style={styles.inlineForm}>
            <TextInput
              value={importUrl}
              onChangeText={setImportUrl}
              placeholder="https://open.spotify.com/playlist/..."
              placeholderTextColor={LumeTheme.textMuted}
              autoCapitalize="none"
              autoCorrect={false}
              style={styles.input}
            />
            <Pressable
              onPress={() => void handleImportUrl()}
              style={styles.primaryBtn}
            >
              <Text style={styles.btnText}>Import</Text>
            </Pressable>
          </View>
        </View>

        <View style={styles.cardPanel}>
          <Text style={styles.panelTitle}>Spotify Sync (Keychain + Smart YTM Fallback)</Text>
          <Text style={styles.panelDesc}>
            Tambahkan Redirect URI{' '}
            <Text style={styles.mono}>lumemusic://callback</Text> di Spotify
            Developer Dashboard, lalu masukkan Client ID kamu.
          </Text>
          <TextInput
            value={clientId}
            onChangeText={setClientId}
            placeholder="Spotify Client ID"
            placeholderTextColor={LumeTheme.textMuted}
            autoCapitalize="none"
            autoCorrect={false}
            style={[styles.input, { marginBottom: 10 }]}
          />
          <View style={styles.btnRow}>
            <Pressable
              onPress={() => void handleConnectSpotify()}
              style={[styles.primaryBtn, { flex: 1, backgroundColor: LumeTheme.spotify }]}
            >
              <Text style={styles.btnText}>
                {spotifyConnected ? 'Reconnect Spotify' : 'Connect Spotify'}
              </Text>
            </Pressable>
            {spotifyConnected && (
              <>
                <Pressable
                  onPress={() => void handleSyncSpotify()}
                  style={[styles.primaryBtn, { flex: 1 }]}
                >
                  <Text style={styles.btnText}>Sync Now</Text>
                </Pressable>
                <Pressable
                  onPress={async () => {
                    await clearSpotifySession();
                    setSpotifyConnected(false);
                  }}
                  style={styles.dangerBtn}
                >
                  <Ionicons name="log-out-outline" size={18} color={LumeTheme.rose} />
                </Pressable>
              </>
            )}
          </View>
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
  },
  scrollContent: {
    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 160,
    gap: 16,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 28,
    fontWeight: '800',
  },
  detailHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 14,
    paddingTop: 8,
  },
  backBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    paddingVertical: 6,
  },
  backText: {
    color: LumeTheme.textPrimary,
    fontSize: 15,
    fontWeight: '600',
  },
  downloadAllBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    backgroundColor: LumeTheme.accent,
    paddingHorizontal: 12,
    paddingVertical: 8,
    borderRadius: 10,
  },
  downloadAllText: {
    color: '#FFFFFF',
    fontSize: 12.5,
    fontWeight: '700',
  },
  detailTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 24,
    fontWeight: '800',
    paddingHorizontal: 16,
    marginVertical: 10,
  },
  listContent: {
    paddingHorizontal: 10,
    paddingBottom: 160,
  },
  busyRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  busyText: {
    color: LumeTheme.accentBright,
    fontSize: 13,
  },
  likedCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    padding: 14,
    borderRadius: 16,
    backgroundColor: 'rgba(244, 63, 94, 0.14)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.30)',
  },
  likedIcon: {
    width: 44,
    height: 44,
    borderRadius: 12,
    backgroundColor: LumeTheme.rose,
    alignItems: 'center',
    justifyContent: 'center',
  },
  section: {
    gap: 10,
  },
  sectionTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 18,
    fontWeight: '700',
  },
  playlistRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    padding: 12,
    borderRadius: 14,
    backgroundColor: LumeTheme.bgCard,
    borderWidth: 1,
    borderColor: LumeTheme.border,
  },
  plIcon: {
    width: 42,
    height: 42,
    borderRadius: 10,
    backgroundColor: LumeTheme.bgElevated,
    alignItems: 'center',
    justifyContent: 'center',
  },
  cardTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 15,
    fontWeight: '700',
  },
  cardSub: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    marginTop: 2,
  },
  cardPanel: {
    padding: 14,
    borderRadius: 16,
    backgroundColor: LumeTheme.bgCard,
    borderWidth: 1,
    borderColor: LumeTheme.border,
    gap: 10,
  },
  panelTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 15,
    fontWeight: '700',
  },
  panelDesc: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    lineHeight: 18,
  },
  mono: {
    color: LumeTheme.accentBright,
    fontWeight: '700',
  },
  inlineForm: {
    flexDirection: 'row',
    gap: 8,
  },
  input: {
    flex: 1,
    backgroundColor: LumeTheme.bgElevated,
    borderWidth: 1,
    borderColor: LumeTheme.border,
    borderRadius: 12,
    paddingHorizontal: 12,
    height: 42,
    color: LumeTheme.textPrimary,
    fontSize: 14,
  },
  btnRow: {
    flexDirection: 'row',
    gap: 8,
  },
  primaryBtn: {
    backgroundColor: LumeTheme.accent,
    paddingHorizontal: 16,
    height: 42,
    borderRadius: 12,
    alignItems: 'center',
    justifyContent: 'center',
  },
  dangerBtn: {
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    width: 42,
    height: 42,
    borderRadius: 12,
    alignItems: 'center',
    justifyContent: 'center',
  },
  btnText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 13.5,
  },
});
