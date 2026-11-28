import React, { useCallback, useState } from 'react';
import {
  ActivityIndicator,
  Image,
  Pressable,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useFocusEffect, useRouter } from 'expo-router';
import { LumeCore } from '../../modules/lume-core';
import { AmbientBackdrop } from '../../src/components/AmbientBackdrop';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { LumeTheme } from '../../src/theme';
import { type Track, trackArtistDisplay } from '../../src/types';

function getGreeting(): string {
  const hour = new Date().getHours();
  if (hour < 11) return 'Selamat pagi';
  if (hour < 15) return 'Selamat siang';
  if (hour < 19) return 'Selamat sore';
  return 'Selamat malam';
}

export default function HomeScreen() {
  const router = useRouter();
  const {
    currentTrack,
    offlineUids,
    downloadingUids,
    offlineEntries,
    errorMessage,
    playTrack,
    downloadOffline,
    toggleLikeTrack,
    refreshLibraryState,
    clearError,
  } = usePlayer();

  const [indonesia, setIndonesia] = useState(true);
  const [charts, setCharts] = useState<Track[]>([]);
  const [recent, setRecent] = useState<Track[]>([]);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const loadHome = useCallback(
    async (forceRefresh = false) => {
      if (!LumeCore.isAvailable()) return;
      if (!forceRefresh && charts.length === 0) setLoading(true);
      try {
        const [rec, chartTracks] = await Promise.all([
          LumeCore.listRecentTracks(12),
          LumeCore.getCharts(indonesia, 25, forceRefresh),
        ]);
        setRecent(rec);
        setCharts(chartTracks);
        void refreshLibraryState();
      } catch {
        // Offline tracks remain accessible even without network.
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [charts.length, indonesia, refreshLibraryState]
  );

  useFocusEffect(
    useCallback(() => {
      void loadHome(false);
    }, [loadHome])
  );

  const subtitleText =
    offlineEntries.length > 0
      ? `${getGreeting()} • ${offlineEntries.length} lagu siap offline`
      : `${getGreeting()} • YouTube Music & Offline`;

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.container} edges={['top']}>
        <ScrollView
          contentContainerStyle={styles.scrollContent}
          showsVerticalScrollIndicator={false}
          refreshControl={
            <RefreshControl
              refreshing={refreshing}
              tintColor="#FFFFFF"
              onRefresh={() => {
                setRefreshing(true);
                void loadHome(true);
              }}
            />
          }
        >
          {/* Clean Desktop-inspired Header */}
          <View style={styles.header}>
            <View style={{ flex: 1 }}>
              <Text style={styles.brand}>Lume Music</Text>
              <Text style={styles.subBrand} numberOfLines={1}>
                {subtitleText}
              </Text>
            </View>

            <View style={styles.headerActions}>
              <Pressable
                onPress={() => router.push('/queue')}
                style={({ pressed }) => [styles.iconCircle, pressed && styles.pressedGlass]}
                accessibilityLabel="Queue"
              >
                <Ionicons name="list" size={18} color="#FFFFFF" />
              </Pressable>
              <Pressable
                onPress={() => router.push('/settings')}
                style={({ pressed }) => [styles.iconCircle, pressed && styles.pressedGlass]}
                accessibilityLabel="Settings"
              >
                <Ionicons name="settings-outline" size={18} color="#FFFFFF" />
              </Pressable>
              <Pressable
                onPress={() => router.push('/driving')}
                style={({ pressed }) => [styles.driveChip, pressed && styles.pressedWhite]}
              >
                <Ionicons name="car-sport" size={14} color={LumeTheme.inkDark} />
                <Text style={styles.driveChipText}>Drive</Text>
              </Pressable>
            </View>
          </View>

          {errorMessage && (
            <Pressable onPress={clearError} style={styles.errorBanner}>
              <Ionicons name="alert-circle" size={18} color={LumeTheme.rose} />
              <Text style={styles.errorText}>{errorMessage}</Text>
            </Pressable>
          )}

          {/* Horizontal Visual Carousel for Recently Played */}
          {recent.length > 0 && (
            <View style={styles.section}>
              <View style={styles.sectionHeader}>
                <Text style={styles.sectionTitle}>Baru Diputar</Text>
                <Text style={styles.sectionMeta}>{recent.length} lagu</Text>
              </View>
              <ScrollView
                horizontal
                showsHorizontalScrollIndicator={false}
                contentContainerStyle={styles.recentCarousel}
              >
                {recent.slice(0, 10).map((track) => {
                  const isActive = currentTrack?.uid === track.uid;
                  const isOffline = offlineUids.has(track.uid);
                  return (
                    <Pressable
                      key={track.uid}
                      onPress={() => void playTrack(track, recent)}
                      style={({ pressed }) => [
                        styles.recentCard,
                        isActive && styles.recentCardActive,
                        pressed && styles.recentCardPressed,
                      ]}
                    >
                      <View style={styles.recentArtWrap}>
                        {track.artwork_ref ? (
                          <Image
                            source={{ uri: track.artwork_ref }}
                            style={styles.recentArt}
                          />
                        ) : (
                          <View style={[styles.recentArt, styles.artFallback]}>
                            <Ionicons name="musical-notes" size={32} color="rgba(255,255,255,0.6)" />
                          </View>
                        )}
                        {isOffline && (
                          <View style={styles.recentOfflineDot}>
                            <Ionicons name="checkmark-circle" size={14} color={LumeTheme.emerald} />
                          </View>
                        )}
                        {isActive && (
                          <View style={styles.recentPlayingBadge}>
                            <Ionicons name="volume-high" size={12} color={LumeTheme.inkDark} />
                          </View>
                        )}
                      </View>
                      <Text style={styles.recentTitle} numberOfLines={1}>
                        {track.title}
                      </Text>
                      <Text style={styles.recentArtist} numberOfLines={1}>
                        {trackArtistDisplay(track)}
                      </Text>
                    </Pressable>
                  );
                })}
              </ScrollView>
            </View>
          )}

          {/* Trending Charts inside Frosted Glass Surface with Ranked Rows */}
          <View style={styles.section}>
            <View style={styles.sectionHeader}>
              <Text style={styles.sectionTitle}>Trending Charts</Text>
              <View style={styles.regionToggle}>
                <Pressable
                  onPress={() => setIndonesia(true)}
                  style={[styles.regionBtn, indonesia && styles.regionBtnActive]}
                >
                  <Text style={[styles.regionText, indonesia && styles.regionTextActive]}>
                    Indonesia
                  </Text>
                </Pressable>
                <Pressable
                  onPress={() => setIndonesia(false)}
                  style={[styles.regionBtn, !indonesia && styles.regionBtnActive]}
                >
                  <Text style={[styles.regionText, !indonesia && styles.regionTextActive]}>
                    Global
                  </Text>
                </Pressable>
              </View>
            </View>

            {loading ? (
              <View style={styles.glassListBox}>
                <ActivityIndicator
                  size="large"
                  color="#FFFFFF"
                  style={{ marginVertical: 36 }}
                />
              </View>
            ) : charts.length === 0 ? (
              <View style={styles.emptyBox}>
                <Ionicons name="sparkles-outline" size={26} color={LumeTheme.textMuted} />
                <Text style={styles.emptyText}>
                  Tarik ke bawah untuk memuat chart, atau buka tab Explore / Offline.
                </Text>
              </View>
            ) : (
              <View style={styles.glassListBox}>
                {charts.map((track, idx) => (
                  <TrackRow
                    key={track.uid}
                    track={track}
                    rank={idx + 1}
                    isActive={currentTrack?.uid === track.uid}
                    isOffline={offlineUids.has(track.uid)}
                    isDownloading={downloadingUids.has(track.uid)}
                    onPress={() => void playTrack(track, charts)}
                    onDownloadPress={() => void downloadOffline(track)}
                    onLikePress={() => void toggleLikeTrack(track)}
                  />
                ))}
              </View>
            )}
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
    paddingBottom: 170,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 18,
    marginTop: 10,
    marginBottom: 20,
    gap: 10,
  },
  brand: {
    color: '#FFFFFF',
    fontSize: 28,
    fontWeight: '800',
    letterSpacing: -0.6,
  },
  subBrand: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    marginTop: 2,
    fontWeight: '500',
  },
  headerActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
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
  pressedGlass: {
    backgroundColor: 'rgba(255, 255, 255, 0.18)',
  },
  driveChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    paddingHorizontal: 13,
    height: 36,
    borderRadius: 999,
    backgroundColor: '#FFFFFF',
  },
  pressedWhite: {
    opacity: 0.86,
  },
  driveChipText: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
    fontSize: 12.5,
  },
  errorBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    marginHorizontal: 16,
    padding: 12,
    borderRadius: 14,
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.34)',
    marginBottom: 16,
  },
  errorText: {
    color: '#FECDD3',
    fontSize: 13,
    flex: 1,
  },
  section: {
    marginBottom: 24,
  },
  sectionHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 18,
    marginBottom: 12,
  },
  sectionTitle: {
    color: '#FFFFFF',
    fontSize: 19,
    fontWeight: '800',
    letterSpacing: -0.3,
  },
  sectionMeta: {
    color: LumeTheme.textMuted,
    fontSize: 12,
    fontWeight: '600',
  },
  recentCarousel: {
    paddingHorizontal: 18,
    gap: 12,
  },
  recentCard: {
    width: 138,
    padding: 8,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.07)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.12)',
  },
  recentCardActive: {
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
    borderColor: 'rgba(255, 255, 255, 0.28)',
  },
  recentCardPressed: {
    transform: [{ scale: 0.97 }],
  },
  recentArtWrap: {
    position: 'relative',
    marginBottom: 8,
  },
  recentArt: {
    width: 120,
    height: 120,
    borderRadius: 12,
  },
  artFallback: {
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  recentOfflineDot: {
    position: 'absolute',
    top: 6,
    right: 6,
    backgroundColor: 'rgba(8, 10, 16, 0.72)',
    borderRadius: 999,
    padding: 2,
  },
  recentPlayingBadge: {
    position: 'absolute',
    bottom: 6,
    right: 6,
    width: 24,
    height: 24,
    borderRadius: 12,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  recentTitle: {
    color: '#FFFFFF',
    fontSize: 13.5,
    fontWeight: '700',
    paddingHorizontal: 2,
  },
  recentArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 11.5,
    marginTop: 2,
    paddingHorizontal: 2,
  },
  regionToggle: {
    flexDirection: 'row',
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
    borderRadius: 999,
    padding: 3,
  },
  regionBtn: {
    paddingHorizontal: 12,
    paddingVertical: 5,
    borderRadius: 999,
  },
  regionBtnActive: {
    backgroundColor: '#FFFFFF',
  },
  regionText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontWeight: '700',
  },
  regionTextActive: {
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
    marginHorizontal: 16,
    padding: 24,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.12)',
    alignItems: 'center',
    gap: 10,
  },
  emptyText: {
    color: LumeTheme.textSecondary,
    fontSize: 13.5,
    lineHeight: 20,
    textAlign: 'center',
  },
});
