use std::sync::Arc;
use std::time::{Duration, Instant};

use async_imap::Authenticator;
use futures::StreamExt;
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConfig, DigitallySignedStruct, RootCertStore, SignatureScheme};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tracing::{error, info, warn};
use zeroize::Zeroizing;

use crate::db::Database;
use crate::error::{CipherError, CipherResult};
use crate::mail::mime::parse_and_sanitize_raw_message;
use crate::models::{
    Account, AuthType, ConnectionTestResult, NewAccountInput, SyncEventPayload, TlsHandshakeStep,
    TlsMode,
};
use crate::security::keychain;

type ImapTlsStream = tokio_rustls::client::TlsStream<TcpStream>;

/// SASL XOAUTH2 authenticator for Gmail OAuth2 direct IMAP connections.
struct XOAuth2Auth {
    user: String,
    access_token: Zeroizing<String>,
}

impl Authenticator for XOAuth2Auth {
    type Response = String;

    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        format!(
            "user={}\x01auth=Bearer {}\x01\x01",
            self.user,
            self.access_token.as_str()
        )
    }
}

/// Localhost-only certificate verifier strictly scoped to `127.0.0.1` / `localhost`
/// so Proton Mail Bridge's self-signed local TLS certificate works out of the box
/// without relaxing TLS verification for any external internet host.
#[derive(Debug)]
struct LoopbackBridgeVerifier;

impl ServerCertVerifier for LoopbackBridgeVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &CertificateDer<'_>,
        _dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        Ok(HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        vec![
            SignatureScheme::RSA_PKCS1_SHA256,
            SignatureScheme::ECDSA_NISTP256_SHA256,
            SignatureScheme::RSA_PSS_SHA256,
            SignatureScheme::ED25519,
            SignatureScheme::RSA_PKCS1_SHA384,
            SignatureScheme::ECDSA_NISTP384_SHA384,
            SignatureScheme::RSA_PSS_SHA384,
            SignatureScheme::RSA_PSS_SHA512,
        ]
    }
}

fn is_loopback_host(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

fn build_tls_connector(host: &str) -> CipherResult<TlsConnector> {
    let config = if is_loopback_host(host) {
        // Proton Bridge binds locally on 127.0.0.1:1143 with a local self-signed cert
        ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(LoopbackBridgeVerifier))
            .with_no_client_auth()
    } else {
        let mut root_store = RootCertStore::empty();
        root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth()
    };
    Ok(TlsConnector::from(Arc::new(config)))
}

async fn connect_imap_tls(
    host: &str,
    port: u16,
    tls_mode: &TlsMode,
) -> CipherResult<(ImapTlsStream, String)> {
    let addr = format!("{host}:{port}");
    let mut tcp = tokio::time::timeout(Duration::from_secs(12), TcpStream::connect(&addr))
        .await
        .map_err(|_| CipherError::Network(format!("TCP connection timed out to {addr}")))?
        .map_err(|e| CipherError::Network(format!("TCP connect failed to {addr}: {e}")))?;

    if matches!(tls_mode, TlsMode::Starttls) {
        // Perform plaintext IMAP greeting + STARTTLS upgrade before TLS handshake
        let mut reader = BufReader::new(&mut tcp);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| CipherError::Imap(format!("Failed reading IMAP greeting: {e}")))?;
        reader
            .get_mut()
            .write_all(b"CP01 STARTTLS\r\n")
            .await
            .map_err(|e| CipherError::Imap(format!("Failed sending STARTTLS: {e}")))?;
        line.clear();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| CipherError::Imap(format!("Failed reading STARTTLS reply: {e}")))?;
        if !line.to_ascii_uppercase().contains("OK") {
            return Err(CipherError::Imap(format!(
                "Server rejected STARTTLS upgrade: {}",
                line.trim()
            )));
        }
    }

    let connector = build_tls_connector(host)?;
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|e| CipherError::Network(format!("Invalid TLS SNI hostname '{host}': {e}")))?;

    let tls_stream = tokio::time::timeout(
        Duration::from_secs(12),
        connector.connect(server_name, tcp),
    )
    .await
    .map_err(|_| CipherError::Network("TLS handshake timed out".to_string()))?
    .map_err(|e| CipherError::Network(format!("TLS handshake failed: {e}")))?;

    let tls_info = {
        let (_, conn) = tls_stream.get_ref();
        let ver = conn
            .protocol_version()
            .map(|v| format!("{v:?}"))
            .unwrap_or_else(|| "TLS".to_string());
        let cipher = conn
            .negotiated_cipher_suite()
            .map(|c| format!("{:?}", c.suite()))
            .unwrap_or_else(|| "Negotiated".to_string());
        format!("{ver} ({cipher})")
    };

    Ok((tls_stream, tls_info))
}

async fn authenticate_imap(
    client: async_imap::Client<ImapTlsStream>,
    email: &str,
    auth_type: &AuthType,
    secret: Zeroizing<String>,
) -> CipherResult<async_imap::Session<ImapTlsStream>> {
    match auth_type {
        AuthType::Oauth2 => {
            let auth = XOAuth2Auth {
                user: email.to_string(),
                access_token: secret,
            };
            client
                .authenticate("XOAUTH2", auth)
                .await
                .map_err(|(e, _)| CipherError::Imap(format!("XOAUTH2 auth failed: {e}")))
        }
        AuthType::Password => client
            .login(email, secret.as_str())
            .await
            .map_err(|(e, _)| CipherError::Imap(format!("IMAP LOGIN failed: {e}"))),
    }
}

/// Runs a live, step-by-step TCP + TLS Handshake + IMAP Capability + Auth diagnostic test
/// (plus SMTP check) for the Account Setup onboarding modal.
pub async fn test_account_connection(input: &NewAccountInput) -> ConnectionTestResult {
    let mut steps = Vec::new();
    let mut imap_tls_version = None;
    let mut imap_capabilities = Vec::new();

    // Step 1 & 2: IMAP TCP + TLS Handshake
    let t0 = Instant::now();
    let tls_res = connect_imap_tls(&input.imap_host, input.imap_port, &input.imap_tls).await;
    match tls_res {
        Ok((stream, tls_info)) => {
            imap_tls_version = Some(tls_info.clone());
            steps.push(TlsHandshakeStep {
                step: "IMAP TLS Handshake".to_string(),
                status: "passed".to_string(),
                detail: format!("{}:{} — {tls_info}", input.imap_host, input.imap_port),
                latency_ms: t0.elapsed().as_millis() as u64,
            });

            // Step 3: IMAP Greeting & Capabilities
            let t1 = Instant::now();
            let mut client = async_imap::Client::new(stream);
            match client.read_response().await {
                Some(Ok(_)) => {}
                Some(Err(e)) => {
                    steps.push(TlsHandshakeStep {
                        step: "IMAP Greeting".to_string(),
                        status: "failed".to_string(),
                        detail: format!("Greeting error: {e}"),
                        latency_ms: t1.elapsed().as_millis() as u64,
                    });
                    return ConnectionTestResult {
                        success: false,
                        imap_tls_version,
                        imap_capabilities,
                        smtp_verified: false,
                        steps,
                        error: Some(format!("IMAP greeting failed: {e}")),
                    };
                }
                None => {
                    steps.push(TlsHandshakeStep {
                        step: "IMAP Greeting".to_string(),
                        status: "failed".to_string(),
                        detail: "Server closed connection before greeting".to_string(),
                        latency_ms: t1.elapsed().as_millis() as u64,
                    });
                    return ConnectionTestResult {
                        success: false,
                        imap_tls_version,
                        imap_capabilities,
                        smtp_verified: false,
                        steps,
                        error: Some("IMAP server closed connection before greeting".to_string()),
                    };
                }
            }

            // Step 4: IMAP Authentication
            let t2 = Instant::now();
            match authenticate_imap(
                client,
                &input.email,
                &input.auth_type,
                Zeroizing::new(input.secret.clone()),
            )
            .await
            {
                Ok(mut session) => {
                    if let Ok(caps) = session.capabilities().await {
                        for cap in caps.iter() {
                            imap_capabilities.push(format!("{cap:?}"));
                        }
                    }
                    let has_idle = imap_capabilities
                        .iter()
                        .any(|c| c.to_ascii_uppercase().contains("IDLE"));
                    steps.push(TlsHandshakeStep {
                        step: "IMAP Auth & Capabilities".to_string(),
                        status: "passed".to_string(),
                        detail: format!(
                            "Authenticated ({}) · IDLE push: {}",
                            input.auth_type.as_str(),
                            if has_idle { "supported" } else { "polling fallback" }
                        ),
                        latency_ms: t2.elapsed().as_millis() as u64,
                    });

                    // Verify INBOX is selectable
                    let t3 = Instant::now();
                    match session.select("INBOX").await {
                        Ok(mailbox) => {
                            steps.push(TlsHandshakeStep {
                                step: "INBOX Access".to_string(),
                                status: "passed".to_string(),
                                detail: format!(
                                    "INBOX ready ({} messages, UIDVALIDITY={:?})",
                                    mailbox.exists, mailbox.uid_validity
                                ),
                                latency_ms: t3.elapsed().as_millis() as u64,
                            });
                        }
                        Err(e) => {
                            steps.push(TlsHandshakeStep {
                                step: "INBOX Access".to_string(),
                                status: "failed".to_string(),
                                detail: format!("SELECT INBOX failed: {e}"),
                                latency_ms: t3.elapsed().as_millis() as u64,
                            });
                        }
                    }

                    let _ = session.logout().await;
                }
                Err(e) => {
                    steps.push(TlsHandshakeStep {
                        step: "IMAP Authentication".to_string(),
                        status: "failed".to_string(),
                        detail: e.to_string(),
                        latency_ms: t2.elapsed().as_millis() as u64,
                    });
                    return ConnectionTestResult {
                        success: false,
                        imap_tls_version,
                        imap_capabilities,
                        smtp_verified: false,
                        steps,
                        error: Some(e.to_string()),
                    };
                }
            }
        }
        Err(e) => {
            steps.push(TlsHandshakeStep {
                step: "IMAP TLS Handshake".to_string(),
                status: "failed".to_string(),
                detail: e.to_string(),
                latency_ms: t0.elapsed().as_millis() as u64,
            });
            return ConnectionTestResult {
                success: false,
                imap_tls_version,
                imap_capabilities,
                smtp_verified: false,
                steps,
                error: Some(e.to_string()),
            };
        }
    }

    // Step 5: Live SMTP TLS & Auth Check
    let t_smtp = Instant::now();
    let smtp_res = crate::mail::smtp_client::test_smtp_connection(input).await;
    let smtp_verified = match smtp_res {
        Ok(detail) => {
            steps.push(TlsHandshakeStep {
                step: "SMTP Delivery & TLS".to_string(),
                status: "passed".to_string(),
                detail,
                latency_ms: t_smtp.elapsed().as_millis() as u64,
            });
            true
        }
        Err(e) => {
            steps.push(TlsHandshakeStep {
                step: "SMTP Delivery & TLS".to_string(),
                status: "failed".to_string(),
                detail: e.to_string(),
                latency_ms: t_smtp.elapsed().as_millis() as u64,
            });
            false
        }
    };

    ConnectionTestResult {
        success: smtp_verified,
        imap_tls_version,
        imap_capabilities,
        smtp_verified,
        steps,
        error: if smtp_verified {
            None
        } else {
            Some("IMAP verified, but SMTP handshake failed".to_string())
        },
    }
}

/// Two-phase IMAP sync worker:
/// - Phase 1: Fetches envelope headers (`UID FLAGS ENVELOPE INTERNALDATE RFC822.SIZE`) for
///   instantaneous inbox rendering.
/// - Phase 2: Streams full RFC 5322 MIME bodies (`BODY.PEEK[]`) asynchronously, running the
///   Rust HTML sanitizer, tracker shield, PGP detector, and SQLCipher FTS5 indexing triggers.
pub async fn sync_account_once<F>(
    db: Arc<Database>,
    account: &Account,
    mut on_event: F,
) -> CipherResult<usize>
where
    F: FnMut(SyncEventPayload),
{
    let secret_ref = keychain::account_secret_ref(&account.id);
    let secret = keychain::load_secret(&secret_ref)?;

    let (stream, _tls_info) =
        connect_imap_tls(&account.imap_host, account.imap_port, &account.imap_tls).await?;
    let mut client = async_imap::Client::new(stream);
    client
        .read_response()
        .await
        .transpose()
        .map_err(|e| CipherError::Imap(format!("IMAP greeting failed: {e}")))?
        .ok_or_else(|| CipherError::Imap("IMAP server closed connection before greeting".to_string()))?;

    let mut session = authenticate_imap(client, &account.email, &account.auth_type, secret).await?;
    let mailbox = session
        .select("INBOX")
        .await
        .map_err(|e| CipherError::Imap(format!("SELECT INBOX failed: {e}")))?;

    let total = mailbox.exists;
    if total == 0 {
        let _ = session.logout().await;
        return Ok(0);
    }

    // Fetch latest 100 messages for fast initial sync
    let start_seq = total.saturating_sub(99).max(1);
    let seq_range = format!("{start_seq}:{total}");

    // -------------------------------------------------------------------------
    // PHASE 1: Instant Envelope Headers Fetch (zero body latency)
    // -------------------------------------------------------------------------
    let mut uids_to_fetch_body = Vec::new();
    {
        let mut fetches = session
            .fetch(&seq_range, "(UID FLAGS ENVELOPE INTERNALDATE RFC822.SIZE)")
            .await
            .map_err(|e| CipherError::Imap(format!("Header FETCH failed: {e}")))?;

        while let Some(fetch_res) = fetches.next().await {
            let fetch = match fetch_res {
                Ok(f) => f,
                Err(e) => {
                    warn!("Skipping malformed envelope fetch item: {e}");
                    continue;
                }
            };
            let Some(uid) = fetch.uid else { continue };
            let is_read = fetch
                .flags()
                .any(|f| matches!(f, async_imap::types::Flag::Seen));
            let is_starred = fetch
                .flags()
                .any(|f| matches!(f, async_imap::types::Flag::Flagged));
            let size_bytes = fetch.size.unwrap_or(0) as i64;
            let received_at = fetch
                .internal_date()
                .map(|dt| dt.timestamp())
                .unwrap_or_else(|| chrono::Utc::now().timestamp());

            if let Some(env) = fetch.envelope() {
                let subject = env
                    .subject
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).to_string())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "(No Subject)".to_string());

                let (sender_name, sender_email) = env
                    .from
                    .as_ref()
                    .and_then(|list| list.first())
                    .map(|addr| {
                        let mailbox_part = addr
                            .mailbox
                            .as_ref()
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_default();
                        let host_part = addr
                            .host
                            .as_ref()
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .unwrap_or_default();
                        let email = format!("{mailbox_part}@{host_part}");
                        let name = addr
                            .name
                            .as_ref()
                            .map(|b| String::from_utf8_lossy(b).to_string())
                            .filter(|s| !s.trim().is_empty())
                            .unwrap_or_else(|| email.clone());
                        (name, email)
                    })
                    .unwrap_or_else(|| ("Unknown".to_string(), "unknown@invalid".to_string()));

                let msg_id_hdr = env
                    .message_id
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).to_string());
                let in_reply_to = env
                    .in_reply_to
                    .as_ref()
                    .map(|b| String::from_utf8_lossy(b).to_string());

                let inserted = db.upsert_envelope_header(
                    &account.id,
                    "inbox",
                    uid as i64,
                    msg_id_hdr.as_deref(),
                    in_reply_to.as_deref(),
                    &subject,
                    &sender_name,
                    &sender_email,
                    is_read,
                    is_starred,
                    size_bytes,
                    received_at,
                )?;
                if inserted {
                    uids_to_fetch_body.push(uid);
                }
            }
        }
    }

    on_event(SyncEventPayload {
        account_id: account.id.clone(),
        phase: "headers".to_string(),
        synced_count: uids_to_fetch_body.len(),
        detail: format!("Fetched {} envelope headers", uids_to_fetch_body.len()),
    });

    // -------------------------------------------------------------------------
    // PHASE 2: Async Body Streaming + Rust HTML Sanitizer + PGP + FTS5 Indexing
    // -------------------------------------------------------------------------
    let mut bodies_synced = 0usize;
    for chunk in uids_to_fetch_body.chunks(10) {
        let uid_set = chunk
            .iter()
            .map(|u| u.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let mut fetches = session
            .uid_fetch(&uid_set, "(UID BODY.PEEK[])")
            .await
            .map_err(|e| CipherError::Imap(format!("UID BODY.PEEK[] fetch failed: {e}")))?;

        while let Some(fetch_res) = fetches.next().await {
            if let Ok(fetch) = fetch_res {
                if let (Some(uid), Some(raw_body)) = (fetch.uid, fetch.body()) {
                    match parse_and_sanitize_raw_message(raw_body) {
                        Ok(parsed) => {
                            if let Err(e) =
                                db.hydrate_sanitized_message_body(&account.id, uid as i64, &parsed)
                            {
                                warn!("Failed storing sanitized message UID {uid}: {e}");
                            } else {
                                bodies_synced += 1;
                            }
                        }
                        Err(e) => warn!("Failed parsing MIME for UID {uid}: {e}"),
                    }
                }
            }
        }
    }

    db.update_account_sync_status(&account.id, "idle")?;

    on_event(SyncEventPayload {
        account_id: account.id.clone(),
        phase: "bodies".to_string(),
        synced_count: bodies_synced,
        detail: format!("Sanitized & indexed {bodies_synced} messages"),
    });

    let _ = session.logout().await;
    Ok(bodies_synced)
}

/// Long-running background IMAP IDLE push loop with exponential backoff reconnect.
/// Automatically listens for new mail notifications and triggers incremental 2-phase syncs.
pub fn spawn_imap_idle_listener<F>(db: Arc<Database>, account: Account, on_event: F)
where
    F: Fn(SyncEventPayload) + Send + Sync + 'static,
{
    let on_event = Arc::new(on_event);
    tokio::spawn(async move {
        let mut backoff_secs = 5u64;
        loop {
            let secret_ref = keychain::account_secret_ref(&account.id);
            let Ok(secret) = keychain::load_secret(&secret_ref) else {
                warn!("No keychain credential for {}; pausing IDLE worker", account.email);
                tokio::time::sleep(Duration::from_secs(60)).await;
                continue;
            };

            info!("Starting IMAP IDLE push listener for {}", account.email);
            let _ = db.update_account_sync_status(&account.id, "syncing");

            // Perform initial 2-phase sync before entering IDLE
            let cb = Arc::clone(&on_event);
            if let Err(e) = sync_account_once(Arc::clone(&db), &account, move |ev| cb(ev)).await {
                error!("Initial IMAP sync failed for {}: {e}", account.email);
                let _ = db.update_account_sync_status(&account.id, "error");
                on_event(SyncEventPayload {
                    account_id: account.id.clone(),
                    phase: "error".to_string(),
                    synced_count: 0,
                    detail: e.to_string(),
                });
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(300);
                continue;
            }

            backoff_secs = 5;

            // Connect dedicated IDLE push session
            let idle_result: CipherResult<()> = async {
                let (stream, _) =
                    connect_imap_tls(&account.imap_host, account.imap_port, &account.imap_tls)
                        .await?;
                let mut client = async_imap::Client::new(stream);
                client
                    .read_response()
                    .await
                    .transpose()
                    .map_err(|e| CipherError::Imap(e.to_string()))?
                    .ok_or_else(|| CipherError::Imap("IMAP server closed IDLE connection".to_string()))?;
                let mut session =
                    authenticate_imap(client, &account.email, &account.auth_type, secret).await?;
                session
                    .select("INBOX")
                    .await
                    .map_err(|e| CipherError::Imap(e.to_string()))?;

                let _ = db.update_account_sync_status(&account.id, "idle");
                on_event(SyncEventPayload {
                    account_id: account.id.clone(),
                    phase: "idle".to_string(),
                    synced_count: 0,
                    detail: "IMAP IDLE push active".to_string(),
                });

                let mut idle = session.idle();
                idle.init()
                    .await
                    .map_err(|e| CipherError::Imap(format!("IDLE init failed: {e}")))?;

                // RFC 2177: renew IDLE every 24 minutes or immediately on server push event
                let (idle_wait, _interrupt) = idle.wait_with_timeout(Duration::from_secs(24 * 60));
                let _ = idle_wait.await;

                let mut session = idle
                    .done()
                    .await
                    .map_err(|e| CipherError::Imap(format!("IDLE done failed: {e}")))?;
                let _ = session.logout().await;
                Ok(())
            }
            .await;

            if let Err(e) = idle_result {
                warn!("IMAP IDLE cycle interrupted for {}: {e}", account.email);
                tokio::time::sleep(Duration::from_secs(backoff_secs)).await;
                backoff_secs = (backoff_secs * 2).min(300);
            }
        }
    });
}
