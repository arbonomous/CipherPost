use std::time::Duration;

use lettre::message::header::ContentType;
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use zeroize::Zeroizing;

use crate::error::{CipherError, CipherResult};
use crate::models::{Account, AuthType, ComposePayload, NewAccountInput, TlsMode};
use crate::security::keychain;

fn is_loopback(host: &str) -> bool {
    matches!(
        host.trim().to_ascii_lowercase().as_str(),
        "localhost" | "127.0.0.1" | "::1"
    )
}

fn build_smtp_transport(
    host: &str,
    port: u16,
    tls_mode: &TlsMode,
    email: &str,
    auth_type: &AuthType,
    secret: &Zeroizing<String>,
) -> CipherResult<AsyncSmtpTransport<Tokio1Executor>> {
    let mut tls_builder = TlsParameters::builder(host.to_string());
    if is_loopback(host) {
        // Proton Mail Bridge uses a local self-signed TLS certificate on 127.0.0.1:1025
        tls_builder = tls_builder.dangerous_accept_invalid_certs(true);
    }
    let tls_params = tls_builder
        .build_rustls()
        .map_err(|e| CipherError::Smtp(format!("Failed building SMTP TLS parameters: {e}")))?;

    let tls = match tls_mode {
        TlsMode::Implicit => Tls::Wrapper(tls_params),
        TlsMode::Starttls => Tls::Required(tls_params),
    };

    let creds = Credentials::new(email.to_string(), secret.as_str().to_string());
    let mechanisms = match auth_type {
        AuthType::Oauth2 => vec![Mechanism::Xoauth2],
        AuthType::Password => vec![Mechanism::Plain, Mechanism::Login],
    };

    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host)
        .port(port)
        .tls(tls)
        .credentials(creds)
        .authentication(mechanisms)
        .timeout(Some(Duration::from_secs(15)))
        .build();

    Ok(transport)
}

/// Runs a live SMTP TLS + credential handshake test for the onboarding account modal.
pub async fn test_smtp_connection(input: &NewAccountInput) -> CipherResult<String> {
    let secret = Zeroizing::new(input.secret.clone());
    let transport = build_smtp_transport(
        &input.smtp_host,
        input.smtp_port,
        &input.smtp_tls,
        &input.email,
        &input.auth_type,
        &secret,
    )?;

    let ok = transport
        .test_connection()
        .await
        .map_err(|e| CipherError::Smtp(format!("SMTP handshake/auth failed: {e}")))?;

    if ok {
        Ok(format!(
            "{}:{} ({}) authenticated",
            input.smtp_host,
            input.smtp_port,
            input.smtp_tls.as_str()
        ))
    } else {
        Err(CipherError::Smtp(
            "SMTP server rejected test connection".to_string(),
        ))
    }
}

/// Sends an outbound email directly from the user's machine to the provider's SMTP server via `lettre`.
pub async fn send_email_direct(account: &Account, payload: &ComposePayload) -> CipherResult<String> {
    if payload.to.is_empty() {
        return Err(CipherError::Validation(
            "At least one recipient is required".to_string(),
        ));
    }

    let secret_ref = keychain::account_secret_ref(&account.id);
    let secret = keychain::load_secret(&secret_ref)?;

    let from_mailbox = format!("{} <{}>", account.display_name, account.email)
        .parse()
        .map_err(|e| CipherError::Validation(format!("Invalid sender address: {e}")))?;

    let generated_msg_id = format!(
        "<{}@{}>",
        uuid::Uuid::new_v4(),
        account.email.split('@').nth(1).unwrap_or("cipherpost.local")
    );

    let mut builder = Message::builder()
        .from(from_mailbox)
        .subject(&payload.subject)
        .message_id(Some(generated_msg_id.clone()));

    for to_addr in &payload.to {
        let parsed = to_addr
            .trim()
            .parse()
            .map_err(|e| CipherError::Validation(format!("Invalid To address '{to_addr}': {e}")))?;
        builder = builder.to(parsed);
    }
    for cc_addr in &payload.cc {
        if !cc_addr.trim().is_empty() {
            let parsed = cc_addr.trim().parse().map_err(|e| {
                CipherError::Validation(format!("Invalid Cc address '{cc_addr}': {e}"))
            })?;
            builder = builder.cc(parsed);
        }
    }
    for bcc_addr in &payload.bcc {
        if !bcc_addr.trim().is_empty() {
            let parsed = bcc_addr.trim().parse().map_err(|e| {
                CipherError::Validation(format!("Invalid Bcc address '{bcc_addr}': {e}"))
            })?;
            builder = builder.bcc(parsed);
        }
    }
    if let Some(ref reply_to) = payload.in_reply_to {
        if !reply_to.trim().is_empty() {
            builder = builder.in_reply_to(reply_to.clone());
            builder = builder.references(reply_to.clone());
        }
    }

    let email = builder
        .header(ContentType::TEXT_PLAIN)
        .body(payload.body_text.clone())
        .map_err(|e| CipherError::Smtp(format!("Failed constructing RFC 5322 email: {e}")))?;

    let transport = build_smtp_transport(
        &account.smtp_host,
        account.smtp_port,
        &account.smtp_tls,
        &account.email,
        &account.auth_type,
        &secret,
    )?;

    transport
        .send(email)
        .await
        .map_err(|e| CipherError::Smtp(format!("Direct SMTP delivery failed: {e}")))?;

    Ok(generated_msg_id)
}
