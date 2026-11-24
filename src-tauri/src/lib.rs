pub mod ai;
pub mod commands;
pub mod crypto;
pub mod db;
pub mod error;
pub mod mail;
pub mod models;
pub mod sanitizer;
pub mod security;

use std::sync::Arc;

use tauri::{Emitter, Manager};
use tracing::{error, info};

use crate::commands::AppState;
use crate::db::{seed, Database};
use crate::mail::imap_worker;
use crate::security::keychain;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cipherpost_lib=info,warn".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("resolve Tauri app data directory");
            let db_path = app_data_dir.join("cipherpost_vault.db");

            // Retrieve or provision the 256-bit SQLCipher master key strictly in OS Keychain
            let master_key = keychain::get_or_create_db_master_key()
                .expect("provision SQLCipher master key in Keychain");

            let database = Arc::new(
                Database::open(&db_path, &master_key)
                    .expect("open SQLCipher encrypted local vault"),
            );

            if let Err(e) = seed::seed_demo_vault_if_empty(&database) {
                error!("Failed seeding initial local vault: {e}");
            }

            // Spawn background IMAP IDLE push listeners for all configured real accounts
            if let Ok(accounts) = database.list_accounts() {
                for acc in accounts {
                    if acc.id.starts_with("acc-proton-vault") || acc.id.starts_with("acc-work-gmail") {
                        continue; // Demo preview accounts don't dial external servers
                    }
                    let db_clone = Arc::clone(&database);
                    let handle = app.handle().clone();
                    imap_worker::spawn_imap_idle_listener(db_clone, acc, move |payload| {
                        let _ = handle.emit("cipherpost://sync", payload);
                    });
                }
            }

            info!("CipherPost local-first engine booted at {}", db_path.display());
            app.manage(AppState { db: database });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::start_gmail_oauth,
            commands::list_accounts,
            commands::test_account_connection,
            commands::add_account,
            commands::remove_account,
            commands::sync_account_now,
            commands::list_threads,
            commands::get_thread_detail,
            commands::search_messages,
            commands::archive_threads,
            commands::trash_threads,
            commands::move_threads_to_inbox,
            commands::snooze_threads,
            commands::set_threads_read,
            commands::toggle_thread_star,
            commands::send_message,
            commands::get_folder_counts,
            commands::get_privacy_stats,
            commands::get_settings,
            commands::save_settings,
            commands::check_local_ai,
            commands::ai_summarize_thread,
            commands::ai_draft_reply,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CipherPost");
}
