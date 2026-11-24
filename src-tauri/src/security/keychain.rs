use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use keyring::Entry;
use rand::RngCore;
use tracing::{info, warn};
use zeroize::Zeroizing;

use crate::error::{CipherError, CipherResult};

const KEYCHAIN_SERVICE: &str = "app.cipherpost.desktop";
const DB_MASTER_KEY_ENTRY: &str = "sqlcipher_master_key_v1";

/// Ephemeral in-memory store used strictly for unit tests or headless Linux environments
/// without an active D-Bus Secret Service daemon. On macOS, `apple-native` Keychain is
/// always used directly via Security.framework.
static EPHEMERAL_FALLBACK: LazyLock<Mutex<HashMap<String, Zeroizing<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn keychain_backend_name() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "macOS Keychain (Security.framework)"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "OS Keyring (Secret Service)"
    }
}

/// Retrieves the 256-bit SQLCipher master encryption key from the macOS Keychain,
/// generating and persisting a fresh cryptographically secure 32-byte key on first launch.
/// Never writes the key to disk, logs, or environment variables.
pub fn get_or_create_db_master_key() -> CipherResult<Zeroizing<String>> {
    match load_secret(DB_MASTER_KEY_ENTRY) {
        Ok(existing) if existing.len() == 64 => Ok(existing),
        Ok(_) => {
            warn!("Keychain master key had unexpected length; regenerating 256-bit key");
            generate_and_store_master_key()
        }
        Err(_) => generate_and_store_master_key(),
    }
}

fn generate_and_store_master_key() -> CipherResult<Zeroizing<String>> {
    let mut raw = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut raw);
    let hex_key = Zeroizing::new(hex::encode(raw));
    raw.iter_mut().for_each(|b| *b = 0);

    store_secret(DB_MASTER_KEY_ENTRY, &hex_key)?;
    info!("Provisioned new 256-bit SQLCipher master key in OS Keychain");
    Ok(hex_key)
}

/// Stores an account credential (OAuth2 token, App Password, Proton Bridge secret, or IMAP password)
/// in the native OS Keychain.
pub fn store_secret(secret_ref: &str, secret: &str) -> CipherResult<()> {
    match Entry::new(KEYCHAIN_SERVICE, secret_ref).and_then(|e| e.set_password(secret)) {
        Ok(()) => Ok(()),
        Err(err) => {
            #[cfg(target_os = "macos")]
            {
                Err(CipherError::Keychain(format!(
                    "Failed to store credential in macOS Keychain: {err}"
                )))
            }
            #[cfg(not(target_os = "macos"))]
            {
                warn!(
                    "OS keyring unavailable ({err}); using process-ephemeral memory store (non-persistent)"
                );
                let mut guard = EPHEMERAL_FALLBACK
                    .lock()
                    .map_err(|e| CipherError::Keychain(e.to_string()))?;
                guard.insert(secret_ref.to_string(), Zeroizing::new(secret.to_string()));
                Ok(())
            }
        }
    }
}

/// Loads a credential from the native OS Keychain wrapped in `Zeroizing<String>`
/// so plaintext secrets are zeroed from RAM upon drop.
pub fn load_secret(secret_ref: &str) -> CipherResult<Zeroizing<String>> {
    match Entry::new(KEYCHAIN_SERVICE, secret_ref).and_then(|e| e.get_password()) {
        Ok(password) => Ok(Zeroizing::new(password)),
        Err(err) => {
            #[cfg(not(target_os = "macos"))]
            {
                if let Ok(guard) = EPHEMERAL_FALLBACK.lock() {
                    if let Some(val) = guard.get(secret_ref) {
                        return Ok(val.clone());
                    }
                }
            }
            Err(CipherError::Keychain(format!(
                "Credential '{secret_ref}' not found in Keychain: {err}"
            )))
        }
    }
}

/// Removes an account credential from the OS Keychain when an account is deleted.
pub fn delete_secret(secret_ref: &str) -> CipherResult<()> {
    if let Ok(entry) = Entry::new(KEYCHAIN_SERVICE, secret_ref) {
        let _ = entry.delete_credential();
    }
    if let Ok(mut guard) = EPHEMERAL_FALLBACK.lock() {
        guard.remove(secret_ref);
    }
    Ok(())
}

/// Generates a deterministic, opaque Keychain reference key for an account ID.
pub fn account_secret_ref(account_id: &str) -> String {
    format!("account_secret:{account_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_secret_storage() {
        let key_ref = format!("test_secret_{}", uuid::Uuid::new_v4());
        store_secret(&key_ref, "super-secret-app-password").expect("store secret");
        let loaded = load_secret(&key_ref).expect("load secret");
        assert_eq!(loaded.as_str(), "super-secret-app-password");
        delete_secret(&key_ref).expect("delete secret");
    }
}
