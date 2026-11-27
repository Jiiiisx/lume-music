import React from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useRouter } from 'expo-router';
import { AmbientBackdrop } from '../src/components/AmbientBackdrop';
import { usePlayer } from '../src/context/PlayerContext';
import { LumeTheme } from '../src/theme';
import { formatDurationMs, trackArtistDisplay } from '../src/types';

export default function QueueModal() {
  const router = useRouter();
  const {
    queue,
    queueIndex,
    palette,
    jumpToQueueIndex,
    removeFromQueue,
    clearUpcomingQueue,
  } = usePlayer();

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.root}>
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={10}>
            <Ionicons name="chevron-down" size={26} color="#FFFFFF" />
          </Pressable>
          <Text style={styles.title}>Antrean ({queue.length})</Text>
          <Pressable onPress={clearUpcomingQueue} hitSlop={10}>
            <Text style={styles.clearText}>Bersihkan</Text>
          </Pressable>
        </View>

        <ScrollView contentContainerStyle={styles.content}>
          {queue.length === 0 ? (
            <Text style={styles.empty}>Antrean masih kosong.</Text>
          ) : (
            queue.map((track, idx) => {
              const active = idx === queueIndex;
              return (
                <Pressable
                  key={`${track.uid}-${idx}`}
                  onPress={() => void jumpToQueueIndex(idx)}
                  style={[
                    styles.row,
                    active && {
                      backgroundColor: palette.accent_soft,
                      borderColor: 'rgba(255,255,255,0.24)',
                    },
                  ]}
                >
                  <Text style={[styles.idx, active && { color: palette.accent }]}>
                    {active ? '▶' : idx + 1}
                  </Text>
                  <View style={{ flex: 1 }}>
                    <Text
                      style={[styles.trackTitle, active && { color: palette.accent }]}
                      numberOfLines={1}
                    >
                      {track.title}
                    </Text>
                    <Text style={styles.trackArtist} numberOfLines={1}>
                      {trackArtistDisplay(track)} • {formatDurationMs(track.duration_ms)}
                    </Text>
                  </View>
                  {!active && (
                    <Pressable
                      onPress={(e) => {
                        e.stopPropagation();
                        removeFromQueue(idx);
                      }}
                      hitSlop={8}
                    >
                      <Ionicons name="close" size={20} color={LumeTheme.textMuted} />
                    </Pressable>
                  )}
                </Pressable>
              );
            })
          )}
        </ScrollView>
      </SafeAreaView>
    </AmbientBackdrop>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingVertical: 12,
    borderBottomWidth: 1,
    borderBottomColor: LumeTheme.border,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 17,
    fontWeight: '800',
  },
  clearText: {
    color: LumeTheme.rose,
    fontSize: 13.5,
    fontWeight: '700',
  },
  content: {
    padding: 14,
    gap: 8,
    paddingBottom: 40,
  },
  empty: {
    color: LumeTheme.textSecondary,
    textAlign: 'center',
    marginTop: 48,
    fontSize: 15,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    paddingVertical: 11,
    paddingHorizontal: 12,
    borderRadius: 12,
    backgroundColor: LumeTheme.bgCard,
    borderWidth: 1,
    borderColor: 'transparent',
  },
  idx: {
    color: LumeTheme.textMuted,
    width: 24,
    textAlign: 'center',
    fontWeight: '700',
  },
  trackTitle: {
    color: '#FFFFFF',
    fontSize: 14.5,
    fontWeight: '700',
  },
  trackArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    marginTop: 2,
  },
});
