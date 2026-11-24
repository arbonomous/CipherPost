use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use url::Url;

use crate::error::{CipherError, CipherResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GmailOAuthResult {
    pub email: String,
    pub display_name: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_in: i64,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    email: String,
    name: Option<String>,
}

fn generate_pkce_pair() -> (String, String) {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let verifier = URL_SAFE_NO_PAD.encode(bytes);
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());
    (verifier, challenge)
}

fn generate_state() -> String {
    let mut bytes = [0u8; 16];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Runs a zero-trust local loopback (`127.0.0.1:<ephemeral_port>`) OAuth2 PKCE flow:
/// 1. Binds a temporary localhost TCP listener.
/// 2. Opens the system browser to Google's official OAuth2 consent screen.
/// 3. Waits for the local redirect with the authorization `code`, exchanges it directly
///    with `oauth2.googleapis.com` using PKCE `code_verifier`, and returns the Gmail
///    address + OAuth2 tokens.
pub async fn run_gmail_oauth_pkce<F>(
    client_id: &str,
    client_secret: Option<&str>,
    open_browser: F,
) -> CipherResult<GmailOAuthResult>
where
    F: FnOnce(&str) -> CipherResult<()>,
{
    let client_id = client_id.trim();
    if client_id.is_empty() {
        return Err(CipherError::Validation(
            "A Google OAuth Desktop Client ID is required for Sign in with Google.".to_string(),
        ));
    }

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| CipherError::Network(format!("Failed binding OAuth loopback port: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| CipherError::Network(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");

    let (verifier, challenge) = generate_pkce_pair();
    let expected_state = generate_state();

    let mut auth_url = Url::parse("https://accounts.google.com/o/oauth2/v2/auth")
        .map_err(|e| CipherError::Validation(e.to_string()))?;
    auth_url
        .query_pairs_mut()
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair(
            "scope",
            "https://mail.google.com/ openid email profile",
        )
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &expected_state)
        .append_pair("access_type", "offline")
        .append_pair("prompt", "consent");

    open_browser(auth_url.as_str())?;

    // Wait up to 3 minutes for the user to complete Google Sign-In in their browser
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(180), listener.accept())
        .await
        .map_err(|_| CipherError::Network("Google sign-in timed out after 3 minutes".to_string()))?
        .map_err(|e| CipherError::Network(format!("Loopback accept error: {e}")))?;

    let mut buf = [0u8; 4096];
    let n = socket
        .read(&mut buf)
        .await
        .map_err(|e| CipherError::Network(e.to_string()))?;
    let req_str = String::from_utf8_lossy(&buf[..n]);
    let request_path = req_str
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    let parsed_cb = Url::parse(&format!("http://127.0.0.1:{port}{request_path}"))
        .map_err(|e| CipherError::Network(format!("Invalid OAuth callback URL: {e}")))?;

    let mut code: Option<String> = None;
    let mut state: Option<String> = None;
    let mut oauth_err: Option<String> = None;

    for (k, v) in parsed_cb.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => oauth_err = Some(v.into_owned()),
            _ => {}
        }
    }

    let html_body = if oauth_err.is_some() || code.is_none() {
        "<html><body style='background:#090a0d;color:#edf0f5;font-family:-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0'><div style='text-align:center'><h2>Sign-in cancelled or failed</h2><p style='color:#7c8599'>You can close this tab and return to CipherPost.</p></div></body></html>"
    } else {
        "<html><body style='background:#090a0d;color:#edf0f5;font-family:-apple-system,sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0'><div style='text-align:center'><h2 style='color:#34d399'>✓ Connected to CipherPost</h2><p style='color:#7c8599'>You can close this tab and return to CipherPost.</p></div></body></html>"
    };

    let http_resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        html_body.len(),
        html_body
    );
    let _ = socket.write_all(http_resp.as_bytes()).await;
    let _ = socket.shutdown().await;

    if let Some(err) = oauth_err {
        return Err(CipherError::Network(format!("Google sign-in denied: {err}")));
    }
    if state.as_deref() != Some(expected_state.as_str()) {
        return Err(CipherError::Validation(
            "OAuth2 state mismatch — aborted for security".to_string(),
        ));
    }
    let auth_code = code.ok_or_else(|| {
        CipherError::Network("Missing authorization code in Google OAuth callback".to_string())
    })?;

    // Exchange authorization code directly with Google
    let http = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| CipherError::Network(e.to_string()))?;

    let mut params = vec![
        ("client_id", client_id.to_string()),
        ("code", auth_code),
        ("code_verifier", verifier),
        ("grant_type", "authorization_code".to_string()),
        ("redirect_uri", redirect_uri),
    ];
    if let Some(sec) = client_secret.filter(|s| !s.trim().is_empty()) {
        params.push(("client_secret", sec.trim().to_string()));
    }

    let token_resp = http
        .post("https://oauth2.googleapis.com/token")
        .form(&params)
        .send()
        .await
        .map_err(|e| CipherError::Network(format!("Token exchange failed: {e}")))?;

    if !token_resp.status().is_success() {
        let err_text = token_resp.text().await.unwrap_or_default();
        return Err(CipherError::Network(format!(
            "Google token exchange rejected: {err_text}"
        )));
    }

    let tokens: TokenResponse = token_resp
        .json()
        .await
        .map_err(|e| CipherError::Network(format!("Invalid token JSON: {e}")))?;

    let user_resp = http
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|e| CipherError::Network(format!("Failed reading Google profile: {e}")))?;

    let user: UserInfoResponse = user_resp
        .json()
        .await
        .map_err(|e| CipherError::Network(format!("Invalid profile JSON: {e}")))?;

    Ok(GmailOAuthResult {
        display_name: user.name.unwrap_or_else(|| user.email.clone()),
        email: user.email,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        expires_in: tokens.expires_in.unwrap_or(3600),
    })
}
