use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::StorageError;

/// OAuth 2.0 token pair for a single Spotify session (Web API or `librespot` playback).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotifyTokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_epoch_s: i64,
    #[serde(default)]
    pub scopes: Vec<String>,
}

impl SpotifyTokenSet {
    /// Returns true if the access token is expired or within `leeway_s` seconds of expiring.
    pub fn is_expired(&self, now_epoch_s: i64, leeway_s: i64) -> bool {
        self.access_token.trim().is_empty() || now_epoch_s + leeway_s >= self.expires_at_epoch_s
    }
}

/// Encrypted bundle storing the two cleanly separated Spotify OAuth sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpotifyTokenBundle {
    pub web_api: Option<SpotifyTokenSet>,
    pub playback: Option<SpotifyTokenSet>,
}

/// Windows DPAPI-backed token store (`%LOCALAPPDATA%\LumeMusic\tokens.dpapi`).
#[derive(Debug, Clone)]
pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn default_token_path() -> PathBuf {
        let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("LumeMusic").join("tokens.dpapi")
    }

    pub fn open_default() -> Self {
        Self::new(Self::default_token_path())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<SpotifyTokenBundle, StorageError> {
        if !self.path.exists() {
            return Ok(SpotifyTokenBundle::default());
        }
        let protected = std::fs::read(&self.path)?;
        if protected.is_empty() {
            return Ok(SpotifyTokenBundle::default());
        }
        let plain = unprotect_bytes(&protected)?;
        serde_json::from_slice(&plain).map_err(|e| StorageError::Serialization(e.to_string()))
    }

    pub fn save(&self, bundle: &SpotifyTokenBundle) -> Result<(), StorageError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let plain =
            serde_json::to_vec(bundle).map_err(|e| StorageError::Serialization(e.to_string()))?;
        let protected = protect_bytes(&plain)?;

        let tmp = self.path.with_extension("dpapi.tmp");
        std::fs::write(&tmp, &protected)?;
        if let Err(err) = std::fs::rename(&tmp, &self.path) {
            // Windows rename can fail if target exists; fall back to direct overwrite.
            let _ = std::fs::remove_file(&tmp);
            std::fs::write(&self.path, &protected).map_err(|_| StorageError::Io(err))?;
        }
        Ok(())
    }

    pub fn save_web_api(&self, token: Option<SpotifyTokenSet>) -> Result<(), StorageError> {
        let mut bundle = self.load().unwrap_or_default();
        bundle.web_api = token;
        self.save(&bundle)
    }

    pub fn save_playback(&self, token: Option<SpotifyTokenSet>) -> Result<(), StorageError> {
        let mut bundle = self.load().unwrap_or_default();
        bundle.playback = token;
        self.save(&bundle)
    }

    pub fn clear_all(&self) -> Result<(), StorageError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn protect_bytes(plain: &[u8]) -> Result<Vec<u8>, StorageError> {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    if plain.len() > u32::MAX as usize {
        return Err(StorageError::Crypto("payload exceeds DPAPI size limit".into()));
    }

    let input = CRYPT_INTEGER_BLOB {
        cbData: plain.len() as u32,
        pbData: plain.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: `input` points to valid immutable slice for the duration of the Win32 call.
    // On success, `output.pbData` is allocated by DPAPI and freed via `LocalFree`.
    let ok = unsafe {
        CryptProtectData(
            &input,
            ptr::null(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(StorageError::Crypto(format!(
            "CryptProtectData failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    let bytes = if output.cbData == 0 || output.pbData.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() }
    };
    if !output.pbData.is_null() {
        unsafe {
            LocalFree(output.pbData as _);
        }
    }
    Ok(bytes)
}

#[cfg(target_os = "windows")]
fn unprotect_bytes(protected: &[u8]) -> Result<Vec<u8>, StorageError> {
    use std::ptr;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    if protected.len() > u32::MAX as usize {
        return Err(StorageError::Crypto("ciphertext exceeds DPAPI size limit".into()));
    }

    let input = CRYPT_INTEGER_BLOB {
        cbData: protected.len() as u32,
        pbData: protected.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: ptr::null_mut(),
    };

    // SAFETY: `input` points to valid immutable slice for the duration of the Win32 call.
    // On success, `output.pbData` is allocated by DPAPI and freed via `LocalFree`.
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
            ptr::null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
    };
    if ok == 0 {
        return Err(StorageError::Crypto(format!(
            "CryptUnprotectData failed: {}",
            std::io::Error::last_os_error()
        )));
    }

    let bytes = if output.cbData == 0 || output.pbData.is_null() {
        Vec::new()
    } else {
        unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec() }
    };
    if !output.pbData.is_null() {
        unsafe {
            LocalFree(output.pbData as _);
        }
    }
    Ok(bytes)
}

#[cfg(not(target_os = "windows"))]
fn protect_bytes(plain: &[u8]) -> Result<Vec<u8>, StorageError> {
    Ok(plain.iter().map(|b| b ^ 0xA5).collect())
}

#[cfg(not(target_os = "windows"))]
fn unprotect_bytes(protected: &[u8]) -> Result<Vec<u8>, StorageError> {
    Ok(protected.iter().map(|b| b ^ 0xA5).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn dpapi_token_store_round_trips_and_never_writes_plaintext() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("tokens.dpapi");
        let store = TokenStore::new(path.clone());

        assert_eq!(store.load().unwrap(), SpotifyTokenBundle::default());

        let web = SpotifyTokenSet {
            access_token: "super-secret-web-access-token".into(),
            refresh_token: "super-secret-web-refresh-token".into(),
            expires_at_epoch_s: 2_000_000_000,
            scopes: vec!["user-library-read".into(), "playlist-read-private".into()],
        };
        let playback = SpotifyTokenSet {
            access_token: "super-secret-librespot-access".into(),
            refresh_token: "super-secret-librespot-refresh".into(),
            expires_at_epoch_s: 2_000_003_600,
            scopes: vec!["streaming".into()],
        };

        store.save_web_api(Some(web.clone())).unwrap();
        store.save_playback(Some(playback.clone())).unwrap();

        let raw_on_disk = std::fs::read(&path).unwrap();
        let raw_lossy = String::from_utf8_lossy(&raw_on_disk);
        assert!(
            !raw_lossy.contains("super-secret-web-access-token"),
            "ciphertext on disk must never contain plaintext access token"
        );
        assert!(
            !raw_lossy.contains("super-secret-librespot-refresh"),
            "ciphertext on disk must never contain plaintext refresh token"
        );

        let loaded = store.load().unwrap();
        assert_eq!(loaded.web_api, Some(web));
        assert_eq!(loaded.playback, Some(playback));

        store.save_web_api(None).unwrap();
        let after_clear = store.load().unwrap();
        assert!(after_clear.web_api.is_none());
        assert!(after_clear.playback.is_some());
    }

    #[test]
    fn token_expiry_respects_leeway() {
        let token = SpotifyTokenSet {
            access_token: "abc".into(),
            refresh_token: "def".into(),
            expires_at_epoch_s: 1_000,
            scopes: vec![],
        };
        assert!(!token.is_expired(900, 60));
        assert!(token.is_expired(940, 60));
        assert!(token.is_expired(1_001, 0));
    }
}
