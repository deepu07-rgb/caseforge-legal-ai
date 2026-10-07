use serde::Serialize;
use std::path::PathBuf;

pub type VaultResult<T> = Result<T, VaultError>;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "code", content = "detail")]
pub enum VaultError {
    #[serde(rename = "CF_VAULT_NOT_FOUND")]
    NotFound {
        path: PathBuf,
    },
    #[serde(rename = "CF_VAULT_ALREADY_EXISTS")]
    AlreadyExists {
        path: PathBuf,
    },
    #[serde(rename = "CF_VAULT_PASSPHRASE_REQUIRED")]
    PassphraseRequired,
    #[serde(rename = "CF_VAULT_PASSPHRASE_INVALID")]
    PassphraseInvalid,
    #[serde(rename = "CF_VAULT_LOCKED")]
    Locked,
    #[serde(rename = "CF_VAULT_CORRUPT")]
    Corrupt {
        detail: String,
    },
    #[serde(rename = "CF_VAULT_WRITE_FAILED")]
    Io {
        detail: String,
    },
    #[serde(rename = "CF_SYSTEM_INTERNAL")]
    Internal {
        detail: String,
    },
    #[serde(rename = "CF_VAULT_ALREADY_EXISTS")]
    AlreadyUnlocked,
    LockPoisoned,
    Sql {
        detail: String,
    },
}

impl std::fmt::Display for VaultError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VaultError::NotFound { path } => write!(f, "CF_VAULT_NOT_FOUND: {}", path.display()),
            VaultError::AlreadyExists { path } => {
                write!(f, "CF_VAULT_ALREADY_EXISTS: {}", path.display())
            }
            VaultError::PassphraseRequired => write!(f, "CF_VAULT_PASSPHRASE_REQUIRED"),
            VaultError::PassphraseInvalid => write!(
                f,
                "CF_VAULT_PASSPHRASE_INVALID: Wrong password for this vault. \
There is no recovery — use Reset local vault if you forgot it (deletes local case data)."
            ),
            VaultError::Locked => write!(f, "CF_VAULT_LOCKED"),
            VaultError::Corrupt { detail } => write!(f, "CF_VAULT_CORRUPT: {detail}"),
            VaultError::Io { detail } => write!(f, "CF_VAULT_WRITE_FAILED: {detail}"),
            VaultError::Internal { detail } => write!(f, "CF_SYSTEM_INTERNAL: {detail}"),
            VaultError::AlreadyUnlocked => write!(f, "CF_VAULT_ALREADY_EXISTS: already unlocked"),
            VaultError::LockPoisoned => write!(f, "CF_SYSTEM_INTERNAL: vault lock poisoned"),
            VaultError::Sql { detail } => write!(f, "CF_SYSTEM_INTERNAL: sql: {detail}"),
        }
    }
}

impl std::error::Error for VaultError {}

impl From<std::io::Error> for VaultError {
    fn from(e: std::io::Error) -> Self {
        VaultError::Io {
            detail: e.to_string(),
        }
    }
}

impl From<rusqlite::Error> for VaultError {
    fn from(e: rusqlite::Error) -> Self {
        VaultError::Sql {
            detail: e.to_string(),
        }
    }
}

impl From<VaultError> for String {
    fn from(e: VaultError) -> Self {
        e.to_string()
    }
}
