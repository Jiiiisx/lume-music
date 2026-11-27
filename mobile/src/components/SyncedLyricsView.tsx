import React, { type ComponentRef, useEffect, useMemo, useRef } from 'react';
import {
  ActivityIndicator,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { LumeTheme } from '../theme';
import type { LyricsData, LyricsLine } from '../types';

interface SyncedLyricsViewProps {
  lyrics: LyricsData | null;
  loading: boolean;
  positionMs: number;
  onSeekToMs: (ms: number) => void;
  onRefreshLyrics?: () => void;
}

function findActiveLineIndex(lines: LyricsLine[], positionMs: number): number {
  if (lines.length === 0) return -1;
  let active = -1;
  for (let i = 0; i < lines.length; i++) {
    if (positionMs >= lines[i].time_ms) {
      active = i;
    } else {
      break;
    }
  }
  return active;
}

export function SyncedLyricsView({
  lyrics,
  loading,
  positionMs,
  onSeekToMs,
  onRefreshLyrics,
}: SyncedLyricsViewProps) {
  const scrollRef = useRef<ComponentRef<typeof ScrollView>>(null);
  const lineOffsetsRef = useRef<Record<number, number>>({});

  const activeIndex = useMemo(
    () => (lyrics ? findActiveLineIndex(lyrics.synced, positionMs) : -1),
    [lyrics, positionMs]
  );

  const isInterlude = useMemo(() => {
    if (!lyrics || lyrics.synced.length === 0) return false;
    if (activeIndex < 0) return positionMs > 1500 && lyrics.synced[0].time_ms - positionMs > 3000;
    const cur = lyrics.synced[activeIndex];
    const next = lyrics.synced[activeIndex + 1];
    if (cur.end_ms && next && positionMs > cur.end_ms + 800 && next.time_ms - positionMs > 2500) {
      return true;
    }
    return false;
  }, [lyrics, activeIndex, positionMs]);

  useEffect(() => {
    if (activeIndex < 0) return;
    const y = lineOffsetsRef.current[activeIndex];
    if (y !== undefined) {
      scrollRef.current?.scrollTo({
        y: Math.max(0, y - 140),
        animated: true,
      });
    }
  }, [activeIndex]);

  if (loading) {
    return (
      <View style={styles.center}>
        <ActivityIndicator size="large" color="#FFFFFF" />
        <Text style={styles.hint}>Memuat lirik tersinkronisasi...</Text>
      </View>
    );
  }

  if (!lyrics || (lyrics.synced.length === 0 && !lyrics.plain)) {
    return (
      <View style={styles.center}>
        <Text style={styles.hint}>Lirik tidak tersedia untuk lagu ini.</Text>
        {onRefreshLyrics && (
          <Pressable onPress={onRefreshLyrics} style={styles.refreshBtn}>
            <Ionicons name="refresh" size={16} color="#FFFFFF" />
            <Text style={styles.refreshText}>Coba Muat Ulang Lirik</Text>
          </Pressable>
        )}
      </View>
    );
  }

  if (lyrics.synced.length === 0 && lyrics.plain) {
    return (
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.content}
        showsVerticalScrollIndicator={false}
      >
        <View style={styles.providerRow}>
          <Text style={styles.providerTag}>Sumber: {lyrics.provider}</Text>
          {onRefreshLyrics && (
            <Pressable onPress={onRefreshLyrics} hitSlop={8}>
              <Ionicons name="refresh" size={15} color={LumeTheme.textSecondary} />
            </Pressable>
          )}
        </View>
        <Text style={styles.plainText}>{lyrics.plain}</Text>
      </ScrollView>
    );
  }

  return (
    <ScrollView
      ref={scrollRef}
      style={styles.scroll}
      contentContainerStyle={styles.content}
      showsVerticalScrollIndicator={false}
    >
      <View style={styles.providerRow}>
        <Text style={styles.providerTag}>
          {lyrics.provider.toUpperCase()} • Tap baris untuk lompat
        </Text>
        {onRefreshLyrics && (
          <Pressable onPress={onRefreshLyrics} hitSlop={8}>
            <Ionicons name="refresh" size={15} color={LumeTheme.textSecondary} />
          </Pressable>
        )}
      </View>

      {isInterlude && (
        <View style={styles.interludeRow}>
          <View style={styles.dot} />
          <View style={[styles.dot, { opacity: 0.7 }]} />
          <View style={[styles.dot, { opacity: 0.4 }]} />
        </View>
      )}

      {lyrics.synced.map((line, index) => {
        const isActive = index === activeIndex && !isInterlude;
        const isPast = index < activeIndex;
        const hasWords = line.words && line.words.length > 0;

        return (
          <Pressable
            key={`${line.time_ms}-${index}`}
            onLayout={(e) => {
              lineOffsetsRef.current[index] = e.nativeEvent.layout.y;
            }}
            onPress={() => onSeekToMs(line.time_ms)}
            style={[
              styles.lineBox,
              line.opposite_aligned && styles.rightAligned,
            ]}
          >
            {isActive && hasWords && line.words ? (
              <Text
                style={[
                  styles.lineText,
                  styles.activeLine,
                  line.opposite_aligned && styles.textRight,
                ]}
              >
                {line.words.map((word, wi) => {
                  const sung = positionMs >= word.start_ms;
                  const sep = word.is_part_of_word ? '' : ' ';
                  return (
                    <Text
                      key={`${word.start_ms}-${wi}`}
                      style={sung ? styles.sungWord : styles.upcomingWord}
                    >
                      {word.text}
                      {wi < (line.words?.length ?? 0) - 1 ? sep : ''}
                    </Text>
                  );
                })}
              </Text>
            ) : (
              <Text
                style={[
                  styles.lineText,
                  isActive && styles.activeLine,
                  isPast && styles.pastLine,
                  line.is_background && styles.bgLine,
                  line.opposite_aligned && styles.textRight,
                ]}
              >
                {line.text}
              </Text>
            )}
          </Pressable>
        );
      })}
      <View style={{ height: 140 }} />
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  center: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 24,
    gap: 14,
  },
  hint: {
    color: LumeTheme.textSecondary,
    fontSize: 15,
    textAlign: 'center',
  },
  refreshBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    backgroundColor: 'rgba(255,255,255,0.14)',
    paddingHorizontal: 14,
    paddingVertical: 9,
    borderRadius: 999,
  },
  refreshText: {
    color: '#FFFFFF',
    fontSize: 13,
    fontWeight: '700',
  },
  scroll: {
    flex: 1,
  },
  content: {
    paddingHorizontal: 20,
    paddingTop: 10,
    paddingBottom: 40,
  },
  providerRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 10,
    marginBottom: 16,
  },
  providerTag: {
    color: 'rgba(255,255,255,0.55)',
    fontSize: 11,
    fontWeight: '700',
    letterSpacing: 1,
  },
  interludeRow: {
    flexDirection: 'row',
    gap: 8,
    paddingVertical: 10,
  },
  dot: {
    width: 9,
    height: 9,
    borderRadius: 5,
    backgroundColor: '#FFFFFF',
  },
  lineBox: {
    paddingVertical: 9,
  },
  rightAligned: {
    alignItems: 'flex-end',
  },
  lineText: {
    fontSize: 22,
    lineHeight: 30,
    fontWeight: '700',
    color: 'rgba(248, 250, 252, 0.34)',
  },
  activeLine: {
    fontSize: 25,
    lineHeight: 34,
    color: '#FFFFFF',
  },
  pastLine: {
    color: 'rgba(248, 250, 252, 0.50)',
  },
  bgLine: {
    fontSize: 17,
    fontWeight: '600',
  },
  textRight: {
    textAlign: 'right',
  },
  sungWord: {
    color: '#FFFFFF',
  },
  upcomingWord: {
    color: 'rgba(248, 250, 252, 0.42)',
  },
  plainText: {
    color: LumeTheme.textPrimary,
    fontSize: 17,
    lineHeight: 28,
  },
});
