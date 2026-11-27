import React, { useEffect, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  Image,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useLocalSearchParams, useRouter } from 'expo-router';
import { LumeCore } from '../../modules/lume-core';
import { AmbientBackdrop } from '../../src/components/AmbientBackdrop';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { LumeTheme } from '../../src/theme';
import type { RemotePlaylistDetail } from '../../src/types';

export default function RemotePlaylistScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
  const router = useRouter();
  const {
    currentTrack,
    palette,
    offlineUids,
    downloadingUids,
    playTrack,
    downloadOffline,
    downloadPlaylistOffline,
    toggleLikeTrack,
    refreshLibraryState,
  } = usePlayer();

  const [detail, setDetail] = useState<RemotePlaylistDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    LumeCore.getYtmPlaylistDetail(id)
      .then(setDetail)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, [id]);

  const handleSaveToLibrary = async () => {
    if (!detail) return;
    setSaving(true);
    try {
      const saved = await LumeCore.importYtmPlaylistUrl(detail.id);
      await refreshLibraryState();
      Alert.alert('Tersimpan', `Playlist "${saved.name}" (${saved.track_count} lagu) masuk ke Library.`);
    } catch (e) {
      Alert.alert('Gagal', e instanceof Error ? e.message : String(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.root} edges={['top']}>
        <View style={styles.topBar}>
          <Pressable onPress={() => router.back()} style={styles.backBtn}>
            <Ionicons name="chevron-back" size={24} color="#FFFFFF" />
            <Text style={styles.backText}>Kembali</Text>
          </Pressable>
        </View>

        {loading ? (
          <ActivityIndicator size="large" color={palette.accent} style={{ marginTop: 64 }} />
        ) : error || !detail ? (
          <Text style={styles.error}>{error ?? 'Playlist tidak ditemukan.'}</Text>
        ) : (
          <ScrollView contentContainerStyle={styles.content}>
            <View style={styles.hero}>
              {detail.cover_url ? (
                <Image source={{ uri: detail.cover_url }} style={styles.cover} />
              ) : (
                <View style={[styles.cover, styles.fallback]}>
                  <Ionicons name="musical-notes" size={56} color={palette.accent} />
                </View>
              )}
              <Text style={styles.title}>{detail.name}</Text>
              <Text style={styles.sub}>
                {detail.owner ?? 'YouTube Music'} • {detail.tracks.length} lagu
              </Text>
              <View style={styles.btnRow}>
                {detail.tracks.length > 0 && (
                  <Pressable
                    onPress={() => void playTrack(detail.tracks[0], detail.tracks)}
                    style={[styles.playBtn, { backgroundColor: palette.accent }]}
                  >
                    <Ionicons name="play" size={18} color="#0A0C12" />
                    <Text style={styles.playBtnText}>Putar</Text>
                  </Pressable>
                )}
                <Pressable
                  onPress={() => void handleSaveToLibrary()}
                  style={styles.saveBtn}
                  disabled={saving}
                >
                  <Ionicons name="bookmark-outline" size={17} color="#FFFFFF" />
                  <Text style={styles.saveBtnText}>
                    {saving ? 'Menyimpan...' : 'Simpan'}
                  </Text>
                </Pressable>
                {detail.tracks.length > 0 && (
                  <Pressable
                    onPress={() => void downloadPlaylistOffline(detail.name, detail.tracks)}
                    style={styles.saveBtn}
                  >
                    <Ionicons name="cloud-download-outline" size={17} color="#FFFFFF" />
                    <Text style={styles.saveBtnText}>Offline</Text>
                  </Pressable>
                )}
              </View>
            </View>

            {detail.tracks.map((track) => (
              <TrackRow
                key={track.uid}
                track={track}
                isActive={currentTrack?.uid === track.uid}
                isOffline={offlineUids.has(track.uid)}
                isDownloading={downloadingUids.has(track.uid)}
                onPress={() => void playTrack(track, detail.tracks)}
                onDownloadPress={() => void downloadOffline(track)}
                onLikePress={() => void toggleLikeTrack(track)}
              />
            ))}
          </ScrollView>
        )}
      </SafeAreaView>
    </AmbientBackdrop>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1 },
  topBar: { paddingHorizontal: 14, paddingVertical: 8 },
  backBtn: { flexDirection: 'row', alignItems: 'center', gap: 4 },
  backText: { color: '#FFFFFF', fontSize: 15, fontWeight: '600' },
  error: { color: LumeTheme.rose, textAlign: 'center', marginTop: 40, paddingHorizontal: 24 },
  content: { paddingHorizontal: 12, paddingBottom: 140 },
  hero: { alignItems: 'center', paddingVertical: 14, gap: 8, marginBottom: 12 },
  cover: { width: 190, height: 190, borderRadius: 20 },
  fallback: { backgroundColor: LumeTheme.bgElevated, alignItems: 'center', justifyContent: 'center' },
  title: { color: '#FFFFFF', fontSize: 22, fontWeight: '800', textAlign: 'center' },
  sub: { color: LumeTheme.textSecondary, fontSize: 13 },
  btnRow: { flexDirection: 'row', gap: 10, marginTop: 6 },
  playBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    paddingHorizontal: 20,
    paddingVertical: 11,
    borderRadius: 999,
  },
  playBtnText: { color: '#0A0C12', fontWeight: '800', fontSize: 14 },
  saveBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    backgroundColor: 'rgba(255,255,255,0.14)',
    paddingHorizontal: 18,
    paddingVertical: 11,
    borderRadius: 999,
  },
  saveBtnText: { color: '#FFFFFF', fontWeight: '700', fontSize: 14 },
});
