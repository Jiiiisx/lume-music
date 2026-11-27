import * as SecureStore from 'expo-secure-store';
import * as WebBrowser from 'expo-web-browser';
import { LumeCore } from '../../modules/lume-core';
import type { SpotifySyncSummaryDto, SpotifyTokenSet } from '../types';

const KEY_CLIENT_ID = 'lume_spotify_client_id';
const KEY_TOKEN_SET = 'lume_spotify_token_set';

export async function loadSpotifyClientId(): Promise<string> {
  return (await SecureStore.getItemAsync(KEY_CLIENT_ID)) ?? '';
}

export async function saveSpotifyClientId(clientId: string): Promise<void> {
  const trimmed = clientId.trim();
  if (!trimmed) {
    await SecureStore.deleteItemAsync(KEY_CLIENT_ID);
  } else {
    await SecureStore.setItemAsync(KEY_CLIENT_ID, trimmed);
  }
}

export async function loadSpotifyTokenSet(): Promise<SpotifyTokenSet | null> {
  const raw = await SecureStore.getItemAsync(KEY_TOKEN_SET);
  if (!raw) return null;
  try {
    return JSON.parse(raw) as SpotifyTokenSet;
  } catch {
    return null;
  }
}

export async function clearSpotifySession(): Promise<void> {
  await SecureStore.deleteItemAsync(KEY_TOKEN_SET);
}

/**
 * Runs Spotify OAuth 2.0 PKCE via native iOS ASWebAuthenticationSession (`lumemusic://callback`)
 * and stores the resulting token set securely inside iOS Keychain.
 */
export async function connectSpotifyMobile(clientId: string): Promise<SpotifyTokenSet> {
  const trimmed = clientId.trim();
  if (!trimmed) {
    throw new Error('Masukkan Spotify Client ID terlebih dahulu.');
  }
  await saveSpotifyClientId(trimmed);

  const started = await LumeCore.beginSpotifyMobileAuth(trimmed);
  const authResult = await WebBrowser.openAuthSessionAsync(
    started.authorize_url,
    started.redirect_uri
  );

  if (authResult.type !== 'success' || !authResult.url) {
    throw new Error('Login Spotify dibatalkan.');
  }

  const parsed = new URL(authResult.url);
  const err = parsed.searchParams.get('error');
  if (err) {
    throw new Error(`Spotify menolak otorisasi: ${err}`);
  }
  const returnedState = parsed.searchParams.get('state');
  if (returnedState !== started.state) {
    throw new Error('OAuth state tidak cocok. Coba login ulang.');
  }
  const code = parsed.searchParams.get('code');
  if (!code) {
    throw new Error('Kode otorisasi Spotify tidak ditemukan pada callback.');
  }

  const tokenSet = await LumeCore.exchangeSpotifyMobileCode(
    trimmed,
    code,
    started.code_verifier
  );
  await SecureStore.setItemAsync(KEY_TOKEN_SET, JSON.stringify(tokenSet));
  return tokenSet;
}

export async function ensureFreshSpotifyToken(): Promise<SpotifyTokenSet> {
  const clientId = await loadSpotifyClientId();
  const current = await loadSpotifyTokenSet();
  if (!clientId || !current) {
    throw new Error('Spotify belum terhubung. Hubungkan dulu di tab Library.');
  }

  const nowSec = Math.floor(Date.now() / 1000);
  if (nowSec + 90 < current.expires_at_epoch_s) {
    return current;
  }

  const refreshed = await LumeCore.refreshSpotifyToken(clientId, current);
  await SecureStore.setItemAsync(KEY_TOKEN_SET, JSON.stringify(refreshed));
  return refreshed;
}

export async function syncSpotifyNow(): Promise<SpotifySyncSummaryDto> {
  const token = await ensureFreshSpotifyToken();
  return LumeCore.syncSpotifyLibrary(token.access_token);
}
