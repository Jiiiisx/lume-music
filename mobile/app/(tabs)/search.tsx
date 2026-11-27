import React, { useState } from 'react';
import {
  ActivityIndicator,
  FlatList,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Ionicons } from '@expo/vector-icons';
import { LumeCore } from '../../modules/lume-core';
import { TrackRow } from '../../src/components/TrackRow';
import { usePlayer } from '../../src/context/PlayerContext';
import { LumeTheme } from '../../src/theme';
import type { Track } from '../../src/types';

export default function SearchScreen() {
  const {
    currentTrack,
    offlineUids,
    downloadingUids,
    playTrack,
    downloadOffline,
    toggleLikeTrack,
  } = usePlayer();

  const [query, setQuery] = useState('');
  const [results, setResults] = useState<Track[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const runSearch = async () => {
    const trimmed = query.trim();
    if (!trimmed) return;
    setSearching(true);
    setError(null);
    try {
      const catalog = await LumeCore.searchYtm(trimmed, 25);
      setResults(catalog.tracks);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSearching(false);
    }
  };

  return (
    <SafeAreaView style={styles.container} edges={['top']}>
      <View style={styles.header}>
        <Text style={styles.title}>Search</Text>
        <View style={styles.searchBar}>
          <Ionicons name="search" size={18} color={LumeTheme.textSecondary} />
          <TextInput
            value={query}
            onChangeText={setQuery}
            onSubmitEditing={() => void runSearch()}
            placeholder="Cari lagu atau artis di YouTube Music..."
            placeholderTextColor={LumeTheme.textMuted}
            returnKeyType="search"
            autoCorrect={false}
            style={styles.input}
          />
          {query.length > 0 && (
            <Pressable onPress={() => setQuery('')} hitSlop={8}>
              <Ionicons name="close-circle" size={18} color={LumeTheme.textMuted} />
            </Pressable>
          )}
        </View>
      </View>

      {error && <Text style={styles.error}>{error}</Text>}

      {searching ? (
        <ActivityIndicator
          size="large"
          color={LumeTheme.accentBright}
          style={{ marginTop: 40 }}
        />
      ) : (
        <FlatList
          data={results}
          keyExtractor={(item) => item.uid}
          contentContainerStyle={styles.listContent}
          ListEmptyComponent={
            <View style={styles.empty}>
              <Ionicons
                name="musical-notes-outline"
                size={38}
                color={LumeTheme.textMuted}
              />
              <Text style={styles.emptyText}>
                Ketik judul lagu lalu tekan Search. Ketuk ikon awan untuk
                mengunduh lagu ke penyimpanan offline berkendara.
              </Text>
            </View>
          }
          renderItem={({ item }) => (
            <TrackRow
              track={item}
              isActive={currentTrack?.uid === item.uid}
              isOffline={offlineUids.has(item.uid)}
              isDownloading={downloadingUids.has(item.uid)}
              onPress={() => void playTrack(item, results)}
              onDownloadPress={() => void downloadOffline(item)}
              onLikePress={() => void toggleLikeTrack(item)}
            />
          )}
        />
      )}
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: LumeTheme.bg,
  },
  header: {
    paddingHorizontal: 16,
    paddingTop: 8,
    paddingBottom: 12,
    gap: 12,
  },
  title: {
    color: LumeTheme.textPrimary,
    fontSize: 28,
    fontWeight: '800',
  },
  searchBar: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: LumeTheme.bgElevated,
    borderWidth: 1,
    borderColor: LumeTheme.border,
    borderRadius: 14,
    paddingHorizontal: 14,
    height: 46,
    gap: 10,
  },
  input: {
    flex: 1,
    color: LumeTheme.textPrimary,
    fontSize: 15,
  },
  error: {
    color: LumeTheme.rose,
    paddingHorizontal: 16,
    marginBottom: 8,
    fontSize: 13,
  },
  listContent: {
    paddingHorizontal: 10,
    paddingBottom: 160,
  },
  empty: {
    alignItems: 'center',
    paddingHorizontal: 32,
    paddingTop: 64,
    gap: 12,
  },
  emptyText: {
    color: LumeTheme.textSecondary,
    fontSize: 14,
    textAlign: 'center',
    lineHeight: 20,
  },
});
