import React from 'react';
import { ActivityIndicator, Image, Pressable, StyleSheet, Text, View } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { BlurView } from 'expo-blur';
import { useRouter } from 'expo-router';
import { usePlayer } from '../context/PlayerContext';
import { LumeTheme } from '../theme';
import { trackArtistDisplay } from '../types';

export function MiniPlayer() {
  const router = useRouter();
  const {
    currentTrack,
    currentStream,
    isPlaying,
    isBuffering,
    positionMs,
    durationMs,
    togglePlayPause,
    nextTrack,
  } = usePlayer();

  if (!currentTrack) return null;

  const progress = durationMs > 0 ? Math.min(1, Math.max(0, positionMs / durationMs)) : 0;

  return (
    <Pressable
      onPress={() => router.push('/player')}
      style={styles.outer}
      accessibilityRole="button"
      accessibilityLabel={`Now playing ${currentTrack.title}`}
    >
      <BlurView intensity={48} tint="dark" style={styles.blur}>
        <View style={styles.row}>
          {currentTrack.artwork_ref ? (
            <Image source={{ uri: currentTrack.artwork_ref }} style={styles.art} />
          ) : (
            <View style={[styles.art, styles.artFallback]}>
              <Ionicons name="musical-notes" size={20} color={LumeTheme.accentBright} />
            </View>
          )}

          <View style={styles.meta}>
            <View style={styles.titleRow}>
              <Text style={styles.title} numberOfLines={1}>
                {currentTrack.title}
              </Text>
              {currentStream?.is_offline && (
                <View style={styles.offlineBadge}>
                  <Text style={styles.offlineBadgeText}>OFFLINE</Text>
                </View>
              )}
            </View>
            <Text style={styles.artist} numberOfLines={1}>
              {trackArtistDisplay(currentTrack)}
            </Text>
          </View>

          <Pressable
            onPress={(e) => {
              e.stopPropagation();
              router.push('/driving');
            }}
            style={styles.iconBtn}
            hitSlop={8}
            accessibilityLabel="Open Driving Mode"
          >
            <Ionicons name="car-sport-outline" size={20} color={LumeTheme.accentBright} />
          </Pressable>

          <Pressable
            onPress={(e) => {
              e.stopPropagation();
              togglePlayPause();
            }}
            style={styles.playBtn}
            hitSlop={8}
            accessibilityLabel={isPlaying ? 'Pause' : 'Play'}
          >
            {isBuffering ? (
              <ActivityIndicator size="small" color="#FFFFFF" />
            ) : (
              <Ionicons name={isPlaying ? 'pause' : 'play'} size={20} color="#FFFFFF" />
            )}
          </Pressable>

          <Pressable
            onPress={(e) => {
              e.stopPropagation();
              void nextTrack();
            }}
            style={styles.iconBtn}
            hitSlop={8}
            accessibilityLabel="Next track"
          >
            <Ionicons name="play-skip-forward" size={20} color={LumeTheme.textPrimary} />
          </Pressable>
        </View>

        <View style={styles.progressTrack}>
          <View style={[styles.progressFill, { width: `${progress * 100}%` }]} />
        </View>
      </BlurView>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  outer: {
    marginHorizontal: 12,
    marginBottom: 8,
    borderRadius: 18,
    overflow: 'hidden',
    borderWidth: 1,
    borderColor: LumeTheme.border,
  },
  blur: {
    backgroundColor: 'rgba(16, 20, 32, 0.78)',
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: 10,
    paddingVertical: 8,
    gap: 10,
  },
  art: {
    width: 44,
    height: 44,
    borderRadius: 10,
  },
  artFallback: {
    backgroundColor: LumeTheme.bgElevated,
    alignItems: 'center',
    justifyContent: 'center',
  },
  meta: {
    flex: 1,
  },
  titleRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 14,
    fontWeight: '700',
    flexShrink: 1,
  },
  offlineBadge: {
    backgroundColor: 'rgba(16, 185, 129, 0.18)',
    borderRadius: 6,
    paddingHorizontal: 5,
    paddingVertical: 1,
  },
  offlineBadgeText: {
    color: LumeTheme.emerald,
    fontSize: 9,
    fontWeight: '800',
  },
  artist: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    marginTop: 2,
  },
  iconBtn: {
    width: 34,
    height: 34,
    alignItems: 'center',
    justifyContent: 'center',
  },
  playBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: LumeTheme.accent,
    alignItems: 'center',
    justifyContent: 'center',
  },
  progressTrack: {
    height: 2.5,
    backgroundColor: 'rgba(255,255,255,0.08)',
  },
  progressFill: {
    height: 2.5,
    backgroundColor: LumeTheme.accentBright,
  },
});
