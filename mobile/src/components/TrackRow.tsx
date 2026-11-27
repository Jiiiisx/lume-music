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
  isActive?: boolean;
  isOffline?: boolean;
  isDownloading?: boolean;
  onPress: () => void;
  onDownloadPress?: () => void;
  onDeleteOfflinePress?: () => void;
  onLikePress?: () => void;
  onMorePress?: () => void;
}

export function TrackRow({
  track,
  isActive,
  isOffline,
  isDownloading,
  onPress,
  onDownloadPress,
  onDeleteOfflinePress,
  onLikePress,
  onMorePress,
}: TrackRowProps) {
  const { likedUids, palette, openTrackMenu } = usePlayer();
  const badge = trackSourceBadge(track);
  const liked = likedUids.has(track.uid);

  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [
        styles.row,
        isActive && {
          backgroundColor: palette.accent_soft,
          borderWidth: 1,
          borderColor: 'rgba(255,255,255,0.22)',
        },
        pressed && styles.pressedRow,
      ]}
    >
      {track.artwork_ref ? (
        <Image source={{ uri: track.artwork_ref }} style={styles.art} />
      ) : (
        <View style={[styles.art, styles.artFallback]}>
          <Ionicons name="musical-note" size={20} color={palette.accent} />
        </View>
      )}

      <View style={styles.info}>
        <Text
          style={[styles.title, isActive && { color: palette.accent }]}
          numberOfLines={1}
        >
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
              size={18}
              color={liked ? LumeTheme.rose : LumeTheme.textSecondary}
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
            <Ionicons name="trash-outline" size={18} color={LumeTheme.rose} />
          </Pressable>
        ) : (
          onDownloadPress && (
            <Pressable
              onPress={(e) => {
                e.stopPropagation();
                onDownloadPress();
              }}
              hitSlop={6}
              style={styles.actionBtn}
            >
              {isDownloading ? (
                <ActivityIndicator size="small" color={palette.accent} />
              ) : (
                <Ionicons
                  name={isOffline ? 'cloud-done' : 'cloud-download-outline'}
                  size={18}
                  color={isOffline ? LumeTheme.emerald : LumeTheme.textSecondary}
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
          <Ionicons name="ellipsis-horizontal" size={18} color={LumeTheme.textSecondary} />
        </Pressable>
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 10,
    paddingHorizontal: 12,
    borderRadius: 14,
    gap: 11,
  },
  pressedRow: {
    backgroundColor: LumeTheme.bgCard,
  },
  art: {
    width: 48,
    height: 48,
    borderRadius: 10,
  },
  artFallback: {
    backgroundColor: LumeTheme.bgElevated,
    alignItems: 'center',
    justifyContent: 'center',
  },
  info: {
    flex: 1,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 15,
    fontWeight: '600',
  },
  subRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    marginTop: 4,
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
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
  },
  badgeLocal: {
    backgroundColor: 'rgba(148, 163, 184, 0.22)',
  },
  badgeText: {
    color: LumeTheme.textPrimary,
    fontSize: 9,
    fontWeight: '800',
  },
  artist: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    flex: 1,
  },
  duration: {
    color: LumeTheme.textMuted,
    fontSize: 11.5,
  },
  actions: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  actionBtn: {
    width: 30,
    height: 34,
    alignItems: 'center',
    justifyContent: 'center',
  },
});
