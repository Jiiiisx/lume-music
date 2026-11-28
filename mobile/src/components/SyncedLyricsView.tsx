import React, { type ComponentRef, useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  LayoutChangeEvent,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { LumeTheme } from '../theme';
import type { LyricsData, LyricsLine, LyricsWord } from '../types';

interface SyncedLyricsViewProps {
  lyrics: LyricsData | null;
  loading: boolean;
  positionMs: number;
  durationMs?: number;
  onSeekToMs: (ms: number) => void;
  onRefreshLyrics?: () => void;
}

type TimelineItem =
  | {
      kind: 'interlude';
      key: string;
      startMs: number;
      endMs: number;
      oppositeAligned: boolean;
    }
  | {
      kind: 'line';
      key: string;
      lineIdx: number;
      line: LyricsLine;
      startMs: number;
      endMs: number;
    };

const INTERLUDE_GAP_MS = 3500;
const INTRO_INTERLUDE_MS = 2600;
const USER_SCROLL_PAUSE_MS = 3500;

/**
 * Mirrors Desktop's `build_display_timeline` so instrumental breaks appear inline
 * between lyric lines (instead of pinned at the top of the scroll view).
 */
function buildDisplayTimeline(lines: LyricsLine[], durationMs: number): TimelineItem[] {
  const items: TimelineItem[] = [];
  if (lines.length === 0) return items;

  if (lines[0].time_ms >= INTRO_INTERLUDE_MS) {
    items.push({
      kind: 'interlude',
      key: `intro-${lines[0].time_ms}`,
      startMs: 0,
      endMs: lines[0].time_ms,
      oppositeAligned: Boolean(lines[0].opposite_aligned),
    });
  }

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const next = lines[i + 1];
    const lastWordEnd =
      line.words && line.words.length > 0
        ? line.words[line.words.length - 1].end_ms
        : undefined;
    const rawEnd =
      line.end_ms ??
      lastWordEnd ??
      (next ? next.time_ms : durationMs > line.time_ms ? durationMs : line.time_ms + 4000);
    const endMs = Math.max(line.time_ms + 300, rawEnd);

    items.push({
      kind: 'line',
      key: `line-${line.time_ms}-${i}`,
      lineIdx: i,
      line,
      startMs: line.time_ms,
      endMs,
    });

    const hasExactEnd = line.end_ms !== undefined || lastWordEnd !== undefined;
    if (next && hasExactEnd && next.time_ms > endMs + INTERLUDE_GAP_MS) {
      items.push({
        kind: 'interlude',
        key: `gap-${endMs}-${next.time_ms}`,
        startMs: endMs,
        endMs: next.time_ms,
        oppositeAligned: Boolean(next.opposite_aligned),
      });
    }
  }

  return items;
}

function findActiveTimelineIndex(items: TimelineItem[], positionMs: number): number {
  if (items.length === 0) return -1;
  let active = -1;
  for (let i = 0; i < items.length; i++) {
    if (positionMs >= items[i].startMs) {
      active = i;
    } else {
      break;
    }
  }
  return active;
}

/**
 * Produces per-word timings for BOTH native syllable-synced lines (LyricsPlus / Apple Music TTML)
 * and line-only synced lines (LRCLIB), matching Desktop's `resolve_line_words` karaoke sweep.
 */
function resolveLineWords(line: LyricsLine, startMs: number, endMs: number): LyricsWord[] {
  if (line.words && line.words.length > 0) {
    return line.words;
  }
  const tokens = line.text.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return [];

  const totalSpan = Math.max(400, endMs - startMs);
  // Slight 8% breathing cushion at the end of line-synced rows so the final word finishes crisply.
  const usableSpan = totalSpan * 0.92;
  const weights = tokens.map((tok) => Math.max(1, tok.length));
  const totalWeight = weights.reduce((acc, w) => acc + w, 0);

  let cursor = startMs;
  return tokens.map((tok, idx) => {
    const share = (weights[idx] / totalWeight) * usableSpan;
    const wStart = Math.round(cursor);
    const wEnd = Math.round(cursor + Math.max(80, share));
    cursor += share;
    return {
      text: tok,
      start_ms: wStart,
      end_ms: wEnd,
      is_part_of_word: false,
    };
  });
}

export function SyncedLyricsView({
  lyrics,
  loading,
  positionMs,
  durationMs = 0,
  onSeekToMs,
  onRefreshLyrics,
}: SyncedLyricsViewProps) {
  const scrollRef = useRef<ComponentRef<typeof ScrollView>>(null);
  const itemOffsetsRef = useRef<Record<number, number>>({});
  const userScrollUntilRef = useRef(0);
  const [viewportHeight, setViewportHeight] = useState(420);

  const timeline = useMemo(
    () => (lyrics ? buildDisplayTimeline(lyrics.synced, durationMs) : []),
    [lyrics, durationMs]
  );

  // Clear cached layout offsets whenever the lyrics payload changes.
  useEffect(() => {
    itemOffsetsRef.current = {};
  }, [lyrics]);

  const activeIndex = useMemo(
    () => findActiveTimelineIndex(timeline, positionMs),
    [timeline, positionMs]
  );

  useEffect(() => {
    if (activeIndex < 0) return;
    if (Date.now() < userScrollUntilRef.current) return;
    const y = itemOffsetsRef.current[activeIndex];
    if (y !== undefined) {
      const focalOffset = Math.round(viewportHeight * 0.31);
      scrollRef.current?.scrollTo({
        y: Math.max(0, y - focalOffset),
        animated: true,
      });
    }
  }, [activeIndex, viewportHeight]);

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
        <View style={styles.emptyIcon}>
          <Ionicons name="musical-notes-outline" size={30} color="rgba(255,255,255,0.65)" />
        </View>
        <Text style={styles.hint}>Lirik tidak tersedia untuk lagu ini.</Text>
        {onRefreshLyrics && (
          <Pressable onPress={onRefreshLyrics} style={styles.refreshBtn}>
            <Ionicons name="refresh" size={16} color={LumeTheme.inkDark} />
            <Text style={styles.refreshText}>Coba Muat Ulang</Text>
          </Pressable>
        )}
      </View>
    );
  }

  if (lyrics.synced.length === 0 && lyrics.plain) {
    return (
      <View style={styles.root}>
        <ScrollView
          style={styles.scroll}
          contentContainerStyle={styles.plainContent}
          showsVerticalScrollIndicator={false}
        >
          <Text style={styles.plainText}>{lyrics.plain}</Text>
        </ScrollView>
        {onRefreshLyrics && (
          <View style={styles.footerBar}>
            <Text style={styles.providerTag}>{lyrics.provider.toUpperCase()} • PLAIN</Text>
            <Pressable onPress={onRefreshLyrics} style={styles.miniRefreshBtn} hitSlop={8}>
              <Ionicons name="refresh" size={14} color="#FFFFFF" />
            </Pressable>
          </View>
        )}
      </View>
    );
  }

  const topSpacer = Math.max(64, Math.round(viewportHeight * 0.24));
  const bottomSpacer = Math.max(140, Math.round(viewportHeight * 0.44));

  return (
    <View
      style={styles.root}
      onLayout={(e: LayoutChangeEvent) => {
        const h = e.nativeEvent.layout.height;
        if (h > 100) setViewportHeight(h);
      }}
    >
      <ScrollView
        ref={scrollRef}
        style={styles.scroll}
        contentContainerStyle={styles.content}
        showsVerticalScrollIndicator={false}
        onScrollBeginDrag={() => {
          userScrollUntilRef.current = Date.now() + USER_SCROLL_PAUSE_MS;
        }}
        onMomentumScrollEnd={() => {
          userScrollUntilRef.current = Date.now() + USER_SCROLL_PAUSE_MS;
        }}
      >
        <View style={{ height: topSpacer }} />

        {timeline.map((item, index) => {
          const distance = activeIndex >= 0 ? index - activeIndex : index + 1;
          const absDist = Math.abs(distance);
          const isActive = distance === 0;

          if (item.kind === 'interlude') {
            const span = Math.max(1, item.endMs - item.startMs);
            const prog = isActive
              ? Math.min(1, Math.max(0, (positionMs - item.startMs) / span))
              : distance < 0
                ? 1
                : 0;
            const dot1 = isActive ? 0.35 + 0.65 * Math.min(1, prog * 3) : 0.22;
            const dot2 = isActive
              ? 0.35 + 0.65 * Math.min(1, Math.max(0, (prog - 0.33) * 3))
              : 0.22;
            const dot3 = isActive
              ? 0.35 + 0.65 * Math.min(1, Math.max(0, (prog - 0.66) * 3))
              : 0.22;

            return (
              <Pressable
                key={item.key}
                onLayout={(e) => {
                  itemOffsetsRef.current[index] = e.nativeEvent.layout.y;
                }}
                onPress={() => {
                  userScrollUntilRef.current = 0;
                  onSeekToMs(item.startMs);
                }}
                style={[
                  styles.interludeRow,
                  item.oppositeAligned && styles.interludeRight,
                  !isActive && { opacity: absDist === 1 ? 0.4 : 0.16 },
                ]}
              >
                <View style={[styles.dot, isActive && styles.dotActive, { opacity: dot1 }]} />
                <View style={[styles.dot, isActive && styles.dotActive, { opacity: dot2 }]} />
                <View style={[styles.dot, isActive && styles.dotActive, { opacity: dot3 }]} />
              </Pressable>
            );
          }

          const { line, startMs, endMs } = item;
          const isOverlappingDuet =
            !isActive &&
            Boolean(line.words && line.words.length > 0) &&
            positionMs >= startMs &&
            positionMs <= endMs;
          const highlightActive = isActive || isOverlappingDuet;

          const words = highlightActive ? resolveLineWords(line, startMs, endMs) : null;

          // Distance-graded Apple Music / Desktop opacity & scale hierarchy.
          const idleColor =
            absDist === 1
              ? 'rgba(255, 255, 255, 0.46)'
              : absDist === 2
                ? 'rgba(255, 255, 255, 0.28)'
                : 'rgba(255, 255, 255, 0.17)';
          const idleSize = line.is_background
            ? 16.5
            : absDist === 1
              ? 22.5
              : absDist === 2
                ? 21
                : 20;

          return (
            <Pressable
              key={item.key}
              onLayout={(e) => {
                itemOffsetsRef.current[index] = e.nativeEvent.layout.y;
              }}
              onPress={() => {
                userScrollUntilRef.current = 0;
                onSeekToMs(line.time_ms);
              }}
              style={({ pressed }) => [
                styles.lineBox,
                highlightActive && styles.lineBoxActive,
                line.opposite_aligned && styles.rightAligned,
                pressed && styles.linePressed,
              ]}
            >
              {highlightActive && words && words.length > 0 ? (
                <Text
                  style={[
                    styles.activeLineText,
                    line.is_background && styles.activeBgLineText,
                    line.opposite_aligned && styles.textRight,
                  ]}
                >
                  {words.map((word, wi) => {
                    const isSingingNow =
                      positionMs >= word.start_ms && positionMs <= word.end_ms + 80;
                    const isSung = positionMs > word.end_ms + 80;
                    const sep = word.is_part_of_word ? '' : ' ';

                    return (
                      <Text
                        key={`${word.start_ms}-${wi}`}
                        style={
                          isSingingNow
                            ? styles.wordGlowing
                            : isSung
                              ? styles.wordSung
                              : styles.wordUpcoming
                        }
                      >
                        {word.text}
                        {wi < words.length - 1 ? sep : ''}
                      </Text>
                    );
                  })}
                </Text>
              ) : (
                <Text
                  style={[
                    styles.idleLineText,
                    { color: idleColor, fontSize: idleSize, lineHeight: Math.round(idleSize * 1.36) },
                    line.is_background && styles.bgLineItalic,
                    line.opposite_aligned && styles.textRight,
                  ]}
                >
                  {line.text}
                </Text>
              )}
            </Pressable>
          );
        })}

        <View style={{ height: bottomSpacer }} />
      </ScrollView>

      <View style={styles.footerBar}>
        <Text style={styles.providerTag}>
          {lyrics.provider.toUpperCase()} • Tap lirik untuk lompat
        </Text>
        {onRefreshLyrics && (
          <Pressable onPress={onRefreshLyrics} style={styles.miniRefreshBtn} hitSlop={8}>
            <Ionicons name="refresh" size={14} color="#FFFFFF" />
          </Pressable>
        )}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
  },
  center: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: 28,
    gap: 14,
  },
  emptyIcon: {
    width: 60,
    height: 60,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
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
    backgroundColor: '#FFFFFF',
    paddingHorizontal: 18,
    paddingVertical: 10,
    borderRadius: 999,
  },
  refreshText: {
    color: LumeTheme.inkDark,
    fontSize: 13.5,
    fontWeight: '800',
  },
  scroll: {
    flex: 1,
  },
  content: {
    paddingHorizontal: 24,
  },
  plainContent: {
    paddingHorizontal: 24,
    paddingVertical: 20,
    paddingBottom: 60,
  },
  interludeRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 9,
    paddingVertical: 14,
  },
  interludeRight: {
    justifyContent: 'flex-end',
  },
  dot: {
    width: 9,
    height: 9,
    borderRadius: 4.5,
    backgroundColor: '#FFFFFF',
  },
  dotActive: {
    width: 11,
    height: 11,
    borderRadius: 5.5,
    shadowColor: '#FFFFFF',
    shadowOpacity: 0.75,
    shadowRadius: 8,
    shadowOffset: { width: 0, height: 0 },
  },
  lineBox: {
    paddingVertical: 10,
    borderRadius: 12,
  },
  lineBoxActive: {
    paddingVertical: 12,
  },
  linePressed: {
    backgroundColor: 'rgba(255, 255, 255, 0.07)',
  },
  rightAligned: {
    alignItems: 'flex-end',
  },
  idleLineText: {
    fontWeight: '700',
    letterSpacing: -0.3,
  },
  activeLineText: {
    fontSize: 27,
    lineHeight: 37,
    fontWeight: '800',
    letterSpacing: -0.4,
  },
  activeBgLineText: {
    fontSize: 20,
    lineHeight: 28,
    fontWeight: '700',
    fontStyle: 'italic',
  },
  bgLineItalic: {
    fontWeight: '600',
    fontStyle: 'italic',
  },
  textRight: {
    textAlign: 'right',
  },
  wordGlowing: {
    color: '#FFFFFF',
    textShadowColor: 'rgba(255, 255, 255, 0.65)',
    textShadowOffset: { width: 0, height: 0 },
    textShadowRadius: 12,
  },
  wordSung: {
    color: 'rgba(255, 255, 255, 0.94)',
    textShadowColor: 'rgba(255, 255, 255, 0.25)',
    textShadowOffset: { width: 0, height: 0 },
    textShadowRadius: 5,
  },
  wordUpcoming: {
    color: 'rgba(255, 255, 255, 0.36)',
  },
  plainText: {
    color: 'rgba(255, 255, 255, 0.88)',
    fontSize: 18,
    lineHeight: 30,
    fontWeight: '600',
  },
  footerBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 24,
    paddingVertical: 8,
  },
  providerTag: {
    color: 'rgba(255, 255, 255, 0.42)',
    fontSize: 10.5,
    fontWeight: '700',
    letterSpacing: 0.9,
  },
  miniRefreshBtn: {
    width: 30,
    height: 30,
    borderRadius: 15,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.15)',
    alignItems: 'center',
    justifyContent: 'center',
  },
});
