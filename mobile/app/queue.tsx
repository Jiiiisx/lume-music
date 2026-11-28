import React from 'react';
import { Image, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
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
    shuffle,
    repeat,
    toggleShuffle,
    cycleRepeat,
    jumpToQueueIndex,
    reorderQueue,
    removeFromQueue,
    clearUpcomingQueue,
  } = usePlayer();

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.root}>
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} style={styles.closeBtn} hitSlop={10}>
            <Ionicons name="chevron-down" size={22} color="#FFFFFF" />
          </Pressable>
          <Text style={styles.title}>Antrean ({queue.length})</Text>
          <Pressable onPress={clearUpcomingQueue} hitSlop={10}>
            <Text style={styles.clearText}>Bersihkan</Text>
          </Pressable>
        </View>

        <View style={styles.modeBar}>
          <Pressable
            onPress={toggleShuffle}
            style={[styles.modePill, shuffle && styles.modePillActive]}
          >
            <Ionicons
              name="shuffle"
              size={15}
              color={shuffle ? LumeTheme.inkDark : '#FFFFFF'}
            />
            <Text style={[styles.modeText, shuffle && styles.modeTextActive]}>
              {shuffle ? 'Acak Aktif' : 'Acak Lagu'}
            </Text>
          </Pressable>

          <Pressable
            onPress={cycleRepeat}
            style={[styles.modePill, repeat !== 'off' && styles.modePillActive]}
          >
            <Ionicons
              name="repeat"
              size={15}
              color={repeat !== 'off' ? LumeTheme.inkDark : '#FFFFFF'}
            />
            <Text style={[styles.modeText, repeat !== 'off' && styles.modeTextActive]}>
              {repeat === 'one'
                ? 'Ulangi 1 Lagu'
                : repeat === 'all'
                  ? 'Ulangi Semua'
                  : 'Ulangi Mati'}
            </Text>
          </Pressable>
        </View>

        <ScrollView contentContainerStyle={styles.content} showsVerticalScrollIndicator={false}>
          {queue.length === 0 ? (
            <Text style={styles.empty}>Antrean masih kosong.</Text>
          ) : (
            queue.map((track, idx) => {
              const active = idx === queueIndex;
              return (
                <Pressable
                  key={`${track.uid}-${idx}`}
                  onPress={() => void jumpToQueueIndex(idx)}
                  style={[styles.row, active && styles.activeRow]}
                >
                  <Text style={[styles.idx, active && styles.idxActive]}>
                    {active ? '▶' : String(idx + 1).padStart(2, '0')}
                  </Text>
                  {track.artwork_ref ? (
                    <Image source={{ uri: track.artwork_ref }} style={styles.art} />
                  ) : (
                    <View style={[styles.art, styles.artFallback]}>
                      <Ionicons name="musical-note" size={16} color="rgba(255,255,255,0.7)" />
                    </View>
                  )}
                  <View style={{ flex: 1 }}>
                    <Text
                      style={[styles.trackTitle, active && styles.trackTitleActive]}
                      numberOfLines={1}
                    >
                      {track.title}
                    </Text>
                    <Text style={styles.trackArtist} numberOfLines={1}>
                      {trackArtistDisplay(track)} • {formatDurationMs(track.duration_ms)}
                    </Text>
                  </View>
                  {!active && (
                    <View style={styles.rowTools}>
                      <Pressable
                        onPress={(e) => {
                          e.stopPropagation();
                          if (idx > 0) reorderQueue(idx, idx - 1);
                        }}
                        disabled={idx === 0}
                        hitSlop={6}
                        style={[styles.toolBtn, idx === 0 && styles.toolDisabled]}
                      >
                        <Ionicons name="chevron-up" size={16} color="#FFFFFF" />
                      </Pressable>
                      <Pressable
                        onPress={(e) => {
                          e.stopPropagation();
                          if (idx + 1 < queue.length) reorderQueue(idx, idx + 1);
                        }}
                        disabled={idx + 1 >= queue.length}
                        hitSlop={6}
                        style={[styles.toolBtn, idx + 1 >= queue.length && styles.toolDisabled]}
                      >
                        <Ionicons name="chevron-down" size={16} color="#FFFFFF" />
                      </Pressable>
                      <Pressable
                        onPress={(e) => {
                          e.stopPropagation();
                          removeFromQueue(idx);
                        }}
                        hitSlop={6}
                        style={styles.toolBtn}
                      >
                        <Ionicons name="close" size={16} color={LumeTheme.rose} />
                      </Pressable>
                    </View>
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
    paddingVertical: 10,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: 'rgba(255, 255, 255, 0.14)',
  },
  closeBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.15)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    color: '#FFFFFF',
    fontSize: 17,
    fontWeight: '800',
  },
  clearText: {
    color: '#FECDD3',
    fontSize: 13,
    fontWeight: '700',
  },
  modeBar: {
    flexDirection: 'row',
    gap: 10,
    paddingHorizontal: 16,
    paddingVertical: 12,
  },
  modePill: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 6,
    height: 38,
    borderRadius: 999,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
  },
  modePillActive: {
    backgroundColor: '#FFFFFF',
    borderColor: '#FFFFFF',
  },
  modeText: {
    color: '#FFFFFF',
    fontSize: 12.5,
    fontWeight: '700',
  },
  modeTextActive: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
  },
  content: {
    paddingHorizontal: 14,
    gap: 8,
    paddingBottom: 48,
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
    gap: 10,
    paddingVertical: 9,
    paddingHorizontal: 10,
    borderRadius: 14,
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.10)',
  },
  activeRow: {
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
    borderColor: 'rgba(255, 255, 255, 0.28)',
  },
  idx: {
    color: LumeTheme.textMuted,
    width: 22,
    textAlign: 'center',
    fontSize: 11.5,
    fontWeight: '700',
    fontVariant: ['tabular-nums'],
  },
  idxActive: {
    color: '#FFFFFF',
  },
  art: {
    width: 40,
    height: 40,
    borderRadius: 9,
  },
  artFallback: {
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  trackTitle: {
    color: '#FFFFFF',
    fontSize: 14,
    fontWeight: '600',
  },
  trackTitleActive: {
    fontWeight: '800',
  },
  trackArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 11.5,
    marginTop: 2,
  },
  rowTools: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 2,
  },
  toolBtn: {
    width: 28,
    height: 30,
    borderRadius: 8,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  toolDisabled: {
    opacity: 0.28,
  },
});
