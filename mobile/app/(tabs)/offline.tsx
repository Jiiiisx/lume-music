import React from 'react';
import { Alert, FlatList, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useRouter } from 'expo-router';
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
    toggleShuffle,
    shuffle,
  } = usePlayer();

  const tracks = offlineEntries.map((e) => e.track);

  const playAllOfflineDriving = async () => {
    if (tracks.length === 0) return;
    if (!shuffle) toggleShuffle();
    const randomStart = tracks[Math.floor(Math.random() * tracks.length)];
    await playTrack(randomStart, tracks);
    router.push('/driving');
  };

  return (
    <SafeAreaView style={styles.container} edges={['top']}>
      <View style={styles.header}>
        <Text style={styles.title}>Offline Cache</Text>
        <Text style={styles.subtitle}>
          {offlineEntries.length} lagu siap putar tanpa sinyal •{' '}
          {formatBytes(offlineBytes)} terpakai
        </Text>

        {tracks.length > 0 && (
          <Pressable
            onPress={() => void playAllOfflineDriving()}
            style={styles.playAllBtn}
          >
            <Ionicons name="car-sport" size={18} color="#FFFFFF" />
            <Text style={styles.playAllText}>
              Putar Acak Semua Offline (Mode Berkendara)
            </Text>
          </Pressable>
        )}
      </View>

      <FlatList
        data={offlineEntries}
        keyExtractor={(item) => item.track.uid}
        contentContainerStyle={styles.listContent}
        ListEmptyComponent={
          <View style={styles.empty}>
            <Ionicons name="cloud-offline-outline" size={42} color={LumeTheme.textMuted} />
            <Text style={styles.emptyTitle}>Belum Ada Lagu Offline</Text>
            <Text style={styles.emptyDesc}>
              Ketuk ikon awan pada lagu di tab Home, Search, atau Library untuk
              menyimpan audio M4A + lirik ke penyimpanan iPhone sebelum berkendara.
            </Text>
          </View>
        }
        renderItem={({ item }) => (
          <TrackRow
            track={item.track}
            isActive={currentTrack?.uid === item.track.uid}
            isOffline
            onPress={() => void playTrack(item.track, tracks)}
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
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
  },
  header: {
    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 12,
    gap: 6,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 28,
    fontWeight: '800',
  },
  subtitle: {
    color: LumeTheme.textSecondary,
    fontSize: 13,
  },
  playAllBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 8,
    backgroundColor: LumeTheme.emerald,
    borderRadius: 14,
    paddingVertical: 13,
    marginTop: 8,
  },
  playAllText: {
    color: '#FFFFFF',
    fontWeight: '800',
    fontSize: 14,
  },
  listContent: {
    paddingHorizontal: 10,
    paddingBottom: 160,
  },
  empty: {
    alignItems: 'center',
    paddingHorizontal: 32,
    paddingTop: 64,
    gap: 10,
  },
  emptyTitle: {
    color: LumeTheme.textPrimary,
    fontSize: 17,
    fontWeight: '700',
  },
  emptyDesc: {
    color: LumeTheme.textSecondary,
    fontSize: 13.5,
    textAlign: 'center',
    lineHeight: 20,
  },
});
