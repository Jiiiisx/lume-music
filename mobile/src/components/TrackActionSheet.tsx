import React, { useState } from 'react';
import {
  Alert,
  Image,
  Modal,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { LumeCore } from '../../modules/lume-core';
import { usePlayer } from '../context/PlayerContext';
import { LumeTheme } from '../theme';
import { isPlaylistEditable, trackArtistDisplay } from '../types';

export function TrackActionSheet() {
  const {
    menuTrack,
    closeTrackMenu,
    playlists,
    likedUids,
    offlineUids,
    playTrack,
    playNext,
    addToQueue,
    toggleLikeTrack,
    downloadOffline,
    removeOffline,
    refreshLibraryState,
  } = usePlayer();

  const [mode, setMode] = useState<'actions' | 'playlist' | 'rematch'>('actions');
  const [rematchInput, setRematchInput] = useState('');

  if (!menuTrack) return null;

  const isLiked = likedUids.has(menuTrack.uid);
  const isOffline = offlineUids.has(menuTrack.uid);
  const isSpotify = 'Spotify' in menuTrack.source;
  const editablePlaylists = playlists.filter((p) => isPlaylistEditable(p.origin));

  const dismiss = () => {
    setMode('actions');
    setRematchInput('');
    closeTrackMenu();
  };

  const handleAddToPlaylist = async (playlistId: string, playlistName: string) => {
    try {
      await LumeCore.addTrackToPlaylist(playlistId, menuTrack);
      await refreshLibraryState();
      Alert.alert('Ditambahkan', `"${menuTrack.title}" masuk ke "${playlistName}".`);
      dismiss();
    } catch (err) {
      Alert.alert('Gagal', err instanceof Error ? err.message : String(err));
    }
  };

  const handleRematch = async () => {
    if (!('Spotify' in menuTrack.source)) return;
    const raw = rematchInput.trim();
    if (!raw) {
      // Force auto-rematch via Smart YTM Matcher.
      dismiss();
      await playTrack(menuTrack, undefined, true);
      return;
    }
    let videoId = raw;
    const match = raw.match(/(?:v=|youtu\.be\/)([A-Za-z0-9_-]{11})/);
    if (match?.[1]) videoId = match[1];
    try {
      await LumeCore.rematchSpotifyTrack(menuTrack.source.Spotify.id, videoId);
      Alert.alert('Re-match Berhasil', `Dipetakan ke YouTube ID: ${videoId}`);
      dismiss();
      await playTrack(menuTrack);
    } catch (err) {
      Alert.alert('Gagal Re-match', err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <Modal
      visible
      transparent
      animationType="slide"
      onRequestClose={dismiss}
    >
      <Pressable style={styles.backdrop} onPress={dismiss}>
        <Pressable style={styles.sheet} onPress={(e) => e.stopPropagation()}>
          <View style={styles.handle} />

          <View style={styles.trackHeader}>
            {menuTrack.artwork_ref ? (
              <Image source={{ uri: menuTrack.artwork_ref }} style={styles.art} />
            ) : (
              <View style={[styles.art, styles.artFallback]}>
                <Ionicons name="musical-note" size={20} color={LumeTheme.accentBright} />
              </View>
            )}
            <View style={{ flex: 1 }}>
              <Text style={styles.trackTitle} numberOfLines={1}>
                {menuTrack.title}
              </Text>
              <Text style={styles.trackArtist} numberOfLines={1}>
                {trackArtistDisplay(menuTrack)}
              </Text>
            </View>
          </View>

          {mode === 'actions' && (
            <View style={styles.list}>
              <Pressable
                style={styles.item}
                onPress={() => {
                  playNext(menuTrack);
                  dismiss();
                }}
              >
                <Ionicons name="play-forward-outline" size={22} color="#FFFFFF" />
                <Text style={styles.itemText}>Putar Berikutnya (Play Next)</Text>
              </Pressable>

              <Pressable
                style={styles.item}
                onPress={() => {
                  addToQueue(menuTrack);
                  dismiss();
                }}
              >
                <Ionicons name="list-outline" size={22} color="#FFFFFF" />
                <Text style={styles.itemText}>Tambahkan ke Antrean (Queue)</Text>
              </Pressable>

              <Pressable
                style={styles.item}
                onPress={() => setMode('playlist')}
              >
                <Ionicons name="add-circle-outline" size={22} color="#FFFFFF" />
                <Text style={styles.itemText}>Tambahkan ke Playlist</Text>
              </Pressable>

              <Pressable
                style={styles.item}
                onPress={async () => {
                  await toggleLikeTrack(menuTrack);
                  dismiss();
                }}
              >
                <Ionicons
                  name={isLiked ? 'heart' : 'heart-outline'}
                  size={22}
                  color={isLiked ? LumeTheme.rose : '#FFFFFF'}
                />
                <Text style={styles.itemText}>
                  {isLiked ? 'Hapus dari Liked Songs' : 'Simpan ke Liked Songs'}
                </Text>
              </Pressable>

              <Pressable
                style={styles.item}
                onPress={async () => {
                  if (isOffline) {
                    await removeOffline(menuTrack.uid);
                  } else {
                    void downloadOffline(menuTrack);
                  }
                  dismiss();
                }}
              >
                <Ionicons
                  name={isOffline ? 'trash-outline' : 'cloud-download-outline'}
                  size={22}
                  color={isOffline ? LumeTheme.rose : LumeTheme.emerald}
                />
                <Text style={styles.itemText}>
                  {isOffline ? 'Hapus File Offline' : 'Download Offline (Berkendara)'}
                </Text>
              </Pressable>

              {isSpotify && (
                <Pressable
                  style={styles.item}
                  onPress={() => setMode('rematch')}
                >
                  <Ionicons name="swap-horizontal-outline" size={22} color={LumeTheme.amber} />
                  <Text style={styles.itemText}>Re-match YouTube Track</Text>
                </Pressable>
              )}
            </View>
          )}

          {mode === 'playlist' && (
            <View style={styles.subView}>
              <Pressable onPress={() => setMode('actions')} style={styles.subBack}>
                <Ionicons name="chevron-back" size={18} color={LumeTheme.textSecondary} />
                <Text style={styles.subBackText}>Pilih Playlist</Text>
              </Pressable>
              <ScrollView style={{ maxHeight: 240 }}>
                {editablePlaylists.length === 0 ? (
                  <Text style={styles.emptySub}>
                    Belum ada playlist lokal. Buat dulu di tab Library.
                  </Text>
                ) : (
                  editablePlaylists.map((pl) => (
                    <Pressable
                      key={pl.id}
                      style={styles.item}
                      onPress={() => void handleAddToPlaylist(pl.id, pl.name)}
                    >
                      <Ionicons name="musical-notes" size={20} color={LumeTheme.accentBright} />
                      <Text style={styles.itemText}>
                        {pl.name} ({pl.track_count})
                      </Text>
                    </Pressable>
                  ))
                )}
              </ScrollView>
            </View>
          )}

          {mode === 'rematch' && (
            <View style={styles.subView}>
              <Pressable onPress={() => setMode('actions')} style={styles.subBack}>
                <Ionicons name="chevron-back" size={18} color={LumeTheme.textSecondary} />
                <Text style={styles.subBackText}>Re-match YouTube Track</Text>
              </Pressable>
              <Text style={styles.rematchDesc}>
                Kosongkan untuk cari otomatis versi lain, atau tempel link / video ID YouTube:
              </Text>
              <TextInput
                value={rematchInput}
                onChangeText={setRematchInput}
                placeholder="https://music.youtube.com/watch?v=... atau Video ID"
                placeholderTextColor={LumeTheme.textMuted}
                autoCapitalize="none"
                autoCorrect={false}
                style={styles.input}
              />
              <Pressable onPress={() => void handleRematch()} style={styles.confirmBtn}>
                <Text style={styles.confirmBtnText}>
                  {rematchInput.trim() ? 'Terapkan & Putar' : 'Auto Re-match & Putar'}
                </Text>
              </Pressable>
            </View>
          )}
        </Pressable>
      </Pressable>
    </Modal>
  );
}

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
    backgroundColor: 'rgba(0,0,0,0.62)',
    justifyContent: 'flex-end',
  },
  sheet: {
    backgroundColor: '#131724',
    borderTopLeftRadius: 24,
    borderTopRightRadius: 24,
    paddingHorizontal: 18,
    paddingBottom: 34,
    paddingTop: 10,
    borderTopWidth: 1,
    borderColor: LumeTheme.border,
  },
  handle: {
    width: 40,
    height: 4,
    borderRadius: 2,
    backgroundColor: 'rgba(255,255,255,0.22)',
    alignSelf: 'center',
    marginBottom: 14,
  },
  trackHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    paddingBottom: 14,
    borderBottomWidth: 1,
    borderBottomColor: LumeTheme.border,
    marginBottom: 8,
  },
  art: {
    width: 46,
    height: 46,
    borderRadius: 10,
  },
  artFallback: {
    backgroundColor: LumeTheme.bgElevated,
    alignItems: 'center',
    justifyContent: 'center',
  },
  trackTitle: {
    color: '#FFFFFF',
    fontSize: 16,
    fontWeight: '700',
  },
  trackArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 13,
    marginTop: 2,
  },
  list: {
    gap: 2,
  },
  item: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 14,
    paddingVertical: 13,
    paddingHorizontal: 6,
  },
  itemText: {
    color: LumeTheme.textPrimary,
    fontSize: 15,
    fontWeight: '600',
  },
  subView: {
    gap: 10,
    paddingTop: 4,
  },
  subBack: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    marginBottom: 4,
  },
  subBackText: {
    color: LumeTheme.textSecondary,
    fontSize: 14,
    fontWeight: '700',
  },
  emptySub: {
    color: LumeTheme.textSecondary,
    fontSize: 13.5,
    paddingVertical: 16,
    textAlign: 'center',
  },
  rematchDesc: {
    color: LumeTheme.textSecondary,
    fontSize: 13,
    lineHeight: 18,
  },
  input: {
    backgroundColor: LumeTheme.bgElevated,
    borderWidth: 1,
    borderColor: LumeTheme.border,
    borderRadius: 12,
    paddingHorizontal: 12,
    height: 44,
    color: '#FFFFFF',
    fontSize: 14,
  },
  confirmBtn: {
    backgroundColor: LumeTheme.accent,
    borderRadius: 12,
    height: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },
  confirmBtnText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 14,
  },
});
