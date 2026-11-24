-- ============================================================================
-- CipherPost Encrypted Local Storage Schema (SQLCipher + SQLite FTS5)
-- ============================================================================
-- Encryption at rest is enforced before executing this migration via:
--   PRAGMA key = "x'<64-hex-char-256bit-key-from-macos-keychain>'";
--   PRAGMA cipher_page_size = 4096;
--   PRAGMA kdf_iter = 256000;
--   PRAGMA journal_mode = WAL;
--   PRAGMA foreign_keys = ON;
-- ============================================================================

CREATE TABLE IF NOT EXISTS accounts (
    id                  TEXT PRIMARY KEY NOT NULL,
    email               TEXT NOT NULL UNIQUE,
    display_name        TEXT NOT NULL,
    provider            TEXT NOT NULL CHECK (provider IN ('gmail', 'icloud', 'proton_bridge', 'custom')),
    auth_type           TEXT NOT NULL CHECK (auth_type IN ('password', 'oauth2')),
    imap_host           TEXT NOT NULL,
    imap_port           INTEGER NOT NULL,
    imap_tls            TEXT NOT NULL CHECK (imap_tls IN ('implicit', 'starttls')),
    smtp_host           TEXT NOT NULL,
    smtp_port           INTEGER NOT NULL,
    smtp_tls            TEXT NOT NULL CHECK (smtp_tls IN ('implicit', 'starttls')),
    keychain_secret_ref TEXT NOT NULL,
    accent_color        TEXT NOT NULL DEFAULT '#10b981',
    sync_status         TEXT NOT NULL DEFAULT 'idle',
    last_synced_at      INTEGER,
    created_at          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS folders (
    id                  TEXT PRIMARY KEY NOT NULL,
    account_id          TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name                TEXT NOT NULL,
    remote_path         TEXT NOT NULL,
    role                TEXT NOT NULL CHECK (role IN ('inbox', 'sent', 'drafts', 'snoozed', 'archive', 'trash', 'custom')),
    uid_validity        INTEGER,
    highest_uid         INTEGER NOT NULL DEFAULT 0,
    highest_modseq      INTEGER NOT NULL DEFAULT 0,
    unread_count        INTEGER NOT NULL DEFAULT 0,
    total_count         INTEGER NOT NULL DEFAULT 0,
    UNIQUE(account_id, remote_path)
);

CREATE TABLE IF NOT EXISTS threads (
    id                  TEXT PRIMARY KEY NOT NULL,
    account_id          TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_role         TEXT NOT NULL DEFAULT 'inbox',
    subject             TEXT NOT NULL DEFAULT '(No Subject)',
    snippet             TEXT NOT NULL DEFAULT '',
    participants_json   TEXT NOT NULL DEFAULT '[]',
    last_sender_name    TEXT NOT NULL DEFAULT '',
    last_sender_email   TEXT NOT NULL DEFAULT '',
    message_count       INTEGER NOT NULL DEFAULT 1,
    unread_count        INTEGER NOT NULL DEFAULT 0,
    is_starred          INTEGER NOT NULL DEFAULT 0,
    has_attachments     INTEGER NOT NULL DEFAULT 0,
    pgp_status          TEXT NOT NULL DEFAULT 'none' CHECK (pgp_status IN ('none', 'signed_verified', 'signed_unverified', 'encrypted', 'encrypted_verified')),
    trackers_blocked    INTEGER NOT NULL DEFAULT 0,
    snoozed_until       INTEGER,
    last_message_at     INTEGER NOT NULL,
    updated_at          INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS messages (
    id                      TEXT PRIMARY KEY NOT NULL,
    thread_id               TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    account_id              TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    folder_id               TEXT REFERENCES folders(id) ON DELETE SET NULL,
    folder_role             TEXT NOT NULL DEFAULT 'inbox',
    imap_uid                INTEGER,
    message_id_header       TEXT,
    in_reply_to             TEXT,
    references_header       TEXT,
    subject                 TEXT NOT NULL DEFAULT '(No Subject)',
    sender_name             TEXT NOT NULL DEFAULT '',
    sender_email            TEXT NOT NULL DEFAULT '',
    to_json                 TEXT NOT NULL DEFAULT '[]',
    cc_json                 TEXT NOT NULL DEFAULT '[]',
    bcc_json                TEXT NOT NULL DEFAULT '[]',
    headers_text            TEXT NOT NULL DEFAULT '',
    body_text               TEXT NOT NULL DEFAULT '',
    body_html_sanitized     TEXT NOT NULL DEFAULT '',
    body_html_with_assets   TEXT NOT NULL DEFAULT '',
    blocked_trackers_json   TEXT NOT NULL DEFAULT '[]',
    trackers_blocked_count  INTEGER NOT NULL DEFAULT 0,
    remote_assets_count     INTEGER NOT NULL DEFAULT 0,
    pgp_status              TEXT NOT NULL DEFAULT 'none' CHECK (pgp_status IN ('none', 'signed_verified', 'signed_unverified', 'encrypted', 'encrypted_verified')),
    pgp_fingerprint         TEXT,
    pgp_signer              TEXT,
    is_read                 INTEGER NOT NULL DEFAULT 0,
    is_starred              INTEGER NOT NULL DEFAULT 0,
    is_body_fetched         INTEGER NOT NULL DEFAULT 0,
    size_bytes              INTEGER NOT NULL DEFAULT 0,
    received_at             INTEGER NOT NULL,
    created_at              INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS attachments (
    id                  TEXT PRIMARY KEY NOT NULL,
    message_id          TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
    filename            TEXT NOT NULL,
    mime_type           TEXT NOT NULL,
    size_bytes          INTEGER NOT NULL DEFAULT 0,
    content_id          TEXT,
    is_inline           INTEGER NOT NULL DEFAULT 0,
    sha256              TEXT
);

CREATE TABLE IF NOT EXISTS settings (
    key                 TEXT PRIMARY KEY NOT NULL,
    value               TEXT NOT NULL,
    updated_at          INTEGER NOT NULL
);

-- ============================================================================
-- High-Performance Covering Indexes for 50,000+ Thread Rendering at 120fps
-- ============================================================================

CREATE INDEX IF NOT EXISTS idx_threads_folder_time
    ON threads(folder_role, last_message_at DESC);

CREATE INDEX IF NOT EXISTS idx_threads_account_folder_time
    ON threads(account_id, folder_role, last_message_at DESC);

CREATE INDEX IF NOT EXISTS idx_threads_snoozed
    ON threads(snoozed_until) WHERE snoozed_until IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_messages_thread_time
    ON messages(thread_id, received_at ASC);

CREATE INDEX IF NOT EXISTS idx_messages_account_uid
    ON messages(account_id, folder_id, imap_uid);

CREATE INDEX IF NOT EXISTS idx_messages_msgid
    ON messages(message_id_header) WHERE message_id_header IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_messages_unfetched
    ON messages(account_id, is_body_fetched, received_at DESC) WHERE is_body_fetched = 0;

CREATE INDEX IF NOT EXISTS idx_attachments_message
    ON attachments(message_id);

-- ============================================================================
-- FTS5 Full-Text Search Virtual Table (Sub-20ms Local Search)
-- External-content table synced automatically via triggers on `messages`
-- ============================================================================

CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    subject,
    sender_name,
    sender_email,
    to_json,
    headers_text,
    body_text,
    content='messages',
    content_rowid='rowid',
    tokenize='unicode61 remove_diacritics 2'
);

-- Trigger: Automatically index newly inserted messages into FTS5
CREATE TRIGGER IF NOT EXISTS trg_messages_ai
AFTER INSERT ON messages
BEGIN
    INSERT INTO messages_fts(
        rowid, subject, sender_name, sender_email, to_json, headers_text, body_text
    )
    VALUES (
        new.rowid,
        new.subject,
        new.sender_name,
        new.sender_email,
        new.to_json,
        new.headers_text,
        new.body_text
    );
END;

-- Trigger: Automatically purge deleted messages from FTS5
CREATE TRIGGER IF NOT EXISTS trg_messages_ad
AFTER DELETE ON messages
BEGIN
    INSERT INTO messages_fts(
        messages_fts, rowid, subject, sender_name, sender_email, to_json, headers_text, body_text
    )
    VALUES (
        'delete',
        old.rowid,
        old.subject,
        old.sender_name,
        old.sender_email,
        old.to_json,
        old.headers_text,
        old.body_text
    );
END;

-- Trigger: Automatically re-index when body_text / headers arrive in Phase 2 sync
CREATE TRIGGER IF NOT EXISTS trg_messages_au
AFTER UPDATE OF subject, sender_name, sender_email, to_json, headers_text, body_text ON messages
BEGIN
    INSERT INTO messages_fts(
        messages_fts, rowid, subject, sender_name, sender_email, to_json, headers_text, body_text
    )
    VALUES (
        'delete',
        old.rowid,
        old.subject,
        old.sender_name,
        old.sender_email,
        old.to_json,
        old.headers_text,
        old.body_text
    );
    INSERT INTO messages_fts(
        rowid, subject, sender_name, sender_email, to_json, headers_text, body_text
    )
    VALUES (
        new.rowid,
        new.subject,
        new.sender_name,
        new.sender_email,
        new.to_json,
        new.headers_text,
        new.body_text
    );
END;

-- Default privacy-hardened settings (Local AI strictly points to loopback)
INSERT OR IGNORE INTO settings (key, value, updated_at) VALUES
    ('ai_enabled', 'true', strftime('%s', 'now')),
    ('ai_endpoint', 'http://localhost:11434', strftime('%s', 'now')),
    ('ai_model', 'llama3.2:latest', strftime('%s', 'now')),
    ('block_remote_images', 'true', strftime('%s', 'now')),
    ('strip_trackers', 'true', strftime('%s', 'now')),
    ('auto_verify_pgp', 'true', strftime('%s', 'now'));
