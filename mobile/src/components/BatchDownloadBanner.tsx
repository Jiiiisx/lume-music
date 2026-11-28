import React from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';
import { Ionicons } from '@expo/vector-icons';
import { usePlayer } from '../context/PlayerContext';
import { LumeTheme } from '../theme';

export function BatchDownloadBanner() {
  const { batchDownload, cancelBatchDownload } = usePlayer();
  if (!batchDownload) return null;

  const doneCount = batchDownload.completed + batchDownload.failed;
  const pct =
    batchDownload.total > 0
      ? Math.min(1, Math.max(0, doneCount / batchDownload.total))
      : 0;

  return (
    <View style={styles.card}>
      <View style={styles.topRow}>
        {batchDownload.active ? (
          <ActivityIndicator size="small" color="#FFFFFF" />
        ) : (
          <Ionicons name="checkmark-circle" size={18} color={LumeTheme.emerald} />
        )}
        <View style={{ flex: 1 }}>
          <Text style={styles.title} numberOfLines={1}>
            {batchDownload.playlistName} • {batchDownload.completed}/{batchDownload.total}
            {batchDownload.failed > 0 ? ` (${batchDownload.failed} gagal)` : ''}
          </Text>
          <Text style={styles.sub} numberOfLines={1}>
            {batchDownload.currentTitle}
            {batchDownload.statusNote ? ` — ${batchDownload.statusNote}` : ''}
          </Text>
        </View>
        {batchDownload.active && (
          <Pressable onPress={cancelBatchDownload} style={styles.cancelBtn}>
            <Text style={styles.cancelText}>Batal</Text>
          </Pressable>
        )}
      </View>

      <View style={styles.barTrack}>
        <View
          style={[
            styles.barFill,
            {
              width: `${pct * 100}%`,
              backgroundColor: batchDownload.active ? '#FFFFFF' : LumeTheme.emerald,
            },
          ]}
        />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    marginHorizontal: 16,
    marginBottom: 12,
    padding: 13,
    borderRadius: 16,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    gap: 8,
  },
  topRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 10,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 13.5,
    fontWeight: '700',
  },
  sub: {
    color: LumeTheme.textSecondary,
    fontSize: 11.5,
    marginTop: 2,
  },
  cancelBtn: {
    backgroundColor: 'rgba(244, 63, 94, 0.22)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.36)',
    paddingHorizontal: 10,
    paddingVertical: 5,
    borderRadius: 999,
  },
  cancelText: {
    color: '#FECDD3',
    fontSize: 11.5,
    fontWeight: '700',
  },
  barTrack: {
    height: 3.5,
    borderRadius: 2,
    backgroundColor: 'rgba(255, 255, 255, 0.14)',
    overflow: 'hidden',
  },
  barFill: {
    height: 3.5,
    borderRadius: 2,
  },
});
