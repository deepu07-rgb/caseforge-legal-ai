use super::error::{VaultError, VaultResult};
use rusqlite::Connection;
use std::path::Path;
use zeroize::Zeroizing;

/// Create a new SQLCipher database with raw 256-bit key.
pub fn create_encrypted(path: &Path, key: &Zeroizing<[u8; 32]>) -> VaultResult<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if path.exists() {
        return Err(VaultError::AlreadyExists {
            path: path.to_path_buf(),
        });
    }
    let conn = Connection::open(path)?;
    apply_key(&conn, key)?;
    // Force cipher settings consistent with AES-256 page encryption (SQLCipher defaults).
    conn.pragma_update(None, "cipher_page_size", 4096)?;
    conn.pragma_update(None, "kdf_iter", 1)?; // raw key already Argon2id-derived; avoid double-PBKDF
    Ok(conn)
}

/// Open existing SQLCipher database with raw key.
pub fn open_encrypted(path: &Path, key: &Zeroizing<[u8; 32]>) -> VaultResult<Connection> {
    if !path.exists() {
        return Err(VaultError::NotFound {
            path: path.to_path_buf(),
        });
    }
    let conn = Connection::open(path)?;
    apply_key(&conn, key)?;
    // Validate key by touching schema.
    conn.query_row("SELECT count(*) FROM sqlite_master", [], |r| {
        r.get::<_, i64>(0)
    })?;
    Ok(conn)
}

fn apply_key(conn: &Connection, key: &Zeroizing<[u8; 32]>) -> VaultResult<()> {
    // SQLCipher raw key: x'<64 hex chars>'
    let hexkey = hex::encode(**key);
    let pragma = format!("x'{hexkey}'");
    conn.pragma_update(None, "key", pragma)?;
    Ok(())
}

pub fn migrate(conn: &Connection, sql: &str) -> VaultResult<()> {
    conn.execute_batch(sql)?;
    Ok(())
}

/// Apply pending numbered migrations. Each SQL file must insert its version into schema_migrations.
pub fn apply_pending_migrations(conn: &Connection, migrations: &[(i64, &str)]) -> VaultResult<()> {
    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    for (version, sql) in migrations {
        if *version > current {
            conn.execute_batch(sql)?;
        }
    }
    Ok(())
}

pub fn set_meta(conn: &Connection, key: &str, value: &str) -> VaultResult<()> {
    conn.execute(
        "INSERT INTO vault_meta (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )?;
    Ok(())
}

pub fn current_migration_version(conn: &Connection) -> VaultResult<i64> {
    let v: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |r| r.get(0),
    )?;
    Ok(v)
}
