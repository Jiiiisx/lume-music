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
import { LinearGradient } from 'expo-linear-gradient';
import Animated, {
  Easing,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated';
import { LumeTheme } from '../theme';
import type { LyricsData, LyricsLine, LyricsWord } from '../types';

interface SyncedLyricsViewProps {
  lyrics: LyricsData | null;
  loading: boolean;
  positionMs: number;
  durationMs?: number;
  isPlaying?: boolean;
  fadeColor?: string;
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

function resolveLineWords(line: LyricsLine, startMs: number, endMs: number): LyricsWord[] {
  if (line.words && line.words.length > 0) {
    return line.words;
  }
  const tokens = line.text.trim().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return [];

  const totalSpan = Math.max(400, endMs - startMs);
  const usableSpan = totalSpan * 0.92;
  const weights = tokens.map((tok) => Math.max(1, tok.length));
  const totalWeight = weights.reduce((acc, w) => acc + w, 0);

  let cursor = startMs;
  return tokens.map((tok, idx) => {
    const share = (weights[idx] / totalWeight) * usableSpan;
    const wStart = Math.round(cursor);
    const wEnd = Math.round(cursor + Math.max(90, share));
    cursor += share;
    return {
      text: tok,
      start_ms: wStart,
      end_ms: wEnd,
      is_part_of_word: false,
    };
  });
}

function smoothstep(t: number): number {
  const c = Math.min(1, Math.max(0, t));
  return c * c * (3 - 2 * c);
}

interface LyricLineRowProps {
  line: LyricsLine;
  startMs: number;
  endMs: number;
  isActive: boolean;
  distance: number;
  smoothPosMs: number;
  onLayoutY: (y: number) => void;
  onPress: () => void;
}

/**
 * Animated lyric line row using constant layout typography + UI-thread Reanimated
 * springs (`scale`, `translateY`, `opacity`) so activating/deactivating lines NEVER
 * causes text reflow jumps or scroll jank.
 */
const LyricLineRow = React.memo(function LyricLineRow({
  line,
  startMs,
  endMs,
  isActive,
  distance,
  smoothPosMs,
  onLayoutY,
  onPress,
}: LyricLineRowProps) {
  const absDist = Math.abs(distance);
  const targetOpacity = isActive
    ? 1
    : absDist === 1
      ? 0.44
      : absDist === 2
        ? 0.27
        : 0.16;
  const targetScale = isActive ? 1 : absDist === 1 ? 0.94 : 0.91;
  const targetLift = isActive ? -2 : 0;

  const animOpacity = useSharedValue(targetOpacity);
  const animScale = useSharedValue(targetScale);
  const animLift = useSharedValue(targetLift);

  useEffect(() => {
    animOpacity.value = withTiming(targetOpacity, {
      duration: 280,
      easing: Easing.out(Easing.cubic),
    });
    animScale.value = withSpring(targetScale, {
      damping: 22,
      stiffness: 175,
      mass: 0.85,
    });
    animLift.value = withSpring(targetLift, {
      damping: 22,
      stiffness: 175,
      mass: 0.85,
    });
  }, [animLift, animOpacity, animScale, targetLift, targetOpacity, targetScale]);

  const animatedStyle = useAnimatedStyle(() => ({
    opacity: animOpacity.value,
    transform: [{ translateY: animLift.value }, { scale: animScale.value }],
  }));

  const words = useMemo(
    () => (isActive ? resolveLineWords(line, startMs, endMs) : null),
    [isActive, line, startMs, endMs]
  );

  return (
    <Pressable
      onLayout={(e) => onLayoutY(e.nativeEvent.layout.y)}
      onPress={onPress}
      style={({ pressed }) => [
        styles.lineBox,
        line.opposite_aligned && styles.rightAligned,
        pressed && styles.linePressed,
      ]}
    >
      <Animated.View
        style={[
          styles.lineInner,
          line.opposite_aligned && styles.rightAligned,
          animatedStyle,
        ]}
      >
        {isActive && words && words.length > 0 ? (
          <Text
            style={[
              styles.baseLineText,
              styles.activeLineGlow,
              line.is_background && styles.bgLineText,
              line.opposite_aligned && styles.textRight,
            ]}
          >
            {words.map((word, wi) => {
              const dur = Math.max(130, word.end_ms - word.start_ms);
              const rawProg = (smoothPosMs - word.start_ms) / dur;
              const eased = smoothstep(rawProg);
              // Continuous smooth fade 0.34 -> 1.00 across each syllable/word (no harsh single-word halo).
              const alpha = (0.34 + 0.66 * eased).toFixed(2);
              const sep = word.is_part_of_word ? '' : ' ';

              return (
                <Text
                  key={`${word.start_ms}-${wi}`}
                  style={{ color: `rgba(255, 255, 255, ${alpha})` }}
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
              styles.baseLineText,
              styles.idleLineColor,
              line.is_background && styles.bgLineText,
              line.opposite_aligned && styles.textRight,
            ]}
          >
            {line.text}
          </Text>
        )}
      </Animated.View>
    </Pressable>
  );
});

export function SyncedLyricsView({
  lyrics,
  loading,
  positionMs,
  durationMs = 0,
  isPlaying = true,
  fadeColor = '#080A10',
  onSeekToMs,
  onRefreshLyrics,
}: SyncedLyricsViewProps) {
  const scrollRef = useRef<ComponentRef<typeof ScrollView>>(null);
  const itemOffsetsRef = useRef<Record<number, number>>({});
  const userScrollUntilRef = useRef(0);
  const [viewportHeight, setViewportHeight] = useState(420);

  // 60fps interpolated playhead anchored to AVPlayer's position updates so syllable sweeps glide smoothly.
  const clockAnchorRef = useRef({ baseMs: positionMs, wallMs: Date.now() });
  const [smoothPosMs, setSmoothPosMs] = useState(positionMs);

  useEffect(() => {
    clockAnchorRef.current = { baseMs: positionMs, wallMs: Date.now() };
    setSmoothPosMs(positionMs);
  }, [positionMs]);

  useEffect(() => {
    if (!isPlaying || !lyrics || lyrics.synced.length === 0) return;
    let rafId = 0;
    const tick = () => {
      const elapsed = Math.min(180, Math.max(0, Date.now() - clockAnchorRef.current.wallMs));
      setSmoothPosMs(clockAnchorRef.current.baseMs + elapsed);
      rafId = requestAnimationFrame(tick);
    };
    rafId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(rafId);
  }, [isPlaying, lyrics]);

  const timeline = useMemo(
    () => (lyrics ? buildDisplayTimeline(lyrics.synced, durationMs) : []),
    [lyrics, durationMs]
  );

  useEffect(() => {
    itemOffsetsRef.current = {};
  }, [lyrics]);

  const activeIndex = useMemo(
    () => findActiveTimelineIndex(timeline, smoothPosMs),
    [timeline, smoothPosMs]
  );

  useEffect(() => {
    if (activeIndex < 0) return;
    if (Date.now() < userScrollUntilRef.current) return;
    const y = itemOffsetsRef.current[activeIndex];
    if (y !== undefined) {
      const focalOffset = Math.round(viewportHeight * 0.30);
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

  const topSpacer = Math.max(68, Math.round(viewportHeight * 0.25));
  const bottomSpacer = Math.max(150, Math.round(viewportHeight * 0.45));

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
              ? Math.min(1, Math.max(0, (smoothPosMs - item.startMs) / span))
              : distance < 0
                ? 1
                : 0;
            const d1 = isActive ? 0.30 + 0.70 * smoothstep(prog * 3) : 0.20;
            const d2 = isActive ? 0.30 + 0.70 * smoothstep((prog - 0.33) * 3) : 0.20;
            const d3 = isActive ? 0.30 + 0.70 * smoothstep((prog - 0.66) * 3) : 0.20;

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
                  !isActive && { opacity: absDist === 1 ? 0.38 : 0.14 },
                ]}
              >
                <View style={[styles.dot, { opacity: d1, transform: [{ scale: isActive ? 0.9 + d1 * 0.22 : 0.9 }] }]} />
                <View style={[styles.dot, { opacity: d2, transform: [{ scale: isActive ? 0.9 + d2 * 0.22 : 0.9 }] }]} />
                <View style={[styles.dot, { opacity: d3, transform: [{ scale: isActive ? 0.9 + d3 * 0.22 : 0.9 }] }]} />
              </Pressable>
            );
          }

          const { line, startMs, endMs } = item;
          const isOverlappingDuet =
            !isActive &&
            Boolean(line.words && line.words.length > 0) &&
            smoothPosMs >= startMs &&
            smoothPosMs <= endMs;

          return (
            <LyricLineRow
              key={item.key}
              line={line}
              startMs={startMs}
              endMs={endMs}
              isActive={isActive || isOverlappingDuet}
              distance={distance}
              smoothPosMs={isActive || isOverlappingDuet ? smoothPosMs : 0}
              onLayoutY={(y) => {
                itemOffsetsRef.current[index] = y;
              }}
              onPress={() => {
                userScrollUntilRef.current = 0;
                onSeekToMs(line.time_ms);
              }}
            />
          );
        })}

        <View style={{ height: bottomSpacer }} />
      </ScrollView>

      {/* Soft top & bottom gradient edge fades so lyrics melt seamlessly into the backdrop */}
      <LinearGradient
        pointerEvents="none"
        colors={[fadeColor, 'transparent']}
        style={styles.topFade}
      />
      <LinearGradient
        pointerEvents="none"
        colors={['transparent', fadeColor]}
        style={styles.bottomFade}
      />

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
    position: 'relative',
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
    paddingHorizontal: 26,
  },
  plainContent: {
    paddingHorizontal: 24,
    paddingVertical: 20,
    paddingBottom: 60,
  },
  topFade: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    height: 36,
  },
  bottomFade: {
    position: 'absolute',
    bottom: 44,
    left: 0,
    right: 0,
    height: 44,
  },
  interludeRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
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
  lineBox: {
    paddingVertical: 10,
    borderRadius: 12,
  },
  lineInner: {
    width: '100%',
  },
  linePressed: {
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
  },
  rightAligned: {
    alignItems: 'flex-end',
  },
  baseLineText: {
    fontSize: 25,
    lineHeight: 34,
    fontWeight: '800',
    letterSpacing: -0.35,
  },
  idleLineColor: {
    color: '#FFFFFF',
  },
  activeLineGlow: {
    textShadowColor: 'rgba(255, 255, 255, 0.22)',
    textShadowOffset: { width: 0, height: 0 },
    textShadowRadius: 6,
  },
  bgLineText: {
    fontSize: 18.5,
    lineHeight: 26,
    fontWeight: '700',
    fontStyle: 'italic',
  },
  textRight: {
    textAlign: 'right',
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
