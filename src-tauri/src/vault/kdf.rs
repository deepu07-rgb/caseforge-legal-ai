//! Argon2id key derivation — parameters MUST match contracts/constants.json → crypto.argon2id
//! Frozen via Q-DB-01 owner decision (OWASP 2023 interactive).

use super::error::{VaultError, VaultResult};
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use zeroize::Zeroizing;

/// Must match contracts/constants.json crypto.argon2id.params_version
pub const PARAMS_VERSION: u32 = 1;
/// Must match contracts/constants.json crypto.argon2id.memory_kib
pub const MEMORY_KIB: u32 = 65536;
/// Must match contracts/constants.json crypto.argon2id.iterations
pub const ITERATIONS: u32 = 3;
/// Must match contracts/constants.json crypto.argon2id.parallelism
pub const PARALLELISM: u32 = 4;
/// Must match contracts/constants.json crypto.argon2id.salt_len_bytes
pub const SALT_LEN: usize = 16;
/// Must match contracts/constants.json crypto.argon2id.key_len_bytes
pub const KEY_LEN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KdfSidecar {
    pub kdf: String,
    pub params_version: u32,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub salt_hex: String,
    pub key_len_bytes: usize,
}

impl KdfSidecar {
    pub fn new(salt: [u8; SALT_LEN]) -> Self {
        Self {
            kdf: "argon2id".into(),
            params_version: PARAMS_VERSION,
            memory_kib: MEMORY_KIB,
            iterations: ITERATIONS,
            parallelism: PARALLELISM,
            salt_hex: hex::encode(salt),
            key_len_bytes: KEY_LEN,
        }
    }

    pub fn load(path: &Path) -> VaultResult<Self> {
        let text = fs::read_to_string(path).map_err(|e| VaultError::Io {
            detail: format!("read kdf sidecar {}: {e}", path.display()),
        })?;
        let sc: Self = serde_json::from_str(&text).map_err(|e| VaultError::Corrupt {
            detail: format!("kdf sidecar parse: {e}"),
        })?;
        if sc.params_version != PARAMS_VERSION
            || sc.memory_kib != MEMORY_KIB
            || sc.iterations != ITERATIONS
            || sc.parallelism != PARALLELISM
            || sc.key_len_bytes != KEY_LEN
        {
            return Err(VaultError::Corrupt {
                detail: "kdf sidecar params do not match frozen contract params_version 1".into(),
            });
        }
        Ok(sc)
    }

    pub fn save(&self, path: &Path) -> VaultResult<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| VaultError::Internal {
            detail: e.to_string(),
        })?;
        fs::write(path, text)?;
        Ok(())
    }

    pub fn salt_bytes(&self) -> VaultResult<[u8; SALT_LEN]> {
        let v = hex::decode(&self.salt_hex).map_err(|e| VaultError::Corrupt {
            detail: format!("salt hex: {e}"),
        })?;
        if v.len() != SALT_LEN {
            return Err(VaultError::Corrupt {
                detail: format!("salt length {} != {SALT_LEN}", v.len()),
            });
        }
        let mut out = [0u8; SALT_LEN];
        out.copy_from_slice(&v);
        Ok(out)
    }
}

pub fn generate_salt() -> [u8; SALT_LEN] {
    let mut salt = [0u8; SALT_LEN];
    rand::thread_rng().fill_bytes(&mut salt);
    salt
}

pub fn derive_key(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
) -> VaultResult<Zeroizing<[u8; KEY_LEN]>> {
    if passphrase.is_empty() {
        return Err(VaultError::PassphraseRequired);
    }
    let params = Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, Some(KEY_LEN)).map_err(|e| {
        VaultError::Internal {
            detail: format!("argon2 params: {e}"),
        }
    })?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(passphrase.as_bytes(), salt, &mut *key)
        .map_err(|e| VaultError::Internal {
            detail: format!("argon2 derive: {e}"),
        })?;
    Ok(key)
}

pub fn load_or_create_kdf_sidecar(path: &Path) -> VaultResult<(KdfSidecar, bool)> {
    if path.exists() {
        Ok((KdfSidecar::load(path)?, false))
    } else {
        let sc = KdfSidecar::new(generate_salt());
        sc.save(path)?;
        Ok((sc, true))
    }
}
