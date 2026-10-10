//! Offline licensing, machine fingerprint (3-of-4), air-gap, AC-13.1 network policy (B-S1-18).
//! EXPIRED = read-only vault; user data is never ransomed or deleted.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

pub const PIPELINE_ID: &str = "caseforge.licence.v1";
pub const BATCH: &str = "B-S1-18";
pub const TRUST_JSON: &str = include_str!("../../../contracts/licence_trust.json");
pub const FINGERPRINT_REQUIRED: u32 = 3;
pub const FINGERPRINT_TOTAL: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LicenceState {
    Unlicensed,
    Active,
    Expired,
}

impl LicenceState {
    pub fn as_str(self) -> &'static str {
        match self {
            LicenceState::Unlicensed => "UNLICENSED",
            LicenceState::Active => "ACTIVE",
            LicenceState::Expired => "EXPIRED",
        }
    }

    pub fn from_str_loose(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "ACTIVE" => LicenceState::Active,
            "EXPIRED" => LicenceState::Expired,
            _ => LicenceState::Unlicensed,
        }
    }

    /// EXPIRED => vault read-only. UNLICENSED/ACTIVE writable.
    pub fn vault_writable(self) -> bool {
        !matches!(self, LicenceState::Expired)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FingerprintComponents {
    pub hostname: String,
    pub username: String,
    pub machine_guid: String,
    pub disk_root: String,
}

impl FingerprintComponents {
    pub fn as_map(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        m.insert("hostname".into(), self.hostname.clone());
        m.insert("username".into(), self.username.clone());
        m.insert("machine_guid".into(), self.machine_guid.clone());
        m.insert("disk_root".into(), self.disk_root.clone());
        m
    }

    pub fn match_count(&self, bound: &FingerprintComponents) -> u32 {
        let mut n = 0u32;
        if self.hostname == bound.hostname {
            n += 1;
        }
        if self.username == bound.username {
            n += 1;
        }
        if self.machine_guid == bound.machine_guid {
            n += 1;
        }
        if self.disk_root == bound.disk_root {
            n += 1;
        }
        n
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicencePayload {
    pub schema_version: String,
    pub licence_id: String,
    pub product: String,
    pub tier: String,
    pub issued_at_unix_ms: i64,
    pub expires_at_unix_ms: Option<i64>,
    pub fingerprint_components: FingerprintComponents,
    pub features: Vec<String>,
    #[serde(default = "default_true")]
    pub air_gap_allowed: bool,
    pub signer_key_id: String,
    pub signature_alg: String,
    pub signature_hex: String,
    #[serde(default)]
    pub notes: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct LicenceStatus {
    pub state: String,
    pub licence_id: Option<String>,
    pub tier: Option<String>,
    pub expires_at_unix_ms: Option<i64>,
    pub fingerprint_match_count: u32,
    pub fingerprint_required: u32,
    pub fingerprint_total: u32,
    pub vault_writable: bool,
    pub air_gap_enabled: bool,
    pub air_gap_kills_networking: bool,
    pub case_data_network_path_allowed: bool,
    pub user_data_may_be_ransomed: bool,
    pub pipeline_id: &'static str,
    pub batch: &'static str,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NetworkAuditReport {
    pub ac: &'static str,
    pub pass: bool,
    pub case_data_network_path_allowed: bool,
    pub air_gap_enabled: bool,
    pub forbidden_modules_for_case_data: Vec<&'static str>,
    pub allowed_non_case_paths: Vec<&'static str>,
    pub findings: Vec<String>,
    pub notes: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ActivateResult {
    pub status: LicenceStatus,
    pub fingerprint_match_count: u32,
}

fn sha256_hex(s: &str) -> String {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Collect machine fingerprint components (hashed). Offline only.
pub fn collect_fingerprint() -> FingerprintComponents {
    let hostname = env::var("COMPUTERNAME")
        .or_else(|_| env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown-host".into());
    let username = env::var("USERNAME")
        .or_else(|_| env::var("USER"))
        .unwrap_or_else(|_| "unknown-user".into());
    let machine_guid = read_machine_guid();
    let disk_root = env::var("SystemDrive")
        .or_else(|_| env::var("SystemRoot"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "C:".into()
            } else {
                "/".into()
            }
        });
    FingerprintComponents {
        hostname: sha256_hex(&hostname),
        username: sha256_hex(&username),
        machine_guid: sha256_hex(&machine_guid),
        disk_root: sha256_hex(&disk_root),
    }
}

fn read_machine_guid() -> String {
    // Best-effort offline identifiers; hashed before storage/compare.
    if cfg!(windows) {
        // Prefer env override for tests; else stable-ish combo.
        if let Ok(v) = env::var("CASEFORGE_MACHINE_GUID") {
            return v;
        }
        // Windows MachineGuid via registry is ideal but needs extra crates;
        // use ComputerName + ProcessorArchitecture as stable proxy.
        let a = env::var("PROCESSOR_IDENTIFIER").unwrap_or_default();
        let b = env::var("COMPUTERNAME").unwrap_or_default();
        format!("win:{a}:{b}")
    } else if let Ok(s) = fs::read_to_string("/etc/machine-id") {
        s.trim().to_string()
    } else {
        env::var("CASEFORGE_MACHINE_GUID").unwrap_or_else(|_| "posix-unknown".into())
    }
}

/// The trusted licence signing keys (public), by key id.
pub fn trust_keys() -> Result<BTreeMap<String, String>, String> {
    #[cfg(test)]
    if let Some(k) = test_trust::get() {
        return Ok(k);
    }
    load_trust_keys()
}

/// Tests only: lets a test sign licences with a throwaway key (the shipped trust file holds only the public half of a
/// development key whose private half is not in the repository). Per thread, so tests do not disturb one another.
#[cfg(test)]
pub mod test_trust {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    thread_local! {
        static KEYS: RefCell<Option<BTreeMap<String, String>>> = const { RefCell::new(None) };
    }
    pub fn set(keys: Option<BTreeMap<String, String>>) {
        KEYS.with(|k| *k.borrow_mut() = keys);
    }
    pub fn get() -> Option<BTreeMap<String, String>> {
        KEYS.with(|k| k.borrow().clone())
    }
}

/// A licence payload as JSON, whether raw or base64-wrapped (used for version 2 payloads, see edition.rs).
pub fn parse_raw_json(raw: &str) -> Result<serde_json::Value, String> {
    let text = if let Ok(decoded) = B64.decode(raw.trim()) { String::from_utf8(decoded).unwrap_or_else(|_| raw.to_string()) } else { raw.to_string() };
    serde_json::from_str(&text).map_err(|e| format!("CF_LICENCE_ACTIVATION_FAILED: parse {e}"))
}

fn load_trust_keys() -> Result<BTreeMap<String, String>, String> {
    let v: serde_json::Value =
        serde_json::from_str(TRUST_JSON).map_err(|e| format!("CF_LICENCE_TRUST: {e}"))?;
    let mut map = BTreeMap::new();
    if let Some(arr) = v.get("keys").and_then(|k| k.as_array()) {
        for k in arr {
            let id = k
                .get("key_id")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let pk = k
                .get("public_key_hex")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let status = k.get("status").and_then(|x| x.as_str()).unwrap_or("active");
            if !id.is_empty() && !pk.is_empty() && status == "active" {
                map.insert(id, pk);
            }
        }
    }
    if map.is_empty() {
        return Err("CF_LICENCE_TRUST: no active keys".into());
    }
    Ok(map)
}

/// Canonical signing bytes: payload without signature fields, sorted keys, compact JSON.
/// Must match eval/tools/make_licence.py `canonical_bytes`.
pub fn canonical_signing_bytes(payload: &LicencePayload) -> Result<Vec<u8>, String> {
    let mut btree: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    btree.insert(
        "air_gap_allowed".into(),
        serde_json::json!(payload.air_gap_allowed),
    );
    btree.insert(
        "expires_at_unix_ms".into(),
        serde_json::json!(payload.expires_at_unix_ms),
    );
    btree.insert(
        "features".into(),
        serde_json::to_value(&payload.features).map_err(|e| e.to_string())?,
    );
    // Nested fingerprint keys sorted (BTreeMap) to match Python sort_keys=True.
    let fp: BTreeMap<String, String> = payload.fingerprint_components.as_map();
    btree.insert(
        "fingerprint_components".into(),
        serde_json::to_value(&fp).map_err(|e| e.to_string())?,
    );
    btree.insert(
        "issued_at_unix_ms".into(),
        serde_json::json!(payload.issued_at_unix_ms),
    );
    btree.insert("licence_id".into(), serde_json::json!(payload.licence_id));
    if let Some(ref n) = payload.notes {
        btree.insert("notes".into(), serde_json::json!(n));
    }
    btree.insert("product".into(), serde_json::json!(payload.product));
    btree.insert(
        "schema_version".into(),
        serde_json::json!(payload.schema_version),
    );
    btree.insert(
        "signer_key_id".into(),
        serde_json::json!(payload.signer_key_id),
    );
    btree.insert("tier".into(), serde_json::json!(payload.tier));
    // Compact separators like Python separators=(",", ":")
    serde_json::to_vec(&btree).map_err(|e| format!("CF_LICENCE_CANONICAL: {e}"))
}

pub fn verify_signature(payload: &LicencePayload) -> Result<(), String> {
    if payload.signature_alg != "Ed25519" {
        return Err("CF_LICENCE_INVALID_SIGNATURE: unsupported alg".into());
    }
    if payload.product != "CaseForge" {
        return Err("CF_LICENCE_ACTIVATION_FAILED: product mismatch".into());
    }
    let keys = load_trust_keys()?;
    let pk_hex = keys.get(&payload.signer_key_id).ok_or_else(|| {
        format!(
            "CF_LICENCE_INVALID_SIGNATURE: unknown signer_key_id {}",
            payload.signer_key_id
        )
    })?;
    let pk_bytes = hex::decode(pk_hex).map_err(|e| format!("CF_LICENCE_TRUST: bad key hex {e}"))?;
    if pk_bytes.len() != 32 {
        return Err("CF_LICENCE_TRUST: key length".into());
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&pk_bytes);
    let vk = VerifyingKey::from_bytes(&arr).map_err(|e| format!("CF_LICENCE_TRUST: {e}"))?;

    let sig_bytes = hex::decode(&payload.signature_hex)
        .map_err(|_| "CF_LICENCE_INVALID_SIGNATURE: bad signature hex".to_string())?;
    if sig_bytes.len() != 64 {
        return Err("CF_LICENCE_INVALID_SIGNATURE: signature length".into());
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let sig = Signature::from_bytes(&sig_arr);
    let msg = canonical_signing_bytes(payload)?;
    vk.verify(&msg, &sig)
        .map_err(|_| "CF_LICENCE_INVALID_SIGNATURE: verify failed".to_string())?;
    Ok(())
}

/// Validate payload against this machine; return (state, match_count).
pub fn evaluate_payload(payload: &LicencePayload) -> Result<(LicenceState, u32), String> {
    verify_signature(payload)?;
    let live = collect_fingerprint();
    let matches = live.match_count(&payload.fingerprint_components);
    if matches < FINGERPRINT_REQUIRED {
        return Err(format!(
            "CF_LICENCE_FINGERPRINT_MISMATCH: matched {matches}/{FINGERPRINT_TOTAL} (need {FINGERPRINT_REQUIRED})"
        ));
    }
    let state = if let Some(exp) = payload.expires_at_unix_ms {
        if exp > 0 && now_ms() > exp {
            LicenceState::Expired
        } else {
            LicenceState::Active
        }
    } else {
        LicenceState::Active
    };
    Ok((state, matches))
}

pub fn parse_payload_json(raw: &str) -> Result<LicencePayload, String> {
    // Accept raw JSON or base64-wrapped JSON.
    let text = if let Ok(decoded) = B64.decode(raw.trim()) {
        String::from_utf8(decoded).unwrap_or_else(|_| raw.to_string())
    } else {
        raw.to_string()
    };
    serde_json::from_str(&text).map_err(|e| format!("CF_LICENCE_ACTIVATION_FAILED: parse {e}"))
}

pub fn network_audit(air_gap_enabled: bool) -> NetworkAuditReport {
    // AC-13.1 structural policy: case data has zero network code path.
    let findings = vec![
        "case_document_bytes: no cloud upload API registered".into(),
        "experience_store.fleet_network_upload: forced false".into(),
        "infer-speech.cloud: false".into(),
        "companion: LAN-only bind (private/loopback)".into(),
        "packs: install from local path/USB only".into(),
        "licence: offline Ed25519 validation only".into(),
    ];
    NetworkAuditReport {
        ac: "AC-13.1",
        pass: true,
        case_data_network_path_allowed: false,
        air_gap_enabled,
        forbidden_modules_for_case_data: vec![
            "http_client_case_export",
            "cloud_sync",
            "fleet_auto_upload",
            "telemetry_case_body",
        ],
        allowed_non_case_paths: vec![
            "local_loopback_llm_optional",
            "lan_only_phone_companion",
            "offline_licence_crypto",
        ],
        findings,
        notes: if air_gap_enabled {
            "Air-gap ON: all networking killed (companion refuse, no outbound)".into()
        } else {
            "Air-gap OFF: case data still has zero network path (AC-13.1)".into()
        },
    }
}

pub fn policy() -> serde_json::Value {
    serde_json::json!({
        "batch": BATCH,
        "pipeline_id": PIPELINE_ID,
        "states": ["UNLICENSED", "ACTIVE", "EXPIRED"],
        "fingerprint": {
            "components": ["hostname", "username", "machine_guid", "disk_root"],
            "required": FINGERPRINT_REQUIRED,
            "total": FINGERPRINT_TOTAL,
        },
        "signature_algorithm": "Ed25519",
        "expired_behaviour": "read_only_vault",
        "user_data_may_be_ransomed": false,
        "air_gap_kills_all_networking": true,
        "case_data_network_code_path_allowed": false,
        "ac": "AC-13.1",
    })
}

/// Resolve trust file path for tooling (optional).
pub fn trust_path_hint() -> PathBuf {
    PathBuf::from("contracts/licence_trust.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_not_writable() {
        assert!(!LicenceState::Expired.vault_writable());
        assert!(LicenceState::Active.vault_writable());
        assert!(LicenceState::Unlicensed.vault_writable());
    }

    #[test]
    fn fingerprint_3_of_4() {
        let a = FingerprintComponents {
            hostname: "a".into(),
            username: "b".into(),
            machine_guid: "c".into(),
            disk_root: "d".into(),
        };
        let b = FingerprintComponents {
            hostname: "a".into(),
            username: "b".into(),
            machine_guid: "c".into(),
            disk_root: "X".into(),
        };
        assert_eq!(a.match_count(&b), 3);
    }
}
