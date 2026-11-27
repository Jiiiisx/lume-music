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
  const isOffline = offlineUids.has(currentTrack.uid);
  const isDownloading = downloadingUids.has(currentTrack.uid);

  return (
    <View style={styles.root}>
      {currentTrack.artwork_ref && (
        <Image
          source={{ uri: currentTrack.artwork_ref }}
          style={StyleSheet.absoluteFill}
          blurRadius={40}
        />
      )}
      <BlurView intensity={75} tint="dark" style={StyleSheet.absoluteFill} />
      <LinearGradient
        colors={['rgba(10,12,18,0.45)', 'rgba(10,12,18,0.88)', '#0A0C12']}
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
              style={[styles.modePill, !showLyrics && styles.modePillActive]}
            >
              <Text style={styles.modePillText}>Player</Text>
            </Pressable>
            <Pressable
              onPress={() => setShowLyrics(true)}
              style={[styles.modePill, showLyrics && styles.modePillActive]}
            >
              <Text style={styles.modePillText}>Lyrics</Text>
            </Pressable>
          </View>

          <Pressable
            onPress={() => router.push('/driving')}
            style={styles.topBtn}
            hitSlop={10}
            accessibilityLabel="Driving Mode"
          >
            <Ionicons name="car-sport" size={22} color={LumeTheme.accentBright} />
          </Pressable>
        </View>

        {showLyrics ? (
          <View style={styles.lyricsContainer}>
            <SyncedLyricsView
              lyrics={lyrics}
              loading={lyricsLoading}
              positionMs={positionMs}
              onSeekToMs={(ms) => void seekToMs(ms)}
            />
          </View>
        ) : (
          <View style={styles.artContainer}>
            {currentTrack.artwork_ref ? (
              <Image source={{ uri: currentTrack.artwork_ref }} style={styles.coverArt} />
            ) : (
              <View style={[styles.coverArt, styles.coverFallback]}>
                <Ionicons name="musical-notes" size={76} color={LumeTheme.accentBright} />
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
                <ActivityIndicator size="small" color={LumeTheme.accentBright} />
              ) : (
                <Ionicons
                  name={isOffline ? 'cloud-done' : 'cloud-download-outline'}
                  size={23}
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
                name={currentTrack.is_liked ? 'heart' : 'heart-outline'}
                size={24}
                color={currentTrack.is_liked ? LumeTheme.rose : LumeTheme.textPrimary}
              />
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
                color={shuffle ? LumeTheme.accentBright : LumeTheme.textMuted}
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
                color={repeat !== 'off' ? LumeTheme.accentBright : LumeTheme.textMuted}
              />
              {repeat === 'one' && <Text style={styles.repeatOne}>1</Text>}
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
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  topBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  modeSwitch: {
    flexDirection: 'row',
    backgroundColor: 'rgba(255,255,255,0.10)',
    borderRadius: 999,
    padding: 3,
  },
  modePill: {
    paddingHorizontal: 16,
    paddingVertical: 6,
    borderRadius: 999,
  },
  modePillActive: {
    backgroundColor: LumeTheme.accent,
  },
  modePillText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 13,
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
    gap: 10,
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
    width: 38,
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
    backgroundColor: 'rgba(255,255,255,0.20)',
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
    color: LumeTheme.accentBright,
    fontSize: 10,
    fontWeight: '900',
  },
});
