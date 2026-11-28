import React from 'react';
import { Alert, FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useRouter } from 'expo-router';
import { AmbientBackdrop } from '../../src/components/AmbientBackdrop';
import { BatchDownloadBanner } from '../../src/components/BatchDownloadBanner';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { LumeTheme } from '../../src/theme';
import { formatBytes } from '../../src/types';

export default function OfflineScreen() {
  const router = useRouter();
  const {
    currentTrack,
    offlineEntries,
    offlineBytes,
    playTrack,
    removeOffline,
    toggleLikeTrack,
  } = usePlayer();

  const tracks = offlineEntries.map((e) => e.track);

  const playAllOffline = async (openDriving: boolean) => {
    if (tracks.length === 0) return;
    const randomStart = tracks[Math.floor(Math.random() * tracks.length)];
    await playTrack(randomStart, tracks, false, true);
    if (openDriving) {
      router.push('/driving');
    }
  };

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.container} edges={['top']}>
        <View style={styles.header}>
          <Text style={styles.title}>Offline Cache</Text>

          <View style={styles.statsCard}>
            <View style={styles.statsTop}>
              <View style={{ flex: 1 }}>
                <Text style={styles.statsMain}>
                  {offlineEntries.length} lagu tersimpan
                </Text>
                <Text style={styles.statsSub}>
                  {formatBytes(offlineBytes)} terpakai di penyimpanan iPhone
                </Text>
              </View>
              <View style={styles.readyBadge}>
                <Ionicons name="checkmark-circle" size={14} color={LumeTheme.emerald} />
                <Text style={styles.readyBadgeText}>Tanpa Sinyal</Text>
              </View>
            </View>

            {tracks.length > 0 && (
              <View style={styles.actionRow}>
                <Pressable
                  onPress={() => void playAllOffline(false)}
                  style={({ pressed }) => [styles.primaryBtn, pressed && styles.pressed]}
                >
                  <Ionicons name="shuffle" size={17} color={LumeTheme.inkDark} />
                  <Text style={styles.primaryBtnText} numberOfLines={1}>
                    Putar Acak
                  </Text>
                </Pressable>

                <Pressable
                  onPress={() => void playAllOffline(true)}
                  style={({ pressed }) => [styles.driveBtn, pressed && styles.pressed]}
                >
                  <Ionicons name="car-sport" size={17} color="#FFFFFF" />
                  <Text style={styles.driveBtnText} numberOfLines={1}>
                    Mode Drive
                  </Text>
                </Pressable>
              </View>
            )}
          </View>
        </View>

        <BatchDownloadBanner />

        <FlatList
          data={offlineEntries}
          keyExtractor={(item) => item.track.uid}
          contentContainerStyle={styles.listContent}
          showsVerticalScrollIndicator={false}
          ListEmptyComponent={
            <View style={styles.empty}>
              <View style={styles.emptyIconWrap}>
                <Ionicons name="cloud-offline-outline" size={34} color="#FFFFFF" />
              </View>
              <Text style={styles.emptyTitle}>Belum Ada Lagu Offline</Text>
              <Text style={styles.emptyDesc}>
                Ketuk ikon awan pada lagu atau playlist di tab Home, Explore, atau Library untuk
                menyimpan audio M4A + lirik ke iPhone sebelum berkendara.
              </Text>
            </View>
          }
          renderItem={({ item, index }) => (
            <TrackRow
              track={item.track}
              rank={index + 1}
              isActive={currentTrack?.uid === item.track.uid}
              isOffline
              onPress={() => void playTrack(item.track, tracks)}
              onLikePress={() => void toggleLikeTrack(item.track)}
              onDeleteOfflinePress={() => {
                Alert.alert(
                  'Hapus File Offline?',
                  `Hapus "${item.track.title}" (${formatBytes(item.record.file_size_bytes)}) dari penyimpanan iPhone?`,
                  [
                    { text: 'Batal', style: 'cancel' },
                    {
                      text: 'Hapus',
                      style: 'destructive',
                      onPress: () => void removeOffline(item.track.uid),
                    },
                  ]
                );
              }}
            />
          )}
        />
      </SafeAreaView>
    </AmbientBackdrop>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
  },
  header: {
    paddingHorizontal: 16,
    paddingTop: 10,
    paddingBottom: 12,
    gap: 12,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 28,
    fontWeight: '800',
    letterSpacing: -0.6,
    paddingHorizontal: 2,
  },
  statsCard: {
    padding: 15,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.15)',
    gap: 14,
  },
  statsTop: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: 10,
  },
  statsMain: {
    color: '#FFFFFF',
    fontSize: 16,
    fontWeight: '800',
  },
  statsSub: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    marginTop: 2,
  },
  readyBadge: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 999,
    backgroundColor: 'rgba(16, 185, 129, 0.16)',
    borderWidth: 1,
    borderColor: 'rgba(16, 185, 129, 0.34)',
  },
  readyBadgeText: {
    color: '#6EE7B7',
    fontSize: 11,
    fontWeight: '700',
  },
  actionRow: {
    flexDirection: 'row',
    gap: 10,
  },
  primaryBtn: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 7,
    height: 44,
    paddingHorizontal: 14,
    borderRadius: 999,
    backgroundColor: '#FFFFFF',
  },
  primaryBtnText: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
    fontSize: 13.5,
  },
  driveBtn: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 7,
    height: 44,
    paddingHorizontal: 14,
    borderRadius: 999,
    backgroundColor: 'rgba(16, 185, 129, 0.24)',
    borderWidth: 1,
    borderColor: 'rgba(16, 185, 129, 0.45)',
  },
  driveBtnText: {
    color: '#FFFFFF',
    fontWeight: '800',
    fontSize: 13.5,
  },
  pressed: {
    opacity: 0.84,
  },
  listContent: {
    paddingHorizontal: 12,
    paddingBottom: 170,
  },
  empty: {
    alignItems: 'center',
    paddingHorizontal: 30,
    paddingTop: 52,
    gap: 10,
  },
  emptyIconWrap: {
    width: 64,
    height: 64,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: 4,
  },
  emptyTitle: {
    color: '#FFFFFF',
    fontSize: 17.5,
    fontWeight: '800',
  },
  emptyDesc: {
    color: LumeTheme.textSecondary,
    fontSize: 13.5,
    textAlign: 'center',
    lineHeight: 20,
  },
});
