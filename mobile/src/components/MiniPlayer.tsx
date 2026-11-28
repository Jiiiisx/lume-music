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
      style={({ pressed }) => [styles.outer, pressed && styles.outerPressed]}
      accessibilityRole="button"
      accessibilityLabel={`Now playing ${currentTrack.title}`}
    >
      <BlurView intensity={70} tint="dark" style={styles.blur}>
        <View style={styles.glassTint}>
          <View style={styles.row}>
            {currentTrack.artwork_ref ? (
              <Image source={{ uri: currentTrack.artwork_ref }} style={styles.art} />
            ) : (
              <View style={[styles.art, styles.artFallback]}>
                <Ionicons name="musical-notes" size={19} color="#FFFFFF" />
              </View>
            )}

            <View style={styles.meta}>
              <Text style={styles.title} numberOfLines={1}>
                {currentTrack.title}
              </Text>
              <View style={styles.artistRow}>
                {currentStream?.is_offline && (
                  <Ionicons name="checkmark-circle" size={12} color={LumeTheme.emerald} />
                )}
                <Text style={styles.artist} numberOfLines={1}>
                  {trackArtistDisplay(currentTrack)}
                </Text>
              </View>
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
              <Ionicons name="car-sport-outline" size={19} color="rgba(255,255,255,0.82)" />
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
                <ActivityIndicator size="small" color={LumeTheme.inkDark} />
              ) : (
                <Ionicons
                  name={isPlaying ? 'pause' : 'play'}
                  size={18}
                  color={LumeTheme.inkDark}
                  style={!isPlaying ? { marginLeft: 1.5 } : undefined}
                />
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
              <Ionicons name="play-skip-forward" size={19} color="#FFFFFF" />
            </Pressable>
          </View>

          <View style={styles.progressTrack}>
            <View style={[styles.progressFill, { width: `${progress * 100}%` }]} />
          </View>
        </View>
      </BlurView>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  outer: {
    marginHorizontal: 12,
    marginBottom: 8,
    borderRadius: 20,
    overflow: 'hidden',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.20)',
    shadowColor: '#000000',
    shadowOpacity: 0.42,
    shadowRadius: 16,
    shadowOffset: { width: 0, height: 8 },
  },
  outerPressed: {
    transform: [{ scale: 0.99 }],
  },
  blur: {
    overflow: 'hidden',
  },
  glassTint: {
    backgroundColor: 'rgba(18, 22, 34, 0.62)',
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
    borderRadius: 11,
  },
  artFallback: {
    backgroundColor: 'rgba(255,255,255,0.12)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  meta: {
    flex: 1,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 14,
    fontWeight: '700',
    letterSpacing: -0.1,
  },
  artistRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    marginTop: 2,
  },
  artist: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    flexShrink: 1,
  },
  iconBtn: {
    width: 32,
    height: 34,
    alignItems: 'center',
    justifyContent: 'center',
  },
  playBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: '#FFFFFF',
    alignItems: 'center',
    justifyContent: 'center',
  },
  progressTrack: {
    height: 2.5,
    backgroundColor: 'rgba(255, 255, 255, 0.12)',
  },
  progressFill: {
    height: 2.5,
    backgroundColor: 'rgba(255, 255, 255, 0.90)',
  },
});
