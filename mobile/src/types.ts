export type TrackSource =
  | { Spotify: { id: string } }
  | { YouTubeMusic: { video_id: string } }
  | { Local: { path: string } };

export interface Track {
  uid: string;
  source: TrackSource;
  title: string;
  artists: string[];
  album: string | null;
  duration_ms: number;
  artwork_ref: string | null;
  is_liked: boolean;
}

export type PlaylistOrigin =
  | 'SpotifySynced'
  | { SpotifyImported: { source_url: string } }
  | { YouTubeImported: { source_url: string } }
  | 'LocalCustom';

export interface Playlist {
  id: string;
  name: string;
  origin: PlaylistOrigin;
  track_count: number;
}

export interface ArtistSummary {
  id: string;
  source: 'Spotify' | 'YouTubeMusic';
  name: string;
  avatar_url: string | null;
  subtitle: string | null;
}

export interface AlbumSummary {
  id: string;
  source: 'Spotify' | 'YouTubeMusic';
  title: string;
  artist_name: string;
  year: string | null;
  album_type: string | null;
  cover_url: string | null;
}

export interface RemotePlaylistSummary {
  id: string;
  source: 'Spotify' | 'YouTubeMusic';
  name: string;
  owner: string | null;
  track_count: number | null;
  cover_url: string | null;
}

export interface ArtistProfile {
  id: string;
  source: 'Spotify' | 'YouTubeMusic';
  name: string;
  avatar_url: string | null;
  description: string | null;
  subscriber_label: string | null;
  top_tracks: Track[];
  albums: AlbumSummary[];
  singles: AlbumSummary[];
  playlists: RemotePlaylistSummary[];
}

export interface AlbumDetail {
  id: string;
  source: 'Spotify' | 'YouTubeMusic';
  title: string;
  artist_name: string;
  artist_id: string | null;
  year: string | null;
  cover_url: string | null;
  tracks: Track[];
}

export interface RemotePlaylistDetail {
  id: string;
  source: 'Spotify' | 'YouTubeMusic';
  name: string;
  owner: string | null;
  cover_url: string | null;
  source_url: string;
  tracks: Track[];
  is_truncated_embed: boolean;
}

export interface YtmCatalogSearchResult {
  tracks: Track[];
  artists: ArtistSummary[];
  playlists: RemotePlaylistSummary[];
}

export type SpotifyCatalogSearchResult = YtmCatalogSearchResult;

export interface LyricsWord {
  start_ms: number;
  end_ms: number;
  text: string;
  is_part_of_word?: boolean;
  is_background?: boolean;
}

export interface LyricsLine {
  time_ms: number;
  end_ms: number | null;
  text: string;
  words: LyricsWord[] | null;
  opposite_aligned?: boolean;
  is_background?: boolean;
}

export interface LyricsData {
  synced: LyricsLine[];
  plain: string | null;
  provider: string;
}

export type PlaybackSourceUsed =
  | 'SpotifyNative'
  | 'YouTubeFallback'
  | 'YouTubeDirect'
  | 'LocalFile';

export interface PlayableStreamDto {
  uri: string;
  is_offline: boolean;
  source_used: PlaybackSourceUsed;
  resolved_video_id: string | null;
}

export interface OfflineTrackRecord {
  track_uid: string;
  file_path: string;
  file_size_bytes: number;
  mime_type: string;
  downloaded_at: number;
}

export interface OfflineEntryDto {
  track: Track;
  record: OfflineTrackRecord;
}

export interface SpotifyTokenSet {
  access_token: string;
  refresh_token: string;
  expires_at_epoch_s: number;
  scopes: string[];
}

export interface MobileSpotifyAuthStartDto {
  authorize_url: string;
  code_verifier: string;
  state: string;
  redirect_uri: string;
}

export interface SpotifySyncSummaryDto {
  display_name: string;
  liked_count: number;
  playlist_count: number;
}

export interface HealthCheckDto {
  version: string;
  target_os: string;
  db_path: string;
  offline_dir: string;
  offline_track_count: number;
  offline_storage_bytes: number;
}

export interface DynamicPaletteDto {
  bg_top: string;
  bg_mid: string;
  bg_bottom: string;
  accent: string;
  accent_soft: string;
}

export const DEFAULT_PALETTE: DynamicPaletteDto = {
  bg_top: '#181C2E',
  bg_mid: '#0E111B',
  bg_bottom: '#0A0C12',
  accent: '#818CF8',
  accent_soft: 'rgba(99, 102, 241, 0.22)',
};

export type RepeatMode = 'off' | 'all' | 'one';

export function trackArtistDisplay(track: Track): string {
  return track.artists.length > 0 ? track.artists.join(', ') : 'Unknown Artist';
}

export function trackSourceBadge(track: Track): 'SP' | 'YTM' | 'LOCAL' {
  if ('Spotify' in track.source) return 'SP';
  if ('YouTubeMusic' in track.source) return 'YTM';
  return 'LOCAL';
}

export function playlistOriginBadge(origin: PlaylistOrigin): string {
  if (origin === 'SpotifySynced') return 'SP';
  if (origin === 'LocalCustom') return 'LOCAL';
  if ('SpotifyImported' in origin) return 'IMP';
  return 'YTM';
}

export function isPlaylistEditable(origin: PlaylistOrigin): boolean {
  return origin !== 'SpotifySynced';
}

export function formatDurationMs(ms: number): string {
  const totalSec = Math.max(0, Math.floor(ms / 1000));
  const m = Math.floor(totalSec / 60);
  const s = totalSec % 60;
  return `${m}:${s.toString().padStart(2, '0')}`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}
