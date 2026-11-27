import React, {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from 'react';
import { createAudioPlayer, setAudioModeAsync, type AudioPlayer } from 'expo-audio';
import * as Haptics from 'expo-haptics';
import { LumeCore } from '../../modules/lume-core';
import {
  type LyricsData,
  type OfflineEntryDto,
  type PlayableStreamDto,
  type RepeatMode,
  type Track,
  trackArtistDisplay,
} from '../types';

interface PlayerContextValue {
  currentTrack: Track | null;
  currentStream: PlayableStreamDto | null;
  queue: Track[];
  queueIndex: number;
  isPlaying: boolean;
  isBuffering: boolean;
  positionMs: number;
  durationMs: number;
  shuffle: boolean;
  repeat: RepeatMode;
  lyrics: LyricsData | null;
  lyricsLoading: boolean;
  offlineUids: Set<string>;
  downloadingUids: Set<string>;
  offlineEntries: OfflineEntryDto[];
  offlineBytes: number;
  errorMessage: string | null;
  playTrack: (track: Track, newQueue?: Track[]) => Promise<void>;
  togglePlayPause: () => void;
  nextTrack: () => Promise<void>;
  previousTrack: () => Promise<void>;
  seekToMs: (ms: number) => Promise<void>;
  toggleShuffle: () => void;
  cycleRepeat: () => void;
  toggleLikeCurrent: () => Promise<void>;
  toggleLikeTrack: (track: Track) => Promise<boolean>;
  downloadOffline: (track: Track) => Promise<void>;
  removeOffline: (trackUid: string) => Promise<void>;
  refreshOfflineCache: () => Promise<void>;
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
  const [positionMs, setPositionMs] = useState(0);
  const [durationMs, setDurationMs] = useState(0);
  const [shuffle, setShuffle] = useState(false);
  const [repeat, setRepeat] = useState<RepeatMode>('all');
  const [lyrics, setLyrics] = useState<LyricsData | null>(null);
  const [lyricsLoading, setLyricsLoading] = useState(false);
  const [offlineEntries, setOfflineEntries] = useState<OfflineEntryDto[]>([]);
  const [offlineBytes, setOfflineBytes] = useState(0);
  const [downloadingUids, setDownloadingUids] = useState<Set<string>>(new Set());
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const queueRef = useRef(queue);
  const queueIndexRef = useRef(queueIndex);
  const shuffleRef = useRef(shuffle);
  const repeatRef = useRef(repeat);
  const positionMsRef = useRef(positionMs);

  queueRef.current = queue;
  queueIndexRef.current = queueIndex;
  shuffleRef.current = shuffle;
  repeatRef.current = repeat;
  positionMsRef.current = positionMs;

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

  const loadTrackInternal = useCallback(
    async (track: Track, newQueue?: Track[], explicitIndex?: number) => {
      const gen = ++loadGenRef.current;
      finishedHandledForGenRef.current = -1;
      setErrorMessage(null);
      setCurrentTrack(track);
      setIsBuffering(true);
      setPositionMs(0);
      setDurationMs(track.duration_ms);
      setLyrics(null);

      if (newQueue) {
        setQueue(newQueue);
        const idx =
          explicitIndex ?? Math.max(0, newQueue.findIndex((t) => t.uid === track.uid));
        setQueueIndex(idx);
      } else if (explicitIndex !== undefined) {
        setQueueIndex(explicitIndex);
      }

      if (!LumeCore.isAvailable()) {
        setIsBuffering(false);
        setErrorMessage(
          'Native Rust core belum terpasang. Install .ipa hasil GitHub Actions di iPhone.'
        );
        return;
      }

      try {
        const resolved = await LumeCore.resolvePlayableTrack(track, false);
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
              albumTitle: track.album ?? (resolved.is_offline ? 'Offline Library' : 'Lume Music'),
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

        // Best-effort history & lyrics fetch in parallel without blocking playback start.
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
        setErrorMessage(err instanceof Error ? err.message : String(err));
      }
    },
    []
  );

  const nextTrack = useCallback(async () => {
    const list = queueRef.current;
    if (list.length === 0) return;

    const currentIdx = queueIndexRef.current;
    if (repeatRef.current === 'one' && currentIdx >= 0 && list[currentIdx]) {
      await loadTrackInternal(list[currentIdx], undefined, currentIdx);
      return;
    }

    let nextIdx: number;
    if (shuffleRef.current && list.length > 1) {
      do {
        nextIdx = Math.floor(Math.random() * list.length);
      } while (nextIdx === currentIdx);
    } else {
      nextIdx = currentIdx + 1;
      if (nextIdx >= list.length) {
        if (repeatRef.current === 'all') {
          nextIdx = 0;
        } else {
          playerRef.current?.pause();
          return;
        }
      }
    }

    await loadTrackInternal(list[nextIdx], undefined, nextIdx);
  }, [loadTrackInternal]);

  const previousTrack = useCallback(async () => {
    const list = queueRef.current;
    if (list.length === 0) return;

    // Standard music player behavior: if > 3 seconds into song, restart current track.
    if (positionMsRef.current > 3000 && playerRef.current) {
      await playerRef.current.seekTo(0);
      setPositionMs(0);
      return;
    }

    const currentIdx = queueIndexRef.current;
    const prevIdx = currentIdx <= 0 ? list.length - 1 : currentIdx - 1;
    await loadTrackInternal(list[prevIdx], undefined, prevIdx);
  }, [loadTrackInternal]);

  // Initialize native iOS AVAudioSession + persistent AudioPlayer + Lock Screen Remote Commands.
  useEffect(() => {
    let mounted = true;

    setAudioModeAsync({
      playsInSilentMode: true,
      shouldPlayInBackground: true,
      interruptionMode: 'doNotMix',
      allowsRecording: false,
      shouldRouteThroughEarpiece: false,
    }).catch(() => {});

    const player = createAudioPlayer(null, {
      updateInterval: 100,
      keepAudioSessionActive: true,
      preferredForwardBufferDuration: 30,
      allowsExternalPlayback: false,
    });
    playerRef.current = player;

    const statusSub = player.addListener('playbackStatusUpdate', (status) => {
      if (!mounted) return;
      setIsPlaying(status.playing);
      setIsBuffering(status.isBuffering);
      setPositionMs(Math.round((status.currentTime || 0) * 1000));
      if (status.duration && status.duration > 0) {
        setDurationMs(Math.round(status.duration * 1000));
      }
      if (status.error) {
        setErrorMessage(status.error);
      }
      if (status.didJustFinish && finishedHandledForGenRef.current !== loadGenRef.current) {
        finishedHandledForGenRef.current = loadGenRef.current;
        void nextTrack();
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

    return () => {
      mounted = false;
      statusSub.remove();
      unsubRemote();
      player.clearLockScreenControls();
      player.remove();
      playerRef.current = null;
    };
  }, [nextTrack, previousTrack, refreshOfflineCache]);

  const playTrack = useCallback(
    async (track: Track, newQueue?: Track[]) => {
      await loadTrackInternal(track, newQueue ?? [track]);
    },
    [loadTrackInternal]
  );

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
    const clampedSec = Math.max(0, ms / 1000);
    await player.seekTo(clampedSec);
    setPositionMs(Math.round(clampedSec * 1000));
  }, []);

  const toggleShuffle = useCallback(() => {
    setShuffle((prev) => !prev);
  }, []);

  const cycleRepeat = useCallback(() => {
    setRepeat((prev) => (prev === 'off' ? 'all' : prev === 'all' ? 'one' : 'off'));
  }, []);

  const toggleLikeTrack = useCallback(async (track: Track): Promise<boolean> => {
    const nextLiked = !track.is_liked;
    await LumeCore.setTrackLiked(track, nextLiked);
    setCurrentTrack((prev) =>
      prev && prev.uid === track.uid ? { ...prev, is_liked: nextLiked } : prev
    );
    setQueue((prev) =>
      prev.map((t) => (t.uid === track.uid ? { ...t, is_liked: nextLiked } : t))
    );
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light).catch(() => {});
    return nextLiked;
  }, []);

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
        setErrorMessage(err instanceof Error ? err.message : String(err));
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

  const removeOffline = useCallback(
    async (trackUid: string) => {
      await LumeCore.deleteOfflineTrack(trackUid);
      await refreshOfflineCache();
    },
    [refreshOfflineCache]
  );

  const clearError = useCallback(() => setErrorMessage(null), []);

  const value = useMemo<PlayerContextValue>(
    () => ({
      currentTrack,
      currentStream,
      queue,
      queueIndex,
      isPlaying,
      isBuffering,
      positionMs,
      durationMs,
      shuffle,
      repeat,
      lyrics,
      lyricsLoading,
      offlineUids,
      downloadingUids,
      offlineEntries,
      offlineBytes,
      errorMessage,
      playTrack,
      togglePlayPause,
      nextTrack,
      previousTrack,
      seekToMs,
      toggleShuffle,
      cycleRepeat,
      toggleLikeCurrent,
      toggleLikeTrack,
      downloadOffline,
      removeOffline,
      refreshOfflineCache,
      clearError,
    }),
    [
      currentTrack,
      currentStream,
      queue,
      queueIndex,
      isPlaying,
      isBuffering,
      positionMs,
      durationMs,
      shuffle,
      repeat,
      lyrics,
      lyricsLoading,
      offlineUids,
      downloadingUids,
      offlineEntries,
      offlineBytes,
      errorMessage,
      playTrack,
      togglePlayPause,
      nextTrack,
      previousTrack,
      seekToMs,
      toggleShuffle,
      cycleRepeat,
      toggleLikeCurrent,
      toggleLikeTrack,
      downloadOffline,
      removeOffline,
      refreshOfflineCache,
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
