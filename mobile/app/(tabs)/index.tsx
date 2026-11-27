import React, { useCallback, useEffect, useState } from 'react';
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
import { LinearGradient } from 'expo-linear-gradient';
import { useRouter } from 'expo-router';
import { LumeCore } from '../../modules/lume-core';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { LumeTheme } from '../../src/theme';
import { formatBytes, type HealthCheckDto, type Track } from '../../src/types';

export default function HomeScreen() {
  const router = useRouter();
  const {
    currentTrack,
    offlineUids,
    downloadingUids,
    offlineEntries,
    offlineBytes,
    errorMessage,
    playTrack,
    downloadOffline,
    toggleLikeTrack,
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
      if (!forceRefresh) setLoading(true);
      try {
        const [h, rec, chartTracks] = await Promise.all([
          LumeCore.healthCheck(),
          LumeCore.listRecentTracks(12),
          LumeCore.getCharts(indonesia, 25, forceRefresh),
        ]);
        setHealth(h);
        setRecent(rec);
        setCharts(chartTracks);
      } catch {
        // Errors are gracefully handled; offline tracks remain accessible.
      } finally {
        setLoading(false);
        setRefreshing(false);
      }
    },
    [indonesia]
  );

  useEffect(() => {
    void loadHome(false);
  }, [loadHome]);

  return (
    <SafeAreaView style={styles.container} edges={['top']}>
      <ScrollView
        contentContainerStyle={styles.scrollContent}
        refreshControl={
          <RefreshControl
            refreshing={refreshing}
            tintColor={LumeTheme.accentBright}
            onRefresh={() => {
              setRefreshing(true);
              void loadHome(true);
            }}
          />
        }
      >
        <View style={styles.header}>
          <View>
            <Text style={styles.brand}>Lume Music</Text>
            <Text style={styles.subBrand}>
              {health
                ? `Rust Core v${health.version} (${health.target_os}) • ${offlineEntries.length} Offline (${formatBytes(offlineBytes)})`
                : 'Native Rust Core menunggu build iOS'}
            </Text>
          </View>
          <Pressable
            onPress={() => router.push('/driving')}
            style={styles.driveChip}
          >
            <Ionicons name="car-sport" size={16} color="#FFFFFF" />
            <Text style={styles.driveChipText}>Drive</Text>
          </Pressable>
        </View>

        {errorMessage && (
          <Pressable onPress={clearError} style={styles.errorBanner}>
            <Ionicons name="alert-circle" size={18} color={LumeTheme.rose} />
            <Text style={styles.errorText}>{errorMessage}</Text>
          </Pressable>
        )}

        <Pressable
          onPress={() => {
            if (offlineEntries.length > 0) {
              const tracks = offlineEntries.map((e) => e.track);
              void playTrack(tracks[0], tracks);
              router.push('/driving');
            } else {
              router.push('/driving');
            }
          }}
        >
          <LinearGradient
            colors={['rgba(99, 102, 241, 0.38)', 'rgba(139, 92, 246, 0.16)']}
            start={{ x: 0, y: 0 }}
            end={{ x: 1, y: 1 }}
            style={styles.heroCard}
          >
            <View style={styles.heroIcon}>
              <Ionicons name="car-sport" size={26} color="#FFFFFF" />
            </View>
            <View style={{ flex: 1 }}>
              <Text style={styles.heroTitle}>Mode Berkendara (Standalone)</Text>
              <Text style={styles.heroDesc}>
                {offlineEntries.length > 0
                  ? `Putar ${offlineEntries.length} lagu offline siap jalan tanpa sinyal + kontrol Bluetooth.`
                  : 'Tombol ekstra besar, swipe ganti lagu, & kontrol penuh Lock Screen / Bluetooth.'}
              </Text>
            </View>
            <Ionicons name="chevron-forward" size={20} color={LumeTheme.textPrimary} />
          </LinearGradient>
        </Pressable>

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
                style={[styles.regionBtn, indonesia && styles.regionBtnActive]}
              >
                <Text
                  style={[styles.regionText, indonesia && styles.regionTextActive]}
                >
                  Indonesia
                </Text>
              </Pressable>
              <Pressable
                onPress={() => setIndonesia(false)}
                style={[styles.regionBtn, !indonesia && styles.regionBtnActive]}
              >
                <Text
                  style={[styles.regionText, !indonesia && styles.regionTextActive]}
                >
                  Global
                </Text>
              </Pressable>
            </View>
          </View>

          {loading ? (
            <ActivityIndicator
              size="large"
              color={LumeTheme.accentBright}
              style={{ marginVertical: 32 }}
            />
          ) : charts.length === 0 ? (
            <View style={styles.emptyBox}>
              <Text style={styles.emptyText}>
                {LumeCore.isAvailable()
                  ? 'Tarik ke bawah untuk memuat chart, atau gunakan tab Search / Offline.'
                  : 'Rust Core belum tertaut (mode preview JS). Build .ipa lewat GitHub Actions untuk menjalankan YouTube Music & SQLite.'}
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
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
  },
  scrollContent: {
    paddingHorizontal: 16,
    paddingBottom: 150,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginTop: 8,
    marginBottom: 16,
  },
  brand: {
    color: LumeTheme.textPrimary,
    fontSize: 28,
    fontWeight: '800',
    letterSpacing: -0.5,
  },
  subBrand: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    marginTop: 3,
  },
  driveChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    backgroundColor: LumeTheme.accent,
    paddingHorizontal: 14,
    paddingVertical: 9,
    borderRadius: 999,
  },
  driveChipText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 13,
  },
  errorBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    padding: 12,
    borderRadius: 12,
    backgroundColor: 'rgba(244, 63, 94, 0.14)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.35)',
    marginBottom: 14,
  },
  errorText: {
    color: '#FECDD3',
    fontSize: 13,
    flex: 1,
  },
  heroCard: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 14,
    padding: 16,
    borderRadius: 18,
    borderWidth: 1,
    borderColor: LumeTheme.borderAccent,
    marginBottom: 22,
  },
  heroIcon: {
    width: 48,
    height: 48,
    borderRadius: 14,
    backgroundColor: LumeTheme.accent,
    alignItems: 'center',
    justifyContent: 'center',
  },
  heroTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 16,
    fontWeight: '700',
  },
  heroDesc: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    marginTop: 3,
    lineHeight: 17,
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
    backgroundColor: LumeTheme.bgCard,
    borderRadius: 10,
    padding: 3,
  },
  regionBtn: {
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 8,
  },
  regionBtnActive: {
    backgroundColor: LumeTheme.accent,
  },
  regionText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontWeight: '600',
  },
  regionTextActive: {
    color: '#FFFFFF',
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
