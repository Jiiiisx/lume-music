import React from 'react';
import { ActivityIndicator, Image, Pressable, StyleSheet, Text, View } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
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
}: TrackRowProps) {
  const badge = trackSourceBadge(track);

  return (
    <Pressable
      onPress={onPress}
      style={({ pressed }) => [
        styles.row,
        isActive && styles.activeRow,
        pressed && styles.pressedRow,
      ]}
    >
      {track.artwork_ref ? (
        <Image source={{ uri: track.artwork_ref }} style={styles.art} />
      ) : (
        <View style={[styles.art, styles.artFallback]}>
          <Ionicons name="musical-note" size={20} color={LumeTheme.accentBright} />
        </View>
      )}

      <View style={styles.info}>
        <Text
          style={[styles.title, isActive && styles.activeTitle]}
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
            <Ionicons
              name="checkmark-circle"
              size={13}
              color={LumeTheme.emerald}
            />
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
            hitSlop={8}
            style={styles.actionBtn}
          >
            <Ionicons
              name={track.is_liked ? 'heart' : 'heart-outline'}
              size={19}
              color={track.is_liked ? LumeTheme.rose : LumeTheme.textSecondary}
            />
          </Pressable>
        )}

        {onDeleteOfflinePress && isOffline ? (
          <Pressable
            onPress={(e) => {
              e.stopPropagation();
              onDeleteOfflinePress();
            }}
            hitSlop={8}
            style={styles.actionBtn}
          >
            <Ionicons name="trash-outline" size={19} color={LumeTheme.rose} />
          </Pressable>
        ) : (
          onDownloadPress && (
            <Pressable
              onPress={(e) => {
                e.stopPropagation();
                onDownloadPress();
              }}
              hitSlop={8}
              style={styles.actionBtn}
            >
              {isDownloading ? (
                <ActivityIndicator size="small" color={LumeTheme.accentBright} />
              ) : (
                <Ionicons
                  name={isOffline ? 'cloud-done' : 'cloud-download-outline'}
                  size={19}
                  color={isOffline ? LumeTheme.emerald : LumeTheme.textSecondary}
                />
              )}
            </Pressable>
          )
        )}
      </View>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: 10,
    paddingHorizontal: 14,
    borderRadius: 14,
    gap: 12,
  },
  activeRow: {
    backgroundColor: 'rgba(99, 102, 241, 0.16)',
    borderWidth: 1,
    borderColor: LumeTheme.borderAccent,
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
  activeTitle: {
    color: LumeTheme.accentBright,
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
    backgroundColor: 'rgba(99, 102, 241, 0.25)',
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
    gap: 4,
  },
  actionBtn: {
    width: 34,
    height: 34,
    alignItems: 'center',
    justifyContent: 'center',
  },
});
