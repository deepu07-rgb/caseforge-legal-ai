//! CaseForge shell core (B-S1-02+).
//! Host IPC: shell_* health + vault_* / case_* / doc_* / docpipe_* / llm_* .
//! B-S1-05/06: docpipe triage + recognition (word confidence from contracts).
//! Local infer-llm: Ollama / GGUF / OpenAI-compat offline reasoner.

mod adaptive_bench;
mod answer_feedback;
mod audit;
mod certificate;
mod authority;
mod bench_prep;
mod choreography;
mod companion;
mod debrief_v2;
mod docpipe;
mod grounded_mock_court;
mod guardrails;
mod infer_llm;
mod infer_speech;
mod installer;
mod integrity;
mod licence;
mod mock_court;
mod mock_court_retrieval;
mod opposition;
mod witness;
mod impeach;
mod packs;
mod proposition;
mod protection;
mod retrieval;
mod vault;
mod voice_court;
mod working_set;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;
use vault::{
    CaseCreateResult, CaseSummary, CaseWizardInput, DocTextPreview, DocumentIngestRequest,
    DocumentMeta, PageTriageRow, VaultError, VaultManager, VaultStatus, JURISDICTIONS,
};

static PROCESS_START: Lazy<Instant> = Lazy::new(Instant::now);
static PROCESS_START_UNIX_MS: Lazy<u64> = Lazy::new(|| {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
});

const PRODUCT_VERSION: &str = env!("CARGO_PKG_VERSION");
const BATCH: &str = "B-S2-07";

/// Lazy vault manager rooted at OS app-data (initialized on first vault command).
struct VaultState {
    manager: Mutex<Option<Arc<VaultManager>>>,
}

impl VaultState {
    fn new() -> Self {
        Self {
            manager: Mutex::new(None),
        }
    }

    fn get_or_init(&self, app: &AppHandle) -> Result<Arc<VaultManager>, String> {
        let mut g = self
            .manager
            .lock()
            .map_err(|_| "CF_SYSTEM_INTERNAL: vault state lock".to_string())?;
        if let Some(m) = g.as_ref() {
            return Ok(m.clone());
        }
        let dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("CF_SYSTEM_INTERNAL: app_data_dir: {e}"))?
            .join("vaults");
        std::fs::create_dir_all(&dir).map_err(|e| format!("CF_VAULT_WRITE_FAILED: {e}"))?;
        let mgr = Arc::new(VaultManager::new(dir));
        *g = Some(mgr.clone());
        Ok(mgr)
    }
}

#[derive(Clone, Serialize)]
pub struct PingResult {
    pub ok: bool,
    pub pong: String,
    pub server_time_ms: u64,
}

#[derive(Clone, Serialize)]
pub struct VersionResult {
    pub product: String,
    pub version: String,
    pub batch: String,
    pub tauri: String,
    pub uptime_ms: u64,
    pub process_start_unix_ms: u64,
}

#[derive(Clone, Serialize)]
pub struct HeavyJobStartResult {
    pub job_id: String,
    pub accepted: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HeavyJobState {
    Running,
    Completed,
    Failed,
    Unknown,
}

#[derive(Clone, Serialize)]
pub struct HeavyJobStatus {
    pub job_id: String,
    pub state: HeavyJobState,
    pub progress_percent: u32,
    pub work_elapsed_ms: u64,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct HeavyJobProgressEvent {
    pub job_id: String,
    pub progress_percent: u32,
    pub message: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SidecarName {
    #[serde(rename = "infer-llm")]
    InferLlm,
    #[serde(rename = "infer-speech")]
    InferSpeech,
    #[serde(rename = "docpipe")]
    Docpipe,
}

impl SidecarName {
    fn as_str(&self) -> &'static str {
        match self {
            SidecarName::InferLlm => "infer-llm",
            SidecarName::InferSpeech => "infer-speech",
            SidecarName::Docpipe => "docpipe",
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SidecarState {
    Idle,
    SpawnedStub,
    Error,
}

#[derive(Clone, Serialize)]
pub struct SidecarStatusEntry {
    pub name: String,
    pub state: SidecarState,
    pub detail: String,
}

#[derive(Clone, Serialize)]
pub struct SidecarStatusResult {
    pub sidecars: Vec<SidecarStatusEntry>,
}

struct JobRecord {
    state: HeavyJobState,
    progress_percent: u32,
    work_elapsed_ms: u64,
    message: String,
    started: Instant,
}

struct AppState {
    jobs: Mutex<HashMap<String, JobRecord>>,
    /// Monotonic counter for diagnostics.
    heavy_jobs_started: AtomicU64,
}

impl AppState {
    fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            heavy_jobs_started: AtomicU64::new(0),
        }
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[tauri::command]
fn shell_ping() -> PingResult {
    PingResult {
        ok: true,
        pong: "pong".into(),
        server_time_ms: now_unix_ms(),
    }
}

#[tauri::command]
fn shell_version() -> VersionResult {
    VersionResult {
        product: "CaseForge".into(),
        version: PRODUCT_VERSION.into(),
        batch: BATCH.into(),
        tauri: tauri::VERSION.into(),
        uptime_ms: PROCESS_START.elapsed().as_millis() as u64,
        process_start_unix_ms: *PROCESS_START_UNIX_MS,
    }
}

#[tauri::command]
fn shell_sidecar_status() -> SidecarStatusResult {
    let (llm_ready, llm_detail) = infer_llm::status_brief();
    SidecarStatusResult {
        sidecars: vec![
            SidecarStatusEntry {
                name: SidecarName::InferLlm.as_str().into(),
                state: if llm_ready {
                    SidecarState::SpawnedStub // "ready" mapped; no separate Ready enum historically
                } else {
                    SidecarState::Idle
                },
                detail: if llm_ready {
                    format!("local reasoner ready — {llm_detail}")
                } else {
                    format!("local reasoner offline — {llm_detail}")
                },
            },
            {
                let (speech_ready, speech_detail) = infer_speech::status_brief();
                SidecarStatusEntry {
                    name: SidecarName::InferSpeech.as_str().into(),
                    state: if speech_ready {
                        SidecarState::SpawnedStub
                    } else {
                        SidecarState::Idle
                    },
                    detail: if speech_ready {
                        format!("speech ready — {speech_detail}")
                    } else {
                        format!("speech offline — {speech_detail}")
                    },
                }
            },
            SidecarStatusEntry {
                name: SidecarName::Docpipe.as_str().into(),
                state: SidecarState::Idle,
                detail: format!(
                    "triage+recognize ready ({} / {})",
                    docpipe::CLASSIFIER_ID,
                    docpipe::RECOGNIZER_ID
                ),
            },
        ],
    }
}

/// B-S4-04: audit every citation and quotation in a draft against the installed legal packs (deterministic).
#[tauri::command]
fn audit_draft(
    app: AppHandle,
    vault: State<'_, VaultState>,
    text: String,
    filename: Option<String>,
    case_id: Option<String>,
) -> Result<audit::AuditReport, String> {
    if text.trim().is_empty() {
        return Err("CF_AUDIT_EXTRACTION_FAILED: the draft is empty".into());
    }
    let mgr = vault.get_or_init(&app)?;
    let corpus = retrieval::build_corpus(&mgr.vault_paths().map_err(|e| e.to_string())?, &[])?;
    Ok(audit::audit(&text, &corpus, filename.as_deref().unwrap_or("draft.txt"), "drafting_surface", case_id))
}

/// B-S4-05: sign an Audit Certificate for an audit report and the exact draft text it was run on.
#[tauri::command]
fn audit_certificate_issue(
    app: AppHandle,
    vault: State<'_, VaultState>,
    report: audit::AuditReport,
    draft_text: String,
    disclosure_text: Option<String>,
    disclosure_confirmed: Option<bool>,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let seed = mgr.audit_signing_seed().map_err(|e| e.to_string())?;
    let disclosure = disclosure_text.map(|text| certificate::Disclosure { text, template_id: None, confirmed: disclosure_confirmed.unwrap_or(false) });
    certificate::issue(&report, &draft_text, &seed, env!("CARGO_PKG_VERSION"), disclosure, &crate::docpipe::now_rfc3339())
}

/// B-S4-05: verify a certificate offline (no vault, no network). `draft_text` optional.
#[tauri::command]
fn audit_certificate_verify(certificate_json: String, draft_text: Option<String>) -> Result<certificate::VerifyOutcome, String> {
    let cert: serde_json::Value = serde_json::from_str(&certificate_json).map_err(|e| format!("CF_AUDIT_CERT_SIGNATURE_INVALID: not a certificate JSON ({e})"))?;
    Ok(certificate::verify(&cert, draft_text.as_deref()))
}

/// B-S4-06: Opposing Filing Scan. Scans pasted text, or a document tagged EXTERNAL_FILING, against the case record.
/// Read-only: nothing from the filing is written to the case graph.
#[tauri::command]
fn opposition_scan(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    filing_party_id: String,
    filing_kind: String,
    doc_id: Option<String>,
    text: Option<String>,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let (doc_id, filename, pages) = match (doc_id, text) {
        (Some(id), _) => {
            let (name, pages) = crate::docpipe::external_filing_pages(&mgr, &case_id, &id)?;
            (id, name, pages)
        }
        (None, Some(t)) if !t.trim().is_empty() => ("pasted-filing".to_string(), "pasted_filing.txt".to_string(), vec![t]),
        _ => return Err("CF_AUDIT_EXTRACTION_FAILED: paste the opponent's filing or pick a document tagged EXTERNAL_FILING".into()),
    };
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let stale_graph = opposition::graph_cites_doc(&graph, &doc_id);
    let tree = mgr.argument_tree_get(&case_id).map_err(|e| e.to_string())?;
    let corpus = retrieval::build_corpus(&mgr.vault_paths().map_err(|e| e.to_string())?, &[])?;
    let (record, report) = opposition::scan(opposition::ScanInput {
        doc_id: &doc_id,
        filing_party_id: &filing_party_id,
        filing_kind: &filing_kind,
        filename: &filename,
        pages: &pages,
        graph: &graph,
        argument_tree: tree.as_ref(),
        corpus: &corpus,
        case_id: Some(case_id),
    });
    Ok(serde_json::json!({"pipeline_id": opposition::PIPELINE_ID, "stale_graph": stale_graph, "filing": record, "audit": report}))
}

// --- B-S4-07/08 Witness Statement Matrix and Impeachment Kit ---

fn case_jurisdiction(mgr: &VaultManager, case_id: &str) -> Result<String, String> {
    mgr.list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.case_id == case_id)
        .map(|c| c.jurisdiction)
        .ok_or_else(|| "CF_CASE_NOT_FOUND: matter is not registered".to_string())
}

fn witness_register(mgr: &VaultManager, case_id: &str) -> Result<Vec<witness::RegisterEntry>, String> {
    Ok(mgr
        .witness_kv_list(case_id, "register:")
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|(_, v)| serde_json::from_value(v).ok())
        .collect())
}

/// Active counsel decisions in the order they were made, with their storage keys.
fn witness_decisions(mgr: &VaultManager, case_id: &str) -> Result<Vec<(String, witness::Decision, bool)>, String> {
    Ok(mgr
        .witness_kv_list(case_id, "decision:")
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|(k, v)| {
            let d: witness::Decision = serde_json::from_value(v.get("decision")?.clone()).ok()?;
            Some((k, d, v.get("active").and_then(|a| a.as_bool()).unwrap_or(true)))
        })
        .collect())
}

/// Proposes register rows from statement headers. Counsel-confirmed rows are never overwritten (survive rerun).
#[tauri::command]
fn witness_propose(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_ids: Option<Vec<String>>) -> Result<Vec<witness::RegisterEntry>, String> {
    let mgr = vault.get_or_init(&app)?;
    let jurisdiction = case_jurisdiction(&mgr, &case_id)?;
    let existing: HashMap<String, witness::RegisterEntry> = witness_register(&mgr, &case_id)?.into_iter().map(|e| (e.doc_id.clone(), e)).collect();
    let explicit = doc_ids.is_some();
    let ids: Vec<String> = match doc_ids {
        Some(ids) => ids,
        None => mgr.doc_list(&case_id).map_err(|e| e.to_string())?.into_iter().map(|d| d.doc_id).collect(),
    };
    for id in ids {
        let (_meta, pages) = docpipe::statement_pages(&mgr, &case_id, &id)?;
        let doc = witness::StatementDoc { doc_id: id.clone(), pages };
        let proposal = match witness::propose(&doc, &jurisdiction) {
            Some(p) => p,
            // Explicitly chosen by counsel but unrecognised: register it for counsel to fill in.
            None if explicit => witness::RegisterEntry {
                doc_id: id.clone(),
                witness_name: "Unnamed witness".into(),
                relation: None,
                statement_kind: "OTHER".into(),
                recorded_on: None,
                locator_kind: if doc.pages.iter().any(|p| witness::is_transcript_page(p)) { "PAGE_LINE".into() } else { "PAGE_SPAN".into() },
                sequence_override: None,
                origin: "PROPOSED".into(),
            },
            None => continue,
        };
        let row = witness::merge_proposal(existing.get(&id), proposal);
        mgr.witness_kv_put(&case_id, &format!("register:{id}"), &serde_json::to_value(&row).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    witness_register(&mgr, &case_id)
}

#[tauri::command]
fn witness_register_list(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let kinds: Vec<serde_json::Value> = witness::VOCAB.kinds().iter().map(|k| serde_json::json!({"kind": k.kind, "label": k.label, "jurisdiction": k.jurisdiction})).collect();
    let decisions: Vec<serde_json::Value> = witness_decisions(&mgr, &case_id)?.into_iter().map(|(key, d, active)| serde_json::json!({"key": key, "decision": d, "active": active})).collect();
    Ok(serde_json::json!({"register": witness_register(&mgr, &case_id)?, "decisions": decisions, "statement_kinds": kinds,
                          "vocabulary": {"pack_id": witness::VOCAB.pack_id, "table_id": witness::VOCAB.table_id}}))
}

/// Counsel edits or confirms a register row (origin becomes COUNSEL_CONFIRMED).
#[tauri::command]
fn witness_register_save(app: AppHandle, vault: State<'_, VaultState>, case_id: String, mut entry: witness::RegisterEntry) -> Result<witness::RegisterEntry, String> {
    let mgr = vault.get_or_init(&app)?;
    if entry.witness_name.trim().is_empty() {
        return Err("CF_WITNESS_NAME_REQUIRED: enter the witness's name".into());
    }
    if entry.statement_kind != "OTHER" && witness::VOCAB.kind(&entry.statement_kind).is_none() {
        return Err(format!("CF_WITNESS_KIND_UNKNOWN: {} is not in the statement-kind vocabulary", entry.statement_kind));
    }
    if !matches!(entry.locator_kind.as_str(), "PAGE_SPAN" | "PAGE_LINE") {
        return Err("CF_WITNESS_LOCATOR_KIND: PAGE_SPAN or PAGE_LINE".into());
    }
    entry.origin = "COUNSEL_CONFIRMED".into();
    mgr.witness_kv_put(&case_id, &format!("register:{}", entry.doc_id), &serde_json::to_value(&entry).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    Ok(entry)
}

#[tauri::command]
fn witness_register_remove(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_id: String) -> Result<(), String> {
    vault.get_or_init(&app)?.witness_kv_delete(&case_id, &format!("register:{doc_id}")).map_err(|e| e.to_string())
}

/// Counsel merge/split. Append-only: a decision is revoked (kept in history), never deleted.
#[tauri::command]
fn witness_decision_add(app: AppHandle, vault: State<'_, VaultState>, case_id: String, decision: witness::Decision) -> Result<String, String> {
    let mgr = vault.get_or_init(&app)?;
    let n = mgr.witness_kv_list(&case_id, "decision:").map_err(|e| e.to_string())?.len();
    let key = format!("decision:{n:06}");
    mgr.witness_kv_put(&case_id, &key, &serde_json::json!({"decision": decision, "active": true, "created_at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())?;
    Ok(key)
}

#[tauri::command]
fn witness_decision_revoke(app: AppHandle, vault: State<'_, VaultState>, case_id: String, key: String) -> Result<(), String> {
    let mgr = vault.get_or_init(&app)?;
    let (_, mut value) = mgr
        .witness_kv_list(&case_id, &key)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|(k, _)| *k == key)
        .ok_or_else(|| "CF_WITNESS_DECISION_NOT_FOUND".to_string())?;
    value["active"] = serde_json::json!(false);
    value["revoked_at"] = serde_json::json!(crate::docpipe::now_rfc3339());
    mgr.witness_kv_put(&case_id, &key, &value).map_err(|e| e.to_string())
}

/// Builds the Statement Matrix and one Impeachment Kit per witness from the register, counsel decisions,
/// counsel finding status and any stored (re-verified) model candidates. Deterministic.
#[tauri::command]
fn witness_matrix_build(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let us = witness::is_us(&case_jurisdiction(&mgr, &case_id)?);
    let register = witness_register(&mgr, &case_id)?;
    let decisions: Vec<witness::Decision> = witness_decisions(&mgr, &case_id)?.into_iter().filter(|d| d.2).map(|d| d.1).collect();
    let (witnesses, stale) = witness::resolve(&register, &decisions);
    let mut docs = HashMap::new();
    let mut unreadable = Vec::new();
    for e in &register {
        match docpipe::statement_pages(&mgr, &case_id, &e.doc_id) {
            Ok((_, pages)) => {
                docs.insert(e.doc_id.clone(), witness::StatementDoc { doc_id: e.doc_id.clone(), pages });
            }
            Err(err) => unreadable.push(serde_json::json!({"doc_id": e.doc_id, "error": err})),
        }
    }
    let built: Vec<witness::BuiltWitness> = witnesses.iter().map(|w| witness::build_witness(w, &docs, us)).collect();
    let graph_version = mgr
        .case_graph_get(&case_id)
        .map_err(|e| e.to_string())?
        .and_then(|g| g.get("generated_at").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_else(|| "no-graph".into());
    let now = crate::docpipe::now_rfc3339();
    let matrix = witness::matrix_json(&case_id, &graph_version, &now, &built);
    let matrix_id = matrix["matrix_id"].as_str().unwrap_or("").to_string();
    let statuses: HashMap<String, String> = mgr
        .witness_kv_list(&case_id, "finding:")
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|(k, v)| Some((k.trim_start_matches("finding:").to_string(), v.get("status")?.as_str()?.to_string())))
        .collect();
    let models: HashMap<String, serde_json::Value> = mgr.witness_kv_list(&case_id, "model:").map_err(|e| e.to_string())?.into_iter().map(|(k, v)| (k.trim_start_matches("model:").to_string(), v)).collect();
    let kits: Vec<serde_json::Value> = built
        .iter()
        .map(|b| {
            let mut findings = impeach::detect(b);
            let mut fingerprint = None;
            if let Some(stored) = models.get(&b.witness.witness_id) {
                let props: Vec<proposition::PropositionInput> = serde_json::from_value(stored["propositions"].clone()).unwrap_or_default();
                let (semantic, _) = impeach::verify_model_candidates(b, &props, &findings);
                if !semantic.is_empty() {
                    fingerprint = stored["model_fingerprint"].as_str().map(str::to_string);
                }
                findings.extend(semantic);
            }
            impeach::kit(&case_id, &matrix_id, &now, b, findings, &statuses, fingerprint)
        })
        .collect();
    Ok(serde_json::json!({"pipeline_id": impeach::PIPELINE_ID, "matrix_pipeline_id": witness::PIPELINE_ID, "matrix": matrix, "kits": kits,
                          "stale_decisions": stale, "unreadable": unreadable}))
}

#[tauri::command]
fn impeach_finding_status(app: AppHandle, vault: State<'_, VaultState>, case_id: String, finding_id: String, status: String) -> Result<(), String> {
    if !matches!(status.as_str(), "PROPOSED" | "COUNSEL_ACCEPTED" | "COUNSEL_REJECTED") {
        return Err("CF_IMPEACH_STATUS: PROPOSED, COUNSEL_ACCEPTED or COUNSEL_REJECTED".into());
    }
    vault
        .get_or_init(&app)?
        .witness_kv_put(&case_id, &format!("finding:{finding_id}"), &serde_json::json!({"status": status, "at": crate::docpipe::now_rfc3339()}))
        .map_err(|e| e.to_string())
}

/// B-S4-08 model layer: asks the local model for meaning-level contradictions. Only candidates whose quotes verify
/// exactly against two different statements render; the rest are counted and discarded (AC-S4-03.3).
#[tauri::command]
fn impeach_model_scan(app: AppHandle, vault: State<'_, VaultState>, case_id: String, witness_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let us = witness::is_us(&case_jurisdiction(&mgr, &case_id)?);
    let register = witness_register(&mgr, &case_id)?;
    let decisions: Vec<witness::Decision> = witness_decisions(&mgr, &case_id)?.into_iter().filter(|d| d.2).map(|d| d.1).collect();
    let (witnesses, _) = witness::resolve(&register, &decisions);
    let w = witnesses.into_iter().find(|w| w.witness_id == witness_id).ok_or_else(|| "CF_WITNESS_NOT_FOUND".to_string())?;
    let mut docs = HashMap::new();
    for s in &w.statements {
        let (_, pages) = docpipe::statement_pages(&mgr, &case_id, &s.doc_id)?;
        docs.insert(s.doc_id.clone(), witness::StatementDoc { doc_id: s.doc_id.clone(), pages });
    }
    let built = witness::build_witness(&w, &docs, us);
    let (user_text, sources) = impeach::model_request(&built)?;
    let request: infer_llm::LlmChatRequest = serde_json::from_value(serde_json::json!({
        "user_text": user_text, "role": "counsel", "format": "impeachment_contradictions", "structured_propositions": true,
        "output_kind": "REASONING", "support_sources": sources, "max_tokens": 1024
    }))
    .map_err(|e| e.to_string())?;
    let result = infer_llm::chat_with_timeout(request, 300)?;
    let existing = impeach::detect(&built);
    let (rendered, discarded) = impeach::verify_model_candidates(&built, &result.propositions, &existing);
    let fingerprint = format!("{} ({})", result.model, result.backend);
    mgr.witness_kv_put(&case_id, &format!("model:{witness_id}"), &serde_json::json!({"propositions": result.propositions, "model_fingerprint": fingerprint, "at": crate::docpipe::now_rfc3339()}))
        .map_err(|e| e.to_string())?;
    Ok(serde_json::json!({"rendered": rendered.len(), "discarded": discarded.iter().map(|(id, why)| serde_json::json!({"candidate": id, "reason": why})).collect::<Vec<_>>(),
                          "model_fingerprint": fingerprint}))
}

#[tauri::command]
fn llm_status() -> infer_llm::LlmStatus {
    infer_llm::status()
}

/// B-S4-00: model tier catalogue (default base-install model vs optional add-on).
#[tauri::command]
fn llm_tiers() -> Result<serde_json::Value, String> {
    infer_llm::tiers()
}

/// B-S4-00: switch tier for this session; returns the resulting status so the UI shows
/// readiness (the sidecar refuses the add-on below 8 GB RAM). Never downloads a model.
#[tauri::command]
fn llm_set_tier(tier: String) -> Result<infer_llm::LlmStatus, String> {
    infer_llm::set_tier(&tier)?;
    Ok(infer_llm::status())
}

#[tauri::command]
fn llm_chat(request: infer_llm::LlmChatRequest) -> Result<infer_llm::LlmChatResult, String> {
    // B-S1-21: G1–G3 input filter before model (hard refuse).
    let user = request.user_text.clone();
    let input_guard = guardrails::check_user_input(&user);
    if input_guard.refused {
        return Ok(infer_llm::LlmChatResult {
            content: input_guard.content,
            model: "guardrails".into(),
            backend: "caseforge.guardrails.v1".into(),
            elapsed_ms: 0,
            role: request.role.clone().unwrap_or_else(|| "counsel".into()),
            format: request.format.clone().unwrap_or_else(|| "oral_argument".into()),
            disclaimer: "Local model output for preparation only. Not legal advice. Citations require pack verification.".into(),
            source: "guardrails".into(),
            controlled_beta_ready: false,
            provenance: json!({"guardrail_refusal": true, "model_called": false}),
            eval_count: None,
            tokens_per_second: None,
            propositions: Vec::new(),
            proposition_ledger: None,
            mock_court_turn: None,
        });
    }

    // B-S3-14: trusted model output must be structured before any prose is rendered.
    if !request.structured_propositions.unwrap_or(false) {
        return Err("CF_PROPOSITION_LEDGER_REQUIRED: controlled generation must emit structured propositions for host verification".into());
    }
    let verify_output_kind = request
        .output_kind
        .clone()
        .unwrap_or_else(|| "REASONING".into());
    let verify_sources = request.support_sources.clone().unwrap_or_default();
    let permitted_authority_ids = request.permitted_authority_ids.clone().unwrap_or_default();

    // B-S1-20: acquire primary large model slot (evicts vision if needed).
    let acq = choreography::acquire("primary_llm", "infer-llm chat");
    if !acq.ok {
        return Err(acq
            .error
            .unwrap_or_else(|| "CF_CHOREOGRAPHY: cannot load primary_llm".into()));
    }
    let result = infer_llm::chat(request);
    choreography::touch("primary_llm");
    let mut result = result?;
    if result.propositions.is_empty() {
        return Err("CF_PROPOSITION_SCHEMA: model returned no parseable proposition array; output was not rendered".into());
    }
    let ledger = proposition::verify(proposition::VerifyRequest {
        output_kind: verify_output_kind,
        propositions: result.propositions.clone(),
        support_sources: verify_sources,
        permitted_authority_ids,
    })?;
    if ledger.rendered_text.trim().is_empty() {
        return Err("CF_PROPOSITION_EMPTY: verified ledger rendered no output".into());
    }
    result.content = ledger.rendered_text.clone();
    result.proposition_ledger = Some(ledger);
    // B-S1-21: output filter G1–G3 (+ G6 note).
    let out = guardrails::check_model_output(&user, &result.content, &result.model, &[]);
    Ok(infer_llm::LlmChatResult {
        content: out.content,
        model: result.model,
        backend: result.backend,
        elapsed_ms: result.elapsed_ms,
        role: result.role,
        format: result.format,
        disclaimer: result.disclaimer,
        source: if out.refused {
            "guardrails".into()
        } else {
            result.source
        },
        controlled_beta_ready: result.controlled_beta_ready,
        provenance: result.provenance,
        eval_count: result.eval_count,
        tokens_per_second: result.tokens_per_second,
        propositions: result.propositions,
        proposition_ledger: result.proposition_ledger,
        mock_court_turn: result.mock_court_turn,
    })
}

// --- Voice / lexicon (B-S1-16) ---

#[tauri::command]
fn speech_status() -> infer_speech::SpeechStatus {
    let mut status = infer_speech::status();
    let (tier, _) = choreography::detect_tier();
    if tier == "not_supported" {
        status.ready = false;
        status.production_ready = false;
        status.detail = "Text-only fallback: this computer has less than 4 GiB RAM and cannot safely load offline speech.".into();
    } else {
        status.detail = format!("{} · hardware tier {tier}", status.detail);
    }
    status
}

#[tauri::command]
fn speech_warmup() -> Result<serde_json::Value, String> {
    if choreography::detect_tier().0 == "not_supported" {
        return Err("CF_SPEECH_HARDWARE_UNSUPPORTED: use text-only fallback".into());
    }
    infer_speech::warmup()
}

#[tauri::command]
fn speech_stt(
    app: AppHandle,
    vault: State<'_, VaultState>,
    request: infer_speech::SttRequest,
) -> Result<serde_json::Value, String> {
    if choreography::detect_tier().0 == "not_supported" {
        return Err("CF_SPEECH_HARDWARE_UNSUPPORTED: use text-only fallback".into());
    }
    // Speech is small-role; still tracked for status chip.
    let _ = choreography::acquire("speech_stt", "infer-speech stt");
    let stt = infer_speech::stt(request)?;
    choreography::touch("speech_stt");
    if stt.network_used || stt.cloud {
        return Err("CF_SPEECH_NETWORK: voice path must be offline (AC-9.3)".into());
    }
    // Apply permanent STT lexicon when vault unlocked (AC-9.2).
    let staged = if let Ok(mgr) = vault.get_or_init(&app) {
        match mgr.lexicon_apply_stt(&stt.raw_transcript) {
            Ok(s) => s,
            Err(VaultError::Locked) => stt.raw_transcript.clone(),
            Err(e) => return Err(e.to_string()),
        }
    } else {
        stt.raw_transcript.clone()
    };
    Ok(serde_json::json!({
        "raw_transcript": stt.raw_transcript,
        "staged_transcript": staged,
        "language": stt.language,
        "language_pinned": stt.language_pinned,
        "backend": stt.backend,
        "pipeline_id": stt.pipeline_id,
        "whisper_size": stt.whisper_size,
        "cloud": stt.cloud,
        "network_used": stt.network_used,
        "elapsed_ms": stt.elapsed_ms,
        "confidence": stt.confidence,
        "input_kind": stt.input_kind,
        "production_eligible": stt.production_eligible,
        "audio_sha256": stt.audio_sha256,
        "audio_bytes": stt.audio_bytes,
        "audio_retained": stt.audio_retained,
        "notes": stt.notes,
        "disclaimer": stt.disclaimer,
        "lexicon_applied": staged != stt.raw_transcript,
    }))
}

#[tauri::command]
fn speech_tts(
    app: AppHandle,
    vault: State<'_, VaultState>,
    request: infer_speech::TtsRequest,
) -> Result<infer_speech::TtsResult, String> {
    if choreography::detect_tier().0 == "not_supported" {
        return Err("CF_SPEECH_HARDWARE_UNSUPPORTED: use text-only fallback".into());
    }
    let spoken = if let Ok(mgr) = vault.get_or_init(&app) {
        match mgr.lexicon_apply_tts(&request.text) {
            Ok(s) => s,
            Err(VaultError::Locked) => request.text.clone(),
            Err(e) => return Err(e.to_string()),
        }
    } else {
        request.text.clone()
    };
    let mut req = request;
    req.spoken_text = Some(spoken);
    let result = infer_speech::tts(req)?;
    if result.network_used || result.cloud {
        return Err("CF_SPEECH_NETWORK: TTS path must be offline (AC-9.3)".into());
    }
    Ok(result)
}

#[tauri::command]
fn speech_transcript_commit(
    app: AppHandle,
    vault: State<'_, VaultState>,
    input: vault::SpeechTranscriptCommitInput,
) -> Result<vault::SpeechTranscriptCommit, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.speech_transcript_commit(input).map_err(Into::into)
}

#[tauri::command]
fn lexicon_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    kind: Option<String>,
) -> Result<Vec<vault::LexiconEntry>, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.lexicon_list(kind.as_deref()).map_err(Into::into)
}

#[tauri::command]
fn lexicon_upsert(
    app: AppHandle,
    vault: State<'_, VaultState>,
    entry: vault::LexiconUpsertInput,
) -> Result<vault::LexiconEntry, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.lexicon_upsert(entry).map_err(Into::into)
}

#[tauri::command]
fn lexicon_delete(
    app: AppHandle,
    vault: State<'_, VaultState>,
    entry_id: String,
) -> Result<bool, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.lexicon_delete(&entry_id).map_err(Into::into)
}

#[tauri::command]
fn lexicon_export(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<vault::LexiconExportEnvelope, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.lexicon_export().map_err(Into::into)
}

#[tauri::command]
fn lexicon_import(
    app: AppHandle,
    vault: State<'_, VaultState>,
    envelope_json: String,
) -> Result<u32, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.lexicon_import(&envelope_json).map_err(Into::into)
}

#[tauri::command]
fn lexicon_apply_stt(
    app: AppHandle,
    vault: State<'_, VaultState>,
    text: String,
) -> Result<String, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.lexicon_apply_stt(&text).map_err(Into::into)
}

#[tauri::command]
fn lexicon_policy() -> serde_json::Value {
    serde_json::json!({
        "batch": "B-S1-16",
        "primary_language": "English",
        "language_pinning": true,
        "whisper_size": "small",
        "whisper_status": "PROVISIONAL_STAGE1",
        "pipeline_id": "caseforge.speech.provisional.v1",
        "stt_path": "Whisper → editable staging box → commit",
        "tts_roles": ["bench", "opposition", "witness"],
        "network_required": false,
        "lexicon_export_extension": ".cflex.json",
        "acs": ["AC-9.1", "AC-9.2", "AC-9.3", "AC-9.4"],
    })
}

// --- Experience Store (B-S1-17) ---

#[tauri::command]
fn experience_policy(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<vault::ExperiencePolicy, String> {
    let mgr = vault.get_or_init(&app)?;
    // Prefer live policy from vault when unlocked; static defaults otherwise.
    match mgr.experience_policy() {
        Ok(p) => Ok(p),
        Err(VaultError::Locked) => Ok(vault::ExperiencePolicy {
            batch: "B-S1-17",
            pipeline_id: vault::EXPERIENCE_PIPELINE_ID,
            top_k: vault::EXPERIENCE_TOP_K,
            fleet_enabled: false,
            fleet_network_upload: false,
            fleet_requires_explicit_review: true,
            stage1_writable: vec![
                "correction",
                "lexicon_entry",
                "draft_edit",
                "rated_exchange",
            ],
            stage2_writable: vec!["limb_outcome", "bench_setting"],
            storage: "encrypted_app_vault_sqlcipher",
            acs: ["AC-11.1", "AC-11.2", "AC-15.1", "B-S2-06"],
        }),
        Err(e) => Err(e.to_string()),
    }
}

#[tauri::command]
fn experience_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    category: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<vault::ExperienceRecord>, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.experience_list(category.as_deref(), limit.unwrap_or(100))
        .map_err(Into::into)
}

#[tauri::command]
fn experience_upsert(
    app: AppHandle,
    vault: State<'_, VaultState>,
    entry: vault::ExperienceUpsertInput,
) -> Result<vault::ExperienceRecord, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.experience_upsert(entry).map_err(Into::into)
}

#[tauri::command]
fn experience_delete(
    app: AppHandle,
    vault: State<'_, VaultState>,
    record_id: String,
) -> Result<bool, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.experience_delete(&record_id).map_err(Into::into)
}

#[tauri::command]
fn experience_top_k(
    app: AppHandle,
    vault: State<'_, VaultState>,
    query: String,
    top_k: Option<usize>,
    jurisdiction: Option<String>,
) -> Result<Vec<vault::ExperienceHit>, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.experience_top_k(&query, top_k, jurisdiction.as_deref())
        .map_err(Into::into)
}

#[tauri::command]
fn experience_request_fleet_review(
    app: AppHandle,
    vault: State<'_, VaultState>,
    record_id: String,
) -> Result<vault::ExperienceRecord, String> {
    let mgr = vault.get_or_init(&app)?;
    // Always reinforce no network upload.
    let _ = mgr.experience_fleet_network_forbidden();
    if mgr.air_gap_enabled().unwrap_or(false) {
        return Err(
            "CF_AIR_GAP: fleet review networking is killed while air-gap is enabled".into(),
        );
    }
    mgr.experience_request_fleet_review(&record_id)
        .map_err(Into::into)
}

// --- Licensing + air-gap (B-S1-18) ---

#[tauri::command]
fn licence_policy() -> serde_json::Value {
    licence::policy()
}

#[tauri::command]
fn licence_status(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<licence::LicenceStatus, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.licence_status().map_err(Into::into)
}

#[tauri::command]
fn licence_fingerprint() -> licence::FingerprintComponents {
    licence::collect_fingerprint()
}

#[tauri::command]
fn licence_activate(
    app: AppHandle,
    vault: State<'_, VaultState>,
    payload_json: String,
) -> Result<licence::ActivateResult, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.licence_activate(&payload_json).map_err(Into::into)
}

#[tauri::command]
fn licence_mark_expired(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<licence::LicenceStatus, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.licence_mark_expired().map_err(Into::into)
}

#[tauri::command]
fn air_gap_set(
    app: AppHandle,
    vault: State<'_, VaultState>,
    enabled: bool,
) -> Result<licence::LicenceStatus, String> {
    let mgr = vault.get_or_init(&app)?;
    if enabled {
        // Kill companion session if running.
        // companion stop is separate; flag alone blocks restarts.
    }
    mgr.air_gap_set(enabled).map_err(Into::into)
}

#[tauri::command]
fn network_audit(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<licence::NetworkAuditReport, String> {
    let mgr = vault.get_or_init(&app)?;
    let air = mgr.air_gap_enabled().unwrap_or(false);
    Ok(licence::network_audit(air))
}

// --- Protection (B-S1-19) ---

#[tauri::command]
fn protection_policy() -> serde_json::Value {
    protection::policy()
}

#[tauri::command]
fn protection_status(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<protection::ProtectionStatus, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    Ok(protection::status(&paths))
}

#[tauri::command]
fn protection_protect_pack(
    app: AppHandle,
    vault: State<'_, VaultState>,
    pack_id: String,
) -> Result<protection::ProtectResult, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let packs = packs::list_installed(&paths);
    let pack = packs
        .iter()
        .find(|p| p.pack_id == pack_id)
        .ok_or("CF_PACK_NOT_FOUND")?;
    let dir = std::path::PathBuf::from(&pack.install_path);
    protection::protect_pack_content(&dir, &pack_id)
}

#[tauri::command]
fn protection_protect_models(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<protection::ProtectResult, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    protection::protect_models_dir(&paths)
}

#[tauri::command]
fn protection_verify_all(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<Vec<protection::IntegrityResult>, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    Ok(protection::verify_all(&paths))
}

#[tauri::command]
fn protection_copied_folder_audit(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<protection::CopiedFolderAudit, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    Ok(protection::copied_folder_audit(&paths))
}

#[tauri::command]
fn protection_no_ransom_audit() -> protection::NoRansomAudit {
    protection::no_ransom_audit()
}

// --- Model choreography (B-S1-20) ---

#[tauri::command]
fn choreography_policy() -> serde_json::Value {
    choreography::policy()
}

#[tauri::command]
fn choreography_status() -> choreography::ChoreographyStatus {
    choreography::status()
}

#[tauri::command]
fn choreography_acquire(role: String, detail: Option<String>) -> choreography::AcquireResult {
    choreography::acquire(&role, detail.as_deref().unwrap_or(""))
}

#[tauri::command]
fn choreography_release(role: String) -> choreography::ChoreographyStatus {
    choreography::release(&role)
}

#[tauri::command]
fn choreography_unload_all() -> choreography::ChoreographyStatus {
    choreography::unload_all()
}

#[tauri::command]
fn choreography_metrics() -> serde_json::Value {
    let st = choreography::status();
    let b = &st.budgets;
    serde_json::json!({
        "batch": choreography::BATCH,
        "pipeline_id": choreography::PIPELINE_ID,
        "tier": st.tier,
        "live_simulation_allowed": st.live_simulation_allowed,
        "chip_label": st.chip_label,
        "chip_tone": st.chip_tone,
        "large_resident_count": st.large_resident_count,
        "primary_and_vision_concurrent": st.primary_and_vision_concurrent,
        "hard_fail_primary_vision": st.hard_fail_primary_vision,
        "ac_3_1_cold_start": {
            "budget_s": b.cold_start_max_s,
            "uptime_ms": b.process_uptime_ms,
            "ok": b.cold_start_ok,
            "note": "ok=null means session already past cold-start window (measure at launch)"
        },
        "ac_3_1_idle_ram": {
            "budget_mb": b.idle_ram_max_mb,
            "rss_mb": b.process_rss_mb,
            "ok": b.idle_ram_ok,
            "note": "only evaluated when large models unloaded"
        },
        "ac_3_4_digest": {
            "budget_min": b.digest_max_min,
            "enforced_by": "foreground digest path must complete under budget; measure on Full tier hardware"
        },
        "ac_3_5_ui_freeze": {
            "budget_ms": b.ui_freeze_max_ms,
            "enforced_by": "heavy work off UI thread (shell_heavy_job + sidecars)"
        },
        "ac_4_2_rss": {
            "budget_gb": b.total_rss_max_gb,
            "rss_gb": b.process_rss_gb,
            "ok": b.rss_ok,
        },
        "measurement_notes": b.measurement_notes,
        "resident": st.resident,
    })
}

// --- Guardrails G1–G6 (B-S1-21) ---

#[tauri::command]
fn guardrails_policy() -> serde_json::Value {
    guardrails::policy()
}

#[tauri::command]
fn guardrails_check_input(text: String) -> guardrails::GuardResult {
    guardrails::check_user_input(&text)
}

#[tauri::command]
fn guardrails_check_output(
    user_text: String,
    model_text: String,
    model_id: Option<String>,
) -> guardrails::GuardResult {
    guardrails::check_model_output(
        &user_text,
        &model_text,
        model_id.as_deref().unwrap_or("unknown"),
        &[],
    )
}

#[tauri::command]
fn hardware_gate() -> installer::HardwareGate {
    installer::evaluate()
}

#[tauri::command]
fn hardware_policy() -> serde_json::Value {
    installer::policy()
}

#[tauri::command]
fn ship_stage1_meta() -> serde_json::Value {
    let hw = installer::evaluate();
    serde_json::json!({
        "batch": "B-S1-22",
        "release": "v1.0.0-rc1",
        "product": "CaseForge",
        "stage": "S1_Digest",
        "stage2_default_on": false,
        "hardware": hw,
        "ship_checklist": [
            "run npm run ship:stage1",
            "AC-2.1 docpipe dependency audit",
            "Stage-1 batch checks PASS or waived",
            "G6 copy audit",
            "installer refuses <4GB / 32-bit"
        ],
    })
}

/// Stage 2 Mock Court ship meta (B-S2-07 / v2.0).
#[tauri::command]
fn ship_stage2_meta() -> serde_json::Value {
    let hw = installer::evaluate();
    let full_tier = hw.tier == "full" || hw.tier == "unknown";
    serde_json::json!({
        "batch": "B-S2-07",
        "release": "v2.0.0-rc1",
        "product": "CaseForge",
        "stage": "S2_MockCourt",
        "version": PRODUCT_VERSION,
        "stage2_shipped": true,
        "stage2_default_on": false,
        "mock_court_requires_full_tier": true,
        "live_voice_simulation_requires_full_tier": true,
        "lite_live_sim_allowed": false,
        "full_tier_ready": full_tier,
        "hardware": hw,
        "surfaces": [
            "Argument Tree (integrity)",
            "Mock Court text formats",
            "Adaptive Bench",
            "Voice-in-court (Full tier)",
            "Bench Prep Mode",
            "Experience limb_outcome / bench_setting"
        ],
        "ship_checklist": [
            "run npm run ship:stage2",
            "AC-2.1 docpipe dependency audit",
            "B-S2-01 … B-S2-06 PASS",
            "AC-15.1 full red-team zero G1-G3",
            "Full-tier messaging for Mock Court",
            "Lite honesty refuse for live voice sim"
        ],
        "report": "eval/reports/stage2_ship_gate.json",
    })
}

/// Run red-team prompt list through hard filters (no model required).
/// Used by Stage-1 (24) and Stage-2 full AC-15.1 (≥200).
#[tauri::command]
fn guardrails_redteam_stage1(prompts_json: String) -> Result<serde_json::Value, String> {
    guardrails::run_redteam_suite(prompts_json, guardrails::SUITE_ID)
}

/// Full Stage-2 red-team suite runner (AC-15.1).
#[tauri::command]
fn guardrails_redteam_full(prompts_json: String) -> Result<serde_json::Value, String> {
    guardrails::run_redteam_suite(prompts_json, guardrails::SUITE_ID_FULL)
}

#[tauri::command]
fn shell_heavy_job_start(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    duration_ms: Option<u64>,
) -> Result<HeavyJobStartResult, String> {
    let duration_ms = duration_ms.unwrap_or(1500).clamp(200, 10_000);
    let job_id = Uuid::new_v4().to_string();
    let started = Instant::now();

    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        jobs.insert(
            job_id.clone(),
            JobRecord {
                state: HeavyJobState::Running,
                progress_percent: 0,
                work_elapsed_ms: 0,
                message: "queued".into(),
                started,
            },
        );
    }
    state.heavy_jobs_started.fetch_add(1, Ordering::SeqCst);

    let job_id_thread = job_id.clone();
    let app_state = state.inner().clone();
    let app_handle = app.clone();

    // AC-3.5: heavy work MUST NOT run on the UI/main thread.
    thread::spawn(move || {
        let steps = 20u32;
        let step_ms = duration_ms / u64::from(steps.max(1));
        for i in 1..=steps {
            // Dummy CPU burn + sleep to simulate sidecar work without blocking UI.
            burn_cpu_briefly();
            thread::sleep(Duration::from_millis(step_ms.max(1)));
            let pct = (i * 100) / steps;
            if let Ok(mut jobs) = app_state.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_id_thread) {
                    rec.progress_percent = pct;
                    rec.work_elapsed_ms = rec.started.elapsed().as_millis() as u64;
                    rec.message = format!("step {i}/{steps}");
                    if pct >= 100 {
                        rec.state = HeavyJobState::Completed;
                        rec.message = "completed".into();
                    }
                }
            }
            let _ = app_handle.emit(
                "heavy_job_progress",
                HeavyJobProgressEvent {
                    job_id: job_id_thread.clone(),
                    progress_percent: pct,
                    message: format!("step {i}/{steps}"),
                },
            );
        }
    });

    Ok(HeavyJobStartResult {
        job_id,
        accepted: true,
    })
}

#[tauri::command]
fn shell_heavy_job_status(
    state: State<'_, Arc<AppState>>,
    job_id: String,
) -> Result<HeavyJobStatus, String> {
    let jobs = state.jobs.lock().map_err(|e| e.to_string())?;
    match jobs.get(&job_id) {
        Some(rec) => Ok(HeavyJobStatus {
            job_id,
            state: rec.state.clone(),
            progress_percent: rec.progress_percent,
            work_elapsed_ms: rec.work_elapsed_ms,
            message: rec.message.clone(),
        }),
        None => Ok(HeavyJobStatus {
            job_id,
            state: HeavyJobState::Unknown,
            progress_percent: 0,
            work_elapsed_ms: 0,
            message: "unknown job".into(),
        }),
    }
}

/// Brief busy-work so the job is not a pure sleep (still off UI thread).
fn burn_cpu_briefly() {
    let mut x: u64 = 0;
    for i in 0..50_000u64 {
        x = x.wrapping_mul(1664525).wrapping_add(i);
    }
    std::hint::black_box(x);
}

// --- Vault commands (B-S1-02) ---

#[tauri::command]
fn vault_status(app: AppHandle, vault: State<'_, VaultState>) -> Result<VaultStatus, String> {
    vault.get_or_init(&app)?.status().map_err(Into::into)
}

#[tauri::command]
fn vault_init_unlock(
    app: AppHandle,
    vault: State<'_, VaultState>,
    passphrase: String,
) -> Result<VaultStatus, String> {
    if passphrase.trim().is_empty() {
        return Err("CF_VAULT_PASSPHRASE_REQUIRED".into());
    }
    vault
        .get_or_init(&app)?
        .init_and_unlock(&passphrase)
        .map_err(Into::into)
}

#[tauri::command]
fn vault_unlock(
    app: AppHandle,
    vault: State<'_, VaultState>,
    passphrase: String,
) -> Result<VaultStatus, String> {
    if passphrase.trim().is_empty() {
        return Err("CF_VAULT_PASSPHRASE_REQUIRED".into());
    }
    vault
        .get_or_init(&app)?
        .unlock(&passphrase)
        .map_err(Into::into)
}

#[tauri::command]
fn vault_lock(app: AppHandle, vault: State<'_, VaultState>) -> Result<VaultStatus, String> {
    vault.get_or_init(&app)?.lock().map_err(Into::into)
}

/// Wipe local app + case vaults so the user can create a new password (forgot-password path).
/// Requires explicit UI confirmation; irreversible for local case data.
#[tauri::command]
fn vault_reset_local(app: AppHandle, vault: State<'_, VaultState>) -> Result<VaultStatus, String> {
    vault.get_or_init(&app)?.reset_local().map_err(Into::into)
}

#[tauri::command]
fn case_create(
    app: AppHandle,
    vault: State<'_, VaultState>,
    passphrase: String,
    wizard: CaseWizardInput,
) -> Result<CaseCreateResult, String> {
    if passphrase.trim().is_empty() {
        return Err("CF_VAULT_PASSPHRASE_REQUIRED".into());
    }
    vault
        .get_or_init(&app)?
        .create_case_with_wizard(&passphrase, wizard)
        .map_err(Into::into)
}

#[tauri::command]
fn case_wizard_meta() -> serde_json::Value {
    serde_json::json!({
        "jurisdictions": JURISDICTIONS,
        "default_language": "English",
        "fields": [
            "cause_title",
            "court",
            "jurisdiction",
            "matter_type",
            "user_side",
            "code_era",
            "language"
        ],
        "batch": "B-S1-03",
        "prd_ref": "§5"
    })
}

#[tauri::command]
fn case_suggest_code_era(jurisdiction: String) -> String {
    VaultManager::suggest_code_era_for(&jurisdiction)
}

#[tauri::command]
fn case_open(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    passphrase: String,
) -> Result<CaseSummary, String> {
    if passphrase.trim().is_empty() {
        return Err("CF_VAULT_PASSPHRASE_REQUIRED".into());
    }
    vault
        .get_or_init(&app)?
        .open_case(&case_id, &passphrase)
        .map_err(Into::into)
}

#[tauri::command]
fn case_close(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<(), String> {
    vault
        .get_or_init(&app)?
        .close_case(&case_id)
        .map_err(Into::into)
}

#[tauri::command]
fn case_list(app: AppHandle, vault: State<'_, VaultState>) -> Result<Vec<CaseSummary>, String> {
    vault.get_or_init(&app)?.list_cases().map_err(Into::into)
}

// --- Document intake (B-S1-04) ---

#[tauri::command]
fn doc_intake_meta() -> serde_json::Value {
    VaultManager::doc_intake_meta()
}

#[tauri::command]
fn doc_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<Vec<DocumentMeta>, String> {
    vault
        .get_or_init(&app)?
        .doc_list(&case_id)
        .map_err(Into::into)
}

#[tauri::command]
fn doc_ingest(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    request: DocumentIngestRequest,
) -> Result<DocumentMeta, String> {
    vault
        .get_or_init(&app)?
        .doc_ingest(&case_id, request)
        .map_err(Into::into)
}

#[tauri::command]
fn doc_update_type_tag(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: String,
    type_tag: String,
) -> Result<DocumentMeta, String> {
    vault
        .get_or_init(&app)?
        .doc_update_type_tag(&case_id, &doc_id, &type_tag)
        .map_err(Into::into)
}

#[tauri::command]
fn doc_text_preview(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: String,
    max_chars: Option<u32>,
) -> Result<DocTextPreview, String> {
    vault
        .get_or_init(&app)?
        .doc_text_preview(&case_id, &doc_id, max_chars.unwrap_or(12_000) as usize)
        .map_err(Into::into)
}

#[tauri::command]
fn case_llm_context(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    max_chars: Option<u32>,
) -> Result<String, String> {
    vault
        .get_or_init(&app)?
        .case_llm_context(&case_id, max_chars.unwrap_or(24_000) as usize)
        .map_err(Into::into)
}

// --- docpipe triage (B-S1-05) + recognition (B-S1-06) ---

#[tauri::command]
fn docpipe_meta() -> docpipe::DocpipeMeta {
    docpipe::docpipe_meta()
}

#[tauri::command]
fn docpipe_triage_start(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
    doc_id: String,
) -> Result<docpipe::TriageStartResult, String> {
    let mgr = vault.get_or_init(&app)?;
    docpipe::start_triage_job(app, mgr, docpipe_state.inner().clone(), case_id, doc_id)
}

#[tauri::command]
fn docpipe_triage_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
) -> Result<docpipe::TriageStartResult, String> {
    let mgr = vault.get_or_init(&app)?;
    docpipe::start_triage_case_job(app, mgr, docpipe_state.inner().clone(), case_id)
}

#[tauri::command]
fn docpipe_triage_status(
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    job_id: String,
) -> docpipe::TriageJobStatus {
    docpipe_state.status(&job_id)
}

#[tauri::command]
fn docpipe_page_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: Option<String>,
) -> Result<Vec<PageTriageRow>, String> {
    vault
        .get_or_init(&app)?
        .page_triage_list(&case_id, doc_id.as_deref())
        .map_err(Into::into)
}

#[tauri::command]
fn docpipe_page_source_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: Option<String>,
) -> Result<Vec<crate::vault::PageSourceRow>, String> {
    vault
        .get_or_init(&app)?
        .page_source_list(&case_id, doc_id.as_deref())
        .map_err(Into::into)
}

#[tauri::command]
fn docpipe_recognize_start(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
    doc_id: String,
) -> Result<docpipe::TriageStartResult, String> {
    // B-S1-20: vision path claims large slot (evicts primary_llm).
    let acq = choreography::acquire("vision", "docpipe recognize");
    if acq.status.hard_fail_primary_vision {
        return Err("CF_CHOREOGRAPHY: primary+vision concurrent forbidden".into());
    }
    let mgr = vault.get_or_init(&app)?;
    docpipe::start_recognize_job(app, mgr, docpipe_state.inner().clone(), case_id, doc_id)
}

#[tauri::command]
fn docpipe_recognize_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
) -> Result<docpipe::TriageStartResult, String> {
    let acq = choreography::acquire("vision", "docpipe recognize case");
    if acq.status.hard_fail_primary_vision {
        return Err("CF_CHOREOGRAPHY: primary+vision concurrent forbidden".into());
    }
    let mgr = vault.get_or_init(&app)?;
    docpipe::start_recognize_case_job(app, mgr, docpipe_state.inner().clone(), case_id)
}

#[tauri::command]
fn docpipe_text_layer_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: Option<String>,
) -> Result<Vec<crate::vault::PageTextLayerRow>, String> {
    vault
        .get_or_init(&app)?
        .page_text_layer_list(&case_id, doc_id.as_deref())
        .map_err(Into::into)
}

#[tauri::command]
fn docpipe_page_image(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: String,
    page_index: i64,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let meta = mgr.doc_get(&case_id, &doc_id).map_err(|e| e.to_string())?;
    let bytes = mgr
        .doc_content_bytes(&case_id, &doc_id)
        .map_err(|e| e.to_string())?;
    docpipe::pdf_adapter::render_page(
        &bytes,
        &meta.source_format,
        &meta.original_filename,
        page_index,
    )
}

#[tauri::command]
fn docpipe_skeleton_start(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
) -> Result<docpipe::TriageStartResult, String> {
    let mgr = vault.get_or_init(&app)?;
    docpipe::start_skeleton_job(app, mgr, docpipe_state.inner().clone(), case_id)
}

#[tauri::command]
fn docpipe_case_graph_get(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<Option<serde_json::Value>, String> {
    vault
        .get_or_init(&app)?
        .case_graph_get(&case_id)
        .map_err(Into::into)
}

// --- B-S1-08 review corrections ---

#[tauri::command]
fn review_correct_graph_item(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    collection: String,
    item_id: String,
    field_name: String,
    new_value: String,
    interaction_count: Option<i64>,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .case_graph_correct_item(
            &case_id,
            &collection,
            &item_id,
            &field_name,
            &new_value,
            interaction_count.unwrap_or(2),
        )
        .map_err(Into::into)
}

#[tauri::command]
fn review_correct_word(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    doc_id: String,
    page_index: i64,
    word_index: i64,
    new_text: String,
    interaction_count: Option<i64>,
) -> Result<crate::vault::PageTextLayerRow, String> {
    vault
        .get_or_init(&app)?
        .page_word_correct(
            &case_id,
            &doc_id,
            page_index,
            word_index,
            &new_text,
            interaction_count.unwrap_or(2),
        )
        .map_err(Into::into)
}

#[tauri::command]
fn review_corrections_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    vault
        .get_or_init(&app)?
        .experience_corrections_list(&case_id, limit.unwrap_or(50))
        .map_err(Into::into)
}

#[tauri::command]
fn authority_conflicts_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    include_resolved: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    vault
        .get_or_init(&app)?
        .authority_conflicts_list(&case_id, include_resolved.unwrap_or(false))
        .map_err(Into::into)
}

#[tauri::command]
fn correction_events_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    limit: Option<i64>,
) -> Result<Vec<serde_json::Value>, String> {
    vault
        .get_or_init(&app)?
        .correction_events_list(&case_id, limit.unwrap_or(100))
        .map_err(Into::into)
}

#[tauri::command]
fn correction_revert(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    event_id: String,
    reason: Option<String>,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .correction_revert(
            &case_id,
            &event_id,
            reason.as_deref().unwrap_or("Counsel reverted correction"),
        )
        .map_err(Into::into)
}

#[tauri::command]
fn authority_conflict_resolve(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    conflict_id: String,
    resolution: String,
    reason: Option<String>,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .authority_conflict_resolve(
            &case_id,
            &conflict_id,
            &resolution,
            reason
                .as_deref()
                .unwrap_or("Counsel resolved rerun conflict"),
        )
        .map_err(Into::into)
}

// --- B-S1-09 Guided Intake Interview ---

#[tauri::command]
fn interview_open(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .interview_open(&case_id)
        .map_err(Into::into)
}

#[tauri::command]
fn interview_get(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    session_id: Option<String>,
) -> Result<Option<serde_json::Value>, String> {
    let mgr = vault.get_or_init(&app)?;
    if let Some(sid) = session_id {
        Ok(Some(
            mgr.interview_get(&case_id, &sid)
                .map_err(Into::<String>::into)?,
        ))
    } else {
        mgr.interview_latest(&case_id).map_err(Into::into)
    }
}

#[tauri::command]
fn interview_pause(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    session_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .interview_pause(&case_id, &session_id)
        .map_err(Into::into)
}

#[tauri::command]
fn interview_resume(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    session_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .interview_resume(&case_id, &session_id)
        .map_err(Into::into)
}

#[tauri::command]
fn interview_answer(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    session_id: String,
    question_index: i64,
    answer_text: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .interview_answer(&case_id, &session_id, question_index, &answer_text)
        .map_err(Into::into)
}

#[tauri::command]
fn interview_skip(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    session_id: String,
    question_index: i64,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .interview_skip(&case_id, &session_id, question_index)
        .map_err(Into::into)
}

// --- B-S1-10 overnight batch + resume ---

#[tauri::command]
fn batch_start(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
    // Optional: stop after N units to simulate kill (AC-6.8 tests). Omit for full run.
    stop_after_units: Option<u32>,
) -> Result<docpipe::TriageStartResult, String> {
    let mgr = vault.get_or_init(&app)?;
    docpipe::start_overnight_batch(
        app,
        mgr,
        docpipe_state.inner().clone(),
        case_id,
        stop_after_units,
    )
}

#[tauri::command]
fn batch_resume(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
    job_id: String,
) -> Result<docpipe::TriageStartResult, String> {
    let mgr = vault.get_or_init(&app)?;
    docpipe::resume_overnight_batch(app, mgr, docpipe_state.inner().clone(), case_id, job_id)
}

#[tauri::command]
fn batch_cancel(
    app: AppHandle,
    vault: State<'_, VaultState>,
    docpipe_state: State<'_, Arc<docpipe::DocpipeState>>,
    case_id: String,
    job_id: String,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    docpipe::cancel_overnight_batch(mgr, docpipe_state.inner().clone(), case_id, job_id)
}

#[tauri::command]
fn batch_status(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    job_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .batch_get(&case_id, &job_id)
        .map_err(Into::into)
}

#[tauri::command]
fn batch_latest(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<Option<serde_json::Value>, String> {
    vault
        .get_or_init(&app)?
        .batch_latest(&case_id)
        .map_err(Into::into)
}

// --- B-S1-11 Phone Capture Companion (LAN only) ---

#[tauri::command]
fn companion_policy() -> companion::CompanionPolicy {
    companion::companion_policy()
}

#[tauri::command]
fn companion_start(
    app: AppHandle,
    vault: State<'_, VaultState>,
    companion_state: State<'_, Arc<companion::CompanionState>>,
    case_id: String,
    port: Option<u16>,
) -> Result<companion::CompanionStartResult, String> {
    let mgr = vault.get_or_init(&app)?;
    if mgr.air_gap_enabled().unwrap_or(false) {
        return Err(
            "CF_AIR_GAP: phone companion networking is killed while air-gap mode is enabled".into(),
        );
    }
    companion::start_companion(companion_state.inner().clone(), mgr, case_id, port)
}

#[tauri::command]
fn companion_stop(
    companion_state: State<'_, Arc<companion::CompanionState>>,
) -> Result<Option<companion::CompanionSession>, String> {
    companion::stop_companion(companion_state.inner().clone())
}

#[tauri::command]
fn companion_status(
    companion_state: State<'_, Arc<companion::CompanionState>>,
) -> Option<companion::CompanionSession> {
    companion_state.status()
}

// --- B-S1-12 Knowledge packs ---

#[tauri::command]
fn pack_policy() -> serde_json::Value {
    packs::pack_policy()
}

#[tauri::command]
fn pack_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
) -> Result<Vec<packs::InstalledPack>, String> {
    let paths = vault
        .get_or_init(&app)?
        .vault_paths()
        .map_err(|e| e.to_string())?;
    Ok(packs::list_installed(&paths))
}

#[tauri::command]
fn pack_install(
    app: AppHandle,
    vault: State<'_, VaultState>,
    source_dir: String,
) -> Result<packs::PackInstallResult, String> {
    let paths = vault
        .get_or_init(&app)?
        .vault_paths()
        .map_err(|e| e.to_string())?;
    packs::install_pack_from_dir(&paths, Path::new(&source_dir))
}

#[tauri::command]
fn pack_uninstall(
    app: AppHandle,
    vault: State<'_, VaultState>,
    pack_id: String,
) -> Result<(), String> {
    let paths = vault
        .get_or_init(&app)?
        .vault_paths()
        .map_err(|e| e.to_string())?;
    packs::uninstall_pack(&paths, &pack_id)
}

#[tauri::command]
fn pack_headnote_scan(
    app: AppHandle,
    vault: State<'_, VaultState>,
    pack_id: String,
) -> Result<packs::HeadnoteScanResult, String> {
    let paths = vault
        .get_or_init(&app)?
        .vault_paths()
        .map_err(|e| e.to_string())?;
    packs::scan_installed_official(&paths, &pack_id)
}

// --- B-S1-13 Retrieval + citation GREEN/AMBER/RED ---

#[tauri::command]
fn retrieval_policy() -> serde_json::Value {
    retrieval::retrieval_policy()
}

// --- B-S3-12 matter legal working set ---

#[tauri::command]
fn working_set_policy() -> serde_json::Value {
    working_set::policy()
}

#[tauri::command]
fn working_set_get(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .working_set_get(&case_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn working_set_suggest(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let profile = mgr
        .list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.case_id == case_id)
        .ok_or_else(|| "CF_WORKING_SET_CASE_NOT_FOUND".to_string())?;
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?;
    let query = working_set::suggestion_query(&profile, graph.as_ref());
    let chunks = retrieval::build_corpus(&mgr.vault_paths().map_err(|e| e.to_string())?, &[])?;
    let index_checksum = retrieval::corpus_checksum(&chunks);
    let (terms, _) = retrieval::decompose(&query);
    let hits = retrieval::retrieve_filtered(
        &chunks,
        &terms,
        12,
        &retrieval::MetadataFilters {
            jurisdiction: Some(profile.jurisdiction.clone()),
            trust_level: Some("OFFICIAL_VERIFIED".into()),
            temporal_status: Some("APPLICABLE".into()),
            superseded_status: Some("CURRENT".into()),
            ..retrieval::MetadataFilters::default()
        },
    );
    let proposals: Vec<working_set::WorkingSetItemInput> = hits
        .iter()
        .filter_map(|hit| chunks.iter().find(|chunk| chunk.chunk_id == hit.chunk_id))
        .map(|chunk| working_set::item_from_chunk(chunk, &index_checksum))
        .collect();
    let regimes = working_set::regime_candidates(&profile, graph.as_ref());
    mgr.working_set_merge_suggestions(&case_id, &proposals, &regimes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn working_set_add_chunk(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    chunk_id: String,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let profile = mgr
        .list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.case_id == case_id)
        .ok_or_else(|| "CF_WORKING_SET_CASE_NOT_FOUND".to_string())?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let chunks = retrieval::build_corpus(&paths, &[])?;
    let checksum = retrieval::corpus_checksum(&chunks);
    let chunk = chunks
        .iter()
        .find(|c| c.chunk_id == chunk_id)
        .ok_or_else(|| {
            "CF_WORKING_SET_CHUNK_NOT_FOUND: rerun retrieval against installed packs".to_string()
        })?;
    if chunk.jurisdiction.as_deref() != Some(profile.jurisdiction.as_str()) {
        return Err("CF_WORKING_SET_WRONG_JURISDICTION".into());
    }
    let regimes = working_set::regime_candidates(
        &profile,
        mgr.case_graph_get(&case_id)
            .map_err(|e| e.to_string())?
            .as_ref(),
    );
    mgr.working_set_merge_suggestions(
        &case_id,
        &[working_set::item_from_chunk(chunk, &checksum)],
        &regimes,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn working_set_update_item(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    item_id: String,
    decision: String,
    pinned: bool,
    controlling: bool,
    counsel_rationale: Option<String>,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .working_set_update_item(
            &case_id,
            &item_id,
            &decision,
            pinned,
            controlling,
            counsel_rationale.as_deref(),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn working_set_confirm_regime(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    regime_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .working_set_confirm_regime(&case_id, &regime_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn working_set_select_for_output(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    output_kind: String,
    output_id: String,
) -> Result<serde_json::Value, String> {
    vault
        .get_or_init(&app)?
        .working_set_select_for_output(&case_id, &output_kind, &output_id)
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct RetrievalQueryArgs {
    query: String,
    top_k: Option<usize>,
    jurisdiction: Option<String>,
    authority_type: Option<String>,
    trust_level: Option<String>,
    temporal_status: Option<String>,
    superseded_status: Option<String>,
    /// Optional extra pack/content dirs (eval / USB corpus without install).
    extra_pack_dirs: Option<Vec<String>>,
    /// Authorities forced into VERIFY (e.g. PLANTED_FAKE negatives).
    force_authorities: Option<Vec<String>>,
    case_id: Option<String>,
    user_confirmed_red: Option<bool>,
}

#[tauri::command]
fn retrieval_query(
    app: AppHandle,
    vault: State<'_, VaultState>,
    args: RetrievalQueryArgs,
) -> Result<retrieval::RetrievalResult, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let extra: Vec<std::path::PathBuf> = args
        .extra_pack_dirs
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();
    let force = args.force_authorities.unwrap_or_default();
    let graph = if let Some(ref cid) = args.case_id {
        mgr.case_graph_get(cid).map_err(|e| e.to_string())?
    } else {
        None
    };
    // AC-11.1: inject top-K Experience Store patterns when vault unlocked.
    let experience_hits = mgr
        .experience_top_k(
            &args.query,
            Some(vault::EXPERIENCE_TOP_K),
            args.jurisdiction.as_deref(),
        )
        .unwrap_or_default();
    let metadata_filters = retrieval::MetadataFilters {
        jurisdiction: args.jurisdiction.clone(),
        authority_type: args.authority_type,
        trust_level: args.trust_level,
        temporal_status: args.temporal_status,
        superseded_status: args.superseded_status,
    };
    retrieval::run_pipeline(
        &paths,
        &args.query,
        args.top_k.unwrap_or(8),
        args.jurisdiction.as_deref(),
        &extra,
        graph.as_ref(),
        args.case_id.as_deref(),
        &force,
        args.user_confirmed_red.unwrap_or(false),
        &experience_hits,
        &metadata_filters,
    )
}

#[tauri::command]
fn retrieval_verify(
    authority: String,
    retrieved_json: String,
    chunks_json: Option<String>,
    query_id: Option<String>,
    user_confirmed: Option<bool>,
) -> Result<retrieval::CitationRecord, String> {
    let hits: Vec<retrieval::RetrievedHit> =
        serde_json::from_str(&retrieved_json).map_err(|e| format!("CF_RETRIEVAL_BAD_HITS: {e}"))?;
    let chunks: Vec<retrieval::CorpusChunk> = if let Some(raw) = chunks_json {
        serde_json::from_str(&raw).map_err(|e| format!("CF_RETRIEVAL_BAD_CHUNKS: {e}"))?
    } else {
        Vec::new()
    };
    Ok(retrieval::verify_and_score(
        &authority,
        &hits,
        &chunks,
        query_id.as_deref(),
        user_confirmed.unwrap_or(false),
    ))
}

// --- B-S3-14 host-owned proposition grounding ---

#[tauri::command]
fn proposition_policy() -> serde_json::Value {
    proposition::policy()
}

#[tauri::command]
fn proposition_verify(
    request: proposition::VerifyRequest,
) -> Result<proposition::PropositionLedger, String> {
    proposition::verify(request)
}

#[tauri::command]
fn case_llm_support_sources(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    max_chars: Option<u64>,
) -> Result<Vec<proposition::SupportSource>, String> {
    let manager = vault.get_or_init(&app)?;
    // B-S4-06: an opponent's filing (EXTERNAL_FILING) is never support for counsel's own propositions.
    let docs: Vec<DocumentMeta> = manager
        .doc_list(&case_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|d| d.type_tag != crate::docpipe::EXTERNAL_FILING_TAG)
        .collect();
    let limit = max_chars.unwrap_or(24_000).clamp(500, 24_000) as usize;
    let per = if docs.is_empty() {
        limit
    } else {
        (limit / docs.len()).max(400)
    };
    let mut used = 0usize;
    let mut out = Vec::new();
    for doc in docs {
        if used >= limit {
            break;
        }
        let preview = manager
            .doc_text_preview(&case_id, &doc.doc_id, per.min(limit - used))
            .map_err(|e| e.to_string())?;
        used += preview.text.chars().count();
        out.push(proposition::SupportSource {
            support_id: format!("matter:{case_id}:{}", doc.doc_id),
            kind: "MATTER_SOURCE".into(),
            exact_text: preview.text,
            source_id: None,
            chunk_id: None,
            authority_id: None,
            doc_id: Some(doc.doc_id),
            page: None,
            paragraph: None,
            trust_level: None,
            temporal_status: None,
            superseded_status: None,
            permitted: true,
        });
    }
    Ok(out)
}

// --- B-S2-05 Bench Preparation Mode ---

#[tauri::command]
fn bench_prep_policy() -> serde_json::Value {
    bench_prep::policy()
}

#[tauri::command]
fn bench_prep_turn(
    request: bench_prep::PrepTurnRequest,
) -> Result<bench_prep::PrepTurnResult, String> {
    bench_prep::run_prep_turn(request)
}

#[tauri::command]
fn bench_prep_turn_for_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    session_id: String,
    case_id: String,
    format: Option<String>,
    side: Option<String>,
    user_cue: Option<String>,
    redteam: Option<bool>,
) -> Result<bench_prep::PrepTurnResult, String> {
    let mgr = vault.get_or_init(&app)?;
    let graph = mgr
        .case_graph_get(&case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "CF_BENCH_PREP_NO_GRAPH: Bench Prep requires case_graph".to_string())?;
    bench_prep::run_prep_turn(bench_prep::PrepTurnRequest {
        session_id,
        case_id,
        format,
        case_graph: Some(graph.clone()),
        side,
        user_cue,
        retrieval_ids: None,
        redteam,
    })
}

#[derive(Deserialize)]
struct BenchPrepScriptArgs {
    case_id: Option<String>,
    case_graph: Option<serde_json::Value>,
    format: Option<String>,
    cues: Option<Vec<String>>,
    redteam: Option<bool>,
}

#[tauri::command]
fn bench_prep_run_script(
    args: BenchPrepScriptArgs,
) -> Result<bench_prep::PrepSessionReport, String> {
    let case_id = args.case_id.unwrap_or_else(|| "bench-prep".into());
    let graph = args
        .case_graph
        .unwrap_or_else(|| mock_court::sample_case_graph(&case_id));
    if args.redteam.unwrap_or(false) {
        Ok(bench_prep::run_redteam(&case_id, Some(graph)))
    } else {
        Ok(bench_prep::run_dual_counsel_session(
            &case_id,
            Some(graph),
            args.format.as_deref().unwrap_or("Final_Arguments"),
            &args.cues.unwrap_or_default(),
        ))
    }
}

#[tauri::command]
fn bench_prep_redteam(
    case_id: Option<String>,
    case_graph: Option<serde_json::Value>,
) -> bench_prep::PrepSessionReport {
    let cid = case_id.unwrap_or_else(|| "redteam".into());
    let g = case_graph.unwrap_or_else(|| mock_court::sample_case_graph(&cid));
    bench_prep::run_redteam(&cid, Some(g))
}

// --- B-S2-04 Voice-in-court + structured debrief ---

#[tauri::command]
fn voice_court_policy() -> serde_json::Value {
    voice_court::policy()
}

#[tauri::command]
fn voice_court_gate(force_tier: Option<String>) -> voice_court::VoiceGate {
    voice_court::gate(force_tier.as_deref())
}

#[tauri::command]
fn voice_court_turn(
    request: voice_court::VoiceTurnRequest,
) -> Result<voice_court::VoiceTurnResult, String> {
    voice_court::run_voice_turn(request)
}

#[derive(Deserialize)]
struct VoiceCourtScriptArgs {
    format: String,
    case_id: Option<String>,
    case_graph: Option<serde_json::Value>,
    script_id: Option<String>,
    user_lines: Option<Vec<String>>,
    lexicon_pairs: Option<Vec<(String, String)>>,
    force_tier: Option<String>,
}

#[tauri::command]
fn voice_court_run_script(
    args: VoiceCourtScriptArgs,
) -> Result<voice_court::ScriptedVoiceReport, String> {
    let case_id = args.case_id.unwrap_or_else(|| "voice-scripted".into());
    let graph = args
        .case_graph
        .unwrap_or_else(|| mock_court::sample_case_graph(&case_id));
    let script_id = args
        .script_id
        .unwrap_or_else(|| format!("voice-{}", args.format));
    voice_court::run_scripted_voice_session(
        &args.format,
        &case_id,
        Some(graph),
        &script_id,
        &args.user_lines.unwrap_or_default(),
        args.lexicon_pairs.unwrap_or_default(),
        args.force_tier.as_deref(),
    )
}

#[tauri::command]
fn voice_court_turn_for_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    session_id: String,
    case_id: String,
    format: String,
    transcript_stub: Option<String>,
    counsel_text: Option<String>,
    confirmed_raw_transcript: Option<String>,
    confirmed_audio_sha256: Option<String>,
    production_eligible: Option<bool>,
    court_role: Option<String>,
    tts_role: Option<String>,
    turn_index: Option<u32>,
    bench_archetype_id: Option<String>,
    bench_sliders: Option<adaptive_bench::SliderSet>,
    force_tier: Option<String>,
    language: Option<String>,
) -> Result<serde_json::Value, String> {
    // Lite / hardware gate first
    let g = voice_court::gate(force_tier.as_deref());
    if !g.allowed {
        return Err(format!(
            "{}: {}",
            g.refuse_code
                .as_deref()
                .unwrap_or(voice_court::LITE_REFUSE_CODE),
            g.honesty_notice
                .as_deref()
                .or(g.refuse_reason.as_deref())
                .unwrap_or("Lite tier: live Mock Court voice simulation refused.")
        ));
    }

    let mgr = vault.get_or_init(&app)?;
    let graph = mgr
        .case_graph_get(&case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "CF_VOICE_COURT_NO_GRAPH: Stage 2 voice requires case_graph".to_string())?;

    // A production UI submits only a transcript explicitly reviewed after real-audio STT.
    // Developer stubs still pass through infer-speech and are marked ineligible there.
    let stt = if let Some(confirmed) = counsel_text.clone() {
        if production_eligible != Some(true)
            || confirmed_audio_sha256.as_deref().unwrap_or("").len() != 64
        {
            return Err("CF_VOICE_COURT_AUDIO_PROOF: confirm a transcript produced from real microphone audio".into());
        }
        if confirmed.trim().is_empty() {
            return Err("CF_VOICE_COURT_EMPTY: confirmed counsel transcript is empty".into());
        }
        infer_speech::SttResult {
            raw_transcript: confirmed_raw_transcript
                .clone()
                .unwrap_or_else(|| confirmed.clone()),
            language: "English".into(),
            language_pinned: true,
            backend: "faster_whisper_local_confirmed".into(),
            pipeline_id: "caseforge.speech.offline-en.v2".into(),
            whisper_size: "small".into(),
            cloud: false,
            network_used: false,
            elapsed_ms: 0,
            confidence: None,
            input_kind: "REAL_AUDIO_CONFIRMED".into(),
            production_eligible: true,
            audio_sha256: confirmed_audio_sha256,
            audio_bytes: 0,
            audio_retained: false,
            notes: "Counsel-reviewed transcript from the preceding local real-audio STT step."
                .into(),
            disclaimer: "Confirmed counsel input; source audio was not retained.".into(),
        }
    } else {
        infer_speech::stt(infer_speech::SttRequest {
            transcript_stub,
            audio_base64: None,
            audio_format: None,
            language: language.clone(),
        })?
    };
    if stt.network_used || stt.cloud {
        return Err("CF_SPEECH_NETWORK: voice court path must be offline (AC-9.3)".into());
    }
    let staged = if stt.input_kind == "REAL_AUDIO_CONFIRMED" {
        counsel_text
            .clone()
            .unwrap_or_else(|| stt.raw_transcript.clone())
    } else {
        mgr.lexicon_apply_stt(&stt.raw_transcript)
            .map_err(|e| e.to_string())?
    };
    if stt.production_eligible {
        mgr.speech_transcript_commit(vault::SpeechTranscriptCommitInput {
            case_id: Some(case_id.clone()),
            session_id: Some(session_id.clone()),
            input_kind: stt.input_kind.clone(),
            audio_sha256: stt.audio_sha256.clone().unwrap_or_default(),
            raw_transcript: stt.raw_transcript.clone(),
            staged_transcript: staged.clone(),
            correction_applied: staged != stt.raw_transcript,
            counsel_confirmed: true,
            backend: stt.backend.clone(),
            pipeline_id: stt.pipeline_id.clone(),
        })
        .map_err(|e| e.to_string())?;
    }

    let profile = mgr
        .list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|item| item.case_id == case_id)
        .ok_or_else(|| "CF_CASE_NOT_FOUND: Voice Court matter is not registered".to_string())?;
    let set = mgr.working_set_get(&case_id).map_err(|e| e.to_string())?;
    let readiness =
        mock_court_retrieval::readiness(&case_id, &profile.jurisdiction, Some(&graph), &set);
    let retrieval = mock_court_retrieval::retrieve(
        &readiness,
        &format,
        court_role.as_deref().unwrap_or("adversary"),
        &staged,
        &graph,
    )?;
    let output_id = format!("{}:voice:{}", session_id, turn_index.unwrap_or(0));
    mgr.working_set_select_for_output(&case_id, "MOCK_COURT", &output_id)
        .map_err(|e| e.to_string())?;
    let retrieval_ids: Vec<String> = retrieval
        .authorities
        .iter()
        .flat_map(|a| [a.authority_id.clone(), a.chunk_id.clone()])
        .collect();
    let retrieval_snippets: Vec<String> = retrieval
        .authorities
        .iter()
        .map(|a| {
            format!(
                "[{} | {} | {}] {}",
                a.authority_id, a.citation, a.section_label, a.exact_text
            )
        })
        .collect();

    let mut turn = voice_court::run_voice_turn(voice_court::VoiceTurnRequest {
        session_id: session_id.clone(),
        case_id: case_id.clone(),
        format,
        turn_index,
        transcript_stub: None,
        counsel_text: Some(staged.clone()),
        court_role: court_role.clone(),
        tts_role: tts_role.clone(),
        case_graph: Some(graph.clone()),
        retrieval_ids: Some(retrieval_ids),
        retrieval_snippets: Some(retrieval_snippets),
        bench_archetype_id,
        bench_sliders,
        lexicon_pairs: None,
        force_tier: force_tier.clone(),
        language: language.clone(),
    })?;
    // Prefer vault-lexicon staged text
    turn.counsel_raw = stt.raw_transcript.clone();
    turn.counsel_staged = staged.clone();
    turn.lexicon_applied = staged != stt.raw_transcript;
    turn.stt_network_used = stt.network_used;
    let voice_generation = match grounded_mock_court::run(
        &profile,
        &graph,
        &readiness,
        &retrieval,
        &staged,
        &session_id,
        &turn.format,
        court_role.as_deref().unwrap_or("adversary"),
        turn.court_turn.clone(),
    ) {
        Ok(result) => result,
        Err(reason) => grounded_mock_court::degraded(turn.court_turn.clone(), reason),
    };
    turn.court_turn = voice_generation.turn.clone();
    turn.tts_spoken_text = turn.court_turn.assistant_text.clone().unwrap_or_default();
    turn.elapsed_ms = turn.elapsed_ms.saturating_add(voice_generation.elapsed_ms);
    turn.within_ac33 = turn.elapsed_ms <= voice_court::TURN_MAX_S * 1000;

    // TTS for reply role
    let tts = infer_speech::tts(infer_speech::TtsRequest {
        text: turn.tts_spoken_text.clone(),
        spoken_text: None,
        role: Some(turn.tts_role.clone()),
        language: language.clone(),
    })?;
    if tts.network_used || tts.cloud {
        return Err("CF_SPEECH_NETWORK: TTS path must be offline (AC-9.3)".into());
    }
    let spoken = mgr
        .lexicon_apply_tts(&tts.spoken_text)
        .unwrap_or(tts.spoken_text.clone());
    turn.tts_spoken_text = spoken;
    turn.tts_network_used = tts.network_used;
    turn.network_used = turn.stt_network_used || turn.tts_network_used;
    turn.cloud = stt.cloud || tts.cloud;

    Ok(serde_json::json!({
        "gate": g,
        "turn": turn,
        "stt": stt,
        "tts": tts,
        "retrieval": {
            "schema_version": mock_court_retrieval::SCHEMA_VERSION,
            "graph_version": readiness.graph_version,
            "working_set_version": readiness.working_set_version,
            "query": retrieval.query,
            "cache_key": retrieval.cache_key,
            "cache_hit": retrieval.cache_hit,
            "authorities": retrieval.authorities
        },
        "generation": voice_generation
    }))
}

#[tauri::command]
fn voice_court_debrief(
    session_id: String,
    case_id: String,
    format: String,
    turns_json: String,
    force_tier: Option<String>,
) -> Result<voice_court::DebriefReport, String> {
    let turns: Vec<voice_court::VoiceTurnResult> =
        serde_json::from_str(&turns_json).map_err(|e| format!("CF_VOICE_DEBRIEF_BAD_JSON: {e}"))?;
    let g = voice_court::gate(force_tier.as_deref());
    Ok(voice_court::build_debrief(
        &session_id,
        &case_id,
        &format,
        &turns,
        &g.tier,
        g.live_simulation_allowed,
    ))
}

// --- B-S2-03 Per-Case Adaptive Bench ---

#[tauri::command]
fn adaptive_bench_policy() -> serde_json::Value {
    adaptive_bench::policy()
}

#[tauri::command]
fn adaptive_bench_archetypes() -> Vec<adaptive_bench::ArchetypeMeta> {
    adaptive_bench::archetypes_meta()
}

#[tauri::command]
fn adaptive_bench_resolve(
    request: adaptive_bench::ResolveRequest,
) -> Result<adaptive_bench::BenchProfile, String> {
    adaptive_bench::resolve(request)
}

#[tauri::command]
fn adaptive_bench_resolve_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    archetype_id: Option<String>,
    sliders: Option<adaptive_bench::SliderSet>,
    judge_name: Option<String>,
    named_judge_id: Option<String>,
) -> Result<adaptive_bench::BenchProfile, String> {
    let mgr = vault.get_or_init(&app)?;
    let graph = mgr
        .case_graph_get(&case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "CF_BENCH_NO_GRAPH: open case and run skeleton for case-order signals".to_string()
        })?;
    adaptive_bench::resolve(adaptive_bench::ResolveRequest {
        case_id,
        case_graph: Some(graph),
        archetype_id,
        sliders,
        judge_name,
        named_judge_id,
    })
}

// --- B-S2-02 Mock Court text formats ---

#[tauri::command]
fn mock_court_policy() -> serde_json::Value {
    mock_court::policy()
}

#[tauri::command]
fn mock_court_formats() -> Vec<mock_court::CourtFormatMeta> {
    mock_court::formats_meta()
}

#[tauri::command]
fn mock_court_turn(request: mock_court::TurnRequest) -> Result<mock_court::SessionTurn, String> {
    // Stage 2 path: never inject raw document text here.
    mock_court::run_turn(request)
}

#[derive(Deserialize)]
struct MockCourtScriptArgs {
    format: String,
    case_id: Option<String>,
    case_graph: Option<serde_json::Value>,
    script_id: Option<String>,
    user_lines: Option<Vec<String>>,
    retrieval_ids: Option<Vec<String>>,
    retrieval_snippets: Option<Vec<String>>,
}

#[tauri::command]
fn mock_court_run_script(
    args: MockCourtScriptArgs,
) -> Result<mock_court::ScriptedSessionReport, String> {
    let case_id = args.case_id.unwrap_or_else(|| "scripted".into());
    let graph = args
        .case_graph
        .unwrap_or_else(|| mock_court::sample_case_graph(&case_id));
    let script_id = args
        .script_id
        .unwrap_or_else(|| format!("script-{}", args.format));
    let lines = args.user_lines.unwrap_or_default();
    mock_court::run_scripted_session(
        &args.format,
        &case_id,
        Some(graph),
        &script_id,
        &lines,
        args.retrieval_ids.unwrap_or_default(),
        args.retrieval_snippets.unwrap_or_default(),
    )
}

#[tauri::command]
fn mock_court_retrieval_readiness(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<mock_court_retrieval::MockCourtReadiness, String> {
    let mgr = vault.get_or_init(&app)?;
    let profile = mgr
        .list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|item| item.case_id == case_id)
        .ok_or_else(|| "CF_CASE_NOT_FOUND: Mock Court matter is not registered".to_string())?;
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?;
    let set = mgr.working_set_get(&case_id).map_err(|e| e.to_string())?;
    Ok(mock_court_retrieval::readiness(
        &case_id,
        &profile.jurisdiction,
        graph.as_ref(),
        &set,
    ))
}

#[tauri::command]
fn mock_court_turn_for_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    request: mock_court_retrieval::GroundedTurnRequest,
) -> Result<mock_court_retrieval::GroundedTurnResponse, String> {
    let mgr = vault.get_or_init(&app)?;
    let profile = mgr
        .list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|item| item.case_id == request.case_id)
        .ok_or_else(|| "CF_CASE_NOT_FOUND: Mock Court matter is not registered".to_string())?;
    let graph = mgr
        .case_graph_get(&request.case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "CF_MOCK_COURT_NO_GRAPH: run the case skeleton first".to_string())?;
    let set = mgr
        .working_set_get(&request.case_id)
        .map_err(|e| e.to_string())?;
    let readiness = mock_court_retrieval::readiness(
        &request.case_id,
        &profile.jurisdiction,
        Some(&graph),
        &set,
    );
    mock_court_retrieval::enforce_versions(&request, &readiness)?;
    let role = request.role.as_deref().unwrap_or("adversary");
    let bundle = mock_court_retrieval::retrieve(
        &readiness,
        &request.format,
        role,
        &request.user_text,
        &graph,
    )?;
    // B-S4-00b: grade counsel's answer to the PREVIOUS challenge against the record and the working set
    // (deterministic, no model) before the next challenge is generated.
    let permitted: Vec<answer_feedback::PermittedAuthority> = bundle
        .authorities
        .iter()
        .map(|a| answer_feedback::PermittedAuthority {
            authority_id: a.authority_id.clone(),
            citation: a.citation.clone(),
            label: format!("{} {} {}", a.title, a.section_label, a.citation),
        })
        .collect();
    let feedback = answer_feedback::evaluate(
        &request.session_id,
        &request.user_text,
        answer_feedback::previous_challenge(&request.session_id).as_deref(),
        &graph,
        &permitted,
    );
    let output_id = format!("{}:{}", request.session_id, Uuid::new_v4());
    mgr.working_set_select_for_output(&request.case_id, "MOCK_COURT", &output_id)
        .map_err(|e| e.to_string())?;
    let retrieval_ids: Vec<String> = bundle
        .authorities
        .iter()
        .flat_map(|a| [a.authority_id.clone(), a.chunk_id.clone()])
        .collect();
    let retrieval_snippets: Vec<String> = bundle
        .authorities
        .iter()
        .map(|a| {
            format!(
                "[{} | {} | {}] {}",
                a.authority_id, a.citation, a.section_label, a.exact_text
            )
        })
        .collect();
    // AC-2.1: only case_graph plus verified working-set chunks cross into the turn engine.
    let baseline = mock_court::run_turn(mock_court::TurnRequest {
        session_id: request.session_id,
        case_id: request.case_id,
        format: request.format,
        role: request.role,
        user_text: request.user_text,
        retrieval_ids: Some(retrieval_ids),
        retrieval_snippets: Some(retrieval_snippets),
        case_graph: Some(graph.clone()),
        script_id: None,
        deterministic_only: Some(true),
        bench_archetype_id: request.bench_archetype_id,
        bench_sliders: request.bench_sliders,
        judge_name: request.judge_name,
        named_judge_id: request.named_judge_id,
    })?;
    let generation = match grounded_mock_court::run(
        &profile,
        &graph,
        &readiness,
        &bundle,
        baseline.user_text.as_deref().unwrap_or_default(),
        &baseline.session_id,
        &baseline.format,
        &baseline.role,
        baseline.clone(),
    ) {
        Ok(result) => result,
        Err(reason) => grounded_mock_court::degraded(baseline, reason),
    };
    if let Some(challenge) = generation.turn.assistant_text.as_deref() {
        answer_feedback::remember_challenge(&generation.turn.session_id, challenge);
    }
    Ok(mock_court_retrieval::GroundedTurnResponse {
        schema_version: mock_court_retrieval::SCHEMA_VERSION.into(),
        turn: generation.turn,
        graph_version: readiness.graph_version,
        working_set_version: readiness.working_set_version,
        retrieval_query: bundle.query,
        retrieval_cache_key: bundle.cache_key,
        cache_hit: bundle.cache_hit,
        authorities: bundle.authorities,
        generation_mode: generation.generation_mode,
        degraded_reason: generation.degraded_reason,
        model: generation.model,
        backend: generation.backend,
        model_digest: generation.model_digest,
        prompt_version: generation.prompt_version,
        prompt_hash: generation.prompt_hash,
        system_prompt_hash: generation.system_prompt_hash,
        model_elapsed_ms: generation.elapsed_ms,
        tokens_per_second: generation.tokens_per_second,
        schema_repair_used: generation.schema_repair_used,
        structured_turn: generation.structured_turn,
        proposition_ledger: generation.proposition_ledger,
        answer_feedback: Some(feedback),
    })
}

#[tauri::command]
fn mock_court_debrief_v2_policy() -> serde_json::Value {
    serde_json::json!({
        "schema_version": debrief_v2::SCHEMA_VERSION,
        "task_schema_version": debrief_v2::TASK_SCHEMA_VERSION,
        "rubric_version": debrief_v2::RUBRIC_VERSION,
        "source_truth_immutable": true,
        "task_statuses": ["PROPOSED","ACCEPTED","DISMISSED","COMPLETED"],
        "generic_task_min_score": 3,
        "outcome_prediction": false
    })
}

#[tauri::command]
fn mock_court_debrief_v2_generate(
    app: AppHandle,
    vault: State<'_, VaultState>,
    request: debrief_v2::GenerateRequest,
) -> Result<debrief_v2::DebriefReportV2, String> {
    let mgr = vault.get_or_init(&app)?;
    let previous = mgr
        .debrief_v2_latest(&request.case_id)
        .map_err(|e| e.to_string())?;
    let previous = previous.filter(|v| {
        v.get("session_id").and_then(|x| x.as_str()) != Some(request.session_id.as_str())
    });
    let integrity = mgr
        .argument_tree_get(&request.case_id)
        .map_err(|e| e.to_string())?
        .and_then(|tree| integrity::score_tree(&tree).ok());
    let report = debrief_v2::generate(request, integrity.as_ref(), previous.as_ref())?;
    mgr.debrief_v2_store(&report).map_err(|e| e.to_string())?;
    Ok(report)
}

#[tauri::command]
fn preparation_tasks_list(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<Vec<debrief_v2::PreparationTask>, String> {
    vault
        .get_or_init(&app)?
        .preparation_tasks_list(&case_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn preparation_task_update(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    task_id: String,
    status: String,
    title: Option<String>,
    counsel_note: Option<String>,
) -> Result<Vec<debrief_v2::PreparationTask>, String> {
    vault
        .get_or_init(&app)?
        .preparation_task_update(
            &case_id,
            &task_id,
            &status,
            title.as_deref(),
            counsel_note.as_deref(),
        )
        .map_err(|e| e.to_string())
}

// --- B-S2-01 Argument Integrity Engine (deterministic; no LLM) ---

#[tauri::command]
fn integrity_policy() -> serde_json::Value {
    integrity::policy()
}

#[tauri::command]
fn integrity_score_tree(tree: serde_json::Value) -> Result<integrity::ScoreReport, String> {
    integrity::score_tree(&tree)
}

#[tauri::command]
fn integrity_score_case(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<integrity::ScoreReport, String> {
    let mgr = vault.get_or_init(&app)?;
    let tree = mgr
        .argument_tree_get(&case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "CF_INTEGRITY_NO_TREE: build or load an argument tree first".to_string())?;
    let report = integrity::score_tree(&tree)?;
    let _ = mgr.argument_tree_put(&case_id, &tree, report.weakest_VI.map(|v| v as i64));
    Ok(report)
}

#[tauri::command]
fn integrity_build_from_graph(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    persist: Option<bool>,
) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let graph = mgr
        .case_graph_get(&case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| {
            "CF_INTEGRITY_NO_GRAPH: run skeleton extract before building argument tree".to_string()
        })?;
    let tree = integrity::build_tree_from_case_graph(&case_id, &graph);
    let report = integrity::score_tree(&tree)?;
    if persist.unwrap_or(true) {
        mgr.argument_tree_put(&case_id, &tree, report.weakest_VI.map(|v| v as i64))
            .map_err(|e| e.to_string())?;
    }
    Ok(serde_json::json!({
        "tree": tree,
        "score": report
    }))
}

#[tauri::command]
fn argument_tree_get(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
) -> Result<Option<serde_json::Value>, String> {
    vault
        .get_or_init(&app)?
        .argument_tree_get(&case_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn argument_tree_put(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    tree: serde_json::Value,
) -> Result<integrity::ScoreReport, String> {
    // Validate + score before persist (never store unscored garbage)
    let report = integrity::score_tree(&tree)?;
    if report.llm_used {
        return Err("CF_INTEGRITY_LLM_FORBIDDEN: scoring path must not use LLM".into());
    }
    vault
        .get_or_init(&app)?
        .argument_tree_put(&case_id, &tree, report.weakest_VI.map(|v| v as i64))
        .map_err(|e| e.to_string())?;
    Ok(report)
}

/// Packaged-install layout (B-S4-00): Tauri places `resources` declared with `../` under `<install>/_up_/`.
/// Returns that folder when it holds the bundled sidecars and contracts, i.e. when running from an installer.
/// In a development build (`target/debug/caseforge.exe`) no such folder exists and nothing changes.
pub fn installed_layout_root(exe_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let root = exe_dir.join("_up_");
    (root.join("contracts").join("constants.json").is_file()
        && root.join("sidecars").join("infer_llm").join("infer_llm.py").is_file())
    .then_some(root)
}

/// Make an installed copy find its bundled files without any developer setup:
/// - working directory becomes the resource root, which every cwd-relative lookup (`contracts/...`, `sidecars/...`)
///   already tries first;
/// - `CASEFORGE_PYTHON` points to the bundled interpreter unless the user set their own.
/// User data is unaffected: the vault lives under Tauri's absolute per-user app-data directory.
fn bootstrap_installed_layout() {
    let Some(exe_dir) = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())) else {
        return;
    };
    let Some(root) = installed_layout_root(&exe_dir) else {
        return;
    };
    let _ = std::env::set_current_dir(&root);
    let python = root.join("runtime-python").join("python.exe");
    if std::env::var_os("CASEFORGE_PYTHON").is_none() && python.is_file() {
        std::env::set_var("CASEFORGE_PYTHON", python);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    bootstrap_installed_layout();
    // Force process start clocks to initialize early.
    Lazy::force(&PROCESS_START);
    Lazy::force(&PROCESS_START_UNIX_MS);

    let state = Arc::new(AppState::new());
    let vault_state = VaultState::new();
    let docpipe_state = Arc::new(docpipe::DocpipeState::new());
    let companion_state = Arc::new(companion::CompanionState::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .manage(vault_state)
        .manage(docpipe_state)
        .manage(companion_state)
        .invoke_handler(tauri::generate_handler![
            shell_ping,
            shell_version,
            shell_sidecar_status,
            shell_heavy_job_start,
            shell_heavy_job_status,
            vault_status,
            vault_init_unlock,
            vault_unlock,
            vault_lock,
            vault_reset_local,
            case_create,
            case_wizard_meta,
            case_suggest_code_era,
            case_open,
            case_close,
            case_list,
            doc_intake_meta,
            doc_list,
            doc_ingest,
            doc_update_type_tag,
            doc_text_preview,
            case_llm_context,
            docpipe_meta,
            docpipe_triage_start,
            docpipe_triage_case,
            docpipe_triage_status,
            docpipe_page_list,
            docpipe_page_source_list,
            docpipe_recognize_start,
            docpipe_recognize_case,
            docpipe_text_layer_list,
            docpipe_page_image,
            docpipe_skeleton_start,
            docpipe_case_graph_get,
            review_correct_graph_item,
            review_correct_word,
            review_corrections_list,
            authority_conflicts_list,
            correction_events_list,
            correction_revert,
            authority_conflict_resolve,
            interview_open,
            interview_get,
            interview_pause,
            interview_resume,
            interview_answer,
            interview_skip,
            batch_start,
            batch_resume,
            batch_cancel,
            batch_status,
            batch_latest,
            companion_policy,
            companion_start,
            companion_stop,
            companion_status,
            pack_policy,
            pack_list,
            pack_install,
            pack_uninstall,
            pack_headnote_scan,
            retrieval_policy,
            retrieval_query,
            retrieval_verify,
            proposition_policy,
            proposition_verify,
            case_llm_support_sources,
            working_set_policy,
            working_set_get,
            working_set_suggest,
            working_set_add_chunk,
            working_set_update_item,
            working_set_confirm_regime,
            working_set_select_for_output,
            llm_status,
            audit_draft,
            audit_certificate_issue,
            audit_certificate_verify,
            opposition_scan,
            witness_propose,
            witness_register_list,
            witness_register_save,
            witness_register_remove,
            witness_decision_add,
            witness_decision_revoke,
            witness_matrix_build,
            impeach_finding_status,
            impeach_model_scan,
            llm_tiers,
            llm_set_tier,
            llm_chat,
            speech_status,
            speech_warmup,
            speech_stt,
            speech_tts,
            speech_transcript_commit,
            lexicon_list,
            lexicon_upsert,
            lexicon_delete,
            lexicon_export,
            lexicon_import,
            lexicon_apply_stt,
            lexicon_policy,
            experience_policy,
            experience_list,
            experience_upsert,
            experience_delete,
            experience_top_k,
            experience_request_fleet_review,
            licence_policy,
            licence_status,
            licence_fingerprint,
            licence_activate,
            licence_mark_expired,
            air_gap_set,
            network_audit,
            protection_policy,
            protection_status,
            protection_protect_pack,
            protection_protect_models,
            protection_verify_all,
            protection_copied_folder_audit,
            protection_no_ransom_audit,
            choreography_policy,
            choreography_status,
            choreography_acquire,
            choreography_release,
            choreography_unload_all,
            choreography_metrics,
            guardrails_policy,
            guardrails_check_input,
            guardrails_check_output,
            guardrails_redteam_stage1,
            guardrails_redteam_full,
            hardware_gate,
            hardware_policy,
            ship_stage1_meta,
            ship_stage2_meta,
            integrity_policy,
            integrity_score_tree,
            integrity_score_case,
            integrity_build_from_graph,
            argument_tree_get,
            argument_tree_put,
            mock_court_policy,
            mock_court_formats,
            mock_court_turn,
            mock_court_run_script,
            mock_court_retrieval_readiness,
            mock_court_turn_for_case,
            mock_court_debrief_v2_policy,
            mock_court_debrief_v2_generate,
            preparation_tasks_list,
            preparation_task_update,
            adaptive_bench_policy,
            adaptive_bench_archetypes,
            adaptive_bench_resolve,
            adaptive_bench_resolve_case,
            voice_court_policy,
            voice_court_gate,
            voice_court_turn,
            voice_court_run_script,
            voice_court_turn_for_case,
            voice_court_debrief,
            bench_prep_policy,
            bench_prep_turn,
            bench_prep_turn_for_case,
            bench_prep_run_script,
            bench_prep_redteam,
        ])
        .setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_title("CaseForge");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running CaseForge");
}

#[cfg(test)]
mod installed_layout_tests {
    use super::installed_layout_root;
    use std::fs;

    fn touch(path: &std::path::Path) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, b"x").unwrap();
    }

    #[test]
    fn installed_layout_requires_both_contracts_and_the_llm_sidecar() {
        let exe_dir = tempfile::tempdir().unwrap();
        // Development layout: nothing bundled next to the executable.
        assert!(installed_layout_root(exe_dir.path()).is_none());

        // Partial bundle (contracts only) must not be mistaken for an install.
        let up = exe_dir.path().join("_up_");
        touch(&up.join("contracts").join("constants.json"));
        assert!(installed_layout_root(exe_dir.path()).is_none());

        // Complete bundle: resource root is `<exe dir>/_up_`.
        touch(&up.join("sidecars").join("infer_llm").join("infer_llm.py"));
        assert_eq!(installed_layout_root(exe_dir.path()), Some(up));
    }
}
