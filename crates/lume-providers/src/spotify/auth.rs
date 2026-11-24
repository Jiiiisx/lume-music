use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use lume_core::CoreError;
use lume_storage::SpotifyTokenSet;
use rand::Rng;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

pub const SPOTIFY_ACCOUNTS_AUTH_URL: &str = "https://accounts.spotify.com/authorize";
pub const SPOTIFY_ACCOUNTS_TOKEN_URL: &str = "https://accounts.spotify.com/api/token";

/// Personal Developer App Web API redirect URI (strictly `127.0.0.1` per 2025/2026 Spotify OAuth rules).
pub const SPOTIFY_WEB_API_REDIRECT_URI: &str = "http://127.0.0.1:43821/callback";
pub const SPOTIFY_WEB_API_BIND_ADDR: &str = "127.0.0.1:43821";
pub const SPOTIFY_WEB_API_SCOPES: &[&str] = &[
    "user-read-private",
    "playlist-read-private",
    "playlist-read-collaborative",
    "user-library-read",
    "user-library-modify",
];

/// Built-in `librespot` desktop keymaster Client ID required for the `streaming` / AudioKey scope.
pub const SPOTIFY_PLAYBACK_CLIENT_ID: &str = "65b708073fc0480ea92a077233ca87bd";
pub const SPOTIFY_PLAYBACK_REDIRECT_URI: &str = "http://127.0.0.1:8898/login";
pub const SPOTIFY_PLAYBACK_BIND_ADDR: &str = "127.0.0.1:8898";
pub const SPOTIFY_PLAYBACK_SCOPES: &[&str] = &["streaming"];

const OAUTH_TIMEOUT: Duration = Duration::from_secs(180);

/// PKCE verifier + S256 challenge + CSRF state pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PkceChallenge {
    pub code_verifier: String,
    pub code_challenge: String,
    pub state: String,
}

impl PkceChallenge {
    pub fn generate() -> Self {
        let mut verifier_bytes = [0u8; 48];
        let mut state_bytes = [0u8; 24];
        let mut rng = rand::rng();
        rng.fill_bytes(&mut verifier_bytes);
        rng.fill_bytes(&mut state_bytes);

        let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
        let code_challenge = compute_pkce_s256(&code_verifier);
        let state = URL_SAFE_NO_PAD.encode(state_bytes);

        Self {
            code_verifier,
            code_challenge,
            state,
        }
    }
}

/// Computes RFC 7636 `S256` code challenge (`BASE64URL-ENCODE(SHA256(ASCII(verifier)))`).
pub fn compute_pkce_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Builds the Spotify OAuth 2.0 Authorization Code + PKCE URL.
pub fn build_authorize_url(
    client_id: &str,
    redirect_uri: &str,
    scopes: &[&str],
    pkce: &PkceChallenge,
) -> Result<String, CoreError> {
    let mut url = Url::parse(SPOTIFY_ACCOUNTS_AUTH_URL)
        .map_err(|e| CoreError::AuthRequired(format!("invalid authorize URL: {e}")))?;
    url.query_pairs_mut()
        .append_pair("client_id", client_id.trim())
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &pkce.code_challenge)
        .append_pair("state", &pkce.state)
        .append_pair("scope", &scopes.join(" "));
    Ok(url.into())
}

/// Parses an HTTP request line (`GET /callback?code=...&state=... HTTP/1.1`) and validates CSRF state.
pub fn parse_oauth_callback_request(
    request_line: &str,
    expected_state: &str,
) -> Result<String, CoreError> {
    let mut parts = request_line.split_whitespace();
    let _method = parts.next();
    let path_and_query = parts
        .next()
        .ok_or_else(|| CoreError::AuthRequired("malformed OAuth callback HTTP request".into()))?;

    let dummy = Url::parse(&format!("http://127.0.0.1{path_and_query}"))
        .map_err(|e| CoreError::AuthRequired(format!("invalid callback URL: {e}")))?;

    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (k, v) in dummy.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            _ => {}
        }
    }

    if let Some(err) = error {
        return Err(CoreError::AuthRequired(format!("Spotify authorization denied: {err}")));
    }
    match state.as_deref() {
        Some(s) if s == expected_state => {}
        _ => {
            return Err(CoreError::AuthRequired(
                "OAuth callback state mismatch (possible stale login tab)".into(),
            ));
        }
    }
    code.filter(|c| !c.trim().is_empty())
        .ok_or_else(|| CoreError::AuthRequired("OAuth callback missing authorization code".into()))
}

#[derive(Debug, Deserialize)]
struct SpotifyTokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    expires_in: i64,
    #[serde(default)]
    scope: String,
}

fn now_epoch_s() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn open_system_browser(url: &str) -> Result<(), CoreError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| CoreError::AuthRequired(format!("failed to open browser: {e}")))?;
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = url;
        Err(CoreError::AuthRequired(
            "automatic browser launch is only implemented on Windows".into(),
        ))
    }
}

/// Runs a complete Authorization Code + PKCE loopback login flow and returns a `SpotifyTokenSet`.
pub async fn run_pkce_loopback_flow(
    client_id: &str,
    bind_addr: &str,
    redirect_uri: &str,
    scopes: &[&str],
) -> Result<SpotifyTokenSet, CoreError> {
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err(CoreError::AuthRequired(
            "Spotify Client ID is empty. Enter your Spotify Developer Client ID in Settings first."
                .into(),
        ));
    }

    let pkce = PkceChallenge::generate();
    let auth_url = build_authorize_url(client_id, redirect_uri, scopes, &pkce)?;
    let listener = TcpListener::bind(bind_addr).await.map_err(|e| {
        CoreError::AuthRequired(format!(
            "cannot listen on {bind_addr} for Spotify OAuth callback: {e}"
        ))
    })?;

    open_system_browser(&auth_url)?;

    let code = tokio::time::timeout(OAUTH_TIMEOUT, async move {
        loop {
            let (mut socket, _) = listener
                .accept()
                .await
                .map_err(|e| CoreError::AuthRequired(format!("OAuth listener accept failed: {e}")))?;

            let mut buf = [0u8; 4096];
            let n = socket.read(&mut buf).await.unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let first_line = req.lines().next().unwrap_or_default();

            // Ignore favicon probes cleanly and keep waiting for the actual callback.
            if first_line.contains("/favicon.ico") {
                let _ = socket
                    .write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n")
                    .await;
                continue;
            }

            match parse_oauth_callback_request(first_line, &pkce.state) {
                Ok(code) => {
                    let body = "<!doctype html><html><body style=\"font-family:Segoe UI,sans-serif;background:#111318;color:#f4f5f8;display:flex;align-items:center;justify-content:center;height:100vh;margin:0\"><div style=\"text-align:center\"><h2>Connected to Lume Music</h2><p>You can close this browser tab and return to Lume Music.</p></div></body></html>";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.shutdown().await;
                    return Ok(code);
                }
                Err(err) => {
                    let body = format!(
                        "<!doctype html><html><body style=\"font-family:Segoe UI,sans-serif;background:#111318;color:#f4f5f8;padding:40px\"><h2>Spotify Login Failed</h2><p>{err}</p></body></html>"
                    );
                    let resp = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(resp.as_bytes()).await;
                    let _ = socket.shutdown().await;
                    return Err(err);
                }
            }
        }
    })
    .await
    .map_err(|_| {
        CoreError::AuthRequired("Spotify login timed out after 3 minutes. Please try again.".into())
    })??;

    exchange_authorization_code(
        SPOTIFY_ACCOUNTS_TOKEN_URL,
        client_id,
        &code,
        redirect_uri,
        &pkce.code_verifier,
    )
    .await
}

/// Runs the Web API OAuth PKCE flow using the user's personal Developer Client ID.
pub async fn login_spotify_web_api(client_id: &str) -> Result<SpotifyTokenSet, CoreError> {
    run_pkce_loopback_flow(
        client_id,
        SPOTIFY_WEB_API_BIND_ADDR,
        SPOTIFY_WEB_API_REDIRECT_URI,
        SPOTIFY_WEB_API_SCOPES,
    )
    .await
}

/// Runs the `librespot` playback OAuth PKCE flow using the built-in keymaster Client ID.
pub async fn login_spotify_playback() -> Result<SpotifyTokenSet, CoreError> {
    run_pkce_loopback_flow(
        SPOTIFY_PLAYBACK_CLIENT_ID,
        SPOTIFY_PLAYBACK_BIND_ADDR,
        SPOTIFY_PLAYBACK_REDIRECT_URI,
        SPOTIFY_PLAYBACK_SCOPES,
    )
    .await
}

pub async fn exchange_authorization_code(
    token_url: &str,
    client_id: &str,
    code: &str,
    redirect_uri: &str,
    code_verifier: &str,
) -> Result<SpotifyTokenSet, CoreError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| CoreError::Network(e.to_string()))?;

    let body_encoded: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("client_id", client_id)
        .append_pair("code", code)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("code_verifier", code_verifier)
        .finish();

    let resp = client
        .post(token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body_encoded)
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("token exchange failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(CoreError::AuthRequired(format!(
            "Spotify token exchange rejected ({status}): {body}"
        )));
    }

    let parsed: SpotifyTokenResponse = resp
        .json()
        .await
        .map_err(|e| CoreError::AuthRequired(format!("invalid token response JSON: {e}")))?;

    let refresh = parsed.refresh_token.unwrap_or_default();
    let scopes = parsed
        .scope
        .split_whitespace()
        .map(ToString::to_string)
        .collect();

    Ok(SpotifyTokenSet {
        access_token: parsed.access_token,
        refresh_token: refresh,
        expires_at_epoch_s: now_epoch_s() + parsed.expires_in.max(60),
        scopes,
    })
}

/// Refreshes an expired Spotify PKCE access token using its refresh token.
pub async fn refresh_spotify_token(
    client_id: &str,
    current: &SpotifyTokenSet,
) -> Result<SpotifyTokenSet, CoreError> {
    refresh_spotify_token_at(SPOTIFY_ACCOUNTS_TOKEN_URL, client_id, current).await
}

pub async fn refresh_spotify_token_at(
    token_url: &str,
    client_id: &str,
    current: &SpotifyTokenSet,
) -> Result<SpotifyTokenSet, CoreError> {
    if current.refresh_token.trim().is_empty() {
        return Err(CoreError::AuthRequired(
            "Spotify refresh token is missing; please sign in again.".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| CoreError::Network(e.to_string()))?;

    let body_encoded: String = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("client_id", client_id.trim())
        .append_pair("refresh_token", current.refresh_token.as_str())
        .finish();

    let resp = client
        .post(token_url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body_encoded)
        .send()
        .await
        .map_err(|e| CoreError::Network(format!("token refresh failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(CoreError::AuthRequired(format!(
            "Spotify token refresh rejected ({status}): {body}"
        )));
    }

    let parsed: SpotifyTokenResponse = resp
        .json()
        .await
        .map_err(|e| CoreError::AuthRequired(format!("invalid refresh response JSON: {e}")))?;

    let scopes = if parsed.scope.trim().is_empty() {
        current.scopes.clone()
    } else {
        parsed
            .scope
            .split_whitespace()
            .map(ToString::to_string)
            .collect()
    };

    Ok(SpotifyTokenSet {
        access_token: parsed.access_token,
        refresh_token: parsed
            .refresh_token
            .filter(|r| !r.trim().is_empty())
            .unwrap_or_else(|| current.refresh_token.clone()),
        expires_at_epoch_s: now_epoch_s() + parsed.expires_in.max(60),
        scopes,
    })
}

/// Ensures a `SpotifyTokenSet` is fresh, refreshing it if within 90s of expiry.
/// Returns `(token_set, was_refreshed)`.
pub async fn ensure_fresh_token(
    client_id: &str,
    current: SpotifyTokenSet,
) -> Result<(SpotifyTokenSet, bool), CoreError> {
    if !current.is_expired(now_epoch_s(), 90) {
        return Ok((current, false));
    }
    let refreshed = refresh_spotify_token(client_id, &current).await?;
    Ok((refreshed, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256_matches_rfc7636_test_vector() {
        // RFC 7636 Appendix B test vector.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            compute_pkce_s256(verifier),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_uses_loopback_ip_never_localhost_and_includes_pkce() {
        let pkce = PkceChallenge {
            code_verifier: "v".into(),
            code_challenge: "chal123".into(),
            state: "st456".into(),
        };
        let url = build_authorize_url(
            "my_client_id",
            SPOTIFY_WEB_API_REDIRECT_URI,
            SPOTIFY_WEB_API_SCOPES,
            &pkce,
        )
        .unwrap();

        assert!(url.contains("client_id=my_client_id"));
        assert!(url.contains("127.0.0.1%3A43821%2Fcallback"));
        assert!(!url.contains("localhost"));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("code_challenge=chal123"));
        assert!(url.contains("state=st456"));
    }

    #[test]
    fn callback_parser_extracts_code_and_rejects_state_mismatch_or_error() {
        let ok = parse_oauth_callback_request(
            "GET /callback?code=AUTH_CODE_99&state=expected_state HTTP/1.1",
            "expected_state",
        )
        .unwrap();
        assert_eq!(ok, "AUTH_CODE_99");

        let bad_state = parse_oauth_callback_request(
            "GET /callback?code=AUTH_CODE_99&state=wrong HTTP/1.1",
            "expected_state",
        );
        assert!(bad_state.is_err());

        let denied = parse_oauth_callback_request(
            "GET /callback?error=access_denied&state=expected_state HTTP/1.1",
            "expected_state",
        );
        assert!(denied.is_err());
    }
}
