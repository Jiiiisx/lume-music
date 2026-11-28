import { requireOptionalNativeModule, EventSubscription } from 'expo-modules-core';
import type {
  AlbumDetail,
  ArtistProfile,
  DynamicPaletteDto,
  HealthCheckDto,
  LyricsData,
  MobileSpotifyAuthStartDto,
  OfflineEntryDto,
  PlayableStreamDto,
  Playlist,
  RemotePlaylistDetail,
  SpotifyCatalogSearchResult,
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
  clearAllOfflineTracks(): Promise<void>;
  offlineStorageBytes(): Promise<number>;
  searchYtm(query: string, limit: number): Promise<string>;
  searchSpotifyCatalog(accessToken: string, query: string, limit: number): Promise<string>;
  getCharts(indonesia: boolean, limit: number, forceRefresh: boolean): Promise<string>;
  getArtistProfile(artistId: string): Promise<string>;
  getAlbumDetail(albumId: string): Promise<string>;
  getYtmPlaylistDetail(playlistId: string): Promise<string>;
  listPlaylists(): Promise<string>;
  createPlaylist(name: string): Promise<string>;
  renamePlaylist(playlistId: string, newName: string): Promise<void>;
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
  importYtmPlaylistUrl(urlOrId: string): Promise<string>;
  importExportifyCsv(csvText: string, playlistName: string): Promise<string>;
  importLocalAudioFile(sourceUriOrPath: string, fileName: string): Promise<string>;
  listLocalTracks(): Promise<string>;
  searchLocalTracks(query: string): Promise<string>;
  rematchSpotifyTrack(spotifyTrackId: string, ytVideoId: string): Promise<void>;
  extractPalette(artworkRef: string): Promise<string>;
  enrichSpotifyArtwork(tracksJson: string, maxBatch: number): Promise<string>;
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
      'LumeCore native Rust module is not linked. Install the Lume Music iOS .ipa built via GitHub Actions.'
    );
  }
  return native;
}

export const LumeCore = {
  isAvailable(): boolean {
    return native !== null;
  },

  async healthCheck(): Promise<HealthCheckDto> {
    return JSON.parse(await ensureNative().healthCheck()) as HealthCheckDto;
  },

  async resolvePlayableTrack(
    track: Track,
    forceRematch = false
  ): Promise<PlayableStreamDto> {
    return JSON.parse(
      await ensureNative().resolvePlayableTrack(JSON.stringify(track), forceRematch)
    ) as PlayableStreamDto;
  },

  async downloadTrackOffline(track: Track): Promise<OfflineEntryDto> {
    return JSON.parse(
      await ensureNative().downloadTrackOffline(JSON.stringify(track))
    ) as OfflineEntryDto;
  },

  async listOfflineTracks(): Promise<OfflineEntryDto[]> {
    return JSON.parse(await ensureNative().listOfflineTracks()) as OfflineEntryDto[];
  },

  async deleteOfflineTrack(trackUid: string): Promise<void> {
    await ensureNative().deleteOfflineTrack(trackUid);
  },

  async clearAllOfflineTracks(): Promise<void> {
    await ensureNative().clearAllOfflineTracks();
  },

  async offlineStorageBytes(): Promise<number> {
    return ensureNative().offlineStorageBytes();
  },

  async searchYtm(query: string, limit = 20): Promise<YtmCatalogSearchResult> {
    return JSON.parse(await ensureNative().searchYtm(query, limit)) as YtmCatalogSearchResult;
  },

  async searchSpotifyCatalog(
    accessToken: string,
    query: string,
    limit = 20
  ): Promise<SpotifyCatalogSearchResult> {
    return JSON.parse(
      await ensureNative().searchSpotifyCatalog(accessToken, query, limit)
    ) as SpotifyCatalogSearchResult;
  },

  async getCharts(
    indonesia: boolean,
    limit = 25,
    forceRefresh = false
  ): Promise<Track[]> {
    return JSON.parse(
      await ensureNative().getCharts(indonesia, limit, forceRefresh)
    ) as Track[];
  },

  async getArtistProfile(artistId: string): Promise<ArtistProfile> {
    return JSON.parse(await ensureNative().getArtistProfile(artistId)) as ArtistProfile;
  },

  async getAlbumDetail(albumId: string): Promise<AlbumDetail> {
    return JSON.parse(await ensureNative().getAlbumDetail(albumId)) as AlbumDetail;
  },

  async getYtmPlaylistDetail(playlistId: string): Promise<RemotePlaylistDetail> {
    return JSON.parse(
      await ensureNative().getYtmPlaylistDetail(playlistId)
    ) as RemotePlaylistDetail;
  },

  async listPlaylists(): Promise<Playlist[]> {
    return JSON.parse(await ensureNative().listPlaylists()) as Playlist[];
  },

  async createPlaylist(name: string): Promise<Playlist> {
    return JSON.parse(await ensureNative().createPlaylist(name)) as Playlist;
  },

  async renamePlaylist(playlistId: string, newName: string): Promise<void> {
    await ensureNative().renamePlaylist(playlistId, newName);
  },

  async deletePlaylist(playlistId: string): Promise<void> {
    await ensureNative().deletePlaylist(playlistId);
  },

  async getPlaylistTracks(playlistId: string): Promise<Track[]> {
    return JSON.parse(await ensureNative().getPlaylistTracks(playlistId)) as Track[];
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
    return JSON.parse(await ensureNative().listLikedTracks()) as Track[];
  },

  async recordPlayHistory(track: Track): Promise<void> {
    await ensureNative().recordPlayHistory(JSON.stringify(track));
  },

  async listRecentTracks(limit = 20): Promise<Track[]> {
    return JSON.parse(await ensureNative().listRecentTracks(limit)) as Track[];
  },

  async fetchLyrics(track: Track, forceRefresh = false): Promise<LyricsData | null> {
    return JSON.parse(
      await ensureNative().fetchLyrics(JSON.stringify(track), forceRefresh)
    ) as LyricsData | null;
  },

  async beginSpotifyMobileAuth(clientId: string): Promise<MobileSpotifyAuthStartDto> {
    return JSON.parse(
      await ensureNative().beginSpotifyMobileAuth(clientId)
    ) as MobileSpotifyAuthStartDto;
  },

  async exchangeSpotifyMobileCode(
    clientId: string,
    code: string,
    codeVerifier: string
  ): Promise<SpotifyTokenSet> {
    return JSON.parse(
      await ensureNative().exchangeSpotifyMobileCode(clientId, code, codeVerifier)
    ) as SpotifyTokenSet;
  },

  async refreshSpotifyToken(
    clientId: string,
    current: SpotifyTokenSet
  ): Promise<SpotifyTokenSet> {
    return JSON.parse(
      await ensureNative().refreshSpotifyToken(clientId, JSON.stringify(current))
    ) as SpotifyTokenSet;
  },

  async syncSpotifyLibrary(accessToken: string): Promise<SpotifySyncSummaryDto> {
    return JSON.parse(
      await ensureNative().syncSpotifyLibrary(accessToken)
    ) as SpotifySyncSummaryDto;
  },

  async importSpotifyPlaylistUrl(url: string): Promise<Playlist> {
    return JSON.parse(await ensureNative().importSpotifyPlaylistUrl(url)) as Playlist;
  },

  async importYtmPlaylistUrl(urlOrId: string): Promise<Playlist> {
    return JSON.parse(await ensureNative().importYtmPlaylistUrl(urlOrId)) as Playlist;
  },

  async importExportifyCsv(csvText: string, playlistName: string): Promise<Playlist> {
    return JSON.parse(
      await ensureNative().importExportifyCsv(csvText, playlistName)
    ) as Playlist;
  },

  async importLocalAudioFile(sourceUriOrPath: string, fileName: string): Promise<Track[]> {
    return JSON.parse(
      await ensureNative().importLocalAudioFile(sourceUriOrPath, fileName)
    ) as Track[];
  },

  async listLocalTracks(): Promise<Track[]> {
    return JSON.parse(await ensureNative().listLocalTracks()) as Track[];
  },

  async searchLocalTracks(query: string): Promise<Track[]> {
    return JSON.parse(await ensureNative().searchLocalTracks(query)) as Track[];
  },

  async rematchSpotifyTrack(spotifyTrackId: string, ytVideoId: string): Promise<void> {
    await ensureNative().rematchSpotifyTrack(spotifyTrackId, ytVideoId);
  },

  async extractPalette(artworkRef: string): Promise<DynamicPaletteDto> {
    return JSON.parse(await ensureNative().extractPalette(artworkRef)) as DynamicPaletteDto;
  },

  async enrichSpotifyArtwork(
    tracks: Track[],
    maxBatch = 16
  ): Promise<Record<string, string>> {
    if (tracks.length === 0) return {};
    return JSON.parse(
      await ensureNative().enrichSpotifyArtwork(JSON.stringify(tracks), maxBatch)
    ) as Record<string, string>;
  },

  subscribeRemoteCommands(onNext: () => void, onPrev: () => void): () => void {
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
