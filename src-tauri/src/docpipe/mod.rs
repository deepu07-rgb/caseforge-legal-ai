//! docpipe triage (B-S1-05) + recognition (B-S1-06) + skeleton (B-S1-07)
//! + overnight batch resume (B-S1-10).

pub(crate) mod pdf_adapter;
mod recognize;
mod skeleton;
pub(crate) use skeleton::now_rfc3339;
mod triage;

pub use recognize::{recognize_document_bytes, RECOGNIZER_ID};
pub use skeleton::{assert_extracted_sources, extract_case_graph};
pub use triage::{classify_document_bytes, CLASSIFIER_ID, TRIAGE_CLASSES};

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

use crate::vault::{PageSourceRow, PageTextLayerRow, PageTriageRow, PageWordRow, VaultManager};

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TriageJobState {
    Running,
    Completed,
    Failed,
    Unknown,
}

#[derive(Clone, Serialize)]
pub struct TriageJobStatus {
    pub job_id: String,
    pub case_id: String,
    pub doc_id: String,
    pub state: TriageJobState,
    pub progress_percent: u32,
    pub message: String,
    pub page_count: Option<u32>,
    pub error: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct TriageProgressEvent {
    pub job_id: String,
    pub case_id: String,
    pub doc_id: String,
    pub progress_percent: u32,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct TriageStartResult {
    pub job_id: String,
    pub accepted: bool,
}

struct JobRec {
    status: TriageJobStatus,
    started: Instant,
}

pub struct DocpipeState {
    jobs: Mutex<HashMap<String, JobRec>>,
    /// Soft-cancel flags for overnight batch (AC-6.8 kill path).
    batch_cancel: Mutex<HashMap<String, bool>>,
}

impl DocpipeState {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            batch_cancel: Mutex::new(HashMap::new()),
        }
    }

    pub fn status(&self, job_id: &str) -> TriageJobStatus {
        let jobs = self.jobs.lock().ok();
        if let Some(g) = jobs {
            if let Some(rec) = g.get(job_id) {
                return rec.status.clone();
            }
        }
        TriageJobStatus {
            job_id: job_id.into(),
            case_id: String::new(),
            doc_id: String::new(),
            state: TriageJobState::Unknown,
            progress_percent: 0,
            message: "unknown job".into(),
            page_count: None,
            error: None,
        }
    }

    pub fn request_batch_cancel(&self, job_id: &str) {
        if let Ok(mut m) = self.batch_cancel.lock() {
            m.insert(job_id.to_string(), true);
        }
    }

    pub fn clear_batch_cancel(&self, job_id: &str) {
        if let Ok(mut m) = self.batch_cancel.lock() {
            m.remove(job_id);
        }
    }

    pub fn is_batch_cancelled(&self, job_id: &str) -> bool {
        self.batch_cancel
            .lock()
            .ok()
            .and_then(|m| m.get(job_id).copied())
            .unwrap_or(false)
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn page_source_rows(
    doc_id: &str,
    pages: &[triage::PageTriageResult],
    parsed_at: i64,
) -> Vec<PageSourceRow> {
    pages
        .iter()
        .filter_map(|page| {
            let raw = page.page_source_json.as_ref()?;
            let value: serde_json::Value = serde_json::from_str(raw).ok()?;
            Some(PageSourceRow {
                doc_id: doc_id.into(),
                page_index: page.page_index,
                parser_id: value.get("parser_id")?.as_str()?.into(),
                source_kind: value.get("source_kind")?.as_str()?.into(),
                state: value.get("state")?.as_str()?.into(),
                width_points: value.get("width_points")?.as_f64()?,
                height_points: value.get("height_points")?.as_f64()?,
                rotation_degrees: value.get("rotation_degrees")?.as_i64()?,
                source_sha256: value.get("source_sha256")?.as_str()?.into(),
                page_source_json: raw.clone(),
                parsed_at_unix_ms: parsed_at,
            })
        })
        .collect()
}

/// Start async triage for one document (off UI thread).
pub fn start_triage_job(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
    doc_id: String,
) -> Result<TriageStartResult, String> {
    // Ensure doc exists and is readable before accepting job.
    let _bytes = vault
        .doc_content_bytes(&case_id, &doc_id)
        .map_err(|e| e.to_string())?;
    let meta = vault
        .doc_get(&case_id, &doc_id)
        .map_err(|e| e.to_string())?;

    let job_id = Uuid::new_v4().to_string();
    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        jobs.insert(
            job_id.clone(),
            JobRec {
                status: TriageJobStatus {
                    job_id: job_id.clone(),
                    case_id: case_id.clone(),
                    doc_id: doc_id.clone(),
                    state: TriageJobState::Running,
                    progress_percent: 0,
                    message: "queued".into(),
                    page_count: None,
                    error: None,
                },
                started: Instant::now(),
            },
        );
    }

    let job_id_t = job_id.clone();
    let state_t = state.clone();
    let app_t = app.clone();
    let vault_t = vault.clone();
    let case_t = case_id.clone();
    let doc_t = doc_id.clone();
    let filename = meta.original_filename.clone();
    let source_format = meta.source_format.clone();

    thread::spawn(move || {
        let emit = |pct: u32, msg: &str| {
            if let Ok(mut jobs) = state_t.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_id_t) {
                    rec.status.progress_percent = pct;
                    rec.status.message = msg.into();
                }
            }
            let _ = app_t.emit(
                "docpipe_triage_progress",
                TriageProgressEvent {
                    job_id: job_id_t.clone(),
                    case_id: case_t.clone(),
                    doc_id: doc_t.clone(),
                    progress_percent: pct,
                    message: msg.into(),
                },
            );
        };

        emit(5, "loading document");
        let bytes = match vault_t.doc_content_bytes(&case_t, &doc_t) {
            Ok(b) => b,
            Err(e) => {
                fail_job(&state_t, &job_id_t, &e.to_string());
                emit(100, "failed");
                return;
            }
        };

        emit(20, "classifying pages");
        // Small yield so progress IPC is observable (AC-3.5 spirit).
        thread::sleep(Duration::from_millis(30));

        let pages = match classify_document_bytes(&bytes, &source_format, &filename) {
            Ok(value) => value,
            Err(error) => {
                fail_job(&state_t, &job_id_t, &error);
                emit(100, "failed");
                return;
            }
        };
        let page_count = pages.len() as u32;
        let parsed_at = now_ms();
        let source_rows = page_source_rows(&doc_t, &pages, parsed_at);
        if !source_rows.is_empty() {
            if let Err(error) = vault_t.page_source_replace(&case_t, &doc_t, &source_rows) {
                fail_job(&state_t, &job_id_t, &error.to_string());
                emit(100, "failed");
                return;
            }
        }
        emit(60, &format!("writing {page_count} page label(s)"));

        let classified_at = now_ms();
        let rows: Vec<PageTriageRow> = pages
            .into_iter()
            .map(|p| PageTriageRow {
                doc_id: doc_t.clone(),
                page_index: p.page_index,
                triage_class: p.triage_class,
                confidence: p.confidence,
                classifier_id: p.classifier_id,
                classified_at_unix_ms: classified_at,
                notes: p.notes,
            })
            .collect();

        if let Err(e) = vault_t.page_triage_replace(&case_t, &doc_t, &rows) {
            fail_job(&state_t, &job_id_t, &e.to_string());
            emit(100, "failed");
            return;
        }

        // Update page_count on document meta.
        let _ = vault_t.doc_set_page_count(&case_t, &doc_t, page_count as i64);

        if let Ok(mut jobs) = state_t.jobs.lock() {
            if let Some(rec) = jobs.get_mut(&job_id_t) {
                rec.status.state = TriageJobState::Completed;
                rec.status.progress_percent = 100;
                rec.status.message = "completed".into();
                rec.status.page_count = Some(page_count);
                rec.status.error = None;
            }
        }
        emit(100, "completed");
        let _ = app_t.emit(
            "docpipe_triage_progress",
            TriageProgressEvent {
                job_id: job_id_t,
                case_id: case_t,
                doc_id: doc_t,
                progress_percent: 100,
                message: "completed".into(),
            },
        );
    });

    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

/// Start async triage for every document in an open case (off UI thread).
pub fn start_triage_case_job(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
) -> Result<TriageStartResult, String> {
    let docs = vault.doc_list(&case_id).map_err(|e| e.to_string())?;
    if docs.is_empty() {
        return Err("CF_DOCPIPE_TRIAGE_FAILED: no documents in case".into());
    }

    let job_id = Uuid::new_v4().to_string();
    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        jobs.insert(
            job_id.clone(),
            JobRec {
                status: TriageJobStatus {
                    job_id: job_id.clone(),
                    case_id: case_id.clone(),
                    doc_id: "*".into(),
                    state: TriageJobState::Running,
                    progress_percent: 0,
                    message: format!("queued {} document(s)", docs.len()),
                    page_count: None,
                    error: None,
                },
                started: Instant::now(),
            },
        );
    }

    let job_id_t = job_id.clone();
    let state_t = state.clone();
    let app_t = app.clone();
    let vault_t = vault.clone();
    let case_t = case_id.clone();
    let doc_metas: Vec<(String, String, String)> = docs
        .into_iter()
        .map(|d| (d.doc_id, d.original_filename, d.source_format))
        .collect();
    let n_docs = doc_metas.len() as u32;

    thread::spawn(move || {
        let emit = |pct: u32, msg: &str, doc_id: &str| {
            if let Ok(mut jobs) = state_t.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_id_t) {
                    rec.status.progress_percent = pct;
                    rec.status.message = msg.into();
                    rec.status.doc_id = doc_id.into();
                }
            }
            let _ = app_t.emit(
                "docpipe_triage_progress",
                TriageProgressEvent {
                    job_id: job_id_t.clone(),
                    case_id: case_t.clone(),
                    doc_id: doc_id.into(),
                    progress_percent: pct,
                    message: msg.into(),
                },
            );
        };

        let mut total_pages: u32 = 0;
        for (i, (doc_id, filename, source_format)) in doc_metas.iter().enumerate() {
            let base = (i as u32 * 100) / n_docs.max(1);
            let span = 100 / n_docs.max(1);
            emit(base, &format!("doc {}/{}: loading", i + 1, n_docs), doc_id);
            let bytes = match vault_t.doc_content_bytes(&case_t, doc_id) {
                Ok(b) => b,
                Err(e) => {
                    fail_job(&state_t, &job_id_t, &e.to_string());
                    emit(100, "failed", doc_id);
                    return;
                }
            };
            emit(
                base + span.saturating_mul(1) / 3,
                &format!("doc {}/{}: classifying", i + 1, n_docs),
                doc_id,
            );
            thread::sleep(Duration::from_millis(10));
            let pages = match classify_document_bytes(&bytes, source_format, filename) {
                Ok(value) => value,
                Err(error) => {
                    fail_job(&state_t, &job_id_t, &error);
                    emit(100, "failed", doc_id);
                    return;
                }
            };
            let page_count = pages.len() as u32;
            let source_rows = page_source_rows(doc_id, &pages, now_ms());
            if !source_rows.is_empty() {
                if let Err(error) = vault_t.page_source_replace(&case_t, doc_id, &source_rows) {
                    fail_job(&state_t, &job_id_t, &error.to_string());
                    emit(100, "failed", doc_id);
                    return;
                }
            }
            total_pages = total_pages.saturating_add(page_count);
            let classified_at = now_ms();
            let rows: Vec<PageTriageRow> = pages
                .into_iter()
                .map(|p| PageTriageRow {
                    doc_id: doc_id.clone(),
                    page_index: p.page_index,
                    triage_class: p.triage_class,
                    confidence: p.confidence,
                    classifier_id: p.classifier_id,
                    classified_at_unix_ms: classified_at,
                    notes: p.notes,
                })
                .collect();
            if let Err(e) = vault_t.page_triage_replace(&case_t, doc_id, &rows) {
                fail_job(&state_t, &job_id_t, &e.to_string());
                emit(100, "failed", doc_id);
                return;
            }
            let _ = vault_t.doc_set_page_count(&case_t, doc_id, page_count as i64);
            emit(
                base + span.saturating_sub(1),
                &format!("doc {}/{}: done ({page_count} page(s))", i + 1, n_docs),
                doc_id,
            );
        }

        if let Ok(mut jobs) = state_t.jobs.lock() {
            if let Some(rec) = jobs.get_mut(&job_id_t) {
                rec.status.state = TriageJobState::Completed;
                rec.status.progress_percent = 100;
                rec.status.message = format!("completed {n_docs} document(s)");
                rec.status.page_count = Some(total_pages);
                rec.status.doc_id = "*".into();
                rec.status.error = None;
            }
        }
        emit(100, "completed", "*");
    });

    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

fn fail_job(state: &DocpipeState, job_id: &str, err: &str) {
    if let Ok(mut jobs) = state.jobs.lock() {
        if let Some(rec) = jobs.get_mut(job_id) {
            rec.status.state = TriageJobState::Failed;
            rec.status.progress_percent = 100;
            rec.status.message = "failed".into();
            rec.status.error = Some(format!("CF_DOCPIPE_TRIAGE_FAILED: {err}"));
        }
    }
}

/// Which calibration map scored a recognizer's words: the Hindi route and the English route each have their own (B-S4-14).
fn calibration_id_for(recognizer_id: &str) -> Option<String> {
    if recognizer_id.contains("tesseract-hin") {
        Some("caseforge.tesseract-hin-eng.synthetic-isotonic.v1".into())
    } else if recognizer_id.contains("tesseract") {
        Some("caseforge.tesseract-eng-fast.synthetic-isotonic.v1".into())
    } else {
        None
    }
}

fn layer_to_rows(
    doc_id: &str,
    layers: Vec<recognize::PageTextLayerResult>,
    recognized_at: i64,
) -> Vec<PageTextLayerRow> {
    layers
        .into_iter()
        .map(|p| PageTextLayerRow {
            doc_id: doc_id.into(),
            page_index: p.page_index,
            recognizer_id: p.recognizer_id.clone(),
            recognized_at_unix_ms: recognized_at,
            full_text: p.full_text,
            mean_confidence: p.mean_confidence,
            page_quality_warn: p.page_quality_warn,
            notes: p.notes,
            calibration_id: calibration_id_for(&p.recognizer_id),
            critical_fields: p.critical_fields,
            words: p
                .words
                .into_iter()
                .map(|w| PageWordRow {
                    word_index: w.word_index,
                    text: w.text,
                    confidence: w.confidence,
                    raw_engine_confidence: w.raw_engine_confidence,
                    calibrated_confidence: w.calibrated_confidence,
                    confidence_state: w.confidence_state,
                    counsel_confirmed: false,
                    char_start: w.char_start,
                    char_end: w.char_end,
                    low_confidence: w.low_confidence,
                    force_review: w.force_review,
                    suppress_from_auto: w.suppress_from_auto,
                    bbox_json: w.bbox.map(|b| b.to_string()),
                })
                .collect(),
        })
        .collect()
}

fn trusted_layer_text(layer: &PageTextLayerRow) -> String {
    let mut chars: Vec<char> = layer.full_text.chars().collect();
    if let Some(fields) = layer.critical_fields.as_array() {
        for field in fields {
            if !field
                .get("suppress_from_auto")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
                || field
                    .get("counsel_confirmed")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            {
                continue;
            }
            let start = field
                .get("char_start")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let end = field
                .get("char_end")
                .and_then(|v| v.as_u64())
                .unwrap_or(start as u64) as usize;
            let char_len = chars.len();
            for value in chars
                .iter_mut()
                .take(end.min(char_len))
                .skip(start.min(char_len))
            {
                *value = ' ';
            }
        }
    }
    chars.into_iter().collect()
}

/// Start async recognition for one document (off UI thread).
pub fn start_recognize_job(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
    doc_id: String,
) -> Result<TriageStartResult, String> {
    let _bytes = vault
        .doc_content_bytes(&case_id, &doc_id)
        .map_err(|e| e.to_string())?;
    let meta = vault
        .doc_get(&case_id, &doc_id)
        .map_err(|e| e.to_string())?;

    let job_id = Uuid::new_v4().to_string();
    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        jobs.insert(
            job_id.clone(),
            JobRec {
                status: TriageJobStatus {
                    job_id: job_id.clone(),
                    case_id: case_id.clone(),
                    doc_id: doc_id.clone(),
                    state: TriageJobState::Running,
                    progress_percent: 0,
                    message: "recognize queued".into(),
                    page_count: None,
                    error: None,
                },
                started: Instant::now(),
            },
        );
    }

    let job_id_t = job_id.clone();
    let state_t = state.clone();
    let app_t = app.clone();
    let vault_t = vault.clone();
    let case_t = case_id.clone();
    let doc_t = doc_id.clone();
    let filename = meta.original_filename.clone();
    let source_format = meta.source_format.clone();

    thread::spawn(move || {
        let emit = |pct: u32, msg: &str| {
            if let Ok(mut jobs) = state_t.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_id_t) {
                    rec.status.progress_percent = pct;
                    rec.status.message = msg.into();
                }
            }
            let _ = app_t.emit(
                "docpipe_recognize_progress",
                TriageProgressEvent {
                    job_id: job_id_t.clone(),
                    case_id: case_t.clone(),
                    doc_id: doc_t.clone(),
                    progress_percent: pct,
                    message: msg.into(),
                },
            );
        };

        emit(5, "loading document");
        let bytes = match vault_t.doc_content_bytes(&case_t, &doc_t) {
            Ok(b) => b,
            Err(e) => {
                fail_job(&state_t, &job_id_t, &e.to_string());
                emit(100, "failed");
                return;
            }
        };
        emit(25, "recognizing pages");
        thread::sleep(Duration::from_millis(20));
        let layers = match recognize_document_bytes(&bytes, &source_format, &filename) {
            Ok(value) => value,
            Err(error) => {
                fail_job(&state_t, &job_id_t, &error);
                emit(100, "failed");
                return;
            }
        };
        let page_count = layers.len() as u32;
        emit(70, &format!("writing {page_count} text layer(s)"));
        let rows = layer_to_rows(&doc_t, layers, now_ms());
        if let Err(e) = vault_t.page_text_layer_replace(&case_t, &doc_t, &rows) {
            fail_job(&state_t, &job_id_t, &e.to_string());
            emit(100, "failed");
            return;
        }
        let _ = vault_t.doc_set_page_count(&case_t, &doc_t, page_count as i64);
        if let Ok(mut jobs) = state_t.jobs.lock() {
            if let Some(rec) = jobs.get_mut(&job_id_t) {
                rec.status.state = TriageJobState::Completed;
                rec.status.progress_percent = 100;
                rec.status.message = "recognition completed".into();
                rec.status.page_count = Some(page_count);
                rec.status.error = None;
            }
        }
        emit(100, "completed");
    });

    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

/// Start async recognition for every document in an open case.
pub fn start_recognize_case_job(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
) -> Result<TriageStartResult, String> {
    let docs = vault.doc_list(&case_id).map_err(|e| e.to_string())?;
    if docs.is_empty() {
        return Err("CF_DOCPIPE_RECOGNIZE_FAILED: no documents in case".into());
    }

    let job_id = Uuid::new_v4().to_string();
    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        jobs.insert(
            job_id.clone(),
            JobRec {
                status: TriageJobStatus {
                    job_id: job_id.clone(),
                    case_id: case_id.clone(),
                    doc_id: "*".into(),
                    state: TriageJobState::Running,
                    progress_percent: 0,
                    message: format!("recognize queued {} document(s)", docs.len()),
                    page_count: None,
                    error: None,
                },
                started: Instant::now(),
            },
        );
    }

    let job_id_t = job_id.clone();
    let state_t = state.clone();
    let app_t = app.clone();
    let vault_t = vault.clone();
    let case_t = case_id.clone();
    let doc_metas: Vec<(String, String, String)> = docs
        .into_iter()
        .map(|d| (d.doc_id, d.original_filename, d.source_format))
        .collect();
    let n_docs = doc_metas.len() as u32;

    thread::spawn(move || {
        let emit = |pct: u32, msg: &str, doc_id: &str| {
            if let Ok(mut jobs) = state_t.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_id_t) {
                    rec.status.progress_percent = pct;
                    rec.status.message = msg.into();
                    rec.status.doc_id = doc_id.into();
                }
            }
            let _ = app_t.emit(
                "docpipe_recognize_progress",
                TriageProgressEvent {
                    job_id: job_id_t.clone(),
                    case_id: case_t.clone(),
                    doc_id: doc_id.into(),
                    progress_percent: pct,
                    message: msg.into(),
                },
            );
        };

        let mut total_pages: u32 = 0;
        for (i, (doc_id, filename, source_format)) in doc_metas.iter().enumerate() {
            let base = (i as u32 * 100) / n_docs.max(1);
            let span = 100 / n_docs.max(1);
            emit(
                base,
                &format!("doc {}/{}: recognizing", i + 1, n_docs),
                doc_id,
            );
            let bytes = match vault_t.doc_content_bytes(&case_t, doc_id) {
                Ok(b) => b,
                Err(e) => {
                    fail_job(&state_t, &job_id_t, &e.to_string());
                    emit(100, "failed", doc_id);
                    return;
                }
            };
            thread::sleep(Duration::from_millis(10));
            let layers = match recognize_document_bytes(&bytes, source_format, filename) {
                Ok(value) => value,
                Err(error) => {
                    fail_job(&state_t, &job_id_t, &error);
                    emit(100, "failed", doc_id);
                    return;
                }
            };
            let page_count = layers.len() as u32;
            total_pages = total_pages.saturating_add(page_count);
            let rows = layer_to_rows(doc_id, layers, now_ms());
            if let Err(e) = vault_t.page_text_layer_replace(&case_t, doc_id, &rows) {
                fail_job(&state_t, &job_id_t, &e.to_string());
                emit(100, "failed", doc_id);
                return;
            }
            let _ = vault_t.doc_set_page_count(&case_t, doc_id, page_count as i64);
            emit(
                base + span.saturating_sub(1),
                &format!("doc {}/{}: done ({page_count} page(s))", i + 1, n_docs),
                doc_id,
            );
        }

        if let Ok(mut jobs) = state_t.jobs.lock() {
            if let Some(rec) = jobs.get_mut(&job_id_t) {
                rec.status.state = TriageJobState::Completed;
                rec.status.progress_percent = 100;
                rec.status.message = format!("recognition completed {n_docs} document(s)");
                rec.status.page_count = Some(total_pages);
                rec.status.doc_id = "*".into();
                rec.status.error = None;
            }
        }
        emit(100, "completed", "*");
    });

    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

/// Static metadata for the shell UI / health endpoints (serialize-only).
#[derive(Debug, Clone, Serialize)]
pub struct DocpipeMeta {
    pub batch: &'static str,
    pub pdf_parser_id: &'static str,
    pub classifier_id: &'static str,
    pub recognizer_id: &'static str,
    pub triage_classes: &'static [&'static str],
    pub provisional: bool,
    pub methods: &'static [&'static str],
    pub out_of_scope: &'static [&'static str],
}

/// Start async skeleton extraction for an open case (uses text layers when present).
pub fn start_skeleton_job(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
) -> Result<TriageStartResult, String> {
    let docs = vault.doc_list(&case_id).map_err(|e| e.to_string())?;
    if docs.is_empty() {
        return Err("CF_DOCPIPE_SKELETON_FAILED: no documents in case".into());
    }
    let cases = vault.list_cases().map_err(|e| e.to_string())?;
    let profile = cases
        .into_iter()
        .find(|c| c.case_id == case_id)
        .ok_or_else(|| "CF_DOCPIPE_SKELETON_FAILED: case not found".to_string())?;

    let job_id = Uuid::new_v4().to_string();
    {
        let mut jobs = state.jobs.lock().map_err(|e| e.to_string())?;
        jobs.insert(
            job_id.clone(),
            JobRec {
                status: TriageJobStatus {
                    job_id: job_id.clone(),
                    case_id: case_id.clone(),
                    doc_id: "*".into(),
                    state: TriageJobState::Running,
                    progress_percent: 0,
                    message: "skeleton queued".into(),
                    page_count: None,
                    error: None,
                },
                started: Instant::now(),
            },
        );
    }

    let job_id_t = job_id.clone();
    let state_t = state.clone();
    let app_t = app.clone();
    let vault_t = vault.clone();
    let case_t = case_id.clone();
    let jurisdiction = profile.jurisdiction.clone();

    thread::spawn(move || {
        let emit = |pct: u32, msg: &str| {
            if let Ok(mut jobs) = state_t.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_id_t) {
                    rec.status.progress_percent = pct;
                    rec.status.message = msg.into();
                }
            }
            let _ = app_t.emit(
                "docpipe_skeleton_progress",
                TriageProgressEvent {
                    job_id: job_id_t.clone(),
                    case_id: case_t.clone(),
                    doc_id: "*".into(),
                    progress_percent: pct,
                    message: msg.into(),
                },
            );
        };

        emit(10, "loading text layers");
        let layers = match vault_t.page_text_layer_list(&case_t, None) {
            Ok(l) => l,
            Err(e) => {
                fail_job(&state_t, &job_id_t, &e.to_string());
                emit(100, "failed");
                return;
            }
        };

        let mut pages: Vec<skeleton::PageInput> = Vec::new();
        if layers.is_empty() {
            // Fall back: raw document bytes → crude text split
            emit(25, "no text layers; falling back to document text");
            let docs = match vault_t.doc_list(&case_t) {
                Ok(d) => d,
                Err(e) => {
                    fail_job(&state_t, &job_id_t, &e.to_string());
                    emit(100, "failed");
                    return;
                }
            };
            for d in docs {
                let bytes = match vault_t.doc_content_bytes(&case_t, &d.doc_id) {
                    Ok(b) => b,
                    Err(_) => continue,
                };
                let texts = match triage::document_page_texts(
                    &bytes,
                    &d.source_format,
                    &d.original_filename,
                ) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                for (idx, text, _) in texts {
                    pages.push(skeleton::PageInput {
                        doc_id: d.doc_id.clone(),
                        page_index: idx,
                        text,
                        mean_confidence: 0.7,
                    });
                }
            }
        } else {
            for layer in layers {
                // B-S3-05: never present warned/force-review OCR as understood.
                // A human correction produces a reviewed layer before extraction resumes.
                if layer.page_quality_warn || layer.words.iter().any(|word| word.force_review) {
                    continue;
                }
                let trusted_text = trusted_layer_text(&layer);
                if trusted_text.trim().is_empty() {
                    continue;
                }
                pages.push(skeleton::PageInput {
                    doc_id: layer.doc_id,
                    page_index: layer.page_index,
                    text: trusted_text,
                    mean_confidence: layer.mean_confidence,
                });
            }
        }

        emit(
            55,
            &format!("extracting skeleton from {} page(s)", pages.len()),
        );
        thread::sleep(Duration::from_millis(20));
        // B-S4-06 leak barrier: an opponent's filing is adverse assertion, never matter fact.
        let external = match external_doc_ids(&vault_t, &case_t) {
            Ok(ids) => ids,
            Err(e) => {
                fail_job(&state_t, &job_id_t, &e);
                emit(100, "failed");
                return;
            }
        };
        let pages = exclude_external_pages(pages, &external);
        let graph =
            extract_case_graph(&case_t, &jurisdiction, &pages, CLASSIFIER_ID, RECOGNIZER_ID);
        let defects = assert_extracted_sources(&graph);
        if !defects.is_empty() {
            fail_job(
                &state_t,
                &job_id_t,
                &format!("source pointer defects: {}", defects.join("; ")),
            );
            emit(100, "failed");
            return;
        }

        emit(80, "writing case_graph");
        if let Err(e) = vault_t.case_graph_put_generated(&case_t, &graph) {
            fail_job(&state_t, &job_id_t, &e.to_string());
            emit(100, "failed");
            return;
        }

        let event_count = graph["chronology"].as_array().map(|a| a.len()).unwrap_or(0) as u32;
        if let Ok(mut jobs) = state_t.jobs.lock() {
            if let Some(rec) = jobs.get_mut(&job_id_t) {
                rec.status.state = TriageJobState::Completed;
                rec.status.progress_percent = 100;
                rec.status.message =
                    format!("skeleton completed ({event_count} chronology events)");
                rec.status.page_count = Some(pages.len() as u32);
                rec.status.error = None;
            }
        }
        emit(100, "completed");
    });

    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

// --- B-S1-10 overnight batch + resume ---

fn run_unit(
    vault: &VaultManager,
    case_id: &str,
    unit_kind: &str,
    unit_id: &str,
) -> Result<String, String> {
    match unit_kind {
        "doc_triage" => {
            let meta = vault.doc_get(case_id, unit_id).map_err(|e| e.to_string())?;
            let bytes = vault
                .doc_content_bytes(case_id, unit_id)
                .map_err(|e| e.to_string())?;
            let pages =
                classify_document_bytes(&bytes, &meta.source_format, &meta.original_filename)?;
            let source_rows = page_source_rows(unit_id, &pages, now_ms());
            if !source_rows.is_empty() {
                vault
                    .page_source_replace(case_id, unit_id, &source_rows)
                    .map_err(|e| e.to_string())?;
            }
            let classified_at = now_ms();
            let rows: Vec<PageTriageRow> = pages
                .into_iter()
                .map(|p| PageTriageRow {
                    doc_id: unit_id.into(),
                    page_index: p.page_index,
                    triage_class: p.triage_class,
                    confidence: p.confidence,
                    classifier_id: p.classifier_id,
                    classified_at_unix_ms: classified_at,
                    notes: p.notes,
                })
                .collect();
            let n = rows.len() as i64;
            vault
                .page_triage_replace(case_id, unit_id, &rows)
                .map_err(|e| e.to_string())?;
            let _ = vault.doc_set_page_count(case_id, unit_id, n);
            Ok(format!("triaged {n} page(s)"))
        }
        "doc_recognize" => {
            let meta = vault.doc_get(case_id, unit_id).map_err(|e| e.to_string())?;
            let bytes = vault
                .doc_content_bytes(case_id, unit_id)
                .map_err(|e| e.to_string())?;
            let layers =
                recognize_document_bytes(&bytes, &meta.source_format, &meta.original_filename)?;
            let recognized_at = now_ms();
            let rows: Vec<PageTextLayerRow> = layers
                .into_iter()
                .map(|p| PageTextLayerRow {
                    doc_id: unit_id.into(),
                    page_index: p.page_index,
                    recognizer_id: p.recognizer_id.clone(),
                    recognized_at_unix_ms: recognized_at,
                    full_text: p.full_text,
                    mean_confidence: p.mean_confidence,
                    page_quality_warn: p.page_quality_warn,
                    notes: p.notes,
                    calibration_id: calibration_id_for(&p.recognizer_id),
                    critical_fields: p.critical_fields,
                    words: p
                        .words
                        .into_iter()
                        .map(|w| PageWordRow {
                            word_index: w.word_index,
                            text: w.text,
                            confidence: w.confidence,
                            raw_engine_confidence: w.raw_engine_confidence,
                            calibrated_confidence: w.calibrated_confidence,
                            confidence_state: w.confidence_state,
                            counsel_confirmed: false,
                            char_start: w.char_start,
                            char_end: w.char_end,
                            low_confidence: w.low_confidence,
                            force_review: w.force_review,
                            suppress_from_auto: w.suppress_from_auto,
                            bbox_json: w.bbox.map(|b| b.to_string()),
                        })
                        .collect(),
                })
                .collect();
            let n = rows.len() as i64;
            vault
                .page_text_layer_replace(case_id, unit_id, &rows)
                .map_err(|e| e.to_string())?;
            let _ = vault.doc_set_page_count(case_id, unit_id, n);
            Ok(format!("recognized {n} page layer(s)"))
        }
        "case_skeleton" => {
            let cases = vault.list_cases().map_err(|e| e.to_string())?;
            let profile = cases
                .into_iter()
                .find(|c| c.case_id == case_id)
                .ok_or_else(|| "case not found".to_string())?;
            let layers = vault
                .page_text_layer_list(case_id, None)
                .map_err(|e| e.to_string())?;
            let mut pages: Vec<skeleton::PageInput> = Vec::new();
            if layers.is_empty() {
                let docs = vault.doc_list(case_id).map_err(|e| e.to_string())?;
                for d in docs {
                    let bytes = match vault.doc_content_bytes(case_id, &d.doc_id) {
                        Ok(b) => b,
                        Err(_) => continue,
                    };
                    let texts = match triage::document_page_texts(
                        &bytes,
                        &d.source_format,
                        &d.original_filename,
                    ) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    for (idx, text, _) in texts {
                        pages.push(skeleton::PageInput {
                            doc_id: d.doc_id.clone(),
                            page_index: idx,
                            text,
                            mean_confidence: 0.7,
                        });
                    }
                }
            } else {
                for layer in layers {
                    if layer.page_quality_warn || layer.words.iter().any(|word| word.force_review) {
                        continue;
                    }
                    let trusted_text = trusted_layer_text(&layer);
                    if trusted_text.trim().is_empty() {
                        continue;
                    }
                    pages.push(skeleton::PageInput {
                        doc_id: layer.doc_id,
                        page_index: layer.page_index,
                        text: trusted_text,
                        mean_confidence: layer.mean_confidence,
                    });
                }
            }
            // B-S4-06 leak barrier: an opponent's filing is adverse assertion, never matter fact.
            let pages = exclude_external_pages(pages, &external_doc_ids(vault, case_id)?);
            let graph = extract_case_graph(
                case_id,
                &profile.jurisdiction,
                &pages,
                CLASSIFIER_ID,
                RECOGNIZER_ID,
            );
            let defects = assert_extracted_sources(&graph);
            if !defects.is_empty() {
                return Err(format!("source defects: {}", defects.join("; ")));
            }
            vault
                .case_graph_put_generated(case_id, &graph)
                .map_err(|e| e.to_string())?;
            let n = graph["chronology"].as_array().map(|a| a.len()).unwrap_or(0);
            Ok(format!("skeleton events={n}"))
        }
        other => Err(format!("unknown unit_kind {other}")),
    }
}

/// Start overnight batch (creates durable job + worker). Optional stop_after_units for AC-6.8 tests.
pub fn start_overnight_batch(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
    stop_after_units: Option<u32>,
) -> Result<TriageStartResult, String> {
    let job = vault.batch_create(&case_id).map_err(|e| e.to_string())?;
    let job_id = job
        .get("job_id")
        .and_then(|v| v.as_str())
        .ok_or("batch_create missing job_id")?
        .to_string();
    state.clear_batch_cancel(&job_id);
    spawn_batch_worker(app, vault, state, case_id, job_id.clone(), stop_after_units);
    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

/// Resume interrupted/failed batch from next pending checkpoint (AC-6.8).
pub fn resume_overnight_batch(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
    job_id: String,
) -> Result<TriageStartResult, String> {
    let job = vault
        .batch_get(&case_id, &job_id)
        .map_err(|e| e.to_string())?;
    let st = job.get("status").and_then(|s| s.as_str()).unwrap_or("");
    if st == "completed" {
        return Err("CF_BATCH_ALREADY_DONE: job already completed".into());
    }
    if st == "running" {
        return Err("CF_BATCH_ALREADY_RUNNING: wait or cancel first".into());
    }
    state.clear_batch_cancel(&job_id);
    vault
        .batch_set_status(
            &case_id,
            &job_id,
            "running",
            None,
            Some("resuming from checkpoint"),
            None,
        )
        .map_err(|e| e.to_string())?;
    spawn_batch_worker(app, vault, state, case_id, job_id.clone(), None);
    Ok(TriageStartResult {
        job_id,
        accepted: true,
    })
}

pub fn cancel_overnight_batch(
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
    job_id: String,
) -> Result<serde_json::Value, String> {
    state.request_batch_cancel(&job_id);
    vault
        .batch_set_status(
            &case_id,
            &job_id,
            "interrupted",
            None,
            Some("cancel requested (AC-6.8 kill path)"),
            None,
        )
        .map_err(|e| e.to_string())?;
    vault
        .batch_get(&case_id, &job_id)
        .map_err(|e| e.to_string())
}

fn spawn_batch_worker(
    app: AppHandle,
    vault: Arc<VaultManager>,
    state: Arc<DocpipeState>,
    case_id: String,
    job_id: String,
    stop_after_units: Option<u32>,
) {
    let app_t = app.clone();
    let vault_t = vault.clone();
    let state_t = state.clone();
    let case_t = case_id.clone();
    let job_t = job_id.clone();

    // Track in in-memory job map for UI polling compatibility
    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.insert(
            job_id.clone(),
            JobRec {
                status: TriageJobStatus {
                    job_id: job_id.clone(),
                    case_id: case_id.clone(),
                    doc_id: "*".into(),
                    state: TriageJobState::Running,
                    progress_percent: 0,
                    message: "overnight batch running".into(),
                    page_count: None,
                    error: None,
                },
                started: Instant::now(),
            },
        );
    }
    let _ = vault.batch_set_status(
        &case_id,
        &job_id,
        "running",
        Some("triage"),
        Some("worker started"),
        None,
    );

    thread::spawn(move || {
        let emit = |pct: u32, msg: &str| {
            if let Ok(mut jobs) = state_t.jobs.lock() {
                if let Some(rec) = jobs.get_mut(&job_t) {
                    rec.status.progress_percent = pct;
                    rec.status.message = msg.into();
                }
            }
            let _ = app_t.emit(
                "docpipe_batch_progress",
                TriageProgressEvent {
                    job_id: job_t.clone(),
                    case_id: case_t.clone(),
                    doc_id: "*".into(),
                    progress_percent: pct,
                    message: msg.into(),
                },
            );
        };

        let mut units_this_run: u32 = 0;
        loop {
            if state_t.is_batch_cancelled(&job_t) {
                let _ = vault_t.batch_set_status(
                    &case_t,
                    &job_t,
                    "interrupted",
                    None,
                    Some("interrupted by cancel/kill"),
                    None,
                );
                if let Ok(mut jobs) = state_t.jobs.lock() {
                    if let Some(rec) = jobs.get_mut(&job_t) {
                        rec.status.state = TriageJobState::Failed;
                        rec.status.message = "interrupted".into();
                        rec.status.error = Some("CF_BATCH_INTERRUPTED".into());
                    }
                }
                emit(0, "interrupted");
                return;
            }

            let next = match vault_t.batch_next_pending(&case_t, &job_t) {
                Ok(n) => n,
                Err(e) => {
                    let _ = vault_t.batch_set_status(
                        &case_t,
                        &job_t,
                        "failed",
                        None,
                        Some("next_pending failed"),
                        Some(&e.to_string()),
                    );
                    fail_job(&state_t, &job_t, &e.to_string());
                    emit(100, "failed");
                    return;
                }
            };

            let Some((unit_kind, unit_id)) = next else {
                let _ = vault_t.batch_mark_completed(&case_t, &job_t);
                if let Ok(mut jobs) = state_t.jobs.lock() {
                    if let Some(rec) = jobs.get_mut(&job_t) {
                        rec.status.state = TriageJobState::Completed;
                        rec.status.progress_percent = 100;
                        rec.status.message = "overnight batch completed".into();
                        rec.status.error = None;
                    }
                }
                emit(100, "completed");
                return;
            };

            emit(0, &format!("running {unit_kind}/{unit_id}"));
            // Yield so progress IPC is observable and kill can land between units
            thread::sleep(Duration::from_millis(15));

            match run_unit(&vault_t, &case_t, &unit_kind, &unit_id) {
                Ok(detail) => {
                    match vault_t.batch_checkpoint_done(
                        &case_t,
                        &job_t,
                        &unit_kind,
                        &unit_id,
                        Some(&detail),
                    ) {
                        Ok(job) => {
                            let pct = job
                                .get("progress_percent")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            emit(pct, &detail);
                        }
                        Err(e) => {
                            let _ = vault_t.batch_set_status(
                                &case_t,
                                &job_t,
                                "failed",
                                None,
                                Some("checkpoint failed"),
                                Some(&e.to_string()),
                            );
                            fail_job(&state_t, &job_t, &e.to_string());
                            emit(100, "failed");
                            return;
                        }
                    }
                }
                Err(e) => {
                    let _ = vault_t.batch_set_status(
                        &case_t,
                        &job_t,
                        "failed",
                        None,
                        Some(&format!("unit failed {unit_kind}/{unit_id}")),
                        Some(&e),
                    );
                    fail_job(&state_t, &job_t, &e);
                    emit(100, "failed");
                    return;
                }
            }

            units_this_run = units_this_run.saturating_add(1);
            // Test hook: stop after N units to simulate kill mid-batch (AC-6.8)
            if let Some(limit) = stop_after_units {
                if units_this_run >= limit {
                    let _ = vault_t.batch_set_status(
                        &case_t,
                        &job_t,
                        "interrupted",
                        None,
                        Some(&format!(
                            "simulated kill after {units_this_run} unit(s) (AC-6.8)"
                        )),
                        None,
                    );
                    if let Ok(mut jobs) = state_t.jobs.lock() {
                        if let Some(rec) = jobs.get_mut(&job_t) {
                            rec.status.state = TriageJobState::Failed;
                            rec.status.message = "interrupted (simulated kill)".into();
                            rec.status.error = Some("CF_BATCH_INTERRUPTED".into());
                        }
                    }
                    emit(0, "interrupted");
                    return;
                }
            }
        }
    });
}

pub fn docpipe_meta() -> DocpipeMeta {
    DocpipeMeta {
        batch: "B-S3-07",
        pdf_parser_id: pdf_adapter::PDF_PARSER_ID,
        classifier_id: CLASSIFIER_ID,
        recognizer_id: RECOGNIZER_ID,
        triage_classes: TRIAGE_CLASSES,
        provisional: true,
        methods: &[
            "docpipe.ping",
            "docpipe.version",
            "docpipe.triage_pages",
            "docpipe.recognize_pages",
            "docpipe.extract_skeleton",
            "docpipe_triage_start",
            "docpipe_triage_case",
            "docpipe_triage_status",
            "docpipe_page_list",
            "docpipe_page_source_list",
            "docpipe_recognize_start",
            "docpipe_recognize_case",
            "docpipe_text_layer_list",
            "docpipe_skeleton_start",
            "docpipe_case_graph_get",
            "batch_start",
            "batch_resume",
            "batch_cancel",
            "batch_status",
            "batch_latest",
        ],
        out_of_scope: &[
            "vision_ocr_word_boxes",
            "vision_llm_load",
            "cloud_queue",
            "graph_model_inference",
        ],
    }
}


/// Document type tag for an opponent's filing (B-S4-06). Its text is adverse assertion, never matter fact.
pub const EXTERNAL_FILING_TAG: &str = "EXTERNAL_FILING";

/// Doc ids tagged as external filings in this case. Fails closed: if the document list cannot be read, the graph
/// build stops rather than risk an opponent's filing being extracted as matter fact.
pub(crate) fn external_doc_ids(vault: &crate::vault::VaultManager, case_id: &str) -> Result<HashSet<String>, String> {
    vault
        .doc_list(case_id)
        .map(|docs| docs.into_iter().filter(|d| d.type_tag == EXTERNAL_FILING_TAG).map(|d| d.doc_id).collect())
        .map_err(|e| format!("CF_AUDIT_EXTERNAL_ASSERTION_LEAK: cannot confirm which documents are external filings ({e}); case graph not rebuilt"))
}

/// B-S4-06 leak barrier (AC-S4-02.3): pages of external filings never reach case-graph extraction, so an opponent's
/// assertions can never become Digest facts, Drafting sources or Mock Court factual bases.
pub(crate) fn exclude_external_pages(pages: Vec<skeleton::PageInput>, external: &HashSet<String>) -> Vec<skeleton::PageInput> {
    pages.into_iter().filter(|p| !external.contains(&p.doc_id)).collect()
}

/// B-S4-06: the page texts of ONE document tagged EXTERNAL_FILING, for the Opposing Filing Scan only.
/// Refuses any other document so a matter document can never be scanned as "the opponent's assertion".
pub(crate) fn external_filing_pages(
    vault: &crate::vault::VaultManager,
    case_id: &str,
    doc_id: &str,
) -> Result<(String, Vec<String>), String> {
    let meta = vault
        .doc_list(case_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|d| d.doc_id == doc_id)
        .ok_or_else(|| format!("CF_DOC_NOT_FOUND: no document {doc_id} in this case"))?;
    if meta.type_tag != EXTERNAL_FILING_TAG {
        return Err(format!("CF_AUDIT_EXTRACTION_FAILED: {} is not tagged EXTERNAL_FILING", meta.original_filename));
    }
    let bytes = vault.doc_content_bytes(case_id, doc_id).map_err(|e| e.to_string())?;
    let pages = triage::document_page_texts(&bytes, &meta.source_format, &meta.original_filename)?
        .into_iter()
        .map(|(_, text, _)| text)
        .collect();
    Ok((meta.original_filename, pages))
}

/// B-S4-20: the page texts of a file given as bytes (a cause list in PDF form), without storing it in a matter.
pub(crate) fn bytes_page_texts(bytes: &[u8], source_format: &str, filename: &str) -> Result<Vec<String>, String> {
    Ok(triage::document_page_texts(bytes, source_format, filename)?.into_iter().map(|(_, text, _)| text).collect())
}

/// B-S4-07: page texts of a witness statement. Uses the document's own text; for a scanned document, the reviewed
/// (trusted) OCR layer. A text file with form feeds is split into its pages (deposition transcripts).
pub(crate) fn statement_pages(
    vault: &VaultManager,
    case_id: &str,
    doc_id: &str,
) -> Result<(crate::vault::DocumentMeta, Vec<String>), String> {
    let meta = vault.doc_get(case_id, doc_id).map_err(|e| e.to_string())?;
    let bytes = vault.doc_content_bytes(case_id, doc_id).map_err(|e| e.to_string())?;
    let mut pages: Vec<String> = triage::document_page_texts(&bytes, &meta.source_format, &meta.original_filename)?
        .into_iter()
        .map(|(_, text, _)| text)
        .collect();
    if pages.iter().all(|p| p.trim().is_empty()) {
        let mut layers = vault.page_text_layer_list(case_id, Some(doc_id)).map_err(|e| e.to_string())?;
        layers.sort_by_key(|l| l.page_index);
        pages = layers
            .iter()
            .filter(|l| !l.page_quality_warn && !l.words.iter().any(|w| w.force_review))
            .map(trusted_layer_text)
            .collect();
    }
    if pages.len() == 1 && pages[0].contains('\u{c}') {
        pages = pages[0].split('\u{c}').map(str::to_string).collect();
    }
    Ok((meta, pages))
}

#[cfg(test)]
mod external_filing_barrier_tests {
    use super::*;

    #[test]
    fn external_filing_pages_never_reach_graph_extraction() {
        let page = |doc: &str| skeleton::PageInput { doc_id: doc.into(), page_index: 0, text: "The deed was executed on 14 April 2019.".into(), mean_confidence: 0.9 };
        let external: HashSet<String> = ["doc-reply".to_string()].into_iter().collect();
        let kept = exclude_external_pages(vec![page("doc-deed"), page("doc-reply"), page("doc-decree")], &external);
        assert_eq!(kept.iter().map(|p| p.doc_id.as_str()).collect::<Vec<_>>(), vec!["doc-deed", "doc-decree"]);
        let graph = extract_case_graph("case-x", "India", &kept, CLASSIFIER_ID, RECOGNIZER_ID);
        assert!(!graph.to_string().contains("doc-reply"), "an external filing leaked into the case graph");
    }
}

#[cfg(test)]
mod indic_calibration_tests {
    use super::*;

    #[test]
    fn each_route_is_scored_by_its_own_calibration_map() {
        assert_eq!(calibration_id_for("caseforge.ocr.tesseract-eng-fast.v1").as_deref(), Some("caseforge.tesseract-eng-fast.synthetic-isotonic.v1"));
        assert_eq!(calibration_id_for("caseforge.ocr.tesseract-hin-eng-best.v1").as_deref(), Some("caseforge.tesseract-hin-eng.synthetic-isotonic.v1"));
        assert_eq!(calibration_id_for("caseforge.pdfium.native-text.v1"), None, "native text has no OCR calibration");
    }
}
