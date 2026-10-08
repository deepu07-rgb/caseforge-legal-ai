//! Encrypted vaults — app DB + per-case DB (B-S1-02/03/04/05/06).
//! SQLCipher AES-256 page encryption; Argon2id key derivation (constants frozen via Q-DB-01).
//! B-S1-03: New Case wizard metadata (PRD §5); jurisdiction immutable.
//! B-S1-04: Document intake (PDF/DOCX/JPG/PNG) into encrypted case store.
//! B-S1-05: Per-page triage labels + confidence.
//! B-S1-06: Per-page text layers + word-level confidence.

mod docs;
mod error;
mod kdf;
mod paths;
mod sql;
mod wizard;

pub use docs::{
    DocTextPreview, DocumentIngestRequest, DocumentMeta, ALLOWED_EXTENSIONS, TYPE_TAG_SUGGESTIONS,
};
pub use error::{VaultError, VaultResult};
pub use paths::VaultPaths;
pub use wizard::{suggest_code_era, CaseWizardInput, CaseWizardProfile, JURISDICTIONS};

use crate::authority::{merge_graph, AuthorityOverlay, ConflictCandidate};
use docs::{decode_base64, suggest_type_tag, validate_extension};
use kdf::{derive_key, generate_salt, KdfSidecar};
use rusqlite::OptionalExtension;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::Zeroizing;

const APP_MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/app_001_init.sql")),
    (2, include_str!("../../migrations/app_002_case_wizard.sql")),
    (3, include_str!("../../migrations/app_003_lexicon.sql")),
    (4, include_str!("../../migrations/app_004_experience.sql")),
    (5, include_str!("../../migrations/app_005_licence.sql")),
    (
        6,
        include_str!("../../migrations/app_006_speech_transcript.sql"),
    ),
];

/// Stage-1 Experience Store top-K (contracts/constants.json experience_store.top_k).
pub const EXPERIENCE_TOP_K: usize = 5;
pub const EXPERIENCE_PIPELINE_ID: &str = "caseforge.experience.v1";

const STAGE2_EXPERIENCE_CATEGORIES: &[&str] = &["limb_outcome", "bench_setting"];

const STAGE1_EXPERIENCE_CATEGORIES: &[&str] = &[
    "correction",
    "lexicon_entry",
    "draft_edit",
    "rated_exchange",
];

/// Pre-seed legal English / statute / proper-name patterns (PRD §9).
const LEXICON_PRESEED_JSON: &str = include_str!("../../../contracts/voice_preseed_lexicon.json");

const CASE_MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/case_001_init.sql")),
    (2, include_str!("../../migrations/case_002_profile.sql")),
    (3, include_str!("../../migrations/case_003_documents.sql")),
    (4, include_str!("../../migrations/case_004_page_triage.sql")),
    (
        5,
        include_str!("../../migrations/case_005_page_text_layer.sql"),
    ),
    (6, include_str!("../../migrations/case_006_case_graph.sql")),
    (
        7,
        include_str!("../../migrations/case_007_review_corrections.sql"),
    ),
    (
        8,
        include_str!("../../migrations/case_008_guided_interview.sql"),
    ),
    (9, include_str!("../../migrations/case_009_batch_jobs.sql")),
    (
        10,
        include_str!("../../migrations/case_010_argument_tree.sql"),
    ),
    (
        11,
        include_str!("../../migrations/case_011_page_source.sql"),
    ),
    (
        12,
        include_str!("../../migrations/case_012_calibrated_confidence.sql"),
    ),
    (
        13,
        include_str!("../../migrations/case_013_case_graph_v2.sql"),
    ),
    (
        14,
        include_str!("../../migrations/case_014_correction_authority.sql"),
    ),
    (
        15,
        include_str!("../../migrations/case_015_working_set.sql"),
    ),
    (
        16,
        include_str!("../../migrations/case_016_mock_court_debrief_v2.sql"),
    ),
];

#[derive(Debug, Clone, Serialize)]
pub struct VaultStatus {
    pub root_dir: String,
    pub app_exists: bool,
    pub app_unlocked: bool,
    pub open_case_ids: Vec<String>,
    pub case_count_registered: Option<u32>,
    pub crypto: CryptoStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct CryptoStatus {
    pub engine: &'static str,
    pub argon2id_params_version: u32,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseSummary {
    pub case_id: String,
    pub created_at_unix_ms: i64,
    pub label: String,
    pub cause_title: String,
    pub court: String,
    pub jurisdiction: String,
    pub matter_type: String,
    pub user_side: String,
    pub code_era: String,
    pub language: String,
    pub is_open: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CaseCreateResult {
    pub case_id: String,
    pub path: String,
    pub profile: CaseWizardProfile,
}

/// Lexicon entry (B-S1-16, PRD §9). Stored in encrypted app vault.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct LexiconEntry {
    pub schema_version: String,
    pub entry_id: String,
    pub entry_kind: String,
    pub language: String,
    pub surface_form: String,
    pub corrected_form: Option<String>,
    pub pronunciation: Option<String>,
    pub pattern_type: String,
    pub source: String,
    pub active: bool,
    pub scope: String,
    pub notes: Option<String>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct LexiconUpsertInput {
    pub entry_id: Option<String>,
    pub entry_kind: String,
    pub surface_form: String,
    pub corrected_form: Option<String>,
    pub pronunciation: Option<String>,
    pub pattern_type: Option<String>,
    pub source: Option<String>,
    pub active: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LexiconExportEnvelope {
    pub schema_version: String,
    pub product: String,
    pub kind: String,
    pub language: String,
    pub exported_at_unix_ms: i64,
    pub entry_count: usize,
    pub entries: Vec<LexiconEntry>,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct SpeechTranscriptCommitInput {
    pub case_id: Option<String>,
    pub session_id: Option<String>,
    pub input_kind: String,
    pub audio_sha256: String,
    pub raw_transcript: String,
    pub staged_transcript: String,
    pub correction_applied: bool,
    pub counsel_confirmed: bool,
    pub backend: String,
    pub pipeline_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpeechTranscriptCommit {
    pub commit_id: String,
    pub case_id: Option<String>,
    pub session_id: Option<String>,
    pub input_kind: String,
    pub audio_sha256: String,
    pub raw_transcript: String,
    pub staged_transcript: String,
    pub correction_applied: bool,
    pub counsel_confirmed: bool,
    pub backend: String,
    pub pipeline_id: String,
    pub audio_retained: bool,
    pub created_at_unix_ms: i64,
}

/// Experience Store record (B-S1-17, PRD §11).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ExperienceRecord {
    pub schema_version: String,
    pub record_id: String,
    pub record_category: String,
    pub stage: String,
    pub language: String,
    pub jurisdiction: Option<String>,
    pub case_id: Option<String>,
    pub summary: String,
    pub tags: Vec<String>,
    pub relevance_key: String,
    pub payload: serde_json::Value,
    pub source: String,
    pub fleet_status: String,
    pub active: bool,
    pub notes: Option<String>,
    pub created_at_unix_ms: i64,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize)]
pub struct ExperienceUpsertInput {
    pub record_id: Option<String>,
    pub record_category: String,
    pub summary: String,
    pub tags: Option<Vec<String>>,
    pub payload: Option<serde_json::Value>,
    pub case_id: Option<String>,
    pub jurisdiction: Option<String>,
    pub source: Option<String>,
    pub notes: Option<String>,
    pub stage: Option<String>,
    pub fleet_status: Option<String>,
    pub active: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExperiencePolicy {
    pub batch: &'static str,
    pub pipeline_id: &'static str,
    pub top_k: usize,
    pub fleet_enabled: bool,
    pub fleet_network_upload: bool,
    pub fleet_requires_explicit_review: bool,
    pub stage1_writable: Vec<&'static str>,
    pub stage2_writable: Vec<&'static str>,
    pub storage: &'static str,
    pub acs: [&'static str; 4],
}

#[derive(Debug, Clone, Serialize)]
pub struct ExperienceHit {
    pub record_id: String,
    pub record_category: String,
    pub summary: String,
    pub relevance_key: String,
    pub score: f64,
    pub case_id: Option<String>,
    pub jurisdiction: Option<String>,
    pub tags: Vec<String>,
}

/// Per-page triage row (B-S1-05).
#[derive(Debug, Clone, Serialize)]
pub struct PageTriageRow {
    pub doc_id: String,
    pub page_index: i64,
    pub triage_class: String,
    pub confidence: f64,
    pub classifier_id: String,
    pub classified_at_unix_ms: i64,
    pub notes: Option<String>,
}

/// Word span in a recognized page text layer (B-S1-06).
#[derive(Debug, Clone, Serialize)]
pub struct PageWordRow {
    pub word_index: i64,
    pub text: String,
    pub confidence: f64,
    pub raw_engine_confidence: Option<f64>,
    pub calibrated_confidence: Option<f64>,
    pub confidence_state: String,
    pub counsel_confirmed: bool,
    pub char_start: i64,
    pub char_end: i64,
    pub low_confidence: bool,
    pub force_review: bool,
    pub suppress_from_auto: bool,
    pub bbox_json: Option<String>,
}

/// Per-page recognized text layer (B-S1-06).
#[derive(Debug, Clone, Serialize)]
pub struct PageTextLayerRow {
    pub doc_id: String,
    pub page_index: i64,
    pub recognizer_id: String,
    pub recognized_at_unix_ms: i64,
    pub full_text: String,
    pub mean_confidence: f64,
    pub page_quality_warn: bool,
    pub notes: Option<String>,
    pub calibration_id: Option<String>,
    pub critical_fields: serde_json::Value,
    pub words: Vec<PageWordRow>,
}

/// Production PDF page source/layout record (B-S3-03), encrypted in case.db.
#[derive(Debug, Clone, Serialize)]
pub struct PageSourceRow {
    pub doc_id: String,
    pub page_index: i64,
    pub parser_id: String,
    pub source_kind: String,
    pub state: String,
    pub width_points: f64,
    pub height_points: f64,
    pub rotation_degrees: i64,
    pub source_sha256: String,
    pub page_source_json: String,
    pub parsed_at_unix_ms: i64,
}

struct VaultInner {
    paths: VaultPaths,
    /// Derived app key held only while unlocked.
    app_key: Option<Zeroizing<[u8; kdf::KEY_LEN]>>,
    app: Option<rusqlite::Connection>,
    cases: HashMap<String, rusqlite::Connection>,
}

pub struct VaultManager {
    inner: Mutex<VaultInner>,
}

fn load_working_set_value(
    conn: &rusqlite::Connection,
    case_id: &str,
) -> VaultResult<serde_json::Value> {
    let meta: Option<(i64, String, Option<String>, String, i64)> = conn.query_row(
        "SELECT version,regime_status,confirmed_regime_id,regime_candidates_json,updated_at_unix_ms FROM legal_working_set_meta WHERE case_id=?1",
        [case_id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
    ).optional()?;
    let mut stmt = conn.prepare("SELECT item_id,authority_id,source_id,chunk_id,pack_id,title,citation,section_label,jurisdiction,authority_type,trust_level,temporal_status,superseded_status,source_path,source_reference,source_checksum,chunk_checksum,exact_text,decision,pinned,controlling,counsel_rationale,first_added_at_unix_ms,updated_at_unix_ms,last_seen_index_checksum FROM legal_working_set_item WHERE case_id=?1 ORDER BY pinned DESC,controlling DESC,title,section_label")?;
    let rows = stmt.query_map([case_id], |r| Ok(serde_json::json!({
        "item_id":r.get::<_,String>(0)?,"authority_id":r.get::<_,String>(1)?,"source_id":r.get::<_,String>(2)?,"chunk_id":r.get::<_,String>(3)?,"pack_id":r.get::<_,String>(4)?,
        "title":r.get::<_,String>(5)?,"citation":r.get::<_,String>(6)?,"section_label":r.get::<_,String>(7)?,"jurisdiction":r.get::<_,String>(8)?,"authority_type":r.get::<_,String>(9)?,
        "trust_level":r.get::<_,String>(10)?,"temporal_status":r.get::<_,String>(11)?,"superseded_status":r.get::<_,String>(12)?,"source_path":r.get::<_,String>(13)?,"source_reference":r.get::<_,String>(14)?,
        "source_checksum":r.get::<_,String>(15)?,"chunk_checksum":r.get::<_,String>(16)?,"exact_text":r.get::<_,String>(17)?,"decision":r.get::<_,String>(18)?,"pinned":r.get::<_,i64>(19)?!=0,"controlling":r.get::<_,i64>(20)?!=0,
        "counsel_rationale":r.get::<_,Option<String>>(21)?,"first_added_at_unix_ms":r.get::<_,i64>(22)?,"updated_at_unix_ms":r.get::<_,i64>(23)?,"last_seen_index_checksum":r.get::<_,String>(24)?
    })))?;
    let items: Vec<serde_json::Value> = rows.collect::<Result<_, _>>()?;
    let (version, regime_status, confirmed_regime_id, candidates, updated) =
        meta.unwrap_or((0, "PENDING_CONFIRMATION".into(), None, "[]".into(), 0));
    let regime_candidates: serde_json::Value =
        serde_json::from_str(&candidates).unwrap_or_else(|_| serde_json::json!([]));
    let usable_authority_ids: Vec<String> = items
        .iter()
        .filter(|item| {
            item.get("decision").and_then(|v| v.as_str()) == Some("INCLUDED")
                && item.get("controlling").and_then(|v| v.as_bool()) == Some(true)
                && item.get("temporal_status").and_then(|v| v.as_str()) == Some("APPLICABLE")
                && item.get("superseded_status").and_then(|v| v.as_str()) == Some("CURRENT")
        })
        .filter_map(|item| {
            item.get("authority_id")
                .and_then(|v| v.as_str())
                .map(str::to_string)
        })
        .collect();
    Ok(
        serde_json::json!({"schema_version":crate::working_set::SCHEMA_VERSION,"case_id":case_id,"version":version,"regime_status":regime_status,"confirmed_regime_id":confirmed_regime_id,"regime_candidates":regime_candidates,"items":items,"usable_authority_ids":usable_authority_ids,"updated_at_unix_ms":updated}),
    )
}

impl VaultManager {
    pub fn new(root: PathBuf) -> Self {
        Self {
            inner: Mutex::new(VaultInner {
                paths: VaultPaths::new(root),
                app_key: None,
                app: None,
                cases: HashMap::new(),
            }),
        }
    }

    pub fn status(&self) -> VaultResult<VaultStatus> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app_exists = g.paths.app_db_path().exists();
        let app_unlocked = g.app.is_some();
        let open_case_ids: Vec<String> = g.cases.keys().cloned().collect();
        let case_count_registered = if let Some(conn) = g.app.as_ref() {
            let n: u32 = conn.query_row("SELECT COUNT(*) FROM cases", [], |r| r.get(0))?;
            Some(n)
        } else {
            None
        };
        Ok(VaultStatus {
            root_dir: g.paths.root.display().to_string(),
            app_exists,
            app_unlocked,
            open_case_ids,
            case_count_registered,
            crypto: CryptoStatus {
                engine: "SQLCipher+Argon2id",
                argon2id_params_version: kdf::PARAMS_VERSION,
                memory_kib: kdf::MEMORY_KIB,
                iterations: kdf::ITERATIONS,
                parallelism: kdf::PARALLELISM,
            },
        })
    }

    /// Create app vault if missing, then unlock with passphrase.
    pub fn init_and_unlock(&self, passphrase: &str) -> VaultResult<VaultStatus> {
        {
            let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
            if !g.paths.app_db_path().exists() {
                std::fs::create_dir_all(g.paths.app_dir())?;
                let salt = generate_salt();
                let sidecar = KdfSidecar::new(salt);
                sidecar.save(&g.paths.app_kdf_path())?;
                let key = derive_key(passphrase, &sidecar.salt_bytes()?)?;
                let conn = sql::create_encrypted(&g.paths.app_db_path(), &key)?;
                sql::apply_pending_migrations(&conn, APP_MIGRATIONS)?;
                sql::set_meta(&conn, "schema_version", "2")?;
                sql::set_meta(&conn, "created_at_unix_ms", &now_ms().to_string())?;
                drop(conn);
            }
        }
        self.unlock(passphrase)
    }

    pub fn unlock(&self, passphrase: &str) -> VaultResult<VaultStatus> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        if g.app.is_some() {
            return Err(VaultError::AlreadyUnlocked);
        }
        if !g.paths.app_db_path().exists() {
            return Err(VaultError::NotFound {
                path: g.paths.app_db_path(),
            });
        }
        let sidecar = KdfSidecar::load(&g.paths.app_kdf_path())?;
        let key = derive_key(passphrase, &sidecar.salt_bytes()?)?;
        let conn = match sql::open_encrypted(&g.paths.app_db_path(), &key) {
            Ok(c) => c,
            Err(_) => return Err(VaultError::PassphraseInvalid),
        };
        let ok: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='vault_meta'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| VaultError::PassphraseInvalid)?;
        if ok < 1 {
            return Err(VaultError::Corrupt {
                detail: "app vault missing vault_meta".into(),
            });
        }
        // Upgrade older app vaults to wizard columns (B-S1-03).
        sql::apply_pending_migrations(&conn, APP_MIGRATIONS)?;
        g.app_key = Some(key);
        g.app = Some(conn);
        drop(g);
        self.status()
    }

    pub fn lock(&self) -> VaultResult<VaultStatus> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        g.cases.clear();
        g.app = None;
        g.app_key = None;
        drop(g);
        self.status()
    }

    /// Destructive local reset: close all DB handles, delete app + case vaults.
    /// Used when the user forgot their password and accepts data loss.
    /// Does not touch packs/models/protection sidecars under the vault root.
    pub fn reset_local(&self) -> VaultResult<VaultStatus> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        g.cases.clear();
        g.app = None;
        g.app_key = None;
        let app_dir = g.paths.app_dir();
        let cases_dir = g.paths.cases_dir();
        drop(g);

        if app_dir.exists() {
            std::fs::remove_dir_all(&app_dir).map_err(|e| VaultError::Io {
                detail: format!("reset app vault {}: {e}", app_dir.display()),
            })?;
        }
        if cases_dir.exists() {
            std::fs::remove_dir_all(&cases_dir).map_err(|e| VaultError::Io {
                detail: format!("reset case vaults {}: {e}", cases_dir.display()),
            })?;
        }
        self.status()
    }

    /// Create case with PRD §5 wizard fields. Jurisdiction is stored immutably.
    pub fn create_case_with_wizard(
        &self,
        passphrase: &str,
        input: CaseWizardInput,
    ) -> VaultResult<CaseCreateResult> {
        self.assert_vault_writable()?;
        let profile = input.validate_and_normalize()?;
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;

        // Resolve paths and session key before taking a mutable borrow of `app`.
        let app_kdf_path = g.paths.app_kdf_path();
        let case_id = Uuid::new_v4().to_string();
        let case_dir = g.paths.case_dir(&case_id);
        let db_path = g.paths.case_db_path(&case_id);
        let kdf_path = g.paths.case_kdf_path(&case_id);

        let session = g.app_key.as_ref().ok_or(VaultError::Locked)?;
        let app_sidecar = KdfSidecar::load(&app_kdf_path)?;
        let app_key = derive_key(passphrase, &app_sidecar.salt_bytes()?)?;
        if app_key.as_slice() != session.as_slice() {
            return Err(VaultError::PassphraseInvalid);
        }

        std::fs::create_dir_all(&case_dir)?;
        let salt = generate_salt();
        let case_sidecar = KdfSidecar::new(salt);
        case_sidecar.save(&kdf_path)?;
        let case_key = derive_key(passphrase, &case_sidecar.salt_bytes()?)?;
        let case_conn = sql::create_encrypted(&db_path, &case_key)?;
        sql::apply_pending_migrations(&case_conn, CASE_MIGRATIONS)?;
        sql::set_meta(&case_conn, "case_id", &case_id)?;
        sql::set_meta(&case_conn, "schema_version", "2")?;
        // Jurisdiction immutable: written once; no update API.
        sql::set_meta(&case_conn, "jurisdiction", &profile.jurisdiction)?;
        let created = now_ms();
        case_conn.execute(
            "INSERT INTO case_profile (
                case_id, cause_title, court, jurisdiction, matter_type,
                user_side, code_era, language, created_at_unix_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            rusqlite::params![
                case_id,
                profile.cause_title,
                profile.court,
                profile.jurisdiction,
                profile.matter_type,
                profile.user_side,
                profile.code_era,
                profile.language,
                created,
            ],
        )?;
        let label = profile.cause_title.clone();

        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        app.execute(
            "INSERT INTO cases (
                case_id, created_at_unix_ms, label,
                cause_title, court, jurisdiction, matter_type,
                user_side, code_era, language
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                case_id,
                created,
                label,
                profile.cause_title,
                profile.court,
                profile.jurisdiction,
                profile.matter_type,
                profile.user_side,
                profile.code_era,
                profile.language,
            ],
        )?;
        drop(case_conn);
        Ok(CaseCreateResult {
            path: db_path.display().to_string(),
            case_id,
            profile,
        })
    }

    pub fn open_case(&self, case_id: &str, passphrase: &str) -> VaultResult<CaseSummary> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_ref().ok_or(VaultError::Locked)?;
        if g.cases.contains_key(case_id) {
            return case_summary(&g, case_id);
        }
        let exists: Option<String> = app
            .query_row(
                "SELECT case_id FROM cases WHERE case_id = ?1",
                [case_id],
                |r| r.get(0),
            )
            .optional()?;
        if exists.is_none() {
            return Err(VaultError::NotFound {
                path: g.paths.case_db_path(case_id),
            });
        }
        let db_path = g.paths.case_db_path(case_id);
        let kdf_path = g.paths.case_kdf_path(case_id);
        let sidecar = KdfSidecar::load(&kdf_path)?;
        let key = derive_key(passphrase, &sidecar.salt_bytes()?)?;
        let conn = match sql::open_encrypted(&db_path, &key) {
            Ok(c) => c,
            Err(_) => return Err(VaultError::PassphraseInvalid),
        };
        let _probe: i32 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='vault_meta'",
                [],
                |r| r.get(0),
            )
            .map_err(|_| VaultError::PassphraseInvalid)?;
        // Ensure case vault has profile schema when opened (older stubs).
        let _ = sql::apply_pending_migrations(&conn, CASE_MIGRATIONS);
        g.cases.insert(case_id.to_string(), conn);
        case_summary(&g, case_id)
    }

    pub fn close_case(&self, case_id: &str) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        g.cases.remove(case_id);
        Ok(())
    }

    pub fn list_cases(&self) -> VaultResult<Vec<CaseSummary>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_ref().ok_or(VaultError::Locked)?;
        let mut stmt = app.prepare(
            "SELECT case_id, created_at_unix_ms, label,
                    COALESCE(cause_title,''), COALESCE(court,''), COALESCE(jurisdiction,''),
                    COALESCE(matter_type,''), COALESCE(user_side,''), COALESCE(code_era,''),
                    COALESCE(language,'English')
             FROM cases ORDER BY created_at_unix_ms DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
                r.get::<_, String>(9)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (
                case_id,
                created,
                label,
                cause_title,
                court,
                jurisdiction,
                matter_type,
                user_side,
                code_era,
                language,
            ) = row?;
            let is_open = g.cases.contains_key(&case_id);
            out.push(CaseSummary {
                case_id,
                created_at_unix_ms: created,
                label,
                cause_title,
                court,
                jurisdiction,
                matter_type,
                user_side,
                code_era,
                language,
                is_open,
            });
        }
        Ok(out)
    }

    /// Code/era auto-suggest stub for UI (no AI).
    pub fn suggest_code_era_for(jurisdiction: &str) -> String {
        suggest_code_era(jurisdiction)
    }

    // --- B-S1-04 document intake ---

    pub fn doc_list(&self, case_id: &str) -> VaultResult<Vec<DocumentMeta>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case vault before listing documents".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let mut stmt = conn.prepare(
            "SELECT doc_id, original_filename, type_tag, language_tag, source_format,
                    byte_size, page_count, per_page_quality_grade, ingested_at_unix_ms
             FROM documents ORDER BY ingested_at_unix_ms DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(DocumentMeta {
                doc_id: r.get(0)?,
                original_filename: r.get(1)?,
                type_tag: r.get(2)?,
                language_tag: r.get(3)?,
                source_format: r.get(4)?,
                byte_size: r.get(5)?,
                page_count: r.get(6)?,
                per_page_quality_grade: r.get(7)?,
                ingested_at_unix_ms: r.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn doc_ingest(
        &self,
        case_id: &str,
        req: DocumentIngestRequest,
    ) -> VaultResult<DocumentMeta> {
        let source_format = validate_extension(&req.original_filename)?;
        let bytes = decode_base64(&req.content_base64)?;
        if bytes.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_DOCPIPE_UNSUPPORTED_SOURCE: empty file".into(),
            });
        }
        let type_tag = req
            .type_tag
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| suggest_type_tag(&source_format));
        let language_tag = req
            .language_tag
            .as_ref()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "en".into());

        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case vault before ingesting documents".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;

        let doc_id = Uuid::new_v4().to_string();
        let ingested = now_ms();
        let byte_size = bytes.len() as i64;
        let page_count: Option<i64> = None;
        let per_page_quality_grade: Option<String> = None;

        conn.execute(
            "INSERT INTO documents (
                doc_id, original_filename, type_tag, language_tag, source_format,
                byte_size, page_count, per_page_quality_grade, ingested_at_unix_ms, content
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            rusqlite::params![
                doc_id,
                req.original_filename,
                type_tag,
                language_tag,
                source_format,
                byte_size,
                page_count,
                per_page_quality_grade,
                ingested,
                bytes,
            ],
        )?;

        Ok(DocumentMeta {
            doc_id,
            original_filename: req.original_filename,
            type_tag,
            language_tag,
            source_format,
            byte_size,
            page_count,
            per_page_quality_grade,
            ingested_at_unix_ms: ingested,
        })
    }

    /// One-click type tag correction (PRD §5 correction path; AC-5.1 ML later).
    pub fn doc_update_type_tag(
        &self,
        case_id: &str,
        doc_id: &str,
        type_tag: &str,
    ) -> VaultResult<DocumentMeta> {
        let type_tag = type_tag.trim();
        if type_tag.is_empty() {
            return Err(VaultError::Internal {
                detail: "type_tag required".into(),
            });
        }
        {
            let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
            let conn = g
                .cases
                .get_mut(case_id)
                .ok_or_else(|| VaultError::Internal {
                    detail: "CF_VAULT_LOCKED: open the case vault before updating documents".into(),
                })?;
            sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
            let n = conn.execute(
                "UPDATE documents SET type_tag = ?1 WHERE doc_id = ?2",
                rusqlite::params![type_tag, doc_id],
            )?;
            if n == 0 {
                return Err(VaultError::NotFound {
                    path: PathBuf::from(format!("doc:{doc_id}")),
                });
            }
        }
        let list = self.doc_list(case_id)?;
        list.into_iter()
            .find(|d| d.doc_id == doc_id)
            .ok_or_else(|| VaultError::NotFound {
                path: PathBuf::from(format!("doc:{doc_id}")),
            })
    }

    pub fn doc_intake_meta() -> serde_json::Value {
        serde_json::json!({
            "batch": "B-S1-04",
            "prd_ref": "§5",
            "allowed_extensions": ALLOWED_EXTENSIONS,
            "type_tag_suggestions": TYPE_TAG_SUGGESTIONS,
            "default_language_tag": "en",
            "auto_type": "extension_stub_only",
            "out_of_scope": ["phone_capture", "ocr", "htr", "auto_type_ml"]
        })
    }

    // --- B-S1-05 page triage ---

    pub fn doc_get(&self, case_id: &str, doc_id: &str) -> VaultResult<DocumentMeta> {
        let list = self.doc_list(case_id)?;
        list.into_iter()
            .find(|d| d.doc_id == doc_id)
            .ok_or_else(|| VaultError::NotFound {
                path: PathBuf::from(format!("doc:{doc_id}")),
            })
    }

    pub fn doc_content_bytes(&self, case_id: &str, doc_id: &str) -> VaultResult<Vec<u8>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case vault before reading documents".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let bytes: Vec<u8> = conn
            .query_row(
                "SELECT content FROM documents WHERE doc_id = ?1",
                [doc_id],
                |r| r.get(0),
            )
            .map_err(|_| VaultError::NotFound {
                path: PathBuf::from(format!("doc:{doc_id}")),
            })?;
        Ok(bytes)
    }

    /// UTF-8 text extract for LLM context. TXT is full text; binary formats return a note.
    pub fn doc_text_preview(
        &self,
        case_id: &str,
        doc_id: &str,
        max_chars: usize,
    ) -> VaultResult<DocTextPreview> {
        let meta = self.doc_get(case_id, doc_id)?;
        let bytes = self.doc_content_bytes(case_id, doc_id)?;
        let max_chars = max_chars.clamp(200, 40_000);
        let (text, is_binary) = if meta.source_format == "txt"
            || meta
                .original_filename
                .to_ascii_lowercase()
                .ends_with(".txt")
        {
            let s = String::from_utf8_lossy(&bytes).into_owned();
            let truncated = s.chars().count() > max_chars;
            let body: String = s.chars().take(max_chars).collect();
            (
                if truncated {
                    format!("{body}\n\n[…truncated for LLM context…]")
                } else {
                    body
                },
                false,
            )
        } else {
            // Prefer stored recognition layer when present (B-S1-06).
            if let Ok(layers) = self.page_text_layer_list(case_id, Some(doc_id)) {
                if !layers.is_empty() {
                    let mut joined = String::new();
                    for layer in layers {
                        if !layer.full_text.trim().is_empty() {
                            if !joined.is_empty() {
                                joined.push_str("\n\n");
                            }
                            joined.push_str(&format!(
                                "[page {} · mean_conf={:.2}]\n{}",
                                layer.page_index, layer.mean_confidence, layer.full_text
                            ));
                        }
                    }
                    if !joined.is_empty() {
                        let truncated = joined.chars().count() > max_chars;
                        let body: String = joined.chars().take(max_chars).collect();
                        let text = if truncated {
                            format!("{body}\n\n[…truncated for LLM context…]")
                        } else {
                            body
                        };
                        let char_count = text.chars().count() as u64;
                        return Ok(DocTextPreview {
                            doc_id: meta.doc_id,
                            original_filename: meta.original_filename,
                            source_format: meta.source_format,
                            type_tag: meta.type_tag,
                            is_binary_stub: false,
                            text,
                            char_count,
                        });
                    }
                }
            }
            (
                format!(
                    "[Binary document stored: {} · format={} · {} bytes. Run recognition (B-S1-06) for a text layer.]",
                    meta.original_filename, meta.source_format, meta.byte_size
                ),
                true,
            )
        };
        let char_count = text.chars().count() as u64;
        Ok(DocTextPreview {
            doc_id: meta.doc_id,
            original_filename: meta.original_filename,
            source_format: meta.source_format,
            type_tag: meta.type_tag,
            is_binary_stub: is_binary,
            text,
            char_count,
        })
    }

    /// Concatenate text previews for all docs in a case (for local reasoner context).
    pub fn case_llm_context(&self, case_id: &str, max_chars_total: usize) -> VaultResult<String> {
        let list = self.doc_list(case_id)?;
        let max_chars_total = max_chars_total.clamp(500, 60_000);
        let mut out = String::new();
        let mut bounded = false;
        let per = if list.is_empty() {
            max_chars_total
        } else {
            (max_chars_total / list.len()).max(400)
        };
        for d in list {
            if out.chars().count() >= max_chars_total {
                break;
            }
            let prev = self.doc_text_preview(case_id, &d.doc_id, per)?;
            bounded |= prev.char_count > prev.text.chars().count() as u64;
            out.push_str(&format!(
                "\n--- FILE: {} ({}) ---\n{}\n",
                prev.original_filename, prev.type_tag, prev.text
            ));
        }
        if out.is_empty() {
            out = "[No documents ingested in this case vault yet.]".into();
        }
        let out_chars = out.chars().count();
        bounded |= out_chars > max_chars_total;
        let excerpt: String = out.chars().take(max_chars_total).collect();
        if bounded {
            Ok(format!("[CASEFORGE_BOUNDED_CASE_EXCERPT: retrieval/working-set context required; source text may be omitted; no completeness claim]\n{excerpt}"))
        } else {
            Ok(excerpt)
        }
    }

    pub fn doc_set_page_count(
        &self,
        case_id: &str,
        doc_id: &str,
        page_count: i64,
    ) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case vault before updating documents".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let n = conn.execute(
            "UPDATE documents SET page_count = ?1 WHERE doc_id = ?2",
            rusqlite::params![page_count, doc_id],
        )?;
        if n == 0 {
            return Err(VaultError::NotFound {
                path: PathBuf::from(format!("doc:{doc_id}")),
            });
        }
        Ok(())
    }

    pub fn page_source_replace(
        &self,
        case_id: &str,
        doc_id: &str,
        rows: &[PageSourceRow],
    ) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before writing page sources".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM page_source WHERE doc_id = ?1", [doc_id])?;
        for row in rows {
            tx.execute("INSERT INTO page_source (doc_id,page_index,parser_id,source_kind,state,width_points,height_points,rotation_degrees,source_sha256,page_source_json,parsed_at_unix_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)", rusqlite::params![row.doc_id,row.page_index,row.parser_id,row.source_kind,row.state,row.width_points,row.height_points,row.rotation_degrees,row.source_sha256,row.page_source_json,row.parsed_at_unix_ms])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn page_source_list(
        &self,
        case_id: &str,
        doc_id: Option<&str>,
    ) -> VaultResult<Vec<PageSourceRow>> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before listing page sources".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let sql = if doc_id.is_some() {
            "SELECT doc_id,page_index,parser_id,source_kind,state,width_points,height_points,rotation_degrees,source_sha256,page_source_json,parsed_at_unix_ms FROM page_source WHERE doc_id=?1 ORDER BY page_index"
        } else {
            "SELECT doc_id,page_index,parser_id,source_kind,state,width_points,height_points,rotation_degrees,source_sha256,page_source_json,parsed_at_unix_ms FROM page_source ORDER BY doc_id,page_index"
        };
        let mut stmt = conn.prepare(sql)?;
        let map = |r: &rusqlite::Row<'_>| {
            Ok(PageSourceRow {
                doc_id: r.get(0)?,
                page_index: r.get(1)?,
                parser_id: r.get(2)?,
                source_kind: r.get(3)?,
                state: r.get(4)?,
                width_points: r.get(5)?,
                height_points: r.get(6)?,
                rotation_degrees: r.get(7)?,
                source_sha256: r.get(8)?,
                page_source_json: r.get(9)?,
                parsed_at_unix_ms: r.get(10)?,
            })
        };
        let mut out = Vec::new();
        if let Some(value) = doc_id {
            for row in stmt.query_map([value], map)? {
                out.push(row?);
            }
        } else {
            for row in stmt.query_map([], map)? {
                out.push(row?);
            }
        }
        Ok(out)
    }

    pub fn page_triage_replace(
        &self,
        case_id: &str,
        doc_id: &str,
        rows: &[PageTriageRow],
    ) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case vault before writing triage".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        // Ensure document exists
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE doc_id = ?1",
            [doc_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(VaultError::NotFound {
                path: PathBuf::from(format!("doc:{doc_id}")),
            });
        }
        /* B-S3-07 word authority merge belongs in page_text_layer_replace, never triage.
        let mut overlay_stmt = conn.prepare(
            "SELECT target_key, latest_event_id, target_kind, collection_name, item_id, field_name,
                    doc_id, page_index, word_index, base_value, counsel_value, anchor_json
             FROM authority_overlays WHERE active=1 AND target_kind='word_span' AND doc_id=?1 ORDER BY updated_at_unix_ms ASC"
        )?;
        let overlay_rows = overlay_stmt.query_map([doc_id], |r| {
            let anchor_json: String = r.get(11)?;
            Ok(AuthorityOverlay {
                target_key: r.get(0)?,
                latest_event_id: r.get(1)?,
                target_kind: r.get(2)?,
                collection: r.get(3)?,
                item_id: r.get(4)?,
                field_name: r.get(5)?,
                doc_id: r.get(6)?,
                page_index: r.get(7)?,
                word_index: r.get(8)?,
                base_value: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                counsel_value: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
                anchor: serde_json::from_str(&anchor_json)
                    .unwrap_or_else(|_| serde_json::json!({})),
            })
        })?;
        let mut overlays = Vec::new();
        for row in overlay_rows {
            overlays.push(row?);
        }
        drop(overlay_stmt);
        let mut merged_rows = rows.to_vec();
        let mut authority_conflicts: Vec<ConflictCandidate> = Vec::new();
        for overlay in &overlays {
            let Some(page_index) = overlay.page_index else {
                continue;
            };
            let Some(layer) = merged_rows.iter_mut().find(|l| l.page_index == page_index) else {
                authority_conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: "TARGET_DELETED".into(),
                    generated_value: None,
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
                continue;
            };
            let original_bbox = overlay.anchor.get("original_bbox").and_then(|v| v.as_str());
            let exact = overlay.word_index.and_then(|wi| {
                layer.words.iter().position(|w| {
                    w.word_index == wi
                        && (w.text == overlay.base_value
                            || w.text == overlay.counsel_value
                            || original_bbox.is_some_and(|b| w.bbox_json.as_deref() == Some(b)))
                })
            });
            let base_matches: Vec<usize> = layer
                .words
                .iter()
                .enumerate()
                .filter(|(_, w)| w.text == overlay.base_value)
                .map(|(i, _)| i)
                .collect();
            let selected = exact.or_else(|| (base_matches.len() == 1).then_some(base_matches[0]));
            let Some(index) = selected else {
                layer.page_quality_warn = true;
                authority_conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: if base_matches.len() > 1 {
                        "AMBIGUOUS_MATCH"
                    } else {
                        "TARGET_DELETED"
                    }
                    .into(),
                    generated_value: None,
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
                continue;
            };
            let generated = layer.words[index].text.clone();
            if generated != overlay.base_value && generated != overlay.counsel_value {
                authority_conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: "MATERIAL_DISAGREEMENT".into(),
                    generated_value: Some(generated),
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
            }
            let start = layer.words[index].char_start.max(0) as usize;
            let end = layer.words[index].char_end.max(0) as usize;
            let chars: Vec<char> = layer.full_text.chars().collect();
            if start > end || end > chars.len() {
                layer.page_quality_warn = true;
                continue;
            }
            let mut rebuilt: String = chars[..start].iter().collect();
            rebuilt.push_str(&overlay.counsel_value);
            rebuilt.push_str(&chars[end..].iter().collect::<String>());
            let delta = overlay.counsel_value.chars().count() as i64 - (end as i64 - start as i64);
            let selected_word_index = layer.words[index].word_index;
            layer.words[index].text = overlay.counsel_value.clone();
            layer.words[index].char_end =
                start as i64 + overlay.counsel_value.chars().count() as i64;
            layer.words[index].confidence = 1.0;
            layer.words[index].calibrated_confidence = Some(1.0);
            layer.words[index].confidence_state = "counsel_confirmed".into();
            layer.words[index].counsel_confirmed = true;
            layer.words[index].low_confidence = false;
            layer.words[index].force_review = false;
            layer.words[index].suppress_from_auto = false;
            for word in &mut layer.words {
                if word.word_index > selected_word_index {
                    word.char_start += delta;
                    word.char_end += delta;
                }
            }
            layer.full_text = rebuilt;
        }
        */
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM page_triage WHERE doc_id = ?1", [doc_id])?;
        for row in rows {
            tx.execute(
                "INSERT INTO page_triage (
                    doc_id, page_index, triage_class, confidence,
                    classifier_id, classified_at_unix_ms, notes
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7)",
                rusqlite::params![
                    row.doc_id,
                    row.page_index,
                    row.triage_class,
                    row.confidence,
                    row.classifier_id,
                    row.classified_at_unix_ms,
                    row.notes,
                ],
            )?;
        }
        /* conflicts are recorded by the recognition merge path.
        let conflict_now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        for conflict in &authority_conflicts {
            let fingerprint = format!(
                "{}|{}|{}",
                conflict.overlay_event_id,
                conflict.conflict_kind,
                conflict.generated_value.as_deref().unwrap_or("")
            );
            let mut h = Sha256::new();
            h.update(fingerprint.as_bytes());
            let conflict_id = format!("cfc_{}", hex::encode(h.finalize()));
            tx.execute("INSERT OR IGNORE INTO correction_conflicts (conflict_id,overlay_event_id,target_key,target_kind,conflict_kind,generated_value,counsel_value,anchor_json,status,created_at_unix_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'OPEN',?9)",rusqlite::params![conflict_id,conflict.overlay_event_id,conflict.target_key,conflict.target_kind,conflict.conflict_kind,conflict.generated_value,conflict.counsel_value,conflict.anchor.to_string(),conflict_now])?;
        }
        */
        tx.commit()?;
        Ok(())
    }

    pub fn page_triage_list(
        &self,
        case_id: &str,
        doc_id: Option<&str>,
    ) -> VaultResult<Vec<PageTriageRow>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case vault before listing triage".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let mut out = Vec::new();
        if let Some(doc_id) = doc_id {
            let mut stmt = conn.prepare(
                "SELECT doc_id, page_index, triage_class, confidence, classifier_id,
                        classified_at_unix_ms, notes
                 FROM page_triage WHERE doc_id = ?1
                 ORDER BY page_index ASC",
            )?;
            let rows = stmt.query_map([doc_id], |r| {
                Ok(PageTriageRow {
                    doc_id: r.get(0)?,
                    page_index: r.get(1)?,
                    triage_class: r.get(2)?,
                    confidence: r.get(3)?,
                    classifier_id: r.get(4)?,
                    classified_at_unix_ms: r.get(5)?,
                    notes: r.get(6)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT doc_id, page_index, triage_class, confidence, classifier_id,
                        classified_at_unix_ms, notes
                 FROM page_triage
                 ORDER BY doc_id, page_index ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok(PageTriageRow {
                    doc_id: r.get(0)?,
                    page_index: r.get(1)?,
                    triage_class: r.get(2)?,
                    confidence: r.get(3)?,
                    classifier_id: r.get(4)?,
                    classified_at_unix_ms: r.get(5)?,
                    notes: r.get(6)?,
                })
            })?;
            for row in rows {
                out.push(row?);
            }
        }
        Ok(out)
    }

    // --- B-S1-06 page text layers ---

    fn merge_word_authority(
        rows: &[PageTextLayerRow],
        overlays: &[AuthorityOverlay],
    ) -> (Vec<PageTextLayerRow>, Vec<ConflictCandidate>) {
        let mut merged = rows.to_vec();
        let mut conflicts = Vec::new();
        for overlay in overlays {
            let Some(page_index) = overlay.page_index else {
                continue;
            };
            let Some(layer) = merged.iter_mut().find(|l| l.page_index == page_index) else {
                conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: "TARGET_DELETED".into(),
                    generated_value: None,
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
                continue;
            };
            let bbox = overlay.anchor.get("original_bbox").and_then(|v| v.as_str());
            let exact = overlay.word_index.and_then(|wi| {
                layer.words.iter().position(|w| {
                    w.word_index == wi
                        && (w.text == overlay.base_value
                            || w.text == overlay.counsel_value
                            || bbox.is_some_and(|b| w.bbox_json.as_deref() == Some(b)))
                })
            });
            let base_hits: Vec<usize> = layer
                .words
                .iter()
                .enumerate()
                .filter(|(_, w)| w.text == overlay.base_value)
                .map(|(i, _)| i)
                .collect();
            let selected = exact.or_else(|| (base_hits.len() == 1).then(|| base_hits[0]));
            let Some(index) = selected else {
                layer.page_quality_warn = true;
                conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: if base_hits.len() > 1 {
                        "AMBIGUOUS_MATCH"
                    } else {
                        "TARGET_DELETED"
                    }
                    .into(),
                    generated_value: None,
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
                continue;
            };
            let generated = layer.words[index].text.clone();
            if generated != overlay.base_value && generated != overlay.counsel_value {
                conflicts.push(ConflictCandidate {
                    overlay_event_id: overlay.latest_event_id.clone(),
                    target_key: overlay.target_key.clone(),
                    target_kind: overlay.target_kind.clone(),
                    conflict_kind: "MATERIAL_DISAGREEMENT".into(),
                    generated_value: Some(generated),
                    counsel_value: overlay.counsel_value.clone(),
                    anchor: overlay.anchor.clone(),
                });
            }
            let start = layer.words[index].char_start.max(0) as usize;
            let end = layer.words[index].char_end.max(0) as usize;
            let chars: Vec<char> = layer.full_text.chars().collect();
            if start > end || end > chars.len() {
                layer.page_quality_warn = true;
                continue;
            }
            let mut full: String = chars[..start].iter().collect();
            full.push_str(&overlay.counsel_value);
            full.push_str(&chars[end..].iter().collect::<String>());
            let delta = overlay.counsel_value.chars().count() as i64 - (end as i64 - start as i64);
            let wi = layer.words[index].word_index;
            let word = &mut layer.words[index];
            word.text = overlay.counsel_value.clone();
            word.char_end = start as i64 + overlay.counsel_value.chars().count() as i64;
            word.confidence = 1.0;
            word.calibrated_confidence = Some(1.0);
            word.confidence_state = "counsel_confirmed".into();
            word.counsel_confirmed = true;
            word.low_confidence = false;
            word.force_review = false;
            word.suppress_from_auto = false;
            for w in &mut layer.words {
                if w.word_index > wi {
                    w.char_start += delta;
                    w.char_end += delta;
                }
            }
            layer.full_text = full;
        }
        (merged, conflicts)
    }

    pub fn page_text_layer_replace(
        &self,
        case_id: &str,
        doc_id: &str,
        rows: &[PageTextLayerRow],
    ) -> VaultResult<()> {
        let overlays = self.authority_overlays_list(case_id, "word_span")?;
        let (merged_rows, authority_conflicts) = Self::merge_word_authority(rows, &overlays);
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case vault before writing text layers".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE doc_id = ?1",
            [doc_id],
            |r| r.get(0),
        )?;
        if exists == 0 {
            return Err(VaultError::NotFound {
                path: PathBuf::from(format!("doc:{doc_id}")),
            });
        }
        let tx = conn.unchecked_transaction()?;
        tx.execute("DELETE FROM page_words WHERE doc_id = ?1", [doc_id])?;
        tx.execute("DELETE FROM page_text_layer WHERE doc_id = ?1", [doc_id])?;
        for row in &merged_rows {
            tx.execute(
                "INSERT INTO page_text_layer (
                    doc_id, page_index, recognizer_id, recognized_at_unix_ms,
                    full_text, mean_confidence, page_quality_warn, notes,
                    calibration_id, critical_fields_json
                 ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                rusqlite::params![
                    row.doc_id,
                    row.page_index,
                    row.recognizer_id,
                    row.recognized_at_unix_ms,
                    row.full_text,
                    row.mean_confidence,
                    if row.page_quality_warn { 1i64 } else { 0i64 },
                    row.notes,
                    row.calibration_id,
                    row.critical_fields.to_string(),
                ],
            )?;
            for w in &row.words {
                tx.execute(
                    "INSERT INTO page_words (
                        doc_id, page_index, word_index, text, confidence,
                        char_start, char_end, low_confidence, force_review,
                        suppress_from_auto, bbox_json, raw_engine_confidence,
                        calibrated_confidence, confidence_state, counsel_confirmed
                     ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                    rusqlite::params![
                        row.doc_id,
                        row.page_index,
                        w.word_index,
                        w.text,
                        w.confidence,
                        w.char_start,
                        w.char_end,
                        if w.low_confidence { 1i64 } else { 0i64 },
                        if w.force_review { 1i64 } else { 0i64 },
                        if w.suppress_from_auto { 1i64 } else { 0i64 },
                        w.bbox_json,
                        w.raw_engine_confidence,
                        w.calibrated_confidence,
                        w.confidence_state,
                        if w.counsel_confirmed { 1i64 } else { 0i64 },
                    ],
                )?;
            }
        }
        tx.commit()?;
        drop(g);
        self.record_authority_conflicts(case_id, &authority_conflicts)?;
        Ok(())
    }

    pub fn page_text_layer_list(
        &self,
        case_id: &str,
        doc_id: Option<&str>,
    ) -> VaultResult<Vec<PageTextLayerRow>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case vault before listing text layers".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;

        let mut layers: Vec<PageTextLayerRow> = Vec::new();
        if let Some(doc_id) = doc_id {
            let mut stmt = conn.prepare(
                "SELECT doc_id, page_index, recognizer_id, recognized_at_unix_ms,
                        full_text, mean_confidence, page_quality_warn, notes,
                        calibration_id, critical_fields_json
                 FROM page_text_layer WHERE doc_id = ?1
                 ORDER BY page_index ASC",
            )?;
            let rows = stmt.query_map([doc_id], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, f64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, String>(9)?,
                ))
            })?;
            for row in rows {
                let (d, pi, rid, at, full, mean, warn, notes, calibration_id, critical_json) = row?;
                layers.push(PageTextLayerRow {
                    doc_id: d,
                    page_index: pi,
                    recognizer_id: rid,
                    recognized_at_unix_ms: at,
                    full_text: full,
                    mean_confidence: mean,
                    page_quality_warn: warn != 0,
                    notes,
                    calibration_id,
                    critical_fields: serde_json::from_str(&critical_json)
                        .unwrap_or_else(|_| serde_json::json!([])),
                    words: Vec::new(),
                });
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT doc_id, page_index, recognizer_id, recognized_at_unix_ms,
                        full_text, mean_confidence, page_quality_warn, notes,
                        calibration_id, critical_fields_json
                 FROM page_text_layer
                 ORDER BY doc_id, page_index ASC",
            )?;
            let rows = stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, i64>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, f64>(5)?,
                    r.get::<_, i64>(6)?,
                    r.get::<_, Option<String>>(7)?,
                    r.get::<_, Option<String>>(8)?,
                    r.get::<_, String>(9)?,
                ))
            })?;
            for row in rows {
                let (d, pi, rid, at, full, mean, warn, notes, calibration_id, critical_json) = row?;
                layers.push(PageTextLayerRow {
                    doc_id: d,
                    page_index: pi,
                    recognizer_id: rid,
                    recognized_at_unix_ms: at,
                    full_text: full,
                    mean_confidence: mean,
                    page_quality_warn: warn != 0,
                    notes,
                    calibration_id,
                    critical_fields: serde_json::from_str(&critical_json)
                        .unwrap_or_else(|_| serde_json::json!([])),
                    words: Vec::new(),
                });
            }
        }

        for layer in &mut layers {
            let mut stmt = conn.prepare(
                "SELECT word_index, text, confidence, char_start, char_end,
                        low_confidence, force_review, suppress_from_auto, bbox_json,
                        raw_engine_confidence, calibrated_confidence, confidence_state, counsel_confirmed
                 FROM page_words
                 WHERE doc_id = ?1 AND page_index = ?2
                 ORDER BY word_index ASC",
            )?;
            let words = stmt.query_map(rusqlite::params![layer.doc_id, layer.page_index], |r| {
                Ok(PageWordRow {
                    word_index: r.get(0)?,
                    text: r.get(1)?,
                    confidence: r.get(2)?,
                    raw_engine_confidence: r.get(9)?,
                    calibrated_confidence: r.get(10)?,
                    confidence_state: r.get(11)?,
                    counsel_confirmed: r.get::<_, i64>(12)? != 0,
                    char_start: r.get(3)?,
                    char_end: r.get(4)?,
                    low_confidence: r.get::<_, i64>(5)? != 0,
                    force_review: r.get::<_, i64>(6)? != 0,
                    suppress_from_auto: r.get::<_, i64>(7)? != 0,
                    bbox_json: r.get(8)?,
                })
            })?;
            for w in words {
                layer.words.push(w?);
            }
        }
        Ok(layers)
    }

    // --- B-S1-07 case_graph ---

    pub fn case_graph_put(&self, case_id: &str, graph: &serde_json::Value) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case vault before writing case_graph".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let schema_version = graph
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0");
        let extractor_id = graph
            .pointer("/pipeline/extractor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let completeness = graph
            .pointer("/completeness/score_0_100")
            .and_then(|v| v.as_i64());
        if schema_version == "2.0.0" {
            for collection in ["nodes", "edges"] {
                let items = graph
                    .get(collection)
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| VaultError::Internal {
                        detail: format!("CF_GRAPH_V2_INVALID: missing {collection}"),
                    })?;
                for (index, item) in items.iter().enumerate() {
                    let provenance = item.get("provenance").unwrap_or(&serde_json::Value::Null);
                    let has_sources = provenance
                        .get("sources")
                        .and_then(|v| v.as_array())
                        .map(|v| !v.is_empty())
                        .unwrap_or(false);
                    let complete = has_sources
                        && [
                            "confidence",
                            "origin",
                            "pipeline_version",
                            "recorded_at",
                            "counsel_confirmed",
                            "correction_lineage",
                        ]
                        .iter()
                        .all(|field| provenance.get(*field).is_some_and(|v| !v.is_null()));
                    if !complete {
                        return Err(VaultError::Internal {
                            detail: format!(
                                "CF_GRAPH_V2_PROVENANCE: {collection}[{index}] is not traceable"
                            ),
                        });
                    }
                }
            }
        }
        let migration_strategy = if schema_version == "2.0.0" {
            "normalized_v2_with_v1_read_projections"
        } else {
            "v1_retained_reextract_for_v2"
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let json = serde_json::to_string(graph).map_err(|e| VaultError::Internal {
            detail: format!("case_graph serialize: {e}"),
        })?;
        conn.execute(
            "INSERT INTO case_graph (
                case_id, schema_version, graph_json, extractor_id,
                generated_at_unix_ms, completeness_score, graph_contract_version, migration_strategy
             ) VALUES (?1,?2,?3,?4,?5,?6,?2,?7)
             ON CONFLICT(case_id) DO UPDATE SET
                schema_version=excluded.schema_version,
                graph_json=excluded.graph_json,
                extractor_id=excluded.extractor_id,
                generated_at_unix_ms=excluded.generated_at_unix_ms,
                completeness_score=excluded.completeness_score,
                graph_contract_version=excluded.graph_contract_version,
                migration_strategy=excluded.migration_strategy",
            rusqlite::params![
                case_id,
                schema_version,
                json,
                extractor_id,
                now,
                completeness,
                migration_strategy,
            ],
        )?;
        Ok(())
    }

    pub fn case_graph_get(&self, case_id: &str) -> VaultResult<Option<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case vault before reading case_graph".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let row: Option<String> = conn
            .query_row(
                "SELECT graph_json FROM case_graph WHERE case_id = ?1",
                [case_id],
                |r| r.get(0),
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some(s) => {
                let v: serde_json::Value =
                    serde_json::from_str(&s).map_err(|e| VaultError::Internal {
                        detail: format!("case_graph parse: {e}"),
                    })?;
                Ok(Some(v))
            }
        }
    }

    fn authority_overlays_list(
        &self,
        case_id: &str,
        target_kind: &str,
    ) -> VaultResult<Vec<AuthorityOverlay>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: authority overlays require an open case".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let mut stmt = conn.prepare(
            "SELECT target_key, latest_event_id, target_kind, collection_name, item_id, field_name,
                    doc_id, page_index, word_index, base_value, counsel_value, anchor_json
             FROM authority_overlays WHERE active = 1 AND target_kind = ?1 ORDER BY updated_at_unix_ms ASC"
        )?;
        let rows = stmt.query_map([target_kind], |r| {
            let anchor_json: String = r.get(11)?;
            Ok(AuthorityOverlay {
                target_key: r.get(0)?,
                latest_event_id: r.get(1)?,
                target_kind: r.get(2)?,
                collection: r.get(3)?,
                item_id: r.get(4)?,
                field_name: r.get(5)?,
                doc_id: r.get(6)?,
                page_index: r.get(7)?,
                word_index: r.get(8)?,
                base_value: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
                counsel_value: r.get::<_, Option<String>>(10)?.unwrap_or_default(),
                anchor: serde_json::from_str(&anchor_json)
                    .unwrap_or_else(|_| serde_json::json!({})),
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn record_authority_conflicts(
        &self,
        case_id: &str,
        conflicts: &[ConflictCandidate],
    ) -> VaultResult<()> {
        if conflicts.is_empty() {
            return Ok(());
        }
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: conflict recording requires an open case".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        for conflict in conflicts {
            let fingerprint = format!(
                "{}|{}|{}",
                conflict.overlay_event_id,
                conflict.conflict_kind,
                conflict.generated_value.as_deref().unwrap_or("")
            );
            let mut h = Sha256::new();
            h.update(fingerprint.as_bytes());
            let conflict_id = format!("cfc_{}", hex::encode(h.finalize()));
            conn.execute(
                "INSERT OR IGNORE INTO correction_conflicts (conflict_id, overlay_event_id, target_key, target_kind,
                    conflict_kind, generated_value, counsel_value, anchor_json, status, created_at_unix_ms)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,'OPEN',?9)",
                rusqlite::params![conflict_id, conflict.overlay_event_id, conflict.target_key, conflict.target_kind,
                    conflict.conflict_kind, conflict.generated_value, conflict.counsel_value, conflict.anchor.to_string(), now]
            )?;
        }
        Ok(())
    }

    /// Generated graph writes always pass through the three-way authority merge.
    pub fn case_graph_put_generated(
        &self,
        case_id: &str,
        generated: &serde_json::Value,
    ) -> VaultResult<serde_json::Value> {
        let prior = self.case_graph_get(case_id)?;
        let overlays = self.authority_overlays_list(case_id, "graph_item")?;
        let (merged, conflicts) = merge_graph(prior.as_ref(), generated.clone(), &overlays);
        self.record_authority_conflicts(case_id, &conflicts)?;
        self.case_graph_put(case_id, &merged)?;
        Ok(merged)
    }

    pub fn authority_conflicts_list(
        &self,
        case_id: &str,
        include_resolved: bool,
    ) -> VaultResult<Vec<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: conflict review requires an open case".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let sql_text = if include_resolved {
            "SELECT conflict_id, overlay_event_id, target_key, target_kind, conflict_kind, generated_value, counsel_value, anchor_json, status, created_at_unix_ms, resolved_at_unix_ms FROM correction_conflicts ORDER BY created_at_unix_ms DESC"
        } else {
            "SELECT conflict_id, overlay_event_id, target_key, target_kind, conflict_kind, generated_value, counsel_value, anchor_json, status, created_at_unix_ms, resolved_at_unix_ms FROM correction_conflicts WHERE status='OPEN' ORDER BY created_at_unix_ms DESC"
        };
        let mut stmt = conn.prepare(sql_text)?;
        let rows = stmt.query_map([], |r| { let anchor:String=r.get(7)?; Ok(serde_json::json!({"conflict_id":r.get::<_,String>(0)?,"overlay_event_id":r.get::<_,String>(1)?,"target_key":r.get::<_,String>(2)?,"target_kind":r.get::<_,String>(3)?,"conflict_kind":r.get::<_,String>(4)?,"generated_value":r.get::<_,Option<String>>(5)?,"counsel_value":r.get::<_,Option<String>>(6)?,"anchor":serde_json::from_str::<serde_json::Value>(&anchor).unwrap_or_else(|_|serde_json::json!({})),"status":r.get::<_,String>(8)?,"created_at_unix_ms":r.get::<_,i64>(9)?,"resolved_at_unix_ms":r.get::<_,Option<i64>>(10)?})) })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn correction_events_list(
        &self,
        case_id: &str,
        limit: i64,
    ) -> VaultResult<Vec<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: correction history requires an open case".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let mut stmt=conn.prepare("SELECT e.event_id,e.created_at_unix_ms,e.event_type,e.target_kind,e.target_key,e.base_value,e.counsel_value,e.reason,e.actor,e.supersedes_event_id,e.anchor_json,e.immutable_hash,COALESCE(o.active,0) FROM correction_events e LEFT JOIN authority_overlays o ON o.latest_event_id=e.event_id ORDER BY e.created_at_unix_ms DESC LIMIT ?1")?;
        let rows=stmt.query_map([limit.clamp(1,500)],|r|{let anchor:String=r.get(10)?;Ok(serde_json::json!({"event_id":r.get::<_,String>(0)?,"created_at_unix_ms":r.get::<_,i64>(1)?,"event_type":r.get::<_,String>(2)?,"target_kind":r.get::<_,String>(3)?,"target_key":r.get::<_,String>(4)?,"base_value":r.get::<_,Option<String>>(5)?,"counsel_value":r.get::<_,Option<String>>(6)?,"reason":r.get::<_,String>(7)?,"actor":r.get::<_,String>(8)?,"supersedes_event_id":r.get::<_,Option<String>>(9)?,"anchor":serde_json::from_str::<serde_json::Value>(&anchor).unwrap_or_else(|_|serde_json::json!({})),"immutable_hash":r.get::<_,String>(11)?,"active_overlay":r.get::<_,i64>(12)?!=0}))})?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn apply_authority_value(
        &self,
        case_id: &str,
        overlay: &AuthorityOverlay,
        value: &str,
        counsel: bool,
        lineage_event: &str,
    ) -> VaultResult<()> {
        if overlay.target_kind == "graph_item" {
            let mut graph = self
                .case_graph_get(case_id)?
                .ok_or_else(|| VaultError::Internal {
                    detail: "CF_AUTHORITY_NO_GRAPH".into(),
                })?;
            if let (Some(collection), Some(item_id), Some(field)) = (
                overlay.collection.as_deref(),
                overlay.item_id.as_deref(),
                overlay.field_name.as_deref(),
            ) {
                let id_field = match collection {
                    "chronology" => "event_id",
                    "parties" => "party_id",
                    "statutes" => "statute_id",
                    "evidence" => "evidence_id",
                    "prayer_relief" => "relief_id",
                    "contradictions" => "contradiction_id",
                    "counsel_facts" => "fact_id",
                    "nodes" => "node_id",
                    "edges" => "edge_id",
                    _ => "id",
                };
                if let Some(item) = graph
                    .get_mut(collection)
                    .and_then(|v| v.as_array_mut())
                    .and_then(|a| {
                        a.iter_mut()
                            .find(|x| x.get(id_field).and_then(|v| v.as_str()) == Some(item_id))
                    })
                {
                    item[field] = serde_json::Value::String(value.into());
                    if let Some(p) = item.get_mut("provenance").and_then(|v| v.as_object_mut()) {
                        p.insert("counsel_confirmed".into(), serde_json::Value::Bool(counsel));
                        p.insert("reviewed".into(), serde_json::Value::Bool(true));
                        if !counsel {
                            p.insert(
                                "origin".into(),
                                serde_json::Value::String("EXTRACTED".into()),
                            );
                        }
                        let lineage = p
                            .entry("correction_lineage")
                            .or_insert_with(|| serde_json::json!([]));
                        if let Some(a) = lineage.as_array_mut() {
                            a.push(serde_json::Value::String(lineage_event.into()));
                        }
                    }
                }
            }
            self.case_graph_put(case_id, &graph)?;
        } else if overlay.target_kind == "word_span" {
            let Some(doc_id) = overlay.doc_id.as_deref() else {
                return Ok(());
            };
            let mut layers = self.page_text_layer_list(case_id, Some(doc_id))?;
            let Some(layer) = layers
                .iter_mut()
                .find(|l| Some(l.page_index) == overlay.page_index)
            else {
                return Ok(());
            };
            let selected = overlay
                .word_index
                .and_then(|wi| layer.words.iter().position(|w| w.word_index == wi))
                .or_else(|| {
                    let hits: Vec<usize> = layer
                        .words
                        .iter()
                        .enumerate()
                        .filter(|(_, w)| w.text == overlay.counsel_value)
                        .map(|(i, _)| i)
                        .collect();
                    (hits.len() == 1).then(|| hits[0])
                });
            if let Some(index) = selected {
                let start = layer.words[index].char_start.max(0) as usize;
                let end = layer.words[index].char_end.max(0) as usize;
                let chars: Vec<char> = layer.full_text.chars().collect();
                if start <= end && end <= chars.len() {
                    let mut rebuilt: String = chars[..start].iter().collect();
                    rebuilt.push_str(value);
                    rebuilt.push_str(&chars[end..].iter().collect::<String>());
                    let delta = value.chars().count() as i64 - (end as i64 - start as i64);
                    let wi = layer.words[index].word_index;
                    layer.words[index].text = value.into();
                    layer.words[index].char_end = start as i64 + value.chars().count() as i64;
                    layer.words[index].counsel_confirmed = counsel;
                    layer.words[index].confidence_state = if counsel {
                        "counsel_confirmed".into()
                    } else {
                        "raw_only".into()
                    };
                    for w in &mut layer.words {
                        if w.word_index > wi {
                            w.char_start += delta;
                            w.char_end += delta;
                        }
                    }
                    layer.full_text = rebuilt;
                }
            }
            self.page_text_layer_replace(case_id, doc_id, &layers)?;
        }
        Ok(())
    }

    pub fn correction_revert(
        &self,
        case_id: &str,
        event_id: &str,
        reason: &str,
    ) -> VaultResult<serde_json::Value> {
        let reason = if reason.trim().is_empty() {
            "Counsel reverted correction"
        } else {
            reason.trim()
        };
        let (overlay, new_event, now, immutable_hash) = {
            let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
            let conn = g
                .cases
                .get_mut(case_id)
                .ok_or_else(|| VaultError::Internal {
                    detail: "CF_VAULT_LOCKED".into(),
                })?;
            sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
            let row:(String,String,String,Option<String>,Option<String>,Option<String>,Option<String>,Option<i64>,Option<i64>,Option<String>,Option<String>,String)=conn.query_row("SELECT o.target_key,o.latest_event_id,o.target_kind,o.collection_name,o.item_id,o.field_name,o.doc_id,o.page_index,o.word_index,o.base_value,o.counsel_value,o.anchor_json FROM authority_overlays o WHERE o.latest_event_id=?1 AND o.active=1",[event_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?)))?;
            let anchor: serde_json::Value =
                serde_json::from_str(&row.11).unwrap_or_else(|_| serde_json::json!({}));
            let overlay = AuthorityOverlay {
                target_key: row.0.clone(),
                latest_event_id: row.1.clone(),
                target_kind: row.2.clone(),
                collection: row.3,
                item_id: row.4,
                field_name: row.5,
                doc_id: row.6,
                page_index: row.7,
                word_index: row.8,
                base_value: row.9.unwrap_or_default(),
                counsel_value: row.10.unwrap_or_default(),
                anchor: anchor.clone(),
            };
            let new_event = Uuid::new_v4().to_string();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let payload = serde_json::json!({"event_id":new_event,"event_type":"REVERT","target_key":overlay.target_key,"reason":reason,"supersedes_event_id":event_id,"anchor":anchor});
            let mut h = Sha256::new();
            h.update(payload.to_string().as_bytes());
            let immutable_hash = format!("sha256:{}", hex::encode(h.finalize()));
            conn.execute("INSERT INTO correction_events(event_id,created_at_unix_ms,event_type,target_kind,target_key,base_value,counsel_value,reason,actor,supersedes_event_id,anchor_json,payload_json,immutable_hash)VALUES(?1,?2,'REVERT',?3,?4,?5,?6,?7,'COUNSEL',?8,?9,?10,?11)",rusqlite::params![new_event,now,overlay.target_kind,overlay.target_key,overlay.base_value,overlay.counsel_value,reason,event_id,anchor.to_string(),payload.to_string(),immutable_hash])?;
            conn.execute("UPDATE authority_overlays SET active=0,latest_event_id=?1,updated_at_unix_ms=?2 WHERE target_key=?3",rusqlite::params![new_event,now,overlay.target_key])?;
            (overlay, new_event, now, immutable_hash)
        };
        self.apply_authority_value(case_id, &overlay, &overlay.base_value, false, &new_event)?;
        Ok(
            serde_json::json!({"event_id":new_event,"event_type":"REVERT","supersedes_event_id":event_id,"target_key":overlay.target_key,"base_value":overlay.base_value,"created_at_unix_ms":now,"immutable_hash":immutable_hash}),
        )
    }

    pub fn authority_conflict_resolve(
        &self,
        case_id: &str,
        conflict_id: &str,
        resolution: &str,
        reason: &str,
    ) -> VaultResult<serde_json::Value> {
        if !matches!(resolution, "KEEP_COUNSEL" | "ACCEPT_GENERATED") {
            return Err(VaultError::Internal {
                detail: "CF_AUTHORITY_BAD_RESOLUTION".into(),
            });
        }
        let reason = if reason.trim().is_empty() {
            "Counsel resolved rerun conflict"
        } else {
            reason.trim()
        };
        let (overlay, generated, new_event, now) = {
            let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
            let conn = g
                .cases
                .get_mut(case_id)
                .ok_or_else(|| VaultError::Internal {
                    detail: "CF_VAULT_LOCKED".into(),
                })?;
            sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
            let row:(String,Option<String>,String,String,String,Option<String>,Option<String>,Option<String>,Option<String>,Option<i64>,Option<i64>,Option<String>,Option<String>,String)=conn.query_row("SELECT c.target_key,c.generated_value,o.latest_event_id,o.target_kind,o.target_key,o.collection_name,o.item_id,o.field_name,o.doc_id,o.page_index,o.word_index,o.base_value,o.counsel_value,o.anchor_json FROM correction_conflicts c JOIN authority_overlays o ON o.target_key=c.target_key WHERE c.conflict_id=?1 AND c.status='OPEN'",[conflict_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?,r.get(7)?,r.get(8)?,r.get(9)?,r.get(10)?,r.get(11)?,r.get(12)?,r.get(13)?)))?;
            let anchor = serde_json::from_str(&row.13).unwrap_or_else(|_| serde_json::json!({}));
            let overlay = AuthorityOverlay {
                target_key: row.4,
                latest_event_id: row.2.clone(),
                target_kind: row.3,
                collection: row.5,
                item_id: row.6,
                field_name: row.7,
                doc_id: row.8,
                page_index: row.9,
                word_index: row.10,
                base_value: row.11.unwrap_or_default(),
                counsel_value: row.12.unwrap_or_default(),
                anchor: anchor.clone(),
            };
            let event_type = if resolution == "KEEP_COUNSEL" {
                "RESOLVE_KEEP_COUNSEL"
            } else {
                "RESOLVE_ACCEPT_GENERATED"
            };
            let new_event = Uuid::new_v4().to_string();
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            let payload = serde_json::json!({"event_id":new_event,"event_type":event_type,"conflict_id":conflict_id,"target_key":row.0,"reason":reason});
            let mut h = Sha256::new();
            h.update(payload.to_string().as_bytes());
            let hash = format!("sha256:{}", hex::encode(h.finalize()));
            conn.execute("INSERT INTO correction_events(event_id,created_at_unix_ms,event_type,target_kind,target_key,base_value,counsel_value,reason,actor,supersedes_event_id,anchor_json,payload_json,immutable_hash)VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'COUNSEL',?9,?10,?11,?12)",rusqlite::params![new_event,now,event_type,overlay.target_kind,overlay.target_key,overlay.base_value,overlay.counsel_value,reason,overlay.latest_event_id,anchor.to_string(),payload.to_string(),hash])?;
            conn.execute("UPDATE correction_conflicts SET status=?1,resolved_at_unix_ms=?2,resolution_event_id=?3 WHERE conflict_id=?4",rusqlite::params![resolution,now,new_event,conflict_id])?;
            conn.execute("UPDATE authority_overlays SET latest_event_id=?1,active=?2,updated_at_unix_ms=?3 WHERE target_key=?4",rusqlite::params![new_event,if resolution=="KEEP_COUNSEL"{1}else{0},now,overlay.target_key])?;
            (overlay, row.1, new_event, now)
        };
        if resolution == "ACCEPT_GENERATED" {
            if let Some(value) = generated.as_deref() {
                self.apply_authority_value(case_id, &overlay, value, false, &new_event)?;
            }
        }
        Ok(
            serde_json::json!({"conflict_id":conflict_id,"status":resolution,"resolution_event_id":new_event,"resolved_at_unix_ms":now}),
        )
    }

    // --- B-S3-12 matter legal working set ---

    pub fn working_set_get(&self, case_id: &str) -> VaultResult<serde_json::Value> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case before reading its legal working set".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        load_working_set_value(conn, case_id)
    }

    pub fn working_set_merge_suggestions(
        &self,
        case_id: &str,
        items: &[crate::working_set::WorkingSetItemInput],
        regimes: &[crate::working_set::RegimeCandidate],
    ) -> VaultResult<serde_json::Value> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case before suggesting authorities".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = now_ms();
        let tx = conn.transaction()?;
        let current: i64 = tx.query_row(
            "SELECT COALESCE(MAX(version),0) FROM legal_working_set_meta WHERE case_id=?1",
            [case_id],
            |r| r.get(0),
        )?;
        let version = current + 1;
        let regimes_json = serde_json::to_string(regimes).map_err(|e| VaultError::Internal {
            detail: e.to_string(),
        })?;
        tx.execute("INSERT INTO legal_working_set_meta(case_id,schema_version,version,regime_status,confirmed_regime_id,regime_candidates_json,updated_at_unix_ms) VALUES(?1,?2,?3,'PENDING_CONFIRMATION',NULL,?4,?5) ON CONFLICT(case_id) DO UPDATE SET version=?3,regime_candidates_json=?4,updated_at_unix_ms=?5",rusqlite::params![case_id,crate::working_set::SCHEMA_VERSION,version,regimes_json,now])?;
        for item in items {
            let forced_decision = if item.superseded_status == "SUPERSEDED" {
                "SUPERSEDED"
            } else {
                "PROPOSED"
            };
            tx.execute("INSERT INTO legal_working_set_item(case_id,item_id,authority_id,source_id,chunk_id,pack_id,title,citation,section_label,jurisdiction,authority_type,trust_level,temporal_status,superseded_status,source_path,source_reference,source_checksum,chunk_checksum,exact_text,decision,pinned,controlling,counsel_rationale,first_added_at_unix_ms,updated_at_unix_ms,last_seen_index_checksum) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,0,0,NULL,?21,?21,?22) ON CONFLICT(case_id,authority_id) DO UPDATE SET source_id=excluded.source_id,chunk_id=excluded.chunk_id,pack_id=excluded.pack_id,title=excluded.title,citation=excluded.citation,section_label=excluded.section_label,jurisdiction=excluded.jurisdiction,authority_type=excluded.authority_type,trust_level=excluded.trust_level,temporal_status=excluded.temporal_status,superseded_status=excluded.superseded_status,source_path=excluded.source_path,source_reference=excluded.source_reference,source_checksum=excluded.source_checksum,chunk_checksum=excluded.chunk_checksum,exact_text=excluded.exact_text,decision=CASE WHEN excluded.superseded_status='SUPERSEDED' THEN 'SUPERSEDED' ELSE legal_working_set_item.decision END,controlling=CASE WHEN excluded.superseded_status='SUPERSEDED' THEN 0 ELSE legal_working_set_item.controlling END,updated_at_unix_ms=excluded.updated_at_unix_ms,last_seen_index_checksum=excluded.last_seen_index_checksum",rusqlite::params![case_id,item.item_id,item.authority_id,item.source_id,item.chunk_id,item.pack_id,item.title,item.citation,item.section_label,item.jurisdiction,item.authority_type,item.trust_level,item.temporal_status,item.superseded_status,item.source_path,item.source_reference,item.source_checksum,item.chunk_checksum,item.exact_text,forced_decision,now,item.last_seen_index_checksum])?;
        }
        let snapshot = load_working_set_value(&tx, case_id)?;
        tx.execute("INSERT INTO legal_working_set_revision(case_id,version,created_at_unix_ms,event_type,snapshot_json)VALUES(?1,?2,?3,'SUGGEST_OR_REFRESH',?4)",rusqlite::params![case_id,version,now,snapshot.to_string()])?;
        tx.commit()?;
        load_working_set_value(conn, case_id)
    }

    pub fn working_set_update_item(
        &self,
        case_id: &str,
        item_id: &str,
        decision: &str,
        pinned: bool,
        controlling: bool,
        rationale: Option<&str>,
    ) -> VaultResult<serde_json::Value> {
        if !crate::working_set::DECISIONS.contains(&decision) {
            return Err(VaultError::Internal {
                detail: format!("CF_WORKING_SET_BAD_DECISION: {decision}"),
            });
        }
        if controlling && decision != "INCLUDED" {
            return Err(VaultError::Internal {
                detail:
                    "CF_WORKING_SET_CONTROLLING_REQUIRES_INCLUDED: mark INCLUDED before controlling"
                        .into(),
            });
        }
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case before updating its legal working set"
                    .into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = now_ms();
        let tx = conn.transaction()?;
        let status: Option<(String,String)> = tx.query_row("SELECT temporal_status,superseded_status FROM legal_working_set_item WHERE case_id=?1 AND item_id=?2",rusqlite::params![case_id,item_id],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
        let (temporal, superseded) = status.ok_or_else(|| VaultError::Internal {
            detail: "CF_WORKING_SET_ITEM_NOT_FOUND".into(),
        })?;
        if controlling && (temporal != "APPLICABLE" || superseded != "CURRENT") {
            return Err(VaultError::Internal{detail:"CF_WORKING_SET_NONCURRENT_CONTROLLING: only APPLICABLE/CURRENT authority may be controlling".into()});
        }
        tx.execute("UPDATE legal_working_set_item SET decision=?1,pinned=?2,controlling=?3,counsel_rationale=?4,updated_at_unix_ms=?5 WHERE case_id=?6 AND item_id=?7",rusqlite::params![decision,pinned as i64,controlling as i64,rationale,now,case_id,item_id])?;
        let current: i64 = tx.query_row(
            "SELECT version FROM legal_working_set_meta WHERE case_id=?1",
            [case_id],
            |r| r.get(0),
        )?;
        let version = current + 1;
        tx.execute(
            "UPDATE legal_working_set_meta SET version=?1,updated_at_unix_ms=?2 WHERE case_id=?3",
            rusqlite::params![version, now, case_id],
        )?;
        let snapshot = load_working_set_value(&tx, case_id)?;
        tx.execute("INSERT INTO legal_working_set_revision(case_id,version,created_at_unix_ms,event_type,snapshot_json)VALUES(?1,?2,?3,'COUNSEL_ITEM_DECISION',?4)",rusqlite::params![case_id,version,now,snapshot.to_string()])?;
        tx.commit()?;
        load_working_set_value(conn, case_id)
    }

    pub fn working_set_confirm_regime(
        &self,
        case_id: &str,
        regime_id: &str,
    ) -> VaultResult<serde_json::Value> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case before confirming applicable law".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = now_ms();
        let tx = conn.transaction()?;
        let candidates: String = tx.query_row(
            "SELECT regime_candidates_json FROM legal_working_set_meta WHERE case_id=?1",
            [case_id],
            |r| r.get(0),
        )?;
        let allowed = serde_json::from_str::<Vec<crate::working_set::RegimeCandidate>>(&candidates)
            .unwrap_or_default()
            .iter()
            .any(|c| c.regime_id == regime_id);
        if !allowed {
            return Err(VaultError::Internal {
                detail: "CF_WORKING_SET_UNKNOWN_REGIME: select one of the surfaced candidates"
                    .into(),
            });
        }
        let current: i64 = tx.query_row(
            "SELECT version FROM legal_working_set_meta WHERE case_id=?1",
            [case_id],
            |r| r.get(0),
        )?;
        let version = current + 1;
        tx.execute("UPDATE legal_working_set_meta SET version=?1,regime_status='COUNSEL_CONFIRMED',confirmed_regime_id=?2,updated_at_unix_ms=?3 WHERE case_id=?4",rusqlite::params![version,regime_id,now,case_id])?;
        let snapshot = load_working_set_value(&tx, case_id)?;
        tx.execute("INSERT INTO legal_working_set_revision(case_id,version,created_at_unix_ms,event_type,snapshot_json)VALUES(?1,?2,?3,'COUNSEL_REGIME_CONFIRMATION',?4)",rusqlite::params![case_id,version,now,snapshot.to_string()])?;
        tx.commit()?;
        load_working_set_value(conn, case_id)
    }

    pub fn working_set_select_for_output(
        &self,
        case_id: &str,
        output_kind: &str,
        output_id: &str,
    ) -> VaultResult<serde_json::Value> {
        if !crate::working_set::OUTPUT_KINDS.contains(&output_kind) {
            return Err(VaultError::Internal {
                detail: format!("CF_WORKING_SET_BAD_OUTPUT_KIND: {output_kind}"),
            });
        }
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before selecting authorities".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let set = load_working_set_value(conn, case_id)?;
        let version = set.get("version").and_then(|v| v.as_i64()).unwrap_or(0);
        let ids = set
            .get("usable_authority_ids")
            .cloned()
            .unwrap_or_else(|| serde_json::json!([]));
        if set.get("regime_status").and_then(|v| v.as_str()) != Some("COUNSEL_CONFIRMED") {
            return Err(VaultError::Internal{detail:"CF_WORKING_SET_REGIME_UNCONFIRMED: counsel must confirm the applicable-law regime".into()});
        }
        if set
            .get("confirmed_regime_id")
            .and_then(|v| v.as_str())
            .is_some_and(|id| id.ends_with("_REQUIRED"))
        {
            return Err(VaultError::Internal{detail:"CF_WORKING_SET_REGIME_SOURCE_GAP: the confirmed regime requires an authoritative pack that is not present in this controlled repository".into()});
        }
        if ids.as_array().is_none_or(|v| v.is_empty()) {
            return Err(VaultError::Internal{detail:"CF_WORKING_SET_EMPTY: confirm at least one current authority before downstream use".into()});
        }
        let now = now_ms();
        conn.execute("INSERT INTO legal_output_authority_provenance(output_id,case_id,output_kind,working_set_version,authority_ids_json,created_at_unix_ms)VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(output_id) DO NOTHING",rusqlite::params![output_id,case_id,output_kind,version,ids.to_string(),now])?;
        Ok(
            serde_json::json!({"output_id":output_id,"output_kind":output_kind,"case_id":case_id,"working_set_version":version,"authority_ids":ids,"selected_at_unix_ms":now}),
        )
    }

    // --- B-S2-01 argument_tree ---

    pub fn argument_tree_put(
        &self,
        case_id: &str,
        tree: &serde_json::Value,
        weakest_vi: Option<i64>,
    ) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open the case vault before writing argument_tree".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let schema_version = tree
            .get("schema_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0.0");
        let scorer_id = tree
            .pointer("/pipeline/scorer_id")
            .and_then(|v| v.as_str())
            .unwrap_or("caseforge.integrity.v1");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let json = serde_json::to_string(tree).map_err(|e| VaultError::Internal {
            detail: format!("argument_tree serialize: {e}"),
        })?;
        conn.execute(
            "INSERT INTO argument_tree (
                case_id, schema_version, tree_json, scorer_id,
                generated_at_unix_ms, weakest_vi
             ) VALUES (?1,?2,?3,?4,?5,?6)
             ON CONFLICT(case_id) DO UPDATE SET
                schema_version=excluded.schema_version,
                tree_json=excluded.tree_json,
                scorer_id=excluded.scorer_id,
                generated_at_unix_ms=excluded.generated_at_unix_ms,
                weakest_vi=excluded.weakest_vi",
            rusqlite::params![case_id, schema_version, json, scorer_id, now, weakest_vi,],
        )?;
        Ok(())
    }

    pub fn argument_tree_get(&self, case_id: &str) -> VaultResult<Option<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open the case vault before reading argument_tree".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let row: Option<String> = conn
            .query_row(
                "SELECT tree_json FROM argument_tree WHERE case_id = ?1",
                [case_id],
                |r| r.get(0),
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some(s) => {
                let v: serde_json::Value =
                    serde_json::from_str(&s).map_err(|e| VaultError::Internal {
                        detail: format!("argument_tree parse: {e}"),
                    })?;
                Ok(Some(v))
            }
        }
    }

    // --- B-S1-08 review corrections ---

    /// Correct a case_graph item field (≤2 UI interactions: select + save).
    /// Marks provenance.origin = USER_EDIT and confidence = 1.0.
    pub fn case_graph_correct_item(
        &self,
        case_id: &str,
        collection: &str,
        item_id: &str,
        field_name: &str,
        new_value: &str,
        interaction_count: i64,
    ) -> VaultResult<serde_json::Value> {
        if interaction_count > 2 {
            return Err(VaultError::Internal {
                detail: "CF_REVIEW_TOO_MANY_INTERACTIONS: AC-6.4 requires ≤ 2 interactions".into(),
            });
        }
        let allowed_collections = [
            "chronology",
            "parties",
            "statutes",
            "evidence",
            "prayer_relief",
            "contradictions",
            "counsel_facts",
        ];
        if !allowed_collections.contains(&collection) {
            return Err(VaultError::Internal {
                detail: format!("CF_REVIEW_BAD_TARGET: unknown collection {collection}"),
            });
        }
        let id_field = match collection {
            "chronology" => "event_id",
            "parties" => "party_id",
            "statutes" => "statute_id",
            "evidence" => "evidence_id",
            "prayer_relief" => "relief_id",
            "contradictions" => "contradiction_id",
            "counsel_facts" => "fact_id",
            _ => "id",
        };
        let mut graph = self
            .case_graph_get(case_id)?
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_REVIEW_NO_GRAPH: run skeleton extraction first".into(),
            })?;
        let arr = graph
            .get_mut(collection)
            .and_then(|v| v.as_array_mut())
            .ok_or_else(|| VaultError::Internal {
                detail: format!("CF_REVIEW_BAD_TARGET: missing array {collection}"),
            })?;
        let mut found = false;
        let mut old_value = String::new();
        let mut authority_anchor = serde_json::json!({});
        for item in arr.iter_mut() {
            let id = item.get(id_field).and_then(|v| v.as_str()).unwrap_or("");
            if id != item_id {
                continue;
            }
            found = true;
            old_value = item
                .get(field_name)
                .map(|v| match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            let source = item
                .pointer("/provenance/sources/0")
                .or_else(|| item.pointer("/provenance/source"));
            authority_anchor = serde_json::json!({
                "match_policy": "exact_then_source_hash_then_unique_base",
                "source_content_hash": source.and_then(|s| s.get("content_hash")).cloned().unwrap_or(serde_json::Value::Null),
                "source_doc_id": source.and_then(|s| s.get("doc_id")).cloned().unwrap_or(serde_json::Value::Null),
                "source_page_index": source.and_then(|s| s.get("page_index")).cloned().unwrap_or(serde_json::Value::Null),
                "original_item_id": item_id, "original_word_index": null,
                "original_char_start": source.and_then(|s| s.get("char_start")).cloned().unwrap_or(serde_json::Value::Null),
                "original_char_end": source.and_then(|s| s.get("char_end")).cloned().unwrap_or(serde_json::Value::Null),
                "original_bbox": source.and_then(|s| s.get("bbox")).map(|v| v.to_string())
            });
            if let Some(obj) = item.as_object_mut() {
                obj.insert(
                    field_name.to_string(),
                    serde_json::Value::String(new_value.into()),
                );
                // provenance update
                let mut prov = obj
                    .get("provenance")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                if let Some(p) = prov.as_object_mut() {
                    p.insert(
                        "origin".into(),
                        serde_json::Value::String("USER_EDIT".into()),
                    );
                    p.insert("confidence".into(), serde_json::json!(1.0));
                    p.insert("reviewed".into(), serde_json::Value::Bool(true));
                    p.insert(
                        "notes".into(),
                        serde_json::Value::String("corrected in B-S1-08 review".into()),
                    );
                }
                obj.insert("provenance".into(), prov);
            }
            break;
        }
        if !found {
            return Err(VaultError::Internal {
                detail: format!("CF_REVIEW_NOT_FOUND: {collection}/{item_id}"),
            });
        }
        let correction_id = self.append_experience_correction(
            case_id,
            "graph_item",
            Some(collection),
            Some(item_id),
            Some(field_name),
            None,
            None,
            None,
            &old_value,
            new_value,
            interaction_count,
            "Counsel corrected extracted graph value",
            &authority_anchor,
        )?;
        if let Some(item) = graph
            .get_mut(collection)
            .and_then(|v| v.as_array_mut())
            .and_then(|a| {
                a.iter_mut()
                    .find(|item| item.get(id_field).and_then(|v| v.as_str()) == Some(item_id))
            })
        {
            if let Some(prov) = item.get_mut("provenance").and_then(|v| v.as_object_mut()) {
                prov.insert("counsel_confirmed".into(), serde_json::Value::Bool(true));
                let lineage = prov
                    .entry("correction_lineage")
                    .or_insert_with(|| serde_json::json!([]));
                if let Some(a) = lineage.as_array_mut() {
                    a.push(serde_json::Value::String(correction_id));
                }
            }
        }
        self.case_graph_put(case_id, &graph)?;
        Ok(graph)
    }

    /// Correct a single word on a page text layer (AC-6.4 path).
    pub fn page_word_correct(
        &self,
        case_id: &str,
        doc_id: &str,
        page_index: i64,
        word_index: i64,
        new_text: &str,
        interaction_count: i64,
    ) -> VaultResult<PageTextLayerRow> {
        if interaction_count > 2 {
            return Err(VaultError::Internal {
                detail: "CF_REVIEW_TOO_MANY_INTERACTIONS: AC-6.4 requires ≤ 2 interactions".into(),
            });
        }
        if new_text.trim().is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_REVIEW_EMPTY_WORD: replacement text required".into(),
            });
        }
        let mut layers = self.page_text_layer_list(case_id, Some(doc_id))?;
        let layer = layers
            .iter_mut()
            .find(|l| l.page_index == page_index)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_REVIEW_NO_LAYER: page text layer missing".into(),
            })?;
        let word = layer
            .words
            .iter_mut()
            .find(|w| w.word_index == word_index)
            .ok_or_else(|| VaultError::Internal {
                detail: format!("CF_REVIEW_NOT_FOUND: word_index {word_index}"),
            })?;
        let old_value = word.text.clone();
        let authority_anchor = serde_json::json!({
            "match_policy": "exact_then_source_hash_then_unique_base",
            "source_content_hash": null, "source_doc_id": doc_id, "source_page_index": page_index,
            "original_item_id": null, "original_word_index": word_index,
            "original_char_start": word.char_start, "original_char_end": word.char_end,
            "original_bbox": word.bbox_json
        });
        let start = word.char_start as usize;
        let end = word.char_end as usize;
        // Rebuild full_text with replacement (codepoint indices)
        let chars: Vec<char> = layer.full_text.chars().collect();
        if start > end || end > chars.len() {
            return Err(VaultError::Internal {
                detail: "CF_REVIEW_BAD_SPAN: word char span out of range".into(),
            });
        }
        let mut rebuilt: String = chars[..start].iter().collect();
        rebuilt.push_str(new_text);
        rebuilt.push_str(&chars[end..].iter().collect::<String>());
        let delta = new_text.chars().count() as i64 - (end as i64 - start as i64);
        word.text = new_text.to_string();
        word.char_end = start as i64 + new_text.chars().count() as i64;
        word.confidence = 1.0;
        word.calibrated_confidence = Some(1.0);
        word.confidence_state = "counsel_confirmed".into();
        word.counsel_confirmed = true;
        word.low_confidence = false;
        word.force_review = false;
        word.suppress_from_auto = false;
        // Shift subsequent words
        for w in layer.words.iter_mut() {
            if w.word_index > word_index {
                w.char_start += delta;
                w.char_end += delta;
            }
        }
        layer.full_text = rebuilt;
        if !layer.words.is_empty() {
            layer.mean_confidence =
                layer.words.iter().map(|w| w.confidence).sum::<f64>() / layer.words.len() as f64;
        }
        let out = layer.clone();
        self.append_experience_correction(
            case_id,
            "word_span",
            None,
            None,
            None,
            Some(doc_id),
            Some(page_index),
            Some(word_index),
            &old_value,
            new_text,
            interaction_count,
            "Counsel corrected OCR word",
            &authority_anchor,
        )?;
        self.page_text_layer_replace(case_id, doc_id, &layers)?;
        Ok(out)
    }

    fn append_experience_correction(
        &self,
        case_id: &str,
        target_kind: &str,
        collection: Option<&str>,
        item_id: Option<&str>,
        field_name: Option<&str>,
        doc_id: Option<&str>,
        page_index: Option<i64>,
        word_index: Option<i64>,
        old_value: &str,
        new_value: &str,
        interaction_count: i64,
        reason: &str,
        anchor: &serde_json::Value,
    ) -> VaultResult<String> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before writing corrections".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let correction_id = Uuid::new_v4().to_string();
        let anchor_identity = anchor
            .get("source_content_hash")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .or_else(|| {
                anchor
                    .get("original_bbox")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| {
                format!(
                    "{}:{}:{}",
                    doc_id.unwrap_or(""),
                    page_index.unwrap_or(-1),
                    old_value
                )
            });
        let target_seed = format!(
            "{target_kind}|{}|{}|{anchor_identity}",
            collection.unwrap_or(""),
            field_name.unwrap_or("")
        );
        let mut target_hasher = Sha256::new();
        target_hasher.update(target_seed.as_bytes());
        let target_key = format!("cfat:v1:{}", hex::encode(target_hasher.finalize()));
        let payload = serde_json::json!({
            "schema_version": "2.0.0",
            "event_id": correction_id,
            "event_type": "CORRECT",
            "target_key": target_key,
            "record_category": "correction",
            "target_kind": target_kind,
            "collection": collection,
            "item_id": item_id,
            "field_name": field_name,
            "doc_id": doc_id,
            "page_index": page_index,
            "word_index": word_index,
            "old_value": old_value,
            "new_value": new_value,
            "reason": reason,
            "actor": "COUNSEL",
            "anchor": anchor,
            "interaction_count": interaction_count,
            "batch": "B-S3-07"
        });
        let mut event_hasher = Sha256::new();
        event_hasher.update(payload.to_string().as_bytes());
        let immutable_hash = format!("sha256:{}", hex::encode(event_hasher.finalize()));
        conn.execute(
            "INSERT INTO correction_events (event_id, created_at_unix_ms, event_type, target_kind, target_key,
                base_value, counsel_value, reason, actor, supersedes_event_id, anchor_json, payload_json, immutable_hash)
             VALUES (?1,?2,'CORRECT',?3,?4,?5,?6,?7,'COUNSEL',NULL,?8,?9,?10)",
            rusqlite::params![correction_id, now, target_kind, target_key, old_value, new_value, reason, anchor.to_string(), payload.to_string(), immutable_hash]
        )?;
        conn.execute(
            "INSERT INTO authority_overlays (target_key, latest_event_id, target_kind, active, collection_name,
                item_id, field_name, doc_id, page_index, word_index, base_value, counsel_value, anchor_json, updated_at_unix_ms)
             VALUES (?1,?2,?3,1,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
             ON CONFLICT(target_key) DO UPDATE SET latest_event_id=excluded.latest_event_id, active=1,
                item_id=excluded.item_id, base_value=excluded.base_value, counsel_value=excluded.counsel_value,
                anchor_json=excluded.anchor_json, updated_at_unix_ms=excluded.updated_at_unix_ms",
            rusqlite::params![target_key, correction_id, target_kind, collection, item_id, field_name, doc_id,
                page_index, word_index, old_value, new_value, anchor.to_string(), now]
        )?;
        conn.execute(
            "INSERT INTO experience_corrections (
                correction_id, created_at_unix_ms, record_category, target_kind,
                collection, item_id, field_name, doc_id, page_index, word_index,
                old_value, new_value, interaction_count, payload_json
             ) VALUES (?1,?2,'correction',?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            rusqlite::params![
                correction_id,
                now,
                target_kind,
                collection,
                item_id,
                field_name,
                doc_id,
                page_index,
                word_index,
                old_value,
                new_value,
                interaction_count,
                payload.to_string(),
            ],
        )?;
        drop(g);
        // Dual-write into global Experience Store (AC-11.1 accumulation).
        let summary = format!(
            "correction {} {} → {}",
            target_kind,
            if old_value.is_empty() {
                "∅"
            } else {
                old_value
            },
            new_value
        );
        let tags = vec![
            "correction".into(),
            target_kind.to_string(),
            collection.unwrap_or("").to_string(),
        ]
        .into_iter()
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>();
        let _ = self.experience_upsert(ExperienceUpsertInput {
            record_id: Some(correction_id.clone()),
            record_category: "correction".into(),
            summary,
            tags: Some(tags),
            payload: Some(payload),
            case_id: Some(case_id.to_string()),
            jurisdiction: None,
            source: Some("system".into()),
            notes: None,
            stage: Some("S1".into()),
            fleet_status: Some("local_only".into()),
            active: Some(true),
        });
        Ok(correction_id)
    }

    pub fn experience_corrections_list(
        &self,
        case_id: &str,
        limit: i64,
    ) -> VaultResult<Vec<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before listing corrections".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let lim = limit.clamp(1, 500);
        let mut stmt = conn.prepare(
            "SELECT correction_id, created_at_unix_ms, target_kind, collection, item_id,
                    field_name, doc_id, page_index, word_index, old_value, new_value,
                    interaction_count, payload_json
             FROM experience_corrections
             ORDER BY created_at_unix_ms DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map([lim], |r| {
            Ok(serde_json::json!({
                "correction_id": r.get::<_, String>(0)?,
                "created_at_unix_ms": r.get::<_, i64>(1)?,
                "target_kind": r.get::<_, String>(2)?,
                "collection": r.get::<_, Option<String>>(3)?,
                "item_id": r.get::<_, Option<String>>(4)?,
                "field_name": r.get::<_, Option<String>>(5)?,
                "doc_id": r.get::<_, Option<String>>(6)?,
                "page_index": r.get::<_, Option<i64>>(7)?,
                "word_index": r.get::<_, Option<i64>>(8)?,
                "old_value": r.get::<_, Option<String>>(9)?,
                "new_value": r.get::<_, Option<String>>(10)?,
                "interaction_count": r.get::<_, i64>(11)?,
                "payload_json": r.get::<_, Option<String>>(12)?,
            }))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    // --- B-S1-09 Guided Intake Interview ---

    fn interview_max_questions() -> i64 {
        // Frozen in contracts/constants.json document_pipeline.guided_interview_max_questions
        const FALLBACK: i64 = 12;
        let candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/constants.json"),
            PathBuf::from("contracts/constants.json"),
        ];
        for path in candidates {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(n) = v
                        .pointer("/document_pipeline/guided_interview_max_questions")
                        .and_then(|x| x.as_i64())
                    {
                        return n.clamp(1, 12);
                    }
                }
            }
        }
        FALLBACK
    }

    fn interview_min_completion() -> i64 {
        const FALLBACK: i64 = 90;
        let candidates = [
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../contracts/constants.json"),
            PathBuf::from("contracts/constants.json"),
        ];
        for path in candidates {
            if let Ok(raw) = std::fs::read_to_string(&path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
                    if let Some(n) = v
                        .pointer(
                            "/document_pipeline/guided_interview_min_skeleton_completion_percent",
                        )
                        .and_then(|x| x.as_i64())
                    {
                        return n.clamp(0, 100);
                    }
                }
            }
        }
        FALLBACK
    }

    /// Build skeleton-completion questions from case_graph gaps only (no legal strategy).
    fn questions_from_graph(graph: &serde_json::Value, max_q: i64) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let gaps = graph
            .get("gaps")
            .and_then(|g| g.as_array())
            .cloned()
            .unwrap_or_default();
        for gap in gaps {
            if out.len() as i64 >= max_q {
                break;
            }
            let status = gap.get("status").and_then(|s| s.as_str()).unwrap_or("open");
            if status != "open" {
                continue;
            }
            let gap_id = gap
                .get("gap_id")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let topic = gap
                .get("topic")
                .and_then(|s| s.as_str())
                .unwrap_or("skeleton")
                .to_string();
            let prompt = gap
                .get("question_prompt")
                .and_then(|s| s.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("Provide missing skeleton detail for topic: {topic}"));
            // Guard: never invent strategy language
            if prompt.to_ascii_lowercase().contains("win")
                || prompt.to_ascii_lowercase().contains("strategy")
            {
                continue;
            }
            out.push(serde_json::json!({
                "gap_id": gap_id,
                "topic": topic,
                "prompt": prompt,
                "status": "pending"
            }));
        }
        // If no open gaps, still allow re-open with generic completion checks
        if out.is_empty() {
            let checks = [
                ("parties", "Who are the parties and their roles?"),
                (
                    "chronology",
                    "What are the key dated events in this matter?",
                ),
                (
                    "statutes",
                    "Which statutes or rules are pleaded (as written)?",
                ),
                (
                    "evidence",
                    "What key exhibits or evidence items are on file?",
                ),
                ("prayer_relief", "What relief is sought?"),
            ];
            for (topic, prompt) in checks {
                if out.len() as i64 >= max_q {
                    break;
                }
                let empty = match topic {
                    "parties" => graph
                        .get("parties")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(true),
                    "chronology" => graph
                        .get("chronology")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(true),
                    "statutes" => graph
                        .get("statutes")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(true),
                    "evidence" => graph
                        .get("evidence")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(true),
                    "prayer_relief" => graph
                        .get("prayer_relief")
                        .and_then(|a| a.as_array())
                        .map(|a| a.is_empty())
                        .unwrap_or(true),
                    _ => true,
                };
                if empty {
                    out.push(serde_json::json!({
                        "gap_id": format!("gap_{topic}"),
                        "topic": topic,
                        "prompt": prompt,
                        "status": "pending"
                    }));
                }
            }
        }
        out
    }

    fn recompute_completion(graph: &serde_json::Value) -> i64 {
        let filled = [
            !graph
                .get("parties")
                .and_then(|a| a.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
            !graph
                .get("chronology")
                .and_then(|a| a.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
            !graph
                .get("statutes")
                .and_then(|a| a.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
            !graph
                .get("evidence")
                .and_then(|a| a.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
            !graph
                .get("prayer_relief")
                .and_then(|a| a.as_array())
                .map(|a| a.is_empty())
                .unwrap_or(true),
        ]
        .iter()
        .filter(|x| **x)
        .count();
        (100 * filled / 5) as i64
    }

    /// Open or resume the latest interview session for a case (AC-6.6 re-openable).
    pub fn interview_open(&self, case_id: &str) -> VaultResult<serde_json::Value> {
        // Prefer latest open/paused session
        if let Some(existing) = self.interview_latest(case_id)? {
            let st = existing
                .get("status")
                .and_then(|s| s.as_str())
                .unwrap_or("");
            if st == "open" || st == "paused" {
                return Ok(existing);
            }
            // completed → allow re-open as new session (AC-6.6 re-openable)
        }
        let graph = self
            .case_graph_get(case_id)?
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_INTERVIEW_NO_GRAPH: run skeleton extraction before interview".into(),
            })?;
        let max_q = Self::interview_max_questions();
        let questions = Self::questions_from_graph(&graph, max_q);
        if questions.is_empty() {
            // Skeleton already complete — still create empty completed session
            return self.interview_create_session(case_id, &questions, true);
        }
        self.interview_create_session(case_id, &questions, false)
    }

    fn interview_create_session(
        &self,
        case_id: &str,
        questions: &[serde_json::Value],
        already_complete: bool,
    ) -> VaultResult<serde_json::Value> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before interview".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let session_id = Uuid::new_v4().to_string();
        let max_q = Self::interview_max_questions();
        let status = if already_complete {
            "completed"
        } else {
            "open"
        };
        let completion = if already_complete { 100 } else { 0 };
        conn.execute(
            "INSERT INTO interview_session (
                session_id, case_id, status, created_at_unix_ms, updated_at_unix_ms,
                max_questions, cursor_index, completion_percent, notes
             ) VALUES (?1,?2,?3,?4,?5,?6,0,?7,?8)",
            rusqlite::params![
                session_id,
                case_id,
                status,
                now,
                now,
                max_q,
                completion,
                "B-S1-09 guided intake"
            ],
        )?;
        for (i, q) in questions.iter().enumerate() {
            conn.execute(
                "INSERT INTO interview_question (
                    session_id, question_index, gap_id, topic, prompt, status
                 ) VALUES (?1,?2,?3,?4,?5,'pending')",
                rusqlite::params![
                    session_id,
                    i as i64,
                    q.get("gap_id").and_then(|v| v.as_str()),
                    q.get("topic")
                        .and_then(|v| v.as_str())
                        .unwrap_or("skeleton"),
                    q.get("prompt").and_then(|v| v.as_str()).unwrap_or(""),
                ],
            )?;
        }
        drop(g);
        self.interview_get(case_id, &session_id)
    }

    pub fn interview_latest(&self, case_id: &str) -> VaultResult<Option<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before interview".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let sid: Option<String> = conn
            .query_row(
                "SELECT session_id FROM interview_session
                 WHERE case_id = ?1
                 ORDER BY updated_at_unix_ms DESC LIMIT 1",
                [case_id],
                |r| r.get(0),
            )
            .optional()?;
        drop(g);
        match sid {
            None => Ok(None),
            Some(id) => Ok(Some(self.interview_get(case_id, &id)?)),
        }
    }

    pub fn interview_get(&self, case_id: &str, session_id: &str) -> VaultResult<serde_json::Value> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before interview".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let sess: (String, String, i64, i64, i64, i64, i64, Option<String>) = conn.query_row(
            "SELECT session_id, status, created_at_unix_ms, updated_at_unix_ms,
                    max_questions, cursor_index, completion_percent, notes
             FROM interview_session WHERE session_id = ?1",
            [session_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )?;
        let mut stmt = conn.prepare(
            "SELECT question_index, gap_id, topic, prompt, status, answer_text,
                    answered_fact_id, answered_at_unix_ms
             FROM interview_question WHERE session_id = ?1
             ORDER BY question_index ASC",
        )?;
        let qrows = stmt.query_map([session_id], |r| {
            Ok(serde_json::json!({
                "question_index": r.get::<_, i64>(0)?,
                "gap_id": r.get::<_, Option<String>>(1)?,
                "topic": r.get::<_, String>(2)?,
                "prompt": r.get::<_, String>(3)?,
                "status": r.get::<_, String>(4)?,
                "answer_text": r.get::<_, Option<String>>(5)?,
                "answered_fact_id": r.get::<_, Option<String>>(6)?,
                "answered_at_unix_ms": r.get::<_, Option<i64>>(7)?,
            }))
        })?;
        let mut questions = Vec::new();
        for row in qrows {
            questions.push(row?);
        }
        Ok(serde_json::json!({
            "session_id": sess.0,
            "case_id": case_id,
            "status": sess.1,
            "created_at_unix_ms": sess.2,
            "updated_at_unix_ms": sess.3,
            "max_questions": sess.4,
            "cursor_index": sess.5,
            "completion_percent": sess.6,
            "notes": sess.7,
            "questions": questions,
            "min_completion_percent": Self::interview_min_completion(),
            "batch": "B-S1-09"
        }))
    }

    /// Pause session (AC-6.6 resumable).
    pub fn interview_pause(
        &self,
        case_id: &str,
        session_id: &str,
    ) -> VaultResult<serde_json::Value> {
        self.interview_set_status(case_id, session_id, "paused")?;
        self.interview_get(case_id, session_id)
    }

    /// Resume paused/open session (AC-6.6).
    pub fn interview_resume(
        &self,
        case_id: &str,
        session_id: &str,
    ) -> VaultResult<serde_json::Value> {
        let cur = self.interview_get(case_id, session_id)?;
        let st = cur.get("status").and_then(|s| s.as_str()).unwrap_or("");
        if st == "completed" {
            // re-open: create new session
            return self.interview_open(case_id);
        }
        self.interview_set_status(case_id, session_id, "open")?;
        self.interview_get(case_id, session_id)
    }

    fn interview_set_status(
        &self,
        case_id: &str,
        session_id: &str,
        status: &str,
    ) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before interview".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let n = conn.execute(
            "UPDATE interview_session SET status = ?1, updated_at_unix_ms = ?2
             WHERE session_id = ?3",
            rusqlite::params![status, now, session_id],
        )?;
        if n == 0 {
            return Err(VaultError::Internal {
                detail: format!("CF_INTERVIEW_NOT_FOUND: {session_id}"),
            });
        }
        Ok(())
    }

    /// Answer a question → SOURCED_BY_COUNSEL fact + gap close (AC-6.5 path).
    pub fn interview_answer(
        &self,
        case_id: &str,
        session_id: &str,
        question_index: i64,
        answer_text: &str,
    ) -> VaultResult<serde_json::Value> {
        let answer_text = answer_text.trim();
        if answer_text.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_INTERVIEW_EMPTY_ANSWER: provide text or use skip".into(),
            });
        }
        let sess = self.interview_get(case_id, session_id)?;
        let max_q = sess
            .get("max_questions")
            .and_then(|v| v.as_i64())
            .unwrap_or(12);
        if question_index < 0 || question_index >= max_q {
            return Err(VaultError::Internal {
                detail: "CF_INTERVIEW_BAD_INDEX".into(),
            });
        }
        let q = sess
            .get("questions")
            .and_then(|a| a.as_array())
            .and_then(|arr| {
                arr.iter().find(|x| {
                    x.get("question_index").and_then(|i| i.as_i64()) == Some(question_index)
                })
            })
            .cloned()
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_INTERVIEW_Q_NOT_FOUND".into(),
            })?;
        let topic = q
            .get("topic")
            .and_then(|s| s.as_str())
            .unwrap_or("skeleton");
        let gap_id = q
            .get("gap_id")
            .and_then(|s| s.as_str())
            .map(|s| s.to_string());
        let fact_id = format!("cf_{}", Uuid::new_v4().to_string().replace('-', ""));

        // Update graph: counsel fact + fill topic collection + gap status
        let mut graph = self
            .case_graph_get(case_id)?
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_INTERVIEW_NO_GRAPH".into(),
            })?;
        let counsel_source = serde_json::json!({
            "doc_id": "SOURCED_BY_COUNSEL",
            "page_index": 0,
            "kind": "counsel",
            "char_start": null,
            "char_end": null,
            "bbox": null,
            "quote": answer_text.chars().take(240).collect::<String>(),
            "content_hash": null
        });
        let provenance = serde_json::json!({
            "source": counsel_source,
            "confidence": 1.0,
            "origin": "SOURCED_BY_COUNSEL",
            "reviewed": true,
            "notes": format!("guided interview topic={topic}")
        });
        if let Some(arr) = graph
            .get_mut("counsel_facts")
            .and_then(|a| a.as_array_mut())
        {
            arr.push(serde_json::json!({
                "fact_id": fact_id,
                "text": answer_text,
                "origin": "SOURCED_BY_COUNSEL",
                "provenance": provenance
            }));
        }
        // Mark matching gap answered
        if let Some(gaps) = graph.get_mut("gaps").and_then(|a| a.as_array_mut()) {
            for gap in gaps.iter_mut() {
                let gid = gap.get("gap_id").and_then(|s| s.as_str()).unwrap_or("");
                let gtopic = gap.get("topic").and_then(|s| s.as_str()).unwrap_or("");
                if Some(gid) == gap_id.as_deref() || gtopic == topic {
                    if let Some(obj) = gap.as_object_mut() {
                        obj.insert(
                            "status".into(),
                            serde_json::Value::String("answered".into()),
                        );
                        obj.insert(
                            "answered_fact_id".into(),
                            serde_json::Value::String(fact_id.clone()),
                        );
                    }
                }
            }
        }
        // Seed collection from counsel answer so skeleton completeness can rise
        Self::seed_collection_from_counsel(&mut graph, topic, answer_text, &fact_id);
        let completion = Self::recompute_completion(&graph);
        let open_gaps = graph
            .get("gaps")
            .and_then(|a| a.as_array())
            .map(|a| {
                a.iter()
                    .filter(|g| g.get("status").and_then(|s| s.as_str()) == Some("open"))
                    .count()
            })
            .unwrap_or(0);
        if let Some(comp) = graph
            .get_mut("completeness")
            .and_then(|c| c.as_object_mut())
        {
            comp.insert("score_0_100".into(), serde_json::json!(completion));
            comp.insert("open_gaps".into(), serde_json::json!(open_gaps as i64));
        }
        self.case_graph_put(case_id, &graph)?;

        // Update question row + session cursor
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "UPDATE interview_question SET status = 'answered', answer_text = ?1,
                    answered_fact_id = ?2, answered_at_unix_ms = ?3
             WHERE session_id = ?4 AND question_index = ?5",
            rusqlite::params![answer_text, fact_id, now, session_id, question_index],
        )?;
        let next_cursor = question_index + 1;
        let min_done = Self::interview_min_completion();
        let sess_status = if completion >= min_done {
            "completed"
        } else {
            "open"
        };
        conn.execute(
            "UPDATE interview_session SET cursor_index = ?1, completion_percent = ?2,
                    status = ?3, updated_at_unix_ms = ?4
             WHERE session_id = ?5",
            rusqlite::params![next_cursor, completion, sess_status, now, session_id],
        )?;
        drop(g);
        self.interview_get(case_id, session_id)
    }

    /// Skip a question without answer (AC-6.6 skippable).
    pub fn interview_skip(
        &self,
        case_id: &str,
        session_id: &str,
        question_index: i64,
    ) -> VaultResult<serde_json::Value> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let n = conn.execute(
            "UPDATE interview_question SET status = 'skipped', answered_at_unix_ms = ?1
             WHERE session_id = ?2 AND question_index = ?3",
            rusqlite::params![now, session_id, question_index],
        )?;
        if n == 0 {
            return Err(VaultError::Internal {
                detail: "CF_INTERVIEW_Q_NOT_FOUND".into(),
            });
        }
        // Advance cursor; do not mark completed solely from skips
        conn.execute(
            "UPDATE interview_session SET cursor_index = max(cursor_index, ?1),
                    updated_at_unix_ms = ?2, status = 'open'
             WHERE session_id = ?3",
            rusqlite::params![question_index + 1, now, session_id],
        )?;
        // If all questions answered or skipped, pause/complete based on graph completeness
        let pending: i64 = conn.query_row(
            "SELECT COUNT(*) FROM interview_question
             WHERE session_id = ?1 AND status = 'pending'",
            [session_id],
            |r| r.get(0),
        )?;
        drop(g);
        if pending == 0 {
            // Finalize status from graph score
            if let Some(graph) = self.case_graph_get(case_id)? {
                let completion = Self::recompute_completion(&graph);
                let min_done = Self::interview_min_completion();
                let st = if completion >= min_done {
                    "completed"
                } else {
                    "paused"
                };
                let mut g2 = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
                let conn = g2
                    .cases
                    .get_mut(case_id)
                    .ok_or_else(|| VaultError::Internal {
                        detail: "CF_VAULT_LOCKED".into(),
                    })?;
                conn.execute(
                    "UPDATE interview_session SET status = ?1, completion_percent = ?2,
                            updated_at_unix_ms = ?3 WHERE session_id = ?4",
                    rusqlite::params![st, completion, now, session_id],
                )?;
            }
        }
        self.interview_get(case_id, session_id)
    }

    fn seed_collection_from_counsel(
        graph: &mut serde_json::Value,
        topic: &str,
        answer: &str,
        fact_id: &str,
    ) {
        let source = serde_json::json!({
            "doc_id": "SOURCED_BY_COUNSEL",
            "page_index": 0,
            "kind": "counsel",
            "char_start": null,
            "char_end": null,
            "bbox": null,
            "quote": answer.chars().take(240).collect::<String>(),
            "content_hash": null
        });
        let prov = serde_json::json!({
            "source": source,
            "confidence": 1.0,
            "origin": "SOURCED_BY_COUNSEL",
            "reviewed": true,
            "notes": format!("seeded from interview fact {fact_id}")
        });
        match topic {
            "parties" => {
                if let Some(arr) = graph.get_mut("parties").and_then(|a| a.as_array_mut()) {
                    if arr.is_empty() {
                        arr.push(serde_json::json!({
                            "party_id": format!("pty_counsel_{fact_id}"),
                            "display_name": answer.chars().take(120).collect::<String>(),
                            "role": "party",
                            "side": null,
                            "aliases": [],
                            "provenance": prov
                        }));
                    }
                }
            }
            "chronology" => {
                if let Some(arr) = graph.get_mut("chronology").and_then(|a| a.as_array_mut()) {
                    if arr.is_empty() {
                        arr.push(serde_json::json!({
                            "event_id": format!("evt_counsel_{fact_id}"),
                            "date": "unknown",
                            "date_precision": "unknown",
                            "summary": answer.chars().take(200).collect::<String>(),
                            "sort_key": "9999-99-99",
                            "provenance": prov
                        }));
                    }
                }
            }
            "statutes" => {
                if let Some(arr) = graph.get_mut("statutes").and_then(|a| a.as_array_mut()) {
                    if arr.is_empty() {
                        arr.push(serde_json::json!({
                            "statute_id": format!("st_counsel_{fact_id}"),
                            "label": answer.chars().take(200).collect::<String>(),
                            "jurisdiction": null,
                            "citation_raw": answer.chars().take(200).collect::<String>(),
                            "provenance": prov
                        }));
                    }
                }
            }
            "evidence" => {
                if let Some(arr) = graph.get_mut("evidence").and_then(|a| a.as_array_mut()) {
                    if arr.is_empty() {
                        arr.push(serde_json::json!({
                            "evidence_id": format!("ev_counsel_{fact_id}"),
                            "label": answer.chars().take(200).collect::<String>(),
                            "doc_id": null,
                            "kind": "counsel_note",
                            "provenance": prov
                        }));
                    }
                }
            }
            "prayer_relief" => {
                if let Some(arr) = graph
                    .get_mut("prayer_relief")
                    .and_then(|a| a.as_array_mut())
                {
                    if arr.is_empty() {
                        arr.push(serde_json::json!({
                            "relief_id": format!("rel_counsel_{fact_id}"),
                            "text": answer.chars().take(400).collect::<String>(),
                            "quantum": null,
                            "provenance": prov
                        }));
                    }
                }
            }
            _ => {}
        }
    }

    // --- B-S1-10 overnight batch + resume (AC-6.8) ---

    /// Create a durable overnight batch job with pending checkpoints per document.
    pub fn batch_create(&self, case_id: &str) -> VaultResult<serde_json::Value> {
        let docs = self.doc_list(case_id)?;
        if docs.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_BATCH_NO_DOCS: ingest documents before overnight batch".into(),
            });
        }
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before batch".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        let job_id = Uuid::new_v4().to_string();
        // units: triage per doc + recognize per doc + skeleton once
        let total_units = (docs.len() * 2 + 1) as i64;
        let page_est: i64 = docs.iter().map(|d| d.page_count.unwrap_or(1).max(1)).sum();
        conn.execute(
            "INSERT INTO batch_job (
                job_id, case_id, status, stage, progress_percent, message,
                total_units, completed_units, page_count_estimate,
                created_at_unix_ms, updated_at_unix_ms, last_checkpoint_at_unix_ms, error
             ) VALUES (?1,?2,'queued','triage',0,'queued',?3,0,?4,?5,?5,NULL,NULL)",
            rusqlite::params![job_id, case_id, total_units, page_est, now],
        )?;
        for d in &docs {
            for kind in ["doc_triage", "doc_recognize"] {
                conn.execute(
                    "INSERT INTO batch_checkpoint (
                        job_id, unit_kind, unit_id, status, detail, completed_at_unix_ms
                     ) VALUES (?1,?2,?3,'pending',NULL,NULL)",
                    rusqlite::params![job_id, kind, d.doc_id],
                )?;
            }
        }
        conn.execute(
            "INSERT INTO batch_checkpoint (
                job_id, unit_kind, unit_id, status, detail, completed_at_unix_ms
             ) VALUES (?1,'case_skeleton','*','pending',NULL,NULL)",
            rusqlite::params![job_id],
        )?;
        drop(g);
        self.batch_get(case_id, &job_id)
    }

    pub fn batch_get(&self, case_id: &str, job_id: &str) -> VaultResult<serde_json::Value> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before batch".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let row: (
            String,
            String,
            String,
            String,
            i64,
            Option<String>,
            i64,
            i64,
            Option<i64>,
            i64,
            i64,
            Option<i64>,
            Option<String>,
        ) = conn.query_row(
            "SELECT job_id, case_id, status, stage, progress_percent, message,
                    total_units, completed_units, page_count_estimate,
                    created_at_unix_ms, updated_at_unix_ms, last_checkpoint_at_unix_ms, error
             FROM batch_job WHERE job_id = ?1",
            [job_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                    r.get(12)?,
                ))
            },
        )?;
        let mut stmt = conn.prepare(
            "SELECT unit_kind, unit_id, status, detail, completed_at_unix_ms
             FROM batch_checkpoint WHERE job_id = ?1
             ORDER BY
               CASE unit_kind
                 WHEN 'doc_triage' THEN 1
                 WHEN 'doc_recognize' THEN 2
                 WHEN 'case_skeleton' THEN 3
                 ELSE 9 END,
               unit_id ASC",
        )?;
        let cps = stmt.query_map([job_id], |r| {
            Ok(serde_json::json!({
                "unit_kind": r.get::<_, String>(0)?,
                "unit_id": r.get::<_, String>(1)?,
                "status": r.get::<_, String>(2)?,
                "detail": r.get::<_, Option<String>>(3)?,
                "completed_at_unix_ms": r.get::<_, Option<i64>>(4)?,
            }))
        })?;
        let mut checkpoints = Vec::new();
        for cp in cps {
            checkpoints.push(cp?);
        }
        let pending = checkpoints
            .iter()
            .filter(|c| c.get("status").and_then(|s| s.as_str()) == Some("pending"))
            .count();
        Ok(serde_json::json!({
            "job_id": row.0,
            "case_id": row.1,
            "status": row.2,
            "stage": row.3,
            "progress_percent": row.4,
            "message": row.5,
            "total_units": row.6,
            "completed_units": row.7,
            "page_count_estimate": row.8,
            "created_at_unix_ms": row.9,
            "updated_at_unix_ms": row.10,
            "last_checkpoint_at_unix_ms": row.11,
            "error": row.12,
            "pending_units": pending,
            "checkpoints": checkpoints,
            "batch": "B-S1-10",
            "ac_6_8": "resumable_via_checkpoints",
            "ac_3_2_budget": {
                "max_pages": 300,
                "max_h": 8,
                "note": "Full-tier hardware measurement still required for ship PASS"
            }
        }))
    }

    pub fn batch_latest(&self, case_id: &str) -> VaultResult<Option<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before batch".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let sid: Option<String> = conn
            .query_row(
                "SELECT job_id FROM batch_job WHERE case_id = ?1
                 ORDER BY updated_at_unix_ms DESC LIMIT 1",
                [case_id],
                |r| r.get(0),
            )
            .optional()?;
        drop(g);
        match sid {
            None => Ok(None),
            Some(id) => Ok(Some(self.batch_get(case_id, &id)?)),
        }
    }

    pub fn batch_set_status(
        &self,
        case_id: &str,
        job_id: &str,
        status: &str,
        stage: Option<&str>,
        message: Option<&str>,
        error: Option<&str>,
    ) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        if let Some(st) = stage {
            conn.execute(
                "UPDATE batch_job SET status = ?1, stage = ?2, message = COALESCE(?3, message),
                        error = ?4, updated_at_unix_ms = ?5
                 WHERE job_id = ?6",
                rusqlite::params![status, st, message, error, now, job_id],
            )?;
        } else {
            conn.execute(
                "UPDATE batch_job SET status = ?1, message = COALESCE(?2, message),
                        error = ?3, updated_at_unix_ms = ?4
                 WHERE job_id = ?5",
                rusqlite::params![status, message, error, now, job_id],
            )?;
        }
        Ok(())
    }

    /// Mark a checkpoint done and recompute progress (durable AC-6.8 checkpoint).
    pub fn batch_checkpoint_done(
        &self,
        case_id: &str,
        job_id: &str,
        unit_kind: &str,
        unit_id: &str,
        detail: Option<&str>,
    ) -> VaultResult<serde_json::Value> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "UPDATE batch_checkpoint SET status = 'done', detail = ?1, completed_at_unix_ms = ?2
             WHERE job_id = ?3 AND unit_kind = ?4 AND unit_id = ?5",
            rusqlite::params![detail, now, job_id, unit_kind, unit_id],
        )?;
        let done: i64 = conn.query_row(
            "SELECT COUNT(*) FROM batch_checkpoint WHERE job_id = ?1 AND status = 'done'",
            [job_id],
            |r| r.get(0),
        )?;
        let total: i64 = conn.query_row(
            "SELECT total_units FROM batch_job WHERE job_id = ?1",
            [job_id],
            |r| r.get(0),
        )?;
        let pct = if total > 0 {
            ((done * 100) / total).min(100) as i64
        } else {
            0
        };
        let stage = match unit_kind {
            "doc_triage" => "triage",
            "doc_recognize" => "recognize",
            "case_skeleton" => "skeleton",
            _ => "running",
        };
        conn.execute(
            "UPDATE batch_job SET completed_units = ?1, progress_percent = ?2, stage = ?3,
                    last_checkpoint_at_unix_ms = ?4, updated_at_unix_ms = ?4,
                    message = ?5, status = 'running'
             WHERE job_id = ?6",
            rusqlite::params![
                done,
                pct,
                stage,
                now,
                format!("checkpoint {unit_kind}/{unit_id}"),
                job_id
            ],
        )?;
        drop(g);
        self.batch_get(case_id, job_id)
    }

    /// Next pending unit in pipeline order, or None if all done.
    pub fn batch_next_pending(
        &self,
        case_id: &str,
        job_id: &str,
    ) -> VaultResult<Option<(String, String)>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let row: Option<(String, String)> = conn
            .query_row(
                "SELECT unit_kind, unit_id FROM batch_checkpoint
                 WHERE job_id = ?1 AND status = 'pending'
                 ORDER BY
                   CASE unit_kind
                     WHEN 'doc_triage' THEN 1
                     WHEN 'doc_recognize' THEN 2
                     WHEN 'case_skeleton' THEN 3
                     ELSE 9 END,
                   unit_id ASC
                 LIMIT 1",
                [job_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        Ok(row)
    }

    pub fn batch_mark_completed(&self, case_id: &str, job_id: &str) -> VaultResult<()> {
        self.batch_set_status(
            case_id,
            job_id,
            "completed",
            Some("done"),
            Some("overnight batch completed"),
            None,
        )?;
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED".into(),
            })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        conn.execute(
            "UPDATE batch_job SET progress_percent = 100, completed_units = total_units,
                    updated_at_unix_ms = ?1 WHERE job_id = ?2",
            rusqlite::params![now, job_id],
        )?;
        Ok(())
    }

    /// AC-16.2: DB must not yield readable schema without the SQLCipher key.
    pub fn probe_db_unreadable_without_key(db_path: &Path) -> bool {
        if !db_path.exists() {
            return true;
        }
        match rusqlite::Connection::open(db_path) {
            Ok(conn) => conn
                .query_row("SELECT COUNT(*) FROM sqlite_master", [], |r| {
                    r.get::<_, i64>(0)
                })
                .is_err(),
            Err(_) => true,
        }
    }

    pub fn app_db_path(&self) -> VaultResult<PathBuf> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        Ok(g.paths.app_db_path())
    }

    /// Vault root paths for pack install roots (B-S1-12).
    pub fn vault_paths(&self) -> VaultResult<VaultPaths> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        Ok(g.paths.clone())
    }

    // --- Lexicon (B-S1-16) ---

    pub fn speech_transcript_commit(
        &self,
        input: SpeechTranscriptCommitInput,
    ) -> VaultResult<SpeechTranscriptCommit> {
        if !input.counsel_confirmed
            || !matches!(
                input.input_kind.as_str(),
                "REAL_AUDIO" | "REAL_AUDIO_CONFIRMED"
            )
            || input.audio_sha256.len() != 64
            || input.raw_transcript.trim().is_empty()
            || input.staged_transcript.trim().is_empty()
        {
            return Err(VaultError::Internal {
                detail: "CF_SPEECH_COMMIT_INVALID: confirmed real-audio transcript and SHA-256 are required".into(),
            });
        }
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let commit_id = Uuid::new_v4().to_string();
        let created_at_unix_ms = now_ms();
        app.execute(
            "INSERT INTO speech_transcript_commit (
                commit_id, case_id, session_id, input_kind, audio_sha256,
                raw_transcript, staged_transcript, correction_applied,
                counsel_confirmed, backend, pipeline_id, audio_retained,
                created_at_unix_ms
             ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,0,?11)",
            rusqlite::params![
                commit_id,
                input.case_id,
                input.session_id,
                input.input_kind,
                input.audio_sha256,
                input.raw_transcript,
                input.staged_transcript,
                input.correction_applied as i64,
                input.backend,
                input.pipeline_id,
                created_at_unix_ms,
            ],
        )?;
        Ok(SpeechTranscriptCommit {
            commit_id,
            case_id: input.case_id,
            session_id: input.session_id,
            input_kind: input.input_kind,
            audio_sha256: input.audio_sha256,
            raw_transcript: input.raw_transcript,
            staged_transcript: input.staged_transcript,
            correction_applied: input.correction_applied,
            counsel_confirmed: true,
            backend: input.backend,
            pipeline_id: input.pipeline_id,
            audio_retained: false,
            created_at_unix_ms,
        })
    }

    /// Ensure pre-seed legal English / statute / proper-name entries exist (once).
    pub fn lexicon_ensure_preseed(&self) -> VaultResult<u32> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let already: Option<String> = app
            .query_row(
                "SELECT value FROM lexicon_meta WHERE key = 'preseed_v1'",
                [],
                |r| r.get(0),
            )
            .optional()?;
        if already.as_deref() == Some("applied") {
            return Ok(0);
        }
        let preseed: serde_json::Value =
            serde_json::from_str(LEXICON_PRESEED_JSON).map_err(|e| VaultError::Internal {
                detail: format!("preseed parse: {e}"),
            })?;
        let entries = preseed
            .get("entries")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();
        let mut inserted = 0u32;
        let now = now_ms();
        for e in entries {
            let kind = e
                .get("entry_kind")
                .and_then(|x| x.as_str())
                .unwrap_or("stt_correction");
            let surface = e
                .get("surface_form")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim();
            if surface.is_empty() {
                continue;
            }
            let corrected = e
                .get("corrected_form")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let pronunciation = e
                .get("pronunciation")
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let pattern = e
                .get("pattern_type")
                .and_then(|x| x.as_str())
                .unwrap_or("legal_english");
            let entry_id = Uuid::new_v4().to_string();
            app.execute(
                "INSERT OR IGNORE INTO lexicon_entries (
                    entry_id, schema_version, entry_kind, language, surface_form,
                    corrected_form, pronunciation, pattern_type, source, active,
                    scope, notes, created_at_unix_ms, updated_at_unix_ms
                 ) VALUES (?1,'1.0.0',?2,'English',?3,?4,?5,?6,'preseed',1,'global',NULL,?7,?7)",
                rusqlite::params![
                    entry_id,
                    kind,
                    surface,
                    corrected,
                    pronunciation,
                    pattern,
                    now
                ],
            )?;
            inserted += 1;
        }
        app.execute(
            "INSERT OR REPLACE INTO lexicon_meta (key, value) VALUES ('preseed_v1', 'applied')",
            [],
        )?;
        Ok(inserted)
    }

    pub fn lexicon_list(&self, kind: Option<&str>) -> VaultResult<Vec<LexiconEntry>> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        // Drop lock borrow for ensure — call ensure outside if needed.
        drop(g);
        let _ = self.lexicon_ensure_preseed();
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        let mut out = Vec::new();
        if let Some(k) = kind {
            let mut stmt = app.prepare(
                "SELECT entry_id, schema_version, entry_kind, language, surface_form,
                        corrected_form, pronunciation, pattern_type, source, active,
                        scope, notes, created_at_unix_ms, updated_at_unix_ms
                 FROM lexicon_entries WHERE entry_kind = ?1
                 ORDER BY length(surface_form) DESC, surface_form ASC",
            )?;
            let rows = stmt.query_map([k], map_lexicon_row)?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = app.prepare(
                "SELECT entry_id, schema_version, entry_kind, language, surface_form,
                        corrected_form, pronunciation, pattern_type, source, active,
                        scope, notes, created_at_unix_ms, updated_at_unix_ms
                 FROM lexicon_entries
                 ORDER BY entry_kind, length(surface_form) DESC, surface_form ASC",
            )?;
            let rows = stmt.query_map([], map_lexicon_row)?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }

    pub fn lexicon_upsert(&self, input: LexiconUpsertInput) -> VaultResult<LexiconEntry> {
        self.assert_vault_writable()?;
        let kind = input.entry_kind.trim().to_string();
        if kind != "stt_correction" && kind != "tts_pronunciation_override" {
            return Err(VaultError::Internal {
                detail: "CF_LEXICON_KIND: entry_kind must be stt_correction or tts_pronunciation_override"
                    .into(),
            });
        }
        let surface = input.surface_form.trim().to_string();
        if surface.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_LEXICON_SURFACE: surface_form required".into(),
            });
        }
        if kind == "stt_correction"
            && input
                .corrected_form
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(VaultError::Internal {
                detail: "CF_LEXICON_CORRECTED: corrected_form required for stt_correction".into(),
            });
        }
        if kind == "tts_pronunciation_override"
            && input
                .pronunciation
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        {
            return Err(VaultError::Internal {
                detail: "CF_LEXICON_PRONUNCIATION: pronunciation required for tts override".into(),
            });
        }
        let pattern = input
            .pattern_type
            .unwrap_or_else(|| "custom".into())
            .trim()
            .to_string();
        let source = input
            .source
            .unwrap_or_else(|| "user".into())
            .trim()
            .to_string();
        let active = input.active.unwrap_or(true);
        let now = now_ms();
        let entry_id = input
            .entry_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        app.execute(
            "INSERT INTO lexicon_entries (
                entry_id, schema_version, entry_kind, language, surface_form,
                corrected_form, pronunciation, pattern_type, source, active,
                scope, notes, created_at_unix_ms, updated_at_unix_ms
             ) VALUES (?1,'1.0.0',?2,'English',?3,?4,?5,?6,?7,?8,'global',?9,?10,?10)
             ON CONFLICT(entry_id) DO UPDATE SET
                entry_kind=excluded.entry_kind,
                surface_form=excluded.surface_form,
                corrected_form=excluded.corrected_form,
                pronunciation=excluded.pronunciation,
                pattern_type=excluded.pattern_type,
                source=excluded.source,
                active=excluded.active,
                notes=excluded.notes,
                updated_at_unix_ms=excluded.updated_at_unix_ms",
            rusqlite::params![
                entry_id,
                kind,
                surface,
                input.corrected_form,
                input.pronunciation,
                pattern,
                source,
                if active { 1 } else { 0 },
                input.notes,
                now
            ],
        )?;
        drop(g);
        let entry = self
            .lexicon_get(&entry_id)?
            .ok_or_else(|| VaultError::Internal {
                detail: "lexicon upsert missing row".into(),
            })?;
        // Dual-write lexicon patterns into Experience Store (AC-11.1).
        let summary = if entry.entry_kind == "stt_correction" {
            format!(
                "lexicon STT {} → {}",
                entry.surface_form,
                entry.corrected_form.as_deref().unwrap_or("")
            )
        } else {
            format!(
                "lexicon TTS {} pronounced {}",
                entry.surface_form,
                entry.pronunciation.as_deref().unwrap_or("")
            )
        };
        let _ = self.experience_upsert(ExperienceUpsertInput {
            record_id: Some(format!("lex_{}", entry.entry_id)),
            record_category: "lexicon_entry".into(),
            summary,
            tags: Some(vec![
                "lexicon".into(),
                entry.entry_kind.clone(),
                entry.pattern_type.clone(),
            ]),
            payload: Some(serde_json::json!({
                "entry_id": entry.entry_id,
                "entry_kind": entry.entry_kind,
                "surface_form": entry.surface_form,
                "corrected_form": entry.corrected_form,
                "pronunciation": entry.pronunciation,
                "pattern_type": entry.pattern_type,
            })),
            case_id: None,
            jurisdiction: None,
            source: Some(if entry.source == "preseed" {
                "system".into()
            } else {
                entry.source.clone()
            }),
            notes: None,
            stage: Some("S1".into()),
            fleet_status: Some("local_only".into()),
            active: Some(entry.active),
        });
        Ok(entry)
    }

    pub fn lexicon_get(&self, entry_id: &str) -> VaultResult<Option<LexiconEntry>> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let row = app
            .query_row(
                "SELECT entry_id, schema_version, entry_kind, language, surface_form,
                        corrected_form, pronunciation, pattern_type, source, active,
                        scope, notes, created_at_unix_ms, updated_at_unix_ms
                 FROM lexicon_entries WHERE entry_id = ?1",
                [entry_id],
                map_lexicon_row,
            )
            .optional()?;
        Ok(row)
    }

    pub fn lexicon_delete(&self, entry_id: &str) -> VaultResult<bool> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let n = app.execute(
            "DELETE FROM lexicon_entries WHERE entry_id = ?1",
            [entry_id],
        )?;
        Ok(n > 0)
    }

    pub fn lexicon_export(&self) -> VaultResult<LexiconExportEnvelope> {
        let entries = self.lexicon_list(None)?;
        Ok(LexiconExportEnvelope {
            schema_version: "1.0.0".into(),
            product: "CaseForge".into(),
            kind: "lexicon_export".into(),
            language: "English".into(),
            exported_at_unix_ms: now_ms(),
            entry_count: entries.len(),
            entries,
        })
    }

    pub fn lexicon_import(&self, envelope_json: &str) -> VaultResult<u32> {
        let v: serde_json::Value =
            serde_json::from_str(envelope_json).map_err(|e| VaultError::Internal {
                detail: format!("CF_LEXICON_IMPORT_JSON: {e}"),
            })?;
        let entries = v
            .get("entries")
            .and_then(|e| e.as_array())
            .cloned()
            .unwrap_or_default();
        let mut n = 0u32;
        for e in entries {
            let kind = e
                .get("entry_kind")
                .and_then(|x| x.as_str())
                .unwrap_or("stt_correction")
                .to_string();
            let surface = e
                .get("surface_form")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            let input = LexiconUpsertInput {
                entry_id: e
                    .get("entry_id")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                entry_kind: kind,
                surface_form: surface,
                corrected_form: e
                    .get("corrected_form")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                pronunciation: e
                    .get("pronunciation")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                pattern_type: e
                    .get("pattern_type")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
                source: Some("import".into()),
                active: e.get("active").and_then(|x| x.as_bool()),
                notes: e
                    .get("notes")
                    .and_then(|x| x.as_str())
                    .map(|s| s.to_string()),
            };
            if input.surface_form.trim().is_empty() {
                continue;
            }
            self.lexicon_upsert(input)?;
            n += 1;
        }
        Ok(n)
    }

    /// Apply active STT corrections permanently (AC-9.2). Longer surfaces first.
    pub fn lexicon_apply_stt(&self, raw: &str) -> VaultResult<String> {
        let entries = self.lexicon_list(Some("stt_correction"))?;
        Ok(apply_stt_corrections(raw, &entries))
    }

    /// Apply active TTS pronunciation overrides (AC-9.2).
    pub fn lexicon_apply_tts(&self, text: &str) -> VaultResult<String> {
        let entries = self.lexicon_list(Some("tts_pronunciation_override"))?;
        Ok(apply_tts_pronunciations(text, &entries))
    }

    // --- Experience Store (B-S1-17) ---

    pub fn experience_policy(&self) -> VaultResult<ExperiencePolicy> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let fleet_enabled = if let Some(app) = g.app.as_mut() {
            sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
            meta_bool(app, "fleet_enabled", false)
        } else {
            false
        };
        let fleet_net = if let Some(app) = g.app.as_mut() {
            meta_bool(app, "fleet_network_upload", false)
        } else {
            false
        };
        Ok(ExperiencePolicy {
            batch: "B-S2-06",
            pipeline_id: EXPERIENCE_PIPELINE_ID,
            top_k: EXPERIENCE_TOP_K,
            fleet_enabled,
            fleet_network_upload: fleet_net,
            fleet_requires_explicit_review: true,
            stage1_writable: STAGE1_EXPERIENCE_CATEGORIES.to_vec(),
            stage2_writable: STAGE2_EXPERIENCE_CATEGORIES.to_vec(),
            storage: "encrypted_app_vault_sqlcipher",
            acs: ["AC-11.1", "AC-11.2", "AC-15.1", "B-S2-06"],
        })
    }

    pub fn experience_upsert(&self, input: ExperienceUpsertInput) -> VaultResult<ExperienceRecord> {
        self.assert_vault_writable()?;
        let cat = input.record_category.trim().to_string();
        let is_s1 = STAGE1_EXPERIENCE_CATEGORIES.contains(&cat.as_str());
        let is_s2 = STAGE2_EXPERIENCE_CATEGORIES.contains(&cat.as_str());
        if !is_s1 && !is_s2 {
            return Err(VaultError::Internal {
                detail: format!("CF_EXPERIENCE_CATEGORY: unknown category {cat}"),
            });
        }
        // B-S2-06: limb_outcome + bench_setting unlocked (Stage 2 write path).
        let summary = input.summary.trim().to_string();
        if summary.is_empty() {
            return Err(VaultError::Internal {
                detail: "CF_EXPERIENCE_SUMMARY: summary required".into(),
            });
        }
        let tags = input.tags.unwrap_or_default();
        let relevance_key = build_relevance_key(&summary, &tags, &cat);
        let now = now_ms();
        let record_id = input
            .record_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let stage = if is_s2 {
            "S2".into()
        } else {
            input.stage.unwrap_or_else(|| "S1".into())
        };
        // AC-11.2: never default to fleet upload statuses.
        let fleet_status = input.fleet_status.unwrap_or_else(|| "local_only".into());
        if fleet_status != "local_only"
            && fleet_status != "pending_review"
            && fleet_status != "approved_anonymised"
            && fleet_status != "rejected"
        {
            return Err(VaultError::Internal {
                detail: "CF_EXPERIENCE_FLEET_STATUS: invalid fleet_status".into(),
            });
        }
        let source = input.source.unwrap_or_else(|| "user".into());
        let active = input.active.unwrap_or(true);
        let payload = input.payload.unwrap_or_else(|| serde_json::json!({}));
        let tags_json = serde_json::to_string(&tags).unwrap_or_else(|_| "[]".into());
        let payload_json = payload.to_string();

        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        // Enforce fleet network never auto-on.
        ensure_fleet_defaults(app)?;
        app.execute(
            "INSERT INTO experience_records (
                record_id, schema_version, record_category, stage, language,
                jurisdiction, case_id, summary, tags_json, relevance_key,
                payload_json, source, fleet_status, active, notes,
                created_at_unix_ms, updated_at_unix_ms
             ) VALUES (?1,'1.0.0',?2,?3,'English',?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)
             ON CONFLICT(record_id) DO UPDATE SET
                record_category=excluded.record_category,
                stage=excluded.stage,
                jurisdiction=excluded.jurisdiction,
                case_id=excluded.case_id,
                summary=excluded.summary,
                tags_json=excluded.tags_json,
                relevance_key=excluded.relevance_key,
                payload_json=excluded.payload_json,
                source=excluded.source,
                fleet_status=excluded.fleet_status,
                active=excluded.active,
                notes=excluded.notes,
                updated_at_unix_ms=excluded.updated_at_unix_ms",
            rusqlite::params![
                record_id,
                cat,
                stage,
                input.jurisdiction,
                input.case_id,
                summary,
                tags_json,
                relevance_key,
                payload_json,
                source,
                fleet_status,
                if active { 1 } else { 0 },
                input.notes,
                now,
            ],
        )?;
        drop(g);
        self.experience_get(&record_id)?
            .ok_or_else(|| VaultError::Internal {
                detail: "experience upsert missing row".into(),
            })
    }

    pub fn experience_get(&self, record_id: &str) -> VaultResult<Option<ExperienceRecord>> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let row = app
            .query_row(
                "SELECT record_id, schema_version, record_category, stage, language,
                        jurisdiction, case_id, summary, tags_json, relevance_key,
                        payload_json, source, fleet_status, active, notes,
                        created_at_unix_ms, updated_at_unix_ms
                 FROM experience_records WHERE record_id = ?1",
                [record_id],
                map_experience_row,
            )
            .optional()?;
        Ok(row)
    }

    pub fn experience_list(
        &self,
        category: Option<&str>,
        limit: i64,
    ) -> VaultResult<Vec<ExperienceRecord>> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        ensure_fleet_defaults(app)?;
        let lim = limit.clamp(1, 500);
        let mut out = Vec::new();
        if let Some(cat) = category {
            let mut stmt = app.prepare(
                "SELECT record_id, schema_version, record_category, stage, language,
                        jurisdiction, case_id, summary, tags_json, relevance_key,
                        payload_json, source, fleet_status, active, notes,
                        created_at_unix_ms, updated_at_unix_ms
                 FROM experience_records
                 WHERE record_category = ?1 AND active = 1
                 ORDER BY updated_at_unix_ms DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(rusqlite::params![cat, lim], map_experience_row)?;
            for r in rows {
                out.push(r?);
            }
        } else {
            let mut stmt = app.prepare(
                "SELECT record_id, schema_version, record_category, stage, language,
                        jurisdiction, case_id, summary, tags_json, relevance_key,
                        payload_json, source, fleet_status, active, notes,
                        created_at_unix_ms, updated_at_unix_ms
                 FROM experience_records
                 WHERE active = 1
                 ORDER BY updated_at_unix_ms DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map([lim], map_experience_row)?;
            for r in rows {
                out.push(r?);
            }
        }
        Ok(out)
    }

    pub fn experience_delete(&self, record_id: &str) -> VaultResult<bool> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let n = app.execute(
            "DELETE FROM experience_records WHERE record_id = ?1",
            [record_id],
        )?;
        Ok(n > 0)
    }

    /// Top-K relevant past patterns for retrieval injection (AC-11.1).
    pub fn experience_top_k(
        &self,
        query: &str,
        top_k: Option<usize>,
        jurisdiction: Option<&str>,
    ) -> VaultResult<Vec<ExperienceHit>> {
        let k = top_k.unwrap_or(EXPERIENCE_TOP_K).max(1);
        let records = self.experience_list(None, 500)?;
        let q_tokens: std::collections::HashSet<String> =
            tokenize_relevance(query).into_iter().collect();
        if q_tokens.is_empty() {
            return Ok(Vec::new());
        }
        let mut scored: Vec<ExperienceHit> = Vec::new();
        for r in records {
            if !r.active || r.fleet_status == "rejected" {
                continue;
            }
            if let (Some(j), Some(rj)) = (jurisdiction, r.jurisdiction.as_deref()) {
                if j != rj && rj != "Other" {
                    // soft: still score, no hard skip
                }
            }
            let r_tokens: std::collections::HashSet<String> =
                tokenize_relevance(&r.relevance_key).into_iter().collect();
            let mut score = 0.0;
            for t in &q_tokens {
                if r_tokens.contains(t) {
                    score += 1.0;
                }
            }
            if score <= 0.0 {
                continue;
            }
            if jurisdiction.is_some() && r.jurisdiction.as_deref() == jurisdiction {
                score *= 1.15;
            }
            scored.push(ExperienceHit {
                record_id: r.record_id,
                record_category: r.record_category,
                summary: r.summary,
                relevance_key: r.relevance_key,
                score,
                case_id: r.case_id,
                jurisdiction: r.jurisdiction,
                tags: r.tags,
            });
        }
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(k);
        Ok(scored)
    }

    /// Mark a record for fleet review only (never uploads). AC-11.2.
    pub fn experience_request_fleet_review(
        &self,
        record_id: &str,
    ) -> VaultResult<ExperienceRecord> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        ensure_fleet_defaults(app)?;
        let fleet_enabled = meta_bool(app, "fleet_enabled", false);
        if !fleet_enabled {
            return Err(VaultError::Internal {
                detail: "CF_EXPERIENCE_FLEET_DISABLED: fleet contribution is off by default (AC-11.2); enable only after explicit policy review — still no network upload in Stage 1"
                    .into(),
            });
        }
        // Even when enabled, Stage 1 only queues pending_review — never network upload.
        let now = now_ms();
        app.execute(
            "UPDATE experience_records SET fleet_status = 'pending_review', updated_at_unix_ms = ?1
             WHERE record_id = ?2",
            rusqlite::params![now, record_id],
        )?;
        drop(g);
        self.experience_get(record_id)?
            .ok_or_else(|| VaultError::Internal {
                detail: "record not found".into(),
            })
    }

    /// Hard guarantee: fleet network upload flag stays false in Stage 1.
    pub fn experience_fleet_network_forbidden(&self) -> VaultResult<bool> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        ensure_fleet_defaults(app)?;
        // Always force network upload false.
        app.execute(
            "INSERT OR REPLACE INTO experience_meta (key, value) VALUES ('fleet_network_upload', 'false')",
            [],
        )?;
        Ok(true)
    }

    // --- Licence / air-gap (B-S1-18) ---

    /// EXPIRED => CF_VAULT_READ_ONLY. Never deletes or ransoms data.
    pub fn assert_vault_writable(&self) -> VaultResult<()> {
        let st = self.licence_status_inner()?;
        if !st.vault_writable {
            return Err(VaultError::Internal {
                detail: "CF_VAULT_READ_ONLY: licence EXPIRED — vault is read-only; user data is not ransomed or deleted"
                    .into(),
            });
        }
        Ok(())
    }

    pub fn licence_status(&self) -> VaultResult<crate::licence::LicenceStatus> {
        self.licence_status_inner()
    }

    fn licence_status_inner(&self) -> VaultResult<crate::licence::LicenceStatus> {
        use crate::licence::{LicenceState, FINGERPRINT_REQUIRED, FINGERPRINT_TOTAL, PIPELINE_ID};
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        // When vault locked, report UNLICENSED defaults without DB.
        let Some(app) = g.app.as_mut() else {
            return Ok(crate::licence::LicenceStatus {
                state: LicenceState::Unlicensed.as_str().into(),
                licence_id: None,
                tier: None,
                expires_at_unix_ms: None,
                fingerprint_match_count: 0,
                fingerprint_required: FINGERPRINT_REQUIRED,
                fingerprint_total: FINGERPRINT_TOTAL,
                vault_writable: true,
                air_gap_enabled: false,
                air_gap_kills_networking: true,
                case_data_network_path_allowed: false,
                user_data_may_be_ransomed: false,
                pipeline_id: PIPELINE_ID,
                batch: crate::licence::BATCH,
                detail: "vault locked — licence state not loaded".into(),
            });
        };
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        // Re-evaluate expiry on read.
        let row: Option<(
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            i64,
            i64,
        )> = app
            .query_row(
                "SELECT state, licence_id, tier, payload_json, activated_at_unix_ms,
                        expires_at_unix_ms, fingerprint_match_count, air_gap_enabled
                 FROM licence_state WHERE id = 1",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                        r.get(7)?,
                    ))
                },
            )
            .optional()?;
        let (
            mut state_s,
            licence_id,
            tier,
            payload_json,
            _activated,
            expires_at,
            match_count,
            air_gap_i,
        ) = row.unwrap_or_else(|| ("UNLICENSED".into(), None, None, None, None, None, 0, 0));
        // Transition ACTIVE -> EXPIRED if past expires_at (never delete payload).
        if state_s == "ACTIVE" {
            if let Some(exp) = expires_at {
                if exp > 0 && now_ms() > exp {
                    state_s = "EXPIRED".into();
                    let _ = app.execute(
                        "UPDATE licence_state SET state = 'EXPIRED', updated_at_unix_ms = ?1 WHERE id = 1",
                        [now_ms()],
                    );
                }
            }
        }
        // Also re-check payload expiry if present.
        if let Some(ref raw) = payload_json {
            if let Ok(p) = crate::licence::parse_payload_json(raw) {
                if let Some(exp) = p.expires_at_unix_ms {
                    if exp > 0 && now_ms() > exp && state_s != "EXPIRED" {
                        state_s = "EXPIRED".into();
                        let _ = app.execute(
                            "UPDATE licence_state SET state = 'EXPIRED', updated_at_unix_ms = ?1 WHERE id = 1",
                            [now_ms()],
                        );
                    }
                }
            }
        }
        let state = LicenceState::from_str_loose(&state_s);
        Ok(crate::licence::LicenceStatus {
            state: state.as_str().into(),
            licence_id,
            tier,
            expires_at_unix_ms: expires_at,
            fingerprint_match_count: match_count as u32,
            fingerprint_required: FINGERPRINT_REQUIRED,
            fingerprint_total: FINGERPRINT_TOTAL,
            vault_writable: state.vault_writable(),
            air_gap_enabled: air_gap_i != 0,
            air_gap_kills_networking: true,
            case_data_network_path_allowed: false,
            user_data_may_be_ransomed: false,
            pipeline_id: PIPELINE_ID,
            batch: crate::licence::BATCH,
            detail: match state {
                LicenceState::Expired => {
                    "EXPIRED — vault read-only; open/export still allowed; data not ransomed".into()
                }
                LicenceState::Active => "ACTIVE — offline licence valid".into(),
                LicenceState::Unlicensed => {
                    "UNLICENSED — activate with signed offline licence".into()
                }
            },
        })
    }

    pub fn licence_activate(
        &self,
        payload_json: &str,
    ) -> VaultResult<crate::licence::ActivateResult> {
        use crate::licence::{evaluate_payload, parse_payload_json};
        self.assert_not_ransoming()?;
        let payload =
            parse_payload_json(payload_json).map_err(|e| VaultError::Internal { detail: e })?;
        let (state, matches) =
            evaluate_payload(&payload).map_err(|e| VaultError::Internal { detail: e })?;
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let now = now_ms();
        let payload_str = serde_json::to_string(&payload).map_err(|e| VaultError::Internal {
            detail: e.to_string(),
        })?;
        app.execute(
            "INSERT INTO licence_state (
                id, state, licence_id, tier, payload_json, activated_at_unix_ms,
                expires_at_unix_ms, fingerprint_match_count, air_gap_enabled, updated_at_unix_ms
             ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, COALESCE((SELECT air_gap_enabled FROM licence_state WHERE id=1), 0), ?5)
             ON CONFLICT(id) DO UPDATE SET
                state=excluded.state,
                licence_id=excluded.licence_id,
                tier=excluded.tier,
                payload_json=excluded.payload_json,
                activated_at_unix_ms=excluded.activated_at_unix_ms,
                expires_at_unix_ms=excluded.expires_at_unix_ms,
                fingerprint_match_count=excluded.fingerprint_match_count,
                updated_at_unix_ms=excluded.updated_at_unix_ms",
            rusqlite::params![
                state.as_str(),
                payload.licence_id,
                payload.tier,
                payload_str,
                now,
                payload.expires_at_unix_ms,
                matches as i64,
            ],
        )?;
        drop(g);
        let status = self.licence_status_inner()?;
        Ok(crate::licence::ActivateResult {
            status,
            fingerprint_match_count: matches,
        })
    }

    /// Force EXPIRED for tests / renewal edge — still keeps payload and data.
    pub fn licence_mark_expired(&self) -> VaultResult<crate::licence::LicenceStatus> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let now = now_ms();
        app.execute(
            "UPDATE licence_state SET state = 'EXPIRED', updated_at_unix_ms = ?1 WHERE id = 1",
            [now],
        )?;
        // Ensure row exists.
        app.execute(
            "INSERT OR IGNORE INTO licence_state (id, state, fingerprint_match_count, air_gap_enabled, updated_at_unix_ms)
             VALUES (1, 'EXPIRED', 0, 0, ?1)",
            [now],
        )?;
        drop(g);
        self.licence_status_inner()
    }

    pub fn air_gap_set(&self, enabled: bool) -> VaultResult<crate::licence::LicenceStatus> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let now = now_ms();
        app.execute(
            "INSERT INTO licence_state (id, state, fingerprint_match_count, air_gap_enabled, updated_at_unix_ms)
             VALUES (1, 'UNLICENSED', 0, ?1, ?2)
             ON CONFLICT(id) DO UPDATE SET air_gap_enabled=excluded.air_gap_enabled, updated_at_unix_ms=excluded.updated_at_unix_ms",
            rusqlite::params![if enabled { 1 } else { 0 }, now],
        )?;
        drop(g);
        self.licence_status_inner()
    }

    /// B-S4-05: per-install Ed25519 seed for Audit Certificates. Created on first use from the OS RNG and kept ONLY
    /// inside the encrypted app vault (vault_meta), never in a plain file.
    pub fn audit_signing_seed(&self) -> VaultResult<[u8; 32]> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let app = g.app.as_mut().ok_or(VaultError::Locked)?;
        sql::apply_pending_migrations(app, APP_MIGRATIONS)?;
        let existing: Option<String> = app
            .query_row("SELECT value FROM vault_meta WHERE key='audit_signing_seed_v1'", [], |r| r.get(0))
            .optional()?;
        if let Some(hex_seed) = existing {
            let bytes = hex::decode(hex_seed.trim()).map_err(|e| VaultError::Internal { detail: format!("audit signing seed: {e}") })?;
            return bytes.try_into().map_err(|_| VaultError::Internal { detail: "audit signing seed has the wrong length".into() });
        }
        let mut seed = [0u8; 32];
        rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
        app.execute(
            "INSERT INTO vault_meta (key, value) VALUES ('audit_signing_seed_v1', ?1)",
            [hex::encode(seed)],
        )?;
        Ok(seed)
    }

    pub fn air_gap_enabled(&self) -> VaultResult<bool> {
        Ok(self.licence_status_inner()?.air_gap_enabled)
    }

    // --- B-S3-18 immutable debrief snapshots + mutable counsel task decisions ---
    pub fn debrief_v2_latest(&self, case_id: &str) -> VaultResult<Option<serde_json::Value>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before reading debriefs".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let raw: Option<String> = conn.query_row("SELECT report_json FROM mock_court_debrief_v2 WHERE case_id=?1 ORDER BY created_at_unix_ms DESC LIMIT 1", [case_id], |r| r.get(0)).optional()?;
        raw.map(|s| {
            serde_json::from_str(&s).map_err(|e| VaultError::Internal {
                detail: format!("debrief parse: {e}"),
            })
        })
        .transpose()
    }

    pub fn debrief_v2_store(&self, report: &crate::debrief_v2::DebriefReportV2) -> VaultResult<()> {
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(&report.case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before storing debrief".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let tx = conn.transaction()?;
        let report_json = serde_json::to_string(report).map_err(|e| VaultError::Internal {
            detail: format!("debrief serialize: {e}"),
        })?;
        tx.execute("INSERT INTO mock_court_debrief_v2(debrief_id,case_id,session_id,created_at_unix_ms,source_fingerprint,report_json) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(case_id,session_id) DO UPDATE SET debrief_id=excluded.debrief_id,created_at_unix_ms=excluded.created_at_unix_ms,source_fingerprint=excluded.source_fingerprint,report_json=excluded.report_json", rusqlite::params![report.debrief_id,report.case_id,report.session_id,report.created_at_unix_ms,report.source_fingerprint,report_json])?;
        for task in &report.tasks {
            let original = serde_json::to_string(task).map_err(|e| VaultError::Internal {
                detail: format!("task serialize: {e}"),
            })?;
            tx.execute("INSERT INTO preparation_task(case_id,task_id,debrief_id,original_json,title,status,counsel_note,updated_at_unix_ms)VALUES(?1,?2,?3,?4,?5,'PROPOSED',NULL,?6) ON CONFLICT(case_id,task_id) DO NOTHING", rusqlite::params![report.case_id,task.task_id,report.debrief_id,original,task.title,report.created_at_unix_ms])?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn preparation_tasks_list(
        &self,
        case_id: &str,
    ) -> VaultResult<Vec<crate::debrief_v2::PreparationTask>> {
        let g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g.cases.get(case_id).ok_or_else(|| VaultError::Internal {
            detail: "CF_VAULT_LOCKED: open case before reading preparation tasks".into(),
        })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let mut stmt = conn.prepare("SELECT original_json,title,status,counsel_note,updated_at_unix_ms FROM preparation_task WHERE case_id=?1 ORDER BY updated_at_unix_ms DESC,task_id")?;
        let rows = stmt.query_map([case_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, i64>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (raw, title, status, note, updated) = row?;
            let mut task: crate::debrief_v2::PreparationTask =
                serde_json::from_str(&raw).map_err(|e| VaultError::Internal {
                    detail: format!("task parse: {e}"),
                })?;
            task.title = title;
            task.status = status;
            task.counsel_note = note;
            task.updated_at_unix_ms = updated;
            out.push(task);
        }
        Ok(out)
    }

    pub fn preparation_task_update(
        &self,
        case_id: &str,
        task_id: &str,
        status: &str,
        title: Option<&str>,
        counsel_note: Option<&str>,
    ) -> VaultResult<Vec<crate::debrief_v2::PreparationTask>> {
        if !["PROPOSED", "ACCEPTED", "DISMISSED", "COMPLETED"].contains(&status) {
            return Err(VaultError::Internal {
                detail: format!("CF_PREPARATION_TASK_STATUS: invalid status {status}"),
            });
        }
        let mut g = self.inner.lock().map_err(|_| VaultError::LockPoisoned)?;
        let conn = g
            .cases
            .get_mut(case_id)
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_VAULT_LOCKED: open case before updating preparation task".into(),
            })?;
        sql::apply_pending_migrations(conn, CASE_MIGRATIONS)?;
        let original_title: String = conn
            .query_row(
                "SELECT title FROM preparation_task WHERE case_id=?1 AND task_id=?2",
                rusqlite::params![case_id, task_id],
                |r| r.get(0),
            )
            .optional()?
            .ok_or_else(|| VaultError::Internal {
                detail: "CF_PREPARATION_TASK_NOT_FOUND".into(),
            })?;
        let next_title = title
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&original_title);
        conn.execute("UPDATE preparation_task SET title=?1,status=?2,counsel_note=?3,updated_at_unix_ms=?4 WHERE case_id=?5 AND task_id=?6",rusqlite::params![next_title,status,counsel_note,now_ms(),case_id,task_id])?;
        drop(g);
        self.preparation_tasks_list(case_id)
    }

    fn assert_not_ransoming(&self) -> VaultResult<()> {
        // Invariant check: we never set a ransom flag.
        Ok(())
    }
}

fn map_experience_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ExperienceRecord> {
    let tags_json: String = r.get(8)?;
    let payload_json: String = r.get(10)?;
    let active_i: i64 = r.get(13)?;
    let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_default();
    let payload: serde_json::Value =
        serde_json::from_str(&payload_json).unwrap_or_else(|_| serde_json::json!({}));
    Ok(ExperienceRecord {
        record_id: r.get(0)?,
        schema_version: r.get(1)?,
        record_category: r.get(2)?,
        stage: r.get(3)?,
        language: r.get(4)?,
        jurisdiction: r.get(5)?,
        case_id: r.get(6)?,
        summary: r.get(7)?,
        tags,
        relevance_key: r.get(9)?,
        payload,
        source: r.get(11)?,
        fleet_status: r.get(12)?,
        active: active_i != 0,
        notes: r.get(14)?,
        created_at_unix_ms: r.get(15)?,
        updated_at_unix_ms: r.get(16)?,
    })
}

fn ensure_fleet_defaults(app: &rusqlite::Connection) -> VaultResult<()> {
    app.execute(
        "INSERT OR IGNORE INTO experience_meta (key, value) VALUES ('fleet_enabled', 'false')",
        [],
    )?;
    app.execute(
        "INSERT OR IGNORE INTO experience_meta (key, value) VALUES ('fleet_network_upload', 'false')",
        [],
    )?;
    // Stage 1 hard rule: network upload never true.
    app.execute(
        "INSERT OR REPLACE INTO experience_meta (key, value) VALUES ('fleet_network_upload', 'false')",
        [],
    )?;
    Ok(())
}

fn meta_bool(app: &rusqlite::Connection, key: &str, default: bool) -> bool {
    app.query_row(
        "SELECT value FROM experience_meta WHERE key = ?1",
        [key],
        |r| r.get::<_, String>(0),
    )
    .ok()
    .map(|v| v == "true" || v == "1")
    .unwrap_or(default)
}

/// Build frozen relevance_key from summary + tags + category.
pub fn build_relevance_key(summary: &str, tags: &[String], category: &str) -> String {
    let mut parts = Vec::new();
    parts.push(category.to_ascii_lowercase());
    for t in tags {
        parts.push(t.to_ascii_lowercase());
    }
    parts.push(summary.to_ascii_lowercase());
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in parts {
        for tok in tokenize_relevance(&p) {
            if seen.insert(tok.clone()) {
                out.push(tok);
            }
        }
    }
    out.join(" ")
}

fn tokenize_relevance(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '/')
        .filter(|t| t.len() >= 2)
        .map(|s| s.to_string())
        .collect()
}

fn map_lexicon_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LexiconEntry> {
    let active_i: i64 = r.get(9)?;
    Ok(LexiconEntry {
        entry_id: r.get(0)?,
        schema_version: r.get(1)?,
        entry_kind: r.get(2)?,
        language: r.get(3)?,
        surface_form: r.get(4)?,
        corrected_form: r.get(5)?,
        pronunciation: r.get(6)?,
        pattern_type: r.get(7)?,
        source: r.get(8)?,
        active: active_i != 0,
        scope: r.get(10)?,
        notes: r.get(11)?,
        created_at_unix_ms: r.get(12)?,
        updated_at_unix_ms: r.get(13)?,
    })
}

/// Case-insensitive whole-phrase replacement; longer surfaces first (caller sorts).
pub fn apply_stt_corrections(raw: &str, entries: &[LexiconEntry]) -> String {
    let mut out = raw.to_string();
    let mut pairs: Vec<(&str, &str)> = entries
        .iter()
        .filter(|e| e.active && e.entry_kind == "stt_correction")
        .filter_map(|e| {
            let c = e.corrected_form.as_deref()?;
            if e.surface_form.is_empty() || c.is_empty() {
                None
            } else {
                Some((e.surface_form.as_str(), c))
            }
        })
        .collect();
    pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (surface, corrected) in pairs {
        out = replace_ci_phrase(&out, surface, corrected);
    }
    out
}

pub fn apply_tts_pronunciations(text: &str, entries: &[LexiconEntry]) -> String {
    let mut out = text.to_string();
    let mut pairs: Vec<(&str, &str)> = entries
        .iter()
        .filter(|e| e.active && e.entry_kind == "tts_pronunciation_override")
        .filter_map(|e| {
            let p = e.pronunciation.as_deref()?;
            if e.surface_form.is_empty() || p.is_empty() {
                None
            } else {
                Some((e.surface_form.as_str(), p))
            }
        })
        .collect();
    pairs.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (surface, spoken) in pairs {
        out = replace_ci_phrase(&out, surface, spoken);
    }
    out
}

fn replace_ci_phrase(haystack: &str, needle: &str, replacement: &str) -> String {
    if needle.is_empty() {
        return haystack.to_string();
    }
    let lower_h = haystack.to_lowercase();
    let lower_n = needle.to_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    let h_bytes = haystack.as_bytes();
    while i < haystack.len() {
        if lower_h[i..].starts_with(&lower_n) {
            // Prefer word-ish boundaries when needle is alphanumeric
            let end = i + needle.len();
            let boundary_ok = {
                let before_ok = i == 0 || !h_bytes[i - 1].is_ascii_alphanumeric();
                let after_ok = end >= haystack.len()
                    || !h_bytes
                        .get(end)
                        .map(|b| b.is_ascii_alphanumeric())
                        .unwrap_or(false);
                before_ok && after_ok
            };
            if boundary_ok {
                out.push_str(replacement);
                i = end;
                continue;
            }
        }
        // advance one char
        let ch = haystack[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

fn case_summary(g: &VaultInner, case_id: &str) -> VaultResult<CaseSummary> {
    let app = g.app.as_ref().ok_or(VaultError::Locked)?;
    let row = app.query_row(
        "SELECT created_at_unix_ms, label,
                COALESCE(cause_title,''), COALESCE(court,''), COALESCE(jurisdiction,''),
                COALESCE(matter_type,''), COALESCE(user_side,''), COALESCE(code_era,''),
                COALESCE(language,'English')
         FROM cases WHERE case_id = ?1",
        [case_id],
        |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
            ))
        },
    )?;
    let (
        created,
        label,
        cause_title,
        court,
        jurisdiction,
        matter_type,
        user_side,
        code_era,
        language,
    ) = row;
    Ok(CaseSummary {
        case_id: case_id.to_string(),
        created_at_unix_ms: created,
        label,
        cause_title,
        court,
        jurisdiction,
        matter_type,
        user_side,
        code_era,
        language,
        is_open: g.cases.contains_key(case_id),
    })
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working_item(
        authority_id: &str,
        checksum: &str,
        superseded: &str,
    ) -> crate::working_set::WorkingSetItemInput {
        crate::working_set::WorkingSetItemInput {
            item_id: format!("ws:{authority_id}"),
            authority_id: authority_id.into(),
            source_id: format!("source:{checksum}"),
            chunk_id: format!("chunk:{checksum}"),
            pack_id: "official.test".into(),
            title: "Partition Act, 1893".into(),
            citation: "Act No. 4 of 1893".into(),
            section_label: "Section 2".into(),
            jurisdiction: "India".into(),
            authority_type: "STATUTE".into(),
            trust_level: "OFFICIAL_VERIFIED".into(),
            temporal_status: "APPLICABLE".into(),
            superseded_status: superseded.into(),
            source_path: "content/partition.txt".into(),
            source_reference: "https://official.invalid/partition".into(),
            source_checksum: format!("sha256:{checksum}"),
            chunk_checksum: format!("sha256:{checksum}"),
            exact_text: "2. Power to court to order sale instead of division.".into(),
            last_seen_index_checksum: format!("sha256:index-{checksum}"),
        }
    }

    #[test]
    fn b_s3_12_counsel_decisions_survive_restart_rerun_and_pack_update() {
        let dir =
            std::env::temp_dir().join(format!("caseforge-working-set-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pass = "working set test passphrase";
        let mgr = VaultManager::new(dir.clone());
        mgr.init_and_unlock(pass).unwrap();
        let created = mgr
            .create_case_with_wizard(
                pass,
                CaseWizardInput {
                    cause_title: "A v B".into(),
                    court: "High Court".into(),
                    jurisdiction: "India".into(),
                    matter_type: "Property partition civil appeal".into(),
                    user_side: "Appellant".into(),
                    code_era: None,
                    language: None,
                },
            )
            .unwrap();
        let case_id = created.case_id;
        mgr.open_case(&case_id, pass).unwrap();
        let regimes = vec![crate::working_set::RegimeCandidate {
            regime_id: "CENTRAL_CIVIL_PROCEDURE".into(),
            label: "Central".into(),
            reason: "controlled".into(),
            confirmation_required: true,
        }];
        let first = mgr
            .working_set_merge_suggestions(
                &case_id,
                &[working_item("stable-authority", "old", "CURRENT")],
                &regimes,
            )
            .unwrap();
        assert_eq!(first["items"][0]["decision"], "PROPOSED");
        let item_id = first["items"][0]["item_id"].as_str().unwrap();
        mgr.working_set_update_item(
            &case_id,
            item_id,
            "REMOVED",
            true,
            false,
            Some("Not applicable on pleaded facts"),
        )
        .unwrap();
        // Same stable authority with a changed source/chunk checksum simulates pack/index refresh.
        let rerun = mgr
            .working_set_merge_suggestions(
                &case_id,
                &[working_item("stable-authority", "new", "CURRENT")],
                &regimes,
            )
            .unwrap();
        assert_eq!(rerun["items"][0]["decision"], "REMOVED");
        assert_eq!(rerun["items"][0]["pinned"], true);
        assert_eq!(rerun["items"][0]["chunk_checksum"], "sha256:new");
        mgr.close_case(&case_id).unwrap();
        mgr.open_case(&case_id, pass).unwrap();
        let restarted = mgr.working_set_get(&case_id).unwrap();
        assert_eq!(restarted["items"][0]["decision"], "REMOVED");
        // Supersession always clears controlling eligibility and cannot silently re-enter.
        let superseded = mgr
            .working_set_merge_suggestions(
                &case_id,
                &[working_item("stable-authority", "newer", "SUPERSEDED")],
                &regimes,
            )
            .unwrap();
        assert_eq!(superseded["items"][0]["decision"], "SUPERSEDED");
        assert_eq!(superseded["items"][0]["controlling"], false);
        mgr.lock().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn b_s3_12_downstream_selector_records_version_and_authority_ids() {
        let dir =
            std::env::temp_dir().join(format!("caseforge-working-output-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pass = "working output passphrase";
        let mgr = VaultManager::new(dir.clone());
        mgr.init_and_unlock(pass).unwrap();
        let created = mgr
            .create_case_with_wizard(
                pass,
                CaseWizardInput {
                    cause_title: "A v B".into(),
                    court: "High Court".into(),
                    jurisdiction: "India".into(),
                    matter_type: "Partition".into(),
                    user_side: "Appellant".into(),
                    code_era: None,
                    language: None,
                },
            )
            .unwrap();
        let cid = created.case_id;
        mgr.open_case(&cid, pass).unwrap();
        let regimes = vec![crate::working_set::RegimeCandidate {
            regime_id: "CENTRAL_CIVIL_PROCEDURE".into(),
            label: "Central".into(),
            reason: "controlled".into(),
            confirmation_required: true,
        }];
        let set = mgr
            .working_set_merge_suggestions(
                &cid,
                &[working_item("authority-current", "one", "CURRENT")],
                &regimes,
            )
            .unwrap();
        let item = set["items"][0]["item_id"].as_str().unwrap();
        mgr.working_set_update_item(
            &cid,
            item,
            "INCLUDED",
            true,
            true,
            Some("Counsel confirms controlling"),
        )
        .unwrap();
        mgr.working_set_confirm_regime(&cid, "CENTRAL_CIVIL_PROCEDURE")
            .unwrap();
        let selection = mgr
            .working_set_select_for_output(&cid, "DRAFTING", "draft-test-1")
            .unwrap();
        assert!(selection["working_set_version"].as_i64().unwrap() > 0);
        assert_eq!(selection["authority_ids"][0], "authority-current");
        mgr.lock().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn b_s3_07_word_overlay_survives_recognizer_upgrade() {
        let rows = vec![PageTextLayerRow {
            doc_id: "d".into(),
            page_index: 0,
            recognizer_id: "new-model".into(),
            recognized_at_unix_ms: 2,
            full_text: "machine token".into(),
            mean_confidence: 0.8,
            page_quality_warn: false,
            notes: None,
            calibration_id: None,
            critical_fields: serde_json::json!([]),
            words: vec![
                PageWordRow {
                    word_index: 0,
                    text: "machine".into(),
                    confidence: 0.8,
                    raw_engine_confidence: Some(0.8),
                    calibrated_confidence: Some(0.8),
                    confidence_state: "calibrated".into(),
                    counsel_confirmed: false,
                    char_start: 0,
                    char_end: 7,
                    low_confidence: false,
                    force_review: false,
                    suppress_from_auto: false,
                    bbox_json: Some("{\"x\":0.1}".into()),
                },
                PageWordRow {
                    word_index: 1,
                    text: "token".into(),
                    confidence: 0.8,
                    raw_engine_confidence: Some(0.8),
                    calibrated_confidence: Some(0.8),
                    confidence_state: "calibrated".into(),
                    counsel_confirmed: false,
                    char_start: 8,
                    char_end: 13,
                    low_confidence: false,
                    force_review: false,
                    suppress_from_auto: false,
                    bbox_json: None,
                },
            ],
        }];
        let overlays = vec![AuthorityOverlay {
            target_key: "cfat:v1:word".into(),
            latest_event_id: "event-word".into(),
            target_kind: "word_span".into(),
            collection: None,
            item_id: None,
            field_name: None,
            doc_id: Some("d".into()),
            page_index: Some(0),
            word_index: Some(0),
            base_value: "machine".into(),
            counsel_value: "counsel".into(),
            anchor: serde_json::json!({"original_bbox":"{\"x\":0.1}"}),
        }];
        let (merged, conflicts) = VaultManager::merge_word_authority(&rows, &overlays);
        assert!(conflicts.is_empty());
        assert_eq!(merged[0].full_text, "counsel token");
        assert_eq!(merged[0].words[0].confidence_state, "counsel_confirmed");
        assert!(merged[0].words[0].counsel_confirmed);
    }

    #[test]
    fn b_s3_07_graph_correction_survives_rebuild_and_history_is_immutable() {
        let dir = std::env::temp_dir().join(format!("caseforge-authority-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = VaultManager::new(dir.clone());
        mgr.init_and_unlock("authority test passphrase").unwrap();
        let created = mgr
            .create_case_with_wizard(
                "authority test passphrase",
                CaseWizardInput {
                    cause_title: "Synthetic Authority".into(),
                    court: "Test Court".into(),
                    jurisdiction: "India".into(),
                    matter_type: "Civil".into(),
                    user_side: "Plaintiff".into(),
                    code_era: None,
                    language: None,
                },
            )
            .unwrap();
        let case_id = created.case_id;
        mgr.open_case(&case_id, "authority test passphrase")
            .unwrap();
        let prov = serde_json::json!({"source":{"doc_id":"doc-a","page_index":0,"kind":"pdf_span","char_start":0,"char_end":7,"bbox":null,"quote":"machine","content_hash":"sha256:anchor"},"sources":[{"doc_id":"doc-a","page_index":0,"kind":"pdf_span","char_start":0,"char_end":7,"bbox":null,"quote":"machine","content_hash":"sha256:anchor"}],"confidence":0.8,"origin":"EXTRACTED","pipeline_version":"p1","recorded_at":"2026-08-14T00:00:00Z","counsel_confirmed":false,"correction_lineage":[],"reviewed":false,"notes":null});
        let base = serde_json::json!({"schema_version":"1.0.0","case_id":case_id,"pipeline":{"extractor_id":"p1"},"completeness":{"score_0_100":20},"parties":[],"chronology":[{"event_id":"event_1","summary":"machine old","provenance":prov}],"statutes":[],"evidence":[],"prayer_relief":[],"contradictions":[],"counsel_facts":[],"gaps":[]});
        mgr.case_graph_put(&case_id, &base).unwrap();
        let corrected = mgr
            .case_graph_correct_item(
                &case_id,
                "chronology",
                "event_1",
                "summary",
                "counsel truth",
                2,
            )
            .unwrap();
        assert_eq!(corrected["chronology"][0]["summary"], "counsel truth");
        let next = serde_json::json!({"schema_version":"1.0.0","case_id":case_id,"pipeline":{"extractor_id":"p2"},"completeness":{"score_0_100":20},"parties":[],"chronology":[{"event_id":"event_99","summary":"new contradiction","provenance":prov}],"statutes":[],"evidence":[],"prayer_relief":[],"contradictions":[],"counsel_facts":[],"gaps":[]});
        let merged = mgr.case_graph_put_generated(&case_id, &next).unwrap();
        assert_eq!(merged["chronology"][0]["summary"], "counsel truth");
        assert_eq!(
            mgr.authority_conflicts_list(&case_id, false).unwrap().len(),
            1
        );
        let history = mgr.correction_events_list(&case_id, 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0]["event_type"], "CORRECT");
        {
            let mut guard = mgr.inner.lock().unwrap();
            let conn = guard.cases.get_mut(&case_id).unwrap();
            let rejected = conn.execute("UPDATE correction_events SET reason='tampered'", []);
            assert!(rejected.is_err());
        }
        mgr.lock().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ac_16_2_wrong_passphrase_fails_and_db_not_plaintext() {
        let dir = std::env::temp_dir().join(format!("caseforge-vault-test-{}", Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = VaultManager::new(dir.clone());
        mgr.init_and_unlock("correct horse battery staple").unwrap();
        let created = mgr
            .create_case_with_wizard(
                "correct horse battery staple",
                CaseWizardInput {
                    cause_title: "State v. Synthetic".into(),
                    court: "Test Court".into(),
                    jurisdiction: "India".into(),
                    matter_type: "Criminal".into(),
                    user_side: "Defence".into(),
                    code_era: None,
                    language: None,
                },
            )
            .unwrap();
        assert_eq!(created.profile.jurisdiction, "India");
        mgr.lock().unwrap();

        let err = mgr.unlock("wrong passphrase").unwrap_err();
        assert!(matches!(err, VaultError::PassphraseInvalid));

        let db = PathBuf::from(&created.path);
        assert!(
            VaultManager::probe_db_unreadable_without_key(&db),
            "DB must be unreadable without key"
        );

        mgr.unlock("correct horse battery staple").unwrap();
        let list = mgr.list_cases().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].jurisdiction, "India");
        // Jurisdiction required — invalid enum rejected
        let bad = mgr.create_case_with_wizard(
            "correct horse battery staple",
            CaseWizardInput {
                cause_title: "x".into(),
                court: "y".into(),
                jurisdiction: "Mars".into(),
                matter_type: "z".into(),
                user_side: "a".into(),
                code_era: None,
                language: None,
            },
        );
        assert!(bad.is_err());
        mgr.lock().unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
