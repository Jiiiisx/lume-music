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
    palette,
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
          <Pressable onPress={() => router.back()} hitSlop={10}>
            <Ionicons name="chevron-down" size={26} color="#FFFFFF" />
          </Pressable>
          <Text style={styles.title}>Settings</Text>
          <View style={{ width: 26 }} />
        </View>

        <ScrollView contentContainerStyle={styles.content}>
          {busy && (
            <View style={styles.busyRow}>
              <ActivityIndicator size="small" color={palette.accent} />
              <Text style={styles.busyText}>{busy}</Text>
            </View>
          )}

          <View style={styles.panel}>
            <Text style={styles.panelTitle}>Tampilan (Visual Glassmorphism)</Text>
            <View style={styles.switchRow}>
              <View style={{ flex: 1 }}>
                <Text style={styles.rowTitle}>Dynamic Album Color</Text>
                <Text style={styles.rowDesc}>
                  Warna latar belakang mengikuti warna cover album lagu yang sedang diputar
                  (seperti versi Desktop).
                </Text>
              </View>
              <Switch
                value={dynamicThemeEnabled}
                onValueChange={(v) => void setDynamicThemeEnabled(v)}
                trackColor={{ false: '#334155', true: palette.accent }}
              />
            </View>
          </View>

          <View style={styles.panel}>
            <Text style={styles.panelTitle}>Spotify Sync (Keychain + Smart YTM Fallback)</Text>
            <Text style={styles.rowDesc}>
              Tambahkan Redirect URI{' '}
              <Text style={[styles.mono, { color: palette.accent }]}>lumemusic://callback</Text>{' '}
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
                  {spotifyConnected ? 'Reconnect Spotify' : 'Connect Spotify'}
                </Text>
              </Pressable>
              {spotifyConnected && (
                <>
                  <Pressable
                    onPress={() => void handleSyncSpotify()}
                    style={[styles.btn, { flex: 1, backgroundColor: palette.accent }]}
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
            <Text style={styles.panelTitle}>Penyimpanan Offline Berkendara</Text>
            <Text style={styles.rowDesc}>
              {offlineEntries.length} lagu tersimpan • {formatBytes(offlineBytes)} terpakai
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
    paddingVertical: 12,
    borderBottomWidth: 1,
    borderBottomColor: LumeTheme.border,
  },
  title: {
    color: '#FFFFFF',
    fontSize: 18,
    fontWeight: '800',
  },
  content: {
    padding: 16,
    gap: 16,
    paddingBottom: 48,
  },
  busyRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  busyText: {
    color: '#FFFFFF',
    fontSize: 13,
  },
  panel: {
    padding: 15,
    borderRadius: 16,
    backgroundColor: LumeTheme.bgCard,
    borderWidth: 1,
    borderColor: LumeTheme.border,
    gap: 10,
  },
  panelTitle: {
    color: '#FFFFFF',
    fontSize: 15.5,
    fontWeight: '700',
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
    fontWeight: '700',
  },
  input: {
    backgroundColor: 'rgba(0,0,0,0.35)',
    borderWidth: 1,
    borderColor: LumeTheme.border,
    borderRadius: 12,
    paddingHorizontal: 12,
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
    borderRadius: 12,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: 14,
  },
  btnTextWhite: {
    color: '#FFFFFF',
    fontWeight: '700',
    fontSize: 13.5,
  },
  btnTextDark: {
    color: '#0A0C12',
    fontWeight: '800',
    fontSize: 13.5,
  },
  dangerIconBtn: {
    width: 42,
    height: 42,
    borderRadius: 12,
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    alignItems: 'center',
    justifyContent: 'center',
  },
  clearBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 8,
    backgroundColor: 'rgba(244, 63, 94, 0.16)',
    borderRadius: 12,
    height: 42,
  },
  clearBtnText: {
    color: LumeTheme.rose,
    fontWeight: '700',
    fontSize: 13.5,
  },
});
