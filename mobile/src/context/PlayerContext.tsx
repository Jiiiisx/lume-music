import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
} from 'react';
import { createAudioPlayer, setAudioModeAsync, type AudioPlayer } from 'expo-audio';
import * as Haptics from 'expo-haptics';
import * as SecureStore from 'expo-secure-store';
import { LumeCore } from '../../modules/lume-core';
import {
  DEFAULT_PALETTE,
  type DynamicPaletteDto,
  type LyricsData,
  type OfflineEntryDto,
  type PlayableStreamDto,
  type Playlist,
  type RepeatMode,
  type Track,
  trackArtistDisplay,
} from '../types';

const KEY_DYNAMIC_THEME = 'lume_dynamic_theme_enabled';

export interface PlaybackProgressSnapshot {
  positionMs: number;
  durationMs: number;
}

const progressStore = {
  snapshot: { positionMs: 0, durationMs: 0 } as PlaybackProgressSnapshot,
  listeners: new Set<() => void>(),
};

function publishProgress(positionMs: number, durationMs: number): void {
  const prev = progressStore.snapshot;
  if (prev.positionMs === positionMs && prev.durationMs === durationMs) {
    return;
  }
  progressStore.snapshot = { positionMs, durationMs };
  progressStore.listeners.forEach((listener) => listener());
}

function subscribeProgress(listener: () => void): () => void {
  progressStore.listeners.add(listener);
  return () => {
    progressStore.listeners.delete(listener);
  };
}

function getProgressSnapshot(): PlaybackProgressSnapshot {
  return progressStore.snapshot;
}

/**
 * Isolated 20Hz playback-progress hook backed by `useSyncExternalStore`.
 * Only progress bars & synced lyrics subscribe to this — keeping every other
 * screen (Home, Library, Explore, Offline, TrackRow) at 0 renders/sec during playback.
 */
export function usePlaybackProgress(): PlaybackProgressSnapshot {
  return useSyncExternalStore(subscribeProgress, getProgressSnapshot, getProgressSnapshot);
}

export interface BatchDownloadState {
  playlistName: string;
  total: number;
  completed: number;
  skipped: number;
  failed: number;
  currentTitle: string;
  active: boolean;
  statusNote: string | null;
}

function sanitizeErrorMessage(raw: string): string {
  const cleaned = raw
    .replace(/https?:\/\/\S+/g, '[stream-url]')
    .replace(/^LumeCore\.LumeFfiError\.\w+\(message:\s*"?/, '')
    .replace(/"?\)$/, '')
    .trim();
  return cleaned.length > 180 ? cleaned.slice(0, 177) + '...' : cleaned;
}

function sleepMs(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function fisherYatesShuffle<T>(items: T[]): T[] {
  const arr = [...items];
  for (let i = arr.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [arr[i], arr[j]] = [arr[j], arr[i]];
  }
  return arr;
}

interface PlayerContextValue {
  currentTrack: Track | null;
  currentStream: PlayableStreamDto | null;
  queue: Track[];
  queueIndex: number;
  isPlaying: boolean;
  isBuffering: boolean;
  shuffle: boolean;
  repeat: RepeatMode;
  lyrics: LyricsData | null;
  lyricsLoading: boolean;
  likedTracks: Track[];
  likedUids: Set<string>;
  playlists: Playlist[];
  offlineUids: Set<string>;
  downloadingUids: Set<string>;
  offlineEntries: OfflineEntryDto[];
  offlineBytes: number;
  batchDownload: BatchDownloadState | null;
  palette: DynamicPaletteDto;
  dynamicThemeEnabled: boolean;
  menuTrack: Track | null;
  errorMessage: string | null;
  playTrack: (
    track: Track,
    newQueue?: Track[],
    forceRematch?: boolean,
    overrideShuffle?: boolean
  ) => Promise<void>;
  playNext: (track: Track) => void;
  addToQueue: (track: Track) => void;
  removeFromQueue: (index: number) => void;
  reorderQueue: (fromIndex: number, toIndex: number) => void;
  jumpToQueueIndex: (index: number) => Promise<void>;
  clearUpcomingQueue: () => void;
  togglePlayPause: () => void;
  nextTrack: () => Promise<void>;
  previousTrack: () => Promise<void>;
  seekToMs: (ms: number) => Promise<void>;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
  toggleLikeCurrent: () => Promise<void>;
  toggleLikeTrack: (track: Track) => Promise<boolean>;
  refreshLibraryState: () => Promise<void>;
  downloadOffline: (track: Track) => Promise<void>;
  downloadPlaylistOffline: (playlistName: string, tracks: Track[]) => Promise<void>;
  cancelBatchDownload: () => void;
  removeOffline: (trackUid: string) => Promise<void>;
  clearAllOffline: () => Promise<void>;
  refreshOfflineCache: () => Promise<void>;
  refreshLyrics: () => Promise<void>;
  setDynamicThemeEnabled: (enabled: boolean) => Promise<void>;
  openTrackMenu: (track: Track) => void;
  closeTrackMenu: () => void;
  clearError: () => void;
}

const PlayerContext = createContext<PlayerContextValue | null>(null);

export function PlayerProvider({ children }: { children: React.ReactNode }) {
  const playerRef = useRef<AudioPlayer | null>(null);
  const loadGenRef = useRef(0);
  const finishedHandledForGenRef = useRef(-1);

  const [queue, setQueue] = useState<Track[]>([]);
  const [queueIndex, setQueueIndex] = useState(-1);
  const [currentTrack, setCurrentTrack] = useState<Track | null>(null);
  const [currentStream, setCurrentStream] = useState<PlayableStreamDto | null>(null);
  const [isPlaying, setIsPlaying] = useState(false);
  const [isBuffering, setIsBuffering] = useState(false);
  const [shuffle, setShuffle] = useState(false);
  const [repeat, setRepeat] = useState<RepeatMode>('all');
  const [lyrics, setLyrics] = useState<LyricsData | null>(null);
  const [lyricsLoading, setLyricsLoading] = useState(false);
  const [likedTracks, setLikedTracks] = useState<Track[]>([]);
  const [playlists, setPlaylists] = useState<Playlist[]>([]);
  const [offlineEntries, setOfflineEntries] = useState<OfflineEntryDto[]>([]);
  const [offlineBytes, setOfflineBytes] = useState(0);
  const [downloadingUids, setDownloadingUids] = useState<Set<string>>(new Set());
  const [batchDownload, setBatchDownload] = useState<BatchDownloadState | null>(null);
  const batchCancelRef = useRef(false);
  const batchRunningRef = useRef(false);
  const [palette, setPalette] = useState<DynamicPaletteDto>(DEFAULT_PALETTE);
  const [dynamicThemeEnabled, setDynamicThemeEnabledState] = useState(true);
  const [menuTrack, setMenuTrack] = useState<Track | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const queueRef = useRef(queue);
  const originalQueueRef = useRef<Track[]>([]);
  const queueIndexRef = useRef(queueIndex);
  const shuffleRef = useRef(shuffle);
  const repeatRef = useRef(repeat);
  const isPlayingRef = useRef(isPlaying);
  const isBufferingRef = useRef(isBuffering);
  const currentTrackRef = useRef(currentTrack);
  const effectiveDurationRef = useRef(0);
  const dynamicThemeEnabledRef = useRef(dynamicThemeEnabled);

  queueRef.current = queue;
  queueIndexRef.current = queueIndex;
  shuffleRef.current = shuffle;
  repeatRef.current = repeat;
  isPlayingRef.current = isPlaying;
  isBufferingRef.current = isBuffering;
  currentTrackRef.current = currentTrack;
  dynamicThemeEnabledRef.current = dynamicThemeEnabled;

  const likedUids = useMemo(
    () => new Set(likedTracks.map((t) => t.uid)),
    [likedTracks]
  );

  const offlineUids = useMemo(
    () => new Set(offlineEntries.map((item) => item.track.uid)),
    [offlineEntries]
  );

  const refreshOfflineCache = useCallback(async () => {
    if (!LumeCore.isAvailable()) return;
    try {
      const [entries, bytes] = await Promise.all([
        LumeCore.listOfflineTracks(),
        LumeCore.offlineStorageBytes(),
      ]);
      setOfflineEntries(entries);
      setOfflineBytes(bytes);
    } catch {
      // Non-fatal on cold start.
    }
  }, []);

  const refreshLibraryState = useCallback(async () => {
    if (!LumeCore.isAvailable()) return;
    try {
      const [liked, pls] = await Promise.all([
        LumeCore.listLikedTracks(),
        LumeCore.listPlaylists(),
      ]);
      setLikedTracks(liked);
      setPlaylists(pls);
    } catch {
      // Non-fatal on cold start.
    }
  }, []);

  const updateDynamicPalette = useCallback(async (artworkRef: string | null) => {
    if (!dynamicThemeEnabledRef.current || !artworkRef || !LumeCore.isAvailable()) {
      setPalette(DEFAULT_PALETTE);
      return;
    }
    try {
      const extracted = await LumeCore.extractPalette(artworkRef);
      setPalette(extracted);
    } catch {
      setPalette(DEFAULT_PALETTE);
    }
  }, []);

  const setDynamicThemeEnabled = useCallback(
    async (enabled: boolean) => {
      setDynamicThemeEnabledState(enabled);
      dynamicThemeEnabledRef.current = enabled;
      await SecureStore.setItemAsync(KEY_DYNAMIC_THEME, enabled ? '1' : '0').catch(() => {});
      if (!enabled) {
        setPalette(DEFAULT_PALETTE);
      } else if (currentTrackRef.current?.artwork_ref) {
        void updateDynamicPalette(currentTrackRef.current.artwork_ref);
      }
    },
    [updateDynamicPalette]
  );

  const loadTrackInternal = useCallback(
    async (
      track: Track,
      newQueue?: Track[],
      explicitIndex?: number,
      forceRematch = false
    ) => {
      const gen = ++loadGenRef.current;
      finishedHandledForGenRef.current = -1;
      setErrorMessage(null);
      setCurrentTrack(track);
      currentTrackRef.current = track;
      setIsBuffering(true);
      isBufferingRef.current = true;
      effectiveDurationRef.current = track.duration_ms;
      publishProgress(0, track.duration_ms);
      setLyrics(null);

      if (newQueue) {
        queueRef.current = newQueue;
        setQueue(newQueue);
        const idx =
          explicitIndex ?? Math.max(0, newQueue.findIndex((t) => t.uid === track.uid));
        queueIndexRef.current = idx;
        setQueueIndex(idx);
      } else if (explicitIndex !== undefined) {
        queueIndexRef.current = explicitIndex;
        setQueueIndex(explicitIndex);
      }

      void updateDynamicPalette(track.artwork_ref);

      // Non-blocking Spotify oEmbed artwork fetch for CSV/URL imported tracks missing album art.
      if (
        LumeCore.isAvailable() &&
        'Spotify' in track.source &&
        (!track.artwork_ref || !track.artwork_ref.includes('ab67616d'))
      ) {
        LumeCore.enrichSpotifyArtwork([track], 1)
          .then((map) => {
            const cover = map[track.uid];
            if (!cover || gen !== loadGenRef.current) return;
            const enriched: Track = { ...track, artwork_ref: cover };
            setCurrentTrack((prev) =>
              prev && prev.uid === track.uid ? { ...prev, artwork_ref: cover } : prev
            );
            currentTrackRef.current = enriched;
            setQueue((prev) =>
              prev.map((q) => (q.uid === track.uid ? { ...q, artwork_ref: cover } : q))
            );
            void updateDynamicPalette(cover);
            const activePlayer = playerRef.current;
            if (activePlayer) {
              activePlayer.setActiveForLockScreen(
                true,
                {
                  title: enriched.title,
                  artist: trackArtistDisplay(enriched),
                  albumTitle: enriched.album ?? 'Lume Music',
                  artworkUrl: cover,
                },
                {
                  showNextTrack: true,
                  showPreviousTrack: true,
                  showSeekForward: false,
                  showSeekBackward: false,
                }
              );
            }
          })
          .catch(() => {});
      }

      if (!LumeCore.isAvailable()) {
        setIsBuffering(false);
        setErrorMessage(
          'Native Rust core belum terpasang. Install .ipa hasil GitHub Actions di iPhone.'
        );
        return;
      }

      try {
        const resolved = await LumeCore.resolvePlayableTrack(track, forceRematch);
        if (gen !== loadGenRef.current) return;

        setCurrentStream(resolved);
        const player = playerRef.current;
        if (player) {
          player.replace({ uri: resolved.uri });
          player.setActiveForLockScreen(
            true,
            {
              title: track.title,
              artist: trackArtistDisplay(track),
              albumTitle:
                track.album ?? (resolved.is_offline ? 'Offline Library' : 'Lume Music'),
              artworkUrl: track.artwork_ref ?? undefined,
            },
            {
              showNextTrack: true,
              showPreviousTrack: true,
              showSeekForward: false,
              showSeekBackward: false,
            }
          );
          player.play();
        }

        // Best-effort history & lyrics fetch without blocking audio start.
        LumeCore.recordPlayHistory(track).catch(() => {});
        setLyricsLoading(true);
        LumeCore.fetchLyrics(track, false)
          .then((data) => {
            if (gen === loadGenRef.current) {
              setLyrics(data);
            }
          })
          .catch(() => {})
          .finally(() => {
            if (gen === loadGenRef.current) {
              setLyricsLoading(false);
            }
          });
      } catch (err) {
        if (gen !== loadGenRef.current) return;
        setIsBuffering(false);
        setIsPlaying(false);
        setErrorMessage(
          sanitizeErrorMessage(err instanceof Error ? err.message : String(err))
        );
      }
    },
    [updateDynamicPalette]
  );

  const advanceTrack = useCallback(
    async (isAutoFinish: boolean) => {
      const list = queueRef.current;
      if (list.length === 0) return;

      const currentIdx = queueIndexRef.current;
      // Only repeat the same song when it finishes naturally; manual Next always skips ahead.
      if (isAutoFinish && repeatRef.current === 'one' && currentIdx >= 0 && list[currentIdx]) {
        const player = playerRef.current;
        if (player) {
          finishedHandledForGenRef.current = -1;
          await player.seekTo(0);
          publishProgress(0, effectiveDurationRef.current);
          player.play();
          return;
        }
        await loadTrackInternal(list[currentIdx], undefined, currentIdx);
        return;
      }

      let nextIdx = currentIdx + 1;
      if (nextIdx >= list.length) {
        if (repeatRef.current === 'all' || (!isAutoFinish && list.length > 1)) {
          if (shuffleRef.current && list.length > 1) {
            const reshuffled = fisherYatesShuffle(
              originalQueueRef.current.length > 0 ? originalQueueRef.current : list
            );
            // Avoid immediately repeating the exact song that just finished.
            if (reshuffled.length > 1 && reshuffled[0].uid === list[currentIdx]?.uid) {
              [reshuffled[0], reshuffled[1]] = [reshuffled[1], reshuffled[0]];
            }
            await loadTrackInternal(reshuffled[0], reshuffled, 0);
            return;
          }
          nextIdx = 0;
        } else {
          playerRef.current?.pause();
          return;
        }
      }

      await loadTrackInternal(list[nextIdx], undefined, nextIdx);
    },
    [loadTrackInternal]
  );

  const nextTrack = useCallback(async () => {
    await advanceTrack(false);
  }, [advanceTrack]);

  const previousTrack = useCallback(async () => {
    const list = queueRef.current;
    if (list.length === 0) return;

    if (progressStore.snapshot.positionMs > 3000 && playerRef.current) {
      await playerRef.current.seekTo(0);
      publishProgress(0, effectiveDurationRef.current);
      return;
    }

    const currentIdx = queueIndexRef.current;
    const prevIdx = currentIdx <= 0 ? list.length - 1 : currentIdx - 1;
    await loadTrackInternal(list[prevIdx], undefined, prevIdx);
  }, [loadTrackInternal]);

  // Initialize native iOS AVAudioSession + persistent AudioPlayer + Lock Screen Remote Commands.
  useEffect(() => {
    let mounted = true;

    SecureStore.getItemAsync(KEY_DYNAMIC_THEME)
      .then((val) => {
        if (val === '0') {
          setDynamicThemeEnabledState(false);
          dynamicThemeEnabledRef.current = false;
        }
      })
      .catch(() => {});

    setAudioModeAsync({
      playsInSilentMode: true,
      shouldPlayInBackground: true,
      interruptionMode: 'doNotMix',
      allowsRecording: false,
      shouldRouteThroughEarpiece: false,
    }).catch(() => {});

    const player = createAudioPlayer(null, {
      updateInterval: 50,
      keepAudioSessionActive: true,
      preferredForwardBufferDuration: 30,
      allowsExternalPlayback: false,
    });
    playerRef.current = player;

    const statusSub = player.addListener('playbackStatusUpdate', (status) => {
      if (!mounted) return;
      if (isPlayingRef.current !== status.playing) {
        isPlayingRef.current = status.playing;
        setIsPlaying(status.playing);
      }
      if (isBufferingRef.current !== status.isBuffering) {
        isBufferingRef.current = status.isBuffering;
        setIsBuffering(status.isBuffering);
      }

      const reportedPosMs = Math.round((status.currentTime || 0) * 1000);
      const trackMetaDurationMs = currentTrackRef.current?.duration_ms ?? 0;
      const reportedDurMs =
        status.duration && Number.isFinite(status.duration) && status.duration > 0
          ? Math.round(status.duration * 1000)
          : 0;

      // Guard against Apple AVPlayer's known 2x duration inflation on YouTube DASH M4A streams
      // (e.g. 4:03 reported as 8:03 with 4 minutes of trailing silence).
      let effectiveDur = trackMetaDurationMs;
      if (reportedDurMs > 0) {
        if (trackMetaDurationMs <= 0) {
          effectiveDur = reportedDurMs;
        } else if (reportedDurMs <= trackMetaDurationMs * 1.22) {
          effectiveDur = reportedDurMs;
        } else {
          effectiveDur = trackMetaDurationMs;
        }
      }

      if (effectiveDur > 0) {
        effectiveDurationRef.current = effectiveDur;
      }

      const clampedPos =
        effectiveDur > 0 ? Math.min(reportedPosMs, effectiveDur) : reportedPosMs;
      publishProgress(clampedPos, effectiveDurationRef.current);

      if (status.error) {
        setErrorMessage(status.error);
      }

      const hitInflatedEnd =
        effectiveDur > 0 &&
        reportedDurMs > effectiveDur * 1.22 &&
        reportedPosMs >= effectiveDur - 350 &&
        status.playing;

      if (
        (status.didJustFinish || hitInflatedEnd) &&
        finishedHandledForGenRef.current !== loadGenRef.current
      ) {
        finishedHandledForGenRef.current = loadGenRef.current;
        if (hitInflatedEnd) {
          player.pause();
        }
        void advanceTrack(true);
      }
    });

    const unsubRemote = LumeCore.subscribeRemoteCommands(
      () => {
        void nextTrack();
      },
      () => {
        void previousTrack();
      }
    );

    void refreshOfflineCache();
    void refreshLibraryState();

    return () => {
      mounted = false;
      statusSub.remove();
      unsubRemote();
      player.clearLockScreenControls();
      player.remove();
      playerRef.current = null;
    };
  }, [advanceTrack, nextTrack, previousTrack, refreshLibraryState, refreshOfflineCache]);

  const playTrack = useCallback(
    async (
      track: Track,
      newQueue?: Track[],
      forceRematch = false,
      overrideShuffle?: boolean
    ) => {
      const effectiveShuffle =
        overrideShuffle !== undefined ? overrideShuffle : shuffleRef.current;
      if (overrideShuffle !== undefined && overrideShuffle !== shuffleRef.current) {
        shuffleRef.current = overrideShuffle;
        setShuffle(overrideShuffle);
      }

      const sourceList = newQueue && newQueue.length > 0 ? [...newQueue] : [track];
      originalQueueRef.current = sourceList;

      if (effectiveShuffle && sourceList.length > 1) {
        const rest = sourceList.filter((t) => t.uid !== track.uid);
        const shuffledQueue = [track, ...fisherYatesShuffle(rest)];
        await loadTrackInternal(track, shuffledQueue, 0, forceRematch);
      } else {
        const idx = Math.max(0, sourceList.findIndex((t) => t.uid === track.uid));
        await loadTrackInternal(track, sourceList, idx, forceRematch);
      }
    },
    [loadTrackInternal]
  );

  const playNext = useCallback((track: Track) => {
    setQueue((prev) => {
      const next = prev.length === 0 ? [track] : [...prev];
      if (prev.length > 0) {
        const idx = Math.max(0, queueIndexRef.current);
        next.splice(idx + 1, 0, track);
      }
      queueRef.current = next;
      return next;
    });
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
  }, []);

  const addToQueue = useCallback((track: Track) => {
    setQueue((prev) => {
      const next = [...prev, track];
      queueRef.current = next;
      return next;
    });
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
  }, []);

  const removeFromQueue = useCallback((index: number) => {
    setQueue((prev) => {
      if (index < 0 || index >= prev.length || index === queueIndexRef.current) return prev;
      const next = prev.filter((_, i) => i !== index);
      queueRef.current = next;
      if (index < queueIndexRef.current) {
        const nextIdx = Math.max(0, queueIndexRef.current - 1);
        queueIndexRef.current = nextIdx;
        setQueueIndex(nextIdx);
      }
      return next;
    });
  }, []);

  const reorderQueue = useCallback((fromIndex: number, toIndex: number) => {
    const prev = queueRef.current;
    if (
      fromIndex === toIndex ||
      fromIndex < 0 ||
      toIndex < 0 ||
      fromIndex >= prev.length ||
      toIndex >= prev.length
    ) {
      return;
    }
    const next = [...prev];
    const [moved] = next.splice(fromIndex, 1);
    next.splice(toIndex, 0, moved);

    let nextActiveIdx = queueIndexRef.current;
    if (fromIndex === nextActiveIdx) {
      nextActiveIdx = toIndex;
    } else if (fromIndex < nextActiveIdx && toIndex >= nextActiveIdx) {
      nextActiveIdx -= 1;
    } else if (fromIndex > nextActiveIdx && toIndex <= nextActiveIdx) {
      nextActiveIdx += 1;
    }

    queueRef.current = next;
    queueIndexRef.current = nextActiveIdx;
    setQueue(next);
    setQueueIndex(nextActiveIdx);
    if (!shuffleRef.current) {
      originalQueueRef.current = next;
    }
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
  }, []);

  const jumpToQueueIndex = useCallback(
    async (index: number) => {
      const list = queueRef.current;
      if (index < 0 || index >= list.length) return;
      await loadTrackInternal(list[index], undefined, index);
    },
    [loadTrackInternal]
  );

  const clearUpcomingQueue = useCallback(() => {
    setQueue((prev) => {
      const idx = queueIndexRef.current;
      const next = idx < 0 || idx >= prev.length ? [] : prev.slice(0, idx + 1);
      queueRef.current = next;
      return next;
    });
  }, []);

  const togglePlayPause = useCallback(() => {
    const player = playerRef.current;
    if (!player || !currentTrack) return;
    if (player.playing) {
      player.pause();
    } else {
      player.play();
    }
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
  }, [currentTrack]);

  const seekToMs = useCallback(async (ms: number) => {
    const player = playerRef.current;
    if (!player) return;
    const maxMs = effectiveDurationRef.current > 0 ? effectiveDurationRef.current - 400 : ms;
    const clampedMs = Math.max(0, Math.min(ms, Math.max(0, maxMs)));
    await player.seekTo(clampedMs / 1000);
    publishProgress(Math.round(clampedMs), effectiveDurationRef.current);
  }, []);

  const toggleShuffle = useCallback(() => {
    const nextShuffle = !shuffleRef.current;
    shuffleRef.current = nextShuffle;
    setShuffle(nextShuffle);
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});

    const curQueue = queueRef.current;
    const curIdx = queueIndexRef.current;
    const active = curIdx >= 0 && curIdx < curQueue.length ? curQueue[curIdx] : currentTrackRef.current;

    if (curQueue.length <= 1) return;

    if (nextShuffle) {
      if (originalQueueRef.current.length === 0) {
        originalQueueRef.current = [...curQueue];
      }
      if (active) {
        const rest = curQueue.filter((t, i) => i !== curIdx && t.uid !== active.uid);
        const shuffled = [active, ...fisherYatesShuffle(rest)];
        queueRef.current = shuffled;
        queueIndexRef.current = 0;
        setQueue(shuffled);
        setQueueIndex(0);
      } else {
        const shuffled = fisherYatesShuffle(curQueue);
        queueRef.current = shuffled;
        queueIndexRef.current = 0;
        setQueue(shuffled);
        setQueueIndex(0);
      }
    } else if (originalQueueRef.current.length > 0) {
      const restored = [...originalQueueRef.current];
      const restoredIdx = active
        ? Math.max(0, restored.findIndex((t) => t.uid === active.uid))
        : 0;
      queueRef.current = restored;
      queueIndexRef.current = restoredIdx;
      setQueue(restored);
      setQueueIndex(restoredIdx);
    }
  }, []);

  const cycleRepeat = useCallback(() => {
    const cur = repeatRef.current;
    const next: RepeatMode = cur === 'off' ? 'all' : cur === 'all' ? 'one' : 'off';
    repeatRef.current = next;
    setRepeat(next);
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
  }, []);

  const toggleLikeTrack = useCallback(
    async (track: Track): Promise<boolean> => {
      const currentlyLiked = likedUids.has(track.uid);
      const nextLiked = !currentlyLiked;
      await LumeCore.setTrackLiked(track, nextLiked);
      setCurrentTrack((prev) =>
        prev && prev.uid === track.uid ? { ...prev, is_liked: nextLiked } : prev
      );
      setQueue((prev) =>
        prev.map((t) => (t.uid === track.uid ? { ...t, is_liked: nextLiked } : t))
      );
      await refreshLibraryState();
      Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
      return nextLiked;
    },
    [likedUids, refreshLibraryState]
  );

  const toggleLikeCurrent = useCallback(async () => {
    if (!currentTrack) return;
    await toggleLikeTrack(currentTrack);
  }, [currentTrack, toggleLikeTrack]);

  const downloadOffline = useCallback(
    async (track: Track) => {
      if (downloadingUids.has(track.uid) || offlineUids.has(track.uid)) return;
      setDownloadingUids((prev) => new Set(prev).add(track.uid));
      try {
        await LumeCore.downloadTrackOffline(track);
        await refreshOfflineCache();
        Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success).catch(() => {});
      } catch (err) {
        setErrorMessage(
          sanitizeErrorMessage(err instanceof Error ? err.message : String(err))
        );
      } finally {
        setDownloadingUids((prev) => {
          const next = new Set(prev);
          next.delete(track.uid);
          return next;
        });
      }
    },
    [downloadingUids, offlineUids, refreshOfflineCache]
  );

  const cancelBatchDownload = useCallback(() => {
    batchCancelRef.current = true;
    setBatchDownload((prev) =>
      prev ? { ...prev, statusNote: 'Membatalkan setelah lagu ini...' } : null
    );
  }, []);

  /**
   * Rate-limit-safe sequential playlist downloader:
   * - Concurrency = 1 (never floods InnerTube / Spotify / LRCLIB)
   * - Instant skip for already-cached tracks (supports resume!)
   * - Polite pacing with jitter: 1.2–1.7s for YTM tracks, 2.2–2.9s for Spotify->YTM matches
   * - Exponential backoff on 429/403/network errors; auto-stops after 3 consecutive failures
   */
  const downloadPlaylistOffline = useCallback(
    async (playlistName: string, tracks: Track[]) => {
      if (tracks.length === 0) return;
      if (batchRunningRef.current) {
        setErrorMessage('Masih ada unduhan playlist yang berjalan. Batalkan dulu jika ingin ganti.');
        return;
      }

      batchRunningRef.current = true;
      batchCancelRef.current = false;

      // Fresh snapshot of already-downloaded UIDs so resume always skips completed tracks.
      let alreadyOffline = new Set<string>(offlineUids);
      try {
        const fresh = await LumeCore.listOfflineTracks();
        alreadyOffline = new Set(fresh.map((e) => e.track.uid));
      } catch {
        // Use in-memory set.
      }

      let completed = 0;
      let skipped = 0;
      let failed = 0;
      let consecutiveErrors = 0;

      setBatchDownload({
        playlistName,
        total: tracks.length,
        completed: 0,
        skipped: 0,
        failed: 0,
        currentTitle: 'Menyiapkan antrean aman...',
        active: true,
        statusNote: 'Mode aman rate-limit (1 lagu/giliran)',
      });

      try {
        for (const track of tracks) {
          if (batchCancelRef.current) break;

          if ('Local' in track.source || alreadyOffline.has(track.uid)) {
            skipped += 1;
            completed += 1;
            setBatchDownload((prev) =>
              prev ? { ...prev, completed, skipped, currentTitle: track.title } : null
            );
            continue;
          }

          setBatchDownload((prev) =>
            prev
              ? {
                  ...prev,
                  currentTitle: track.title,
                  statusNote: 'Mengunduh chunk 512 KB...',
                }
              : null
          );
          setDownloadingUids((prev) => new Set(prev).add(track.uid));

          let downloadedThisTrack = false;
          for (let attempt = 0; attempt < 2; attempt++) {
            if (batchCancelRef.current) break;
            try {
              await LumeCore.downloadTrackOffline(track);
              alreadyOffline.add(track.uid);
              downloadedThisTrack = true;
              consecutiveErrors = 0;
              break;
            } catch (err) {
              const msg = err instanceof Error ? err.message : String(err);
              const isRateLimit = msg.includes('429') || msg.includes('403');
              if (attempt === 0) {
                const backoffMs = isRateLimit ? 8000 : 3500;
                setBatchDownload((prev) =>
                  prev
                    ? {
                        ...prev,
                        statusNote: isRateLimit
                          ? 'Menunggu jeda rate-limit (8 dtk)...'
                          : 'Koneksi goyang, coba ulang (3.5 dtk)...',
                      }
                    : null
                );
                await sleepMs(backoffMs);
              }
            }
          }

          setDownloadingUids((prev) => {
            const next = new Set(prev);
            next.delete(track.uid);
            return next;
          });

          if (downloadedThisTrack) {
            completed += 1;
            await refreshOfflineCache();
          } else if (!batchCancelRef.current) {
            failed += 1;
            consecutiveErrors += 1;
          }

          setBatchDownload((prev) =>
            prev ? { ...prev, completed, skipped, failed } : null
          );

          if (consecutiveErrors >= 3) {
            setErrorMessage(
              `Unduhan dijeda otomatis setelah 3 gagal berturut-turut untuk menjaga rate-limit API (${completed}/${tracks.length} selesai). Tekan Offline All lagi nanti untuk lanjut.`
            );
            break;
          }

          if (!batchCancelRef.current && completed + failed < tracks.length) {
            const isSpotify = 'Spotify' in track.source;
            const baseDelay = isSpotify ? 2200 : 1200;
            const jitter = Math.floor(Math.random() * (isSpotify ? 700 : 500));
            setBatchDownload((prev) =>
              prev ? { ...prev, statusNote: 'Jeda aman rate-limit...' } : null
            );
            await sleepMs(baseDelay + jitter);
          }
        }
      } finally {
        batchRunningRef.current = false;
        const wasCancelled = batchCancelRef.current;
        setBatchDownload({
          playlistName,
          total: tracks.length,
          completed,
          skipped,
          failed,
          currentTitle: wasCancelled
            ? `Dibatalkan (${completed}/${tracks.length} tersimpan)`
            : `Selesai (${completed}/${tracks.length} siap offline)`,
          active: false,
          statusNote:
            failed > 0
              ? `${failed} gagal — tekan Offline All lagi untuk lanjutkan yang tersisa`
              : null,
        });
        if (!wasCancelled && failed === 0) {
          Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success).catch(() => {});
        }
        setTimeout(() => {
          setBatchDownload((cur) => (cur && !cur.active ? null : cur));
        }, 5000);
      }
    },
    [offlineUids, refreshOfflineCache]
  );

  const removeOffline = useCallback(
    async (trackUid: string) => {
      await LumeCore.deleteOfflineTrack(trackUid);
      await refreshOfflineCache();
    },
    [refreshOfflineCache]
  );

  const clearAllOffline = useCallback(async () => {
    await LumeCore.clearAllOfflineTracks();
    await refreshOfflineCache();
  }, [refreshOfflineCache]);

  const refreshLyrics = useCallback(async () => {
    const track = currentTrackRef.current;
    if (!track || !LumeCore.isAvailable()) return;
    setLyricsLoading(true);
    try {
      const data = await LumeCore.fetchLyrics(track, true);
      setLyrics(data);
    } catch {
      // Ignore refresh failure.
    } finally {
      setLyricsLoading(false);
    }
  }, []);

  const openTrackMenu = useCallback((track: Track) => setMenuTrack(track), []);
  const closeTrackMenu = useCallback(() => setMenuTrack(null), []);
  const clearError = useCallback(() => setErrorMessage(null), []);

  const value = useMemo<PlayerContextValue>(
    () => ({
      currentTrack,
      currentStream,
      queue,
      queueIndex,
      isPlaying,
      isBuffering,
      shuffle,
      repeat,
      lyrics,
      lyricsLoading,
      likedTracks,
      likedUids,
      playlists,
      offlineUids,
      downloadingUids,
      offlineEntries,
      offlineBytes,
      batchDownload,
      palette,
      dynamicThemeEnabled,
      menuTrack,
      errorMessage,
      playTrack,
      playNext,
      addToQueue,
      removeFromQueue,
      reorderQueue,
      jumpToQueueIndex,
      clearUpcomingQueue,
      togglePlayPause,
      nextTrack,
      previousTrack,
      seekToMs,
      toggleShuffle,
      cycleRepeat,
      toggleLikeCurrent,
      toggleLikeTrack,
      refreshLibraryState,
      downloadOffline,
      downloadPlaylistOffline,
      cancelBatchDownload,
      removeOffline,
      clearAllOffline,
      refreshOfflineCache,
      refreshLyrics,
      setDynamicThemeEnabled,
      openTrackMenu,
      closeTrackMenu,
      clearError,
    }),
    [
      currentTrack,
      currentStream,
      queue,
      queueIndex,
      isPlaying,
      isBuffering,
      shuffle,
      repeat,
      lyrics,
      lyricsLoading,
      likedTracks,
      likedUids,
      playlists,
      offlineUids,
      downloadingUids,
      offlineEntries,
      offlineBytes,
      batchDownload,
      palette,
      dynamicThemeEnabled,
      menuTrack,
      errorMessage,
      playTrack,
      playNext,
      addToQueue,
      removeFromQueue,
      reorderQueue,
      jumpToQueueIndex,
      clearUpcomingQueue,
      togglePlayPause,
      nextTrack,
      previousTrack,
      seekToMs,
      toggleShuffle,
      cycleRepeat,
      toggleLikeCurrent,
      toggleLikeTrack,
      refreshLibraryState,
      downloadOffline,
      downloadPlaylistOffline,
      cancelBatchDownload,
      removeOffline,
      clearAllOffline,
      refreshOfflineCache,
      refreshLyrics,
      setDynamicThemeEnabled,
      openTrackMenu,
      closeTrackMenu,
      clearError,
    ]
  );

  return <PlayerContext.Provider value={value}>{children}</PlayerContext.Provider>;
}

export function usePlayer(): PlayerContextValue {
  const ctx = useContext(PlayerContext);
  if (!ctx) {
    throw new Error('usePlayer must be used within PlayerProvider');
  }
  return ctx;
}
