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
mod deadlines;
mod foundation;
mod issue_matrix;
mod treatment;
mod translation;
mod edition;
mod entitlements;
mod seat;
mod subscription;
mod ship_gate;
mod timed_rehearsal;
mod bundle;
mod bundle_lan;
mod record_only;
mod chambers_lookup;
mod cause_list;
mod g8;
mod judgment_scaffold;
mod opposition;
mod reasoning;
mod witness;
mod impeach;
mod rule_tables;
mod code_bridge;
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
    edition::require_feature("audit")?;
    if text.trim().is_empty() {
        return Err("CF_AUDIT_EXTRACTION_FAILED: the draft is empty".into());
    }
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    edition::require_capability(&edition_of(&mgr)?, &paths.root, "citation_audit")?;
    // B-S4-29: read only the passages this draft needs from the on-disk store.
    let store = retrieval::store::open(&paths)?;
    let small = retrieval::build_small_corpus(&paths, &[])?;
    let mut report = audit::audit_with_store(&store, &small, &text, filename.as_deref().unwrap_or("draft.txt"), "drafting_surface", case_id.clone())?;
    // B-S4-09: signed succession rows add CODE_BRIDGE notes (VERIFIED_WITH_NOTE); unsigned rows change nothing.
    let (tables, _) = rule_tables::load_installed(&paths);
    if !tables.is_empty() {
        let jurisdiction = case_id.as_deref().and_then(|c| case_jurisdiction(&mgr, c).ok());
        let rows = rule_tables::query(&tables, "STATUTE_SUCCESSION", jurisdiction.as_deref(), &crate::docpipe::now_rfc3339()[..10]);
        let dates = case_id.as_deref().map(|c| key_dates(&mgr, c)).transpose()?.unwrap_or_default();
        code_bridge::add_audit_notes(&mut report.items, &mut report.counts, &rows, &rule_tables::index_for_rows(&paths, &rows), &dates);
    }
    // B-S4-13: currency date of each pack, and verified treatment of the cited judgments, as notes.
    treatment::decorate_report(&paths, &store, &mut report);
    Ok(report)
}

// --- B-S4-10 Admissibility & Foundation Gaps ---

fn authority_excerpt(store: Option<&retrieval::store::Store>, a: &serde_json::Value) -> Option<String> {
    let chunk = store?.get_by_chunk_id(a["chunk_id"].as_str()?)?;
    match (a["char_start"].as_u64(), a["char_end"].as_u64()) {
        (Some(s), Some(e)) if e > s => Some(chunk.text.chars().skip(s as usize).take((e - s) as usize).collect()),
        _ => Some(chunk.text.chars().take(240).collect()),
    }
}

fn foundation_rules(tables: &[rule_tables::RuleTable], jurisdiction: &str) -> (Vec<foundation::Rule>, Vec<String>) {
    let (mut rules, mut problems) = (Vec::new(), Vec::new());
    for t in tables.iter().filter(|t| t.table_kind == "ADMISSIBILITY_REQUIREMENT" && t.jurisdiction.eq_ignore_ascii_case(jurisdiction)) {
        for row in &t.rows {
            match foundation::Rule::from_row(row) {
                Ok(r) => rules.push(r),
                Err(e) => problems.push(e),
            }
        }
    }
    (rules, problems)
}

fn graph_items(graph: &serde_json::Value, key: &str) -> Vec<serde_json::Value> {
    graph[key].as_array().cloned().unwrap_or_default()
}

fn graph_evidence(graph: &serde_json::Value) -> Vec<foundation::Evidence> {
    graph_items(graph, "evidence")
        .iter()
        .filter_map(|e| Some(foundation::Evidence { evidence_id: e["evidence_id"].as_str()?.to_string(), label: e["label"].as_str().unwrap_or("").to_string(), kind: e["kind"].as_str().unwrap_or("").to_string(), doc_id: e["doc_id"].as_str().map(str::to_string) }))
        .collect()
}

/// Admissibility foundation checks for the matter's evidence and exhibits against the installed rule rows, with the
/// gaps (rule row, authority and its text) and the Missing Links panel with foundation gaps first.
#[tauri::command]
fn foundation_check(app: AppHandle, vault: State<'_, VaultState>, case_id: String, v1_panel: Option<Vec<serde_json::Value>>) -> Result<serde_json::Value, String> {
    edition::require_feature("foundation_gaps")?;
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let jurisdiction = case_jurisdiction(&mgr, &case_id)?;
    let (tables, rejected) = rule_tables::load_installed(&paths);
    let (rules, rule_problems) = foundation_rules(&tables, &jurisdiction);
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let evidence = graph_evidence(&graph);
    let nodes: Vec<foundation::NodeRef> = graph_items(&graph, "nodes").iter().filter_map(|n| Some(foundation::NodeRef { node_id: n["node_id"].as_str()?.to_string(), node_type: n["node_type"].as_str()?.to_string(), label: n["label"].as_str().unwrap_or("").to_string() })).collect();
    let edges: Vec<foundation::Edge> = graph_items(&graph, "edges").iter().filter_map(|e| Some(foundation::Edge { from: e["from_node_id"].as_str()?.to_string(), to: e["to_node_id"].as_str()?.to_string(), status: e["status"].as_str().unwrap_or("").to_string() })).collect();
    let docs: Vec<foundation::DocInfo> = mgr.doc_list(&case_id).map_err(|e| e.to_string())?.into_iter().map(|d| foundation::DocInfo { doc_id: d.doc_id, filename: d.original_filename, type_tag: d.type_tag }).collect();
    let types: Vec<foundation::TypeChoice> = deadline_kv(&mgr, &case_id, "foundation:type:")?
        .iter()
        .filter_map(|v| Some(foundation::TypeChoice { evidence_id: v["evidence_id"].as_str()?.to_string(), evidence_type: v["evidence_type"].as_str()?.to_string() }))
        .collect();
    let links: Vec<foundation::Link> = deadline_kv(&mgr, &case_id, "foundation:link:")?
        .iter()
        .filter_map(|v| Some(foundation::Link { evidence_id: v["evidence_id"].as_str()?.to_string(), element_id: v["element_id"].as_str()?.to_string(), doc_id: v["doc_id"].as_str()?.to_string() }))
        .collect();
    let today = crate::docpipe::now_rfc3339()[..10].to_string();
    let mut as_of = vec![today];
    if let Some(d) = key_dates(&mgr, &case_id)?.get("INSTITUTION_DATE") {
        if !as_of.contains(&d.date) {
            as_of.push(d.date.clone());
        }
    }
    let out = foundation::check_all(&foundation::Inputs { case_id: &case_id, evidence: &evidence, docs: &docs, nodes: &nodes, edges: &edges, links: &links, types: &types, rules: &rules, as_of: &as_of, vocab: foundation::vocabulary() });
    let store = retrieval::store::open(&paths).ok();
    let mut gaps = out.gaps;
    for g in gaps.iter_mut() {
        g["authority_excerpt"] = serde_json::json!(authority_excerpt(store.as_deref(), &g["authority"]));
    }
    let panel = foundation::merge_panel(&v1_panel.unwrap_or_default(), &gaps);
    let items: Vec<serde_json::Value> = evidence
        .iter()
        .map(|e| {
            let t = foundation::classify(e, &docs, foundation::vocabulary(), &types);
            serde_json::json!({"evidence_id": e.evidence_id, "label": e.label, "evidence_type": t.as_ref().map(|t| t.0.clone()), "type_status": t.as_ref().map(|t| t.1)})
        })
        .collect();
    Ok(serde_json::json!({
        "checks": out.checks, "gaps": gaps, "panel": panel, "evidence": items,
        "types": foundation::vocabulary().types.iter().map(|t| serde_json::json!({"evidence_type": t.evidence_type, "label": t.label})).collect::<Vec<_>>(),
        "docs": docs.iter().map(|d| serde_json::json!({"doc_id": d.doc_id, "filename": d.filename, "type_tag": d.type_tag})).collect::<Vec<_>>(),
        "as_of": as_of, "rules_installed": rules.len(), "unsigned_rules": rules.iter().any(|r| r.trust_state != "SIGNED_OFF"),
        "rule_problems": rule_problems, "rejected_tables": rejected,
        "note": "Foundation gaps are prompts for counsel, not rulings on admissibility. They change no score.",
    }))
}

/// Counsel sets (or clears, with None) the evidence type of one item.
#[tauri::command]
fn foundation_set_type(app: AppHandle, vault: State<'_, VaultState>, case_id: String, evidence_id: String, evidence_type: Option<String>) -> Result<(), String> {
    edition::require_feature("foundation_gaps")?;
    let mgr = vault.get_or_init(&app)?;
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    if !graph_evidence(&graph).iter().any(|e| e.evidence_id == evidence_id) {
        return Err("CF_FOUNDATION_EVIDENCE_UNKNOWN: that item is not in the matter's evidence".into());
    }
    let key = format!("foundation:type:{evidence_id}");
    match evidence_type {
        None => mgr.witness_kv_delete(&case_id, &key).map_err(|e| e.to_string()),
        Some(t) => {
            if !foundation::vocabulary().types.iter().any(|v| v.evidence_type == t) {
                return Err("CF_FOUNDATION_TYPE_UNKNOWN: choose one of the listed evidence types".into());
            }
            mgr.witness_kv_put(&case_id, &key, &serde_json::json!({"evidence_id": evidence_id, "evidence_type": t, "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
        }
    }
}

/// Counsel links a document to one element of one evidence item (or removes the link with None).
#[tauri::command]
fn foundation_link(app: AppHandle, vault: State<'_, VaultState>, case_id: String, evidence_id: String, element_id: String, doc_id: Option<String>) -> Result<(), String> {
    edition::require_feature("foundation_gaps")?;
    let mgr = vault.get_or_init(&app)?;
    let key = format!("foundation:link:{evidence_id}:{element_id}");
    let Some(doc_id) = doc_id else { return mgr.witness_kv_delete(&case_id, &key).map_err(|e| e.to_string()) };
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    if !graph_evidence(&graph).iter().any(|e| e.evidence_id == evidence_id) {
        return Err("CF_FOUNDATION_EVIDENCE_UNKNOWN: that item is not in the matter's evidence".into());
    }
    if !mgr.doc_list(&case_id).map_err(|e| e.to_string())?.iter().any(|d| d.doc_id == doc_id) {
        return Err("CF_FOUNDATION_DOC_UNKNOWN: that document is not in the matter".into());
    }
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let (tables, _) = rule_tables::load_installed(&paths);
    let (rules, _) = foundation_rules(&tables, &case_jurisdiction(&mgr, &case_id)?);
    if !rules.iter().any(|r| r.elements.iter().any(|e| e.element_id == element_id)) {
        return Err("CF_FOUNDATION_ELEMENT_UNKNOWN: no installed rule has that element".into());
    }
    mgr.witness_kv_put(&case_id, &key, &serde_json::json!({"evidence_id": evidence_id, "element_id": element_id, "doc_id": doc_id, "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
}

// --- B-S4-13 Law Currency Stamp and Treatment Graph ---

/// Installed packs with the date up to which each was checked, for the stamp every citation shows.
#[tauri::command]
fn pack_currency(app: AppHandle, vault: State<'_, VaultState>) -> Result<Vec<treatment::PackStamp>, String> {
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let mut stamps = treatment::pack_stamps(&paths);
    // B-S4-22: once the pack subscription has ended every stamp keeps its older date and says updates have expired
    let sub = edition_of(&mgr)?.pack_subscription;
    for s in &mut stamps {
        s.updates_expired = sub.expired();
        s.updates_notice = sub.notice.clone();
        s.updates_expired_on = sub.until_date.clone().filter(|_| sub.expired());
    }
    Ok(stamps)
}

#[derive(Deserialize)]
struct TreatmentQuery {
    source_id: String,
    chunk_id: String,
}

/// What the installed corpus says about how later judgments treated each of these cited sources. Every edge is
/// re-verified against the installed text before it is returned; "no treatment found in installed corpus" is a statement
/// about the library, never about the law.
#[tauri::command]
fn treatment_lookup(app: AppHandle, vault: State<'_, VaultState>, sources: Vec<TreatmentQuery>) -> Result<Vec<serde_json::Value>, String> {
    edition::require_feature("currency_treatment")?;
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let store = retrieval::store::open(&paths)?;
    let set = treatment::TreatmentSet::load(&paths);
    let get = |id: &str| store.get_by_chunk_id(id);
    Ok(sources
        .iter()
        .map(|q| {
            let authority_type = get(&q.chunk_id).map(|c| c.metadata.authority_type).unwrap_or_default();
            treatment::lookup_json(&set.lookup(&q.source_id, &authority_type, &get))
        })
        .collect())
}

// --- B-S4-18 Issue Matrix ---

fn issue_parties(graph: &serde_json::Value) -> Vec<issue_matrix::Party> {
    graph_items(graph, "parties")
        .iter()
        .filter_map(|p| Some(issue_matrix::Party { party_id: p["party_id"].as_str()?.to_string(), display_name: p["display_name"].as_str().unwrap_or("").to_string(), role: p["role"].as_str().unwrap_or("").to_string() }))
        .collect()
}

/// Issues the judge has decided on (or authored), in the order they were first recorded.
fn issue_stored(mgr: &VaultManager, case_id: &str) -> Result<Vec<(u64, issue_matrix::IssueIn)>, String> {
    let mut v: Vec<(u64, issue_matrix::IssueIn)> = deadline_kv(mgr, case_id, "issue:item:")?
        .into_iter()
        .filter_map(|v| Some((v["seq"].as_u64()?, serde_json::from_value(v["issue"].clone()).ok()?)))
        .collect();
    v.sort_by_key(|(s, _)| *s);
    Ok(v)
}

fn issue_put(mgr: &VaultManager, case_id: &str, seq: u64, issue: &issue_matrix::IssueIn) -> Result<(), String> {
    mgr.witness_kv_put(case_id, &format!("issue:item:{}", issue.issue_id), &serde_json::json!({"seq": seq, "issue": issue, "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
}

/// Template and case-graph issues not yet decided, with the matter types the installed templates cover.
fn issue_proposals(mgr: &VaultManager, case_id: &str, paths: &vault::VaultPaths, graph: &serde_json::Value, stored: &[issue_matrix::IssueIn]) -> Result<(Vec<issue_matrix::IssueIn>, Vec<String>, Option<String>), String> {
    let jurisdiction = case_jurisdiction(mgr, case_id)?;
    let (tables, _) = rule_tables::load_installed(paths);
    let matter_types = issue_matrix::matter_types(&tables, &jurisdiction);
    let matter_type = deadline_kv(mgr, case_id, "issue:matter_type")?.into_iter().next().and_then(|v| v["matter_type"].as_str().map(str::to_string));
    let skeletons = matter_type.as_deref().map(|m| issue_matrix::skeletons_for(&tables, &jurisdiction, m)).unwrap_or_default();
    let graph_issues: Vec<(String, String)> = graph_items(graph, "nodes")
        .iter()
        .filter(|n| n["node_type"] == "issue")
        .filter_map(|n| Some((n["node_id"].as_str()?.to_string(), n["label"].as_str()?.to_string())))
        .collect();
    Ok((issue_matrix::propose(stored, &skeletons, &graph_issues), matter_types, matter_type))
}

/// The matrix with everything the screen needs. The filings are the documents counsel assigned to a party; their
/// citations are audited against the installed law so each position shows the audit result of its authorities.
fn issue_matrix_compute(mgr: &VaultManager, case_id: &str) -> Result<serde_json::Value, String> {
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let graph = mgr.case_graph_get(case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let parties = issue_parties(&graph);
    let stored = issue_stored(mgr, case_id)?;
    let decided: Vec<issue_matrix::IssueIn> = stored.iter().map(|(_, i)| i.clone()).collect();
    let (proposed, matter_types, matter_type) = issue_proposals(mgr, case_id, &paths, &graph, &decided)?;
    let mut issues = decided;
    issues.extend(proposed);

    let docs = mgr.doc_list(case_id).map_err(|e| e.to_string())?;
    let mut filings = Vec::new();
    let mut audited = Vec::new();
    let mut authority_info = serde_json::Map::new();
    let mut warnings = Vec::new();
    let store = retrieval::store::open(&paths).ok();
    let small = retrieval::build_small_corpus(&paths, &[]).unwrap_or_default();
    for v in deadline_kv(mgr, case_id, "issue:filing:")? {
        let (Some(doc_id), Some(party_id)) = (v["doc_id"].as_str(), v["party_id"].as_str()) else { continue };
        let Some(meta) = docs.iter().find(|d| d.doc_id == doc_id) else {
            warnings.push(format!("A document assigned to a party is no longer in the matter ({doc_id})."));
            continue;
        };
        let pages = if meta.type_tag == docpipe::EXTERNAL_FILING_TAG {
            docpipe::external_filing_pages(mgr, case_id, doc_id)?.1
        } else {
            docpipe::statement_pages(mgr, case_id, doc_id)?.1
        };
        if let Some(store) = store.as_deref() {
            if let Ok(mut report) = audit::audit_with_store(store, &small, &opposition::filing_text(&pages), &meta.original_filename, "txt", Some(case_id.to_string())) {
                treatment::decorate_report(&paths, store, &mut report);
                for item in report.items.iter().filter(|i| i.resolved.is_some() || i.result == "NOT_IN_CORPUS") {
                    let loc = item.resolved.as_ref();
                    authority_info.insert(
                        format!("{doc_id}:{}", item.item_id),
                        serde_json::json!({
                            "citation": item.draft_text, "result": item.result, "pack_id": loc.map(|l| l.pack_id.clone()), "source_id": loc.map(|l| l.source_id.clone()),
                            "chunk_id": loc.map(|l| l.chunk_id.clone()), "notes": item.notes.iter().map(|n| serde_json::json!({"note_kind": n.note_kind, "text": n.text})).collect::<Vec<_>>(),
                        }),
                    );
                }
                audited.extend(issue_matrix::audited_from_report(doc_id, &pages, &report));
            }
        } else {
            warnings.push("No legal packs are installed, so the authorities in the filings were not checked.".to_string());
        }
        filings.push(issue_matrix::Filing { doc_id: doc_id.to_string(), party_id: party_id.to_string(), pages });
    }
    warnings.dedup();
    let built_at = crate::docpipe::now_rfc3339();
    let out = issue_matrix::build(&issue_matrix::Inputs { case_id, built_at: &built_at, parties: &parties, filings: &filings, issues: &issues, audited: &audited })?;
    warnings.extend(out.warnings);
    let assigned: Vec<serde_json::Value> = filings.iter().map(|f| serde_json::json!({"doc_id": f.doc_id, "party_id": f.party_id, "filename": docs.iter().find(|d| d.doc_id == f.doc_id).map(|d| d.original_filename.clone()).unwrap_or_default()})).collect();
    Ok(serde_json::json!({
        "pipeline_id": issue_matrix::PIPELINE_ID, "matrix": out.matrix, "warnings": warnings, "filings": assigned,
        "docs": docs.iter().map(|d| serde_json::json!({"doc_id": d.doc_id, "filename": d.original_filename, "type_tag": d.type_tag})).collect::<Vec<_>>(),
        "matter_types": matter_types, "matter_type": matter_type, "authority_info": authority_info,
        "note": "A neutral view of the filings. It ranks nothing, scores nothing and suggests no answer; the decisions are the judge's.",
    }))
}

/// The matrix for the matter: accepted issues with each party's pleaded position, agreed and disputed facts, and
/// arguments the other side never answered; undecided issues are listed as proposals.
#[tauri::command]
fn issue_matrix_build(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    edition::require_capability(&ctx, &mgr.vault_paths().map_err(|e| e.to_string())?.root, "issue_matrix")?;
    issue_matrix_compute(&mgr, &case_id)
}

/// Which kind of matter this is, so the installed issue templates for it are proposed.
#[tauri::command]
fn issue_matrix_set_matter_type(app: AppHandle, vault: State<'_, VaultState>, case_id: String, matter_type: Option<String>) -> Result<(), String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    match matter_type {
        None => mgr.witness_kv_delete(&case_id, "issue:matter_type").map_err(|e| e.to_string()),
        Some(m) => {
            let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
            let (tables, _) = rule_tables::load_installed(&paths);
            if !issue_matrix::matter_types(&tables, &case_jurisdiction(&mgr, &case_id)?).contains(&m) {
                return Err("CF_ISSUE_MATTER_TYPE: no installed issue template covers that kind of matter".into());
            }
            mgr.witness_kv_put(&case_id, "issue:matter_type", &serde_json::json!({"matter_type": m})).map_err(|e| e.to_string())
        }
    }
}

/// The judge writes an issue of their own. It starts accepted.
#[tauri::command]
fn issue_matrix_add(app: AppHandle, vault: State<'_, VaultState>, case_id: String, text: String) -> Result<issue_matrix::IssueIn, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    if text.trim().is_empty() {
        return Err("CF_ISSUE_TEXT: write the issue".into());
    }
    let stored = issue_stored(&mgr, &case_id)?;
    let n = stored.iter().filter_map(|(_, i)| i.issue_id.strip_prefix("judge:").and_then(|n| n.parse::<u64>().ok())).max().unwrap_or(0) + 1;
    let seq = stored.iter().map(|(s, _)| *s).max().unwrap_or(0) + 1;
    let issue = issue_matrix::IssueIn { issue_id: format!("judge:{n}"), text: text.trim().to_string(), origin: "JUDGE_AUTHORED".into(), template_rule_id: None, decision: "ACCEPTED".into(), merged_into: None };
    issue_put(&mgr, &case_id, seq, &issue)?;
    Ok(issue)
}

/// The judge accepts, edits, merges or rejects an issue (or sends a decided template/graph issue back to the proposals
/// with PROPOSED). Nothing proceeds into the matrix until the judge has accepted it.
#[tauri::command]
fn issue_matrix_decide(app: AppHandle, vault: State<'_, VaultState>, case_id: String, issue_id: String, decision: String, text: Option<String>, merged_into: Option<String>) -> Result<(), String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    if !["PROPOSED", "ACCEPTED", "EDITED", "MERGED", "REJECTED"].contains(&decision.as_str()) {
        return Err("CF_ISSUE_DECISION: choose accept, edit, merge or reject".into());
    }
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let stored = issue_stored(&mgr, &case_id)?;
    let decided: Vec<issue_matrix::IssueIn> = stored.iter().map(|(_, i)| i.clone()).collect();
    let existing = stored.iter().find(|(_, i)| i.issue_id == issue_id).map(|(s, i)| (*s, i.clone()));
    let (seq, mut issue) = match existing {
        Some(e) => e,
        None => {
            let (proposed, _, _) = issue_proposals(&mgr, &case_id, &paths, &graph, &decided)?;
            let p = proposed.into_iter().find(|i| i.issue_id == issue_id).ok_or_else(|| "CF_ISSUE_UNKNOWN: that issue is not in the matter".to_string())?;
            (stored.iter().map(|(s, _)| *s).max().unwrap_or(0) + 1, p)
        }
    };
    let was_target = matches!(issue.decision.as_str(), "ACCEPTED" | "EDITED");
    let stays_target = matches!(decision.as_str(), "ACCEPTED" | "EDITED");
    if was_target && !stays_target && decided.iter().any(|i| i.decision == "MERGED" && i.merged_into.as_deref() == Some(issue_id.as_str())) {
        return Err("CF_ISSUE_MERGE_TARGET: other issues are merged into this one; send them back first".into());
    }
    match decision.as_str() {
        "PROPOSED" => {
            if issue.origin == "JUDGE_AUTHORED" {
                return Err("CF_ISSUE_DECISION: an issue you wrote is removed by rejecting it".into());
            }
            return mgr.witness_kv_delete(&case_id, &format!("issue:item:{issue_id}")).map_err(|e| e.to_string());
        }
        "EDITED" => {
            let t = text.unwrap_or_default();
            if t.trim().is_empty() {
                return Err("CF_ISSUE_TEXT: write the edited issue".into());
            }
            issue.text = t.trim().to_string();
            issue.merged_into = None;
        }
        "MERGED" => {
            let target = merged_into.unwrap_or_default();
            let ok = target != issue_id && decided.iter().any(|i| i.issue_id == target && matches!(i.decision.as_str(), "ACCEPTED" | "EDITED"));
            if !ok {
                return Err("CF_ISSUE_MERGE_TARGET: merge into an issue you have accepted".into());
            }
            issue.merged_into = Some(target);
        }
        _ => issue.merged_into = None,
    }
    issue.decision = decision;
    issue_put(&mgr, &case_id, seq, &issue)
}

/// Counsel (or the judge's staff) says whose filing a document is, or clears it with None.
#[tauri::command]
fn issue_matrix_set_filing(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_id: String, party_id: Option<String>) -> Result<(), String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    let key = format!("issue:filing:{doc_id}");
    let Some(party_id) = party_id else { return mgr.witness_kv_delete(&case_id, &key).map_err(|e| e.to_string()) };
    if !mgr.doc_list(&case_id).map_err(|e| e.to_string())?.iter().any(|d| d.doc_id == doc_id) {
        return Err("CF_ISSUE_DOC_UNKNOWN: that document is not in the matter".into());
    }
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    if !issue_parties(&graph).iter().any(|p| p.party_id == party_id) {
        return Err("CF_ISSUE_PARTY_UNKNOWN: that party is not in the matter".into());
    }
    mgr.witness_kv_put(&case_id, &key, &serde_json::json!({"doc_id": doc_id, "party_id": party_id, "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
}

/// The bench memo as text, with the G4 footer and G5 provenance. Only accepted issues appear.
#[tauri::command]
fn issue_matrix_memo(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<String, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    edition::require_capability(&ctx, &mgr.vault_paths().map_err(|e| e.to_string())?.root, "memo_export")?;
    let computed = issue_matrix_compute(&mgr, &case_id)?;
    Ok(issue_matrix::bench_memo(&computed["matrix"], &crate::docpipe::now_rfc3339()))
}

// --- B-S4-11 Deadline & Limitation Watch ---

fn deadline_rules(tables: &[rule_tables::RuleTable], jurisdiction: &str) -> (std::collections::BTreeMap<String, Vec<deadlines::Rule>>, Vec<String>) {
    let mut by_rule: std::collections::BTreeMap<String, Vec<deadlines::Rule>> = std::collections::BTreeMap::new();
    let mut problems = Vec::new();
    for t in tables.iter().filter(|t| matches!(t.table_kind.as_str(), "LIMITATION_PERIOD" | "PROCEDURAL_DEADLINE") && t.jurisdiction.eq_ignore_ascii_case(jurisdiction)) {
        for row in &t.rows {
            match deadlines::Rule::from_row(row) {
                Ok(rule) => by_rule.entry(rule.rule_id.clone()).or_default().push(rule),
                Err(e) => problems.push(e),
            }
        }
    }
    (by_rule, problems)
}

fn deadline_calendars(tables: &[rule_tables::RuleTable]) -> deadlines::Calendars {
    let mut cals = deadlines::Calendars::default();
    for t in tables.iter().filter(|t| t.table_kind == "COURT_CALENDAR") {
        for row in &t.rows {
            let _ = cals.add(&t.table_id, &row.payload);
        }
    }
    cals
}

fn deadline_kv(mgr: &VaultManager, case_id: &str, prefix: &str) -> Result<Vec<serde_json::Value>, String> {
    Ok(mgr.witness_kv_list(case_id, prefix).map_err(|e| e.to_string())?.into_iter().map(|(_, v)| v).collect())
}

/// Deadlines for the matter: the rules installed for its jurisdiction, the chronology events counsel can choose as
/// triggers, counsel's assignments and exclusion facts, and the computed records (or the reason one cannot be made).
#[tauri::command]
fn deadlines_compute(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    edition::require_feature("deadline_watch")?;
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let jurisdiction = case_jurisdiction(&mgr, &case_id)?;
    let (tables, rejected) = rule_tables::load_installed(&paths);
    let (by_rule, rule_problems) = deadline_rules(&tables, &jurisdiction);
    let cals = deadline_calendars(&tables);
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let events: Vec<serde_json::Value> = graph["chronology"].as_array().cloned().unwrap_or_default();
    let assignments = deadline_kv(&mgr, &case_id, "deadline:assign:")?;
    let fact_values = deadline_kv(&mgr, &case_id, "deadline:fact:")?;
    let facts: Vec<deadlines::Fact> = fact_values
        .iter()
        .filter_map(|f| {
            Some(deadlines::Fact {
                fact_type: f["fact_type"].as_str()?.to_string(),
                node_id: f["node_id"].as_str()?.to_string(),
                days: f["days"].as_u64().map(|d| d as u32),
                confirmed: f["confirmed"].as_bool().unwrap_or(false),
                locator: deadlines::Locator { doc_id: f["doc_id"].as_str().unwrap_or("").to_string(), excerpt: f["excerpt"].as_str().unwrap_or("").to_string() },
            })
        })
        .collect();
    let now = crate::docpipe::now_rfc3339();
    let (mut records, mut errors) = (Vec::new(), Vec::new());
    for a in &assignments {
        let (rule_id, event_id) = (a["rule_id"].as_str().unwrap_or(""), a["event_id"].as_str().unwrap_or(""));
        let (Some(rules), Some(event)) = (by_rule.get(rule_id), events.iter().find(|e| e["event_id"] == event_id)) else {
            errors.push(serde_json::json!({"rule_id": rule_id, "event_id": event_id, "error": "CF_DEADLINE_ASSIGNMENT_STALE: the rule or the chronology event is no longer available"}));
            continue;
        };
        let trigger = deadlines::trigger_from_event(event, &rules[0].trigger_event_type);
        match deadlines::compute(&deadlines::Inputs { case_id: &case_id, rules, trigger: &trigger, facts: &facts, calendars: &cals, computed_at: &now }) {
            Ok(r) => records.push(r),
            Err(e) => errors.push(serde_json::json!({"rule_id": rule_id, "event_id": event_id, "error": e})),
        }
    }
    let rule_list: Vec<serde_json::Value> = by_rule
        .values()
        .map(|rs| {
            let r = &rs[0];
            serde_json::json!({
                "rule_id": r.rule_id, "label": r.label, "trigger_event_type": r.trigger_event_type, "period": {"value": r.value, "unit": format!("{:?}", r.unit).to_uppercase()},
                "exclude_trigger_day": r.exclude_trigger_day, "roll_forward": r.roll_forward, "calendar_table_id": r.calendar_table_id,
                "trust_state": r.trust_state, "provision": r.authorities.first().and_then(|a| a["provision_label"].as_str()),
                "exclusions": r.exclusions.iter().map(|e| serde_json::json!({"exclusion_id": e.id, "fact_type": e.fact_type})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let event_list: Vec<serde_json::Value> = events
        .iter()
        .map(|e| {
            let t = deadlines::trigger_from_event(e, "");
            serde_json::json!({
                "event_id": e["event_id"], "summary": e["summary"], "date": e["date"], "precision": t.precision, "date_range": t.range, "other_dates": t.conflicting,
                "doc_id": t.locator.doc_id, "excerpt": t.locator.excerpt,
            })
        })
        .collect();
    Ok(serde_json::json!({
        "engine_version": deadlines::ENGINE_VERSION, "jurisdiction": jurisdiction, "rules": rule_list, "events": event_list,
        "assignments": assignments, "facts": fact_values, "records": records, "errors": errors,
        "rule_problems": rule_problems, "rejected_tables": rejected,
        "calendar_tables": tables.iter().filter(|t| t.table_kind == "COURT_CALENDAR").map(|t| t.table_id.clone()).collect::<Vec<_>>(),
        "unsigned_rules": rule_list.iter().any(|r| r["trust_state"] != "SIGNED_OFF"),
    }))
}

/// Counsel chooses which chronology event starts a rule's period.
#[tauri::command]
fn deadlines_assign(app: AppHandle, vault: State<'_, VaultState>, case_id: String, rule_id: String, event_id: String) -> Result<(), String> {
    edition::require_feature("deadline_watch")?;
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let (tables, _) = rule_tables::load_installed(&paths);
    let (by_rule, _) = deadline_rules(&tables, &case_jurisdiction(&mgr, &case_id)?);
    if !by_rule.contains_key(&rule_id) {
        return Err("CF_DEADLINE_RULE_UNKNOWN: that rule is not installed for this matter's jurisdiction".into());
    }
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    if !graph["chronology"].as_array().map(|a| a.iter().any(|e| e["event_id"] == event_id.as_str())).unwrap_or(false) {
        return Err("CF_DEADLINE_EVENT_UNKNOWN: that event is not in the matter's chronology".into());
    }
    mgr.witness_kv_put(&case_id, &format!("deadline:assign:{rule_id}:{event_id}"), &serde_json::json!({"rule_id": rule_id, "event_id": event_id, "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
}

#[tauri::command]
fn deadlines_unassign(app: AppHandle, vault: State<'_, VaultState>, case_id: String, rule_id: String, event_id: String) -> Result<(), String> {
    edition::require_feature("deadline_watch")?;
    vault.get_or_init(&app)?.witness_kv_delete(&case_id, &format!("deadline:assign:{rule_id}:{event_id}")).map_err(|e| e.to_string())
}

/// Counsel records a fact that excludes time (for example the days taken to obtain a copy): how many days, whether it is
/// confirmed, and where it is recorded. Applied only when confirmed; an unconfirmed fact turns the result into a range.
#[tauri::command]
fn deadlines_fact_save(app: AppHandle, vault: State<'_, VaultState>, case_id: String, fact_type: String, days: u32, confirmed: bool, doc_id: String, excerpt: String) -> Result<(), String> {
    edition::require_feature("deadline_watch")?;
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let (tables, _) = rule_tables::load_installed(&paths);
    let (by_rule, _) = deadline_rules(&tables, &case_jurisdiction(&mgr, &case_id)?);
    if !by_rule.values().flatten().any(|r| r.exclusions.iter().any(|e| e.fact_type == fact_type)) {
        return Err("CF_DEADLINE_FACT_UNKNOWN: no installed rule excludes time for that kind of fact".into());
    }
    if doc_id.trim().is_empty() || excerpt.trim().is_empty() || days > 3650 {
        return Err("CF_DEADLINE_FACT_INVALID: give the number of days (up to 3650) and where the fact is recorded".into());
    }
    mgr.witness_kv_put(&case_id, &format!("deadline:fact:{fact_type}"), &serde_json::json!({"fact_type": fact_type, "node_id": format!("counsel_fact:{fact_type}"), "days": days, "confirmed": confirmed, "doc_id": doc_id.trim(), "excerpt": excerpt.trim(), "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
}

#[tauri::command]
fn deadlines_fact_remove(app: AppHandle, vault: State<'_, VaultState>, case_id: String, fact_type: String) -> Result<(), String> {
    edition::require_feature("deadline_watch")?;
    vault.get_or_init(&app)?.witness_kv_delete(&case_id, &format!("deadline:fact:{fact_type}")).map_err(|e| e.to_string())
}

/// The computed deadlines as an iCalendar file (local; nothing is sent anywhere).
#[tauri::command]
fn deadlines_export_ics(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<String, String> {
    edition::require_feature("deadline_watch")?;
    let all = deadlines_compute(app, vault, case_id)?;
    let records = all["records"].as_array().cloned().unwrap_or_default();
    if records.is_empty() {
        return Err("CF_DEADLINE_NOTHING_TO_EXPORT: no deadline has been computed for this matter yet".into());
    }
    Ok(deadlines::to_ics(&records, &crate::docpipe::now_rfc3339()))
}

// --- B-S4-28 step-by-step reasoning with best-of-n verification ---

/// The reasoning loop shared by counsel's reasoning and the Chambers record-only reasoning. In Chambers the model sees
/// passages of the record (instead of the case graph) and holds the record-only permit; nothing the question says is a source.
struct ReasoningRun {
    result: reasoning::ReasoningResult,
    sources: Vec<proposition::SupportSource>,
    status: infer_llm::LlmStatus,
    elapsed_ms: u64,
    coverage: Option<serde_json::Value>,
}

fn reasoning_core(mgr: &VaultManager, case_id: &str, question: &str, depth: reasoning::Depth, chambers: bool) -> Result<ReasoningRun, String> {
    if guardrails::check_user_input(question).refused {
        return Err("CF_REASONING_INPUT_REFUSED: safety policy blocked the question".into());
    }
    let status = infer_llm::status();
    if !status.controlled_beta_ready {
        return Err(format!("CF_REASONING_MODEL_NOT_READY: {}", status.detail));
    }
    let profile = mgr
        .list_cases()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|c| c.case_id == case_id)
        .ok_or_else(|| "CF_CASE_NOT_FOUND: matter is not registered".to_string())?;
    let graph = mgr.case_graph_get(case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let set = mgr.working_set_get(case_id).map_err(|e| e.to_string())?;
    let readiness = mock_court_retrieval::readiness(case_id, &profile.jurisdiction, Some(&graph), &set);
    let retrieval = mock_court_retrieval::retrieve(&readiness, "oral_argument", "counsel", question, &graph)?;
    let (sources, coverage) = if chambers {
        let (s, c) = chambers_record_sources(mgr, case_id, question, &retrieval.authorities)?;
        (s, Some(c))
    } else {
        (grounded_mock_court::support_sources(case_id, &graph, &readiness.graph_version, question, &retrieval.authorities), None)
    };
    let permitted: Vec<String> = retrieval.authorities.iter().map(|a| a.authority_id.clone()).collect();
    let authority_context = retrieval
        .authorities
        .iter()
        .map(|a| format!("[{} | {} | {}] {}", a.authority_id, a.citation, a.section_label, a.exact_text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let started = std::time::Instant::now();
    let budget_ms = depth.budget_ms();
    let mut clock = || started.elapsed().as_millis() as u64;
    let mut generate = |attempt: reasoning::Attempt| -> Result<Vec<proposition::PropositionInput>, String> {
        let (user_text, variant) = match &attempt.repair {
            Some(note) => (reasoning::repair_prompt(question, note.previous, &note.problems), 0),
            None => (reasoning::prompt(question), attempt.variant.min(7)),
        };
        let left_s = (budget_ms.saturating_sub(started.elapsed().as_millis() as u64) / 1000).clamp(30, 300);
        let request = infer_llm::LlmChatRequest {
            user_text,
            role: Some("counsel".into()),
            format: Some("reasoning_steps".into()),
            case_title: Some(profile.cause_title.clone()),
            jurisdiction: Some(profile.jurisdiction.clone()),
            user_side: Some(if chambers { String::new() } else { profile.user_side.clone() }),
            case_context: None,
            authority_context: Some(authority_context.clone()),
            authority_ids: Some(permitted.clone()),
            working_set_version: Some(readiness.working_set_version.to_string()),
            structured_propositions: Some(true),
            output_kind: Some("REASONING".into()),
            support_sources: Some(sources.clone()),
            permitted_authority_ids: Some(permitted.clone()),
            mock_court_schema_v2: None,
            messages: None,
            max_tokens: Some(1024),
            variant: Some(variant),
        };
        let out = infer_llm::chat_with_timeout(request, left_s)?;
        if !out.controlled_beta_ready {
            return Err("CF_REASONING_MODEL_DRIFT: generation did not report controlled-beta readiness".into());
        }
        Ok(out.propositions)
    };
    // B-S4-17: in Chambers this is the only road to the model, and it ends at the G7 classifier.
    let _permit = chambers.then(edition::record_only_permit);
    let result = reasoning::run(&sources, &permitted, depth, budget_ms, &mut clock, &mut generate);
    Ok(ReasoningRun { result, sources, status, elapsed_ms: started.elapsed().as_millis() as u64, coverage })
}

/// Reasons through a question over the matter's working set: several candidate step-by-step ledgers from the local model,
/// each checked by the host verifier, the best-verified one kept, one targeted repair. A result is called verified only
/// when the host verified every step. Needs a counsel-confirmed working set, like Mock Court.
#[tauri::command]
fn reasoning_run(app: AppHandle, vault: State<'_, VaultState>, case_id: String, question: String, depth: Option<String>) -> Result<serde_json::Value, String> {
    let question = question.trim().to_string();
    if question.is_empty() || question.chars().count() > 2000 {
        return Err("CF_REASONING_QUESTION: enter a question of up to 2000 characters".into());
    }
    let depth = reasoning::Depth::parse(depth.as_deref().unwrap_or("CAREFUL")).ok_or_else(|| "CF_REASONING_DEPTH: use QUICK, CAREFUL or THOROUGH".to_string())?;
    let mgr = vault.get_or_init(&app)?;
    edition::require_advocacy(&edition_of(&mgr)?)?; // Chambers reasons through chambers_reason, which is confined to the record
    let run = reasoning_core(&mgr, &case_id, &question, depth, false)?;
    let guarded = guardrails::check_model_output(&question, &run.result.rendered_text, &run.status.model, &[]);
    if guarded.refused {
        return Err("CF_REASONING_OUTPUT_REFUSED: the verified reasoning still failed the safety policy".into());
    }
    let output_id = format!("reasoning:{}:{}", case_id, crate::docpipe::now_rfc3339());
    mgr.working_set_select_for_output(&case_id, "REASONING", &output_id).map_err(|e| e.to_string())?;
    let mut out = serde_json::to_value(&run.result).map_err(|e| e.to_string())?;
    out["question"] = serde_json::json!(question);
    out["model"] = serde_json::json!({"model": run.status.model, "backend": run.status.backend, "digest": run.status.model_digest});
    out["elapsed_ms"] = serde_json::json!(run.elapsed_ms);
    out["disclaimer"] = serde_json::json!("Preparation material only. Not legal advice. Each verified step shows the supplied source it copies from; counsel inspects every source.");
    Ok(out)
}

// --- B-S4-17 Chambers Edition shell and record-only mode (G7) ---

fn edition_of(mgr: &VaultManager) -> Result<edition::EditionContext, String> {
    mgr.refresh_edition().map_err(|e| e.to_string())
}

/// What the licence entitles, the screens to show and hide, and the court's pilot configuration.
#[tauri::command]
fn edition_status(app: AppHandle, vault: State<'_, VaultState>) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    Ok(edition::status_json(&ctx, &paths.root))
}

/// Record-only is on unless the judge switched it off, and always on once a judgment scaffold exists.
fn chambers_record_only(mgr: &VaultManager, case_id: &str) -> Result<(bool, bool), String> {
    let scaffold = !deadline_kv(mgr, case_id, "scaffold:")?.is_empty();
    let stored = deadline_kv(mgr, case_id, "chambers:record_only")?.first().and_then(|v| v["on"].as_bool()).unwrap_or(true);
    Ok((stored || scaffold, scaffold))
}

#[tauri::command]
fn chambers_status(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    let (on, scaffold) = chambers_record_only(&mgr, &case_id)?;
    let withheld = deadline_kv(&mgr, &case_id, "chambers:g7:")?.len();
    let filings = deadline_kv(&mgr, &case_id, "issue:filing:")?.len();
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let caps: serde_json::Map<String, serde_json::Value> = edition::PILOT_CAPABILITIES.iter().map(|c| (c.to_string(), serde_json::json!(edition::require_capability(&ctx, &paths.root, c).is_ok()))).collect();
    Ok(serde_json::json!({
        "record_only": on, "scaffold_exists": scaffold, "locked": scaffold, "withheld_total": withheld, "filings_assigned": filings, "capabilities": caps,
        "classes": record_only::CLASSES,
        "note": "Record-only: every statement shown is tied to the filings, the evidence and orders on the record, or installed law. Anything else is withheld.",
    }))
}

/// Switches record-only mode on or off for a matter. Off is refused once a judgment scaffold exists.
#[tauri::command]
fn chambers_set_record_only(app: AppHandle, vault: State<'_, VaultState>, case_id: String, enabled: bool) -> Result<(), String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    let (_, scaffold) = chambers_record_only(&mgr, &case_id)?;
    record_only::check_change(scaffold, enabled)?;
    mgr.witness_kv_put(&case_id, "chambers:record_only", &serde_json::json!({"on": enabled, "at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())
}

/// Statements withheld by record-only mode: when, which, why, and a hash of the text (never the text).
#[tauri::command]
fn chambers_g7_log(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<Vec<serde_json::Value>, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    deadline_kv(&mgr, &case_id, "chambers:g7:")
}

/// The page texts of one document of the matter (an opponent's filing is read through its own gate).
fn matter_pages(mgr: &VaultManager, case_id: &str, d: &vault::DocumentMeta) -> Result<Vec<String>, String> {
    Ok(if d.type_tag == docpipe::EXTERNAL_FILING_TAG { docpipe::external_filing_pages(mgr, case_id, &d.doc_id)?.1 } else { docpipe::statement_pages(mgr, case_id, &d.doc_id)?.1 })
}

// --- B-S4-21 Judgment Scaffold and the decision-free zone (G8) ---

struct ScaffoldWorld {
    profile: (String, String, String, String),
    parties: Vec<issue_matrix::Party>,
    filings: Vec<issue_matrix::Filing>,
    docs: Vec<cause_list::MatterDoc>,
    issues: Vec<issue_matrix::IssueIn>,
    evidence: Vec<judgment_scaffold::EvidenceItem>,
    matrix: serde_json::Value,
}

fn scaffold_world(mgr: &VaultManager, case_id: &str) -> Result<ScaffoldWorld, String> {
    let c = mgr.list_cases().map_err(|e| e.to_string())?.into_iter().find(|c| c.case_id == case_id).ok_or_else(|| "CF_CASE_NOT_FOUND: matter is not registered".to_string())?;
    let graph = mgr.case_graph_get(case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let parties = issue_parties(&graph);
    let list = mgr.doc_list(case_id).map_err(|e| e.to_string())?;
    let docs: Vec<cause_list::MatterDoc> = list.iter().map(|d| Ok(cause_list::MatterDoc { doc_id: d.doc_id.clone(), filename: d.original_filename.clone(), type_tag: d.type_tag.clone(), pages: matter_pages(mgr, case_id, d)? })).collect::<Result<_, String>>()?;
    let mut filings = Vec::new();
    for v in deadline_kv(mgr, case_id, "issue:filing:")? {
        if let (Some(doc), Some(party)) = (v["doc_id"].as_str(), v["party_id"].as_str()) {
            if let Some(d) = docs.iter().find(|d| d.doc_id == doc) {
                filings.push(issue_matrix::Filing { doc_id: doc.to_string(), party_id: party.to_string(), pages: d.pages.clone() });
            }
        }
    }
    let issues: Vec<issue_matrix::IssueIn> = issue_stored(mgr, case_id)?.into_iter().map(|(_, i)| i).collect();
    let matrix = issue_matrix::build(&issue_matrix::Inputs { case_id, built_at: &crate::docpipe::now_rfc3339(), parties: &parties, filings: &filings, issues: &issues, audited: &[] }).map(|o| o.matrix).unwrap_or_else(|_| serde_json::json!({"issues": []}));
    let evidence = graph_items(&graph, "evidence").iter().filter_map(|e| Some(judgment_scaffold::EvidenceItem { evidence_id: e["evidence_id"].as_str()?.to_string(), label: e["label"].as_str().unwrap_or("").to_string(), doc_id: e["doc_id"].as_str().map(str::to_string) })).collect();
    Ok(ScaffoldWorld { profile: (c.cause_title, c.court, c.label, c.jurisdiction), parties, filings, docs, issues, evidence, matrix })
}

fn scaffold_prefill_of(w: &ScaffoldWorld) -> std::collections::BTreeMap<&'static str, Vec<serde_json::Value>> {
    judgment_scaffold::prefill(&judgment_scaffold::Inputs { cause_title: &w.profile.0, court: &w.profile.1, label: &w.profile.2, jurisdiction: &w.profile.3, parties: &w.parties, filings: &w.filings, docs: &w.docs, issues: &w.issues, evidence: &w.evidence, matrix: &w.matrix })
}

fn scaffold_stored(mgr: &VaultManager, case_id: &str) -> Result<Option<serde_json::Value>, String> {
    Ok(deadline_kv(mgr, case_id, "scaffold:main")?.into_iter().next())
}

fn scaffold_put(mgr: &VaultManager, case_id: &str, sc: &serde_json::Value) -> Result<(), String> {
    mgr.witness_kv_put(case_id, "scaffold:main", sc).map_err(|e| e.to_string())
}

fn scaffold_report(mgr: &VaultManager, case_id: &str, sc: Option<serde_json::Value>) -> Result<serde_json::Value, String> {
    let w = scaffold_world(mgr, case_id)?;
    let problems = match &sc {
        Some(s) => judgment_scaffold::validate(s, &w.docs, &judgment_scaffold::Profile { cause_title: &w.profile.0, court: &w.profile.1, label: &w.profile.2 }).err().unwrap_or_default(),
        None => vec![],
    };
    let names: serde_json::Map<String, serde_json::Value> = w.docs.iter().map(|d| (d.doc_id.clone(), serde_json::json!(d.filename))).collect();
    Ok(serde_json::json!({"scaffold": sc, "exists": sc.is_some(), "problems": problems, "documents": names, "locked_sections": judgment_scaffold::LOCKED,
        "note": "Analysis, findings and the operative order are written only by the judge: CaseForge has no way to draft them, and nothing you type there is sent to any model."}))
}

#[tauri::command]
fn scaffold_get(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    let sc = scaffold_stored(&mgr, &case_id)?;
    scaffold_report(&mgr, &case_id, sc)
}

/// Creates the scaffold from the record, or refreshes the pre-filled sections of an existing one (the judge's own text stays).
/// Once a scaffold exists, record-only mode is locked on for the matter.
#[tauri::command]
fn scaffold_prefill(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    let w = scaffold_world(&mgr, &case_id)?;
    let pre = scaffold_prefill_of(&w);
    let sc = match scaffold_stored(&mgr, &case_id)? {
        Some(mut s) => {
            judgment_scaffold::refresh(&mut s, &pre)?;
            s
        }
        None => judgment_scaffold::new_scaffold(&case_id, &crate::docpipe::now_rfc3339(), Some(format!("im:{case_id}")), &pre),
    };
    scaffold_put(&mgr, &case_id, &sc)?;
    scaffold_report(&mgr, &case_id, Some(sc))
}

/// The judge's own text for a section. Stored as written; no model, no filter, no suggestion.
#[tauri::command]
fn scaffold_set_text(app: AppHandle, vault: State<'_, VaultState>, case_id: String, section_kind: String, text: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    let mut sc = scaffold_stored(&mgr, &case_id)?.ok_or("CF_SCAFFOLD_NONE: create the scaffold from the record first")?;
    judgment_scaffold::set_judge_text(&mut sc, &section_kind, &text)?;
    scaffold_put(&mgr, &case_id, &sc)?;
    scaffold_report(&mgr, &case_id, Some(sc))
}

#[tauri::command]
fn scaffold_remove_sentence(app: AppHandle, vault: State<'_, VaultState>, case_id: String, section_kind: String, index: usize) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::require_chambers(&edition_of(&mgr)?)?;
    let mut sc = scaffold_stored(&mgr, &case_id)?.ok_or("CF_SCAFFOLD_NONE: create the scaffold from the record first")?;
    judgment_scaffold::remove_sentence(&mut sc, &section_kind, index)?;
    scaffold_put(&mgr, &case_id, &sc)?;
    scaffold_report(&mgr, &case_id, Some(sc))
}

/// Citation audit of the judge's own text in a section: allowed help, it checks and generates nothing.
#[tauri::command]
fn scaffold_audit_text(app: AppHandle, vault: State<'_, VaultState>, case_id: String, section_kind: String) -> Result<audit::AuditReport, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    edition::require_capability(&ctx, &paths.root, "citation_audit")?;
    let sc = scaffold_stored(&mgr, &case_id)?.ok_or("CF_SCAFFOLD_NONE: create the scaffold from the record first")?;
    let text = judgment_scaffold::judge_text(&sc, &section_kind);
    if text.trim().is_empty() {
        return Err("CF_AUDIT_EXTRACTION_FAILED: there is no text of yours in that section to check".into());
    }
    let store = retrieval::store::open(&paths)?;
    let small = retrieval::build_small_corpus(&paths, &[])?;
    let mut report = audit::audit_with_store(&store, &small, &text, "judge_text.txt", "txt", Some(case_id))?;
    treatment::decorate_report(&paths, &store, &mut report);
    Ok(report)
}

/// The scaffold as text, with the export footer and a statement of which sections were filled from the record.
#[tauri::command]
fn scaffold_export(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<String, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    edition::require_capability(&ctx, &mgr.vault_paths().map_err(|e| e.to_string())?.root, "memo_export")?;
    let sc = scaffold_stored(&mgr, &case_id)?.ok_or("CF_SCAFFOLD_NONE: create the scaffold from the record first")?;
    let names: std::collections::HashMap<String, String> = mgr.doc_list(&case_id).map_err(|e| e.to_string())?.into_iter().map(|d| (d.doc_id, d.original_filename)).collect();
    Ok(judgment_scaffold::export_text(&sc, &names, &crate::docpipe::now_rfc3339()))
}

// --- B-S4-20 Daily Cause-List Brief ---

#[derive(Deserialize)]
struct CauseListFile {
    /// CSV, PDF or MANUAL (typed lines).
    format: String,
    file_name: Option<String>,
    content_base64: Option<String>,
    text: Option<String>,
}

/// Reads a cause list from a file (CSV, PDF, text) or typed lines into listed matters. Nothing is sent anywhere.
#[tauri::command]
fn cause_list_import(app: AppHandle, vault: State<'_, VaultState>, request: CauseListFile) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    edition::require_capability(&ctx, &mgr.vault_paths().map_err(|e| e.to_string())?.root, "cause_list")?;
    use base64::Engine as _;
    let bytes = match (&request.content_base64, &request.text) {
        (Some(b), _) => Some(base64::engine::general_purpose::STANDARD.decode(b).map_err(|_| "CF_CAUSELIST_FILE: the file could not be read".to_string())?),
        _ => None,
    };
    let name = request.file_name.clone().unwrap_or_else(|| "cause_list".into());
    let (format, entries, sha) = match request.format.as_str() {
        "CSV" => {
            let b = bytes.ok_or("CF_CAUSELIST_FILE: choose the CSV file")?;
            ("CSV", cause_list::parse_csv(&String::from_utf8_lossy(&b))?, Some(cause_list::sha256_hex(&b)))
        }
        "PDF" => {
            let b = bytes.ok_or("CF_CAUSELIST_FILE: choose the file")?;
            let lower = name.to_ascii_lowercase();
            let text = if lower.ends_with(".txt") || lower.ends_with(".csv") { String::from_utf8_lossy(&b).to_string() } else { docpipe::bytes_page_texts(&b, if lower.ends_with(".pdf") { "pdf" } else { "docx" }, &name)?.join("\n") };
            let entries = if lower.ends_with(".csv") { cause_list::parse_csv(&text)? } else { cause_list::parse_lines(&text) };
            ("PDF", entries, Some(cause_list::sha256_hex(&b)))
        }
        "MANUAL" => ("MANUAL", cause_list::parse_lines(request.text.as_deref().unwrap_or("")), None),
        _ => return Err("CF_CAUSELIST_FILE: choose CSV, PDF or typed lines".into()),
    };
    if entries.is_empty() {
        return Err("CF_CAUSELIST_EMPTY: no listed matter was found. Each matter should be on its own line starting with its serial number, e.g. \"1. Ravi Kumar v. Anil Kumar CS 112/2023\"".into());
    }
    Ok(serde_json::json!({"format": format, "sha256": sha, "entries": entries}))
}

/// One brief per listed matter. A listing that cannot be told between matters waits for the judge's choice
/// (`confirmations`: serial -> matter). Matters whose vault is not open cannot be read; they are named so they can be opened.
#[tauri::command]
fn cause_list_briefs(app: AppHandle, vault: State<'_, VaultState>, court_label: String, list_date: String, format: String, sha256: Option<String>, entries: Vec<cause_list::ListEntry>, confirmations: Option<std::collections::BTreeMap<String, String>>) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    edition::require_capability(&ctx, &mgr.vault_paths().map_err(|e| e.to_string())?.root, "cause_list")?;
    let format: &'static str = match format.as_str() { "CSV" => "CSV", "PDF" => "PDF", _ => "MANUAL" };
    let mut matters = Vec::new();
    let mut locked = Vec::new();
    for c in mgr.list_cases().map_err(|e| e.to_string())? {
        let mut docs = Vec::new();
        let mut parties = Vec::new();
        if c.is_open {
            if let Ok(list) = mgr.doc_list(&c.case_id) {
                for d in &list {
                    docs.push(cause_list::MatterDoc { doc_id: d.doc_id.clone(), filename: d.original_filename.clone(), type_tag: d.type_tag.clone(), pages: matter_pages(&mgr, &c.case_id, d).unwrap_or_default() });
                }
            }
            parties = issue_parties(&mgr.case_graph_get(&c.case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({})));
        } else {
            locked.push(c.case_id.clone());
        }
        matters.push(cause_list::Matter { case_id: c.case_id, label: c.label, cause_title: c.cause_title, jurisdiction: c.jurisdiction, parties, docs });
    }
    let info = cause_list::ImportInfo { format, sha256 };
    let (doc, candidates) = cause_list::render(&court_label, &list_date, &info, &entries, &matters, &confirmations.unwrap_or_default(), &crate::docpipe::now_rfc3339());
    let titles: serde_json::Map<String, serde_json::Value> = matters.iter().map(|m| (m.case_id.clone(), serde_json::json!(if m.cause_title.is_empty() { &m.label } else { &m.cause_title }))).collect();
    // matters named by a brief that could not be read
    let unreadable: Vec<&String> = doc["entries"].as_array().into_iter().flatten().filter_map(|e| e["match"]["case_id"].as_str()).filter_map(|id| locked.iter().find(|l| l.as_str() == id)).collect();
    let documents: serde_json::Map<String, serde_json::Value> = matters.iter().flat_map(|m| m.docs.iter()).map(|d| (d.doc_id.clone(), serde_json::json!(d.filename))).collect();
    let party_names: serde_json::Map<String, serde_json::Value> = matters.iter().flat_map(|m| m.parties.iter()).map(|p| (p.party_id.clone(), serde_json::json!(format!("{} ({})", p.display_name, p.role)))).collect();
    Ok(serde_json::json!({
        "brief_list": doc, "candidates": candidates, "matter_titles": titles, "matters_not_open": unreadable, "documents": documents, "parties": party_names,
        "note": "Every line of a brief cites the order or document it comes from, or says it was not found in the record.",
    }))
}

// --- B-S4-19 Both-sides citation check and record lookup ---

/// One action: the Citation Audit over every filing of every party, grouped by party in the order the parties are listed.
/// Nothing ranks the parties against each other; each filing's results stand on their own.
#[tauri::command]
fn chambers_audit_all(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    edition::require_capability(&ctx, &paths.root, "citation_audit")?;
    let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
    let parties = issue_parties(&graph);
    let docs = mgr.doc_list(&case_id).map_err(|e| e.to_string())?;
    let assigned: Vec<(String, String)> = deadline_kv(&mgr, &case_id, "issue:filing:")?.iter().filter_map(|v| Some((v["doc_id"].as_str()?.to_string(), v["party_id"].as_str()?.to_string()))).collect();
    let store = retrieval::store::open(&paths).ok();
    let small = retrieval::build_small_corpus(&paths, &[]).unwrap_or_default();
    let mut warnings: Vec<String> = Vec::new();
    if store.is_none() {
        warnings.push("No legal packs are installed, so no citation could be checked.".into());
    }
    let mut out_parties = Vec::new();
    let mut snapshots: std::collections::BTreeMap<String, audit::PackSnapshot> = std::collections::BTreeMap::new();
    for p in &parties {
        let mut filings = Vec::new();
        let mut totals: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
        for d in docs.iter().filter(|d| assigned.iter().any(|(doc, party)| doc == &d.doc_id && party == &p.party_id)) {
            let pages = matter_pages(&mgr, &case_id, d)?;
            let report = match store.as_deref() {
                Some(store) => audit::audit_with_store(store, &small, &opposition::filing_text(&pages), &d.original_filename, "txt", Some(case_id.clone())).ok().map(|mut r| {
                    treatment::decorate_report(&paths, store, &mut r);
                    r
                }),
                None => None,
            };
            if let Some(r) = &report {
                for (k, n) in &r.counts {
                    *totals.entry(k.clone()).or_default() += n;
                }
                for s in &r.pack_snapshot {
                    snapshots.entry(s.pack_id.clone()).or_insert_with(|| s.clone());
                }
            }
            filings.push(serde_json::json!({
                "doc_id": d.doc_id, "filename": d.original_filename,
                "counts": report.as_ref().map(|r| r.counts.clone()).unwrap_or_default(),
                "items": report.as_ref().map(|r| r.items.clone()).unwrap_or_default(),
                "audited": report.is_some(),
            }));
        }
        out_parties.push(serde_json::json!({"party_id": p.party_id, "display_name": p.display_name, "role": p.role, "filings": filings, "totals": totals}));
    }
    let unassigned: Vec<String> = docs.iter().filter(|d| !assigned.iter().any(|(doc, _)| doc == &d.doc_id)).map(|d| d.original_filename.clone()).collect();
    if out_parties.iter().all(|p| p["filings"].as_array().map(|f| f.is_empty()).unwrap_or(true)) {
        warnings.push("No document is assigned to a party yet. Assign each party's filings in the Issue Matrix first.".into());
    }
    Ok(serde_json::json!({
        "parties": out_parties, "unassigned_documents": unassigned, "warnings": warnings, "pack_snapshot": snapshots.values().collect::<Vec<_>>(),
        "note": "The same check is run on every filing. Results are listed by party in the order the parties are named; nothing here ranks one party against another.",
    }))
}

/// "Where in the record ...?": ranked verbatim excerpts of the matter's own documents, each with its document, page and
/// paragraph, and nothing else - no summary, no answer, no model.
#[tauri::command]
fn chambers_lookup(app: AppHandle, vault: State<'_, VaultState>, case_id: String, question: String, limit: Option<usize>) -> Result<serde_json::Value, String> {
    let question = question.trim().to_string();
    if question.is_empty() || question.chars().count() > 500 {
        return Err("CF_LOOKUP_QUESTION: enter a question of up to 500 characters".into());
    }
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    edition::require_capability(&ctx, &mgr.vault_paths().map_err(|e| e.to_string())?.root, "record_lookup")?;
    let docs = mgr.doc_list(&case_id).map_err(|e| e.to_string())?;
    let key = format!("{case_id}|{}", docs.iter().map(|d| format!("{}:{}:{}", d.doc_id, d.byte_size, d.ingested_at_unix_ms)).collect::<Vec<_>>().join(","));
    let index = chambers_lookup::cached(&key, || {
        docs.iter().map(|d| Ok(chambers_lookup::DocPages { doc_id: d.doc_id.clone(), filename: d.original_filename.clone(), pages: matter_pages(&mgr, &case_id, d)? })).collect()
    })?;
    let hits = chambers_lookup::lookup(&index, chambers_lookup::Synonyms::builtin(), &question, limit.unwrap_or(chambers_lookup::DEFAULT_K));
    Ok(serde_json::json!({
        "question": question, "hits": hits, "passages_in_record": index.passages.len(), "documents_indexed": docs.len(),
        "note": "Passages of the record, word for word. Nothing is summarised or concluded.",
    }))
}

/// Passages of the record to offer the model for a question: paragraphs of every document in the matter that share a word
/// with the question, best first, within what the model can take. The question itself is never a source.
fn chambers_record_sources(mgr: &VaultManager, case_id: &str, question: &str, authorities: &[mock_court_retrieval::MockCourtAuthority]) -> Result<(Vec<proposition::SupportSource>, serde_json::Value), String> {
    const MAX_SOURCES: usize = 40;
    const MAX_CHARS: usize = 40_000;
    const MAX_UNIT_CHARS: usize = 2_500;
    let words = answer_feedback::keywords(question);
    let mut left_sources = MAX_SOURCES.saturating_sub(authorities.len());
    let mut left_chars = MAX_CHARS.saturating_sub(authorities.iter().map(|a| a.exact_text.chars().count()).sum::<usize>() + 2_000);
    let docs = mgr.doc_list(case_id).map_err(|e| e.to_string())?;
    let mut cands: Vec<(usize, usize, proposition::SupportSource)> = Vec::new();
    let (mut considered, mut truncated) = (0usize, 0usize);
    for d in &docs {
        let pages = matter_pages(mgr, case_id, d)?;
        for (pi, page) in pages.iter().enumerate() {
            let paras = opposition::paragraphs(std::slice::from_ref(page));
            let units: Vec<(String, String)> = if paras.is_empty() {
                if page.trim().is_empty() { vec![] } else { vec![(format!("page {}", pi + 1), page.trim().to_string())] }
            } else {
                paras.into_iter().map(|p| (p.id, p.text)).collect()
            };
            for (label, text) in units {
                considered += 1;
                let score = words.intersection(&answer_feedback::keywords(&text)).count();
                if score == 0 {
                    continue;
                }
                let text = if text.chars().count() > MAX_UNIT_CHARS {
                    truncated += 1;
                    text.chars().take(MAX_UNIT_CHARS).collect()
                } else {
                    text
                };
                let order = cands.len();
                cands.push((score, order, proposition::SupportSource {
                    support_id: format!("rec:{}:{}:{}", d.doc_id, pi + 1, label),
                    kind: "MATTER_SOURCE".into(),
                    exact_text: text,
                    source_id: None,
                    chunk_id: None,
                    authority_id: None,
                    doc_id: Some(d.doc_id.clone()),
                    page: Some(pi as i64 + 1),
                    paragraph: Some(label),
                    trust_level: None,
                    temporal_status: None,
                    superseded_status: None,
                    permitted: true,
                    translated: false,
                }));
            }
        }
    }
    cands.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut sources = Vec::new();
    for (_, _, src) in cands {
        let n = src.exact_text.chars().count();
        if left_sources == 0 || n > left_chars {
            continue;
        }
        left_sources -= 1;
        left_chars -= n;
        sources.push(src);
    }
    sources.sort_by(|a, b| a.support_id.cmp(&b.support_id));
    let offered = sources.len();
    let names: serde_json::Map<String, serde_json::Value> = docs.iter().map(|d| (d.doc_id.clone(), serde_json::json!(d.original_filename))).collect();
    Ok((sources, serde_json::json!({"passages_in_record": considered, "passages_offered_to_the_model": offered, "passages_shortened": truncated, "documents": names})))
}

/// Chambers reasoning: the model reads passages of the record and installed law and sets out numbered steps; the host
/// verifies each against its source, and the G7 classifier lets only record, law and inference from them render.
#[tauri::command]
fn chambers_reason(app: AppHandle, vault: State<'_, VaultState>, case_id: String, question: String, depth: Option<String>) -> Result<serde_json::Value, String> {
    let question = question.trim().to_string();
    if question.is_empty() || question.chars().count() > 2000 {
        return Err("CF_REASONING_QUESTION: enter a question of up to 2000 characters".into());
    }
    let depth = reasoning::Depth::parse(depth.as_deref().unwrap_or("CAREFUL")).ok_or_else(|| "CF_REASONING_DEPTH: use QUICK, CAREFUL or THOROUGH".to_string())?;
    let mgr = vault.get_or_init(&app)?;
    let ctx = edition_of(&mgr)?;
    edition::require_chambers(&ctx)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    edition::require_capability(&ctx, &paths.root, "reasoning")?;
    let (record_only_on, _) = chambers_record_only(&mgr, &case_id)?;
    g8::check_input(&question)?; // B-S4-21: a request for a decision is refused here, before any model is called
    let run = reasoning_core(&mgr, &case_id, &question, depth, true)?;
    let classified = match run.result.ledger.as_ref() {
        Some(ledger) => record_only::classify(ledger, &run.sources, record_only_on),
        None => record_only::Classified { record_only: record_only_on, statements: vec![], withheld: vec![], rendered_text: String::new() },
    };
    let mut classified = classified;
    record_only::apply_decision_filter(&mut classified, &|t| g8::check_output(t).is_some());
    let guarded = guardrails::check_model_output(&question, &classified.rendered_text, &run.status.model, &[]);
    if guarded.refused {
        return Err("CF_REASONING_OUTPUT_REFUSED: the verified reasoning still failed the safety policy".into());
    }
    let at = crate::docpipe::now_rfc3339();
    for w in &classified.withheld {
        let _ = mgr.witness_kv_put(&case_id, &format!("chambers:g7:{at}:{}", w.proposition_id), &serde_json::json!({"at": at, "proposition_id": w.proposition_id, "reason": w.reason, "text_sha256": w.text_sha256, "question_sha256": hex::encode(<sha2::Sha256 as sha2::Digest>::digest(question.as_bytes()))}));
    }
    let mut by_reason: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for w in &classified.withheld {
        *by_reason.entry(w.reason).or_default() += 1;
    }
    let cov = run.coverage.clone().unwrap_or_default();
    Ok(serde_json::json!({
        "schema_version": "caseforge.chambers-reason.v1", "status": run.result.status, "verified_answer": run.result.verified_answer, "record_only": classified.record_only,
        "statements": classified.statements, "withheld_count": classified.withheld.len(), "withheld_by_reason": by_reason, "rendered_text": classified.rendered_text,
        "candidates_run": run.result.candidates_run, "stopped_for_time": run.result.stopped_for_time, "elapsed_ms": run.elapsed_ms, "question": question,
        "model": {"model": run.status.model, "backend": run.status.backend, "digest": run.status.model_digest},
        "record_coverage": cov,
        "disclaimer": "A reading aid over the record. It shows where each statement comes from; it states no finding and no conclusion on any issue.",
    }))
}

// --- B-S4-03 rule tables / B-S4-09 Code Bridge ---

fn key_dates(mgr: &VaultManager, case_id: &str) -> Result<HashMap<String, code_bridge::KeyDate>, String> {
    Ok(mgr
        .witness_kv_list(case_id, "keydate:")
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|(_, v)| serde_json::from_value::<code_bridge::KeyDate>(v).ok())
        .map(|d| (d.kind.clone(), d))
        .collect())
}

#[tauri::command]
fn rule_tables_status(app: AppHandle, vault: State<'_, VaultState>) -> Result<serde_json::Value, String> {
    let paths = vault.get_or_init(&app)?.vault_paths().map_err(|e| e.to_string())?;
    let (tables, rejected) = rule_tables::load_installed(&paths);
    let list: Vec<serde_json::Value> = tables
        .iter()
        .map(|t| serde_json::json!({"table_id": t.table_id, "table_kind": t.table_kind, "jurisdiction": t.jurisdiction, "pack_id": t.pack_id,
                                    "rows": t.rows.len(), "signed_rows": t.rows.iter().filter(|r| r.signed()).count()}))
        .collect();
    Ok(serde_json::json!({"tables": list, "rejected": rejected}))
}

#[tauri::command]
fn code_bridge_key_dates(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<Vec<code_bridge::KeyDate>, String> {
    edition::require_feature("code_bridge")?;
    let mgr = vault.get_or_init(&app)?;
    Ok(key_dates(&mgr, &case_id)?.into_values().collect())
}

/// Counsel enters (or clears) a key date. Stored as a counsel-input record, the basis locator of every routing.
#[tauri::command]
fn code_bridge_set_key_date(app: AppHandle, vault: State<'_, VaultState>, case_id: String, kind: String, date: Option<String>, note: Option<String>) -> Result<(), String> {
    edition::require_feature("code_bridge")?;
    let mgr = vault.get_or_init(&app)?;
    if !code_bridge::DATE_KINDS.contains(&kind.as_str()) {
        return Err(format!("CF_CODE_BRIDGE_DATE_KIND: {kind}"));
    }
    let key = format!("keydate:{kind}");
    match date.filter(|d| !d.trim().is_empty()) {
        None => mgr.witness_kv_delete(&case_id, &key).map_err(|e| e.to_string()),
        Some(d) if rule_tables::is_date(&d) => {
            let record = code_bridge::KeyDate {
                kind: kind.clone(),
                locator: serde_json::json!({"kind": "COUNSEL_INPUT", "entered_at": crate::docpipe::now_rfc3339(), "note": note}),
                date: d,
            };
            mgr.witness_kv_put(&case_id, &key, &serde_json::to_value(&record).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
        }
        Some(_) => Err("CF_CODE_BRIDGE_DATE: use YYYY-MM-DD".into()),
    }
}

/// Annotates statutory references in `text` (Drafting / an opposing filing) or, without text, in the case graph's
/// statutes. Values come only from installed STATUTE_SUCCESSION rows; with none installed every reference is UNKNOWN.
#[tauri::command]
fn code_bridge_annotate(app: AppHandle, vault: State<'_, VaultState>, case_id: String, text: Option<String>, context: Option<String>) -> Result<serde_json::Value, String> {
    edition::require_feature("code_bridge")?;
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let (tables, rejected) = rule_tables::load_installed(&paths);
    let jurisdiction = case_jurisdiction(&mgr, &case_id)?;
    let rows = rule_tables::query(&tables, "STATUTE_SUCCESSION", Some(&jurisdiction), &crate::docpipe::now_rfc3339()[..10]);
    let chunks = rule_tables::index_for_rows(&paths, &rows);
    let dates = key_dates(&mgr, &case_id)?;
    let annotated: Vec<code_bridge::Annotated> = match text {
        Some(t) => {
            let ctx = match context.as_deref() {
                Some("EXTERNAL_FILING") => "EXTERNAL_FILING",
                _ => "DRAFT",
            };
            code_bridge::annotate(&t, ctx, None, &rows, &chunks, &dates)
        }
        None => {
            let graph = mgr.case_graph_get(&case_id).map_err(|e| e.to_string())?.unwrap_or_else(|| serde_json::json!({}));
            graph["statutes"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|s| {
                    let label = s.get("citation_raw").and_then(|v| v.as_str()).or_else(|| s.get("label").and_then(|v| v.as_str())).unwrap_or("");
                    code_bridge::annotate(label, "CASE_GRAPH", s.get("statute_id").and_then(|v| v.as_str()), &rows, &chunks, &dates)
                })
                .collect()
        }
    };
    let signed_rows = rows.iter().filter(|r| r.signed()).count();
    Ok(serde_json::json!({"pipeline_id": code_bridge::PIPELINE_ID, "annotations": annotated, "succession_rows": rows.len(), "signed_rows": signed_rows,
                          "rejected_tables": rejected, "key_dates": dates.into_values().collect::<Vec<_>>()}))
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
    edition::require_feature("audit_certificate")?;
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
    edition::require_feature("opposition_scan")?;
    let mgr = vault.get_or_init(&app)?;
    edition::require_advocacy(&edition_of(&mgr)?)?; // opposition work is an advocacy feature
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
    // B-S4-29: the filing's authorities are audited against the on-disk store (only the passages it needs are read).
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let store = retrieval::store::open(&paths)?;
    let small = retrieval::build_small_corpus(&paths, &[])?;
    let audit_report = audit::audit_with_store(&store, &small, &opposition::filing_text(&pages), &filename, "txt", Some(case_id.clone()))?;
    let (record, mut report) = opposition::scan_with_report(
        opposition::ScanInput {
            doc_id: &doc_id,
            filing_party_id: &filing_party_id,
            filing_kind: &filing_kind,
            filename: &filename,
            pages: &pages,
            graph: &graph,
            argument_tree: tree.as_ref(),
            corpus: &[],
            case_id: Some(case_id),
        },
        audit_report,
    );
    treatment::decorate_report(&paths, &store, &mut report);
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
    edition::require_feature("witness_matrix")?;
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
    edition::require_feature("witness_matrix")?;
    let mgr = vault.get_or_init(&app)?;
    let jurisdiction = case_jurisdiction(&mgr, &case_id)?;
    let kinds: Vec<serde_json::Value> = witness::VOCAB
        .kinds()
        .iter()
        .map(|k| serde_json::json!({"kind": k.kind, "label": k.label, "jurisdiction": k.jurisdiction, "applies": witness::VOCAB.kind_applies(k, &jurisdiction)}))
        .collect();
    let decisions: Vec<serde_json::Value> = witness_decisions(&mgr, &case_id)?.into_iter().map(|(key, d, active)| serde_json::json!({"key": key, "decision": d, "active": active})).collect();
    Ok(serde_json::json!({"register": witness_register(&mgr, &case_id)?, "decisions": decisions, "statement_kinds": kinds,
                          "vocabulary": {"pack_id": witness::VOCAB.pack_id, "table_id": witness::VOCAB.table_id}}))
}

/// Counsel edits or confirms a register row (origin becomes COUNSEL_CONFIRMED).
#[tauri::command]
fn witness_register_save(app: AppHandle, vault: State<'_, VaultState>, case_id: String, mut entry: witness::RegisterEntry) -> Result<witness::RegisterEntry, String> {
    edition::require_feature("witness_matrix")?;
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
    edition::require_feature("witness_matrix")?;
    vault.get_or_init(&app)?.witness_kv_delete(&case_id, &format!("register:{doc_id}")).map_err(|e| e.to_string())
}

/// Counsel merge/split. Append-only: a decision is revoked (kept in history), never deleted.
#[tauri::command]
fn witness_decision_add(app: AppHandle, vault: State<'_, VaultState>, case_id: String, decision: witness::Decision) -> Result<String, String> {
    edition::require_feature("witness_matrix")?;
    let mgr = vault.get_or_init(&app)?;
    let n = mgr.witness_kv_list(&case_id, "decision:").map_err(|e| e.to_string())?.len();
    let key = format!("decision:{n:06}");
    mgr.witness_kv_put(&case_id, &key, &serde_json::json!({"decision": decision, "active": true, "created_at": crate::docpipe::now_rfc3339()})).map_err(|e| e.to_string())?;
    Ok(key)
}

#[tauri::command]
fn witness_decision_revoke(app: AppHandle, vault: State<'_, VaultState>, case_id: String, key: String) -> Result<(), String> {
    edition::require_feature("witness_matrix")?;
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
    edition::require_feature("witness_matrix")?;
    let mgr = vault.get_or_init(&app)?;
    let us = witness::month_first(&case_jurisdiction(&mgr, &case_id)?);
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
    edition::require_feature("impeachment_kit")?;
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
    edition::require_feature("impeachment_kit")?;
    let mgr = vault.get_or_init(&app)?;
    edition::require_advocacy(&edition_of(&mgr)?)?; // cross-examination work is an advocacy feature
    let us = witness::month_first(&case_jurisdiction(&mgr, &case_id)?);
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

/// B-S4-22: how many seat transfers this licence has used and has left, and the receipt of a seat this machine gave away.
#[tauri::command]
fn seat_status(app: AppHandle, vault: State<'_, VaultState>) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    edition_of(&mgr)?;
    mgr.seat_status().map_err(Into::into)
}

/// B-S4-22: gives this machine's seat away and returns the signed release receipt for the issuer. Never touches the user's
/// cases, documents or exports.
#[tauri::command]
fn seat_deactivate(app: AppHandle, vault: State<'_, VaultState>) -> Result<serde_json::Value, String> {
    let mgr = vault.get_or_init(&app)?;
    mgr.seat_deactivate().map_err(Into::into)
}

// --- B-S4-16 Encrypted Bundle Exchange ---

fn bundle_me(mgr: &VaultManager) -> Result<bundle::Identity, String> {
    Ok(bundle::Identity::from_seed(&mgr.audit_signing_seed().map_err(|e| e.to_string())?))
}

fn unix_ms_now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn peer_json(v: &serde_json::Value) -> serde_json::Value {
    let c = &v["card"];
    let mut out = v.clone();
    out["verification_code"] = serde_json::json!(bundle::verification_code(c["signing_public_key_hex"].as_str().unwrap_or(""), c["agreement_public_key_hex"].as_str().unwrap_or("")));
    out
}

/// This install's public identity card (to hand to people who will send you bundles) and the code to read aloud to check it.
#[tauri::command]
fn bundle_identity(app: AppHandle, vault: State<'_, VaultState>, display_name: String) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    if display_name.trim().is_empty() {
        return Err("CF_BUNDLE_BAD_CARD: give the name people will see on your bundles".into());
    }
    let me = bundle_me(&*vault.get_or_init(&app)?)?;
    Ok(serde_json::json!({"card": me.card(display_name.trim(), unix_ms_now()), "key_id": me.key_id, "verification_code": me.verification_code()}))
}

fn bundle_peer_get(mgr: &VaultManager, key_id: &str) -> Result<Option<serde_json::Value>, String> {
    let key = format!("bundle_peer:{key_id}");
    Ok(mgr.app_kv_list(&key).map_err(|e| e.to_string())?.into_iter().find(|(k, _)| k == &key).map(|(_, v)| v))
}

fn bundle_peer_add_core(mgr: &VaultManager, card: &serde_json::Value, label: Option<String>) -> Result<serde_json::Value, String> {
    let c = bundle::read_card(card)?;
    let prior = bundle_peer_get(mgr, &c.key_id)?;
    let same_keys = prior.as_ref().map(|p| p["card"]["signing_public_key_hex"] == c.signing_public_key_hex.as_str() && p["card"]["agreement_public_key_hex"] == c.agreement_public_key_hex.as_str()).unwrap_or(false);
    let rec = serde_json::json!({
        "card": c,
        "label": label.filter(|l| !l.trim().is_empty()).unwrap_or_else(|| c.display_name.clone()),
        "verified": same_keys && prior.as_ref().map(|p| p["verified"] == true).unwrap_or(false),
        "added_at_unix_ms": unix_ms_now(),
    });
    mgr.app_kv_put(&format!("bundle_peer:{}", c.key_id), &rec).map_err(|e| e.to_string())?;
    Ok(peer_json(&rec))
}

/// Adds someone's identity card to your contacts. It is not trusted until you have checked its code with them.
#[tauri::command]
fn bundle_peer_add(app: AppHandle, vault: State<'_, VaultState>, card: serde_json::Value, label: Option<String>) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    bundle_peer_add_core(&*vault.get_or_init(&app)?, &card, label)
}

fn bundle_peer_verify_core(mgr: &VaultManager, key_id: &str, code: &str) -> Result<serde_json::Value, String> {
    let mut rec = bundle_peer_get(mgr, key_id)?.ok_or("CF_BUNDLE_NO_PEER: that contact is not in your list")?;
    let want = peer_json(&rec)["verification_code"].as_str().unwrap_or("").replace(' ', "");
    if code.replace(' ', "").trim() != want {
        return Err("CF_BUNDLE_CODE_MISMATCH: that code does not match this card. Do not send them anything until you have the right code from them.".into());
    }
    rec["verified"] = serde_json::json!(true);
    rec["verified_at_unix_ms"] = serde_json::json!(unix_ms_now());
    mgr.app_kv_put(&format!("bundle_peer:{key_id}"), &rec).map_err(|e| e.to_string())?;
    Ok(peer_json(&rec))
}

/// Marks a contact verified when the code you were told matches the code computed from their card.
#[tauri::command]
fn bundle_peer_verify(app: AppHandle, vault: State<'_, VaultState>, key_id: String, code: String) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    bundle_peer_verify_core(&*vault.get_or_init(&app)?, &key_id, &code)
}

#[tauri::command]
fn bundle_peer_list(app: AppHandle, vault: State<'_, VaultState>) -> Result<Vec<serde_json::Value>, String> {
    edition::require_feature("bundle_exchange")?;
    Ok(vault.get_or_init(&app)?.app_kv_list("bundle_peer:").map_err(|e| e.to_string())?.iter().map(|(_, v)| peer_json(v)).collect())
}

#[tauri::command]
fn bundle_peer_remove(app: AppHandle, vault: State<'_, VaultState>, key_id: String) -> Result<(), String> {
    edition::require_feature("bundle_exchange")?;
    vault.get_or_init(&app)?.app_kv_delete(&format!("bundle_peer:{key_id}")).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct BundleExtraItem {
    kind: String,
    payload: serde_json::Value,
}

#[derive(Deserialize)]
struct BundleExportArgs {
    case_id: String,
    recipient_key_ids: Vec<String>,
    kinds: Vec<String>,
    #[serde(default)]
    redactions: Vec<bundle::Redaction>,
    #[serde(default)]
    extra_items: Vec<BundleExtraItem>,
    display_name: String,
}

/// What of the matter can go in a bundle straight from the vault: its graph, its argument tree, its documents (as the text the app
/// has read from them, not the original files) and counsel's corrections. The rest (digest, kits, audit reports, notes) is passed in.
fn bundle_collect(mgr: &VaultManager, case_id: &str, kinds: &[String]) -> Result<(Vec<bundle::PlainItem>, Vec<String>), String> {
    let mut items = vec![];
    let mut skipped = vec![];
    for kind in kinds {
        match kind.as_str() {
            "GRAPH_SUBSET" => {
                let cases = mgr.list_cases().map_err(|e| e.to_string())?;
                let profile = cases.iter().find(|c| c.case_id == case_id).map(|c| serde_json::json!({"cause_title": c.cause_title, "court": c.court, "jurisdiction": c.jurisdiction, "matter_type": c.matter_type, "user_side": c.user_side}));
                match mgr.case_graph_get(case_id).map_err(|e| e.to_string())? {
                    Some(g) => items.push(bundle::PlainItem { kind: kind.clone(), payload: serde_json::json!({"profile": profile, "graph": g}) }),
                    None => skipped.push(format!("{kind}: the matter has no case graph yet")),
                }
            }
            "ARGUMENT_TREE" => match mgr.argument_tree_get(case_id).map_err(|e| e.to_string())? {
                Some(t) => items.push(bundle::PlainItem { kind: kind.clone(), payload: t }),
                None => skipped.push(format!("{kind}: the matter has no argument tree yet")),
            },
            "DOCUMENT" => {
                let docs = mgr.doc_list(case_id).map_err(|e| e.to_string())?;
                if docs.is_empty() {
                    skipped.push(format!("{kind}: the matter has no documents"));
                }
                for d in docs {
                    let layers = mgr.page_text_layer_list(case_id, Some(&d.doc_id)).map_err(|e| e.to_string())?;
                    let (pages, truncated) = if layers.is_empty() {
                        let p = mgr.doc_text_preview(case_id, &d.doc_id, 40_000).map_err(|e| e.to_string())?;
                        (vec![serde_json::json!({"page_index": 0, "text": p.text})], p.char_count > 40_000 && !p.is_binary_stub)
                    } else {
                        (layers.iter().map(|l| serde_json::json!({"page_index": l.page_index, "text": l.full_text})).collect(), false)
                    };
                    items.push(bundle::PlainItem { kind: kind.clone(), payload: serde_json::json!({"doc_id": d.doc_id, "filename": d.original_filename, "type_tag": d.type_tag, "language_tag": d.language_tag, "source_format": d.source_format, "page_count": d.page_count, "pages": pages, "text_truncated": truncated, "note": "the text CaseForge read from the file; the original file is not in the bundle"}) });
                }
            }
            "COUNSEL_CORRECTIONS" => {
                let ev = mgr.correction_events_list(case_id, 500).map_err(|e| e.to_string())?;
                if ev.is_empty() {
                    skipped.push(format!("{kind}: no corrections have been made on the matter"));
                } else {
                    items.push(bundle::PlainItem { kind: kind.clone(), payload: serde_json::Value::Array(ev) });
                }
            }
            other => skipped.push(format!("{other}: pass it in from its screen")),
        }
    }
    Ok((items, skipped))
}

fn bundle_export_core(mgr: &VaultManager, args: BundleExportArgs) -> Result<(bundle::Built, Vec<bundle::PeerCard>, Vec<String>), String> {
    if args.display_name.trim().is_empty() {
        return Err("CF_BUNDLE_BAD_CARD: give the name people will see on your bundles".into());
    }
    let mut recipients = vec![];
    for id in &args.recipient_key_ids {
        let rec = bundle_peer_get(mgr, id)?.ok_or_else(|| format!("CF_BUNDLE_NO_PEER: {id} is not in your contacts"))?;
        if rec["verified"] != true {
            return Err(format!("CF_BUNDLE_PEER_UNVERIFIED: you have not checked {}'s code with them. A bundle is encrypted to the key on their card; check the code first.", rec["label"].as_str().unwrap_or(id)));
        }
        recipients.push(serde_json::from_value::<bundle::PeerCard>(rec["card"].clone()).map_err(|e| format!("CF_BUNDLE_BAD_CARD: {e}"))?);
    }
    let (mut items, skipped) = bundle_collect(mgr, &args.case_id, &args.kinds)?;
    for e in args.extra_items {
        items.push(bundle::PlainItem { kind: e.kind, payload: e.payload });
    }
    let me = bundle_me(mgr)?;
    let built = bundle::build(&me, args.display_name.trim(), &recipients, &args.case_id, items, &args.redactions, unix_ms_now(), env!("CARGO_PKG_VERSION"), &mut rand::rngs::OsRng)?;
    Ok((built, recipients, skipped))
}

/// Builds an encrypted bundle for verified contacts and returns it (as base64) to be saved to a USB stick or offered on the local network.
#[tauri::command]
fn bundle_export(app: AppHandle, vault: State<'_, VaultState>, args: BundleExportArgs) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    let (built, recipients, skipped) = bundle_export_core(&*vault.get_or_init(&app)?, args)?;
    use base64::Engine as _;
    Ok(serde_json::json!({
        "bundle_b64": base64::engine::general_purpose::STANDARD.encode(&built.bytes),
        "bundle_id": built.bundle_id,
        "bytes": built.bytes.len(),
        "items": built.items.iter().map(|(id, kind, n, red)| serde_json::json!({"item_id": id, "kind": kind, "plaintext_bytes": n, "redacted": red})).collect::<Vec<_>>(),
        "redaction_log": built.redaction_log.iter().map(|(id, n)| serde_json::json!({"item_id": id, "redaction_count": n})).collect::<Vec<_>>(),
        "recipients": recipients.iter().map(|r| serde_json::json!({"key_id": r.key_id, "display_name": r.display_name})).collect::<Vec<_>>(),
        "skipped": skipped,
    }))
}

fn bundle_bytes(b64: &str) -> Result<Vec<u8>, String> {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| "CF_BUNDLE_BAD_FILE: this is not a bundle file".to_string())
}

fn sender_standing(mgr: &VaultManager, signing_pub: &str) -> Result<Option<serde_json::Value>, String> {
    Ok(mgr.app_kv_list("bundle_peer:").map_err(|e| e.to_string())?.into_iter().map(|(_, v)| v).find(|p| p["card"]["signing_public_key_hex"] == signing_pub))
}

/// What a bundle says about itself, with its signature and hashes checked. Needs no key; shows who sent it and whether you know them.
#[tauri::command]
fn bundle_inspect(app: AppHandle, vault: State<'_, VaultState>, bundle_b64: String) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    let mgr = vault.get_or_init(&app)?;
    let bytes = bundle_bytes(&bundle_b64)?;
    let p = bundle::parse_and_verify(&bytes)?;
    let m = &p.manifest;
    let me = bundle_me(&mgr)?;
    let sp = m["sender"]["signing_public_key_hex"].as_str().unwrap_or("");
    let known = sender_standing(&mgr, sp)?;
    Ok(serde_json::json!({
        "bundle_id": m["bundle_id"], "created_at": m["created_at"], "app_version": m["app_version"],
        "sender": {"display_name": m["sender"]["display_name"], "key_id": m["sender"]["key_id"], "signing_public_key_hex": sp,
                   "known_as": known.as_ref().map(|k| k["label"].clone()), "verified": known.as_ref().map(|k| k["verified"] == true).unwrap_or(false)},
        "addressed_to_you": m["recipients"].as_array().map(|r| r.iter().any(|e| e["key_id"] == me.key_id.as_str())).unwrap_or(false),
        "items": m["contents"].as_array().map(|c| c.iter().map(|i| serde_json::json!({"item_id": i["item_id"], "kind": i["item_kind"], "plaintext_bytes": i["plaintext_bytes"], "redacted": i["redacted"]})).collect::<Vec<_>>()),
        "redaction_log": m["redaction_log"],
        "signature_valid": true,
    }))
}

/// Opens a bundle addressed to you and keeps what is in it, with who sent it and when, beside the matter. Nothing in it replaces
/// anything of yours: received corrections are listed against your own and never applied.
fn bundle_import_core(mgr: &VaultManager, case_id: &str, bytes: &[u8], accept_unknown_sender: bool) -> Result<serde_json::Value, String> {
    let me = bundle_me(mgr)?;
    let opened = bundle::open(bytes, &me)?;
    let m = &opened.manifest;
    let bundle_id = m["bundle_id"].as_str().unwrap_or("").to_string();
    let sp = m["sender"]["signing_public_key_hex"].as_str().unwrap_or("");
    let known = sender_standing(mgr, sp)?;
    let verified = known.as_ref().map(|k| k["verified"] == true).unwrap_or(false);
    if !verified && !accept_unknown_sender {
        return Err(format!("CF_BUNDLE_UNKNOWN_SENDER: this bundle is signed by a key you have not verified (code {}). Check the code with {} before opening it, or confirm that you accept it.", bundle::verification_code(sp, m["sender"]["key_id"].as_str().unwrap_or("")), m["sender"]["display_name"].as_str().unwrap_or("the sender")));
    }
    let skey = format!("bundle_in:{bundle_id}");
    if mgr.witness_kv_list(case_id, &skey).map_err(|e| e.to_string())?.iter().any(|(k, _)| k == &skey) {
        return Err("CF_BUNDLE_ALREADY_IMPORTED: this bundle is already in the matter".into());
    }
    let sender_name = m["sender"]["display_name"].as_str().unwrap_or("").to_string();
    let created_ms = opened.items.first().and_then(|i| i.envelope["produced_at_unix_ms"].as_i64()).unwrap_or(0);
    let label = bundle::attribution_label(&sender_name, created_ms);
    let mine = mgr.correction_overlay_values(case_id).map_err(|e| e.to_string())?;
    let mut summary_items = vec![];
    for it in &opened.items {
        let mut rec = serde_json::json!({"item_id": it.item_id, "kind": it.kind, "attribution": label, "envelope": it.envelope});
        if it.kind == "COUNSEL_CORRECTIONS" {
            if let Some(arr) = it.envelope["payload"].as_array() {
                rec["plan"] = serde_json::Value::Array(bundle::plan_corrections(&mine, arr));
            }
        }
        mgr.witness_kv_put(case_id, &format!("{skey}:{}", it.item_id), &rec).map_err(|e| e.to_string())?;
        summary_items.push(serde_json::json!({"item_id": it.item_id, "kind": it.kind}));
    }
    let summary = serde_json::json!({
        "bundle_id": bundle_id, "attribution": label, "created_at": m["created_at"], "imported_at_unix_ms": unix_ms_now(),
        "sender": {"display_name": sender_name, "key_id": m["sender"]["key_id"], "signing_public_key_hex": sp, "known_as": known.as_ref().map(|k| k["label"].clone()), "verified": verified},
        "source_case_id": m["source_case_id"], "items": summary_items, "redaction_log": m["redaction_log"],
    });
    mgr.witness_kv_put(case_id, &skey, &summary).map_err(|e| e.to_string())?;
    Ok(summary)
}

#[tauri::command]
fn bundle_import(app: AppHandle, vault: State<'_, VaultState>, case_id: String, bundle_b64: String, accept_unknown_sender: Option<bool>) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    bundle_import_core(&*vault.get_or_init(&app)?, &case_id, &bundle_bytes(&bundle_b64)?, accept_unknown_sender == Some(true))
}

/// Bundles received into the matter, newest first.
#[tauri::command]
fn bundle_received_list(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<Vec<serde_json::Value>, String> {
    edition::require_feature("bundle_exchange")?;
    let mgr = vault.get_or_init(&app)?;
    let mut v: Vec<serde_json::Value> = mgr.witness_kv_list(&case_id, "bundle_in:").map_err(|e| e.to_string())?.into_iter().filter(|(k, _)| k.matches(':').count() == 1).map(|(_, v)| v).collect();
    v.sort_by(|a, b| b["imported_at_unix_ms"].as_i64().cmp(&a["imported_at_unix_ms"].as_i64()));
    Ok(v)
}

#[tauri::command]
fn bundle_received_get(app: AppHandle, vault: State<'_, VaultState>, case_id: String, bundle_id: String) -> Result<serde_json::Value, String> {
    edition::require_feature("bundle_exchange")?;
    let mgr = vault.get_or_init(&app)?;
    let skey = format!("bundle_in:{bundle_id}");
    let all = mgr.witness_kv_list(&case_id, &skey).map_err(|e| e.to_string())?;
    let summary = all.iter().find(|(k, _)| k == &skey).map(|(_, v)| v.clone()).ok_or("CF_BUNDLE_NOT_FOUND: that bundle is not in this matter")?;
    let items: Vec<serde_json::Value> = all.into_iter().filter(|(k, _)| k != &skey).map(|(_, v)| v).collect();
    Ok(serde_json::json!({"summary": summary, "items": items}))
}

/// Offers a bundle once on the local network. Refused under air-gap (a USB file needs no network).
#[tauri::command]
fn bundle_lan_share(app: AppHandle, vault: State<'_, VaultState>, bundle_b64: String) -> Result<bundle_lan::ShareInfo, String> {
    edition::require_feature("bundle_exchange")?;
    let mgr = vault.get_or_init(&app)?;
    if mgr.air_gap_enabled().unwrap_or(false) {
        return Err("CF_AIR_GAP: local-network transfer is switched off while air-gap mode is on. Save the bundle to a USB stick instead.".into());
    }
    bundle_lan::share(bundle_bytes(&bundle_b64)?, None, std::time::Duration::from_secs(600), None)
}

#[tauri::command]
fn bundle_lan_stop() -> Result<(), String> {
    edition::require_feature("bundle_exchange")?;
    bundle_lan::stop();
    Ok(())
}

/// Fetches a bundle offered on the local network; the address must be the numeric one shown on the other computer.
#[tauri::command]
fn bundle_lan_fetch(app: AppHandle, vault: State<'_, VaultState>, url: String) -> Result<String, String> {
    edition::require_feature("bundle_exchange")?;
    let mgr = vault.get_or_init(&app)?;
    if mgr.air_gap_enabled().unwrap_or(false) {
        return Err("CF_AIR_GAP: local-network transfer is switched off while air-gap mode is on. Copy the bundle from a USB stick instead.".into());
    }
    use base64::Engine as _;
    Ok(base64::engine::general_purpose::STANDARD.encode(bundle_lan::fetch(&url)?))
}

// --- B-S4-12 Timed Oral Rehearsal ---

/// Slot presets, rates and the rules the timer follows (data from contracts/rehearsal_policy.json).
#[tauri::command]
fn rehearsal_policy() -> serde_json::Value {
    timed_rehearsal::policy_json()
}

fn rehearsal_sessions() -> Result<std::sync::MutexGuard<'static, std::collections::HashMap<String, timed_rehearsal::Live>>, String> {
    timed_rehearsal::SESSIONS.lock().map_err(|_| "CF_REHEARSAL_STATE: the rehearsal timer state could not be read".to_string())
}

fn rehearsal_events(ev: &[timed_rehearsal::Event]) -> Vec<serde_json::Value> {
    ev.iter().map(|e| serde_json::json!(e)).collect()
}

/// Starts a timed session on the matter's argument tree. The host keeps the clock from this moment.
#[tauri::command]
fn rehearsal_start(
    app: AppHandle,
    vault: State<'_, VaultState>,
    case_id: String,
    session_id: String,
    slot_seconds: i64,
    rate: Option<String>,
    interruption_slider: Option<i64>,
    archetype_id: Option<String>,
) -> Result<serde_json::Value, String> {
    edition::require_feature("timed_rehearsal")?;
    let mgr = vault.get_or_init(&app)?;
    let tree = mgr
        .argument_tree_get(&case_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "CF_REHEARSAL_NO_TREE: build the argument tree first (Argument Integrity), so the bench has limbs and Missing Links to press on".to_string())?;
    let report = integrity::score_tree(&tree)?;
    let limbs = timed_rehearsal::limbs_of(&tree, &report);
    let targets = timed_rehearsal::targets_of(&report, &limbs);
    let archetype = archetype_id.unwrap_or_else(|| "procedure_first".into());
    let setting = interruption_slider.unwrap_or_else(|| adaptive_bench::merge_sliders(&archetype, None).interruption as i64);
    let rate = rate.unwrap_or_else(|| timed_rehearsal::default_rate(setting).to_string());
    let session = timed_rehearsal::Session::start(&session_id, &case_id, slot_seconds, &rate, Some(&archetype), limbs.clone(), targets.clone())?;
    let out = serde_json::json!({
        "session_id": session_id, "slot_ms": session.slot_ms, "rate": session.rate, "interval_ms": timed_rehearsal::interval_ms(session.rate),
        "archetype_id": archetype, "limbs": limbs.iter().map(|l| serde_json::json!({"node_id": l.node_id, "label": l.label})).collect::<Vec<_>>(),
        "targets": targets.len(),
    });
    rehearsal_sessions()?.insert(session_id, timed_rehearsal::Live { session, started: std::time::Instant::now() });
    Ok(out)
}

/// What the screen polls: time left, and any warning or expiry that became due (stamped at its own moment).
#[tauri::command]
fn rehearsal_state(session_id: String) -> Result<serde_json::Value, String> {
    edition::require_feature("timed_rehearsal")?;
    let mut all = rehearsal_sessions()?;
    let live = all.get_mut(&session_id).ok_or_else(|| "CF_REHEARSAL_NO_SESSION: start a timed session first".to_string())?;
    let now = live.now_ms();
    let ev = live.session.observe(now);
    Ok(serde_json::json!({"elapsed_ms": now.min(live.session.slot_ms), "remaining_ms": live.session.remaining_ms(now), "ended_by": live.session.ended_by, "new_events": rehearsal_events(&ev)}))
}

/// Counsel's response (typed, or a confirmed voice transcript), timed by the host. Refused once the slot has ended.
#[tauri::command]
fn rehearsal_response(session_id: String, text: String) -> Result<serde_json::Value, String> {
    edition::require_feature("timed_rehearsal")?;
    if text.trim().is_empty() {
        return Err("CF_REHEARSAL_EMPTY: there is nothing to send".into());
    }
    let mut all = rehearsal_sessions()?;
    let live = all.get_mut(&session_id).ok_or_else(|| "CF_REHEARSAL_NO_SESSION: start a timed session first".to_string())?;
    let now = live.now_ms();
    let r = live.session.respond(now, &text)?;
    Ok(serde_json::json!(r))
}

/// Ends the session (if it has not already ended), keeps its timeline with the matter, and returns the timeline and what the
/// debrief says about it. Calling it again returns the same record.
#[tauri::command]
fn rehearsal_end(app: AppHandle, vault: State<'_, VaultState>, case_id: String, session_id: String, debrief_id: Option<String>) -> Result<serde_json::Value, String> {
    edition::require_feature("timed_rehearsal")?;
    let mgr = vault.get_or_init(&app)?;
    let (timeline, findings, labels) = {
        let mut all = rehearsal_sessions()?;
        let live = all.get_mut(&session_id).ok_or_else(|| "CF_REHEARSAL_NO_SESSION: start a timed session first".to_string())?;
        let now = live.now_ms();
        live.session.end_early(now);
        let findings: Vec<serde_json::Value> = live.session.findings().iter().map(|f| serde_json::json!(f)).collect();
        (live.session.timeline(debrief_id.as_deref())?, findings, live.session.limb_labels())
    };
    let record = serde_json::json!({"timeline": timeline, "findings": findings, "limb_labels": labels, "saved_at": docpipe::now_rfc3339()});
    mgr.witness_kv_put(&case_id, &format!("rehearsal:{session_id}"), &record).map_err(|e| e.to_string())?;
    rehearsal_sessions()?.remove(&session_id);
    Ok(record)
}

/// A kept timeline and its findings.
#[tauri::command]
fn rehearsal_get(app: AppHandle, vault: State<'_, VaultState>, case_id: String, session_id: String) -> Result<Option<serde_json::Value>, String> {
    edition::require_feature("timed_rehearsal")?;
    let mgr = vault.get_or_init(&app)?;
    Ok(mgr.witness_kv_list(&case_id, &format!("rehearsal:{session_id}")).map_err(|e| e.to_string())?.into_iter().find(|(k, _)| k == &format!("rehearsal:{session_id}")).map(|(_, v)| v))
}

/// The kept timelines of a matter, newest first.
#[tauri::command]
fn rehearsal_list(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<Vec<serde_json::Value>, String> {
    edition::require_feature("timed_rehearsal")?;
    let mgr = vault.get_or_init(&app)?;
    let mut v: Vec<serde_json::Value> = mgr.witness_kv_list(&case_id, "rehearsal:").map_err(|e| e.to_string())?.into_iter().map(|(_, v)| v).collect();
    v.sort_by(|a, b| b["saved_at"].as_str().cmp(&a["saved_at"].as_str()));
    Ok(v)
}

#[tauri::command]
fn air_gap_set(
    app: AppHandle,
    vault: State<'_, VaultState>,
    enabled: bool,
) -> Result<licence::LicenceStatus, String> {
    let mgr = vault.get_or_init(&app)?;
    edition::check_air_gap_change(&edition_of(&mgr)?, enabled)?; // a Judiciary licence keeps the app offline
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

// --- B-S4-15 aligned Hindi-to-English translation (guardrail G10) ---

#[tauri::command]
fn translation_start(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_id: Option<String>) -> Result<translation::JobStatus, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    let docs = match doc_id {
        Some(d) => vec![d],
        None => translation::docs_with_hindi(&mgr, &case_id)?,
    };
    if docs.is_empty() {
        return Err("CF_TRANSLATE_NOTHING: no document in this matter has Hindi text to translate; recognize the scans first".into());
    }
    // The translation model is the primary reasoner's model, so it takes that slot: never beside the vision model or a reasoning turn.
    let acq = choreography::acquire("primary_llm", "translation");
    if !acq.ok {
        return Err(acq.error.unwrap_or_else(|| "CF_CHOREOGRAPHY: the model slot is busy".into()));
    }
    let emitter = app.clone();
    Ok(translation::start_job(
        mgr,
        case_id,
        docs,
        move |status| {
            let _ = emitter.emit("translation_progress", status);
        },
        || {
            choreography::release("primary_llm");
        },
    ))
}

#[tauri::command]
fn translation_status(job_id: String) -> Result<translation::JobStatus, String> {
    edition::require_feature("indic_ingestion")?;
    translation::job_status(&job_id).ok_or_else(|| "CF_TRANSLATE_NOT_FOUND: unknown translation job".to_string())
}

#[tauri::command]
fn translation_cancel(job_id: String) -> Result<bool, String> {
    edition::require_feature("indic_ingestion")?;
    Ok(translation::job_cancel(&job_id))
}

#[tauri::command]
fn translation_list(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_id: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    translation::list(&mgr, &case_id, doc_id.as_deref())
}

#[tauri::command]
fn translation_events(app: AppHandle, vault: State<'_, VaultState>, case_id: String) -> Result<Vec<serde_json::Value>, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    translation::events(&mgr, &case_id)
}

#[tauri::command]
fn translation_confirm_field(app: AppHandle, vault: State<'_, VaultState>, case_id: String, segment_id: String, field_id: String, confirmed_value: String, note: Option<String>) -> Result<serde_json::Value, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    translation::confirm_field(&mgr, &case_id, &segment_id, &field_id, &confirmed_value, note.as_deref())
}

#[tauri::command]
fn translation_unconfirm_field(app: AppHandle, vault: State<'_, VaultState>, case_id: String, segment_id: String, field_id: String, note: Option<String>) -> Result<serde_json::Value, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    translation::unconfirm_field(&mgr, &case_id, &segment_id, &field_id, note.as_deref())
}

#[tauri::command]
fn translation_graph(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_id: String, jurisdiction: Option<String>) -> Result<serde_json::Value, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    translation::derive_graph(&mgr, &case_id, jurisdiction.as_deref().unwrap_or("India"), &doc_id)
}

#[tauri::command]
fn translation_graph_get(app: AppHandle, vault: State<'_, VaultState>, case_id: String, doc_id: String) -> Result<Option<serde_json::Value>, String> {
    edition::require_feature("indic_ingestion")?;
    let mgr = vault.get_or_init(&app)?;
    translation::graph_get(&mgr, &case_id, &doc_id)
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
    let mgr = vault.get_or_init(&app)?;
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    // B-S4-22: after the pack subscription ends, newer official content cannot be installed; nothing else is affected
    let sub = edition_of(&mgr)?.pack_subscription;
    if sub.expired() {
        let man = packs::read_manifest(Path::new(&source_dir))?;
        subscription::check_install(&sub, &man.pack_kind, man.delta.as_ref().is_some_and(|d| !d.is_null()), man.currency_date.as_deref(), man.updated_at.as_deref())?;
    }
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
    let (terms, _) = retrieval::decompose(&query);
    let paths = mgr.vault_paths().map_err(|e| e.to_string())?;
    let small = retrieval::build_small_corpus(&paths, &[])?;
    let found = retrieval::store::search(
        &paths,
        &query,
        &terms,
        12,
        &retrieval::MetadataFilters {
            jurisdiction: Some(profile.jurisdiction.clone()),
            trust_level: Some("OFFICIAL_VERIFIED".into()),
            temporal_status: Some("APPLICABLE".into()),
            superseded_status: Some("CURRENT".into()),
            ..retrieval::MetadataFilters::default()
        },
        true,
        &small,
        &[],
    )?;
    let all: Vec<working_set::WorkingSetItemInput> = found
        .hits
        .iter()
        .filter_map(|hit| found.chunks.iter().find(|chunk| chunk.chunk_id == hit.chunk_id))
        .map(|chunk| working_set::item_from_chunk(chunk, &found.corpus_checksum))
        .collect();
    // a clear mismatch (a criminal code for a civil matter) is left out of the list and named beside it
    let mut proposals = Vec::new();
    let mut suppressed = Vec::new();
    for item in all {
        match working_set::misfit(&profile, graph.as_ref(), &item.title) {
            Some(reason) => suppressed.push(json!({"title": item.title, "section_label": item.section_label, "reason": reason})),
            None => proposals.push(item),
        }
    }
    let regimes = working_set::regime_candidates(&profile, graph.as_ref());
    let mut set = mgr
        .working_set_merge_suggestions(&case_id, &proposals, &regimes)
        .map_err(|e| e.to_string())?;
    set["suppressed_proposals"] = serde_json::Value::Array(suppressed);
    Ok(set)
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
    let (chunk, checksum) = retrieval::find_chunk(&paths, &chunk_id)?.ok_or_else(|| {
        "CF_WORKING_SET_CHUNK_NOT_FOUND: rerun retrieval against installed packs".to_string()
    })?;
    let chunk = &chunk;
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
    edition::require_feature("retrieval")?;
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
            translated: false,
        });
    }
    // B-S4-15: a translated page is offered too, marked translated so guardrail G10 keeps anything built on it below GREEN
    let left = limit.saturating_sub(used);
    if left > 0 {
        out.extend(translation::support_sources(&manager, &case_id, left)?);
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
    edition::require_feature("voice")?;
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
    edition::require_feature("voice")?;
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
    edition::require_feature("voice")?;
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
    edition::require_feature("voice")?;
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
    edition::require_feature("mock_court")?;
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
    edition::require_feature("mock_court")?;
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
    edition::require_feature("mock_court")?;
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
    edition::require_feature("mock_court")?;
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
            rule_tables_status,
            code_bridge_key_dates,
            code_bridge_set_key_date,
            code_bridge_annotate,
            reasoning_run,
            deadlines_compute,
            deadlines_assign,
            deadlines_unassign,
            deadlines_fact_save,
            deadlines_fact_remove,
            deadlines_export_ics,
            foundation_check,
            foundation_set_type,
            foundation_link,
            pack_currency,
            edition_status,
            chambers_status,
            chambers_set_record_only,
            chambers_g7_log,
            chambers_reason,
            chambers_audit_all,
            chambers_lookup,
            cause_list_import,
            scaffold_get,
            scaffold_prefill,
            scaffold_set_text,
            scaffold_remove_sentence,
            scaffold_audit_text,
            scaffold_export,
            cause_list_briefs,
            treatment_lookup,
            issue_matrix_build,
            issue_matrix_set_matter_type,
            issue_matrix_add,
            issue_matrix_decide,
            issue_matrix_set_filing,
            issue_matrix_memo,
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
            seat_status,
            rehearsal_policy,
            rehearsal_start,
            rehearsal_state,
            rehearsal_response,
            rehearsal_end,
            rehearsal_get,
            rehearsal_list,
            bundle_identity,
            bundle_peer_add,
            bundle_peer_verify,
            bundle_peer_list,
            bundle_peer_remove,
            bundle_export,
            bundle_inspect,
            bundle_import,
            bundle_received_list,
            bundle_received_get,
            bundle_lan_share,
            bundle_lan_stop,
            bundle_lan_fetch,
            translation_start,
            translation_status,
            translation_cancel,
            translation_list,
            translation_events,
            translation_confirm_field,
            translation_unconfirm_field,
            translation_graph,
            translation_graph_get,
            seat_deactivate,
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

#[cfg(test)]
mod bundle_vault_tests {
    //! B-S4-16 across two real vaults: one sends, the other receives (AC-S4-10.1 to 10.4 at the level of the app).
    use super::*;
    use crate::vault::{CaseWizardInput, DocumentIngestRequest};
    use base64::Engine as _;
    use serde_json::{json, Value};

    const FIXTURE: &str = include_str!("../../golden/G-BUNDLE/fixtures/bundle_core.json");

    fn vault(tag: &str, pass: &str) -> (std::sync::Arc<VaultManager>, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("caseforge-bundle-{tag}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = std::sync::Arc::new(VaultManager::new(dir.clone()));
        mgr.init_and_unlock(pass).unwrap();
        (mgr, dir)
    }
    fn case(mgr: &VaultManager, pass: &str, title: &str) -> String {
        let id = mgr.create_case_with_wizard(pass, CaseWizardInput { cause_title: title.into(), court: "District Court".into(), jurisdiction: "India".into(), matter_type: "Property partition civil appeal".into(), user_side: "Appellant".into(), code_era: None, language: None }).unwrap().case_id;
        mgr.open_case(&id, pass).unwrap();
        id
    }
    fn exchange(a: &VaultManager, a_name: &str, b: &VaultManager, b_name: &str) -> (String, String) {
        // each side makes a card, the other adds it and confirms the code read aloud
        let ca = bundle_me(a).unwrap().card(a_name, 1);
        let cb = bundle_me(b).unwrap().card(b_name, 1);
        let pa = bundle_peer_add_core(b, &ca, None).unwrap();
        let pb = bundle_peer_add_core(a, &cb, None).unwrap();
        bundle_peer_verify_core(b, pa["card"]["key_id"].as_str().unwrap(), pa["verification_code"].as_str().unwrap()).unwrap();
        bundle_peer_verify_core(a, pb["card"]["key_id"].as_str().unwrap(), pb["verification_code"].as_str().unwrap()).unwrap();
        (pa["card"]["key_id"].as_str().unwrap().to_string(), pb["card"]["key_id"].as_str().unwrap().to_string())
    }

    #[test]
    fn a_junior_hands_a_senior_an_encrypted_attributable_bundle_and_nothing_of_the_seniors_changes() {
        let f: Value = serde_json::from_str(FIXTURE).unwrap();
        let (pw_s, pw_r) = ("sender passphrase", "recipient passphrase");
        let (sender, sdir) = vault("s", pw_s);
        let (recipient, rdir) = vault("r", pw_r);
        let (sender_key_at_recipient, recipient_key_at_sender) = exchange(&sender, "Priya Nair", &recipient, "Senior Counsel");

        // the junior's matter: a graph, a document, and a correction of her own
        let sc = case(&sender, pw_s, "Kumar v Verma");
        let graph = json!({"schema_version": "1.0.0", "chronology": [{"event_id": "e1", "date": "2020-03-01", "description": "Suresh Verma took exclusive possession"}], "parties": [{"party_id": "p1", "name": "Ramesh Kumar", "role": "plaintiff"}]});
        sender.case_graph_put(&sc, &graph).unwrap();
        sender.doc_ingest(&sc, DocumentIngestRequest { original_filename: "Plaint of Ramesh Kumar.txt".into(), content_base64: base64::engine::general_purpose::STANDARD.encode(f["items"]["document"]["text"].as_str().unwrap()), type_tag: Some("PLAINT".into()), language_tag: Some("en".into()) }).unwrap();
        sender.case_graph_correct_item(&sc, "chronology", "e1", "date", "2020-03-11", 2).unwrap();

        // the senior's matter has the same event and her own, different correction of it
        let rc = case(&recipient, pw_r, "Kumar v Verma (senior's copy)");
        recipient.case_graph_put(&rc, &graph).unwrap();
        recipient.case_graph_correct_item(&rc, "chronology", "e1", "date", "2020-03-05", 2).unwrap();
        let before_overlays = recipient.correction_overlay_values(&rc).unwrap();
        let before_events = recipient.correction_events_list(&rc, 500).unwrap();
        let before_graph = recipient.case_graph_get(&rc).unwrap();
        let before_docs = recipient.doc_list(&rc).unwrap().len();
        assert_eq!(before_overlays.values().collect::<Vec<_>>(), vec!["2020-03-05"]);

        // build: redactions applied, a note added
        let reds: Vec<bundle::Redaction> = serde_json::from_value(f["redactions"].clone()).unwrap();
        let (built, _, skipped) = bundle_export_core(&sender, BundleExportArgs {
            case_id: sc.clone(),
            recipient_key_ids: vec![recipient_key_at_sender.clone()],
            kinds: vec!["GRAPH_SUBSET".into(), "ARGUMENT_TREE".into(), "DOCUMENT".into(), "COUNSEL_CORRECTIONS".into()],
            redactions: reds,
            extra_items: vec![BundleExtraItem { kind: "NOTES".into(), payload: f["notes_item"].clone() }],
            display_name: "Priya Nair".into(),
        })
        .unwrap();
        assert!(skipped.iter().any(|s| s.starts_with("ARGUMENT_TREE")), "no argument tree yet, said so: {skipped:?}");
        // nothing readable in the file
        assert!(!built.bytes.windows(12).any(|w| w == b"Suresh Verma") && !built.bytes.windows(12).any(|w| w == b"Ramesh Kumar"));

        // the senior inspects it: who it is from, that it is for her, what is in it
        let p = bundle::parse_and_verify(&built.bytes).unwrap();
        assert_eq!(p.manifest["sender"]["display_name"], "Priya Nair");
        assert_eq!(sender_standing(&recipient, p.manifest["sender"]["signing_public_key_hex"].as_str().unwrap()).unwrap().unwrap()["verified"], true);

        // a stranger's vault cannot open it
        let (stranger, tdir) = vault("t", "stranger passphrase");
        let tc = case(&stranger, "stranger passphrase", "Other");
        assert!(bundle_import_core(&stranger, &tc, &built.bytes, true).unwrap_err().starts_with("CF_BUNDLE_NOT_FOR_YOU"));
        assert!(stranger.witness_kv_list(&tc, "bundle_in:").unwrap().is_empty(), "nothing was kept");

        // the senior imports it
        let summary = bundle_import_core(&recipient, &rc, &built.bytes, false).unwrap();
        assert!(summary["attribution"].as_str().unwrap().starts_with("from Priya Nair, "), "{}", summary["attribution"]);
        assert_eq!(summary["sender"]["verified"], true);
        let stored = recipient.witness_kv_list(&rc, &format!("bundle_in:{}", summary["bundle_id"].as_str().unwrap())).unwrap();
        assert_eq!(stored.len(), 1 + summary["items"].as_array().unwrap().len());
        // every kept item carries who it is from
        assert!(stored.iter().filter(|(k, _)| k.matches(':').count() == 2).all(|(_, v)| v["attribution"].as_str().unwrap().starts_with("from Priya Nair")));
        // redacted text is in none of what the senior was given
        let kept = serde_json::to_string(&stored).unwrap();
        for t in ["Ramesh Kumar", "98765 43210", "9876543210", "ramesh.k@example.org"] {
            assert!(!kept.contains(t), "{t} reached the recipient");
        }
        assert!(kept.contains("Suresh Verma") && kept.contains("[REDACTED]"));
        // AC-S4-10.4: her correction is untouched and the junior's differing one is listed against it, not applied
        let plan = stored.iter().map(|(_, v)| v.clone()).find(|v| v["kind"] == "COUNSEL_CORRECTIONS").expect("the corrections item was kept");
        let rows = plan["plan"].as_array().unwrap();
        let my_key = before_overlays.keys().next().unwrap().clone();
        let row = rows.iter().find(|r| r["target_key"] == my_key.as_str()).expect("the correction on the same event is listed against the recipient's");
        assert_eq!((row["status"].as_str(), row["recipient_value"].as_str(), row["sender_value"].as_str()), (Some("CONFLICTS_WITH_YOUR_CORRECTION"), Some("2020-03-05"), Some("2020-03-11")));
        assert_eq!(recipient.correction_overlay_values(&rc).unwrap(), before_overlays, "her active corrections are exactly as they were");
        assert_eq!(recipient.correction_events_list(&rc, 500).unwrap(), before_events, "no correction event was added");
        assert_eq!(recipient.case_graph_get(&rc).unwrap(), before_graph, "her graph is untouched");
        assert_eq!(recipient.doc_list(&rc).unwrap().len(), before_docs, "her documents are untouched");
        // the same bundle twice is refused
        assert!(bundle_import_core(&recipient, &rc, &built.bytes, false).unwrap_err().starts_with("CF_BUNDLE_ALREADY_IMPORTED"));
        // a changed file is refused before anything is kept
        let mut bad = built.bytes.clone();
        let n = bad.len() / 2;
        bad[n] ^= 1;
        let rc2 = case(&recipient, pw_r, "Second");
        assert!(bundle_import_core(&recipient, &rc2, &bad, false).unwrap_err().starts_with("CF_BUNDLE_TAMPERED"));
        assert!(recipient.witness_kv_list(&rc2, "bundle_in:").unwrap().is_empty());
        let _ = sender_key_at_recipient;
        for d in [sdir, rdir, tdir] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn a_bundle_is_not_made_for_someone_whose_code_has_not_been_checked_and_an_unknown_sender_must_be_accepted_by_hand() {
        let (a, adir) = vault("ua", "a passphrase");
        let (b, bdir) = vault("ub", "b passphrase");
        let ca = case(&a, "a passphrase", "A");
        let cb = case(&b, "b passphrase", "B");
        // B's card is added by A but its code is not confirmed
        let card_b = bundle_me(&b).unwrap().card("Bee", 1);
        let pb = bundle_peer_add_core(&a, &card_b, None).unwrap();
        let args = |kid: String| BundleExportArgs { case_id: ca.clone(), recipient_key_ids: vec![kid], kinds: vec![], redactions: vec![], extra_items: vec![BundleExtraItem { kind: "NOTES".into(), payload: json!({"text": "hi"}) }], display_name: "Ay".into() };
        let key_b = pb["card"]["key_id"].as_str().unwrap().to_string();
        assert!(bundle_export_core(&a, args(key_b.clone())).err().unwrap().starts_with("CF_BUNDLE_PEER_UNVERIFIED"));
        // a wrong code does not verify; the right one does
        assert!(bundle_peer_verify_core(&a, &key_b, "00000 00000 00000 00000 00000 00000").unwrap_err().starts_with("CF_BUNDLE_CODE_MISMATCH"));
        bundle_peer_verify_core(&a, &key_b, pb["verification_code"].as_str().unwrap()).unwrap();
        let (built, _, _) = bundle_export_core(&a, args(key_b.clone())).unwrap();
        // B has not added A: the sender is unknown, so B must accept by hand
        let e = bundle_import_core(&b, &cb, &built.bytes, false).unwrap_err();
        assert!(e.starts_with("CF_BUNDLE_UNKNOWN_SENDER") && e.contains("Ay"), "{e}");
        assert!(b.witness_kv_list(&cb, "bundle_in:").unwrap().is_empty());
        let s = bundle_import_core(&b, &cb, &built.bytes, true).unwrap();
        assert_eq!(s["sender"]["verified"], false, "accepting by hand does not make the sender trusted");
        // a card whose key id or signature was altered is not accepted
        let mut forged = bundle_me(&a).unwrap().card("Ay", 1);
        forged["display_name"] = json!("Someone else");
        assert!(bundle_peer_add_core(&b, &forged, None).unwrap_err().starts_with("CF_BUNDLE_BAD_CARD"));
        // changing a contact's keys under the same id un-verifies it
        for d in [adir, bdir] {
            let _ = std::fs::remove_dir_all(d);
        }
    }
}
