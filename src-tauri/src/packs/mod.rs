//! Knowledge pack manager (B-S1-12) — official Ed25519 .akpkg + custom isolation.
//! AC-7.1: official pipeline headnote scan must return zero matches.

use crate::vault::VaultPaths;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub const MAX_PACK_FILES: usize = 4096;
pub const MAX_PACK_FILE_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_PACK_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
pub const MAX_PACK_DEPTH: usize = 16;
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

static PACK_ID_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9][a-z0-9._-]{2,127}$").expect("pack id regex"));
static VERSION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9.-]+)?$").expect("version regex")
});
static DATE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[0-9]{4}-(0[1-9]|1[0-2])-([0-2][0-9]|3[01])$").expect("date regex"));

fn valid_authority(value: &str) -> bool {
    matches!(
        value,
        "CONSTITUTION"
            | "STATUTE"
            | "REGULATION"
            | "RULE"
            | "JUDGMENT"
            | "TREATY"
            | "OFFICIAL_GUIDANCE"
            | "SECONDARY"
            | "CUSTOM"
            | "UNKNOWN"
    )
}

fn valid_date_or_unknown(value: &str) -> bool {
    value == "UNKNOWN" || DATE_RE.is_match(value)
}

fn valid_supersession(value: &str) -> bool {
    matches!(
        value,
        "CURRENT" | "SUPERSEDED" | "PARTIALLY_SUPERSEDED" | "UNKNOWN"
    )
}

fn valid_temporal(value: &str) -> bool {
    matches!(
        value,
        "APPLICABLE" | "NOT_YET_EFFECTIVE" | "EXPIRED" | "UNKNOWN"
    )
}

fn valid_content_kind(value: &str) -> bool {
    matches!(value, "AUTHORITATIVE_TEXT" | "DERIVED_SUMMARY")
}

const HEADNOTE_PATTERNS: &[&str] = &[
    "headnote",
    "head note",
    "editorial headnote",
    "held:",
    "ratio decidendi summary",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    pub schema_version: String,
    pub pack_id: String,
    pub pack_kind: String,
    pub jurisdiction: Option<String>,
    pub version: String,
    pub display_name: String,
    pub description: Option<String>,
    pub content_sha256: String,
    pub files: Vec<PackFileEntry>,
    pub headnotes_stripped: bool,
    pub signature_alg: Option<String>,
    pub signer_key_id: Option<String>,
    #[serde(default)]
    pub metadata_version: Option<String>,
    #[serde(default)]
    pub jurisdiction_path: Vec<String>,
    #[serde(default)]
    pub trust_level: Option<String>,
    #[serde(default)]
    pub controlled_beta_eligible: bool,
    #[serde(default)]
    pub authoritative_content_sha256: Option<String>,
    #[serde(default)]
    pub sources_file: Option<String>,
    #[serde(default)]
    pub chunks_file: Option<String>,
    #[serde(default)]
    pub migration: Option<serde_json::Value>,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub source_status: Option<String>,
    #[serde(default)]
    pub supersedes_version: Option<String>,
    #[serde(default)]
    pub superseded_by_version: Option<String>,
    #[serde(default)]
    pub editorial_review_status: Option<String>,
    #[serde(default)]
    pub rights_basis: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackFileEntry {
    pub path: String,
    pub sha256: String,
    pub bytes: Option<u64>,
    #[serde(default)]
    pub role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledPack {
    pub pack_id: String,
    pub pack_kind: String,
    pub jurisdiction: Option<String>,
    pub version: String,
    pub display_name: String,
    pub install_path: String,
    pub content_sha256: String,
    pub headnotes_stripped: bool,
    pub signature_ok: bool,
    pub installed_at_unix_ms: i64,
    pub isolated: bool,
    #[serde(default = "legacy_schema_version")]
    pub schema_version: String,
    #[serde(default = "development_only_trust")]
    pub trust_level: String,
    #[serde(default)]
    pub controlled_beta_eligible: bool,
    #[serde(default)]
    pub publisher: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub source_status: Option<String>,
    #[serde(default)]
    pub supersedes_version: Option<String>,
    #[serde(default)]
    pub superseded_by_version: Option<String>,
    #[serde(default)]
    pub editorial_review_status: Option<String>,
    #[serde(default)]
    pub rights_basis: Option<String>,
}

fn legacy_schema_version() -> String {
    "1.0.0".into()
}

fn development_only_trust() -> String {
    "DEVELOPMENT_ONLY".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegalSourceRecord {
    pub schema_version: String,
    pub source_id: String,
    pub title: String,
    pub jurisdiction: String,
    pub jurisdiction_path: Vec<String>,
    pub authority_type: String,
    pub issuing_authority: String,
    pub citation: String,
    pub section: String,
    pub effective_from: String,
    pub effective_to: String,
    pub version: String,
    pub amendment: String,
    pub source_file: String,
    pub page: serde_json::Value,
    pub paragraph: String,
    pub heading_path: Vec<String>,
    pub exact_text: String,
    pub checksum: String,
    pub pack_id: String,
    pub trust_level: String,
    pub superseded_status: String,
    pub publisher: String,
    pub source_reference: String,
    pub content_kind: String,
    #[serde(default)]
    pub derived_from_source_id: Option<String>,
    #[serde(default)]
    pub court: Option<String>,
    #[serde(default)]
    pub case_name: Option<String>,
    #[serde(default)]
    pub case_number: Option<String>,
    #[serde(default)]
    pub decision_date: Option<String>,
    #[serde(default)]
    pub paragraph_number: Option<String>,
    #[serde(default)]
    pub holding_tags: Vec<String>,
    #[serde(default)]
    pub proposition_tags: Vec<String>,
    #[serde(default)]
    pub precedential_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LegalChunkRecord {
    pub schema_version: String,
    pub chunk_id: String,
    pub source_id: String,
    pub pack_id: String,
    pub title: String,
    pub jurisdiction: String,
    pub jurisdiction_path: Vec<String>,
    pub authority_type: String,
    pub issuing_authority: String,
    pub citation: String,
    pub section: String,
    pub effective_from: String,
    pub effective_to: String,
    pub version: String,
    pub amendment: String,
    pub source_file: String,
    pub page: serde_json::Value,
    pub paragraph: String,
    pub heading_path: Vec<String>,
    pub exact_text: String,
    pub checksum: String,
    pub source_checksum: String,
    pub pack_checksum: String,
    pub trust_level: String,
    pub superseded_status: String,
    pub publisher: String,
    pub source_reference: String,
    pub content_kind: String,
    pub temporal_status: String,
    #[serde(default)]
    pub derived_from_source_id: Option<String>,
    #[serde(default)]
    pub court: Option<String>,
    #[serde(default)]
    pub case_name: Option<String>,
    #[serde(default)]
    pub case_number: Option<String>,
    #[serde(default)]
    pub decision_date: Option<String>,
    #[serde(default)]
    pub paragraph_number: Option<String>,
    #[serde(default)]
    pub holding_tags: Vec<String>,
    #[serde(default)]
    pub proposition_tags: Vec<String>,
    #[serde(default)]
    pub precedential_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct Registry {
    packs: Vec<InstalledPack>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackInstallResult {
    pub pack: InstalledPack,
    pub headnote_hits: Vec<String>,
    pub security_report: PackSecurityReport,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackSecurityReport {
    pub batch: &'static str,
    pub pack_id: String,
    pub schema_version: String,
    pub files_validated: usize,
    pub bytes_validated: u64,
    pub checks: Vec<String>,
    pub transactional_upgrade: bool,
    pub protection_verified: bool,
    pub active_content_rejected: bool,
    pub trust_domain: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HeadnoteScanResult {
    pub pack_id: String,
    pub hits: Vec<String>,
    pub zero_matches: bool,
    pub ac: &'static str,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn sha256_bytes(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let data = fs::read(path).map_err(|e| format!("CF_PACK_INSTALL_FAILED: read {e}"))?;
    Ok(sha256_bytes(&data))
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

fn reject_reparse(path: &Path) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| {
        format!(
            "CF_PACK_SECURITY_REJECTED: metadata {}: {e}",
            path.display()
        )
    })?;
    if metadata.file_type().is_symlink() || is_reparse(&metadata) {
        return Err(format!(
            "CF_PACK_SECURITY_REJECTED: symlink/reparse point {}",
            path.display()
        ));
    }
    Ok(metadata)
}

fn safe_relative_path(raw: &str) -> Result<PathBuf, String> {
    if raw.is_empty()
        || raw.starts_with('/')
        || raw.starts_with("//")
        || raw.contains('\\')
        || raw.contains(':')
        || raw.contains("//")
        || raw.ends_with('/')
    {
        return Err(format!("CF_PACK_SECURITY_REJECTED: unsafe path {raw}"));
    }
    let path = Path::new(raw);
    let mut depth = 0usize;
    for component in path.components() {
        use std::path::Component;
        let Component::Normal(name) = component else {
            return Err(format!("CF_PACK_SECURITY_REJECTED: unsafe path {raw}"));
        };
        depth += 1;
        let part = name.to_string_lossy();
        if part == "." || part == ".." || part.ends_with('.') || part.ends_with(' ') {
            return Err(format!("CF_PACK_SECURITY_REJECTED: unsafe path {raw}"));
        }
        let stem = part
            .split('.')
            .next()
            .unwrap_or("")
            .trim_end_matches(['.', ' '])
            .to_ascii_uppercase();
        let device = matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || stem
            .strip_prefix("COM")
            .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
            || stem
                .strip_prefix("LPT")
                .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"));
        if device {
            return Err(format!("CF_PACK_SECURITY_REJECTED: device path {raw}"));
        }
    }
    if depth == 0 || depth > MAX_PACK_DEPTH {
        return Err(format!("CF_PACK_SECURITY_REJECTED: path depth {raw}"));
    }
    Ok(path.to_path_buf())
}

fn active_content(bytes: &[u8]) -> bool {
    let sample = String::from_utf8_lossy(&bytes[..bytes.len().min(64 * 1024)]).to_ascii_lowercase();
    [
        "<script",
        "javascript:",
        "<iframe",
        "<object",
        "<embed",
        "<!doctype html",
        "<html",
    ]
    .iter()
    .any(|needle| sample.contains(needle))
}

fn preflight_source_root(source_dir: &Path) -> Result<PathBuf, String> {
    let meta = reject_reparse(source_dir)?;
    if !meta.is_dir() {
        return Err("CF_PACK_NOT_FOUND: source is not a directory".into());
    }
    let manifest = source_dir.join("manifest.json");
    let manifest_meta = reject_reparse(&manifest)?;
    if !manifest_meta.is_file() || manifest_meta.len() > MAX_MANIFEST_BYTES {
        return Err("CF_PACK_SECURITY_REJECTED: manifest missing or oversized".into());
    }
    source_dir
        .canonicalize()
        .map_err(|e| format!("CF_PACK_SECURITY_REJECTED: canonical root {e}"))
}

fn validate_pack_tree(
    source_dir: &Path,
    canonical_root: &Path,
    man: &PackManifest,
) -> Result<PackSecurityReport, String> {
    if !PACK_ID_RE.is_match(&man.pack_id) || !VERSION_RE.is_match(&man.version) {
        return Err("CF_PACK_SECURITY_REJECTED: invalid pack_id or semantic version".into());
    }
    if man.schema_version == "2.0.0" {
        if man.metadata_version.as_deref() != Some("2.0.0")
            || man.jurisdiction.as_deref().is_none_or(str::is_empty)
            || man.jurisdiction_path.is_empty()
            || man
                .authoritative_content_sha256
                .as_deref()
                .is_none_or(|v| v.len() != 64)
            || man.sources_file.as_deref() != Some("metadata/legal_sources.json")
            || man.chunks_file.as_deref() != Some("metadata/legal_chunks.json")
            || man
                .files
                .iter()
                .any(|f| f.bytes.is_none() || f.role.is_none())
            || (man.pack_kind == "official"
                && (man.signature_alg.as_deref() != Some("Ed25519")
                    || man.trust_level.as_deref() != Some("OFFICIAL_VERIFIED")
                    || !man.controlled_beta_eligible))
            || (man.pack_kind == "custom"
                && (man.trust_level.as_deref() != Some("CUSTOM") || man.controlled_beta_eligible))
        {
            return Err("CF_PACK_SECURITY_REJECTED: manifest v2 schema semantics".into());
        }
    }
    if man.files.is_empty() || man.files.len() > MAX_PACK_FILES {
        return Err("CF_PACK_SECURITY_REJECTED: invalid file count".into());
    }
    let mut listed = std::collections::HashSet::new();
    let mut total = 0u64;
    for file in &man.files {
        let rel = safe_relative_path(&file.path)?;
        if !listed.insert(file.path.clone()) {
            return Err("CF_PACK_SECURITY_REJECTED: duplicate manifest path".into());
        }
        let allowed = if file.path.starts_with("content/") {
            rel.extension().and_then(|v| v.to_str()) == Some("txt")
                && (man.schema_version == "1.0.0"
                    || file.role.as_deref() == Some("AUTHORITATIVE_CONTENT"))
        } else if file.path.starts_with("metadata/") {
            man.schema_version == "2.0.0"
                && rel.extension().and_then(|v| v.to_str()) == Some("json")
                && matches!(
                    file.role.as_deref(),
                    Some("LEGAL_SOURCES" | "LEGAL_CHUNKS" | "DERIVED_ARTEFACT")
                )
        } else {
            false
        };
        if !allowed {
            return Err(format!(
                "CF_PACK_SECURITY_REJECTED: unsupported content {}",
                file.path
            ));
        }
        let candidate = source_dir.join(&rel);
        let meta = reject_reparse(&candidate)?;
        if !meta.is_file() || meta.len() > MAX_PACK_FILE_BYTES {
            return Err(format!(
                "CF_PACK_SECURITY_REJECTED: missing/oversized {}",
                file.path
            ));
        }
        if file.bytes.is_some_and(|declared| declared != meta.len()) {
            return Err(format!(
                "CF_PACK_SECURITY_REJECTED: byte count mismatch {}",
                file.path
            ));
        }
        let canonical = candidate
            .canonicalize()
            .map_err(|e| format!("CF_PACK_SECURITY_REJECTED: canonical {}: {e}", file.path))?;
        if !canonical.starts_with(canonical_root) {
            return Err(format!(
                "CF_PACK_SECURITY_REJECTED: path escapes root {}",
                file.path
            ));
        }
        let bytes = fs::read(&candidate)
            .map_err(|e| format!("CF_PACK_SECURITY_REJECTED: read {}: {e}", file.path))?;
        if file.path.starts_with("content/") && active_content(&bytes) {
            return Err(format!(
                "CF_PACK_SECURITY_REJECTED: active content {}",
                file.path
            ));
        }
        total = total
            .checked_add(meta.len())
            .ok_or("CF_PACK_SECURITY_REJECTED: size overflow")?;
        if total > MAX_PACK_TOTAL_BYTES {
            return Err("CF_PACK_SECURITY_REJECTED: pack oversized".into());
        }
    }
    let mut stack = vec![(source_dir.to_path_buf(), 0usize)];
    let mut actual_files = 0usize;
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_PACK_DEPTH {
            return Err("CF_PACK_SECURITY_REJECTED: directory depth".into());
        }
        for entry in fs::read_dir(&dir).map_err(|e| format!("CF_PACK_SECURITY_REJECTED: {e}"))? {
            let entry = entry.map_err(|e| format!("CF_PACK_SECURITY_REJECTED: {e}"))?;
            let path = entry.path();
            let meta = reject_reparse(&path)?;
            if meta.is_dir() {
                stack.push((path, depth + 1));
            } else if meta.is_file() {
                actual_files += 1;
                if actual_files > MAX_PACK_FILES + 2 {
                    return Err("CF_PACK_SECURITY_REJECTED: too many physical files".into());
                }
                let rel = path
                    .strip_prefix(source_dir)
                    .map_err(|_| "CF_PACK_SECURITY_REJECTED: tree escape")?
                    .to_string_lossy()
                    .replace('\\', "/");
                if rel != "manifest.json" && rel != "signature.ed25519" && !listed.contains(&rel) {
                    return Err(format!("CF_PACK_SECURITY_REJECTED: unlisted file {rel}"));
                }
            } else {
                return Err("CF_PACK_SECURITY_REJECTED: unsupported filesystem entry".into());
            }
        }
    }
    Ok(PackSecurityReport {
        batch: "B-S3-09",
        pack_id: man.pack_id.clone(),
        schema_version: man.schema_version.clone(),
        files_validated: man.files.len(),
        bytes_validated: total,
        checks: vec![
            "schema-and-identifiers".into(),
            "contained-canonical-paths".into(),
            "no-symlink-or-reparse".into(),
            "hash-size-count-limits".into(),
            "passive-supported-content-only".into(),
        ],
        transactional_upgrade: false,
        protection_verified: false,
        active_content_rejected: true,
        trust_domain: man.pack_kind.clone(),
    })
}

fn canonical_hash(files: &[PackFileEntry]) -> String {
    let mut lines: Vec<String> = files
        .iter()
        .map(|f| format!("{}:{}", f.path, f.sha256))
        .collect();
    lines.sort();
    let blob = lines.join("\n") + "\n";
    sha256_bytes(blob.as_bytes())
}

fn authoritative_hash(files: &[PackFileEntry]) -> String {
    let authoritative: Vec<PackFileEntry> = files
        .iter()
        .filter(|f| f.role.as_deref() == Some("AUTHORITATIVE_CONTENT"))
        .cloned()
        .collect();
    canonical_hash(&authoritative)
}

/// AC-7.1 automated headnote scan over pack content files.
pub fn headnote_scan_dir(content_root: &Path) -> Vec<String> {
    let mut hits = Vec::new();
    let walker = walkdir_simple(content_root);
    for path in walker {
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let lower = text.to_ascii_lowercase();
        for pat in HEADNOTE_PATTERNS {
            if lower.contains(pat) {
                let rel = path
                    .strip_prefix(content_root)
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| path.display().to_string());
                hits.push(format!("{rel} :: matched '{pat}'"));
                break;
            }
        }
    }
    hits
}

fn walkdir_simple(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn rec(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = fs::read_dir(dir) else {
            return;
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                rec(&p, out);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    if root.is_file() {
        out.push(root.to_path_buf());
    } else {
        rec(root, &mut out);
    }
    out.sort();
    out
}

fn load_trust_pubkey(key_id: &str) -> Result<[u8; 32], String> {
    #[cfg(test)]
    if key_id == "caseforge-security-test" {
        let signing = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        return Ok(signing.verifying_key().to_bytes());
    }
    // Prefer repo contracts when running from source tree
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/pack_trust.json"),
        PathBuf::from("contracts/pack_trust.json"),
    ];
    for path in candidates {
        if !path.exists() {
            continue;
        }
        let raw = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let v: serde_json::Value =
            serde_json::from_str(&raw).map_err(|e| format!("pack_trust parse: {e}"))?;
        if let Some(keys) = v.get("keys").and_then(|k| k.as_array()) {
            for k in keys {
                if k.get("key_id").and_then(|x| x.as_str()) == Some(key_id) {
                    let hex_s = k
                        .get("public_key_hex")
                        .and_then(|x| x.as_str())
                        .ok_or("missing public_key_hex")?;
                    if hex_s == "REPLACE_AT_KEYGEN" || hex_s.len() != 64 {
                        return Err(
                            "CF_PACK_INVALID_SIGNATURE: official trust key not generated (run make_akpkg)"
                                .into(),
                        );
                    }
                    let bytes = hex::decode(hex_s)
                        .map_err(|e| format!("CF_PACK_INVALID_SIGNATURE: bad pubkey hex {e}"))?;
                    if bytes.len() != 32 {
                        return Err("CF_PACK_INVALID_SIGNATURE: pubkey must be 32 bytes".into());
                    }
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    return Ok(arr);
                }
            }
        }
    }
    Err(format!(
        "CF_PACK_NOT_FOUND: trust key_id {key_id} not in pack_trust.json"
    ))
}

fn verify_ed25519(pubkey: &[u8; 32], message: &[u8], sig_b64: &str) -> Result<(), String> {
    let sig_bytes = B64
        .decode(sig_b64.trim())
        .map_err(|e| format!("CF_PACK_INVALID_SIGNATURE: b64 {e}"))?;
    if sig_bytes.len() != 64 {
        return Err("CF_PACK_INVALID_SIGNATURE: signature length".into());
    }
    let mut sig_arr = [0u8; 64];
    sig_arr.copy_from_slice(&sig_bytes);
    let vk = VerifyingKey::from_bytes(pubkey)
        .map_err(|e| format!("CF_PACK_INVALID_SIGNATURE: pubkey {e}"))?;
    let sig = Signature::from_bytes(&sig_arr);
    vk.verify(message, &sig)
        .map_err(|_| "CF_PACK_INVALID_SIGNATURE: verification failed".to_string())
}

fn read_manifest(pack_dir: &Path) -> Result<PackManifest, String> {
    let p = pack_dir.join("manifest.json");
    let raw = fs::read_to_string(&p).map_err(|e| format!("CF_PACK_NOT_FOUND: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("CF_PACK_INSTALL_FAILED: manifest {e}"))
}

fn verify_content_files(pack_dir: &Path, man: &PackManifest) -> Result<(), String> {
    for f in &man.files {
        if !f.path.starts_with("content/") && !f.path.starts_with("metadata/") {
            return Err(format!(
                "CF_PACK_INSTALL_FAILED: file path not under content/ or metadata/: {}",
                f.path
            ));
        }
        let path = pack_dir.join(&f.path);
        if !path.is_file() {
            return Err(format!("CF_PACK_NOT_FOUND: missing {}", f.path));
        }
        let got = sha256_file(&path)?;
        if got != f.sha256 {
            return Err(format!(
                "CF_PACK_INSTALL_FAILED: hash mismatch for {}",
                f.path
            ));
        }
    }
    let canon = canonical_hash(&man.files);
    if canon != man.content_sha256 {
        return Err("CF_PACK_INSTALL_FAILED: content_sha256 mismatch".into());
    }
    if man.schema_version == "2.0.0"
        && man.authoritative_content_sha256.as_deref()
            != Some(authoritative_hash(&man.files).as_str())
    {
        return Err("CF_PACK_INSTALL_FAILED: authoritative_content_sha256 mismatch".into());
    }
    Ok(())
}

fn prefixed_sha256(text: &str) -> String {
    format!("sha256:{}", sha256_bytes(text.as_bytes()))
}

fn expected_source_id(pack_id: &str, checksum: &str) -> Option<String> {
    checksum
        .strip_prefix("sha256:")
        .filter(|v| v.len() >= 16)
        .map(|hex| format!("cfsrc:{pack_id}:{}", &hex[..16]))
}

fn expected_chunk_id(pack_id: &str, source_id: &str, checksum: &str) -> String {
    let seed = format!("{source_id}|{checksum}");
    format!("cfchunk:{pack_id}:{}", &sha256_bytes(seed.as_bytes())[..16])
}

fn read_json_array<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Vec<T>, String> {
    let raw = fs::read_to_string(path)
        .map_err(|e| format!("CF_PACK_METADATA_INVALID: {}: {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("CF_PACK_METADATA_INVALID: {}: {e}", path.display()))
}

fn read_legal_chunks(pack_dir: &Path) -> Result<Vec<LegalChunkRecord>, String> {
    read_json_array(&pack_dir.join("metadata/legal_chunks.json"))
}

/// Revalidates signed metadata before every indexing pass. Protected raw content has its own
/// authenticated reader; the signed manifest binds these metadata file hashes to that content hash.
pub fn load_legal_chunks(pack_dir: &Path) -> Result<Vec<LegalChunkRecord>, String> {
    let man = read_manifest(pack_dir)?;
    if man.schema_version != "2.0.0" {
        return Err("CF_PACK_METADATA_INVALID: legal chunks require manifest v2".into());
    }
    if canonical_hash(&man.files) != man.content_sha256 {
        return Err("CF_PACK_METADATA_INVALID: signed manifest hash mismatch".into());
    }
    if man.pack_kind == "official" {
        let key_id = man
            .signer_key_id
            .as_deref()
            .ok_or("CF_PACK_INVALID_SIGNATURE: signer_key_id missing")?;
        let pk = load_trust_pubkey(key_id)?;
        let sig = fs::read_to_string(pack_dir.join("signature.ed25519"))
            .map_err(|_| "CF_PACK_INVALID_SIGNATURE: missing signature.ed25519".to_string())?;
        verify_ed25519(&pk, man.content_sha256.as_bytes(), &sig)?;
    }
    for expected in ["metadata/legal_sources.json", "metadata/legal_chunks.json"] {
        let entry = man
            .files
            .iter()
            .find(|f| f.path == expected)
            .ok_or_else(|| format!("CF_PACK_METADATA_INVALID: {expected} unlisted"))?;
        if sha256_file(&pack_dir.join(expected))? != entry.sha256 {
            return Err(format!(
                "CF_PACK_METADATA_INVALID: {expected} hash mismatch"
            ));
        }
    }
    let sources: Vec<LegalSourceRecord> =
        read_json_array(&pack_dir.join("metadata/legal_sources.json"))?;
    let by_id: HashMap<String, LegalSourceRecord> = sources
        .into_iter()
        .map(|s| (s.source_id.clone(), s))
        .collect();
    let chunks = read_legal_chunks(pack_dir)?;
    let mut chunk_ids = std::collections::HashSet::new();
    for chunk in &chunks {
        let source = by_id
            .get(&chunk.source_id)
            .ok_or("CF_PACK_METADATA_INVALID: chunk source missing")?;
        if prefixed_sha256(&source.exact_text) != source.checksum
            || prefixed_sha256(&chunk.exact_text) != chunk.checksum
            || chunk.source_checksum != source.checksum
            || !source.exact_text.contains(&chunk.exact_text)
            || Some(chunk.pack_checksum.as_str()) != man.authoritative_content_sha256.as_deref()
            || chunk.chunk_id != expected_chunk_id(&man.pack_id, &chunk.source_id, &chunk.checksum)
        {
            return Err("CF_PACK_METADATA_INVALID: indexed lineage mismatch".into());
        }
        if !chunk_ids.insert(chunk.chunk_id.clone()) {
            return Err("CF_PACK_METADATA_INVALID: duplicate indexed chunk_id".into());
        }
    }
    Ok(chunks)
}

/// B-S3-08 runtime semantic validation. JSON Schema tooling performs the structural pass;
/// this pass proves exact-text lineage and checksum/trust invariants before installation.
fn validate_legal_metadata(pack_dir: &Path, man: &PackManifest) -> Result<(), String> {
    if man.metadata_version.as_deref() != Some("2.0.0")
        || man.sources_file.as_deref() != Some("metadata/legal_sources.json")
        || man.chunks_file.as_deref() != Some("metadata/legal_chunks.json")
    {
        return Err("CF_PACK_METADATA_INVALID: manifest v2 metadata pointers".into());
    }
    let expected_trust = if man.pack_kind == "official" {
        "OFFICIAL_VERIFIED"
    } else {
        "CUSTOM"
    };
    if man.trust_level.as_deref() != Some(expected_trust)
        || (man.pack_kind == "official" && !man.controlled_beta_eligible)
        || (man.pack_kind == "custom" && man.controlled_beta_eligible)
    {
        return Err("CF_PACK_METADATA_INVALID: trust/beta policy mismatch".into());
    }
    let sources: Vec<LegalSourceRecord> =
        read_json_array(&pack_dir.join("metadata/legal_sources.json"))?;
    let chunks: Vec<LegalChunkRecord> = read_legal_chunks(pack_dir)?;
    if sources.is_empty() || chunks.is_empty() {
        return Err("CF_PACK_METADATA_INVALID: sources/chunks cannot be empty".into());
    }
    let mut by_id = HashMap::new();
    for source in sources {
        if source.schema_version != "2.0.0"
            || source.pack_id != man.pack_id
            || source.trust_level != expected_trust
            || source.title.is_empty()
            || source.jurisdiction.is_empty()
            || source.jurisdiction_path.is_empty()
            || !valid_authority(&source.authority_type)
            || !valid_date_or_unknown(&source.effective_from)
            || !valid_date_or_unknown(&source.effective_to)
            || !valid_supersession(&source.superseded_status)
            || !valid_content_kind(&source.content_kind)
            || source.issuing_authority.is_empty()
            || source.citation.is_empty()
            || source.section.is_empty()
            || source.version.is_empty()
            || source.amendment.is_empty()
            || source.publisher.is_empty()
            || source.source_reference.is_empty()
            || source.exact_text.is_empty()
            || prefixed_sha256(&source.exact_text) != source.checksum
            || expected_source_id(&man.pack_id, &source.checksum).as_deref()
                != Some(source.source_id.as_str())
        {
            return Err(format!(
                "CF_PACK_METADATA_INVALID: source identity/checksum/trust {}",
                source.source_id
            ));
        }
        if source.authority_type == "JUDGMENT"
            && (source.court.as_deref().is_none_or(str::is_empty)
                || source.case_name.as_deref().is_none_or(str::is_empty)
                || source.case_number.as_deref().is_none_or(str::is_empty)
                || source
                    .decision_date
                    .as_deref()
                    .is_none_or(|v| !valid_date_or_unknown(v))
                || source.paragraph_number.as_deref().is_none_or(str::is_empty)
                || source.precedential_status.as_deref().is_none_or(|v| {
                    !matches!(
                        v,
                        "BINDING" | "PERSUASIVE" | "NON_PRECEDENTIAL" | "OVERRULED" | "UNKNOWN"
                    )
                }))
        {
            return Err("CF_PACK_METADATA_INVALID: judgment metadata incomplete".into());
        }
        if source.content_kind == "DERIVED_SUMMARY"
            && (source.trust_level == "OFFICIAL_VERIFIED"
                || source.derived_from_source_id.is_none())
        {
            return Err("CF_PACK_METADATA_INVALID: summary cannot be authoritative".into());
        }
        let source_path = pack_dir.join(&source.source_file);
        let file_text = fs::read_to_string(&source_path).map_err(|e| {
            format!(
                "CF_PACK_METADATA_INVALID: source file {}: {e}",
                source.source_file
            )
        })?;
        if !file_text.contains(&source.exact_text) {
            return Err(format!(
                "CF_PACK_METADATA_INVALID: exact source text not found in {}",
                source.source_file
            ));
        }
        if by_id.insert(source.source_id.clone(), source).is_some() {
            return Err("CF_PACK_METADATA_INVALID: duplicate source_id".into());
        }
    }
    let mut chunk_ids = std::collections::HashSet::new();
    for chunk in chunks {
        let source = by_id.get(&chunk.source_id).ok_or_else(|| {
            format!(
                "CF_PACK_METADATA_INVALID: chunk source missing {}",
                chunk.source_id
            )
        })?;
        if chunk.schema_version != "2.0.0"
            || chunk.pack_id != man.pack_id
            || Some(chunk.pack_checksum.as_str()) != man.authoritative_content_sha256.as_deref()
            || chunk.source_checksum != source.checksum
            || chunk.trust_level != expected_trust
            || !valid_authority(&chunk.authority_type)
            || !valid_date_or_unknown(&chunk.effective_from)
            || !valid_date_or_unknown(&chunk.effective_to)
            || !valid_temporal(&chunk.temporal_status)
            || !valid_supersession(&chunk.superseded_status)
            || !valid_content_kind(&chunk.content_kind)
            || chunk.jurisdiction_path.is_empty()
            || chunk.exact_text.is_empty()
            || prefixed_sha256(&chunk.exact_text) != chunk.checksum
            || chunk.chunk_id != expected_chunk_id(&man.pack_id, &chunk.source_id, &chunk.checksum)
            || !source.exact_text.contains(&chunk.exact_text)
        {
            return Err(format!(
                "CF_PACK_METADATA_INVALID: chunk lineage/checksum/trust {}",
                chunk.chunk_id
            ));
        }
        if !chunk_ids.insert(chunk.chunk_id.clone()) {
            return Err("CF_PACK_METADATA_INVALID: duplicate chunk_id".into());
        }
        if chunk.content_kind == "DERIVED_SUMMARY"
            && (chunk.trust_level == "OFFICIAL_VERIFIED" || chunk.derived_from_source_id.is_none())
        {
            return Err("CF_PACK_METADATA_INVALID: summary masquerades as authority".into());
        }
        if chunk.authority_type == "JUDGMENT"
            && (chunk.court.as_deref().is_none_or(str::is_empty)
                || chunk.case_name.as_deref().is_none_or(str::is_empty)
                || chunk.case_number.as_deref().is_none_or(str::is_empty)
                || chunk
                    .decision_date
                    .as_deref()
                    .is_none_or(|v| !valid_date_or_unknown(v))
                || chunk.paragraph_number.as_deref().is_none_or(str::is_empty)
                || chunk.precedential_status.as_deref().is_none_or(|v| {
                    !matches!(
                        v,
                        "BINDING" | "PERSUASIVE" | "NON_PRECEDENTIAL" | "OVERRULED" | "UNKNOWN"
                    )
                }))
        {
            return Err("CF_PACK_METADATA_INVALID: judgment chunk metadata incomplete".into());
        }
    }
    Ok(())
}

fn load_registry(paths: &VaultPaths) -> Registry {
    let p = paths.packs_registry_path();
    if !p.exists() {
        return Registry::default();
    }
    fs::read_to_string(&p)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_registry(paths: &VaultPaths, reg: &Registry) -> Result<(), String> {
    let p = paths.packs_registry_path();
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let s = serde_json::to_string_pretty(reg).map_err(|e| e.to_string())?;
    let temp = p.with_extension(format!("tmp-{}", Uuid::new_v4()));
    fs::write(&temp, s + "\n").map_err(|e| e.to_string())?;
    let backup = p.with_extension(format!("bak-{}", Uuid::new_v4()));
    if p.exists() {
        fs::rename(&p, &backup).map_err(|e| e.to_string())?;
    }
    if let Err(e) = fs::rename(&temp, &p) {
        if backup.exists() {
            let _ = fs::rename(&backup, &p);
        }
        let _ = fs::remove_file(&temp);
        return Err(e.to_string());
    }
    if backup.exists() {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

fn source_ids_from_dir(pack_dir: &Path) -> Result<std::collections::HashSet<String>, String> {
    let sources: Vec<LegalSourceRecord> =
        read_json_array(&pack_dir.join("metadata/legal_sources.json"))?;
    Ok(sources.into_iter().map(|s| s.source_id).collect())
}

fn reject_trust_and_source_collisions(
    paths: &VaultPaths,
    man: &PackManifest,
    source_dir: &Path,
) -> Result<(), String> {
    let registry = load_registry(paths);
    if registry
        .packs
        .iter()
        .any(|p| p.pack_id == man.pack_id && p.pack_kind != man.pack_kind)
    {
        return Err(
            "CF_PACK_SECURITY_REJECTED: pack_id crosses official/custom trust domain".into(),
        );
    }
    if man.schema_version != "2.0.0" {
        return Ok(());
    }
    let incoming = source_ids_from_dir(source_dir)?;
    for installed in registry
        .packs
        .iter()
        .filter(|p| p.pack_id != man.pack_id && p.schema_version == "2.0.0")
    {
        let existing = source_ids_from_dir(Path::new(&installed.install_path))?;
        if let Some(collision) = incoming.intersection(&existing).next() {
            return Err(format!(
                "CF_PACK_SECURITY_REJECTED: source_id collision {collision} with {}",
                installed.pack_id
            ));
        }
    }
    Ok(())
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    let canonical_root = src.canonicalize().map_err(|e| e.to_string())?;
    let mut stack = vec![(src.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        if depth > MAX_PACK_DEPTH {
            return Err("CF_PACK_SECURITY_REJECTED: copy depth".into());
        }
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            let metadata = reject_reparse(&path)?;
            let canonical = path.canonicalize().map_err(|e| e.to_string())?;
            if !canonical.starts_with(&canonical_root) {
                return Err("CF_PACK_SECURITY_REJECTED: copy source escape".into());
            }
            let rel = path.strip_prefix(src).map_err(|e| e.to_string())?;
            let target = dst.join(rel);
            if metadata.is_dir() {
                fs::create_dir_all(&target).map_err(|e| e.to_string())?;
                stack.push((path, depth + 1));
            } else if metadata.is_file() {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::copy(&path, &target).map_err(|e| e.to_string())?;
            } else {
                return Err("CF_PACK_SECURITY_REJECTED: copy entry type".into());
            }
        }
    }
    Ok(())
}

/// Install pack from a source directory (USB/download extracted .akpkg layout).
pub fn install_pack_from_dir(
    paths: &VaultPaths,
    source_dir: &Path,
) -> Result<PackInstallResult, String> {
    let canonical_root = preflight_source_root(source_dir)?;
    let man = read_manifest(source_dir)?;
    if man.schema_version != "1.0.0" && man.schema_version != "2.0.0" {
        return Err("CF_PACK_UNSUPPORTED_VERSION".into());
    }
    let mut security_report = validate_pack_tree(source_dir, &canonical_root, &man)?;
    verify_content_files(source_dir, &man)?;
    if man.schema_version == "2.0.0" {
        validate_legal_metadata(source_dir, &man)?;
    }
    reject_trust_and_source_collisions(paths, &man, source_dir)?;

    let content_root = source_dir.join("content");
    let hits = headnote_scan_dir(&content_root);
    let signature_ok = if man.pack_kind == "official" {
        if !man.headnotes_stripped {
            return Err(
                "CF_PACK_HEADNOTE_SCAN_FAILED: official pack must set headnotes_stripped".into(),
            );
        }
        if !hits.is_empty() {
            return Err(format!(
                "CF_PACK_HEADNOTE_SCAN_FAILED: {} hit(s): {}",
                hits.len(),
                hits.join("; ")
            ));
        }
        let key_id = man
            .signer_key_id
            .as_deref()
            .unwrap_or("caseforge-official-dev-2026");
        let pk = load_trust_pubkey(key_id)?;
        let sig_path = source_dir.join("signature.ed25519");
        let sig_b64 = fs::read_to_string(&sig_path)
            .map_err(|_| "CF_PACK_INVALID_SIGNATURE: missing signature.ed25519".to_string())?;
        verify_ed25519(&pk, man.content_sha256.as_bytes(), &sig_b64)?;
        true
    } else if man.pack_kind == "custom" {
        // Custom: no signature required; isolation only. Headnotes allowed.
        false
    } else {
        return Err("CF_PACK_INSTALL_FAILED: pack_kind must be official|custom".into());
    };

    let dest_root = if man.pack_kind == "official" {
        paths.packs_official_dir()
    } else {
        paths.packs_custom_dir()
    };
    fs::create_dir_all(&dest_root).map_err(|e| format!("CF_PACK_INSTALL_FAILED: {e}"))?;
    let dest = dest_root.join(&man.pack_id);
    let transaction_id = Uuid::new_v4().to_string();
    let staging = dest_root.join(format!(".staging-{}-{transaction_id}", man.pack_id));
    let backup = dest_root.join(format!(".backup-{}-{transaction_id}", man.pack_id));
    if let Err(e) = copy_dir(source_dir, &staging) {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    let staged_check = (|| -> Result<(), String> {
        let staged_root = preflight_source_root(&staging)?;
        validate_pack_tree(&staging, &staged_root, &man)?;
        verify_content_files(&staging, &man)?;
        if man.schema_version == "2.0.0" {
            validate_legal_metadata(&staging, &man)?;
        }
        Ok(())
    })();
    if let Err(e) = staged_check {
        let _ = fs::remove_dir_all(&staging);
        return Err(e);
    }
    if std::env::var("CASEFORGE_PACK_FORCE_POST_STAGE_FAIL")
        .ok()
        .as_deref()
        == Some(man.pack_id.as_str())
    {
        let _ = fs::remove_dir_all(&staging);
        return Err("CF_PACK_INSTALL_FAILED: forced post-stage rollback verification".into());
    }

    // B-S1-19: encrypt official pack content at rest (economic protection).
    // Custom packs stay plaintext unless user opts in later.
    if man.pack_kind == "official" {
        let protected = match crate::protection::protect_pack_content(&staging, &man.pack_id) {
            Ok(result) if !result.degraded => result,
            Ok(_) => {
                let _ = fs::remove_dir_all(&staging);
                return Err("CF_PACK_INSTALL_FAILED: official protection degraded".into());
            }
            Err(e) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(format!(
                    "CF_PACK_INSTALL_FAILED: official protection failed: {e}"
                ));
            }
        };
        let expected_assets = man
            .files
            .iter()
            .filter(|f| f.path.starts_with("content/"))
            .count();
        let integrity = crate::protection::verify_pack_integrity(&staging, &man.pack_id);
        if !integrity.ok
            || protected.assets_protected != expected_assets
            || integrity.assets_checked != expected_assets
        {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!(
                "CF_PACK_INSTALL_FAILED: protected content verification failed: {}",
                integrity.failures.join("; ")
            ));
        }
        security_report.protection_verified = true;
    }

    let installed = InstalledPack {
        pack_id: man.pack_id.clone(),
        pack_kind: man.pack_kind.clone(),
        jurisdiction: man.jurisdiction.clone(),
        version: man.version.clone(),
        display_name: man.display_name.clone(),
        install_path: dest.display().to_string(),
        content_sha256: man.content_sha256.clone(),
        headnotes_stripped: man.headnotes_stripped,
        signature_ok,
        installed_at_unix_ms: now_ms(),
        isolated: man.pack_kind == "custom",
        schema_version: man.schema_version.clone(),
        trust_level: if man.schema_version == "2.0.0" {
            man.trust_level
                .clone()
                .unwrap_or_else(development_only_trust)
        } else {
            development_only_trust()
        },
        controlled_beta_eligible: man.schema_version == "2.0.0"
            && man.controlled_beta_eligible
            && man.pack_kind == "official",
        publisher: man.publisher.clone(),
        updated_at: man.updated_at.clone(),
        source_status: man.source_status.clone(),
        supersedes_version: man.supersedes_version.clone(),
        superseded_by_version: man.superseded_by_version.clone(),
        editorial_review_status: man.editorial_review_status.clone(),
        rights_basis: man.rights_basis.clone(),
    };

    let mut reg = load_registry(paths);
    reg.packs.retain(|p| p.pack_id != installed.pack_id);
    reg.packs.push(installed.clone());
    let had_previous = dest.exists();
    if had_previous {
        fs::rename(&dest, &backup).map_err(|e| {
            let _ = fs::remove_dir_all(&staging);
            format!("CF_PACK_INSTALL_FAILED: preserve prior version: {e}")
        })?;
    }
    if let Err(e) = fs::rename(&staging, &dest) {
        if had_previous {
            let _ = fs::rename(&backup, &dest);
        }
        let _ = fs::remove_dir_all(&staging);
        return Err(format!("CF_PACK_INSTALL_FAILED: activate staged pack: {e}"));
    }
    if let Err(e) = save_registry(paths, &reg) {
        let _ = fs::remove_dir_all(&dest);
        if had_previous {
            let _ = fs::rename(&backup, &dest);
        }
        return Err(format!("CF_PACK_INSTALL_FAILED: registry rollback: {e}"));
    }
    if had_previous && backup.exists() {
        let _ = fs::remove_dir_all(&backup);
    }
    security_report.transactional_upgrade = true;
    security_report.checks.extend([
        "trust-domain-and-source-collision".into(),
        "transactional-stage-activate-rollback".into(),
        "official-protection-fail-closed".into(),
    ]);

    Ok(PackInstallResult {
        pack: installed,
        headnote_hits: hits,
        security_report,
    })
}

pub fn list_installed(paths: &VaultPaths) -> Vec<InstalledPack> {
    load_registry(paths).packs
}

pub fn uninstall_pack(paths: &VaultPaths, pack_id: &str) -> Result<(), String> {
    let mut reg = load_registry(paths);
    let Some(pack) = reg.packs.iter().find(|p| p.pack_id == pack_id).cloned() else {
        return Err("CF_PACK_NOT_FOUND".into());
    };
    let dir = PathBuf::from(&pack.install_path);
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("CF_PACK_INSTALL_FAILED: {e}"))?;
    }
    reg.packs.retain(|p| p.pack_id != pack_id);
    save_registry(paths, &reg)?;
    Ok(())
}

/// Scan an installed official pack for headnotes (AC-7.1 evidence).
pub fn scan_installed_official(
    paths: &VaultPaths,
    pack_id: &str,
) -> Result<HeadnoteScanResult, String> {
    let reg = load_registry(paths);
    let pack = reg
        .packs
        .iter()
        .find(|p| p.pack_id == pack_id)
        .ok_or("CF_PACK_NOT_FOUND")?;
    if pack.pack_kind != "official" {
        return Err("CF_PACK_INSTALL_FAILED: headnote ship scan applies to official packs".into());
    }
    let content = PathBuf::from(&pack.install_path).join("content");
    let hits = headnote_scan_dir(&content);
    Ok(HeadnoteScanResult {
        pack_id: pack_id.into(),
        hits: hits.clone(),
        zero_matches: hits.is_empty(),
        ac: "AC-7.1",
    })
}

pub fn pack_policy() -> serde_json::Value {
    serde_json::json!({
        "batch": "B-S3-09",
        "format": "akpkg directory (manifest.json + content/ + signature.ed25519)",
        "schema": "contracts/akpkg.schema.json v2",
        "legal_source_schema": "contracts/legal_source.schema.json",
        "legal_chunk_schema": "contracts/legal_chunk.schema.json",
        "legacy_v1": "DEVELOPMENT_ONLY and controlled_beta_eligible=false",
        "security": {
            "max_files": MAX_PACK_FILES,
            "max_file_bytes": MAX_PACK_FILE_BYTES,
            "max_total_bytes": MAX_PACK_TOTAL_BYTES,
            "max_depth": MAX_PACK_DEPTH,
            "content_allowlist": ["content/*.txt", "metadata/*.json"],
            "active_content": "rejected; never executed or rendered as HTML",
            "paths": "relative canonical forward-slash paths only; no traversal, devices, symlinks or reparse points",
            "upgrade": "stage, validate, protect+verify, atomic activate, registry commit, rollback",
            "official_protection": "fail closed"
        },
        "signature": "Ed25519 over content_sha256 (ascii)",
        "official_isolation": "packs/official",
        "custom_isolation": "packs/custom",
        "ac_7_1": "official install rejects any headnote pattern match",
        "dual_market": "jurisdiction on pack; features identical — KB only differs"
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headnote_detects() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.txt");
        fs::write(&f, "This is a HEADNOTE for the case.\n").unwrap();
        let hits = headnote_scan_dir(dir.path());
        assert!(!hits.is_empty());
    }

    #[test]
    fn clean_raw_ok() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("x.txt");
        fs::write(
            &f,
            "Neutral citation: [2020] DEMO\nThe Court observed facts.\n",
        )
        .unwrap();
        let hits = headnote_scan_dir(dir.path());
        assert!(hits.is_empty());
    }

    #[test]
    fn b_s3_08_b_s3_09_v2_lineage_duplicate_and_tamper_validation() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("content")).unwrap();
        fs::create_dir_all(dir.path().join("metadata")).unwrap();
        let text = "Section 1. This Act applies to verified demonstrations.";
        fs::write(dir.path().join("content/act.txt"), text).unwrap();
        let pack_id = "caseforge.test.metadata";
        let source_checksum = prefixed_sha256(text);
        let source_id = format!(
            "cfsrc:{pack_id}:{}",
            &source_checksum.trim_start_matches("sha256:")[..16]
        );
        let content_entry = PackFileEntry {
            path: "content/act.txt".into(),
            sha256: sha256_file(&dir.path().join("content/act.txt")).unwrap(),
            bytes: Some(text.len() as u64),
            role: Some("AUTHORITATIVE_CONTENT".into()),
        };
        let authoritative_checksum = authoritative_hash(std::slice::from_ref(&content_entry));
        let source = LegalSourceRecord {
            schema_version: "2.0.0".into(),
            source_id: source_id.clone(),
            title: "Demonstration Act".into(),
            jurisdiction: "India".into(),
            jurisdiction_path: vec!["India".into()],
            authority_type: "STATUTE".into(),
            issuing_authority: "Parliament".into(),
            citation: "Demo Act 2026".into(),
            section: "1".into(),
            effective_from: "2026-01-01".into(),
            effective_to: "UNKNOWN".into(),
            version: "1".into(),
            amendment: "NONE".into(),
            source_file: "content/act.txt".into(),
            page: serde_json::json!(1),
            paragraph: "1".into(),
            heading_path: vec!["Section 1".into()],
            exact_text: text.into(),
            checksum: source_checksum.clone(),
            pack_id: pack_id.into(),
            trust_level: "OFFICIAL_VERIFIED".into(),
            superseded_status: "CURRENT".into(),
            publisher: "Official Gazette".into(),
            source_reference: "https://example.invalid/demo".into(),
            content_kind: "AUTHORITATIVE_TEXT".into(),
            derived_from_source_id: None,
            court: None,
            case_name: None,
            case_number: None,
            decision_date: None,
            paragraph_number: None,
            holding_tags: Vec::new(),
            proposition_tags: Vec::new(),
            precedential_status: None,
        };
        let chunk = LegalChunkRecord {
            schema_version: "2.0.0".into(),
            chunk_id: expected_chunk_id(pack_id, &source_id, &source_checksum),
            source_id,
            pack_id: pack_id.into(),
            title: source.title.clone(),
            jurisdiction: source.jurisdiction.clone(),
            jurisdiction_path: source.jurisdiction_path.clone(),
            authority_type: source.authority_type.clone(),
            issuing_authority: source.issuing_authority.clone(),
            citation: source.citation.clone(),
            section: source.section.clone(),
            effective_from: source.effective_from.clone(),
            effective_to: source.effective_to.clone(),
            version: source.version.clone(),
            amendment: source.amendment.clone(),
            source_file: source.source_file.clone(),
            page: serde_json::json!(1),
            paragraph: "1".into(),
            heading_path: vec!["Section 1".into()],
            exact_text: text.into(),
            checksum: source_checksum.clone(),
            source_checksum,
            pack_checksum: authoritative_checksum.clone(),
            trust_level: "OFFICIAL_VERIFIED".into(),
            superseded_status: "CURRENT".into(),
            publisher: source.publisher.clone(),
            source_reference: source.source_reference.clone(),
            content_kind: "AUTHORITATIVE_TEXT".into(),
            temporal_status: "APPLICABLE".into(),
            derived_from_source_id: None,
            court: None,
            case_name: None,
            case_number: None,
            decision_date: None,
            paragraph_number: None,
            holding_tags: Vec::new(),
            proposition_tags: Vec::new(),
            precedential_status: None,
        };
        fs::write(
            dir.path().join("metadata/legal_sources.json"),
            serde_json::to_vec(&vec![source]).unwrap(),
        )
        .unwrap();
        fs::write(
            dir.path().join("metadata/legal_chunks.json"),
            serde_json::to_vec(&vec![chunk]).unwrap(),
        )
        .unwrap();
        let mut files = vec![content_entry];
        for (path, role) in [
            ("metadata/legal_sources.json", "LEGAL_SOURCES"),
            ("metadata/legal_chunks.json", "LEGAL_CHUNKS"),
        ] {
            let full = dir.path().join(path);
            files.push(PackFileEntry {
                path: path.into(),
                sha256: sha256_file(&full).unwrap(),
                bytes: Some(fs::metadata(&full).unwrap().len()),
                role: Some(role.into()),
            });
        }
        let manifest = PackManifest {
            schema_version: "2.0.0".into(),
            pack_id: pack_id.into(),
            pack_kind: "official".into(),
            jurisdiction: Some("India".into()),
            version: "1".into(),
            display_name: "Metadata Test".into(),
            description: None,
            content_sha256: canonical_hash(&files),
            files,
            headnotes_stripped: true,
            signature_alg: Some("Ed25519".into()),
            signer_key_id: Some("test".into()),
            metadata_version: Some("2.0.0".into()),
            jurisdiction_path: vec!["India".into()],
            trust_level: Some("OFFICIAL_VERIFIED".into()),
            controlled_beta_eligible: true,
            authoritative_content_sha256: Some(authoritative_checksum),
            sources_file: Some("metadata/legal_sources.json".into()),
            chunks_file: Some("metadata/legal_chunks.json".into()),
            migration: None,
            publisher: None,
            updated_at: None,
            source_status: None,
            supersedes_version: None,
            superseded_by_version: None,
            editorial_review_status: None,
            rights_basis: None,
        };
        assert!(validate_legal_metadata(dir.path(), &manifest).is_ok());
        let sources_path = dir.path().join("metadata/legal_sources.json");
        let original_sources = fs::read(&sources_path).unwrap();
        let mut duplicate_sources: Vec<serde_json::Value> =
            serde_json::from_slice(&original_sources).unwrap();
        duplicate_sources.push(duplicate_sources[0].clone());
        fs::write(
            &sources_path,
            serde_json::to_vec(&duplicate_sources).unwrap(),
        )
        .unwrap();
        assert!(validate_legal_metadata(dir.path(), &manifest).is_err());
        fs::write(&sources_path, original_sources).unwrap();
        fs::write(dir.path().join("content/act.txt"), "tampered").unwrap();
        assert!(validate_legal_metadata(dir.path(), &manifest).is_err());
    }

    #[test]
    fn b_s3_10_controlled_partition_pack_has_verified_lineage_and_signature() {
        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../packs/official/caseforge.india.partition.highcourt.civil-appeal");
        let canonical_root = preflight_source_root(&pack).unwrap();
        let manifest = read_manifest(&pack).unwrap();
        validate_pack_tree(&pack, &canonical_root, &manifest).unwrap();
        verify_content_files(&pack, &manifest).unwrap();
        validate_legal_metadata(&pack, &manifest).unwrap();
        let chunks = load_legal_chunks(&pack).unwrap();
        assert_eq!(chunks.len(), 20);
        assert_eq!(
            manifest.publisher.as_deref(),
            Some("India Code / Ministry of Law and Justice; CaseForge original matter")
        );
        assert_eq!(manifest.updated_at.as_deref(), Some("2026-08-16"));
        assert_eq!(manifest.source_status.as_deref(), Some("CURRENT"));
        assert_eq!(
            manifest.rights_basis.as_deref(),
            Some("Copyright Act, 1957, section 52(1)(q)(ii)")
        );
        assert!(manifest.controlled_beta_eligible);
    }

    fn write_v1_custom(root: &Path, pack_id: &str, version: &str, text: &str) {
        fs::create_dir_all(root.join("content")).unwrap();
        let content = root.join("content/law.txt");
        fs::write(&content, text).unwrap();
        let file = PackFileEntry {
            path: "content/law.txt".into(),
            sha256: sha256_file(&content).unwrap(),
            bytes: Some(text.len() as u64),
            role: None,
        };
        let manifest = serde_json::json!({
            "schema_version":"1.0.0","pack_id":pack_id,"pack_kind":"custom",
            "jurisdiction":"Other","version":version,"display_name":"Security fixture",
            "description":null,"content_sha256":canonical_hash(std::slice::from_ref(&file)),
            "files":[file],"headnotes_stripped":false,"signature_alg":"Ed25519","signer_key_id":null
        });
        fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn promote_to_signed_test_official(root: &Path) {
        use ed25519_dalek::Signer;
        let manifest_path = root.join("manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
        manifest["pack_kind"] = serde_json::json!("official");
        manifest["headnotes_stripped"] = serde_json::json!(true);
        manifest["signer_key_id"] = serde_json::json!("caseforge-security-test");
        let content_hash = manifest["content_sha256"].as_str().unwrap();
        let signing = ed25519_dalek::SigningKey::from_bytes(&[7u8; 32]);
        let signature = signing.sign(content_hash.as_bytes());
        fs::write(
            root.join("signature.ed25519"),
            B64.encode(signature.to_bytes()),
        )
        .unwrap();
        fs::write(manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    }

    fn install_error(pack: &Path) -> String {
        let vault = tempfile::tempdir().unwrap();
        install_pack_from_dir(&VaultPaths::new(vault.path().to_path_buf()), pack).unwrap_err()
    }

    #[test]
    fn b_s3_09_malicious_paths_active_content_and_hashes_are_rejected() {
        for path in [
            "../escape.txt",
            "/absolute.txt",
            "C:/drive.txt",
            "content\\alternate.txt",
            "content/../../escape.txt",
            "content/CON.txt",
            "content/file.txt:stream",
        ] {
            assert!(safe_relative_path(path).is_err(), "accepted {path}");
        }

        let active = tempfile::tempdir().unwrap();
        write_v1_custom(
            active.path(),
            "caseforge.security.active",
            "1.0.0",
            "<script>alert('no')</script>",
        );
        assert!(install_error(active.path()).contains("active content"));

        let mismatch = tempfile::tempdir().unwrap();
        write_v1_custom(
            mismatch.path(),
            "caseforge.security.hash",
            "1.0.0",
            "original",
        );
        fs::write(mismatch.path().join("content/law.txt"), "tampered").unwrap();
        assert!(
            install_error(mismatch.path()).contains("byte count mismatch")
                || install_error(mismatch.path()).contains("hash mismatch")
        );

        let unsupported = tempfile::tempdir().unwrap();
        write_v1_custom(
            unsupported.path(),
            "caseforge.security.unsupported",
            "1.0.0",
            "passive",
        );
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(unsupported.path().join("manifest.json")).unwrap(),
        )
        .unwrap();
        fs::rename(
            unsupported.path().join("content/law.txt"),
            unsupported.path().join("content/law.html"),
        )
        .unwrap();
        manifest["files"][0]["path"] = serde_json::json!("content/law.html");
        fs::write(
            unsupported.path().join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        assert!(install_error(unsupported.path()).contains("unsupported content"));

        let invalid = tempfile::tempdir().unwrap();
        write_v1_custom(invalid.path(), "../bad", "1.0.0", "passive");
        assert!(install_error(invalid.path()).contains("invalid pack_id"));

        let bad_version = tempfile::tempdir().unwrap();
        write_v1_custom(
            bad_version.path(),
            "caseforge.security.version",
            "latest",
            "passive",
        );
        assert!(install_error(bad_version.path()).contains("semantic version"));

        let unlisted = tempfile::tempdir().unwrap();
        write_v1_custom(
            unlisted.path(),
            "caseforge.security.unlisted",
            "1.0.0",
            "passive",
        );
        fs::write(unlisted.path().join("content/payload.exe"), b"MZ").unwrap();
        assert!(install_error(unlisted.path()).contains("unlisted file"));

        let oversized = tempfile::tempdir().unwrap();
        write_v1_custom(
            oversized.path(),
            "caseforge.security.oversized",
            "1.0.0",
            "passive",
        );
        let large = oversized.path().join("content/law.txt");
        fs::OpenOptions::new()
            .write(true)
            .open(&large)
            .unwrap()
            .set_len(MAX_PACK_FILE_BYTES + 1)
            .unwrap();
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(oversized.path().join("manifest.json")).unwrap(),
        )
        .unwrap();
        manifest["files"][0]["bytes"] = serde_json::json!(MAX_PACK_FILE_BYTES + 1);
        fs::write(
            oversized.path().join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        assert!(install_error(oversized.path()).contains("oversized"));

        let signature = tempfile::tempdir().unwrap();
        write_v1_custom(
            signature.path(),
            "caseforge.security.signature",
            "1.0.0",
            "passive authority",
        );
        let mut manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(signature.path().join("manifest.json")).unwrap(),
        )
        .unwrap();
        manifest["pack_kind"] = serde_json::json!("official");
        manifest["headnotes_stripped"] = serde_json::json!(true);
        manifest["signer_key_id"] = serde_json::json!("caseforge-official-dev-2026");
        fs::write(
            signature.path().join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        fs::write(signature.path().join("signature.ed25519"), "invalid").unwrap();
        assert!(install_error(signature.path()).contains("CF_PACK_INVALID_SIGNATURE"));

        let trust = tempfile::tempdir().unwrap();
        write_v1_custom(trust.path(), "caseforge.security.trust", "1.0.0", "passive");
        let mut trust_manifest = read_manifest(trust.path()).unwrap();
        trust_manifest.schema_version = "2.0.0".into();
        trust_manifest.metadata_version = Some("2.0.0".into());
        trust_manifest.jurisdiction_path = vec!["Other".into()];
        trust_manifest.trust_level = Some("OFFICIAL_VERIFIED".into());
        trust_manifest.controlled_beta_eligible = false;
        trust_manifest.authoritative_content_sha256 =
            Some(authoritative_hash(&trust_manifest.files));
        trust_manifest.sources_file = Some("metadata/legal_sources.json".into());
        trust_manifest.chunks_file = Some("metadata/legal_chunks.json".into());
        trust_manifest.files[0].role = Some("AUTHORITATIVE_CONTENT".into());
        let canonical = trust.path().canonicalize().unwrap();
        assert!(
            validate_pack_tree(trust.path(), &canonical, &trust_manifest)
                .unwrap_err()
                .contains("manifest v2 schema semantics")
        );
    }

    #[test]
    fn b_s3_09_failed_upgrade_preserves_prior_pack() {
        let vault = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(vault.path().join("vault"));
        let first = tempfile::tempdir().unwrap();
        let pack_id = "caseforge.security.rollback";
        write_v1_custom(first.path(), pack_id, "1.0.0", "working prior version");
        install_pack_from_dir(&paths, first.path()).unwrap();
        let installed_path = paths
            .packs_custom_dir()
            .join(pack_id)
            .join("content/law.txt");
        assert_eq!(
            fs::read_to_string(&installed_path).unwrap(),
            "working prior version"
        );

        let upgrade = tempfile::tempdir().unwrap();
        write_v1_custom(upgrade.path(), pack_id, "1.1.0", "new staged version");
        std::env::set_var("CASEFORGE_PACK_FORCE_POST_STAGE_FAIL", pack_id);
        let result = install_pack_from_dir(&paths, upgrade.path());
        std::env::remove_var("CASEFORGE_PACK_FORCE_POST_STAGE_FAIL");
        assert!(result.is_err());
        assert_eq!(
            fs::read_to_string(&installed_path).unwrap(),
            "working prior version"
        );
        let listed = list_installed(&paths);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].version, "1.0.0");
    }

    #[test]
    fn b_s3_09_protection_failure_is_fail_closed() {
        let pack = tempfile::tempdir().unwrap();
        fs::create_dir_all(pack.path().join("content")).unwrap();
        fs::write(pack.path().join("content/a.txt"), "authority").unwrap();
        std::env::set_var("CASEFORGE_PROTECTION_FORCE_FAIL_SCOPE", "official.fixture");
        let result = crate::protection::protect_pack_content(pack.path(), "official.fixture");
        std::env::remove_var("CASEFORGE_PROTECTION_FORCE_FAIL_SCOPE");
        assert!(result.is_err());
        assert!(pack.path().join("content/a.txt").is_file());
        assert!(!pack.path().join("protection_manifest.json").exists());
    }

    #[test]
    fn b_s3_09_signed_official_positive_and_protection_rollback() {
        let vault = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(vault.path().join("vault"));
        let pack_id = "caseforge.security.official";
        let first = tempfile::tempdir().unwrap();
        write_v1_custom(first.path(), pack_id, "1.0.0", "official authority text");
        promote_to_signed_test_official(first.path());
        let installed = install_pack_from_dir(&paths, first.path()).unwrap();
        assert!(installed.pack.signature_ok);
        assert!(installed.security_report.protection_verified);
        assert!(installed.security_report.transactional_upgrade);
        let dest = paths.packs_official_dir().join(pack_id);
        assert!(crate::protection::verify_pack_integrity(&dest, pack_id).ok);

        let upgrade = tempfile::tempdir().unwrap();
        write_v1_custom(
            upgrade.path(),
            pack_id,
            "1.1.0",
            "replacement authority text",
        );
        promote_to_signed_test_official(upgrade.path());
        std::env::set_var("CASEFORGE_PROTECTION_FORCE_FAIL_SCOPE", pack_id);
        let result = install_pack_from_dir(&paths, upgrade.path());
        std::env::remove_var("CASEFORGE_PROTECTION_FORCE_FAIL_SCOPE");
        assert!(result.unwrap_err().contains("official protection failed"));
        assert!(crate::protection::verify_pack_integrity(&dest, pack_id).ok);
        assert_eq!(list_installed(&paths)[0].version, "1.0.0");
    }

    #[test]
    fn b_s3_09_symlink_or_reparse_is_rejected_when_platform_can_create_it() {
        let pack = tempfile::tempdir().unwrap();
        write_v1_custom(pack.path(), "caseforge.security.link", "1.0.0", "passive");
        let outside = tempfile::NamedTempFile::new().unwrap();
        let link = pack.path().join("content/link.txt");
        #[cfg(windows)]
        let created = if std::os::windows::fs::symlink_file(outside.path(), &link).is_ok() {
            true
        } else {
            let outside_dir = tempfile::tempdir().unwrap();
            let junction = pack.path().join("content/linkdir");
            let quote = |p: &Path| p.display().to_string().replace('\'', "''");
            let command = format!(
                "New-Item -ItemType Junction -Path '{}' -Target '{}' | Out-Null",
                quote(&junction),
                quote(outside_dir.path())
            );
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &command])
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        };
        #[cfg(unix)]
        let created = std::os::unix::fs::symlink(outside.path(), &link).is_ok();
        assert!(
            created,
            "test environment could not create symlink/reparse fixture"
        );
        assert!(install_error(pack.path()).contains("symlink/reparse"));
    }
}
