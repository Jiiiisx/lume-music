import React, { useCallback, useState } from 'react';
import {
  ActivityIndicator,
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
import { formatBytes, type HealthCheckDto, type Track } from '../../src/types';

export default function HomeScreen() {
  const router = useRouter();
  const {
    currentTrack,
    palette,
    offlineUids,
    downloadingUids,
    offlineEntries,
    offlineBytes,
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
  const [health, setHealth] = useState<HealthCheckDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const loadHome = useCallback(
    async (forceRefresh = false) => {
      if (!LumeCore.isAvailable()) return;
      if (!forceRefresh && charts.length === 0) setLoading(true);
      try {
        const [h, rec, chartTracks] = await Promise.all([
          LumeCore.healthCheck(),
          LumeCore.listRecentTracks(12),
          LumeCore.getCharts(indonesia, 25, forceRefresh),
        ]);
        setHealth(h);
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

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.container} edges={['top']}>
        <ScrollView
          contentContainerStyle={styles.scrollContent}
          refreshControl={
            <RefreshControl
              refreshing={refreshing}
              tintColor={palette.accent}
              onRefresh={() => {
                setRefreshing(true);
                void loadHome(true);
              }}
            />
          }
        >
          <View style={styles.header}>
            <View style={{ flex: 1 }}>
              <Text style={styles.brand}>Lume Music</Text>
              <Text style={styles.subBrand} numberOfLines={1}>
                {health
                  ? `Rust Core v${health.version} (${health.target_os}) • ${offlineEntries.length} Offline (${formatBytes(offlineBytes)})`
                  : 'Native Rust Core menunggu build iOS'}
              </Text>
            </View>

            <View style={styles.headerActions}>
              <Pressable
                onPress={() => router.push('/queue')}
                style={styles.iconCircle}
                accessibilityLabel="Queue"
              >
                <Ionicons name="list" size={19} color="#FFFFFF" />
              </Pressable>
              <Pressable
                onPress={() => router.push('/settings')}
                style={styles.iconCircle}
                accessibilityLabel="Settings"
              >
                <Ionicons name="settings-outline" size={19} color="#FFFFFF" />
              </Pressable>
              <Pressable
                onPress={() => router.push('/driving')}
                style={[styles.driveChip, { backgroundColor: palette.accent }]}
              >
                <Ionicons name="car-sport" size={15} color="#0A0C12" />
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

          {recent.length > 0 && (
            <View style={styles.section}>
              <Text style={styles.sectionTitle}>Baru Diputar</Text>
              {recent.slice(0, 6).map((track) => (
                <TrackRow
                  key={track.uid}
                  track={track}
                  isActive={currentTrack?.uid === track.uid}
                  isOffline={offlineUids.has(track.uid)}
                  isDownloading={downloadingUids.has(track.uid)}
                  onPress={() => void playTrack(track, recent)}
                  onDownloadPress={() => void downloadOffline(track)}
                  onLikePress={() => void toggleLikeTrack(track)}
                />
              ))}
            </View>
          )}

          <View style={styles.section}>
            <View style={styles.sectionHeader}>
              <Text style={styles.sectionTitle}>Trending Charts</Text>
              <View style={styles.regionToggle}>
                <Pressable
                  onPress={() => setIndonesia(true)}
                  style={[
                    styles.regionBtn,
                    indonesia && { backgroundColor: palette.accent },
                  ]}
                >
                  <Text
                    style={[
                      styles.regionText,
                      indonesia && styles.regionTextActive,
                    ]}
                  >
                    Indonesia
                  </Text>
                </Pressable>
                <Pressable
                  onPress={() => setIndonesia(false)}
                  style={[
                    styles.regionBtn,
                    !indonesia && { backgroundColor: palette.accent },
                  ]}
                >
                  <Text
                    style={[
                      styles.regionText,
                      !indonesia && styles.regionTextActive,
                    ]}
                  >
                    Global
                  </Text>
                </Pressable>
              </View>
            </View>

            {loading ? (
              <ActivityIndicator
                size="large"
                color={palette.accent}
                style={{ marginVertical: 32 }}
              />
            ) : charts.length === 0 ? (
              <View style={styles.emptyBox}>
                <Text style={styles.emptyText}>
                  Tarik ke bawah untuk memuat chart, atau gunakan tab Search / Offline.
                </Text>
              </View>
            ) : (
              charts.map((track) => (
                <TrackRow
                  key={track.uid}
                  track={track}
                  isActive={currentTrack?.uid === track.uid}
                  isOffline={offlineUids.has(track.uid)}
                  isDownloading={downloadingUids.has(track.uid)}
                  onPress={() => void playTrack(track, charts)}
                  onDownloadPress={() => void downloadOffline(track)}
                  onLikePress={() => void toggleLikeTrack(track)}
                />
              ))
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
    paddingHorizontal: 16,
    paddingBottom: 155,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginTop: 8,
    marginBottom: 16,
    gap: 8,
  },
  brand: {
    color: LumeTheme.textPrimary,
    fontSize: 27,
    fontWeight: '800',
    letterSpacing: -0.5,
  },
  subBrand: {
    color: LumeTheme.textSecondary,
    fontSize: 11.5,
    marginTop: 3,
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
    backgroundColor: 'rgba(255,255,255,0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  driveChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    paddingHorizontal: 12,
    height: 36,
    borderRadius: 999,
  },
  driveChipText: {
    color: '#0A0C12',
    fontWeight: '800',
    fontSize: 12.5,
  },
  errorBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    padding: 12,
    borderRadius: 12,
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.35)',
    marginBottom: 14,
  },
  errorText: {
    color: '#FECDD3',
    fontSize: 13,
    flex: 1,
  },
  section: {
    marginBottom: 22,
  },
  sectionHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: 10,
  },
  sectionTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 19,
    fontWeight: '700',
    marginBottom: 8,
  },
  regionToggle: {
    flexDirection: 'row',
    backgroundColor: 'rgba(255,255,255,0.08)',
    borderRadius: 10,
    padding: 3,
  },
  regionBtn: {
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 8,
  },
  regionText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontWeight: '600',
  },
  regionTextActive: {
    color: '#0A0C12',
    fontWeight: '800',
  },
  emptyBox: {
    padding: 20,
    borderRadius: 14,
    backgroundColor: LumeTheme.bgCard,
    borderWidth: 1,
    borderColor: LumeTheme.border,
  },
  emptyText: {
    color: LumeTheme.textSecondary,
    fontSize: 13.5,
    lineHeight: 20,
    textAlign: 'center',
  },
});
