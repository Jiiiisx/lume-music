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
import type { ArtistProfile } from '../../src/types';

export default function ArtistScreen() {
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

  const [profile, setProfile] = useState<ArtistProfile | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    setLoading(true);
    LumeCore.getArtistProfile(id)
      .then(setProfile)
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
        ) : error || !profile ? (
          <Text style={styles.error}>{error ?? 'Profil artis tidak ditemukan.'}</Text>
        ) : (
          <ScrollView contentContainerStyle={styles.content}>
            <View style={styles.hero}>
              {profile.avatar_url ? (
                <Image source={{ uri: profile.avatar_url }} style={styles.avatar} />
              ) : (
                <View style={[styles.avatar, styles.fallback]}>
                  <Ionicons name="person" size={44} color={palette.accent} />
                </View>
              )}
              <Text style={styles.name}>{profile.name}</Text>
              {profile.subscriber_label && (
                <Text style={styles.sub}>{profile.subscriber_label}</Text>
              )}
            </View>

            {profile.top_tracks.length > 0 && (
              <View style={styles.section}>
                <Text style={styles.sectionTitle}>Lagu Populer</Text>
                {profile.top_tracks.map((track) => (
                  <TrackRow
                    key={track.uid}
                    track={track}
                    isActive={currentTrack?.uid === track.uid}
                    isOffline={offlineUids.has(track.uid)}
                    isDownloading={downloadingUids.has(track.uid)}
                    onPress={() => void playTrack(track, profile.top_tracks)}
                    onDownloadPress={() => void downloadOffline(track)}
                    onLikePress={() => void toggleLikeTrack(track)}
                  />
                ))}
              </View>
            )}

            {[
              { title: 'Albums', items: profile.albums },
              { title: 'Singles & EP', items: profile.singles },
            ].map(
              (group) =>
                group.items.length > 0 && (
                  <View key={group.title} style={styles.section}>
                    <Text style={styles.sectionTitle}>{group.title}</Text>
                    <ScrollView
                      horizontal
                      showsHorizontalScrollIndicator={false}
                      contentContainerStyle={styles.hRow}
                    >
                      {group.items.map((album) => (
                        <Pressable
                          key={album.id}
                          onPress={() =>
                            router.push({ pathname: '/album/[id]', params: { id: album.id } })
                          }
                          style={styles.albumCard}
                        >
                          {album.cover_url ? (
                            <Image source={{ uri: album.cover_url }} style={styles.albumCover} />
                          ) : (
                            <View style={[styles.albumCover, styles.fallback]}>
                              <Ionicons name="disc" size={32} color={palette.accent} />
                            </View>
                          )}
                          <Text style={styles.albumTitle} numberOfLines={1}>
                            {album.title}
                          </Text>
                          {album.year && <Text style={styles.albumYear}>{album.year}</Text>}
                        </Pressable>
                      ))}
                    </ScrollView>
                  </View>
                )
            )}
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
  hero: { alignItems: 'center', paddingVertical: 16, gap: 8 },
  avatar: { width: 120, height: 120, borderRadius: 60 },
  fallback: { backgroundColor: LumeTheme.bgElevated, alignItems: 'center', justifyContent: 'center' },
  name: { color: '#FFFFFF', fontSize: 25, fontWeight: '800', textAlign: 'center' },
  sub: { color: LumeTheme.textSecondary, fontSize: 13 },
  section: { marginTop: 18 },
  sectionTitle: { color: '#FFFFFF', fontSize: 18, fontWeight: '700', paddingHorizontal: 6, marginBottom: 10 },
  hRow: { gap: 12, paddingHorizontal: 6 },
  albumCard: { width: 132, gap: 5 },
  albumCover: { width: 132, height: 132, borderRadius: 14 },
  albumTitle: { color: '#FFFFFF', fontSize: 13, fontWeight: '700' },
  albumYear: { color: LumeTheme.textSecondary, fontSize: 11.5 },
});
