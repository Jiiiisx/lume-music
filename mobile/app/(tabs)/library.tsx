import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  FlatList,
  Image,
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { BlurView } from 'expo-blur';
import * as DocumentPicker from 'expo-document-picker';
import { File as ExpoFile } from 'expo-file-system';
import * as Haptics from 'expo-haptics';
import * as SecureStore from 'expo-secure-store';
import { useFocusEffect, useRouter } from 'expo-router';
import Animated, { FadeIn, FadeInRight } from 'react-native-reanimated';
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

type SortField = 'custom' | 'recently_added' | 'title' | 'artist' | 'album' | 'duration';
type SortDirection = 'asc' | 'desc';

interface PlaylistSortPref {
  field: SortField;
  direction: SortDirection;
}

const DEFAULT_SORT: PlaylistSortPref = { field: 'custom', direction: 'asc' };
const KEY_PLAYLIST_ORDER = 'lume_playlist_order_v1';
const KEY_PLAYLIST_SORTS = 'lume_playlist_sorts_v1';

const SORT_FIELDS: { field: SortField; label: string }[] = [
  { field: 'custom', label: 'Urutan Asli' },
  { field: 'recently_added', label: 'Terbaru' },
  { field: 'title', label: 'Judul' },
  { field: 'artist', label: 'Artis' },
  { field: 'album', label: 'Album' },
  { field: 'duration', label: 'Durasi' },
];

function needsSpotifyArt(track: Track): boolean {
  if (!('Spotify' in track.source)) return false;
  const art = track.artwork_ref?.trim() ?? '';
  return art.length === 0 || !art.includes('ab67616d');
}

export default function LibraryScreen() {
  const router = useRouter();
  const {
    currentTrack,
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
  const [sortPref, setSortPref] = useState<PlaylistSortPref>(DEFAULT_SORT);
  const [savedSorts, setSavedSorts] = useState<Record<string, PlaylistSortPref>>({});
  const [reorderingTracks, setReorderingTracks] = useState(false);
  const [reorderingPlaylists, setReorderingPlaylists] = useState(false);
  const [playlistOrder, setPlaylistOrder] = useState<string[]>([]);
  const enrichRunRef = useRef(0);
  const lastLibraryLoadRef = useRef(0);
  const pickerInFlightRef = useRef(false);
  const pendingImportActionRef = useRef<'csv' | 'audio' | null>(null);

  const [showCreateModal, setShowCreateModal] = useState(false);
  const [showImportModal, setShowImportModal] = useState(false);
  const [importUrl, setImportUrl] = useState('');
  const [newPlaylistName, setNewPlaylistName] = useState('');
  const [renameText, setRenameText] = useState('');
  const [renaming, setRenaming] = useState(false);
  const [busyMessage, setBusyMessage] = useState<string | null>(null);

  const loadLibrary = useCallback(async (force = true) => {
    const now = Date.now();
    if (!force && now - lastLibraryLoadRef.current < 3000) return;
    lastLibraryLoadRef.current = now;
    await refreshLibraryState();
    try {
      const [savedOrder, rawSorts] = await Promise.all([
        SecureStore.getItemAsync(KEY_PLAYLIST_ORDER),
        SecureStore.getItemAsync(KEY_PLAYLIST_SORTS),
      ]);
      if (savedOrder) {
        const parsed = JSON.parse(savedOrder) as unknown;
        if (Array.isArray(parsed)) {
          setPlaylistOrder(parsed.filter((v): v is string => typeof v === 'string'));
        }
      }
      if (rawSorts) {
        const parsedSorts = JSON.parse(rawSorts) as Record<string, PlaylistSortPref>;
        if (parsedSorts && typeof parsedSorts === 'object') {
          setSavedSorts(parsedSorts);
        }
      }
    } catch {
      // Non-fatal.
    }
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
      void loadLibrary(false);
    }, [loadLibrary])
  );

  const currentSortKey =
    activeView === 'liked'
      ? '__liked__'
      : activeView === 'local'
        ? '__local__'
        : selectedPlaylist?.id ?? '__default__';

  const applySortPref = (next: PlaylistSortPref, key = currentSortKey) => {
    setSortPref(next);
    setSavedSorts((prev) => {
      const updated = { ...prev, [key]: next };
      void SecureStore.setItemAsync(KEY_PLAYLIST_SORTS, JSON.stringify(updated)).catch(() => {});
      return updated;
    });
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
  };

  const handleSelectSortField = (field: SortField) => {
    if (sortPref.field === field) {
      applySortPref({
        field,
        direction: sortPref.direction === 'asc' ? 'desc' : 'asc',
      });
    } else {
      applySortPref({
        field,
        direction: field === 'recently_added' ? 'desc' : 'asc',
      });
    }
  };

  const handleToggleSortDirection = () => {
    applySortPref({
      field: sortPref.field,
      direction: sortPref.direction === 'asc' ? 'desc' : 'asc',
    });
  };

  const openPlaylist = async (pl: Playlist) => {
    const tracks = await LumeCore.getPlaylistTracks(pl.id);
    setSelectedPlaylist(pl);
    setSelectedTracks(tracks);
    setRenameText(pl.name);
    setRenaming(false);
    setReorderingTracks(false);
    setSortPref(savedSorts[pl.id] ?? DEFAULT_SORT);
    setActiveView('playlist');
  };

  // Progressively enrich missing Spotify album artwork (Exportify CSV & public URL imports)
  // via Spotify's unauthenticated oEmbed endpoint (matching Lume Desktop), caching results in SQLite.
  useEffect(() => {
    if (!LumeCore.isAvailable()) return;
    if (activeView !== 'playlist' && activeView !== 'liked') return;

    const initialTracks = activeView === 'liked' ? likedTracks : selectedTracks;
    if (!initialTracks.some(needsSpotifyArt)) return;

    const runId = ++enrichRunRef.current;
    let cancelled = false;

    const run = async () => {
      const attempted = new Set<string>();
      const working = [...initialTracks];
      let firstBatch = true;

      while (!cancelled && enrichRunRef.current === runId) {
        const batchSize = firstBatch ? 12 : 24;
        firstBatch = false;
        const pending = working.filter((t) => needsSpotifyArt(t) && !attempted.has(t.uid));
        if (pending.length === 0) break;

        const slice = pending.slice(0, batchSize);
        for (const t of slice) attempted.add(t.uid);

        try {
          const resolvedMap = await LumeCore.enrichSpotifyArtwork(slice, slice.length);
          if (cancelled || enrichRunRef.current !== runId) break;
          const entries = Object.entries(resolvedMap);
          if (entries.length > 0) {
            for (let i = 0; i < working.length; i++) {
              const updatedUrl = resolvedMap[working[i].uid];
              if (updatedUrl) {
                working[i] = { ...working[i], artwork_ref: updatedUrl };
              }
            }
            if (activeView === 'playlist') {
              setSelectedTracks((prev) =>
                prev.map((t) =>
                  resolvedMap[t.uid] ? { ...t, artwork_ref: resolvedMap[t.uid] } : t
                )
              );
            } else if (activeView === 'liked') {
              void refreshLibraryState();
            }
          }
        } catch {
          // Network may be offline; skip remaining enrichment quietly.
          break;
        }

        await new Promise((r) => setTimeout(r, 120));
      }
    };

    void run();
    return () => {
      cancelled = true;
    };
    // Trigger once per opened playlist / liked view (avoid re-triggering on every batch state update).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [activeView, selectedPlaylist?.id]);

  const handleImportUrl = async () => {
    const trimmed = importUrl.trim();
    if (!trimmed) return;
    setShowImportModal(false);
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

  const runImportCsvPicker = async () => {
    if (pickerInFlightRef.current) return;
    pickerInFlightRef.current = true;
    try {
      const picked = await DocumentPicker.getDocumentAsync({
        type: ['text/csv', 'text/comma-separated-values', 'text/plain', '*/*'],
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
      pickerInFlightRef.current = false;
      setBusyMessage(null);
    }
  };

  const runImportLocalAudioPicker = async () => {
    if (pickerInFlightRef.current) return;
    pickerInFlightRef.current = true;
    try {
      const picked = await DocumentPicker.getDocumentAsync({
        type: ['audio/*'],
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
      pickerInFlightRef.current = false;
      setBusyMessage(null);
    }
  };

  const flushPendingImportAction = () => {
    const action = pendingImportActionRef.current;
    if (!action) return;
    pendingImportActionRef.current = null;
    // Give UIKit one runloop turn after Modal dismissal before presenting UIDocumentPickerViewController.
    setTimeout(() => {
      if (action === 'csv') {
        void runImportCsvPicker();
      } else {
        void runImportLocalAudioPicker();
      }
    }, 80);
  };

  const handleImportCsv = () => {
    if (showImportModal) {
      pendingImportActionRef.current = 'csv';
      setShowImportModal(false);
      if (Platform.OS !== 'ios') {
        setTimeout(flushPendingImportAction, 350);
      }
    } else {
      void runImportCsvPicker();
    }
  };

  const handleImportLocalAudio = () => {
    if (showImportModal) {
      pendingImportActionRef.current = 'audio';
      setShowImportModal(false);
      if (Platform.OS !== 'ios') {
        setTimeout(flushPendingImportAction, 350);
      }
    } else {
      void runImportLocalAudioPicker();
    }
  };

  const handleCreatePlaylist = async () => {
    const trimmed = newPlaylistName.trim();
    if (!trimmed) return;
    await LumeCore.createPlaylist(trimmed);
    setNewPlaylistName('');
    setShowCreateModal(false);
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
    const indexed = rawDetailTracks.map((track, idx) => ({ track, idx }));
    const desc = sortPref.direction === 'desc';

    if (sortPref.field === 'custom' || sortPref.field === 'recently_added') {
      if (desc) indexed.reverse();
    } else if (sortPref.field === 'title') {
      indexed.sort((a, b) => {
        const ord =
          a.track.title.localeCompare(b.track.title, undefined, { sensitivity: 'base' }) ||
          trackArtistDisplay(a.track).localeCompare(trackArtistDisplay(b.track), undefined, {
            sensitivity: 'base',
          }) ||
          a.idx - b.idx;
        return desc ? -ord : ord;
      });
    } else if (sortPref.field === 'artist') {
      indexed.sort((a, b) => {
        const ord =
          trackArtistDisplay(a.track).localeCompare(trackArtistDisplay(b.track), undefined, {
            sensitivity: 'base',
          }) ||
          a.track.title.localeCompare(b.track.title, undefined, { sensitivity: 'base' }) ||
          a.idx - b.idx;
        return desc ? -ord : ord;
      });
    } else if (sortPref.field === 'album') {
      indexed.sort((a, b) => {
        const albA = (a.track.album ?? '').trim();
        const albB = (b.track.album ?? '').trim();
        if (!albA && albB) return 1;
        if (albA && !albB) return -1;
        const ord =
          albA.localeCompare(albB, undefined, { sensitivity: 'base' }) ||
          a.track.title.localeCompare(b.track.title, undefined, { sensitivity: 'base' }) ||
          a.idx - b.idx;
        return desc ? -ord : ord;
      });
    } else if (sortPref.field === 'duration') {
      indexed.sort((a, b) => {
        const ord = a.track.duration_ms - b.track.duration_ms || a.idx - b.idx;
        return desc ? -ord : ord;
      });
    }

    return indexed.map((entry) => entry.track);
  }, [rawDetailTracks, sortPref]);

  const orderedPlaylists = useMemo(() => {
    if (playlistOrder.length === 0) return playlists;
    const rankMap = new Map(playlistOrder.map((id, idx) => [id, idx]));
    return [...playlists].sort((a, b) => {
      const ra = rankMap.get(a.id) ?? Number.MAX_SAFE_INTEGER;
      const rb = rankMap.get(b.id) ?? Number.MAX_SAFE_INTEGER;
      return ra - rb;
    });
  }, [playlists, playlistOrder]);

  const movePlaylistOrder = async (fromIdx: number, toIdx: number) => {
    if (
      fromIdx === toIdx ||
      fromIdx < 0 ||
      toIdx < 0 ||
      fromIdx >= orderedPlaylists.length ||
      toIdx >= orderedPlaylists.length
    ) {
      return;
    }
    const ids = orderedPlaylists.map((p) => p.id);
    const [moved] = ids.splice(fromIdx, 1);
    ids.splice(toIdx, 0, moved);
    setPlaylistOrder(ids);
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
    await SecureStore.setItemAsync(KEY_PLAYLIST_ORDER, JSON.stringify(ids)).catch(() => {});
  };

  const handleMovePlaylistTrack = async (fromPos: number, toPos: number) => {
    if (!selectedPlaylist) return;
    if (
      fromPos === toPos ||
      fromPos < 0 ||
      toPos < 0 ||
      fromPos >= selectedTracks.length ||
      toPos >= selectedTracks.length
    ) {
      return;
    }
    const prev = selectedTracks;
    const next = [...prev];
    const [moved] = next.splice(fromPos, 1);
    next.splice(toPos, 0, moved);
    setSelectedTracks(next);
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
    try {
      await LumeCore.reorderPlaylistTrack(selectedPlaylist.id, fromPos, toPos);
    } catch (err) {
      setSelectedTracks(prev);
      Alert.alert('Gagal Mengubah Urutan', err instanceof Error ? err.message : String(err));
    }
  };

  const handleRemoveTrackAt = async (position: number, title: string) => {
    if (!selectedPlaylist) return;
    Alert.alert('Hapus Lagu dari Playlist?', `Hapus "${title}" dari "${selectedPlaylist.name}"?`, [
      { text: 'Batal', style: 'cancel' },
      {
        text: 'Hapus',
        style: 'destructive',
        onPress: async () => {
          try {
            await LumeCore.removeTrackFromPlaylist(selectedPlaylist.id, position);
            const updated = await LumeCore.getPlaylistTracks(selectedPlaylist.id);
            setSelectedTracks(updated);
            await refreshLibraryState();
          } catch (err) {
            Alert.alert('Gagal', err instanceof Error ? err.message : String(err));
          }
        },
      },
    ]);
  };

  const playDetailAll = async (shuffled: boolean) => {
    if (sortedDetailTracks.length === 0) return;
    const startTrack = shuffled
      ? sortedDetailTracks[Math.floor(Math.random() * sortedDetailTracks.length)]
      : sortedDetailTracks[0];
    await playTrack(startTrack, sortedDetailTracks, false, shuffled);
  };

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
          <Animated.View entering={FadeInRight.duration(220)} style={styles.container}>
          <View style={styles.detailHeader}>
            <Pressable
              onPress={() => {
                setActiveView('main');
                setSelectedPlaylist(null);
              }}
              style={styles.backBtn}
            >
              <Ionicons name="chevron-back" size={22} color="#FFFFFF" />
              <Text style={styles.backText}>Library</Text>
            </Pressable>

            <View style={styles.detailActions}>
              {canEdit && selectedPlaylist && (
                <>
                  <Pressable
                    onPress={() => {
                      const next = !reorderingTracks;
                      setReorderingTracks(next);
                      if (next) setSortPref(DEFAULT_SORT);
                    }}
                    style={[
                      styles.iconCircle,
                      reorderingTracks && styles.iconCircleActive,
                    ]}
                    accessibilityLabel="Atur urutan lagu"
                  >
                    <Ionicons
                      name="swap-vertical"
                      size={16}
                      color={reorderingTracks ? LumeTheme.inkDark : '#FFFFFF'}
                    />
                  </Pressable>
                  <Pressable
                    onPress={() => setRenaming((r) => !r)}
                    style={styles.iconCircle}
                    accessibilityLabel="Rename playlist"
                  >
                    <Ionicons name="pencil" size={16} color="#FFFFFF" />
                  </Pressable>
                  <Pressable
                    onPress={() => handleDeletePlaylist(selectedPlaylist)}
                    style={styles.iconCircle}
                    accessibilityLabel="Delete playlist"
                  >
                    <Ionicons name="trash-outline" size={16} color={LumeTheme.rose} />
                  </Pressable>
                </>
              )}
              {activeView === 'local' && (
                <Pressable
                  onPress={() => void handleImportLocalAudio()}
                  style={styles.crystalPillSmall}
                >
                  <Ionicons name="add" size={16} color={LumeTheme.inkDark} />
                  <Text style={styles.crystalPillSmallText}>Import</Text>
                </Pressable>
              )}
            </View>
          </View>

          <FlatList
            data={reorderingTracks && canEdit ? selectedTracks : sortedDetailTracks}
            keyExtractor={(item, idx) => `${item.uid}-${idx}`}
            contentContainerStyle={styles.detailScroll}
            showsVerticalScrollIndicator={false}
            initialNumToRender={14}
            maxToRenderPerBatch={12}
            windowSize={7}
            removeClippedSubviews
            ListHeaderComponent={
              <>
                {/* Desktop-inspired Playlist Hero Header */}
                <View style={styles.detailHero}>
                  {renaming ? (
                    <View style={styles.inlineForm}>
                      <TextInput
                        value={renameText}
                        onChangeText={setRenameText}
                        style={styles.input}
                        placeholder="Nama playlist baru"
                        placeholderTextColor={LumeTheme.textMuted}
                      />
                      <Pressable
                        onPress={() => void handleRenamePlaylist()}
                        style={styles.crystalPrimaryBtn}
                      >
                        <Text style={styles.btnTextDark}>Simpan</Text>
                      </Pressable>
                    </View>
                  ) : (
                    <>
                      <Text style={styles.detailTitle}>{activeTitle}</Text>
                      <Text style={styles.detailSub}>
                        {sortedDetailTracks.length} lagu
                        {activeView === 'playlist' && selectedPlaylist
                          ? ` • ${playlistOriginBadge(selectedPlaylist.origin)}`
                          : ''}
                      </Text>
                    </>
                  )}

                  {sortedDetailTracks.length > 0 && (
                    <View style={styles.heroBtnRow}>
                      <Pressable
                        onPress={() => void playDetailAll(false)}
                        style={styles.heroPlayBtn}
                      >
                        <Ionicons name="play" size={16} color={LumeTheme.inkDark} />
                        <Text style={styles.heroPlayText}>Putar</Text>
                      </Pressable>
                      <Pressable
                        onPress={() => void playDetailAll(true)}
                        style={styles.heroGlassBtn}
                      >
                        <Ionicons name="shuffle" size={16} color="#FFFFFF" />
                        <Text style={styles.heroGlassText}>Acak</Text>
                      </Pressable>
                      {activeView !== 'local' && (
                        <Pressable
                          onPress={() => handleDownloadPlaylist(activeTitle, sortedDetailTracks)}
                          style={styles.heroGlassBtn}
                        >
                          <Ionicons name="cloud-download-outline" size={16} color="#FFFFFF" />
                          <Text style={styles.heroGlassText}>Offline</Text>
                        </Pressable>
                      )}
                    </View>
                  )}
                </View>

                {reorderingTracks && canEdit ? (
                  <View style={styles.reorderBanner}>
                    <Ionicons name="swap-vertical" size={16} color="#FFFFFF" />
                    <Text style={styles.reorderBannerText}>
                      Geser naik/turun untuk mengatur urutan lagu
                    </Text>
                    <Pressable
                      onPress={() => setReorderingTracks(false)}
                      style={styles.reorderDoneBtn}
                    >
                      <Text style={styles.reorderDoneText}>Selesai</Text>
                    </Pressable>
                  </View>
                ) : (
                  <ScrollView
                    horizontal
                    showsHorizontalScrollIndicator={false}
                    style={{ flexGrow: 0 }}
                    contentContainerStyle={styles.sortRow}
                  >
                    <Pressable
                      onPress={handleToggleSortDirection}
                      style={styles.dirChip}
                      accessibilityLabel="Toggle sort direction"
                    >
                      <Ionicons
                        name={sortPref.direction === 'asc' ? 'arrow-up' : 'arrow-down'}
                        size={13}
                        color="#FFFFFF"
                      />
                      <Text style={styles.dirChipText}>
                        {sortPref.direction === 'asc' ? 'Asc' : 'Desc'}
                      </Text>
                    </Pressable>

                    {SORT_FIELDS.map(({ field, label }) => {
                      const active = sortPref.field === field;
                      return (
                        <Pressable
                          key={field}
                          onPress={() => handleSelectSortField(field)}
                          style={[styles.sortChip, active && styles.sortChipActive]}
                        >
                          <Text style={[styles.sortText, active && styles.sortTextActive]}>
                            {label}
                          </Text>
                          {active && (
                            <Ionicons
                              name={sortPref.direction === 'asc' ? 'arrow-up' : 'arrow-down'}
                              size={12}
                              color={LumeTheme.inkDark}
                            />
                          )}
                        </Pressable>
                      );
                    })}
                  </ScrollView>
                )}

                <BatchDownloadBanner />

                {busyMessage && (
                  <View style={styles.busyRow}>
                    <ActivityIndicator size="small" color="#FFFFFF" />
                    <Text style={styles.busyText}>{busyMessage}</Text>
                  </View>
                )}
              </>
            }
            ListEmptyComponent={
              <View style={[styles.emptyBox, { marginHorizontal: 16 }]}>
                <Ionicons name="musical-notes-outline" size={32} color={LumeTheme.textMuted} />
                <Text style={styles.emptyDetail}>Belum ada lagu di sini.</Text>
              </View>
            }
            renderItem={({ item: track, index: idx }) =>
              reorderingTracks && canEdit ? (
                <View style={styles.detailRowPad}>
                  <View style={styles.reorderRow}>
                    <Text style={styles.reorderRank}>{String(idx + 1).padStart(2, '0')}</Text>
                    {track.artwork_ref ? (
                      <Image source={{ uri: track.artwork_ref }} style={styles.reorderArt} />
                    ) : (
                      <View style={[styles.reorderArt, styles.reorderArtFallback]}>
                        <Ionicons name="musical-note" size={16} color="rgba(255,255,255,0.7)" />
                      </View>
                    )}
                    <View style={{ flex: 1 }}>
                      <Text style={styles.reorderTitle} numberOfLines={1}>
                        {track.title}
                      </Text>
                      <Text style={styles.reorderArtist} numberOfLines={1}>
                        {trackArtistDisplay(track)}
                      </Text>
                    </View>
                    <View style={styles.reorderTools}>
                      <Pressable
                        onPress={() => void handleMovePlaylistTrack(idx, idx - 1)}
                        disabled={idx === 0}
                        hitSlop={6}
                        style={[styles.reorderToolBtn, idx === 0 && styles.toolDisabled]}
                      >
                        <Ionicons name="chevron-up" size={17} color="#FFFFFF" />
                      </Pressable>
                      <Pressable
                        onPress={() => void handleMovePlaylistTrack(idx, idx + 1)}
                        disabled={idx + 1 >= selectedTracks.length}
                        hitSlop={6}
                        style={[
                          styles.reorderToolBtn,
                          idx + 1 >= selectedTracks.length && styles.toolDisabled,
                        ]}
                      >
                        <Ionicons name="chevron-down" size={17} color="#FFFFFF" />
                      </Pressable>
                      <Pressable
                        onPress={() => void handleRemoveTrackAt(idx, track.title)}
                        hitSlop={6}
                        style={styles.reorderToolBtn}
                      >
                        <Ionicons name="trash-outline" size={15} color={LumeTheme.rose} />
                      </Pressable>
                    </View>
                  </View>
                </View>
              ) : (
                <View style={styles.detailRowPad}>
                  <TrackRow
                    track={track}
                    rank={idx + 1}
                    isActive={currentTrack?.uid === track.uid}
                    isOffline={offlineUids.has(track.uid)}
                    isDownloading={downloadingUids.has(track.uid)}
                    onPress={() => void playTrack(track, sortedDetailTracks)}
                    onDownloadPress={
                      activeView === 'local' ? undefined : () => void downloadOffline(track)
                    }
                    onLikePress={() => void toggleLikeTrack(track)}
                  />
                </View>
              )
            }
          />
          </Animated.View>
        </SafeAreaView>
      </AmbientBackdrop>
    );
  }

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.container} edges={['top']}>
        <Animated.View entering={FadeIn.duration(180)} style={styles.container}>
        <ScrollView contentContainerStyle={styles.scrollContent} showsVerticalScrollIndicator={false}>
          {/* Header with Desktop-style + & Import quick actions */}
          <View style={styles.topRow}>
            <View>
              <Text style={styles.title}>Library</Text>
              <Text style={styles.subtitle}>
                {playlists.length} playlist • {likedTracks.length} disukai
              </Text>
            </View>
            <View style={styles.headerActions}>
              <Pressable
                onPress={() => setShowImportModal(true)}
                style={styles.importHeaderPill}
                accessibilityLabel="Import playlist or audio"
              >
                <Ionicons name="link-outline" size={16} color="#FFFFFF" />
                <Text style={styles.importHeaderText}>Import</Text>
              </Pressable>
              <Pressable
                onPress={() => setShowCreateModal(true)}
                style={styles.createHeaderCircle}
                accessibilityLabel="Create playlist"
              >
                <Ionicons name="add" size={21} color={LumeTheme.inkDark} />
              </Pressable>
              <Pressable
                onPress={() => router.push('/settings')}
                style={styles.iconCircle}
                accessibilityLabel="Settings"
              >
                <Ionicons name="settings-outline" size={18} color="#FFFFFF" />
              </Pressable>
            </View>
          </View>

          <BatchDownloadBanner />

          {busyMessage && (
            <View style={styles.busyRow}>
              <ActivityIndicator size="small" color="#FFFFFF" />
              <Text style={styles.busyText}>{busyMessage}</Text>
            </View>
          )}

          {/* Hero Quick Cards */}
          <View style={styles.quickGrid}>
            <Pressable
              onPress={() => {
                setSortPref(savedSorts['__liked__'] ?? DEFAULT_SORT);
                setActiveView('liked');
              }}
              style={({ pressed }) => [styles.likedCard, pressed && styles.cardPressed]}
            >
              <View style={styles.likedIcon}>
                <Ionicons name="heart" size={20} color="#FFFFFF" />
              </View>
              <Text style={styles.cardTitle}>Liked Songs</Text>
              <Text style={styles.cardSub}>{likedTracks.length} lagu</Text>
            </Pressable>

            <Pressable
              onPress={() => {
                setSortPref(savedSorts['__local__'] ?? DEFAULT_SORT);
                setActiveView('local');
              }}
              style={({ pressed }) => [styles.localCard, pressed && styles.cardPressed]}
            >
              <View style={styles.localIcon}>
                <Ionicons name="folder-open" size={20} color="#FFFFFF" />
              </View>
              <Text style={styles.cardTitle}>Local Files</Text>
              <Text style={styles.cardSub}>{localTracks.length} file HP</Text>
            </Pressable>
          </View>

          {/* Desktop-style PLAYLISTS + 🔗 Section */}
          <View style={styles.section}>
            <View style={styles.sectionHeader}>
              <Text style={styles.sectionLabel}>PLAYLISTS</Text>
              <View style={styles.sectionTools}>
                {orderedPlaylists.length > 1 && (
                  <Pressable
                    onPress={() => setReorderingPlaylists((r) => !r)}
                    style={[
                      styles.sectionToolBtn,
                      reorderingPlaylists && styles.sectionToolBtnActive,
                    ]}
                    hitSlop={6}
                    accessibilityLabel="Atur urutan playlist"
                  >
                    <Ionicons
                      name="swap-vertical"
                      size={15}
                      color={reorderingPlaylists ? LumeTheme.inkDark : '#FFFFFF'}
                    />
                  </Pressable>
                )}
                <Pressable
                  onPress={() => setShowCreateModal(true)}
                  style={styles.sectionToolBtn}
                  hitSlop={6}
                >
                  <Ionicons name="add" size={18} color="#FFFFFF" />
                </Pressable>
                <Pressable
                  onPress={() => setShowImportModal(true)}
                  style={styles.sectionToolBtn}
                  hitSlop={6}
                >
                  <Ionicons name="link" size={16} color="#FFFFFF" />
                </Pressable>
              </View>
            </View>

            {orderedPlaylists.length === 0 ? (
              <View style={styles.emptyBox}>
                <Ionicons name="albums-outline" size={32} color={LumeTheme.textMuted} />
                <Text style={styles.emptyDetail}>
                  Belum ada playlist. Ketuk + untuk buat baru atau Import dari Spotify / YouTube / CSV.
                </Text>
                <View style={styles.emptyBtnRow}>
                  <Pressable
                    onPress={() => setShowCreateModal(true)}
                    style={styles.crystalPillSmall}
                  >
                    <Ionicons name="add" size={15} color={LumeTheme.inkDark} />
                    <Text style={styles.crystalPillSmallText}>Buat Playlist</Text>
                  </Pressable>
                  <Pressable
                    onPress={() => setShowImportModal(true)}
                    style={styles.glassPillSmall}
                  >
                    <Ionicons name="link-outline" size={15} color="#FFFFFF" />
                    <Text style={styles.glassPillSmallText}>Import</Text>
                  </Pressable>
                </View>
              </View>
            ) : (
              <View style={styles.playlistList}>
                {orderedPlaylists.map((pl, idx) => {
                  const badge = playlistOriginBadge(pl.origin);
                  return (
                    <Pressable
                      key={pl.id}
                      onPress={() => {
                        if (!reorderingPlaylists) void openPlaylist(pl);
                      }}
                      style={({ pressed }) => [styles.playlistRow, pressed && styles.cardPressed]}
                    >
                      <View style={styles.plIcon}>
                        <Ionicons
                          name="musical-notes"
                          size={19}
                          color={badge === 'SP' ? LumeTheme.spotify : '#FFFFFF'}
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
                      {reorderingPlaylists ? (
                        <View style={styles.reorderTools}>
                          <Pressable
                            onPress={(e) => {
                              e.stopPropagation();
                              void movePlaylistOrder(idx, idx - 1);
                            }}
                            disabled={idx === 0}
                            hitSlop={6}
                            style={[styles.reorderToolBtn, idx === 0 && styles.toolDisabled]}
                          >
                            <Ionicons name="chevron-up" size={17} color="#FFFFFF" />
                          </Pressable>
                          <Pressable
                            onPress={(e) => {
                              e.stopPropagation();
                              void movePlaylistOrder(idx, idx + 1);
                            }}
                            disabled={idx + 1 >= orderedPlaylists.length}
                            hitSlop={6}
                            style={[
                              styles.reorderToolBtn,
                              idx + 1 >= orderedPlaylists.length && styles.toolDisabled,
                            ]}
                          >
                            <Ionicons name="chevron-down" size={17} color="#FFFFFF" />
                          </Pressable>
                        </View>
                      ) : (
                        <>
                          <Pressable
                            onPress={(e) => {
                              e.stopPropagation();
                              void handleDownloadPlaylistById(pl);
                            }}
                            hitSlop={8}
                            style={styles.plDownloadBtn}
                            accessibilityLabel={`Download playlist ${pl.name}`}
                          >
                            <Ionicons name="cloud-download-outline" size={17} color="#FFFFFF" />
                          </Pressable>
                          <Ionicons name="chevron-forward" size={17} color={LumeTheme.textMuted} />
                        </>
                      )}
                    </Pressable>
                  );
                })}
              </View>
            )}
          </View>
        </ScrollView>

        {/* Create Playlist Bottom Sheet Modal */}
        <Modal
          visible={showCreateModal}
          transparent
          animationType="fade"
          onRequestClose={() => setShowCreateModal(false)}
        >
          <KeyboardAvoidingView
            behavior={Platform.OS === 'ios' ? 'padding' : undefined}
            style={styles.modalBackdrop}
          >
            <Pressable
              style={StyleSheet.absoluteFill}
              onPress={() => setShowCreateModal(false)}
            />
            <BlurView intensity={75} tint="dark" style={styles.modalSheet}>
              <View style={styles.modalHandle} />
              <Text style={styles.modalTitle}>Buat Playlist Baru</Text>
              <Text style={styles.modalSub}>
                Beri nama koleksi lagu kamu (bisa pakai emoji juga).
              </Text>
              <TextInput
                value={newPlaylistName}
                onChangeText={setNewPlaylistName}
                placeholder="Contoh: Roadtrip Malam"
                placeholderTextColor={LumeTheme.textMuted}
                autoFocus
                returnKeyType="done"
                onSubmitEditing={() => void handleCreatePlaylist()}
                style={styles.modalInput}
              />
              <View style={styles.modalBtnRow}>
                <Pressable
                  onPress={() => setShowCreateModal(false)}
                  style={styles.modalCancelBtn}
                >
                  <Text style={styles.modalCancelText}>Batal</Text>
                </Pressable>
                <Pressable
                  onPress={() => void handleCreatePlaylist()}
                  style={styles.modalConfirmBtn}
                >
                  <Text style={styles.btnTextDark}>Buat</Text>
                </Pressable>
              </View>
            </BlurView>
          </KeyboardAvoidingView>
        </Modal>

        {/* Import Hub Bottom Sheet Modal */}
        <Modal
          visible={showImportModal}
          transparent
          animationType="slide"
          onDismiss={flushPendingImportAction}
          onRequestClose={() => setShowImportModal(false)}
        >
          <KeyboardAvoidingView
            behavior={Platform.OS === 'ios' ? 'padding' : undefined}
            style={styles.modalBackdrop}
          >
            <Pressable
              style={StyleSheet.absoluteFill}
              onPress={() => setShowImportModal(false)}
            />
            <BlurView intensity={80} tint="dark" style={styles.modalSheet}>
              <View style={styles.modalHandle} />
              <Text style={styles.modalTitle}>Import ke Library</Text>
              <Text style={styles.modalSub}>
                Tempel tautan Spotify / YouTube Music, atau pilih file CSV & audio dari iPhone.
              </Text>

              <View style={styles.inlineForm}>
                <TextInput
                  value={importUrl}
                  onChangeText={setImportUrl}
                  placeholder="Tempel URL Spotify / YouTube Music..."
                  placeholderTextColor={LumeTheme.textMuted}
                  autoCapitalize="none"
                  autoCorrect={false}
                  style={styles.input}
                />
                <Pressable
                  onPress={() => void handleImportUrl()}
                  style={styles.crystalPrimaryBtn}
                >
                  <Text style={styles.btnTextDark}>Import</Text>
                </Pressable>
              </View>

              <View style={styles.modalDivider} />

              <Pressable
                onPress={() => void handleImportCsv()}
                style={({ pressed }) => [styles.importOptionRow, pressed && styles.cardPressed]}
              >
                <View style={styles.importOptionIcon}>
                  <Ionicons name="document-text-outline" size={20} color="#FFFFFF" />
                </View>
                <View style={{ flex: 1 }}>
                  <Text style={styles.importOptionTitle}>Import Exportify .CSV</Text>
                  <Text style={styles.importOptionSub}>
                    Bisa pilih banyak file CSV sekaligus
                  </Text>
                </View>
                <Ionicons name="chevron-forward" size={18} color={LumeTheme.textMuted} />
              </Pressable>

              <Pressable
                onPress={() => void handleImportLocalAudio()}
                style={({ pressed }) => [styles.importOptionRow, pressed && styles.cardPressed]}
              >
                <View style={styles.importOptionIcon}>
                  <Ionicons name="musical-note-outline" size={20} color="#FFFFFF" />
                </View>
                <View style={{ flex: 1 }}>
                  <Text style={styles.importOptionTitle}>Import Audio Lokal (MP3 / M4A)</Text>
                  <Text style={styles.importOptionSub}>
                    Tambahkan file musik langsung dari penyimpanan HP
                  </Text>
                </View>
                <Ionicons name="chevron-forward" size={18} color={LumeTheme.textMuted} />
              </Pressable>
            </BlurView>
          </KeyboardAvoidingView>
        </Modal>
        </Animated.View>
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
    paddingTop: 10,
    paddingBottom: 170,
    gap: 18,
  },
  topRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 2,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 28,
    fontWeight: '800',
    letterSpacing: -0.6,
  },
  subtitle: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    marginTop: 2,
  },
  headerActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  importHeaderPill: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    height: 36,
    paddingHorizontal: 12,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
  },
  importHeaderText: {
    color: '#FFFFFF',
    fontSize: 12.5,
    fontWeight: '700',
  },
  createHeaderCircle: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  iconCircle: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  iconCircleActive: {
    backgroundColor: '#FFFFFF',
    borderColor: '#FFFFFF',
  },
  quickGrid: {
    flexDirection: 'row',
    gap: 12,
  },
  likedCard: {
    flex: 1,
    padding: 15,
    borderRadius: 20,
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.34)',
    gap: 4,
  },
  localCard: {
    flex: 1,
    padding: 15,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.15)',
    gap: 4,
  },
  cardPressed: {
    opacity: 0.82,
  },
  likedIcon: {
    width: 40,
    height: 40,
    borderRadius: 13,
    backgroundColor: LumeTheme.rose,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 8,
  },
  localIcon: {
    width: 40,
    height: 40,
    borderRadius: 13,
    backgroundColor: 'rgba(255, 255, 255, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 8,
  },
  section: {
    gap: 10,
  },
  sectionHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 4,
  },
  sectionLabel: {
    color: LumeTheme.textMuted,
    fontSize: 12,
    fontWeight: '800',
    letterSpacing: 1.1,
  },
  sectionTools: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
  },
  sectionToolBtn: {
    width: 30,
    height: 30,
    borderRadius: 10,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  sectionToolBtnActive: {
    backgroundColor: '#FFFFFF',
    borderColor: '#FFFFFF',
  },
  reorderBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    marginHorizontal: 16,
    marginBottom: 12,
    paddingHorizontal: 13,
    paddingVertical: 9,
    borderRadius: 14,
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.20)',
  },
  reorderBannerText: {
    flex: 1,
    color: '#FFFFFF',
    fontSize: 12.5,
    fontWeight: '600',
  },
  reorderDoneBtn: {
    paddingHorizontal: 12,
    paddingVertical: 5,
    borderRadius: 999,
    backgroundColor: '#FFFFFF',
  },
  reorderDoneText: {
    color: LumeTheme.inkDark,
    fontSize: 12,
    fontWeight: '800',
  },
  reorderRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
    paddingVertical: 8,
    paddingHorizontal: 10,
    borderRadius: 12,
  },
  reorderRank: {
    width: 22,
    color: LumeTheme.textMuted,
    fontSize: 11.5,
    fontWeight: '700',
    fontVariant: ['tabular-nums'],
    textAlign: 'center',
  },
  reorderArt: {
    width: 40,
    height: 40,
    borderRadius: 9,
  },
  reorderArtFallback: {
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  reorderTitle: {
    color: '#FFFFFF',
    fontSize: 14,
    fontWeight: '600',
  },
  reorderArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 11.5,
    marginTop: 2,
  },
  reorderTools: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
  },
  reorderToolBtn: {
    width: 30,
    height: 30,
    borderRadius: 9,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  toolDisabled: {
    opacity: 0.28,
  },
  playlistList: {
    gap: 8,
  },
  playlistRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    padding: 12,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.07)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.13)',
  },
  plIcon: {
    width: 44,
    height: 44,
    borderRadius: 12,
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
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
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
    paddingHorizontal: 6,
    paddingVertical: 1,
    borderRadius: 5,
  },
  originBadgeText: {
    color: '#FFFFFF',
    fontSize: 9,
    fontWeight: '800',
  },
  plDownloadBtn: {
    width: 34,
    height: 34,
    borderRadius: 11,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  cardTitle: {
    color: '#FFFFFF',
    fontSize: 15,
    fontWeight: '700',
  },
  cardSub: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
  },
  detailHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 14,
    paddingTop: 8,
    paddingBottom: 6,
  },
  detailActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  backBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 3,
    paddingVertical: 6,
  },
  backText: {
    color: '#FFFFFF',
    fontSize: 15,
    fontWeight: '600',
  },
  detailScroll: {
    paddingBottom: 170,
  },
  detailRowPad: {
    paddingHorizontal: 14,
  },
  detailHero: {
    paddingHorizontal: 18,
    paddingTop: 8,
    paddingBottom: 14,
    gap: 6,
  },
  detailTitle: {
    color: '#FFFFFF',
    fontSize: 27,
    fontWeight: '800',
    letterSpacing: -0.5,
  },
  detailSub: {
    color: LumeTheme.textSecondary,
    fontSize: 13,
    fontWeight: '500',
  },
  heroBtnRow: {
    flexDirection: 'row',
    gap: 8,
    marginTop: 10,
  },
  heroPlayBtn: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
    height: 42,
    borderRadius: 999,
    backgroundColor: '#FFFFFF',
  },
  heroPlayText: {
    color: LumeTheme.inkDark,
    fontSize: 13.5,
    fontWeight: '800',
  },
  heroGlassBtn: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
    height: 42,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
  },
  heroGlassText: {
    color: '#FFFFFF',
    fontSize: 13,
    fontWeight: '700',
  },
  crystalPillSmall: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    paddingHorizontal: 13,
    height: 34,
    borderRadius: 999,
    backgroundColor: '#FFFFFF',
  },
  crystalPillSmallText: {
    color: LumeTheme.inkDark,
    fontSize: 12.5,
    fontWeight: '800',
  },
  glassPillSmall: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    paddingHorizontal: 13,
    height: 34,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
  },
  glassPillSmallText: {
    color: '#FFFFFF',
    fontSize: 12.5,
    fontWeight: '700',
  },
  sortRow: {
    gap: 8,
    paddingHorizontal: 18,
    paddingBottom: 12,
    alignItems: 'center',
  },
  dirChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    paddingHorizontal: 11,
    paddingVertical: 6,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.24)',
  },
  dirChipText: {
    color: '#FFFFFF',
    fontSize: 12,
    fontWeight: '800',
  },
  sortChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    paddingHorizontal: 13,
    paddingVertical: 6,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.12)',
  },
  sortChipActive: {
    backgroundColor: '#FFFFFF',
    borderColor: '#FFFFFF',
  },
  sortText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontWeight: '700',
  },
  sortTextActive: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
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
  emptyBox: {
    padding: 26,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.12)',
    alignItems: 'center',
    gap: 10,
    marginHorizontal: 2,
  },
  emptyDetail: {
    color: LumeTheme.textSecondary,
    textAlign: 'center',
    fontSize: 13.5,
    lineHeight: 20,
  },
  emptyBtnRow: {
    flexDirection: 'row',
    gap: 8,
    marginTop: 4,
  },
  busyRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    paddingHorizontal: 14,
    paddingVertical: 10,
    borderRadius: 14,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
  },
  busyText: {
    color: '#FFFFFF',
    fontSize: 13,
    fontWeight: '600',
  },
  inlineForm: {
    flexDirection: 'row',
    gap: 8,
  },
  input: {
    flex: 1,
    backgroundColor: 'rgba(0, 0, 0, 0.36)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    borderRadius: 14,
    paddingHorizontal: 13,
    height: 44,
    color: '#FFFFFF',
    fontSize: 14,
  },
  crystalPrimaryBtn: {
    paddingHorizontal: 16,
    height: 44,
    borderRadius: 14,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  btnTextDark: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
    fontSize: 13.5,
  },
  modalBackdrop: {
    flex: 1,
    backgroundColor: 'rgba(0, 0, 0, 0.62)',
    justifyContent: 'flex-end',
  },
  modalSheet: {
    backgroundColor: 'rgba(16, 20, 30, 0.90)',
    borderTopLeftRadius: 26,
    borderTopRightRadius: 26,
    paddingHorizontal: 20,
    paddingTop: 10,
    paddingBottom: 38,
    borderTopWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
    overflow: 'hidden',
    gap: 12,
  },
  modalHandle: {
    width: 40,
    height: 4,
    borderRadius: 2,
    backgroundColor: 'rgba(255, 255, 255, 0.24)',
    alignSelf: 'center',
    marginBottom: 4,
  },
  modalTitle: {
    color: '#FFFFFF',
    fontSize: 19,
    fontWeight: '800',
  },
  modalSub: {
    color: LumeTheme.textSecondary,
    fontSize: 13,
    lineHeight: 18,
    marginTop: -4,
  },
  modalInput: {
    backgroundColor: 'rgba(0, 0, 0, 0.38)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
    borderRadius: 14,
    paddingHorizontal: 14,
    height: 46,
    color: '#FFFFFF',
    fontSize: 15,
  },
  modalBtnRow: {
    flexDirection: 'row',
    gap: 10,
    marginTop: 4,
  },
  modalCancelBtn: {
    flex: 1,
    height: 44,
    borderRadius: 14,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  modalCancelText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 14,
  },
  modalConfirmBtn: {
    flex: 1,
    height: 44,
    borderRadius: 14,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  modalDivider: {
    height: StyleSheet.hairlineWidth,
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
    marginVertical: 2,
  },
  importOptionRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    padding: 13,
    borderRadius: 16,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
  },
  importOptionIcon: {
    width: 40,
    height: 40,
    borderRadius: 12,
    backgroundColor: 'rgba(255, 255, 255, 0.12)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  importOptionTitle: {
    color: '#FFFFFF',
    fontSize: 14.5,
    fontWeight: '700',
  },
  importOptionSub: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    marginTop: 2,
  },
});
