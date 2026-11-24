use mail_parser::{Address, HeaderValue, MessageParser, MimeHeaders};
use sha2::{Digest, Sha256};

use crate::crypto::pgp::{inspect_pgp, PgpInspection};
use crate::error::{CipherError, CipherResult};
use crate::models::{AttachmentMeta, BlockedTracker, EmailAddress};
use crate::sanitizer::sanitize_email_html;

/// Fully parsed and privacy-sanitized email message ready for SQLCipher + FTS5 storage.
#[derive(Debug, Clone)]
pub struct ParsedMailMessage {
    pub message_id_header: Option<String>,
    pub in_reply_to: Option<String>,
    pub references_header: Option<String>,
    pub subject: String,
    pub sender: EmailAddress,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub headers_text: String,
    pub body_text: String,
    pub body_html_sanitized: String,
    pub body_html_with_assets: String,
    pub blocked_trackers: Vec<BlockedTracker>,
    pub remote_assets_count: usize,
    pub pgp: PgpInspection,
    pub attachments: Vec<AttachmentMeta>,
    pub received_at: i64,
    pub size_bytes: i64,
}

fn extract_addresses(addr: Option<&Address<'_>>) -> Vec<EmailAddress> {
    let mut out = Vec::new();
    if let Some(address) = addr {
        for item in address.iter() {
            if let Some(email) = item.address() {
                let name = item
                    .name()
                    .map(|n| n.trim().to_string())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| email.to_string());
                out.push(EmailAddress {
                    name,
                    email: email.to_string(),
                });
            }
        }
    }
    out
}

fn plain_text_to_safe_html(text: &str) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    format!(
        r#"<div style="white-space:pre-wrap;font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;line-height:1.65;">{}</div>"#,
        escaped
    )
}

/// Parses raw RFC 5322 MIME bytes using `mail-parser`, runs the Rust Active Privacy HTML sanitizer,
/// extracts attachment metadata, and inspects PGP/MIME or inline OpenPGP signatures/encryption.
pub fn parse_and_sanitize_raw_message(raw_rfc822: &[u8]) -> CipherResult<ParsedMailMessage> {
    let parsed = MessageParser::default()
        .parse(raw_rfc822)
        .ok_or_else(|| CipherError::Imap("Failed to parse RFC 5322 MIME message".to_string()))?;

    let message_id_header = parsed.message_id().map(|s| s.to_string());

    let in_reply_to = match parsed.in_reply_to() {
        HeaderValue::Text(s) => Some(s.to_string()),
        HeaderValue::TextList(list) => list.first().map(|s| s.to_string()),
        _ => None,
    };

    let references_header = match parsed.references() {
        HeaderValue::Text(s) => Some(s.to_string()),
        HeaderValue::TextList(list) => Some(list.join(" ")),
        _ => None,
    };

    let subject = parsed
        .subject()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "(No Subject)".to_string());

    let from_list = extract_addresses(parsed.from());
    let sender = from_list.into_iter().next().unwrap_or(EmailAddress {
        name: "Unknown Sender".to_string(),
        email: "unknown@invalid".to_string(),
    });

    let to = extract_addresses(parsed.to());
    let cc = extract_addresses(parsed.cc());

    let received_at = parsed
        .date()
        .map(|dt| dt.to_timestamp())
        .unwrap_or_else(|| chrono::Utc::now().timestamp());

    // Build searchable header digest for FTS5
    let mut header_lines = Vec::new();
    let mut top_content_type = String::new();
    for header in parsed.headers() {
        let name = header.name();
        if matches!(
            name.to_ascii_lowercase().as_str(),
            "from"
                | "to"
                | "cc"
                | "subject"
                | "message-id"
                | "authentication-results"
                | "dkim-signature"
                | "list-id"
                | "content-type"
                | "x-mailer"
        ) {
            if let HeaderValue::Text(val) = header.value() {
                if name.eq_ignore_ascii_case("content-type") && top_content_type.is_empty() {
                    top_content_type = val.to_string();
                }
                header_lines.push(format!("{name}: {val}"));
            } else if let HeaderValue::ContentType(ct) = header.value() {
                let ct_str = match ct.subtype() {
                    Some(sub) => format!("{}/{sub}", ct.ctype()),
                    None => ct.ctype().to_string(),
                };
                if top_content_type.is_empty() {
                    top_content_type = ct_str.clone();
                }
                header_lines.push(format!("{name}: {ct_str}"));
            }
        }
    }
    let headers_text = header_lines.join("\n");

    let body_text = parsed
        .body_text(0)
        .map(|s| s.into_owned())
        .unwrap_or_default();

    let raw_html = parsed
        .body_html(0)
        .map(|s| s.into_owned())
        .unwrap_or_else(|| plain_text_to_safe_html(&body_text));

    // Run Rust Active Privacy & Tracker Shield sanitizer BEFORE returning or storing HTML
    let sanitized = sanitize_email_html(&raw_html)?;

    // Inspect attachments and detect PGP MIME parts
    let mut attachments = Vec::new();
    let mut detached_pgp_sig: Option<Vec<u8>> = None;
    let mut saw_pgp_encrypted_part = false;

    for part in parsed.attachments() {
        let mime_type = part
            .content_type()
            .map(|ct| match ct.subtype() {
                Some(sub) => format!("{}/{sub}", ct.ctype()),
                None => ct.ctype().to_string(),
            })
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let contents = part.contents();
        if mime_type.eq_ignore_ascii_case("application/pgp-signature") {
            detached_pgp_sig = Some(contents.to_vec());
            continue;
        }
        if mime_type.eq_ignore_ascii_case("application/pgp-encrypted") {
            saw_pgp_encrypted_part = true;
            continue;
        }

        let filename = part
            .attachment_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "attachment.bin".to_string());

        let mut hasher = Sha256::new();
        hasher.update(contents);
        let digest = hex::encode(hasher.finalize());

        attachments.push(AttachmentMeta {
            id: uuid::Uuid::new_v4().to_string(),
            filename,
            mime_type,
            size_bytes: contents.len() as i64,
            is_inline: part
                .content_disposition()
                .map(|cd| cd.ctype().eq_ignore_ascii_case("inline"))
                .unwrap_or(false),
            sha256: Some(digest),
        });
    }

    let effective_ct = if saw_pgp_encrypted_part {
        "multipart/encrypted; protocol=\"application/pgp-encrypted\"".to_string()
    } else if detached_pgp_sig.is_some() {
        "multipart/signed; protocol=\"application/pgp-signature\"".to_string()
    } else {
        top_content_type
    };

    let pgp = inspect_pgp(
        &effective_ct,
        &body_text,
        detached_pgp_sig.as_deref(),
        Some(body_text.as_bytes()),
        &[],
    );

    Ok(ParsedMailMessage {
        message_id_header,
        in_reply_to,
        references_header,
        subject,
        sender,
        to,
        cc,
        headers_text,
        body_text,
        body_html_sanitized: sanitized.html_blocked,
        body_html_with_assets: sanitized.html_with_assets,
        blocked_trackers: sanitized.blocked_trackers,
        remote_assets_count: sanitized.remote_assets_count,
        pgp,
        attachments,
        received_at,
        size_bytes: raw_rfc822.len() as i64,
    })
}

/// Normalizes a subject string for thread grouping when `In-Reply-To`/`References` are absent.
pub fn normalize_thread_subject(subject: &str) -> String {
    let mut s = subject.trim().to_string();
    loop {
        let lower = s.to_ascii_lowercase();
        if let Some(rest) = lower
            .strip_prefix("re:")
            .or_else(|| lower.strip_prefix("fwd:"))
            .or_else(|| lower.strip_prefix("fw:"))
        {
            let diff = s.len() - rest.len();
            s = s[diff..].trim().to_string();
        } else {
            break;
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rfc822_and_strips_trackers() {
        let raw = concat!(
            "From: Security Team <sec@example.org>\r\n",
            "To: Alice <alice@cipherpost.local>\r\n",
            "Subject: Re: Quarterly Audit\r\n",
            "Message-ID: <msg-001@example.org>\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "\r\n",
            "<html><body><p>Audit complete.</p>",
            "<img src=\"https://track.hubspot.com/open.gif\" width=\"1\" height=\"1\" />",
            "<script>evil()</script></body></html>"
        );

        let parsed = parse_and_sanitize_raw_message(raw.as_bytes()).expect("parse mime");
        assert_eq!(parsed.subject, "Re: Quarterly Audit");
        assert_eq!(parsed.sender.email, "sec@example.org");
        assert!(!parsed.body_html_sanitized.contains("script"));
        assert!(!parsed.body_html_sanitized.contains("hubspot.com"));
        assert!(parsed.blocked_trackers.len() >= 2);
        assert_eq!(normalize_thread_subject(&parsed.subject), "Quarterly Audit");
    }
}
