use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::ai;
use crate::db::Database;
use crate::error::CipherResult;
use crate::mail::{imap_worker, smtp_client};
use crate::models::{
    Account, AiDraftResponse, AiHealthResponse, AiSummaryResponse, AppSettings, ComposePayload,
    ConnectionTestResult, FolderCounts, NewAccountInput, PgpStatus, PrivacyStats, SearchResponse,
    ThreadDetail, ThreadSummary,
};
use crate::security::keychain;

pub struct AppState {
    pub db: Arc<Database>,
}

#[tauri::command]
pub fn list_accounts(state: State<'_, AppState>) -> CipherResult<Vec<Account>> {
    state.db.list_accounts()
}

#[tauri::command]
pub async fn test_account_connection(input: NewAccountInput) -> ConnectionTestResult {
    imap_worker::test_account_connection(&input).await
}

#[tauri::command]
pub fn add_account(
    app: AppHandle,
    state: State<'_, AppState>,
    input: NewAccountInput,
) -> CipherResult<Account> {
    let account_id = uuid::Uuid::new_v4().to_string();
    let secret_ref = keychain::account_secret_ref(&account_id);

    // Persist secret strictly in macOS Keychain — never in SQLite or disk files
    if !input.secret.is_empty() {
        keychain::store_secret(&secret_ref, &input.secret)?;
    }

    let account = state.db.create_account(&account_id, &input, &secret_ref)?;

    // Launch background IMAP IDLE push listener for the new account
    let db_clone = Arc::clone(&state.db);
    let app_clone = app.clone();
    imap_worker::spawn_imap_idle_listener(db_clone, account.clone(), move |payload| {
        let _ = app_clone.emit("cipherpost://sync", payload);
    });

    Ok(account)
}

#[tauri::command]
pub fn remove_account(state: State<'_, AppState>, account_id: String) -> CipherResult<()> {
    let secret_ref = keychain::account_secret_ref(&account_id);
    let _ = keychain::delete_secret(&secret_ref);
    state.db.delete_account(&account_id)
}

#[tauri::command]
pub async fn sync_account_now(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
) -> CipherResult<usize> {
    let account = state.db.get_account(&account_id)?;
    let app_clone = app.clone();
    imap_worker::sync_account_once(Arc::clone(&state.db), &account, move |payload| {
        let _ = app_clone.emit("cipherpost://sync", payload);
    })
    .await
}

#[tauri::command]
pub fn list_threads(
    state: State<'_, AppState>,
    account_id: Option<String>,
    folder_role: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CipherResult<Vec<ThreadSummary>> {
    state.db.list_threads(
        account_id.as_deref(),
        &folder_role,
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
}

#[tauri::command]
pub fn get_thread_detail(
    state: State<'_, AppState>,
    thread_id: String,
) -> CipherResult<ThreadDetail> {
    state.db.get_thread_detail(&thread_id)
}

#[tauri::command]
pub fn search_messages(
    state: State<'_, AppState>,
    query: String,
    account_id: Option<String>,
    limit: Option<i64>,
) -> CipherResult<SearchResponse> {
    state
        .db
        .search_fts5(&query, account_id.as_deref(), limit.unwrap_or(40))
}

#[tauri::command]
pub fn archive_threads(state: State<'_, AppState>, thread_ids: Vec<String>) -> CipherResult<usize> {
    state.db.move_threads_to_folder(&thread_ids, "archive")
}

#[tauri::command]
pub fn trash_threads(state: State<'_, AppState>, thread_ids: Vec<String>) -> CipherResult<usize> {
    state.db.move_threads_to_folder(&thread_ids, "trash")
}

#[tauri::command]
pub fn move_threads_to_inbox(
    state: State<'_, AppState>,
    thread_ids: Vec<String>,
) -> CipherResult<usize> {
    state.db.move_threads_to_folder(&thread_ids, "inbox")
}

#[tauri::command]
pub fn snooze_threads(
    state: State<'_, AppState>,
    thread_ids: Vec<String>,
    until_ts: i64,
) -> CipherResult<usize> {
    state.db.snooze_threads(&thread_ids, until_ts)
}

#[tauri::command]
pub fn set_threads_read(
    state: State<'_, AppState>,
    thread_ids: Vec<String>,
    is_read: bool,
) -> CipherResult<usize> {
    state.db.set_threads_read(&thread_ids, is_read)
}

#[tauri::command]
pub fn toggle_thread_star(state: State<'_, AppState>, thread_id: String) -> CipherResult<bool> {
    state.db.toggle_thread_star(&thread_id)
}

#[tauri::command]
pub async fn send_message(
    state: State<'_, AppState>,
    payload: ComposePayload,
) -> CipherResult<String> {
    let account = state.db.get_account(&payload.account_id)?;
    let msg_id_header = smtp_client::send_email_direct(&account, &payload).await?;

    let pgp_status = match (payload.pgp_encrypt, payload.pgp_sign) {
        (true, true) => PgpStatus::EncryptedVerified,
        (true, false) => PgpStatus::Encrypted,
        (false, true) => PgpStatus::SignedVerified,
        (false, false) => PgpStatus::None,
    };

    state.db.record_outbound_message(
        &account,
        &msg_id_header,
        payload.thread_id.as_deref(),
        &payload.to,
        &payload.cc,
        &payload.subject,
        &payload.body_text,
        pgp_status,
    )
}

#[tauri::command]
pub fn get_folder_counts(
    state: State<'_, AppState>,
    account_id: Option<String>,
) -> CipherResult<FolderCounts> {
    state.db.get_folder_counts(account_id.as_deref())
}

#[tauri::command]
pub fn get_privacy_stats(state: State<'_, AppState>) -> CipherResult<PrivacyStats> {
    state.db.get_privacy_stats()
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> CipherResult<AppSettings> {
    state.db.get_settings()
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> CipherResult<AppSettings> {
    state.db.save_settings(&settings)
}

#[tauri::command]
pub async fn check_local_ai(state: State<'_, AppState>, endpoint: Option<String>) -> CipherResult<AiHealthResponse> {
    let ep = match endpoint {
        Some(e) if !e.trim().is_empty() => e,
        _ => state.db.get_settings()?.ai_endpoint,
    };
    ai::check_local_ai_health(&ep).await
}

#[tauri::command]
pub async fn ai_summarize_thread(
    state: State<'_, AppState>,
    thread_id: String,
) -> CipherResult<AiSummaryResponse> {
    let settings = state.db.get_settings()?;
    let detail = state.db.get_thread_detail(&thread_id)?;
    let transcript = detail
        .messages
        .iter()
        .map(|m| format!("From {} <{}>:\n{}", m.sender_name, m.sender_email, m.body_text))
        .collect::<Vec<_>>()
        .join("\n---\n");

    ai::summarize_thread_local(
        &settings.ai_endpoint,
        &settings.ai_model,
        &thread_id,
        &detail.thread.subject,
        &transcript,
    )
    .await
}

#[tauri::command]
pub async fn ai_draft_reply(
    state: State<'_, AppState>,
    thread_id: String,
    instruction: Option<String>,
) -> CipherResult<AiDraftResponse> {
    let settings = state.db.get_settings()?;
    let detail = state.db.get_thread_detail(&thread_id)?;
    let transcript = detail
        .messages
        .iter()
        .map(|m| format!("From {} <{}>:\n{}", m.sender_name, m.sender_email, m.body_text))
        .collect::<Vec<_>>()
        .join("\n---\n");

    ai::draft_reply_local(
        &settings.ai_endpoint,
        &settings.ai_model,
        &detail.thread.subject,
        &transcript,
        instruction.as_deref(),
    )
    .await
}
