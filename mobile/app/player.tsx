import React, { useState } from 'react';
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
import { BlurView } from 'expo-blur';
import { LinearGradient } from 'expo-linear-gradient';
import { useRouter } from 'expo-router';
import { SyncedLyricsView } from '../src/components/SyncedLyricsView';
import { usePlayer } from '../src/context/PlayerContext';
import { LumeTheme } from '../src/theme';
import { formatDurationMs, trackArtistDisplay } from '../src/types';

export default function PlayerModal() {
  const router = useRouter();
  const {
    currentTrack,
    currentStream,
    palette,
    likedUids,
    isPlaying,
    isBuffering,
    positionMs,
    durationMs,
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
  const [barWidth, setBarWidth] = useState(1);

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

  const progress = durationMs > 0 ? Math.min(1, Math.max(0, positionMs / durationMs)) : 0;
  const isLiked = likedUids.has(currentTrack.uid);
  const isOffline = offlineUids.has(currentTrack.uid);
  const isDownloading = downloadingUids.has(currentTrack.uid);

  return (
    <View style={styles.root}>
      {currentTrack.artwork_ref && (
        <Image
          source={{ uri: currentTrack.artwork_ref }}
          style={StyleSheet.absoluteFill}
          blurRadius={44}
        />
      )}
      <BlurView intensity={75} tint="dark" style={StyleSheet.absoluteFill} />
      <LinearGradient
        colors={[palette.bg_top + 'AA', palette.bg_mid + 'D9', palette.bg_bottom]}
        style={StyleSheet.absoluteFill}
      />

      <SafeAreaView style={styles.safe}>
        <View style={styles.topBar}>
          <Pressable onPress={() => router.back()} style={styles.topBtn} hitSlop={10}>
            <Ionicons name="chevron-down" size={26} color={LumeTheme.textPrimary} />
          </Pressable>

          <View style={styles.modeSwitch}>
            <Pressable
              onPress={() => setShowLyrics(false)}
              style={[
                styles.modePill,
                !showLyrics && { backgroundColor: palette.accent },
              ]}
            >
              <Text style={[styles.modePillText, !showLyrics && styles.modePillDark]}>
                Player
              </Text>
            </Pressable>
            <Pressable
              onPress={() => setShowLyrics(true)}
              style={[
                styles.modePill,
                showLyrics && { backgroundColor: palette.accent },
              ]}
            >
              <Text style={[styles.modePillText, showLyrics && styles.modePillDark]}>
                Lyrics
              </Text>
            </Pressable>
          </View>

          <View style={styles.topRight}>
            <Pressable
              onPress={() => router.push('/queue')}
              style={styles.topBtn}
              hitSlop={8}
              accessibilityLabel="Queue"
            >
              <Ionicons name="list" size={22} color="#FFFFFF" />
            </Pressable>
            <Pressable
              onPress={() => router.push('/driving')}
              style={styles.topBtn}
              hitSlop={8}
              accessibilityLabel="Driving Mode"
            >
              <Ionicons name="car-sport" size={21} color={palette.accent} />
            </Pressable>
          </View>
        </View>

        {showLyrics ? (
          <View style={styles.lyricsContainer}>
            <SyncedLyricsView
              lyrics={lyrics}
              loading={lyricsLoading}
              positionMs={positionMs}
              onSeekToMs={(ms) => void seekToMs(ms)}
              onRefreshLyrics={() => void refreshLyrics()}
            />
          </View>
        ) : (
          <View style={styles.artContainer}>
            {currentTrack.artwork_ref ? (
              <Image source={{ uri: currentTrack.artwork_ref }} style={styles.coverArt} />
            ) : (
              <View style={[styles.coverArt, styles.coverFallback]}>
                <Ionicons name="musical-notes" size={76} color={palette.accent} />
              </View>
            )}
          </View>
        )}

        <View style={styles.bottomSection}>
          <View style={styles.metaRow}>
            <View style={{ flex: 1 }}>
              <Text style={styles.trackTitle} numberOfLines={1}>
                {currentTrack.title}
              </Text>
              <Text style={styles.trackArtist} numberOfLines={1}>
                {trackArtistDisplay(currentTrack)}
                {currentStream?.is_offline ? '  •  Offline' : ''}
              </Text>
            </View>

            <Pressable
              onPress={() => void downloadOffline(currentTrack)}
              style={styles.metaBtn}
              hitSlop={8}
            >
              {isDownloading ? (
                <ActivityIndicator size="small" color={palette.accent} />
              ) : (
                <Ionicons
                  name={isOffline ? 'cloud-done' : 'cloud-download-outline'}
                  size={22}
                  color={isOffline ? LumeTheme.emerald : LumeTheme.textSecondary}
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
                color={isLiked ? LumeTheme.rose : LumeTheme.textPrimary}
              />
            </Pressable>

            <Pressable
              onPress={() => openTrackMenu(currentTrack)}
              style={styles.metaBtn}
              hitSlop={8}
            >
              <Ionicons name="ellipsis-horizontal" size={22} color="#FFFFFF" />
            </Pressable>
          </View>

          <Pressable
            onLayout={(e: LayoutChangeEvent) =>
              setBarWidth(Math.max(1, e.nativeEvent.layout.width))
            }
            onPress={(e) => {
              const ratio = Math.min(1, Math.max(0, e.nativeEvent.locationX / barWidth));
              void seekToMs(ratio * durationMs);
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

          <View style={styles.controlsRow}>
            <Pressable onPress={toggleShuffle} style={styles.ctrlBtn} hitSlop={8}>
              <Ionicons
                name="shuffle"
                size={22}
                color={shuffle ? palette.accent : LumeTheme.textMuted}
              />
            </Pressable>

            <Pressable
              onPress={() => void previousTrack()}
              style={styles.ctrlBtn}
              hitSlop={8}
            >
              <Ionicons name="play-skip-back" size={32} color={LumeTheme.textPrimary} />
            </Pressable>

            <Pressable onPress={togglePlayPause} style={styles.playCircle}>
              {isBuffering ? (
                <ActivityIndicator size="large" color="#0A0C12" />
              ) : (
                <Ionicons
                  name={isPlaying ? 'pause' : 'play'}
                  size={34}
                  color="#0A0C12"
                />
              )}
            </Pressable>

            <Pressable
              onPress={() => void nextTrack()}
              style={styles.ctrlBtn}
              hitSlop={8}
            >
              <Ionicons name="play-skip-forward" size={32} color={LumeTheme.textPrimary} />
            </Pressable>

            <Pressable onPress={cycleRepeat} style={styles.ctrlBtn} hitSlop={8}>
              <Ionicons
                name={repeat === 'one' ? 'repeat-outline' : 'repeat'}
                size={22}
                color={repeat !== 'off' ? palette.accent : LumeTheme.textMuted}
              />
              {repeat === 'one' && (
                <Text style={[styles.repeatOne, { color: palette.accent }]}>1</Text>
              )}
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
    backgroundColor: LumeTheme.accent,
    paddingHorizontal: 20,
    paddingVertical: 10,
    borderRadius: 999,
  },
  closePillText: {
    color: '#FFFFFF',
    fontWeight: '700',
  },
  topBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 14,
    paddingVertical: 8,
  },
  topRight: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  topBtn: {
    width: 38,
    height: 38,
    alignItems: 'center',
    justifyContent: 'center',
  },
  modeSwitch: {
    flexDirection: 'row',
    backgroundColor: 'rgba(255,255,255,0.12)',
    borderRadius: 999,
    padding: 3,
  },
  modePill: {
    paddingHorizontal: 16,
    paddingVertical: 6,
    borderRadius: 999,
  },
  modePillText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 13,
  },
  modePillDark: {
    color: '#0A0C12',
    fontWeight: '800',
  },
  artContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 28,
  },
  coverArt: {
    width: '100%',
    aspectRatio: 1,
    maxWidth: 340,
    borderRadius: 24,
  },
  coverFallback: {
    backgroundColor: LumeTheme.bgElevated,
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
    gap: 4,
    marginBottom: 14,
  },
  trackTitle: {
    color: '#FFFFFF',
    fontSize: 22,
    fontWeight: '800',
  },
  trackArtist: {
    color: LumeTheme.textSecondary,
    fontSize: 15,
    marginTop: 3,
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
    backgroundColor: 'rgba(255,255,255,0.22)',
    overflow: 'hidden',
  },
  seekFill: {
    height: 5,
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
  },
  controlsRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
  },
  ctrlBtn: {
    width: 48,
    height: 48,
    alignItems: 'center',
    justifyContent: 'center',
  },
  playCircle: {
    width: 72,
    height: 72,
    borderRadius: 36,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  repeatOne: {
    position: 'absolute',
    bottom: 6,
    right: 8,
    fontSize: 10,
    fontWeight: '900',
  },
});
