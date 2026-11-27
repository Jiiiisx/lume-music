import { requireOptionalNativeModule, EventSubscription } from 'expo-modules-core';
import type {
  HealthCheckDto,
  LyricsData,
  MobileSpotifyAuthStartDto,
  OfflineEntryDto,
  PlayableStreamDto,
  Playlist,
  SpotifySyncSummaryDto,
  SpotifyTokenSet,
  Track,
  YtmCatalogSearchResult,
} from '../../src/types';

interface NativeLumeCoreModule {
  healthCheck(): Promise<string>;
  resolvePlayableTrack(trackJson: string, forceRematch: boolean): Promise<string>;
  downloadTrackOffline(trackJson: string): Promise<string>;
  listOfflineTracks(): Promise<string>;
  deleteOfflineTrack(trackUid: string): Promise<void>;
  offlineStorageBytes(): Promise<number>;
  searchYtm(query: string, limit: number): Promise<string>;
  getCharts(indonesia: boolean, limit: number, forceRefresh: boolean): Promise<string>;
  getArtistProfile(artistId: string): Promise<string>;
  getAlbumDetail(albumId: string): Promise<string>;
  getYtmPlaylistDetail(playlistId: string): Promise<string>;
  listPlaylists(): Promise<string>;
  createPlaylist(name: string): Promise<string>;
  deletePlaylist(playlistId: string): Promise<void>;
  getPlaylistTracks(playlistId: string): Promise<string>;
  addTrackToPlaylist(playlistId: string, trackJson: string): Promise<void>;
  removeTrackFromPlaylist(playlistId: string, position: number): Promise<void>;
  setTrackLiked(trackJson: string, liked: boolean): Promise<void>;
  listLikedTracks(): Promise<string>;
  recordPlayHistory(trackJson: string): Promise<void>;
  listRecentTracks(limit: number): Promise<string>;
  fetchLyrics(trackJson: string, forceRefresh: boolean): Promise<string>;
  beginSpotifyMobileAuth(clientId: string): Promise<string>;
  exchangeSpotifyMobileCode(
    clientId: string,
    code: string,
    codeVerifier: string
  ): Promise<string>;
  refreshSpotifyToken(clientId: string, tokenJson: string): Promise<string>;
  syncSpotifyLibrary(accessToken: string): Promise<string>;
  importSpotifyPlaylistUrl(url: string): Promise<string>;
  rematchSpotifyTrack(spotifyTrackId: string, ytVideoId: string): Promise<void>;
  enableRemoteTrackCommands(): void;
  addListener(
    eventName: 'onRemoteNextTrack' | 'onRemotePreviousTrack',
    listener: () => void
  ): EventSubscription;
}

const native = requireOptionalNativeModule<NativeLumeCoreModule>('LumeCore');

function ensureNative(): NativeLumeCoreModule {
  if (!native) {
    throw new Error(
      'LumeCore native Rust module is not linked. Install the Lume Music iOS DevClient/Release .ipa built via GitHub Actions.'
    );
  }
  return native;
}

export const LumeCore = {
  isAvailable(): boolean {
    return native !== null;
  },

  async healthCheck(): Promise<HealthCheckDto> {
    const raw = await ensureNative().healthCheck();
    return JSON.parse(raw) as HealthCheckDto;
  },

  async resolvePlayableTrack(
    track: Track,
    forceRematch = false
  ): Promise<PlayableStreamDto> {
    const raw = await ensureNative().resolvePlayableTrack(
      JSON.stringify(track),
      forceRematch
    );
    return JSON.parse(raw) as PlayableStreamDto;
  },

  async downloadTrackOffline(track: Track): Promise<OfflineEntryDto> {
    const raw = await ensureNative().downloadTrackOffline(JSON.stringify(track));
    return JSON.parse(raw) as OfflineEntryDto;
  },

  async listOfflineTracks(): Promise<OfflineEntryDto[]> {
    const raw = await ensureNative().listOfflineTracks();
    return JSON.parse(raw) as OfflineEntryDto[];
  },

  async deleteOfflineTrack(trackUid: string): Promise<void> {
    await ensureNative().deleteOfflineTrack(trackUid);
  },

  async offlineStorageBytes(): Promise<number> {
    return ensureNative().offlineStorageBytes();
  },

  async searchYtm(query: string, limit = 20): Promise<YtmCatalogSearchResult> {
    const raw = await ensureNative().searchYtm(query, limit);
    return JSON.parse(raw) as YtmCatalogSearchResult;
  },

  async getCharts(
    indonesia: boolean,
    limit = 25,
    forceRefresh = false
  ): Promise<Track[]> {
    const raw = await ensureNative().getCharts(indonesia, limit, forceRefresh);
    return JSON.parse(raw) as Track[];
  },

  async listPlaylists(): Promise<Playlist[]> {
    const raw = await ensureNative().listPlaylists();
    return JSON.parse(raw) as Playlist[];
  },

  async createPlaylist(name: string): Promise<Playlist> {
    const raw = await ensureNative().createPlaylist(name);
    return JSON.parse(raw) as Playlist;
  },

  async deletePlaylist(playlistId: string): Promise<void> {
    await ensureNative().deletePlaylist(playlistId);
  },

  async getPlaylistTracks(playlistId: string): Promise<Track[]> {
    const raw = await ensureNative().getPlaylistTracks(playlistId);
    return JSON.parse(raw) as Track[];
  },

  async addTrackToPlaylist(playlistId: string, track: Track): Promise<void> {
    await ensureNative().addTrackToPlaylist(playlistId, JSON.stringify(track));
  },

  async removeTrackFromPlaylist(playlistId: string, position: number): Promise<void> {
    await ensureNative().removeTrackFromPlaylist(playlistId, position);
  },

  async setTrackLiked(track: Track, liked: boolean): Promise<void> {
    await ensureNative().setTrackLiked(JSON.stringify(track), liked);
  },

  async listLikedTracks(): Promise<Track[]> {
    const raw = await ensureNative().listLikedTracks();
    return JSON.parse(raw) as Track[];
  },

  async recordPlayHistory(track: Track): Promise<void> {
    await ensureNative().recordPlayHistory(JSON.stringify(track));
  },

  async listRecentTracks(limit = 20): Promise<Track[]> {
    const raw = await ensureNative().listRecentTracks(limit);
    return JSON.parse(raw) as Track[];
  },

  async fetchLyrics(track: Track, forceRefresh = false): Promise<LyricsData | null> {
    const raw = await ensureNative().fetchLyrics(JSON.stringify(track), forceRefresh);
    return JSON.parse(raw) as LyricsData | null;
  },

  async beginSpotifyMobileAuth(clientId: string): Promise<MobileSpotifyAuthStartDto> {
    const raw = await ensureNative().beginSpotifyMobileAuth(clientId);
    return JSON.parse(raw) as MobileSpotifyAuthStartDto;
  },

  async exchangeSpotifyMobileCode(
    clientId: string,
    code: string,
    codeVerifier: string
  ): Promise<SpotifyTokenSet> {
    const raw = await ensureNative().exchangeSpotifyMobileCode(
      clientId,
      code,
      codeVerifier
    );
    return JSON.parse(raw) as SpotifyTokenSet;
  },

  async refreshSpotifyToken(
    clientId: string,
    current: SpotifyTokenSet
  ): Promise<SpotifyTokenSet> {
    const raw = await ensureNative().refreshSpotifyToken(
      clientId,
      JSON.stringify(current)
    );
    return JSON.parse(raw) as SpotifyTokenSet;
  },

  async syncSpotifyLibrary(accessToken: string): Promise<SpotifySyncSummaryDto> {
    const raw = await ensureNative().syncSpotifyLibrary(accessToken);
    return JSON.parse(raw) as SpotifySyncSummaryDto;
  },

  async importSpotifyPlaylistUrl(url: string): Promise<Playlist> {
    const raw = await ensureNative().importSpotifyPlaylistUrl(url);
    return JSON.parse(raw) as Playlist;
  },

  async rematchSpotifyTrack(spotifyTrackId: string, ytVideoId: string): Promise<void> {
    await ensureNative().rematchSpotifyTrack(spotifyTrackId, ytVideoId);
  },

  subscribeRemoteCommands(
    onNext: () => void,
    onPrev: () => void
  ): () => void {
    if (!native) return () => {};
    native.enableRemoteTrackCommands();
    const s1 = native.addListener('onRemoteNextTrack', onNext);
    const s2 = native.addListener('onRemotePreviousTrack', onPrev);
    return () => {
      s1.remove();
      s2.remove();
    };
  },
};
