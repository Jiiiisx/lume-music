import React, { useCallback, useMemo, useState } from 'react';
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
import * as DocumentPicker from 'expo-document-picker';
import { File as ExpoFile } from 'expo-file-system';
import { useFocusEffect, useRouter } from 'expo-router';
import { LumeCore } from '../../modules/lume-core';
import { AmbientBackdrop } from '../../src/components/AmbientBackdrop';
import { BatchDownloadBanner } from '../../src/components/BatchDownloadBanner';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { LumeTheme } from '../../src/theme';
import {
  isPlaylistEditable,
  type Playlist,
  playlistOriginBadge,
  type Track,
  trackArtistDisplay,
} from '../../src/types';

type SortMode = 'custom' | 'title' | 'artist' | 'duration';

export default function LibraryScreen() {
  const router = useRouter();
  const {
    currentTrack,
    palette,
    likedTracks,
    playlists,
    offlineUids,
    downloadingUids,
    playTrack,
    downloadOffline,
    downloadPlaylistOffline,
    toggleLikeTrack,
    refreshLibraryState,
  } = usePlayer();

  const [localTracks, setLocalTracks] = useState<Track[]>([]);
  const [selectedPlaylist, setSelectedPlaylist] = useState<Playlist | null>(null);
  const [selectedTracks, setSelectedTracks] = useState<Track[]>([]);
  const [activeView, setActiveView] = useState<'main' | 'liked' | 'local' | 'playlist'>('main');
  const [sortMode, setSortMode] = useState<SortMode>('custom');

  const [importUrl, setImportUrl] = useState('');
  const [newPlaylistName, setNewPlaylistName] = useState('');
  const [renameText, setRenameText] = useState('');
  const [renaming, setRenaming] = useState(false);
  const [busyMessage, setBusyMessage] = useState<string | null>(null);

  const loadLibrary = useCallback(async () => {
    await refreshLibraryState();
    if (!LumeCore.isAvailable()) return;
    try {
      const local = await LumeCore.listLocalTracks();
      setLocalTracks(local);
    } catch {
      // Non-fatal.
    }
  }, [refreshLibraryState]);

  useFocusEffect(
    useCallback(() => {
      void loadLibrary();
    }, [loadLibrary])
  );

  const openPlaylist = async (pl: Playlist) => {
    const tracks = await LumeCore.getPlaylistTracks(pl.id);
    setSelectedPlaylist(pl);
    setSelectedTracks(tracks);
    setRenameText(pl.name);
    setRenaming(false);
    setSortMode('custom');
    setActiveView('playlist');
  };

  const handleImportUrl = async () => {
    const trimmed = importUrl.trim();
    if (!trimmed) return;
    setBusyMessage('Mengimpor playlist dari URL...');
    try {
      const isYtm =
        trimmed.includes('youtube.com') ||
        trimmed.includes('youtu.be') ||
        trimmed.startsWith('PL') ||
        trimmed.startsWith('OLAK') ||
        trimmed.startsWith('VL');
      const pl = isYtm
        ? await LumeCore.importYtmPlaylistUrl(trimmed)
        : await LumeCore.importSpotifyPlaylistUrl(trimmed);
      setImportUrl('');
      await loadLibrary();
      Alert.alert('Import Berhasil', `Playlist "${pl.name}" (${pl.track_count} lagu) tersimpan.`);
    } catch (err) {
      Alert.alert('Import Gagal', err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMessage(null);
    }
  };

  const handleImportCsv = async () => {
    try {
      const picked = await DocumentPicker.getDocumentAsync({
        type: ['text/csv', 'text/comma-separated-values', 'public.comma-separated-values-text', '*/*'],
        multiple: true,
        copyToCacheDirectory: true,
      });
      if (picked.canceled || !picked.assets || picked.assets.length === 0) return;
      setBusyMessage(`Mengimpor ${picked.assets.length} file Exportify CSV...`);
      const importedNames: string[] = [];
      let totalTracks = 0;
      for (const asset of picked.assets) {
        const csvText = await new ExpoFile(asset.uri).text();
        let rawName = asset.name;
        try {
          rawName = decodeURIComponent(asset.name);
        } catch {
          // Keep original name if not percent-encoded.
        }
        const name = rawName.replace(/\.csv$/i, '').trim() || 'Exportify Playlist';
        const pl = await LumeCore.importExportifyCsv(csvText, name);
        importedNames.push(`${pl.name} (${pl.track_count})`);
        totalTracks += pl.track_count;
      }
      await loadLibrary();
      Alert.alert(
        'Import CSV Berhasil',
        `${importedNames.length} playlist (${totalTracks} lagu total):\n${importedNames.join(', ')}`
      );
    } catch (err) {
      Alert.alert('Import CSV Gagal', err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMessage(null);
    }
  };

  const handleImportLocalAudio = async () => {
    try {
      const picked = await DocumentPicker.getDocumentAsync({
        type: ['audio/*', 'public.audio'],
        multiple: true,
        copyToCacheDirectory: true,
      });
      if (picked.canceled || !picked.assets || picked.assets.length === 0) return;
      setBusyMessage(`Mengimpor ${picked.assets.length} file audio lokal...`);
      let latest: Track[] = [];
      for (const asset of picked.assets) {
        latest = await LumeCore.importLocalAudioFile(asset.uri, asset.name);
      }
      setLocalTracks(latest);
      Alert.alert('Audio Lokal Siap', `Total ${latest.length} file audio lokal tersimpan di HP.`);
    } catch (err) {
      Alert.alert('Import Audio Gagal', err instanceof Error ? err.message : String(err));
    } finally {
      setBusyMessage(null);
    }
  };

  const handleCreatePlaylist = async () => {
    const trimmed = newPlaylistName.trim();
    if (!trimmed) return;
    await LumeCore.createPlaylist(trimmed);
    setNewPlaylistName('');
    await loadLibrary();
  };

  const handleRenamePlaylist = async () => {
    if (!selectedPlaylist) return;
    const trimmed = renameText.trim();
    if (!trimmed) return;
    await LumeCore.renamePlaylist(selectedPlaylist.id, trimmed);
    setSelectedPlaylist({ ...selectedPlaylist, name: trimmed });
    setRenaming(false);
    await loadLibrary();
  };

  const handleDeletePlaylist = (pl: Playlist) => {
    Alert.alert('Hapus Playlist?', `Hapus "${pl.name}" dari Library?`, [
      { text: 'Batal', style: 'cancel' },
      {
        text: 'Hapus',
        style: 'destructive',
        onPress: async () => {
          await LumeCore.deletePlaylist(pl.id);
          setActiveView('main');
          setSelectedPlaylist(null);
          await loadLibrary();
        },
      },
    ]);
  };

  const handleDownloadPlaylist = (name: string, tracks: Track[]) => {
    void downloadPlaylistOffline(name, tracks);
  };

  const handleDownloadPlaylistById = async (pl: Playlist) => {
    try {
      const tracks = await LumeCore.getPlaylistTracks(pl.id);
      if (tracks.length === 0) {
        Alert.alert('Playlist Kosong', `"${pl.name}" belum memiliki lagu.`);
        return;
      }
      void downloadPlaylistOffline(pl.name, tracks);
    } catch (err) {
      Alert.alert('Gagal', err instanceof Error ? err.message : String(err));
    }
  };

  const rawDetailTracks =
    activeView === 'liked'
      ? likedTracks
      : activeView === 'local'
        ? localTracks
        : selectedTracks;

  const sortedDetailTracks = useMemo(() => {
    const copy = [...rawDetailTracks];
    if (sortMode === 'title') {
      copy.sort((a, b) => a.title.localeCompare(b.title));
    } else if (sortMode === 'artist') {
      copy.sort((a, b) => trackArtistDisplay(a).localeCompare(trackArtistDisplay(b)));
    } else if (sortMode === 'duration') {
      copy.sort((a, b) => a.duration_ms - b.duration_ms);
    }
    return copy;
  }, [rawDetailTracks, sortMode]);

  if (activeView !== 'main') {
    const activeTitle =
      activeView === 'liked'
        ? 'Liked Songs'
        : activeView === 'local'
          ? 'Local Files'
          : selectedPlaylist?.name ?? '';
    const canEdit =
      activeView === 'playlist' &&
      selectedPlaylist !== null &&
      isPlaylistEditable(selectedPlaylist.origin);

    return (
      <AmbientBackdrop>
        <SafeAreaView style={styles.container} edges={['top']}>
          <View style={styles.detailHeader}>
            <Pressable
              onPress={() => {
                setActiveView('main');
                setSelectedPlaylist(null);
              }}
              style={styles.backBtn}
            >
              <Ionicons name="chevron-back" size={22} color={LumeTheme.textPrimary} />
              <Text style={styles.backText}>Library</Text>
            </Pressable>

            <View style={styles.detailActions}>
              {activeView === 'local' && (
                <Pressable
                  onPress={() => void handleImportLocalAudio()}
                  style={[styles.smallActionBtn, { backgroundColor: palette.accent }]}
                >
                  <Ionicons name="add" size={16} color="#0A0C12" />
                  <Text style={styles.smallActionDark}>Import Audio</Text>
                </Pressable>
              )}
              {canEdit && selectedPlaylist && (
                <>
                  <Pressable
                    onPress={() => setRenaming((r) => !r)}
                    style={styles.iconSmallBtn}
                  >
                    <Ionicons name="pencil" size={16} color="#FFFFFF" />
                  </Pressable>
                  <Pressable
                    onPress={() => handleDeletePlaylist(selectedPlaylist)}
                    style={styles.iconSmallBtn}
                  >
                    <Ionicons name="trash-outline" size={16} color={LumeTheme.rose} />
                  </Pressable>
                </>
              )}
              {sortedDetailTracks.length > 0 && activeView !== 'local' && (
                <Pressable
                  onPress={() => handleDownloadPlaylist(activeTitle, sortedDetailTracks)}
                  style={[styles.smallActionBtn, { backgroundColor: palette.accent }]}
                >
                  <Ionicons name="cloud-download" size={15} color="#0A0C12" />
                  <Text style={styles.smallActionDark}>Offline All</Text>
                </Pressable>
              )}
            </View>
          </View>

          {renaming ? (
            <View style={[styles.inlineForm, { paddingHorizontal: 16, marginVertical: 8 }]}>
              <TextInput
                value={renameText}
                onChangeText={setRenameText}
                style={styles.input}
                placeholder="Nama playlist baru"
                placeholderTextColor={LumeTheme.textMuted}
              />
              <Pressable
                onPress={() => void handleRenamePlaylist()}
                style={[styles.primaryBtn, { backgroundColor: palette.accent }]}
              >
                <Text style={styles.btnTextDark}>Simpan</Text>
              </Pressable>
            </View>
          ) : (
            <Text style={styles.detailTitle}>{activeTitle}</Text>
          )}

          <ScrollView
            horizontal
            showsHorizontalScrollIndicator={false}
            contentContainerStyle={styles.sortRow}
          >
            {(['custom', 'title', 'artist', 'duration'] as SortMode[]).map((mode) => (
              <Pressable
                key={mode}
                onPress={() => setSortMode(mode)}
                style={[styles.sortChip, sortMode === mode && styles.sortChipActive]}
              >
                <Text style={[styles.sortText, sortMode === mode && styles.sortTextActive]}>
                  {mode === 'custom'
                    ? 'Urutan Asli'
                    : mode === 'title'
                      ? 'Judul'
                      : mode === 'artist'
                        ? 'Artis'
                        : 'Durasi'}
                </Text>
              </Pressable>
            ))}
          </ScrollView>

          <BatchDownloadBanner />

          {busyMessage && (
            <View style={styles.busyRow}>
              <ActivityIndicator size="small" color={palette.accent} />
              <Text style={styles.busyText}>{busyMessage}</Text>
            </View>
          )}

          <ScrollView contentContainerStyle={styles.listContent}>
            {sortedDetailTracks.length === 0 ? (
              <Text style={styles.emptyDetail}>Belum ada lagu di sini.</Text>
            ) : (
              sortedDetailTracks.map((track) => (
                <TrackRow
                  key={track.uid}
                  track={track}
                  isActive={currentTrack?.uid === track.uid}
                  isOffline={offlineUids.has(track.uid)}
                  isDownloading={downloadingUids.has(track.uid)}
                  onPress={() => void playTrack(track, sortedDetailTracks)}
                  onDownloadPress={
                    activeView === 'local' ? undefined : () => void downloadOffline(track)
                  }
                  onLikePress={() => void toggleLikeTrack(track)}
                />
              ))
            )}
          </ScrollView>
        </SafeAreaView>
      </AmbientBackdrop>
    );
  }

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.container} edges={['top']}>
        <ScrollView contentContainerStyle={styles.scrollContent}>
          <View style={styles.topRow}>
            <Text style={styles.title}>Library</Text>
            <Pressable
              onPress={() => router.push('/settings')}
              style={styles.settingsBtn}
            >
              <Ionicons name="settings-outline" size={20} color="#FFFFFF" />
            </Pressable>
          </View>

          <BatchDownloadBanner />

          {busyMessage && (
            <View style={styles.busyRow}>
              <ActivityIndicator size="small" color={palette.accent} />
              <Text style={styles.busyText}>{busyMessage}</Text>
            </View>
          )}

          <View style={styles.quickGrid}>
            <Pressable
              onPress={() => {
                setSortMode('custom');
                setActiveView('liked');
              }}
              style={styles.likedCard}
            >
              <View style={styles.likedIcon}>
                <Ionicons name="heart" size={22} color="#FFFFFF" />
              </View>
              <Text style={styles.cardTitle}>Liked Songs</Text>
              <Text style={styles.cardSub}>{likedTracks.length} lagu</Text>
            </Pressable>

            <Pressable
              onPress={() => {
                setSortMode('custom');
                setActiveView('local');
              }}
              style={styles.localCard}
            >
              <View style={styles.localIcon}>
                <Ionicons name="folder-open" size={22} color="#FFFFFF" />
              </View>
              <Text style={styles.cardTitle}>Local Files</Text>
              <Text style={styles.cardSub}>{localTracks.length} file HP</Text>
            </Pressable>
          </View>

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
                style={[styles.primaryBtn, { backgroundColor: palette.accent }]}
              >
                <Text style={styles.btnTextDark}>Buat</Text>
              </Pressable>
            </View>

            {playlists.map((pl) => {
              const badge = playlistOriginBadge(pl.origin);
              return (
                <Pressable
                  key={pl.id}
                  onPress={() => void openPlaylist(pl)}
                  style={styles.playlistRow}
                >
                  <View style={styles.plIcon}>
                    <Ionicons
                      name="musical-notes"
                      size={18}
                      color={badge === 'SP' ? LumeTheme.spotify : palette.accent}
                    />
                  </View>
                  <View style={{ flex: 1 }}>
                    <Text style={styles.cardTitle} numberOfLines={1}>
                      {pl.name}
                    </Text>
                    <View style={styles.plMetaRow}>
                      <View style={styles.originBadge}>
                        <Text style={styles.originBadgeText}>{badge}</Text>
                      </View>
                      <Text style={styles.cardSub}>{pl.track_count} lagu</Text>
                    </View>
                  </View>
                  <Pressable
                    onPress={(e) => {
                      e.stopPropagation();
                      void handleDownloadPlaylistById(pl);
                    }}
                    hitSlop={8}
                    style={styles.iconSmallBtn}
                    accessibilityLabel={`Download playlist ${pl.name}`}
                  >
                    <Ionicons name="cloud-download-outline" size={18} color={palette.accent} />
                  </Pressable>
                  <Ionicons name="chevron-forward" size={18} color={LumeTheme.textMuted} />
                </Pressable>
              );
            })}
          </View>

          <View style={styles.cardPanel}>
            <Text style={styles.panelTitle}>Import Playlist (Spotify / YouTube Music / CSV)</Text>
            <View style={styles.inlineForm}>
              <TextInput
                value={importUrl}
                onChangeText={setImportUrl}
                placeholder="Tempel URL Spotify atau YouTube Music..."
                placeholderTextColor={LumeTheme.textMuted}
                autoCapitalize="none"
                autoCorrect={false}
                style={styles.input}
              />
              <Pressable
                onPress={() => void handleImportUrl()}
                style={[styles.primaryBtn, { backgroundColor: palette.accent }]}
              >
                <Text style={styles.btnTextDark}>Import</Text>
              </Pressable>
            </View>

            <View style={styles.importRow}>
              <Pressable onPress={() => void handleImportCsv()} style={styles.secondaryBtn}>
                <Ionicons name="document-text-outline" size={16} color="#FFFFFF" />
                <Text style={styles.secondaryBtnText}>Import Exportify .CSV</Text>
              </Pressable>
              <Pressable onPress={() => void handleImportLocalAudio()} style={styles.secondaryBtn}>
                <Ionicons name="musical-note-outline" size={16} color="#FFFFFF" />
                <Text style={styles.secondaryBtnText}>Import MP3/M4A HP</Text>
              </Pressable>
            </View>
          </View>
        </ScrollView>
      </SafeAreaView>
    </AmbientBackdrop>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  scrollContent: {
    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 165,
    gap: 16,
  },
  topRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 27,
    fontWeight: '800',
  },
  settingsBtn: {
    width: 38,
    height: 38,
    borderRadius: 19,
    backgroundColor: 'rgba(255,255,255,0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  quickGrid: {
    flexDirection: 'row',
    gap: 12,
  },
  likedCard: {
    flex: 1,
    padding: 14,
    borderRadius: 16,
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.34)',
    gap: 6,
  },
  localCard: {
    flex: 1,
    padding: 14,
    borderRadius: 16,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: LumeTheme.border,
    gap: 6,
  },
  likedIcon: {
    width: 40,
    height: 40,
    borderRadius: 12,
    backgroundColor: LumeTheme.rose,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 4,
  },
  localIcon: {
    width: 40,
    height: 40,
    borderRadius: 12,
    backgroundColor: 'rgba(255,255,255,0.16)',
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 4,
  },
  detailHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 14,
    paddingTop: 8,
  },
  detailActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
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
  smallActionBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    paddingHorizontal: 12,
    height: 34,
    borderRadius: 10,
  },
  smallActionDark: {
    color: '#0A0C12',
    fontSize: 12.5,
    fontWeight: '800',
  },
  iconSmallBtn: {
    width: 34,
    height: 34,
    borderRadius: 10,
    backgroundColor: 'rgba(255,255,255,0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  detailTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 24,
    fontWeight: '800',
    paddingHorizontal: 16,
    marginTop: 8,
    marginBottom: 8,
  },
  sortRow: {
    gap: 8,
    paddingHorizontal: 16,
    paddingBottom: 10,
  },
  sortChip: {
    paddingHorizontal: 12,
    paddingVertical: 5,
    borderRadius: 999,
    backgroundColor: 'rgba(255,255,255,0.08)',
  },
  sortChipActive: {
    backgroundColor: 'rgba(255,255,255,0.24)',
  },
  sortText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontWeight: '600',
  },
  sortTextActive: {
    color: '#FFFFFF',
    fontWeight: '700',
  },
  listContent: {
    paddingHorizontal: 10,
    paddingBottom: 160,
  },
  emptyDetail: {
    color: LumeTheme.textSecondary,
    textAlign: 'center',
    marginTop: 40,
    fontSize: 14,
  },
  busyRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  busyText: {
    color: '#FFFFFF',
    fontSize: 13,
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
    backgroundColor: 'rgba(255,255,255,0.08)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  plMetaRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    marginTop: 3,
  },
  originBadge: {
    backgroundColor: 'rgba(255,255,255,0.14)',
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 5,
  },
  originBadgeText: {
    color: '#FFFFFF',
    fontSize: 9.5,
    fontWeight: '800',
  },
  cardTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 15,
    fontWeight: '700',
  },
  cardSub: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
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
    fontSize: 14.5,
    fontWeight: '700',
  },
  inlineForm: {
    flexDirection: 'row',
    gap: 8,
  },
  input: {
    flex: 1,
    backgroundColor: 'rgba(0,0,0,0.35)',
    borderWidth: 1,
    borderColor: LumeTheme.border,
    borderRadius: 12,
    paddingHorizontal: 12,
    height: 42,
    color: LumeTheme.textPrimary,
    fontSize: 14,
  },
  primaryBtn: {
    paddingHorizontal: 16,
    height: 42,
    borderRadius: 12,
    alignItems: 'center',
    justifyContent: 'center',
  },
  btnTextDark: {
    color: '#0A0C12',
    fontWeight: '800',
    fontSize: 13.5,
  },
  importRow: {
    flexDirection: 'row',
    gap: 8,
  },
  secondaryBtn: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
    backgroundColor: 'rgba(255,255,255,0.10)',
    borderRadius: 12,
    height: 40,
    paddingHorizontal: 10,
  },
  secondaryBtnText: {
    color: '#FFFFFF',
    fontSize: 12,
    fontWeight: '700',
  },
});
