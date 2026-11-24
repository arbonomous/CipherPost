use serde::Serialize;
use thiserror::Error;

/// Unified error type across the CipherPost Rust backend.
#[derive(Debug, Error)]
pub enum CipherError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Keychain error: {0}")]
    Keychain(String),

    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("SMTP error: {0}")]
    Smtp(String),

    #[error("HTML sanitizer error: {0}")]
    Sanitizer(String),

    #[error("PGP error: {0}")]
    Pgp(String),

    #[error("Local AI error: {0}")]
    Ai(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("Network/TLS error: {0}")]
    Network(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

impl Serialize for CipherError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type CipherResult<T> = Result<T, CipherError>;
