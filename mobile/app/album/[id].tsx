import React, { useEffect, useState } from 'react';
import {
  ActivityIndicator,
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
import type { AlbumDetail } from '../../src/types';

export default function AlbumScreen() {
  const { id } = useLocalSearchParams<{ id: string }>();
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

  const [album, setAlbum] = useState<AlbumDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    LumeCore.getAlbumDetail(id)
      .then(setAlbum)
      .catch((e) => setError(e instanceof Error ? e.message : String(e)))
      .finally(() => setLoading(false));
  }, [id]);

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
        ) : error || !album ? (
          <Text style={styles.error}>{error ?? 'Album tidak ditemukan.'}</Text>
        ) : (
          <ScrollView contentContainerStyle={styles.content}>
            <View style={styles.hero}>
              {album.cover_url ? (
                <Image source={{ uri: album.cover_url }} style={styles.cover} />
              ) : (
                <View style={[styles.cover, styles.fallback]}>
                  <Ionicons name="disc" size={56} color={palette.accent} />
                </View>
              )}
              <Text style={styles.title}>{album.title}</Text>
              <Text style={styles.sub}>
                {album.artist_name}
                {album.year ? ` • ${album.year}` : ''} • {album.tracks.length} lagu
              </Text>
              {album.tracks.length > 0 && (
                <Pressable
                  onPress={() => void playTrack(album.tracks[0], album.tracks)}
                  style={[styles.playBtn, { backgroundColor: palette.accent }]}
                >
                  <Ionicons name="play" size={18} color="#0A0C12" />
                  <Text style={styles.playBtnText}>Putar Album</Text>
                </Pressable>
              )}
            </View>

            {album.tracks.map((track) => (
              <TrackRow
                key={track.uid}
                track={track}
                isActive={currentTrack?.uid === track.uid}
                isOffline={offlineUids.has(track.uid)}
                isDownloading={downloadingUids.has(track.uid)}
                onPress={() => void playTrack(track, album.tracks)}
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
  playBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    paddingHorizontal: 22,
    paddingVertical: 11,
    borderRadius: 999,
    marginTop: 6,
  },
  playBtnText: { color: '#0A0C12', fontWeight: '800', fontSize: 14 },
});
