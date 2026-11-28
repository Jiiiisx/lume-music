import React, { useEffect, useState } from 'react';
import {
  ActivityIndicator,
  Image,
  LayoutChangeEvent,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { LinearGradient } from 'expo-linear-gradient';
import { useRouter } from 'expo-router';
import Animated, {
  FadeIn,
  FadeOut,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';
import { SyncedLyricsView } from '../src/components/SyncedLyricsView';
import { usePlaybackProgress, usePlayer } from '../src/context/PlayerContext';
import { LumeTheme } from '../src/theme';
import { formatDurationMs, type LyricsData, trackArtistDisplay } from '../src/types';

const PlayerLyricsPanel = React.memo(function PlayerLyricsPanel({
  lyrics,
  loading,
  isPlaying,
  fadeColor,
  onSeekToMs,
  onRefreshLyrics,
}: {
  lyrics: LyricsData | null;
  loading: boolean;
  isPlaying: boolean;
  fadeColor: string;
  onSeekToMs: (ms: number) => void;
  onRefreshLyrics: () => void;
}) {
  const { positionMs, durationMs } = usePlaybackProgress();
  return (
    <SyncedLyricsView
      lyrics={lyrics}
      loading={loading}
      positionMs={positionMs}
      durationMs={durationMs}
      isPlaying={isPlaying}
      fadeColor={fadeColor}
      onSeekToMs={onSeekToMs}
      onRefreshLyrics={onRefreshLyrics}
    />
  );
});

const PlayerSeekSection = React.memo(function PlayerSeekSection({
  onSeekToMs,
}: {
  onSeekToMs: (ms: number) => void;
}) {
  const { positionMs, durationMs } = usePlaybackProgress();
  const [barWidth, setBarWidth] = useState(1);
  const progress = durationMs > 0 ? Math.min(1, Math.max(0, positionMs / durationMs)) : 0;

  return (
    <>
      <Pressable
        onLayout={(e: LayoutChangeEvent) =>
          setBarWidth(Math.max(1, e.nativeEvent.layout.width))
        }
        onPress={(e) => {
          const ratio = Math.min(1, Math.max(0, e.nativeEvent.locationX / barWidth));
          onSeekToMs(ratio * durationMs);
        }}
        style={styles.seekHitArea}
      >
        <View style={styles.seekTrack}>
          <View style={[styles.seekFill, { width: `${progress * 100}%` }]} />
        </View>
      </Pressable>

      <View style={styles.timeRow}>
        <Text style={styles.timeText}>{formatDurationMs(positionMs)}</Text>
        <Text style={styles.timeText}>{formatDurationMs(durationMs)}</Text>
      </View>
    </>
  );
});

export default function PlayerModal() {
  const router = useRouter();
  const {
    currentTrack,
    currentStream,
    palette,
    likedUids,
    isPlaying,
    isBuffering,
    shuffle,
    repeat,
    lyrics,
    lyricsLoading,
    offlineUids,
    downloadingUids,
    togglePlayPause,
    nextTrack,
    previousTrack,
    seekToMs,
    toggleShuffle,
    cycleRepeat,
    toggleLikeCurrent,
    downloadOffline,
    refreshLyrics,
    openTrackMenu,
  } = usePlayer();

  const [showLyrics, setShowLyrics] = useState(false);

  // Smooth spring thumb for the Player | Lyrics segmented control + Apple-Music cover art breath.
  const tabProgress = useSharedValue(0);
  const coverScale = useSharedValue(isPlaying ? 1 : 0.93);

  useEffect(() => {
    tabProgress.value = withSpring(showLyrics ? 1 : 0, {
      damping: 22,
      stiffness: 260,
      mass: 0.75,
    });
  }, [showLyrics, tabProgress]);

  useEffect(() => {
    coverScale.value = withSpring(isPlaying ? 1 : 0.93, {
      damping: 18,
      stiffness: 150,
      mass: 0.9,
    });
  }, [isPlaying, coverScale]);

  const thumbStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: tabProgress.value * 82 }],
  }));

  const coverAnimatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: coverScale.value }],
  }));

  if (!currentTrack) {
    return (
      <SafeAreaView style={styles.emptyRoot}>
        <Text style={styles.emptyText}>Belum ada lagu yang diputar.</Text>
        <Pressable onPress={() => router.back()} style={styles.closePill}>
          <Text style={styles.closePillText}>Kembali</Text>
        </Pressable>
      </SafeAreaView>
    );
  }

  const isLiked = likedUids.has(currentTrack.uid);
  const isOffline = offlineUids.has(currentTrack.uid);
  const isDownloading = downloadingUids.has(currentTrack.uid);

  return (
    <View style={styles.root}>
      {currentTrack.artwork_ref && (
        <Image
          source={{ uri: currentTrack.artwork_ref }}
          style={styles.backdropImg}
          blurRadius={70}
          resizeMode="cover"
          fadeDuration={0}
        />
      )}
      <LinearGradient
        colors={[
          palette.bg_top + '8A',
          'rgba(8, 10, 16, 0.74)',
          'rgba(7, 9, 14, 0.96)',
        ]}
        locations={[0, 0.52, 1]}
        style={StyleSheet.absoluteFill}
      />

      <SafeAreaView style={styles.safe}>
        <View style={styles.topBar}>
          <Pressable onPress={() => router.back()} style={styles.topCircleBtn} hitSlop={10}>
            <Ionicons name="chevron-down" size={22} color="#FFFFFF" />
          </Pressable>

          <View style={styles.modeSwitch}>
            <Animated.View style={[styles.modeThumb, thumbStyle]} pointerEvents="none" />
            <Pressable onPress={() => setShowLyrics(false)} style={styles.modePill}>
              <Text style={[styles.modePillText, !showLyrics && styles.modePillDark]}>
                Player
              </Text>
            </Pressable>
            <Pressable onPress={() => setShowLyrics(true)} style={styles.modePill}>
              <Text style={[styles.modePillText, showLyrics && styles.modePillDark]}>
                Lyrics
              </Text>
            </Pressable>
          </View>

          <View style={styles.topRight}>
            <Pressable
              onPress={() => router.push('/queue')}
              style={styles.topCircleBtn}
              hitSlop={8}
              accessibilityLabel="Queue"
            >
              <Ionicons name="list" size={19} color="#FFFFFF" />
            </Pressable>
            <Pressable
              onPress={() => router.push('/driving')}
              style={styles.topCircleBtn}
              hitSlop={8}
              accessibilityLabel="Driving Mode"
            >
              <Ionicons name="car-sport-outline" size={19} color="#FFFFFF" />
            </Pressable>
          </View>
        </View>

        {showLyrics ? (
          <Animated.View
            key="lyrics-view"
            entering={FadeIn.duration(220)}
            exiting={FadeOut.duration(150)}
            style={styles.lyricsContainer}
          >
            <PlayerLyricsPanel
              lyrics={lyrics}
              loading={lyricsLoading}
              isPlaying={isPlaying}
              fadeColor="rgba(9, 11, 18, 0.88)"
              onSeekToMs={(ms) => void seekToMs(ms)}
              onRefreshLyrics={() => void refreshLyrics()}
            />
          </Animated.View>
        ) : (
          <Animated.View
            key="cover-view"
            entering={FadeIn.duration(220)}
            exiting={FadeOut.duration(150)}
            style={styles.artContainer}
          >
            <Animated.View style={[styles.coverFrame, coverAnimatedStyle]}>
              {currentTrack.artwork_ref ? (
                <Image source={{ uri: currentTrack.artwork_ref }} style={styles.coverArt} />
              ) : (
                <View style={[styles.coverArt, styles.coverFallback]}>
                  <Ionicons name="musical-notes" size={76} color="rgba(255,255,255,0.65)" />
                </View>
              )}
            </Animated.View>
          </Animated.View>
        )}

        <View style={styles.bottomSection}>
          <View style={styles.metaRow}>
            <View style={{ flex: 1 }}>
              <Text style={styles.trackTitle} numberOfLines={1}>
                {currentTrack.title}
              </Text>
              <View style={styles.artistLine}>
                {currentStream?.is_offline && (
                  <Ionicons name="checkmark-circle" size={14} color={LumeTheme.emerald} />
                )}
                <Text style={styles.trackArtist} numberOfLines={1}>
                  {trackArtistDisplay(currentTrack)}
                  {currentStream?.is_offline ? '  •  Offline' : ''}
                </Text>
              </View>
            </View>

            <Pressable
              onPress={() => void downloadOffline(currentTrack)}
              style={styles.metaBtn}
              hitSlop={8}
            >
              {isDownloading ? (
                <ActivityIndicator size="small" color="#FFFFFF" />
              ) : (
                <Ionicons
                  name={isOffline ? 'cloud-done' : 'cloud-download-outline'}
                  size={21}
                  color={isOffline ? LumeTheme.emerald : 'rgba(255,255,255,0.78)'}
                />
              )}
            </Pressable>

            <Pressable
              onPress={() => void toggleLikeCurrent()}
              style={styles.metaBtn}
              hitSlop={8}
            >
              <Ionicons
                name={isLiked ? 'heart' : 'heart-outline'}
                size={23}
                color={isLiked ? LumeTheme.rose : '#FFFFFF'}
              />
            </Pressable>

            <Pressable
              onPress={() => openTrackMenu(currentTrack)}
              style={styles.metaBtn}
              hitSlop={8}
            >
              <Ionicons name="ellipsis-horizontal" size={21} color="rgba(255,255,255,0.85)" />
            </Pressable>
          </View>

          <PlayerSeekSection onSeekToMs={(ms) => void seekToMs(ms)} />

          <View style={styles.controlsRow}>
            <Pressable
              onPress={toggleShuffle}
              style={[styles.ctrlBtn, shuffle && styles.ctrlBtnActive]}
              hitSlop={8}
              accessibilityLabel={shuffle ? 'Shuffle On' : 'Shuffle Off'}
            >
              <Ionicons
                name="shuffle"
                size={21}
                color={shuffle ? '#FFFFFF' : 'rgba(255,255,255,0.44)'}
              />
              {shuffle && <View style={styles.activeDot} />}
            </Pressable>

            <Pressable
              onPress={() => void previousTrack()}
              style={styles.ctrlBtn}
              hitSlop={8}
            >
              <Ionicons name="play-skip-back" size={32} color="#FFFFFF" />
            </Pressable>

            <Pressable
              onPress={togglePlayPause}
              style={({ pressed }) => [styles.playCircle, pressed && styles.playPressed]}
            >
              {isBuffering ? (
                <ActivityIndicator size="large" color={LumeTheme.inkDark} />
              ) : (
                <Ionicons
                  name={isPlaying ? 'pause' : 'play'}
                  size={34}
                  color={LumeTheme.inkDark}
                  style={!isPlaying ? { marginLeft: 3 } : undefined}
                />
              )}
            </Pressable>

            <Pressable
              onPress={() => void nextTrack()}
              style={styles.ctrlBtn}
              hitSlop={8}
            >
              <Ionicons name="play-skip-forward" size={32} color="#FFFFFF" />
            </Pressable>

            <Pressable
              onPress={cycleRepeat}
              style={[styles.ctrlBtn, repeat !== 'off' && styles.ctrlBtnActive]}
              hitSlop={8}
              accessibilityLabel={`Repeat ${repeat}`}
            >
              <Ionicons
                name="repeat"
                size={21}
                color={repeat !== 'off' ? '#FFFFFF' : 'rgba(255,255,255,0.44)'}
              />
              {repeat === 'one' && (
                <View style={styles.repeatOneBadge}>
                  <Text style={styles.repeatOne}>1</Text>
                </View>
              )}
              {repeat !== 'off' && <View style={styles.activeDot} />}
            </Pressable>
          </View>
        </View>
      </SafeAreaView>
    </View>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
  },
  backdropImg: {
    position: 'absolute',
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    opacity: 0.62,
    transform: [{ scale: 1.18 }],
  },
  safe: {
    flex: 1,
    justifyContent: 'space-between',
  },
  emptyRoot: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
    alignItems: 'center',
    justifyContent: 'center',
    gap: 16,
  },
  emptyText: {
    color: LumeTheme.textSecondary,
    fontSize: 16,
  },
  closePill: {
    backgroundColor: '#FFFFFF',
    paddingHorizontal: 22,
    paddingVertical: 10,
    borderRadius: 999,
  },
  closePillText: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
  },
  topBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  topRight: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  topCircleBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  modeSwitch: {
    position: 'relative',
    flexDirection: 'row',
    backgroundColor: 'rgba(255, 255, 255, 0.11)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    borderRadius: 999,
    padding: 3,
    width: 170,
  },
  modeThumb: {
    position: 'absolute',
    top: 3,
    left: 3,
    width: 82,
    bottom: 3,
    borderRadius: 999,
    backgroundColor: '#FFFFFF',
  },
  modePill: {
    width: 82,
    paddingVertical: 6,
    borderRadius: 999,
    alignItems: 'center',
    justifyContent: 'center',
  },
  modePillText: {
    color: 'rgba(255, 255, 255, 0.82)',
    fontWeight: '700',
    fontSize: 13,
  },
  modePillDark: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
  },
  artContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 26,
  },
  coverFrame: {
    width: '100%',
    maxWidth: 344,
    aspectRatio: 1,
    borderRadius: 26,
    shadowColor: '#000000',
    shadowOpacity: 0.55,
    shadowRadius: 28,
    shadowOffset: { width: 0, height: 14 },
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.18)',
    overflow: 'hidden',
  },
  coverArt: {
    width: '100%',
    height: '100%',
  },
  coverFallback: {
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  lyricsContainer: {
    flex: 1,
  },
  bottomSection: {
    paddingHorizontal: 24,
    paddingBottom: 20,
  },
  metaRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 2,
    marginBottom: 14,
  },
  trackTitle: {
    color: '#FFFFFF',
    fontSize: 22,
    fontWeight: '800',
    letterSpacing: -0.3,
  },
  artistLine: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    marginTop: 4,
  },
  trackArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 15,
    flexShrink: 1,
  },
  metaBtn: {
    width: 34,
    height: 38,
    alignItems: 'center',
    justifyContent: 'center',
  },
  seekHitArea: {
    paddingVertical: 10,
  },
  seekTrack: {
    height: 5,
    borderRadius: 3,
    backgroundColor: 'rgba(255, 255, 255, 0.22)',
    overflow: 'hidden',
  },
  seekFill: {
    height: 5,
    borderRadius: 3,
    backgroundColor: '#FFFFFF',
  },
  timeRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    marginBottom: 18,
  },
  timeText: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    fontVariant: ['tabular-nums'],
  },
  controlsRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  ctrlBtn: {
    width: 46,
    height: 46,
    borderRadius: 23,
    alignItems: 'center',
    justifyContent: 'center',
  },
  ctrlBtnActive: {
    backgroundColor: 'rgba(255, 255, 255, 0.18)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.28)',
  },
  playCircle: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
    shadowColor: '#000000',
    shadowOpacity: 0.35,
    shadowRadius: 14,
    shadowOffset: { width: 0, height: 6 },
  },
  playPressed: {
    transform: [{ scale: 0.94 }],
  },
  activeDot: {
    position: 'absolute',
    bottom: 5,
    width: 4,
    height: 4,
    borderRadius: 2,
    backgroundColor: '#FFFFFF',
  },
  repeatOneBadge: {
    position: 'absolute',
    top: 5,
    right: 6,
    minWidth: 14,
    height: 14,
    borderRadius: 7,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 2,
  },
  repeatOne: {
    color: LumeTheme.inkDark,
    fontSize: 9,
    fontWeight: '900',
  },
});
