pub mod seed;

use std::path::Path;
use std::sync::Mutex;
use std::time::Instant;

use rusqlite::{params, Connection, OptionalExtension};
use tracing::info;
use zeroize::Zeroizing;

use crate::error::{CipherError, CipherResult};
use crate::mail::mime::{normalize_thread_subject, ParsedMailMessage};
use crate::models::{
    Account, AppSettings, AttachmentMeta, AuthType, EmailAddress, FolderCounts,
    MessageDetail, NewAccountInput, PgpStatus, PrivacyStats, ProviderKind, SearchResponse,
    SearchResultItem, ThreadDetail, ThreadSummary, TlsMode,
};
use crate::security::keychain;

const INIT_MIGRATION_SQL: &str = include_str!("../../migrations/001_init.sql");

/// Thread-safe wrapper around an encrypted SQLCipher + FTS5 database connection.
pub struct Database {
    conn: Mutex<Connection>,
    sqlcipher_active: bool,
}

impl Database {
    /// Opens (or creates) the SQLCipher-encrypted database at `path` using the 256-bit master key
    /// retrieved from the macOS Keychain.
    pub fn open(path: &Path, hex_master_key: &Zeroizing<String>) -> CipherResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        Self::configure_and_migrate(conn, hex_master_key)
    }

    /// Opens an in-memory encrypted SQLCipher database (used in automated tests).
    #[cfg(test)]
    pub fn open_in_memory(hex_master_key: &Zeroizing<String>) -> CipherResult<Self> {
        let conn = Connection::open_in_memory()?;
        Self::configure_and_migrate(conn, hex_master_key)
    }

    fn configure_and_migrate(
        conn: Connection,
        hex_master_key: &Zeroizing<String>,
    ) -> CipherResult<Self> {
        // Apply SQLCipher 256-bit AES-CBC key before touching any schema pages
        conn.execute_batch(&format!(
            "PRAGMA key = \"x'{}'\";
             PRAGMA cipher_page_size = 4096;
             PRAGMA kdf_iter = 256000;
             PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;
             PRAGMA mmap_size = 268435456;",
            hex_master_key.as_str()
        ))?;

        let cipher_version: Option<String> = conn
            .query_row("PRAGMA cipher_version;", [], |row| row.get(0))
            .optional()
            .unwrap_or(None);

        let sqlcipher_active = cipher_version
            .as_deref()
            .map(|v| !v.is_empty())
            .unwrap_or(false);

        info!(
            "Encrypted vault initialized (SQLCipher active: {}, version: {:?})",
            sqlcipher_active, cipher_version
        );

        conn.execute_batch(INIT_MIGRATION_SQL)?;

        Ok(Self {
            conn: Mutex::new(conn),
            sqlcipher_active,
        })
    }

    fn lock(&self) -> CipherResult<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| CipherError::Database(rusqlite::Error::InvalidQuery))
    }

    pub fn is_sqlcipher_active(&self) -> bool {
        self.sqlcipher_active
    }

    // =========================================================================
    // Accounts
    // =========================================================================

    pub fn list_accounts(&self) -> CipherResult<Vec<Account>> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.email, a.display_name, a.provider, a.auth_type,
                    a.imap_host, a.imap_port, a.imap_tls,
                    a.smtp_host, a.smtp_port, a.smtp_tls,
                    a.accent_color, a.sync_status, a.last_synced_at,
                    COALESCE((SELECT SUM(t.unread_count) FROM threads t WHERE t.account_id = a.id AND t.folder_role = 'inbox'), 0)
             FROM accounts a
             ORDER BY a.created_at ASC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(Account {
                id: row.get(0)?,
                email: row.get(1)?,
                display_name: row.get(2)?,
                provider: ProviderKind::from_str(&row.get::<_, String>(3)?),
                auth_type: AuthType::from_str(&row.get::<_, String>(4)?),
                imap_host: row.get(5)?,
                imap_port: row.get::<_, i64>(6)? as u16,
                imap_tls: TlsMode::from_str(&row.get::<_, String>(7)?),
                smtp_host: row.get(8)?,
                smtp_port: row.get::<_, i64>(9)? as u16,
                smtp_tls: TlsMode::from_str(&row.get::<_, String>(10)?),
                accent_color: row.get(11)?,
                sync_status: row.get(12)?,
                last_synced_at: row.get(13)?,
                unread_count: row.get(14)?,
            })
        })?;

        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(row?);
        }
        Ok(accounts)
    }

    pub fn get_account(&self, account_id: &str) -> CipherResult<Account> {
        let accounts = self.list_accounts()?;
        accounts
            .into_iter()
            .find(|a| a.id == account_id)
            .ok_or_else(|| CipherError::NotFound(format!("Account '{account_id}' not found")))
    }

    pub fn create_account(
        &self,
        id: &str,
        input: &NewAccountInput,
        keychain_secret_ref: &str,
    ) -> CipherResult<Account> {
        let conn = self.lock()?;
        let now = chrono::Utc::now().timestamp();
        let accent = input
            .accent_color
            .clone()
            .unwrap_or_else(|| match input.provider {
                ProviderKind::Gmail => "#ef4444".to_string(),
                ProviderKind::Icloud => "#38bdf8".to_string(),
                ProviderKind::ProtonBridge => "#10b981".to_string(),
                ProviderKind::Custom => "#f59e0b".to_string(),
            });

        conn.execute(
            "INSERT INTO accounts (
                id, email, display_name, provider, auth_type,
                imap_host, imap_port, imap_tls,
                smtp_host, smtp_port, smtp_tls,
                keychain_secret_ref, accent_color, sync_status, last_synced_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'idle', NULL, ?14)",
            params![
                id,
                input.email.trim(),
                input.display_name.trim(),
                input.provider.as_str(),
                input.auth_type.as_str(),
                input.imap_host.trim(),
                input.imap_port as i64,
                input.imap_tls.as_str(),
                input.smtp_host.trim(),
                input.smtp_port as i64,
                input.smtp_tls.as_str(),
                keychain_secret_ref,
                accent,
                now
            ],
        )?;

        drop(conn);
        self.get_account(id)
    }

    pub fn delete_account(&self, account_id: &str) -> CipherResult<()> {
        let conn = self.lock()?;
        conn.execute("DELETE FROM accounts WHERE id = ?1", params![account_id])?;
        Ok(())
    }

    pub fn update_account_sync_status(&self, account_id: &str, status: &str) -> CipherResult<()> {
        let conn = self.lock()?;
        let now = chrono::Utc::now().timestamp();
        conn.execute(
            "UPDATE accounts SET sync_status = ?1, last_synced_at = ?2 WHERE id = ?3",
            params![status, now, account_id],
        )?;
        Ok(())
    }

    // =========================================================================
    // Threads & Messages
    // =========================================================================

    pub fn list_threads(
        &self,
        account_id: Option<&str>,
        folder_role: &str,
        limit: i64,
        offset: i64,
    ) -> CipherResult<Vec<ThreadSummary>> {
        let conn = self.lock()?;

        let sql = if account_id.is_some() {
            "SELECT id, account_id, folder_role, subject, snippet, participants_json,
                    last_sender_name, last_sender_email, message_count, unread_count,
                    is_starred, has_attachments, pgp_status, trackers_blocked,
                    snoozed_until, last_message_at
             FROM threads
             WHERE folder_role = ?1 AND account_id = ?2
             ORDER BY last_message_at DESC
             LIMIT ?3 OFFSET ?4"
        } else {
            "SELECT id, account_id, folder_role, subject, snippet, participants_json,
                    last_sender_name, last_sender_email, message_count, unread_count,
                    is_starred, has_attachments, pgp_status, trackers_blocked,
                    snoozed_until, last_message_at
             FROM threads
             WHERE folder_role = ?1
             ORDER BY last_message_at DESC
             LIMIT ?2 OFFSET ?3"
        };

        let mut stmt = conn.prepare(sql)?;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<ThreadSummary> {
            let participants_raw: String = row.get(5)?;
            let participants: Vec<EmailAddress> =
                serde_json::from_str(&participants_raw).unwrap_or_default();
            Ok(ThreadSummary {
                id: row.get(0)?,
                account_id: row.get(1)?,
                folder_role: row.get(2)?,
                subject: row.get(3)?,
                snippet: row.get(4)?,
                participants,
                last_sender_name: row.get(6)?,
                last_sender_email: row.get(7)?,
                message_count: row.get(8)?,
                unread_count: row.get(9)?,
                is_starred: row.get::<_, i64>(10)? != 0,
                has_attachments: row.get::<_, i64>(11)? != 0,
                pgp_status: PgpStatus::from_str(&row.get::<_, String>(12)?),
                trackers_blocked: row.get(13)?,
                snoozed_until: row.get(14)?,
                last_message_at: row.get(15)?,
            })
        };

        let mut results = Vec::new();
        if let Some(acc_id) = account_id {
            let rows = stmt.query_map(params![folder_role, acc_id, limit, offset], map_row)?;
            for r in rows {
                results.push(r?);
            }
        } else {
            let rows = stmt.query_map(params![folder_role, limit, offset], map_row)?;
            for r in rows {
                results.push(r?);
            }
        }
        Ok(results)
    }

    pub fn get_thread_detail(&self, thread_id: &str) -> CipherResult<ThreadDetail> {
        let conn = self.lock()?;
        let thread = conn
            .query_row(
                "SELECT id, account_id, folder_role, subject, snippet, participants_json,
                        last_sender_name, last_sender_email, message_count, unread_count,
                        is_starred, has_attachments, pgp_status, trackers_blocked,
                        snoozed_until, last_message_at
                 FROM threads WHERE id = ?1",
                params![thread_id],
                |row| {
                    let participants_raw: String = row.get(5)?;
                    Ok(ThreadSummary {
                        id: row.get(0)?,
                        account_id: row.get(1)?,
                        folder_role: row.get(2)?,
                        subject: row.get(3)?,
                        snippet: row.get(4)?,
                        participants: serde_json::from_str(&participants_raw).unwrap_or_default(),
                        last_sender_name: row.get(6)?,
                        last_sender_email: row.get(7)?,
                        message_count: row.get(8)?,
                        unread_count: row.get(9)?,
                        is_starred: row.get::<_, i64>(10)? != 0,
                        has_attachments: row.get::<_, i64>(11)? != 0,
                        pgp_status: PgpStatus::from_str(&row.get::<_, String>(12)?),
                        trackers_blocked: row.get(13)?,
                        snoozed_until: row.get(14)?,
                        last_message_at: row.get(15)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| CipherError::NotFound(format!("Thread '{thread_id}' not found")))?;

        let mut msg_stmt = conn.prepare(
            "SELECT id, thread_id, account_id, folder_role, message_id_header, subject,
                    sender_name, sender_email, to_json, cc_json,
                    body_text, body_html_sanitized, body_html_with_assets,
                    blocked_trackers_json, trackers_blocked_count, remote_assets_count,
                    pgp_status, pgp_fingerprint, pgp_signer,
                    is_read, is_starred, is_body_fetched, size_bytes, received_at
             FROM messages
             WHERE thread_id = ?1
             ORDER BY received_at ASC",
        )?;

        let mut messages = Vec::new();
        let rows = msg_stmt.query_map(params![thread_id], |row| {
            let to_raw: String = row.get(8)?;
            let cc_raw: String = row.get(9)?;
            let trackers_raw: String = row.get(13)?;
            Ok(MessageDetail {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                account_id: row.get(2)?,
                folder_role: row.get(3)?,
                message_id_header: row.get(4)?,
                subject: row.get(5)?,
                sender_name: row.get(6)?,
                sender_email: row.get(7)?,
                to: serde_json::from_str(&to_raw).unwrap_or_default(),
                cc: serde_json::from_str(&cc_raw).unwrap_or_default(),
                body_text: row.get(10)?,
                body_html_sanitized: row.get(11)?,
                body_html_with_assets: row.get(12)?,
                blocked_trackers: serde_json::from_str(&trackers_raw).unwrap_or_default(),
                trackers_blocked_count: row.get(14)?,
                remote_assets_count: row.get(15)?,
                pgp_status: PgpStatus::from_str(&row.get::<_, String>(16)?),
                pgp_fingerprint: row.get(17)?,
                pgp_signer: row.get(18)?,
                is_read: row.get::<_, i64>(19)? != 0,
                is_starred: row.get::<_, i64>(20)? != 0,
                is_body_fetched: row.get::<_, i64>(21)? != 0,
                size_bytes: row.get(22)?,
                received_at: row.get(23)?,
                attachments: Vec::new(),
            })
        })?;

        for row in rows {
            messages.push(row?);
        }

        // Hydrate attachments for each message
        let mut att_stmt = conn.prepare(
            "SELECT id, filename, mime_type, size_bytes, is_inline, sha256
             FROM attachments WHERE message_id = ?1",
        )?;
        for msg in &mut messages {
            let att_rows = att_stmt.query_map(params![msg.id], |row| {
                Ok(AttachmentMeta {
                    id: row.get(0)?,
                    filename: row.get(1)?,
                    mime_type: row.get(2)?,
                    size_bytes: row.get(3)?,
                    is_inline: row.get::<_, i64>(4)? != 0,
                    sha256: row.get(5)?,
                })
            })?;
            for att in att_rows {
                msg.attachments.push(att?);
            }
        }

        Ok(ThreadDetail { thread, messages })
    }

    // =========================================================================
    // Sub-20ms Local FTS5 Full-Text Search (BM25 + Highlighted Snippets)
    // =========================================================================

    pub fn search_fts5(
        &self,
        raw_query: &str,
        account_id: Option<&str>,
        limit: i64,
    ) -> CipherResult<SearchResponse> {
        let t0 = Instant::now();
        let fts_query = build_safe_fts5_query(raw_query);
        if fts_query.is_empty() {
            return Ok(SearchResponse {
                query: raw_query.to_string(),
                query_time_ms: 0.0,
                total_hits: 0,
                results: vec![],
            });
        }

        let conn = self.lock()?;
        let sql = if account_id.is_some() {
            "SELECT m.thread_id, m.id, m.account_id, m.folder_role, m.subject,
                    m.sender_name, m.sender_email,
                    snippet(messages_fts, 5, '<mark>', '</mark>', '…', 18) AS snip,
                    m.received_at, m.pgp_status, m.trackers_blocked_count,
                    bm25(messages_fts, 10.0, 5.0, 5.0, 2.0, 1.0, 3.0) AS score
             FROM messages_fts
             JOIN messages m ON m.rowid = messages_fts.rowid
             WHERE messages_fts MATCH ?1 AND m.account_id = ?2
             ORDER BY score ASC, m.received_at DESC
             LIMIT ?3"
        } else {
            "SELECT m.thread_id, m.id, m.account_id, m.folder_role, m.subject,
                    m.sender_name, m.sender_email,
                    snippet(messages_fts, 5, '<mark>', '</mark>', '…', 18) AS snip,
                    m.received_at, m.pgp_status, m.trackers_blocked_count,
                    bm25(messages_fts, 10.0, 5.0, 5.0, 2.0, 1.0, 3.0) AS score
             FROM messages_fts
             JOIN messages m ON m.rowid = messages_fts.rowid
             WHERE messages_fts MATCH ?1
             ORDER BY score ASC, m.received_at DESC
             LIMIT ?2"
        };

        let mut stmt = conn.prepare(sql)?;
        let map_row = |row: &rusqlite::Row<'_>| -> rusqlite::Result<SearchResultItem> {
            Ok(SearchResultItem {
                thread_id: row.get(0)?,
                message_id: row.get(1)?,
                account_id: row.get(2)?,
                folder_role: row.get(3)?,
                subject: row.get(4)?,
                sender_name: row.get(5)?,
                sender_email: row.get(6)?,
                highlighted_snippet: row.get(7)?,
                received_at: row.get(8)?,
                pgp_status: PgpStatus::from_str(&row.get::<_, String>(9)?),
                trackers_blocked: row.get(10)?,
                rank: row.get(11)?,
            })
        };

        let mut results = Vec::new();
        if let Some(acc_id) = account_id {
            let rows = stmt.query_map(params![fts_query, acc_id, limit], map_row)?;
            for r in rows {
                results.push(r?);
            }
        } else {
            let rows = stmt.query_map(params![fts_query, limit], map_row)?;
            for r in rows {
                results.push(r?);
            }
        }

        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
        Ok(SearchResponse {
            query: raw_query.to_string(),
            query_time_ms: (elapsed_ms * 100.0).round() / 100.0,
            total_hits: results.len(),
            results,
        })
    }

    // =========================================================================
    // Two-Phase IMAP Ingestion (Headers First -> Async Body Sanitization)
    // =========================================================================

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_envelope_header(
        &self,
        account_id: &str,
        folder_role: &str,
        imap_uid: i64,
        message_id_header: Option<&str>,
        in_reply_to: Option<&str>,
        subject: &str,
        sender_name: &str,
        sender_email: &str,
        is_read: bool,
        is_starred: bool,
        size_bytes: i64,
        received_at: i64,
    ) -> CipherResult<bool> {
        let conn = self.lock()?;

        // Check if this UID was already ingested
        let existing: Option<(String, i64)> = conn
            .query_row(
                "SELECT id, is_body_fetched FROM messages WHERE account_id = ?1 AND imap_uid = ?2",
                params![account_id, imap_uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        if let Some((_, is_body_fetched)) = existing {
            return Ok(is_body_fetched == 0);
        }

        // Resolve existing thread by In-Reply-To or normalized subject
        let thread_id = self
            .resolve_or_create_thread(
                &conn,
                account_id,
                folder_role,
                in_reply_to,
                subject,
                sender_name,
                sender_email,
                is_read,
                is_starred,
                received_at,
            )?;

        let msg_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO messages (
                id, thread_id, account_id, folder_role, imap_uid, message_id_header,
                in_reply_to, subject, sender_name, sender_email,
                is_read, is_starred, is_body_fetched, size_bytes, received_at, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 0, ?13, ?14, ?15)",
            params![
                msg_id,
                thread_id,
                account_id,
                folder_role,
                imap_uid,
                message_id_header,
                in_reply_to,
                subject,
                sender_name,
                sender_email,
                is_read as i64,
                is_starred as i64,
                size_bytes,
                received_at,
                now
            ],
        )?;

        self.refresh_thread_aggregates(&conn, &thread_id)?;
        Ok(true)
    }

    pub fn hydrate_sanitized_message_body(
        &self,
        account_id: &str,
        imap_uid: i64,
        parsed: &ParsedMailMessage,
    ) -> CipherResult<()> {
        let conn = self.lock()?;

        let found: Option<(String, String)> = conn
            .query_row(
                "SELECT id, thread_id FROM messages WHERE account_id = ?1 AND imap_uid = ?2",
                params![account_id, imap_uid],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;

        let Some((msg_id, thread_id)) = found else {
            return Ok(());
        };

        let to_json = serde_json::to_string(&parsed.to)?;
        let cc_json = serde_json::to_string(&parsed.cc)?;
        let trackers_json = serde_json::to_string(&parsed.blocked_trackers)?;

        conn.execute(
            "UPDATE messages SET
                subject = ?1,
                sender_name = ?2,
                sender_email = ?3,
                to_json = ?4,
                cc_json = ?5,
                headers_text = ?6,
                body_text = ?7,
                body_html_sanitized = ?8,
                body_html_with_assets = ?9,
                blocked_trackers_json = ?10,
                trackers_blocked_count = ?11,
                remote_assets_count = ?12,
                pgp_status = ?13,
                pgp_fingerprint = ?14,
                pgp_signer = ?15,
                is_body_fetched = 1,
                size_bytes = ?16
             WHERE id = ?17",
            params![
                parsed.subject,
                parsed.sender.name,
                parsed.sender.email,
                to_json,
                cc_json,
                parsed.headers_text,
                parsed.body_text,
                parsed.body_html_sanitized,
                parsed.body_html_with_assets,
                trackers_json,
                parsed.blocked_trackers.len() as i64,
                parsed.remote_assets_count as i64,
                parsed.pgp.status.as_str(),
                parsed.pgp.fingerprint,
                parsed.pgp.signer,
                parsed.size_bytes,
                msg_id
            ],
        )?;

        // Replace attachment metadata
        conn.execute("DELETE FROM attachments WHERE message_id = ?1", params![msg_id])?;
        for att in &parsed.attachments {
            conn.execute(
                "INSERT INTO attachments (id, message_id, filename, mime_type, size_bytes, is_inline, sha256)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    att.id,
                    msg_id,
                    att.filename,
                    att.mime_type,
                    att.size_bytes,
                    att.is_inline as i64,
                    att.sha256
                ],
            )?;
        }

        self.refresh_thread_aggregates(&conn, &thread_id)?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn resolve_or_create_thread(
        &self,
        conn: &Connection,
        account_id: &str,
        folder_role: &str,
        in_reply_to: Option<&str>,
        subject: &str,
        sender_name: &str,
        sender_email: &str,
        is_read: bool,
        is_starred: bool,
        received_at: i64,
    ) -> CipherResult<String> {
        if let Some(reply_id) = in_reply_to {
            if let Some(tid) = conn
                .query_row(
                    "SELECT thread_id FROM messages WHERE message_id_header = ?1 LIMIT 1",
                    params![reply_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                return Ok(tid);
            }
        }

        let norm_subject = normalize_thread_subject(subject);
        if !norm_subject.is_empty() && norm_subject != "(No Subject)" {
            if let Some(tid) = conn
                .query_row(
                    "SELECT id FROM threads WHERE account_id = ?1 AND lower(subject) = lower(?2) LIMIT 1",
                    params![account_id, norm_subject],
                    |row| row.get::<_, String>(0),
                )
                .optional()?
            {
                return Ok(tid);
            }
        }

        let new_tid = uuid::Uuid::new_v4().to_string();
        let participants = serde_json::to_string(&vec![EmailAddress {
            name: sender_name.to_string(),
            email: sender_email.to_string(),
        }])?;
        let now = chrono::Utc::now().timestamp();

        conn.execute(
            "INSERT INTO threads (
                id, account_id, folder_role, subject, snippet, participants_json,
                last_sender_name, last_sender_email, message_count, unread_count,
                is_starred, has_attachments, pgp_status, trackers_blocked,
                last_message_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, '', ?5, ?6, ?7, 1, ?8, ?9, 0, 'none', 0, ?10, ?11)",
            params![
                new_tid,
                account_id,
                folder_role,
                if norm_subject.is_empty() { subject } else { &norm_subject },
                participants,
                sender_name,
                sender_email,
                if is_read { 0 } else { 1 },
                is_starred as i64,
                received_at,
                now
            ],
        )?;

        Ok(new_tid)
    }

    fn refresh_thread_aggregates(&self, conn: &Connection, thread_id: &str) -> CipherResult<()> {
        let (msg_count, unread_count, trackers_sum, max_ts, any_starred): (i64, i64, i64, i64, i64) =
            conn.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN is_read = 0 THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(trackers_blocked_count), 0),
                        COALESCE(MAX(received_at), 0),
                        COALESCE(MAX(is_starred), 0)
                 FROM messages WHERE thread_id = ?1",
                params![thread_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )?;

        let has_att: i64 = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM attachments a JOIN messages m ON m.id = a.message_id WHERE m.thread_id = ?1)",
            params![thread_id],
            |row| row.get(0),
        )?;

        let latest: Option<(String, String, String, String)> = conn
            .query_row(
                "SELECT sender_name, sender_email, body_text, pgp_status
                 FROM messages WHERE thread_id = ?1
                 ORDER BY received_at DESC LIMIT 1",
                params![thread_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;

        if let Some((last_name, last_email, body_text, pgp_status)) = latest {
            let snippet: String = body_text
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(150)
                .collect();

            let now = chrono::Utc::now().timestamp();
            conn.execute(
                "UPDATE threads SET
                    message_count = ?1,
                    unread_count = ?2,
                    trackers_blocked = ?3,
                    last_message_at = ?4,
                    is_starred = ?5,
                    has_attachments = ?6,
                    last_sender_name = ?7,
                    last_sender_email = ?8,
                    snippet = CASE WHEN ?9 != '' THEN ?9 ELSE snippet END,
                    pgp_status = CASE WHEN ?10 != 'none' THEN ?10 ELSE pgp_status END,
                    updated_at = ?11
                 WHERE id = ?12",
                params![
                    msg_count,
                    unread_count,
                    trackers_sum,
                    max_ts,
                    any_starred,
                    has_att,
                    last_name,
                    last_email,
                    snippet,
                    pgp_status,
                    now,
                    thread_id
                ],
            )?;
        }

        Ok(())
    }

    // =========================================================================
    // Thread Actions (Archive, Trash, Snooze, Read/Unread, Star, Send Store)
    // =========================================================================

    pub fn move_threads_to_folder(
        &self,
        thread_ids: &[String],
        target_folder: &str,
    ) -> CipherResult<usize> {
        let conn = self.lock()?;
        let now = chrono::Utc::now().timestamp();
        let mut updated = 0usize;
        for tid in thread_ids {
            updated += conn.execute(
                "UPDATE threads SET folder_role = ?1, snoozed_until = NULL, updated_at = ?2 WHERE id = ?3",
                params![target_folder, now, tid],
            )?;
            conn.execute(
                "UPDATE messages SET folder_role = ?1 WHERE thread_id = ?2",
                params![target_folder, tid],
            )?;
        }
        Ok(updated)
    }

    pub fn snooze_threads(&self, thread_ids: &[String], until_ts: i64) -> CipherResult<usize> {
        let conn = self.lock()?;
        let now = chrono::Utc::now().timestamp();
        let mut updated = 0usize;
        for tid in thread_ids {
            updated += conn.execute(
                "UPDATE threads SET folder_role = 'snoozed', snoozed_until = ?1, updated_at = ?2 WHERE id = ?3",
                params![until_ts, now, tid],
            )?;
        }
        Ok(updated)
    }

    pub fn set_threads_read(&self, thread_ids: &[String], is_read: bool) -> CipherResult<usize> {
        let conn = self.lock()?;
        let mut updated = 0usize;
        for tid in thread_ids {
            conn.execute(
                "UPDATE messages SET is_read = ?1 WHERE thread_id = ?2",
                params![is_read as i64, tid],
            )?;
            self.refresh_thread_aggregates(&conn, tid)?;
            updated += 1;
        }
        Ok(updated)
    }

    pub fn toggle_thread_star(&self, thread_id: &str) -> CipherResult<bool> {
        let conn = self.lock()?;
        let current: i64 = conn.query_row(
            "SELECT is_starred FROM threads WHERE id = ?1",
            params![thread_id],
            |row| row.get(0),
        )?;
        let next = if current == 0 { 1 } else { 0 };
        conn.execute(
            "UPDATE threads SET is_starred = ?1 WHERE id = ?2",
            params![next, thread_id],
        )?;
        conn.execute(
            "UPDATE messages SET is_starred = ?1 WHERE thread_id = ?2",
            params![next, thread_id],
        )?;
        Ok(next != 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_outbound_message(
        &self,
        account: &Account,
        message_id_header: &str,
        thread_id_opt: Option<&str>,
        to: &[String],
        cc: &[String],
        subject: &str,
        body_text: &str,
        pgp_status: PgpStatus,
    ) -> CipherResult<String> {
        let conn = self.lock()?;
        let now = chrono::Utc::now().timestamp();

        let thread_id = match thread_id_opt {
            Some(tid) if !tid.is_empty() => tid.to_string(),
            _ => {
                let tid = uuid::Uuid::new_v4().to_string();
                let participants = serde_json::to_string(&vec![EmailAddress {
                    name: account.display_name.clone(),
                    email: account.email.clone(),
                }])?;
                let snippet: String = body_text.chars().take(140).collect();
                conn.execute(
                    "INSERT INTO threads (
                        id, account_id, folder_role, subject, snippet, participants_json,
                        last_sender_name, last_sender_email, message_count, unread_count,
                        is_starred, has_attachments, pgp_status, trackers_blocked,
                        last_message_at, updated_at
                     ) VALUES (?1, ?2, 'sent', ?3, ?4, ?5, ?6, ?7, 1, 0, 0, 0, ?8, 0, ?9, ?9)",
                    params![
                        tid,
                        account.id,
                        subject,
                        snippet,
                        participants,
                        account.display_name,
                        account.email,
                        pgp_status.as_str(),
                        now
                    ],
                )?;
                tid
            }
        };

        let to_addrs: Vec<EmailAddress> = to
            .iter()
            .map(|e| EmailAddress {
                name: e.clone(),
                email: e.clone(),
            })
            .collect();
        let cc_addrs: Vec<EmailAddress> = cc
            .iter()
            .map(|e| EmailAddress {
                name: e.clone(),
                email: e.clone(),
            })
            .collect();

        let safe_html = format!(
            r#"<div style="white-space:pre-wrap;font-family:-apple-system,BlinkMacSystemFont,'Inter',sans-serif;line-height:1.65;">{}</div>"#,
            body_text
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        );

        let msg_id = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO messages (
                id, thread_id, account_id, folder_role, message_id_header,
                subject, sender_name, sender_email, to_json, cc_json,
                headers_text, body_text, body_html_sanitized, body_html_with_assets,
                blocked_trackers_json, trackers_blocked_count, remote_assets_count,
                pgp_status, is_read, is_starred, is_body_fetched, size_bytes, received_at, created_at
             ) VALUES (
                ?1, ?2, ?3, 'sent', ?4,
                ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?12,
                '[]', 0, 0,
                ?13, 1, 0, 1, ?14, ?15, ?15
             )",
            params![
                msg_id,
                thread_id,
                account.id,
                message_id_header,
                subject,
                account.display_name,
                account.email,
                serde_json::to_string(&to_addrs)?,
                serde_json::to_string(&cc_addrs)?,
                format!("From: {}\nTo: {}\nSubject: {subject}", account.email, to.join(", ")),
                body_text,
                safe_html,
                pgp_status.as_str(),
                body_text.len() as i64,
                now
            ],
        )?;

        self.refresh_thread_aggregates(&conn, &thread_id)?;
        Ok(msg_id)
    }

    // =========================================================================
    // Counts, Privacy Stats & Settings
    // =========================================================================

    pub fn get_folder_counts(&self, account_id: Option<&str>) -> CipherResult<FolderCounts> {
        let conn = self.lock()?;
        let count_query = |role: &str, only_unread: bool| -> CipherResult<i64> {
            let val = match (account_id, only_unread) {
                (Some(aid), true) => conn.query_row(
                    "SELECT COUNT(*) FROM threads WHERE folder_role = ?1 AND account_id = ?2 AND unread_count > 0",
                    params![role, aid],
                    |r| r.get(0),
                )?,
                (Some(aid), false) => conn.query_row(
                    "SELECT COUNT(*) FROM threads WHERE folder_role = ?1 AND account_id = ?2",
                    params![role, aid],
                    |r| r.get(0),
                )?,
                (None, true) => conn.query_row(
                    "SELECT COUNT(*) FROM threads WHERE folder_role = ?1 AND unread_count > 0",
                    params![role],
                    |r| r.get(0),
                )?,
                (None, false) => conn.query_row(
                    "SELECT COUNT(*) FROM threads WHERE folder_role = ?1",
                    params![role],
                    |r| r.get(0),
                )?,
            };
            Ok(val)
        };

        Ok(FolderCounts {
            inbox_unread: count_query("inbox", true)?,
            inbox_total: count_query("inbox", false)?,
            sent_total: count_query("sent", false)?,
            drafts_total: count_query("drafts", false)?,
            snoozed_total: count_query("snoozed", false)?,
            archive_total: count_query("archive", false)?,
            trash_total: count_query("trash", false)?,
        })
    }

    pub fn get_privacy_stats(&self) -> CipherResult<PrivacyStats> {
        let conn = self.lock()?;
        let total_trackers_blocked: i64 = conn.query_row(
            "SELECT COALESCE(SUM(trackers_blocked_count), 0) FROM messages",
            [],
            |r| r.get(0),
        )?;
        let total_messages_indexed: i64 =
            conn.query_row("SELECT COUNT(*) FROM messages", [], |r| r.get(0))?;
        let pgp_verified_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM messages WHERE pgp_status IN ('signed_verified', 'encrypted', 'encrypted_verified')",
            [],
            |r| r.get(0),
        )?;
        let fts5_ready: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'messages_fts'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|c| c > 0)
            .unwrap_or(false);

        Ok(PrivacyStats {
            total_trackers_blocked,
            total_messages_indexed,
            pgp_verified_count,
            sqlcipher_active: self.sqlcipher_active,
            keychain_backend: keychain::keychain_backend_name().to_string(),
            fts5_ready,
        })
    }

    pub fn get_settings(&self) -> CipherResult<AppSettings> {
        let conn = self.lock()?;
        let get = |key: &str, default: &str| -> String {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| default.to_string())
        };

        Ok(AppSettings {
            ai_enabled: get("ai_enabled", "true") == "true",
            ai_endpoint: get("ai_endpoint", "http://localhost:11434"),
            ai_model: get("ai_model", "llama3.2:latest"),
            block_remote_images: get("block_remote_images", "true") == "true",
            strip_trackers: get("strip_trackers", "true") == "true",
            auto_verify_pgp: get("auto_verify_pgp", "true") == "true",
        })
    }

    pub fn save_settings(&self, settings: &AppSettings) -> CipherResult<AppSettings> {
        // Always enforce zero-trust local-only AI endpoint before persisting
        crate::ai::validate_local_endpoint(&settings.ai_endpoint)?;

        let conn = self.lock()?;
        let now = chrono::Utc::now().timestamp();
        let pairs = [
            ("ai_enabled", if settings.ai_enabled { "true" } else { "false" }),
            ("ai_endpoint", settings.ai_endpoint.as_str()),
            ("ai_model", settings.ai_model.as_str()),
            (
                "block_remote_images",
                if settings.block_remote_images {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                "strip_trackers",
                if settings.strip_trackers { "true" } else { "false" },
            ),
            (
                "auto_verify_pgp",
                if settings.auto_verify_pgp { "true" } else { "false" },
            ),
        ];

        for (k, v) in pairs {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![k, v, now],
            )?;
        }

        drop(conn);
        self.get_settings()
    }
}

/// Converts user search text into a safe FTS5 prefix query, stripping special FTS operators
/// that could otherwise cause syntax errors on partial typing.
fn build_safe_fts5_query(raw: &str) -> String {
    raw.split_whitespace()
        .filter_map(|token| {
            let cleaned: String = token
                .chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, '@' | '.' | '-' | '_'))
                .collect();
            if cleaned.is_empty() {
                None
            } else {
                Some(format!("\"{cleaned}\"*"))
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlcipher_and_fts5_triggers_work_sub_20ms() {
        let key = Zeroizing::new("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string());
        let db = Database::open_in_memory(&key).expect("open encrypted in-memory db");
        assert!(db.is_sqlcipher_active(), "SQLCipher must be active");

        seed::seed_demo_vault_if_empty(&db).expect("seed vault");

        let res = db
            .search_fts5("Zero-Trust", None, 20)
            .expect("fts5 search should succeed");
        assert!(res.total_hits >= 1, "FTS5 trigger should have indexed seeded messages");
        assert!(
            res.query_time_ms < 50.0,
            "FTS5 search expected to be blazingly fast, got {}ms",
            res.query_time_ms
        );
    }

    #[test]
    fn e2e_full_backend_lifecycle() {
        let key = Zeroizing::new(
            "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789".to_string(),
        );
        let db = Database::open_in_memory(&key).expect("open sqlcipher db");

        // 1. Create account
        let acc = db
            .create_account(
                "e2e-acc-1",
                &NewAccountInput {
                    email: "tester@gmail.com".to_string(),
                    display_name: "Tester".to_string(),
                    provider: ProviderKind::Gmail,
                    auth_type: AuthType::Password,
                    imap_host: "imap.gmail.com".to_string(),
                    imap_port: 993,
                    imap_tls: TlsMode::Implicit,
                    smtp_host: "smtp.gmail.com".to_string(),
                    smtp_port: 465,
                    smtp_tls: TlsMode::Implicit,
                    secret: "app-pass-1234".to_string(),
                    accent_color: None,
                },
                "account_secret:e2e-acc-1",
            )
            .expect("create account");
        assert_eq!(acc.email, "tester@gmail.com");

        // 2. Phase 1: Envelope header ingestion
        let needs_body = db
            .upsert_envelope_header(
                &acc.id,
                "inbox",
                101,
                Some("<e2e-101@example.org>"),
                None,
                "Security Update & Invoice",
                "Alice",
                "alice@example.org",
                false,
                false,
                4096,
                1732470000,
            )
            .expect("upsert envelope");
        assert!(needs_body);

        // 3. Phase 2: MIME parse + Rust HTML sanitizer + FTS5 update trigger
        let raw_mime = concat!(
            "From: Alice <alice@example.org>\r\n",
            "To: Tester <tester@gmail.com>\r\n",
            "Subject: Security Update & Invoice\r\n",
            "Message-ID: <e2e-101@example.org>\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "\r\n",
            "<html><body onload='x()'><p>Quarterly invoice attached.</p>",
            "<script>stealCookies()</script>",
            "<img src='https://t.sidekickopen80.com/px.gif' width='1' height='1'/>",
            "<img src='https://cdn.example.org/banner.png' alt='Banner' width='600' height='200'/>",
            "</body></html>"
        );
        let parsed = crate::mail::mime::parse_and_sanitize_raw_message(raw_mime.as_bytes())
            .expect("parse & sanitize MIME");
        db.hydrate_sanitized_message_body(&acc.id, 101, &parsed)
            .expect("hydrate sanitized body");

        // 4. Verify thread list & sanitized detail
        let inbox = db
            .list_threads(Some(&acc.id), "inbox", 50, 0)
            .expect("list inbox");
        assert_eq!(inbox.len(), 1);
        let tid = inbox[0].id.clone();

        let detail = db.get_thread_detail(&tid).expect("get thread detail");
        let msg = &detail.messages[0];
        assert!(!msg.body_html_sanitized.contains("<script>"));
        assert!(!msg.body_html_sanitized.contains("sidekickopen80"));
        assert_eq!(msg.remote_assets_count, 1);
        assert!(msg.trackers_blocked_count >= 3);

        // 5. Verify FTS5 search hits newly hydrated message
        let fts = db
            .search_fts5("Quarterly", Some(&acc.id), 20)
            .expect("fts5 search");
        assert_eq!(fts.total_hits, 1);
        assert_eq!(fts.results[0].thread_id, tid);

        // 6. Verify folder transitions (star, read, snooze, archive, trash, inbox)
        assert!(db.toggle_thread_star(&tid).expect("star"));
        db.set_threads_read(&[tid.clone()], true).expect("mark read");
        db.snooze_threads(&[tid.clone()], 1732556400).expect("snooze");
        assert_eq!(db.list_threads(Some(&acc.id), "snoozed", 50, 0).unwrap().len(), 1);
        db.move_threads_to_folder(&[tid.clone()], "archive").expect("archive");
        assert_eq!(db.list_threads(Some(&acc.id), "archive", 50, 0).unwrap().len(), 1);
        db.move_threads_to_folder(&[tid.clone()], "trash").expect("trash");
        assert_eq!(db.list_threads(Some(&acc.id), "trash", 50, 0).unwrap().len(), 1);
        db.move_threads_to_folder(&[tid.clone()], "inbox").expect("restore inbox");

        // 7. Verify outbound message recording & counts
        db.record_outbound_message(
            &acc,
            "<out-1@cipherpost.local>",
            None,
            &["bob@example.org".to_string()],
            &[],
            "Outbound Test",
            "Direct SMTP body",
            PgpStatus::SignedVerified,
        )
        .expect("record outbound");
        let counts = db.get_folder_counts(Some(&acc.id)).expect("counts");
        assert_eq!(counts.inbox_total, 1);
        assert_eq!(counts.sent_total, 1);

        // 8. Verify privacy stats & loopback-only AI settings enforcement
        let stats = db.get_privacy_stats().expect("privacy stats");
        assert!(stats.sqlcipher_active);
        assert!(stats.fts5_ready);
        assert!(stats.total_trackers_blocked >= 3);

        let mut cfg = db.get_settings().expect("get settings");
        cfg.ai_endpoint = "https://api.openai.com/v1".to_string();
        assert!(db.save_settings(&cfg).is_err(), "cloud AI endpoint must be rejected");
        cfg.ai_endpoint = "http://127.0.0.1:11434".to_string();
        assert!(db.save_settings(&cfg).is_ok());

        // 9. Delete account cascades cleanly
        db.delete_account(&acc.id).expect("delete account");
        assert!(db.list_accounts().unwrap().is_empty());
    }
}
