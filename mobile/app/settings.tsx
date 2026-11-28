import React, { useEffect, useState } from 'react';
import {
  ActivityIndicator,
  Alert,
  Pressable,
  ScrollView,
  StyleSheet,
  Switch,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { useRouter } from 'expo-router';
import { LumeCore } from '../modules/lume-core';
import { AmbientBackdrop } from '../src/components/AmbientBackdrop';
import { usePlayer } from '../src/context/PlayerContext';
import {
  clearSpotifySession,
  connectSpotifyMobile,
  loadSpotifyClientId,
  loadSpotifyTokenSet,
  syncSpotifyNow,
} from '../src/services/spotifyAuth';
import { LumeTheme } from '../src/theme';
import { formatBytes, type HealthCheckDto } from '../src/types';

export default function SettingsModal() {
  const router = useRouter();
  const {
    dynamicThemeEnabled,
    setDynamicThemeEnabled,
    offlineEntries,
    offlineBytes,
    clearAllOffline,
    refreshLibraryState,
  } = usePlayer();

  const [clientId, setClientId] = useState('');
  const [spotifyConnected, setSpotifyConnected] = useState(false);
  const [health, setHealth] = useState<HealthCheckDto | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    void (async () => {
      const [cid, token] = await Promise.all([
        loadSpotifyClientId(),
        loadSpotifyTokenSet(),
      ]);
      setClientId(cid);
      setSpotifyConnected(token !== null);
      if (LumeCore.isAvailable()) {
        try {
          setHealth(await LumeCore.healthCheck());
        } catch {
          // Ignore.
        }
      }
    })();
  }, []);

  const handleConnectSpotify = async () => {
    setBusy('Menghubungkan ke Spotify...');
    try {
      await connectSpotifyMobile(clientId);
      setSpotifyConnected(true);
      const summary = await syncSpotifyNow();
      await refreshLibraryState();
      Alert.alert(
        'Spotify Terhubung',
        `Halo ${summary.display_name}! ${summary.liked_count} Liked Songs & ${summary.playlist_count} playlist disinkronkan.`
      );
    } catch (err) {
      Alert.alert('Spotify', err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const handleSyncSpotify = async () => {
    setBusy('Menyinkronkan Spotify...');
    try {
      const summary = await syncSpotifyNow();
      await refreshLibraryState();
      Alert.alert(
        'Sync Selesai',
        `${summary.liked_count} Liked Songs & ${summary.playlist_count} playlist diperbarui.`
      );
    } catch (err) {
      Alert.alert('Sync Gagal', err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const handleClearOffline = () => {
    Alert.alert(
      'Hapus Semua Cache Offline?',
      `Ini akan menghapus ${offlineEntries.length} file lagu (${formatBytes(offlineBytes)}) dari penyimpanan iPhone.`,
      [
        { text: 'Batal', style: 'cancel' },
        {
          text: 'Hapus Semua',
          style: 'destructive',
          onPress: () => void clearAllOffline(),
        },
      ]
    );
  };

  return (
    <AmbientBackdrop>
      <SafeAreaView style={styles.root}>
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} style={styles.closeBtn} hitSlop={10}>
            <Ionicons name="chevron-down" size={22} color="#FFFFFF" />
          </Pressable>
          <Text style={styles.title}>Settings</Text>
          <View style={{ width: 36 }} />
        </View>

        <ScrollView contentContainerStyle={styles.content} showsVerticalScrollIndicator={false}>
          {busy && (
            <View style={styles.busyRow}>
              <ActivityIndicator size="small" color="#FFFFFF" />
              <Text style={styles.busyText}>{busy}</Text>
            </View>
          )}

          <View style={styles.panel}>
            <Text style={styles.panelTitle}>Tampilan (Liquid Glass)</Text>
            <View style={styles.switchRow}>
              <View style={{ flex: 1 }}>
                <Text style={styles.rowTitle}>Dynamic Album Backdrop</Text>
                <Text style={styles.rowDesc}>
                  Latar belakang blur & warna dinamis mengikuti cover album lagu yang sedang diputar
                  seperti versi Desktop.
                </Text>
              </View>
              <Switch
                value={dynamicThemeEnabled}
                onValueChange={(v) => void setDynamicThemeEnabled(v)}
                trackColor={{ false: 'rgba(255,255,255,0.16)', true: LumeTheme.emerald }}
                thumbColor="#FFFFFF"
              />
            </View>
          </View>

          <View style={styles.panel}>
            <Text style={styles.panelTitle}>Spotify Sync</Text>
            <Text style={styles.rowDesc}>
              Tambahkan Redirect URI{' '}
              <Text style={styles.mono}>lumemusic://callback</Text>{' '}
              di Spotify Developer Dashboard, lalu masukkan Client ID kamu.
            </Text>
            <TextInput
              value={clientId}
              onChangeText={setClientId}
              placeholder="Spotify Client ID"
              placeholderTextColor={LumeTheme.textMuted}
              autoCapitalize="none"
              autoCorrect={false}
              style={styles.input}
            />
            <View style={styles.btnRow}>
              <Pressable
                onPress={() => void handleConnectSpotify()}
                style={[styles.btn, { flex: 1, backgroundColor: LumeTheme.spotify }]}
              >
                <Text style={styles.btnTextWhite}>
                  {spotifyConnected ? 'Reconnect' : 'Connect Spotify'}
                </Text>
              </Pressable>
              {spotifyConnected && (
                <>
                  <Pressable
                    onPress={() => void handleSyncSpotify()}
                    style={[styles.btn, { flex: 1, backgroundColor: '#FFFFFF' }]}
                  >
                    <Text style={styles.btnTextDark}>Sync Now</Text>
                  </Pressable>
                  <Pressable
                    onPress={async () => {
                      await clearSpotifySession();
                      setSpotifyConnected(false);
                    }}
                    style={styles.dangerIconBtn}
                  >
                    <Ionicons name="log-out-outline" size={18} color={LumeTheme.rose} />
                  </Pressable>
                </>
              )}
            </View>
          </View>

          <View style={styles.panel}>
            <Text style={styles.panelTitle}>Penyimpanan Offline</Text>
            <Text style={styles.rowDesc}>
              {offlineEntries.length} lagu tersimpan • {formatBytes(offlineBytes)} terpakai di iPhone
            </Text>
            {offlineEntries.length > 0 && (
              <Pressable onPress={handleClearOffline} style={styles.clearBtn}>
                <Ionicons name="trash-outline" size={17} color={LumeTheme.rose} />
                <Text style={styles.clearBtnText}>Hapus Semua File Offline</Text>
              </Pressable>
            )}
          </View>

          <View style={styles.panel}>
            <Text style={styles.panelTitle}>Diagnostik Engine</Text>
            <Text style={styles.rowDesc}>
              {health
                ? `Lume Rust Core v${health.version} (${health.target_os})\nAudio Engine: Native iOS AVPlayer + MPRemoteCommandCenter`
                : 'Native Rust Core tidak aktif.'}
            </Text>
          </View>
        </ScrollView>
      </SafeAreaView>
    </AmbientBackdrop>
  );
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: 16,
    paddingVertical: 10,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: 'rgba(255, 255, 255, 0.14)',
  },
  closeBtn: {
    width: 36,
    height: 36,
    borderRadius: 18,
    backgroundColor: 'rgba(255, 255, 255, 0.10)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.15)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    color: '#FFFFFF',
    fontSize: 18,
    fontWeight: '800',
  },
  content: {
    padding: 16,
    gap: 14,
    paddingBottom: 48,
  },
  busyRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    padding: 12,
    borderRadius: 14,
    backgroundColor: 'rgba(255, 255, 255, 0.09)',
  },
  busyText: {
    color: '#FFFFFF',
    fontSize: 13,
  },
  panel: {
    padding: 16,
    borderRadius: 20,
    backgroundColor: 'rgba(255, 255, 255, 0.08)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.14)',
    gap: 10,
  },
  panelTitle: {
    color: '#FFFFFF',
    fontSize: 15.5,
    fontWeight: '800',
  },
  switchRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
  },
  rowTitle: {
    color: '#FFFFFF',
    fontSize: 14.5,
    fontWeight: '600',
  },
  rowDesc: {
    color: LumeTheme.textSecondary,
    fontSize: 12.5,
    lineHeight: 18,
    marginTop: 2,
  },
  mono: {
    color: '#FFFFFF',
    fontWeight: '700',
  },
  input: {
    backgroundColor: 'rgba(0, 0, 0, 0.36)',
    borderWidth: 1,
    borderColor: 'rgba(255, 255, 255, 0.16)',
    borderRadius: 14,
    paddingHorizontal: 13,
    height: 44,
    color: '#FFFFFF',
    fontSize: 14,
  },
  btnRow: {
    flexDirection: 'row',
    gap: 8,
  },
  btn: {
    height: 42,
    borderRadius: 999,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 14,
  },
  btnTextWhite: {
    color: '#FFFFFF',
    fontWeight: '800',
    fontSize: 13.5,
  },
  btnTextDark: {
    color: LumeTheme.inkDark,
    fontWeight: '800',
    fontSize: 13.5,
  },
  dangerIconBtn: {
    width: 42,
    height: 42,
    borderRadius: 21,
    backgroundColor: 'rgba(244, 63, 94, 0.18)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.34)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  clearBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 8,
    backgroundColor: 'rgba(244, 63, 94, 0.18)',
    borderWidth: 1,
    borderColor: 'rgba(244, 63, 94, 0.34)',
    borderRadius: 999,
    height: 42,
  },
  clearBtnText: {
    color: '#FECDD3',
    fontWeight: '700',
    fontSize: 13.5,
  },
});
