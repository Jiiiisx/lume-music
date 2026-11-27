import React from 'react';
import {
  ActivityIndicator,
  Image,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import * as Haptics from 'expo-haptics';
import { useRouter } from 'expo-router';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import { usePlayer } from '../src/context/PlayerContext';
import { LumeTheme } from '../src/theme';
import { formatDurationMs, trackArtistDisplay } from '../src/types';

/**
 * Driving-first screen: high contrast, giant touch targets, and full-screen swipe left/right
 * for next/previous tracks so the driver never has to aim at tiny buttons.
 */
export default function DrivingModeScreen() {
  const router = useRouter();
  const {
    currentTrack,
    currentStream,
    likedUids,
    isPlaying,
    isBuffering,
    positionMs,
    durationMs,
    togglePlayPause,
    nextTrack,
    previousTrack,
    toggleLikeCurrent,
  } = usePlayer();
  const isLiked = currentTrack ? likedUids.has(currentTrack.uid) : false;

  const swipeGesture = Gesture.Pan()
    .runOnJS(true)
    .activeOffsetX([-40, 40])
    .onEnd((e) => {
      if (e.translationX < -60) {
        Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium).catch(() => {});
        void nextTrack();
      } else if (e.translationX > 60) {
        Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium).catch(() => {});
        void previousTrack();
      }
    });

  const progress = durationMs > 0 ? Math.min(1, Math.max(0, positionMs / durationMs)) : 0;

  return (
    <GestureDetector gesture={swipeGesture}>
      <SafeAreaView style={styles.root}>
        <View style={styles.header}>
          <View style={styles.badge}>
            <Ionicons name="car-sport" size={16} color={LumeTheme.emerald} />
            <Text style={styles.badgeText}>
              {currentStream?.is_offline ? 'DRIVING • OFFLINE' : 'DRIVING MODE'}
            </Text>
          </View>

          <Pressable
            onPress={() => router.back()}
            style={styles.exitBtn}
            hitSlop={12}
          >
            <Ionicons name="close" size={24} color="#FFFFFF" />
            <Text style={styles.exitText}>Keluar</Text>
          </Pressable>
        </View>

        <View style={styles.center}>
          {currentTrack?.artwork_ref ? (
            <Image source={{ uri: currentTrack.artwork_ref }} style={styles.art} />
          ) : (
            <View style={[styles.art, styles.artFallback]}>
              <Ionicons name="musical-notes" size={64} color={LumeTheme.accentBright} />
            </View>
          )}

          <Text style={styles.title} numberOfLines={2}>
            {currentTrack?.title ?? 'Pilih lagu untuk mulai berkendara'}
          </Text>
          <Text style={styles.artist} numberOfLines={1}>
            {currentTrack ? trackArtistDisplay(currentTrack) : 'Swipe kiri/kanan untuk ganti lagu'}
          </Text>

          <View style={styles.progressTrack}>
            <View style={[styles.progressFill, { width: `${progress * 100}%` }]} />
          </View>
          <Text style={styles.timeText}>
            {formatDurationMs(positionMs)} / {formatDurationMs(durationMs)}
          </Text>
        </View>

        <View style={styles.controls}>
          <Pressable
            onPress={() => {
              Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium).catch(() => {});
              void previousTrack();
            }}
            style={styles.sideBtn}
            accessibilityLabel="Lagu Sebelumnya"
          >
            <Ionicons name="play-skip-back" size={44} color="#FFFFFF" />
          </Pressable>

          <Pressable
            onPress={togglePlayPause}
            style={styles.giantPlayBtn}
            accessibilityLabel={isPlaying ? 'Pause' : 'Play'}
          >
            {isBuffering ? (
              <ActivityIndicator size="large" color="#0A0C12" />
            ) : (
              <Ionicons
                name={isPlaying ? 'pause' : 'play'}
                size={54}
                color="#0A0C12"
              />
            )}
          </Pressable>

          <Pressable
            onPress={() => {
              Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium).catch(() => {});
              void nextTrack();
            }}
            style={styles.sideBtn}
            accessibilityLabel="Lagu Berikutnya"
          >
            <Ionicons name="play-skip-forward" size={44} color="#FFFFFF" />
          </Pressable>
        </View>

        {currentTrack && (
          <Pressable
            onPress={() => void toggleLikeCurrent()}
            style={styles.likeWideBtn}
          >
            <Ionicons
              name={isLiked ? 'heart' : 'heart-outline'}
              size={24}
              color={isLiked ? LumeTheme.rose : '#FFFFFF'}
            />
            <Text style={styles.likeWideText}>
              {isLiked ? 'Disukai' : 'Sukai Lagu Ini'}
            </Text>
          </Pressable>
        )}
      </SafeAreaView>
    </GestureDetector>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
    backgroundColor: '#05070B',
    paddingHorizontal: 20,
    justifyContent: 'space-between',
    paddingBottom: 24,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingTop: 8,
  },
  badge: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    backgroundColor: 'rgba(16, 185, 129, 0.16)',
    borderWidth: 1,
    borderColor: 'rgba(16, 185, 129, 0.45)',
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 999,
  },
  badgeText: {
    color: LumeTheme.emerald,
    fontWeight: '800',
    fontSize: 12.5,
    letterSpacing: 0.8,
  },
  exitBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    backgroundColor: 'rgba(255,255,255,0.12)',
    paddingHorizontal: 14,
    paddingVertical: 8,
    borderRadius: 999,
  },
  exitText: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 14,
  },
  center: {
    alignItems: 'center',
    gap: 12,
  },
  art: {
    width: 170,
    height: 170,
    borderRadius: 24,
    marginBottom: 6,
  },
  artFallback: {
    backgroundColor: '#121622',
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    color: '#FFFFFF',
    fontSize: 28,
    fontWeight: '900',
    textAlign: 'center',
  },
  artist: {
    color: LumeTheme.textSecondary,
    fontSize: 19,
    fontWeight: '600',
    textAlign: 'center',
  },
  progressTrack: {
    width: '100%',
    height: 8,
    borderRadius: 4,
    backgroundColor: 'rgba(255,255,255,0.16)',
    overflow: 'hidden',
    marginTop: 10,
  },
  progressFill: {
    height: 8,
    backgroundColor: LumeTheme.accentBright,
  },
  timeText: {
    color: LumeTheme.textSecondary,
    fontSize: 14,
    fontWeight: '600',
  },
  controls: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: 14,
  },
  sideBtn: {
    flex: 1,
    height: 108,
    borderRadius: 28,
    backgroundColor: 'rgba(255,255,255,0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  giantPlayBtn: {
    width: 124,
    height: 124,
    borderRadius: 62,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  likeWideBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 10,
    height: 58,
    borderRadius: 18,
    backgroundColor: 'rgba(255,255,255,0.08)',
  },
  likeWideText: {
    color: '#FFFFFF',
    fontSize: 16,
    fontWeight: '700',
  },
});
