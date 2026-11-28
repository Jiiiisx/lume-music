import React from 'react';
import { ActivityIndicator, Image, Pressable, StyleSheet, Text, View } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { usePlayer } from '../context/PlayerContext';
import { LumeTheme } from '../theme';
import {
  formatDurationMs,
  type Track,
  trackArtistDisplay,
  trackSourceBadge,
} from '../types';

interface TrackRowProps {
  track: Track;
  rank?: number;
  isActive?: boolean;
  isOffline?: boolean;
  isDownloading?: boolean;
  onPress: () => void;
  onDownloadPress?: () => void;
  onDeleteOfflinePress?: () => void;
  onLikePress?: () => void;
  onMorePress?: () => void;
}

export const TrackRow = React.memo(function TrackRow({
  track,
  rank,
  isActive,
  isOffline,
  isDownloading,
  onPress,
  onDownloadPress,
  onDeleteOfflinePress,
  onLikePress,
  onMorePress,
}: TrackRowProps) {
  const { likedUids, openTrackMenu } = usePlayer();
  const badge = trackSourceBadge(track);
  const liked = likedUids.has(track.uid);
  const rankLabel = rank !== undefined ? String(rank).padStart(2, '0') : null;

  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [
        styles.row,
        isActive && styles.activeRow,
        pressed && !isActive && styles.pressedRow,
      ]}
    >
      {rankLabel !== null && (
        <Text style={[styles.rank, isActive && styles.rankActive]}>{rankLabel}</Text>
      )}

      <View style={styles.artWrap}>
        {track.artwork_ref ? (
          <Image source={{ uri: track.artwork_ref }} style={styles.art} />
        ) : (
          <View style={[styles.art, styles.artFallback]}>
            <Ionicons name="musical-note" size={18} color="rgba(255,255,255,0.75)" />
          </View>
        )}
        {isActive && (
          <View style={styles.playingOverlay}>
            <Ionicons name="musical-notes" size={16} color="#FFFFFF" />
          </View>
        )}
      </View>

      <View style={styles.info}>
        <Text style={[styles.title, isActive && styles.titleActive]} numberOfLines={1}>
          {track.title}
        </Text>
        <View style={styles.subRow}>
          <View
            style={[
              styles.badge,
              badge === 'SP'
                ? styles.badgeSpotify
                : badge === 'YTM'
                  ? styles.badgeYtm
                  : styles.badgeLocal,
            ]}
          >
            <Text style={styles.badgeText}>{badge}</Text>
          </View>
          {isOffline && (
            <Ionicons name="checkmark-circle" size={13} color={LumeTheme.emerald} />
          )}
          <Text style={styles.artist} numberOfLines={1}>
            {trackArtistDisplay(track)}
          </Text>
          {track.duration_ms > 0 && (
            <Text style={styles.duration}>
              {formatDurationMs(track.duration_ms)}
            </Text>
          )}
        </View>
      </View>

      <View style={styles.actions}>
        {onLikePress && (
          <Pressable
            onPress={(e) => {
              e.stopPropagation();
              onLikePress();
            }}
            hitSlop={6}
            style={styles.actionBtn}
          >
            <Ionicons
              name={liked ? 'heart' : 'heart-outline'}
              size={17}
              color={liked ? LumeTheme.rose : 'rgba(255,255,255,0.58)'}
            />
          </Pressable>
        )}

        {onDeleteOfflinePress && isOffline ? (
          <Pressable
            onPress={(e) => {
              e.stopPropagation();
              onDeleteOfflinePress();
            }}
            hitSlop={6}
            style={styles.actionBtn}
          >
            <Ionicons name="trash-outline" size={17} color={LumeTheme.rose} />
          </Pressable>
        ) : (
          onDownloadPress &&
          (!isOffline || isDownloading) && (
            <Pressable
              onPress={(e) => {
                e.stopPropagation();
                onDownloadPress();
              }}
              hitSlop={6}
              style={styles.actionBtn}
            >
              {isDownloading ? (
                <ActivityIndicator size="small" color="#FFFFFF" />
              ) : (
                <Ionicons
                  name="cloud-download-outline"
                  size={17}
                  color="rgba(255,255,255,0.58)"
                />
              )}
            </Pressable>
          )
        )}

        <Pressable
          onPress={(e) => {
            e.stopPropagation();
            if (onMorePress) {
              onMorePress();
            } else {
              openTrackMenu(track);
            }
          }}
          hitSlop={6}
          style={styles.actionBtn}
          accessibilityLabel="More options"
        >
          <Ionicons name="ellipsis-horizontal" size={17} color="rgba(255,255,255,0.58)" />
        </Pressable>
      </View>
    </Pressable>
  );
});

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 9,
    paddingHorizontal: 10,
    borderRadius: 14,
    gap: 10,
    borderWidth: 1,
    borderColor: 'transparent',
  },
  activeRow: {
    backgroundColor: 'rgba(255, 255, 255, 0.12)',
    borderColor: 'rgba(255, 255, 255, 0.22)',
  },
  pressedRow: {
    backgroundColor: 'rgba(255, 255, 255, 0.06)',
  },
  rank: {
    width: 22,
    color: LumeTheme.textMuted,
    fontSize: 12,
    fontWeight: '700',
    fontVariant: ['tabular-nums'],
    textAlign: 'center',
  },
  rankActive: {
    color: '#FFFFFF',
  },
  artWrap: {
    position: 'relative',
  },
  art: {
    width: 46,
    height: 46,
    borderRadius: 10,
  },
  artFallback: {
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  playingOverlay: {
    position: 'absolute',
    top: 0,
    right: 0,
    bottom: 0,
    left: 0,
    borderRadius: 10,
    backgroundColor: 'rgba(8, 10, 16, 0.56)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  info: {
    flex: 1,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 14.5,
    fontWeight: '600',
    letterSpacing: -0.1,
  },
  titleActive: {
    color: '#FFFFFF',
    fontWeight: '700',
  },
  subRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    marginTop: 3.5,
  },
  badge: {
    paddingHorizontal: 5,
    paddingVertical: 1,
    borderRadius: 5,
  },
  badgeSpotify: {
    backgroundColor: 'rgba(29, 185, 84, 0.22)',
  },
  badgeYtm: {
    backgroundColor: 'rgba(255, 255, 255, 0.13)',
  },
  badgeLocal: {
    backgroundColor: 'rgba(148, 163, 184, 0.22)',
  },
  badgeText: {
    color: '#FFFFFF',
    fontSize: 8.5,
    fontWeight: '800',
    letterSpacing: 0.3,
  },
  artist: {
    color: LumeTheme.textSecondary,
    fontSize: 12,
    flex: 1,
  },
  duration: {
    color: LumeTheme.textMuted,
    fontSize: 11,
    fontVariant: ['tabular-nums'],
  },
  actions: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  actionBtn: {
    width: 28,
    height: 34,
    alignItems: 'center',
    justifyContent: 'center',
  },
});
