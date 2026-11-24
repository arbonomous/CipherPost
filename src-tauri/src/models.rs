use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Gmail,
    Icloud,
    ProtonBridge,
    Custom,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gmail => "gmail",
            Self::Icloud => "icloud",
            Self::ProtonBridge => "proton_bridge",
            Self::Custom => "custom",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "gmail" => Self::Gmail,
            "icloud" => Self::Icloud,
            "proton_bridge" => Self::ProtonBridge,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthType {
    Password,
    Oauth2,
}

impl AuthType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::Oauth2 => "oauth2",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "oauth2" => Self::Oauth2,
            _ => Self::Password,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    Implicit,
    Starttls,
}

impl TlsMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Implicit => "implicit",
            Self::Starttls => "starttls",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "starttls" => Self::Starttls,
            _ => Self::Implicit,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PgpStatus {
    None,
    SignedVerified,
    SignedUnverified,
    Encrypted,
    EncryptedVerified,
}

impl PgpStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SignedVerified => "signed_verified",
            Self::SignedUnverified => "signed_unverified",
            Self::Encrypted => "encrypted",
            Self::EncryptedVerified => "encrypted_verified",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "signed_verified" => Self::SignedVerified,
            "signed_unverified" => Self::SignedUnverified,
            "encrypted" => Self::Encrypted,
            "encrypted_verified" => Self::EncryptedVerified,
            _ => Self::None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub email: String,
    pub display_name: String,
    pub provider: ProviderKind,
    pub auth_type: AuthType,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: TlsMode,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: TlsMode,
    pub accent_color: String,
    pub sync_status: String,
    pub last_synced_at: Option<i64>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAccountInput {
    pub email: String,
    pub display_name: String,
    pub provider: ProviderKind,
    pub auth_type: AuthType,
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_tls: TlsMode,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_tls: TlsMode,
    /// Password, App Password, Proton Bridge Password, or OAuth2 Access Token.
    /// Immediately persisted to macOS Keychain and never stored in SQLite.
    pub secret: String,
    pub accent_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsHandshakeStep {
    pub step: String,
    pub status: String, // "passed" | "failed" | "skipped"
    pub detail: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestResult {
    pub success: bool,
    pub imap_tls_version: Option<String>,
    pub imap_capabilities: Vec<String>,
    pub smtp_verified: bool,
    pub steps: Vec<TlsHandshakeStep>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockedTracker {
    pub url: String,
    pub domain: String,
    pub category: String, // "spy_pixel" | "tracker_domain" | "css_beacon" | "script_injection" | "remote_stylesheet"
    pub reason: String,
    pub element: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentMeta {
    pub id: String,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: i64,
    pub is_inline: bool,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    pub account_id: String,
    pub folder_role: String,
    pub subject: String,
    pub snippet: String,
    pub participants: Vec<EmailAddress>,
    pub last_sender_name: String,
    pub last_sender_email: String,
    pub message_count: i64,
    pub unread_count: i64,
    pub is_starred: bool,
    pub has_attachments: bool,
    pub pgp_status: PgpStatus,
    pub trackers_blocked: i64,
    pub snoozed_until: Option<i64>,
    pub last_message_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageDetail {
    pub id: String,
    pub thread_id: String,
    pub account_id: String,
    pub folder_role: String,
    pub message_id_header: Option<String>,
    pub subject: String,
    pub sender_name: String,
    pub sender_email: String,
    pub to: Vec<EmailAddress>,
    pub cc: Vec<EmailAddress>,
    pub body_text: String,
    pub body_html_sanitized: String,
    pub body_html_with_assets: String,
    pub blocked_trackers: Vec<BlockedTracker>,
    pub trackers_blocked_count: i64,
    pub remote_assets_count: i64,
    pub pgp_status: PgpStatus,
    pub pgp_fingerprint: Option<String>,
    pub pgp_signer: Option<String>,
    pub is_read: bool,
    pub is_starred: bool,
    pub is_body_fetched: bool,
    pub size_bytes: i64,
    pub received_at: i64,
    pub attachments: Vec<AttachmentMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadDetail {
    pub thread: ThreadSummary,
    pub messages: Vec<MessageDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub thread_id: String,
    pub message_id: String,
    pub account_id: String,
    pub folder_role: String,
    pub subject: String,
    pub sender_name: String,
    pub sender_email: String,
    pub highlighted_snippet: String,
    pub received_at: i64,
    pub pgp_status: PgpStatus,
    pub trackers_blocked: i64,
    pub rank: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub query: String,
    pub query_time_ms: f64,
    pub total_hits: usize,
    pub results: Vec<SearchResultItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderCounts {
    pub inbox_unread: i64,
    pub inbox_total: i64,
    pub sent_total: i64,
    pub drafts_total: i64,
    pub snoozed_total: i64,
    pub archive_total: i64,
    pub trash_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivacyStats {
    pub total_trackers_blocked: i64,
    pub total_messages_indexed: i64,
    pub pgp_verified_count: i64,
    pub sqlcipher_active: bool,
    pub keychain_backend: String,
    pub fts5_ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComposePayload {
    pub account_id: String,
    pub to: Vec<String>,
    pub cc: Vec<String>,
    pub bcc: Vec<String>,
    pub subject: String,
    pub body_text: String,
    pub in_reply_to: Option<String>,
    pub thread_id: Option<String>,
    pub pgp_sign: bool,
    pub pgp_encrypt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub ai_enabled: bool,
    pub ai_endpoint: String,
    pub ai_model: String,
    pub block_remote_images: bool,
    pub strip_trackers: bool,
    pub auto_verify_pgp: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSummaryResponse {
    pub thread_id: String,
    pub bullets: Vec<String>,
    pub action_required: bool,
    pub model_used: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiDraftResponse {
    pub draft_text: String,
    pub model_used: String,
    pub latency_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiHealthResponse {
    pub reachable: bool,
    pub endpoint: String,
    pub available_models: Vec<String>,
    pub latency_ms: u64,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncEventPayload {
    pub account_id: String,
    pub phase: String, // "headers" | "bodies" | "idle" | "error"
    pub synced_count: usize,
    pub detail: String,
}
