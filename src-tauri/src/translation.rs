//! Aligned offline Hindi-to-English translation (B-S4-15, PRD v4.0 section 7.9, guardrail G10).
//!
//! The host drives; the model only writes English prose. Per page: the document sidecar cuts the recognized text into segments and masks every
//! critical field (docpipe `translate_prepare`); the local model translates each masked segment (`llm.translate`); the sidecar puts the English form
//! of each field back by fixed rules and builds the record (`translate_finish`); the record is stored beside the matter, encrypted in the vault.
//! Each segment keeps its original text, its place in the page, its OCR confidence, the model and prompt that produced it, provenance TRANSLATED
//! and `quotation_verifiable: false`. Critical fields start unconfirmed; only counsel's confirmation (an append-only event) changes that.
//!
//! The job is resumable: a segment already stored with the same source text is kept, so a stopped or failed run continues where it was.
//! Nothing here opens a network connection or reads a file outside the vault.

use crate::vault::VaultManager;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// The label every translated segment carries (guardrail G10, contracts/translation_config.json).
pub const LABEL: &str = "machine translation - wording not verified";
pub const PROVENANCE_ORIGIN: &str = "TRANSLATED";
/// A fact read from a translation is held below the downstream-trust line until counsel confirms it.
pub const TRANSLATED_FACT_CONFIDENCE_CAP: f64 = 0.49;

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// The three steps of a translation. `SidecarEngine` is the real one; tests supply a fixed one.
pub trait Engine {
    fn prepare(&self, doc_id: &str, page_index: i64, text: &str, words: &Value) -> Result<Vec<Value>, String>;
    fn translate(&self, masked: &str) -> Result<String, String>;
    fn finish(&self, doc_id: &str, page_index: i64, prepared: &Value, output: &str, calibration_id: Option<&str>) -> Result<Value, String>;
}

pub struct SidecarEngine;

impl Engine for SidecarEngine {
    fn prepare(&self, doc_id: &str, page_index: i64, text: &str, words: &Value) -> Result<Vec<Value>, String> {
        let r = crate::docpipe::pdf_adapter::docpipe_rpc("docpipe.translate_prepare", json!({"doc_id": doc_id, "page_index": page_index, "text": text, "words": words}))?;
        r.get("segments").and_then(Value::as_array).cloned().ok_or_else(|| "CF_TRANSLATE_FAILED: the sidecar returned no segments".to_string())
    }
    fn translate(&self, masked: &str) -> Result<String, String> {
        crate::infer_llm::translate(masked)
    }
    fn finish(&self, doc_id: &str, page_index: i64, prepared: &Value, output: &str, calibration_id: Option<&str>) -> Result<Value, String> {
        crate::docpipe::pdf_adapter::docpipe_rpc(
            "docpipe.translate_finish",
            json!({"doc_id": doc_id, "page_index": page_index, "prepared": prepared, "model_output": output, "calibration_id": calibration_id}),
        )
    }
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct Summary {
    pub pages: usize,
    pub segments: usize,
    pub translated: usize,
    pub resumed: usize,
    pub carried: usize,
    pub flagged: usize,
    pub cancelled: bool,
}

pub fn seg_key(doc_id: &str, page_index: i64, seg_index: i64) -> String {
    format!("seg:{doc_id}:{page_index:06}:{seg_index:06}")
}

fn key_of_segment_id(segment_id: &str) -> Result<String, String> {
    let parts: Vec<&str> = segment_id.rsplitn(3, ':').collect();
    if parts.len() != 3 {
        return Err("CF_TRANSLATE_BAD_ID: not a segment id".into());
    }
    let (seg, page, doc) = (parts[0].parse::<i64>(), parts[1].parse::<i64>(), parts[2]);
    match (seg, page) {
        (Ok(s), Ok(p)) if !doc.is_empty() => Ok(seg_key(doc, p, s)),
        _ => Err("CF_TRANSLATE_BAD_ID: not a segment id".into()),
    }
}

fn words_json(layer: &crate::vault::PageTextLayerRow) -> Value {
    Value::Array(
        layer
            .words
            .iter()
            .map(|w| json!({"char_start": w.char_start, "char_end": w.char_end, "raw_engine_confidence": w.raw_engine_confidence, "calibrated_confidence": w.calibrated_confidence}))
            .collect(),
    )
}

/// Translate every page of a document's recognized text. Stops cleanly when `cancelled()` turns true; what is stored stays.
pub fn translate_document(
    vault: &VaultManager,
    case_id: &str,
    doc_id: &str,
    engine: &dyn Engine,
    cancelled: &dyn Fn() -> bool,
    progress: &dyn Fn(u32, &str),
) -> Result<Summary, String> {
    let layers = vault.page_text_layer_list(case_id, Some(doc_id)).map_err(|e| e.to_string())?;
    if layers.is_empty() {
        return Err("CF_TRANSLATE_NO_TEXT: recognize the document first; there is no text to translate".into());
    }
    let mut summary = Summary::default();
    let total_pages = layers.len();
    for (n, layer) in layers.iter().enumerate() {
        if cancelled() {
            summary.cancelled = true;
            break;
        }
        progress((5 + (n * 90) / total_pages) as u32, &format!("page {} of {}", n + 1, total_pages));
        let prepared = engine.prepare(doc_id, layer.page_index, &layer.full_text, &words_json(layer))?;
        summary.pages += 1;
        // segments from an earlier run that no longer exist (the text changed) are removed
        let page_prefix = format!("seg:{doc_id}:{:06}:", layer.page_index);
        let kept: Vec<String> = prepared.iter().filter(|p| p["translate"] == true).map(|p| seg_key(doc_id, layer.page_index, p["seg_index"].as_i64().unwrap_or(0))).collect();
        for (key, _) in vault.translation_kv_list(case_id, &page_prefix).map_err(|e| e.to_string())? {
            if !kept.contains(&key) {
                vault.translation_kv_delete_prefix(case_id, &key).map_err(|e| e.to_string())?;
            }
        }
        for seg in &prepared {
            summary.segments += 1;
            if seg["translate"] != true {
                summary.carried += 1;
                continue;
            }
            if cancelled() {
                summary.cancelled = true;
                break;
            }
            let idx = seg["seg_index"].as_i64().unwrap_or(0);
            let key = seg_key(doc_id, layer.page_index, idx);
            let source_text = seg["source_text"].as_str().unwrap_or("");
            let existing = vault.translation_kv_list(case_id, &key).map_err(|e| e.to_string())?;
            if existing.iter().any(|(k, v)| *k == key && v["segment"]["source_text"] == source_text) {
                summary.resumed += 1;
                continue;
            }
            let output = engine.translate(seg["masked"].as_str().unwrap_or(""))?;
            let record = engine.finish(doc_id, layer.page_index, seg, &output, layer.calibration_id.as_deref())?;
            if record["review"]["segment_flagged"] == true {
                summary.flagged += 1;
            }
            vault
                .translation_kv_put(case_id, &key, &json!({"segment": record["segment"], "review": record["review"], "translated_at_unix_ms": now_ms()}))
                .map_err(|e| e.to_string())?;
            summary.translated += 1;
        }
        if summary.cancelled {
            break;
        }
    }
    progress(100, if summary.cancelled { "stopped; progress is kept" } else { "done" });
    Ok(summary)
}

/// Stored records, optionally for one document, in page and segment order.
pub fn list(vault: &VaultManager, case_id: &str, doc_id: Option<&str>) -> Result<Vec<Value>, String> {
    let prefix = match doc_id {
        Some(d) => format!("seg:{d}:"),
        None => "seg:".to_string(),
    };
    Ok(vault.translation_kv_list(case_id, &prefix).map_err(|e| e.to_string())?.into_iter().map(|(_, v)| v).collect())
}

fn unconfirmed(segment: &Value) -> Vec<Value> {
    segment["critical_fields"]
        .as_array()
        .map(|a| a.iter().filter(|f| f["counsel_confirmed"] != true).map(|f| f["field_id"].clone()).collect())
        .unwrap_or_default()
}

/// Counsel confirms one critical field with the value counsel reads in the original. Appends an event; never edits the machine text.
pub fn confirm_field(vault: &VaultManager, case_id: &str, segment_id: &str, field_id: &str, confirmed_value: &str, note: Option<&str>) -> Result<Value, String> {
    let value = confirmed_value.trim();
    if value.is_empty() || value.chars().count() > 300 {
        return Err("CF_TRANSLATE_CONFIRM_VALUE: type the value as the original document gives it (1 to 300 characters)".into());
    }
    change_field(vault, case_id, segment_id, field_id, Some(value), note)
}

/// Counsel withdraws a confirmation. The field is unconfirmed again.
pub fn unconfirm_field(vault: &VaultManager, case_id: &str, segment_id: &str, field_id: &str, note: Option<&str>) -> Result<Value, String> {
    change_field(vault, case_id, segment_id, field_id, None, note)
}

fn change_field(vault: &VaultManager, case_id: &str, segment_id: &str, field_id: &str, confirmed: Option<&str>, note: Option<&str>) -> Result<Value, String> {
    let key = key_of_segment_id(segment_id)?;
    let mut record = vault
        .translation_kv_list(case_id, &key)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| v)
        .ok_or_else(|| "CF_TRANSLATE_NOT_FOUND: no such translated segment".to_string())?;
    let fields = record["segment"]["critical_fields"].as_array_mut().ok_or_else(|| "CF_TRANSLATE_NOT_FOUND: segment has no fields".to_string())?;
    let field = fields.iter_mut().find(|f| f["field_id"] == field_id).ok_or_else(|| "CF_TRANSLATE_NOT_FOUND: no such field in this segment".to_string())?;
    let before = json!({"counsel_confirmed": field["counsel_confirmed"], "confirmed_value": field["confirmed_value"]});
    match confirmed {
        Some(v) => {
            field["counsel_confirmed"] = json!(true);
            field["confirmed_value"] = json!(v);
        }
        None => {
            field["counsel_confirmed"] = json!(false);
            field["confirmed_value"] = Value::Null;
        }
    }
    let after = json!({"counsel_confirmed": field["counsel_confirmed"], "confirmed_value": field["confirmed_value"]});
    let source_value = field["source_value"].clone();
    let machine_value = field["translated_value"].clone();
    record["review"]["unconfirmed_fields"] = Value::Array(unconfirmed(&record["segment"]));
    vault.translation_kv_put(case_id, &key, &record).map_err(|e| e.to_string())?;
    let at = now_ms();
    vault
        .translation_kv_put(
            case_id,
            &format!("evt:{at:016}:{segment_id}:{field_id}"),
            &json!({"event": if confirmed.is_some() { "CONFIRM" } else { "REVOKE" }, "segment_id": segment_id, "field_id": field_id, "source_value": source_value,
                    "machine_value": machine_value, "before": before, "after": after, "note": note, "at_unix_ms": at}),
        )
        .map_err(|e| e.to_string())?;
    Ok(record)
}

/// The history of confirmations, oldest first.
pub fn events(vault: &VaultManager, case_id: &str) -> Result<Vec<Value>, String> {
    Ok(vault.translation_kv_list(case_id, "evt:").map_err(|e| e.to_string())?.into_iter().map(|(_, v)| v).collect())
}

/// One verifier source per translated page, marked `translated` so guardrail G10 applies to anything built on it.
pub fn support_sources(vault: &VaultManager, case_id: &str, max_chars: usize) -> Result<Vec<crate::proposition::SupportSource>, String> {
    let mut pages: Vec<((String, i64), Vec<String>)> = Vec::new();
    for rec in list(vault, case_id, None)? {
        let seg = &rec["segment"];
        let (doc, page) = (seg["doc_id"].as_str().unwrap_or("").to_string(), seg["page_index"].as_i64().unwrap_or(0));
        let text = seg["target_text"].as_str().unwrap_or("").to_string();
        match pages.last_mut() {
            Some((k, v)) if *k == (doc.clone(), page) => v.push(text),
            _ => pages.push(((doc, page), vec![text])),
        }
    }
    let mut used = 0usize;
    let mut out = Vec::new();
    for ((doc, page), texts) in pages {
        let text = texts.join("\n");
        if used + text.chars().count() > max_chars {
            break;
        }
        used += text.chars().count();
        out.push(crate::proposition::SupportSource {
            support_id: format!("translated:{case_id}:{doc}:{page}"),
            kind: "MATTER_SOURCE".into(),
            exact_text: text,
            source_id: None,
            chunk_id: None,
            authority_id: None,
            doc_id: Some(doc),
            page: Some(page + 1),
            paragraph: None,
            trust_level: None,
            temporal_status: None,
            superseded_status: None,
            permitted: true,
            translated: true,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------- facts read from a translation

/// Every provenance block in the graph becomes TRANSLATED: not confirmed, held below the downstream-trust line, and pointing at the page (the
/// extractor's character spans refer to the English text, not to the original page, so a span would point at the wrong place).
fn mark_translated(v: &mut Value) {
    match v {
        Value::Object(m) => {
            if let Some(Value::Object(p)) = m.get_mut("provenance") {
                if p.contains_key("origin") {
                    p.insert("origin".into(), json!(PROVENANCE_ORIGIN));
                    p.insert("counsel_confirmed".into(), json!(false));
                    let c = p.get("confidence").and_then(Value::as_f64).unwrap_or(0.0).min(TRANSLATED_FACT_CONFIDENCE_CAP);
                    p.insert("confidence".into(), json!(c));
                    p.insert("notes".into(), json!(format!("read from a machine translation - wording not verified; critical fields need counsel's confirmation. {}", p.get("notes").and_then(Value::as_str).unwrap_or(""))));
                    for k in ["sources", "source"] {
                        if let Some(s) = p.get_mut(k) {
                            to_page_only(s);
                        }
                    }
                }
            }
            for (_, x) in m.iter_mut() {
                mark_translated(x);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(mark_translated),
        _ => {}
    }
}

fn to_page_only(v: &mut Value) {
    match v {
        Value::Array(a) => a.iter_mut().for_each(to_page_only),
        Value::Object(m) => {
            m.insert("kind".into(), json!("page_only"));
            m.insert("char_start".into(), Value::Null);
            m.insert("char_end".into(), Value::Null);
            m.insert("bbox".into(), Value::Null);
            m.insert("content_hash".into(), Value::Null);
        }
        _ => {}
    }
}

/// The facts the deterministic extractor finds in a document's English translation, as a separate graph. It is never merged into the matter graph:
/// it is shown beside it, every item TRANSLATED and unconfirmed.
pub fn derive_graph(vault: &VaultManager, case_id: &str, jurisdiction: &str, doc_id: &str) -> Result<Value, String> {
    let mut by_page: Vec<(i64, Vec<String>, f64)> = Vec::new();
    for rec in list(vault, case_id, Some(doc_id))? {
        let seg = &rec["segment"];
        let page = seg["page_index"].as_i64().unwrap_or(0);
        let conf = seg["ocr_confidence"]["calibrated"].as_f64().or_else(|| seg["ocr_confidence"]["raw"].as_f64()).unwrap_or(0.5);
        let text = seg["target_text"].as_str().unwrap_or("").to_string();
        match by_page.last_mut() {
            Some((p, v, c)) if *p == page => {
                v.push(text);
                *c = c.min(conf);
            }
            _ => by_page.push((page, vec![text], conf)),
        }
    }
    if by_page.is_empty() {
        return Err("CF_TRANSLATE_NOT_FOUND: translate the document first".into());
    }
    let pages: Vec<(String, i64, String, f64)> = by_page.into_iter().map(|(p, v, c)| (doc_id.to_string(), p, v.join("\n"), c)).collect();
    let mut graph = crate::docpipe::extract_translated_graph(case_id, jurisdiction, &pages);
    mark_translated(&mut graph);
    graph["schema_version"] = json!("2.1.0");
    graph["translation_notice"] = json!(LABEL);
    vault.translation_kv_put(case_id, &format!("graph:{doc_id}"), &graph).map_err(|e| e.to_string())?;
    Ok(graph)
}

pub fn graph_get(vault: &VaultManager, case_id: &str, doc_id: &str) -> Result<Option<Value>, String> {
    let key = format!("graph:{doc_id}");
    Ok(vault.translation_kv_list(case_id, &key).map_err(|e| e.to_string())?.into_iter().find(|(k, _)| *k == key).map(|(_, v)| v))
}

// ---------------------------------------------------------------- jobs

#[derive(Debug, Clone, Serialize)]
pub struct JobStatus {
    pub job_id: String,
    pub case_id: String,
    pub doc_ids: Vec<String>,
    pub state: String,
    pub progress_percent: u32,
    pub message: String,
    pub summary: Option<Summary>,
    pub error: Option<String>,
}

struct JobRec {
    status: JobStatus,
    cancel: Arc<AtomicBool>,
}

static JOBS: Mutex<Option<HashMap<String, JobRec>>> = Mutex::new(None);

fn with_jobs<R>(f: impl FnOnce(&mut HashMap<String, JobRec>) -> R) -> R {
    let mut g = JOBS.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(HashMap::new))
}

pub fn job_status(job_id: &str) -> Option<JobStatus> {
    with_jobs(|j| j.get(job_id).map(|r| r.status.clone()))
}

pub fn job_cancel(job_id: &str) -> bool {
    with_jobs(|j| j.get(job_id).map(|r| r.cancel.store(true, Ordering::SeqCst)).is_some())
}

/// Documents of the case whose recognized text contains Devanagari, i.e. the ones that need a translation.
pub fn docs_with_hindi(vault: &VaultManager, case_id: &str) -> Result<Vec<String>, String> {
    let mut docs: Vec<String> = Vec::new();
    for layer in vault.page_text_layer_list(case_id, None).map_err(|e| e.to_string())? {
        if layer.full_text.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c)) && !docs.contains(&layer.doc_id) {
            docs.push(layer.doc_id);
        }
    }
    Ok(docs)
}

/// Start a background job over the given documents, one after another, on the primary model's slot. `on_done` runs when it ends (release the slot).
pub fn start_job(
    vault: Arc<VaultManager>,
    case_id: String,
    doc_ids: Vec<String>,
    emit: impl Fn(&JobStatus) + Send + 'static,
    on_done: impl FnOnce() + Send + 'static,
) -> JobStatus {
    let job_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    let status = JobStatus { job_id: job_id.clone(), case_id: case_id.clone(), doc_ids: doc_ids.clone(), state: "running".into(), progress_percent: 0, message: "queued".into(), summary: None, error: None };
    with_jobs(|j| j.insert(job_id.clone(), JobRec { status: status.clone(), cancel: cancel.clone() }));
    std::thread::spawn(move || {
        let update = |f: &dyn Fn(&mut JobStatus)| {
            let s = with_jobs(|j| {
                j.get_mut(&job_id).map(|r| {
                    f(&mut r.status);
                    r.status.clone()
                })
            });
            if let Some(s) = s {
                emit(&s);
            }
        };
        let mut total = Summary::default();
        let mut failure: Option<String> = None;
        let count = doc_ids.len().max(1);
        for (i, doc) in doc_ids.iter().enumerate() {
            let progress = |pct: u32, msg: &str| {
                let overall = ((i as u32) * 100 + pct) / count as u32;
                update(&|s| {
                    s.progress_percent = overall;
                    s.message = format!("document {} of {}: {}", i + 1, count, msg);
                });
            };
            match translate_document(&vault, &case_id, doc, &SidecarEngine, &|| cancel.load(Ordering::SeqCst), &progress) {
                Ok(s) => {
                    total.pages += s.pages;
                    total.segments += s.segments;
                    total.translated += s.translated;
                    total.resumed += s.resumed;
                    total.carried += s.carried;
                    total.flagged += s.flagged;
                    total.cancelled |= s.cancelled;
                    if s.cancelled {
                        break;
                    }
                }
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        update(&|s| {
            s.state = if failure.is_some() { "failed" } else if total.cancelled { "cancelled" } else { "done" }.into();
            s.progress_percent = if failure.is_some() { s.progress_percent } else { 100 };
            s.summary = Some(total.clone());
            s.error = failure.clone();
            s.message = match (&failure, total.cancelled) {
                (Some(e), _) => format!("stopped: {e}. Progress is kept; start again to continue."),
                (None, true) => "stopped. Progress is kept; start again to continue.".into(),
                _ => "done".into(),
            };
        });
        on_done();
    });
    status
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::{CaseWizardInput, DocumentIngestRequest, PageTextLayerRow};
    use base64::Engine as _;

    /// A fixed engine: every non-empty line is a segment; the "model" upper-cases it; a line with a year gets one date field.
    struct Fixed {
        calls: std::cell::Cell<usize>,
    }
    impl Engine for Fixed {
        fn prepare(&self, _doc: &str, _page: i64, text: &str, _words: &Value) -> Result<Vec<Value>, String> {
            let mut cursor = 0usize;
            let mut out = Vec::new();
            for (i, line) in text.split('\n').enumerate() {
                let (s, e) = (cursor, cursor + line.chars().count());
                cursor = e + 1;
                if line.trim().is_empty() {
                    continue;
                }
                let has_dev = line.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
                out.push(json!({"seg_index": i, "char_start": s, "char_end": e, "source_text": line, "translate": has_dev, "fields": [], "masked": line, "ocr_confidence": {"raw": 0.9, "calibrated": 0.95}}));
            }
            Ok(out)
        }
        fn translate(&self, masked: &str) -> Result<String, String> {
            self.calls.set(self.calls.get() + 1);
            Ok(format!("EN[{masked}]"))
        }
        fn finish(&self, doc: &str, page: i64, prepared: &Value, output: &str, _cal: Option<&str>) -> Result<Value, String> {
            let src = prepared["source_text"].as_str().unwrap();
            let fields = if src.contains("२०२४") {
                vec![json!({"field_id": "f1", "field_kind": "DATE", "source_char_start": 0, "source_char_end": 1, "source_value": "१० मार्च २०२४", "translated_value": "10 March 2024", "counsel_confirmed": false, "confirmed_value": null})]
            } else {
                vec![]
            };
            Ok(json!({
                "segment": {"schema_version": "1.0.0", "segment_id": format!("{doc}:{page}:{}", prepared["seg_index"]), "doc_id": doc, "page_index": page, "source_script": "DEVANAGARI", "source_language": "hi",
                            "source_text": src, "source_char_start": prepared["char_start"], "source_char_end": prepared["char_end"],
                            "ocr_confidence": {"raw": 0.9, "calibrated": 0.95, "calibration_id": null}, "target_language": "en", "target_text": output,
                            "model": {"model_id": "fixed", "digest": "none"}, "provenance_origin": "TRANSLATED", "quotation_verifiable": false, "critical_fields": fields},
                "review": {"label": LABEL, "segment_flagged": false, "invented_markers": [], "field_flags": {}, "unconfirmed_fields": fields.iter().map(|f| f["field_id"].clone()).collect::<Vec<_>>()}
            }))
        }
    }

    fn layer(doc: &str, page: i64, text: &str) -> PageTextLayerRow {
        PageTextLayerRow { doc_id: doc.into(), page_index: page, recognizer_id: "test".into(), recognized_at_unix_ms: 0, full_text: text.into(), mean_confidence: 0.9, page_quality_warn: false, notes: None, calibration_id: None, critical_fields: json!([]), words: vec![] }
    }

    fn setup() -> (std::sync::Arc<VaultManager>, String, String, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("caseforge-translation-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mgr = std::sync::Arc::new(VaultManager::new(dir.clone()));
        mgr.init_and_unlock("pw").unwrap();
        let id = mgr
            .create_case_with_wizard("pw", CaseWizardInput { cause_title: "State v Verma".into(), court: "District Court".into(), jurisdiction: "India".into(), matter_type: "Criminal".into(), user_side: "Defence".into(), code_era: None, language: None })
            .unwrap()
            .case_id;
        mgr.open_case(&id, "pw").unwrap();
        // the vault knows a document before it takes a text layer for it
        let content = "FIR placeholder";
        let doc = mgr
            .doc_ingest(&id, DocumentIngestRequest { original_filename: "fir.txt".into(), content_base64: base64::engine::general_purpose::STANDARD.encode(content), type_tag: Some("FIR".into()), language_tag: Some("hi".into()) })
            .unwrap()
            .doc_id;
        (mgr, id, doc, dir)
    }

    const PAGE: &str = "प्रथम सूचना रिपोर्ट\nदिनांक १० मार्च २०२४ को शिकायत दर्ज हुई।\nCriminal Case No. 456/2024";

    #[test]
    fn a_document_is_translated_segment_by_segment_stored_aligned_and_resumed() {
        let (v, case, doc, dir) = setup();
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, PAGE)]).unwrap();
        let engine = Fixed { calls: std::cell::Cell::new(0) };
        let first = translate_document(&v, &case, &doc, &engine, &|| false, &|_, _| {}).unwrap();
        assert_eq!((first.translated, first.carried, first.resumed), (2, 1, 0), "two Hindi segments translated, the English line carried as it is: {first:?}");
        let rows = list(&v, &case, Some(&doc)).unwrap();
        assert_eq!(rows.len(), 2);
        for r in &rows {
            let s = &r["segment"];
            assert_eq!(s["provenance_origin"], "TRANSLATED");
            assert_eq!(s["quotation_verifiable"], false);
            assert!(s["source_text"].as_str().unwrap().chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c)), "the original is kept beside the translation");
            assert!(s["target_text"].as_str().unwrap().starts_with("EN["));
            assert_eq!(r["review"]["label"], LABEL);
        }
        // a second run keeps what is stored and asks the model for nothing
        let calls_before = engine.calls.get();
        let second = translate_document(&v, &case, &doc, &engine, &|| false, &|_, _| {}).unwrap();
        assert_eq!((second.translated, second.resumed), (0, 2));
        assert_eq!(engine.calls.get(), calls_before, "a resumed run makes no model call");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn stopping_keeps_progress_and_a_later_run_finishes() {
        let (v, case, doc, dir) = setup();
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, "एक वाक्य\nदूसरा वाक्य\nतीसरा वाक्य")]).unwrap();
        let engine = Fixed { calls: std::cell::Cell::new(0) };
        let n = std::cell::Cell::new(0);
        // cancel after the first segment has been stored
        let stopped = translate_document(&v, &case, &doc, &engine, &|| { n.set(n.get() + 1); n.get() > 2 }, &|_, _| {}).unwrap();
        assert!(stopped.cancelled && stopped.translated == 1, "{stopped:?}");
        assert_eq!(list(&v, &case, Some(&doc)).unwrap().len(), 1, "what was done is kept");
        let rest = translate_document(&v, &case, &doc, &engine, &|| false, &|_, _| {}).unwrap();
        assert_eq!((rest.resumed, rest.translated, rest.cancelled), (1, 2, false));
        assert_eq!(list(&v, &case, Some(&doc)).unwrap().len(), 3);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn changed_source_text_replaces_the_segment_and_a_vanished_segment_is_removed() {
        let (v, case, doc, dir) = setup();
        let engine = Fixed { calls: std::cell::Cell::new(0) };
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, "पहला वाक्य\nदूसरा वाक्य")]).unwrap();
        translate_document(&v, &case, &doc, &engine, &|| false, &|_, _| {}).unwrap();
        // counsel corrected a word and the second line is gone
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, "पहला सही वाक्य")]).unwrap();
        let s = translate_document(&v, &case, &doc, &engine, &|| false, &|_, _| {}).unwrap();
        assert_eq!((s.translated, s.resumed), (1, 0));
        let rows = list(&v, &case, Some(&doc)).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["segment"]["source_text"], "पहला सही वाक्य");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn counsel_confirms_a_field_by_an_event_and_can_withdraw_it_and_the_machine_text_never_changes() {
        let (v, case, doc, dir) = setup();
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, "दिनांक १० मार्च २०२४ को शिकायत दर्ज हुई।")]).unwrap();
        translate_document(&v, &case, &doc, &Fixed { calls: std::cell::Cell::new(0) }, &|| false, &|_, _| {}).unwrap();
        let rows = list(&v, &case, None).unwrap();
        let id = rows[0]["segment"]["segment_id"].as_str().unwrap().to_string();
        assert_eq!(rows[0]["review"]["unconfirmed_fields"], json!(["f1"]));
        assert!(confirm_field(&v, &case, &id, "f1", "  ", None).is_err(), "an empty confirmation is refused");
        assert!(confirm_field(&v, &case, &id, "f9", "10 March 2024", None).is_err(), "an unknown field is refused");
        let machine = rows[0]["segment"]["critical_fields"][0]["translated_value"].clone();
        let done = confirm_field(&v, &case, &id, "f1", "10 March 2024", Some("read against the FIR")).unwrap();
        assert_eq!(done["segment"]["critical_fields"][0]["counsel_confirmed"], true);
        assert_eq!(done["segment"]["critical_fields"][0]["confirmed_value"], "10 March 2024");
        assert_eq!(done["segment"]["critical_fields"][0]["translated_value"], machine, "the machine's value is kept beside counsel's");
        assert_eq!(done["review"]["unconfirmed_fields"], json!([]));
        let undone = unconfirm_field(&v, &case, &id, "f1", None).unwrap();
        assert_eq!(undone["segment"]["critical_fields"][0]["counsel_confirmed"], false);
        assert_eq!(undone["segment"]["critical_fields"][0]["confirmed_value"], Value::Null);
        let history = events(&v, &case).unwrap();
        assert_eq!(history.iter().map(|e| e["event"].as_str().unwrap()).collect::<Vec<_>>(), vec!["CONFIRM", "REVOKE"]);
        assert_eq!(history[0]["after"]["confirmed_value"], "10 March 2024");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn a_translated_page_reaches_the_verifier_marked_translated() {
        let (v, case, doc, dir) = setup();
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, "पहला वाक्य\nदूसरा वाक्य")]).unwrap();
        translate_document(&v, &case, &doc, &Fixed { calls: std::cell::Cell::new(0) }, &|| false, &|_, _| {}).unwrap();
        let sources = support_sources(&v, &case, 10_000).unwrap();
        assert_eq!(sources.len(), 1);
        assert!(sources[0].translated && sources[0].kind == "MATTER_SOURCE" && sources[0].doc_id.as_deref() == Some(doc.as_str()));
        assert!(sources[0].exact_text.contains("EN[पहला वाक्य]"));
        // a verified-looking fact built on it is held below GREEN by the verifier
        let ledger = crate::proposition::verify(crate::proposition::VerifyRequest {
            output_kind: "REASONING".into(),
            propositions: vec![crate::proposition::PropositionInput {
                proposition_id: "p1".into(), class: "FACTUAL_BASIS".into(), text: "EN[पहला वाक्य]".into(), support_ids: vec![sources[0].support_id.clone()], basis_ids: vec![],
                support_claims: vec![crate::proposition::SupportClaim { support_id: sources[0].support_id.clone(), exact_excerpt: "EN[पहला वाक्य]".into() }], quotation: None,
            }],
            support_sources: sources,
            permitted_authority_ids: vec![],
        })
        .unwrap();
        assert_eq!(ledger.propositions[0].verification_status, "AMBER");
        assert_eq!(ledger.trusted_count, 0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn facts_read_from_a_translation_carry_translated_provenance_and_are_not_trusted() {
        let mut g = json!({"schema_version": "2.0.0", "nodes": [{"node_id": "n1", "provenance": {"origin": "EXTRACTED", "confidence": 0.9, "counsel_confirmed": true, "notes": "x",
                           "source": {"doc_id": "d", "page_index": 0, "kind": "pdf_span", "char_start": 3, "char_end": 9, "quote": "q", "content_hash": "sha256:1"},
                           "sources": [{"doc_id": "d", "page_index": 0, "kind": "pdf_span", "char_start": 3, "char_end": 9, "quote": "q", "content_hash": "sha256:1"}]}}],
                           "parties": [{"party_id": "p1", "provenance": {"origin": "EXTRACTED", "confidence": 0.8, "counsel_confirmed": false, "sources": [{"doc_id": "d", "page_index": 1, "kind": "pdf_span", "char_start": 0, "char_end": 2, "quote": "q", "content_hash": null}]}}]});
        mark_translated(&mut g);
        for p in [&g["nodes"][0]["provenance"], &g["parties"][0]["provenance"]] {
            assert_eq!(p["origin"], PROVENANCE_ORIGIN);
            assert_eq!(p["counsel_confirmed"], false);
            assert!(p["confidence"].as_f64().unwrap() <= TRANSLATED_FACT_CONFIDENCE_CAP);
            assert_eq!(p["sources"][0]["kind"], "page_only");
            assert_eq!(p["sources"][0]["char_start"], Value::Null, "a span in the English text must not point into the original page");
        }
        assert_eq!(g["nodes"][0]["provenance"]["source"]["kind"], "page_only");
    }

    #[test]
    fn the_module_opens_no_network_and_reads_no_file() {
        let src = include_str!("translation.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for word in ["TcpStream", "UdpSocket", "reqwest", "std::net", "std::fs", "ollama"] {
            assert!(!code.contains(word), "translation.rs must not use {word}");
        }
    }

    #[test]
    fn facts_found_in_a_translation_come_out_of_the_real_extractor_as_translated() {
        let (v, case, doc, dir) = setup();
        v.page_text_layer_replace(&case, &doc, &[layer(&doc, 0, "एक वाक्य")]).unwrap();
        // a stored translation whose English the deterministic extractor can read
        let rec = json!({"segment": {"doc_id": doc, "page_index": 0, "target_text": "Plaintiff: Asha Rao
Issue: whether payment was made
Fact: payment was made
Evidence: bank statement
Rule 10",
                                      "ocr_confidence": {"calibrated": 0.9, "raw": 0.9}}, "review": {}});
        v.translation_kv_put(&case, &seg_key(&doc, 0, 0), &rec).unwrap();
        let g = derive_graph(&v, &case, "India", &doc).unwrap();
        assert_eq!(g["schema_version"], "2.1.0");
        assert_eq!(g["translation_notice"], LABEL);
        let nodes = g["nodes"].as_array().unwrap();
        assert!(!nodes.is_empty(), "the extractor found nothing in the translated English");
        for n in nodes {
            assert_eq!(n["provenance"]["origin"], PROVENANCE_ORIGIN, "{n}");
            assert_eq!(n["provenance"]["counsel_confirmed"], false);
            assert!(n["provenance"]["confidence"].as_f64().unwrap() <= TRANSLATED_FACT_CONFIDENCE_CAP);
            assert_eq!(n["provenance"]["sources"][0]["kind"], "page_only");
        }
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            // the gate validates this graph against contracts/case_graph_v2_1.schema.json
            std::fs::write(std::path::Path::new(&dir).join("translated_graph.json"), serde_json::to_string_pretty(&g).unwrap()).ok();
        }
        assert!(graph_get(&v, &case, &doc).unwrap().is_some(), "the derived graph is stored beside the matter, not merged into it");
        assert!(v.case_graph_get(&case).map(|g| g.is_none()).unwrap_or(true), "the matter's own graph is untouched");
        std::fs::remove_dir_all(dir).ok();
    }
}
