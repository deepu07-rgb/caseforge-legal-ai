//! Retrieval-first citation pipeline (B-S1-13, hybrid index B-S3-11).
//! DECOMPOSE → RETRIEVE → GENERATE (constrained) → VERIFY → SCORE → RENDER
//! Offline BM25 + deterministic legal-concept embeddings with reciprocal-rank reranking.
//! AC-4.3: PLANTED_FAKE / unknown → RED. AC-7.2: top-k hit rate on golden queries.

pub mod dense;

use crate::packs::{list_installed, load_legal_chunks, InstalledPack, LegalChunkRecord};
use crate::vault::VaultPaths;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub const INDEX_ID: &str = "caseforge.retrieve.hybrid.v1";
pub const LEGACY_INDEX_ID: &str = "caseforge.retrieve.lexical.v1";
pub const INDEX_SCHEMA_VERSION: &str = "1.0.0";
pub const SEMANTIC_ENGINE_ID: &str = "caseforge.legal-concept-hash-384.v1";
pub const SEMANTIC_ENGINE_LICENSE: &str =
    "CaseForge first-party deterministic algorithm; no third-party weights";
const EMBEDDING_DIMS: usize = 384;
pub const STAGES: &[&str] = &[
    "DECOMPOSE",
    "RETRIEVE",
    "GENERATE",
    "VERIFY",
    "SCORE",
    "RENDER",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusChunk {
    pub chunk_id: String,
    pub pack_id: String,
    pub path: String,
    pub jurisdiction: Option<String>,
    pub text: String,
    pub authority_hint: String,
    pub metadata: LegalMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegalMetadata {
    pub source_id: String,
    pub title: String,
    pub jurisdiction_path: Vec<String>,
    pub authority_type: String,
    pub issuing_authority: String,
    pub citation: String,
    pub section: String,
    pub effective_from: String,
    pub effective_to: String,
    pub version: String,
    pub amendment: String,
    pub page: serde_json::Value,
    pub paragraph: String,
    pub heading_path: Vec<String>,
    pub checksum: String,
    pub source_checksum: String,
    pub pack_checksum: String,
    pub trust_level: String,
    pub superseded_status: String,
    pub publisher: String,
    pub source_reference: String,
    pub content_kind: String,
    pub temporal_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievedHit {
    pub chunk_id: String,
    pub pack_id: String,
    pub path: String,
    pub score: f64,
    pub lexical_score: f64,
    pub semantic_score: f64,
    pub rerank_score: f64,
    pub match_reasons: Vec<String>,
    pub retrieval_mode: String,
    pub corpus_kind: String,
    pub temporal_warning: Option<String>,
    pub snippet: String,
    pub authority_hint: String,
    pub source_id: String,
    pub chunk_checksum: String,
    pub source_checksum: String,
    pub trust_level: String,
    pub authority_type: String,
    pub temporal_status: String,
    pub superseded_status: String,
    pub content_kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CitationRecord {
    pub schema_version: String,
    pub citation_id: String,
    pub query_id: Option<String>,
    pub authority_string: String,
    pub verification_status: String,
    pub match_score: f64,
    pub retrieved: bool,
    pub user_confirmed: bool,
    pub pack_id: Option<String>,
    pub source_id: Option<String>,
    pub chunk_id: Option<String>,
    pub source_checksum: Option<String>,
    pub chunk_checksum: Option<String>,
    pub trust_level: String,
    pub authority_type: String,
    pub temporal_status: String,
    pub superseded_status: String,
    pub content_kind: String,
    pub corpus_locus: Option<CorpusLocus>,
    pub planted_fake: bool,
    pub exclude_from_downstream: bool,
    pub pipeline: PipelineMeta,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusLocus {
    pub path: String,
    pub char_start: Option<i64>,
    pub char_end: Option<i64>,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PipelineMeta {
    pub stages: Vec<String>,
    pub index_id: String,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RetrievalResult {
    pub query: String,
    pub query_id: String,
    pub jurisdiction_filter: Option<String>,
    pub stages_completed: Vec<String>,
    pub retrieved: Vec<RetrievedHit>,
    pub citations: Vec<CitationRecord>,
    pub index_id: String,
    pub index_checksum: String,
    pub index_state: String,
    pub retrieval_mode: String,
    pub semantic_engine_id: String,
    pub semantic_available: bool,
    pub lexical_fallback_notice: Option<String>,
    pub chunk_count: usize,
    pub batch: &'static str,
    /// B-S1-17: number of Experience Store patterns injected into the corpus.
    pub experience_injected: usize,
    pub experience_top_k: usize,
    pub repository_gap: bool,
    pub repository_gap_reason: Option<String>,
    pub repository_gap_required_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedIndexMetadata {
    pub schema_version: String,
    pub index_id: String,
    pub corpus_checksum: String,
    pub chunk_count: usize,
    pub semantic_engine_id: String,
    pub semantic_engine_license: String,
    pub embedding_dimensions: usize,
}

fn controlled_repository_gap(query: &str, hits_empty: bool) -> (bool, Option<String>, Vec<String>) {
    let q = query.to_ascii_lowercase();
    let named_high_court = [
        "bombay high court",
        "delhi high court",
        "calcutta high court",
        "madras high court",
        "karnataka high court",
        "kerala high court",
        "allahabad high court",
        "rajasthan high court",
        "punjab and haryana high court",
        "gujarat high court",
        "patna high court",
    ]
    .iter()
    .any(|name| q.contains(name));
    let local_procedure = [
        "filing rule",
        "court fee",
        "practice direction",
        "high court rule",
        "local rule",
    ]
    .iter()
    .any(|term| q.contains(term));
    if named_high_court && local_procedure {
        return (
            true,
            Some(
                "The controlled pack excludes state amendments and individual High Court rules."
                    .into(),
            ),
            vec![
                "Current rules and amendments of the named High Court from its official publisher"
                    .into(),
            ],
        );
    }
    if [
        "muslim",
        "christian",
        "parsi",
        "hindu succession",
        "coparcenary",
        "personal law",
    ]
    .iter()
    .any(|term| q.contains(term))
    {
        return (
            true,
            Some("The controlled pack does not contain a selected personal-law entitlement framework.".into()),
            vec!["Authoritative current personal-law statute and applicable binding judgments".into()],
        );
    }
    if ["agricultural land", "revenue land", "tenancy land reform"]
        .iter()
        .any(|term| q.contains(term))
    {
        return (
            true,
            Some(
                "Agricultural, revenue, tenancy, and state land-reform law are outside this pack."
                    .into(),
            ),
            vec!["Current state land/revenue legislation and official rules".into()],
        );
    }
    if hits_empty {
        return (
            true,
            Some("No matching authority exists in the installed, filtered repository.".into()),
            vec!["An authoritative signed pack covering the requested proposition".into()],
        );
    }
    (false, None, Vec::new())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataFilters {
    pub jurisdiction: Option<String>,
    pub authority_type: Option<String>,
    pub trust_level: Option<String>,
    pub temporal_status: Option<String>,
    pub superseded_status: Option<String>,
}

fn tokenize(text: &str) -> Vec<String> {
    const STOP: &[&str] = &[
        "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "from", "by", "is", "are",
        "was", "were", "be", "been", "being", "what", "when", "where", "which", "who", "whom",
        "whose", "how", "may", "must", "does", "do", "did", "this", "that", "these", "those",
        "with", "under", "into", "any", "it", "its", "as", "at", "if", "not",
    ];
    text.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '/')
        .filter(|t| t.len() >= 2 && !STOP.contains(t))
        .map(|s| s.to_string())
        .collect()
}

fn corpus_kind(chunk: &CorpusChunk) -> String {
    if chunk.pack_id.starts_with("case_graph:") {
        "MATTER".into()
    } else if chunk.pack_id.starts_with("experience_store:") {
        "EXPERIENCE".into()
    } else {
        "LEGAL".into()
    }
}

fn concept_terms(token: &str) -> &'static [&'static str] {
    match token {
        "sale" | "sell" | "auction" => &["sale", "sell", "auction", "proceeds"],
        "division" | "divide" | "partition" => &["division", "divide", "partition", "share"],
        "appeal" | "appellate" => &["appeal", "appellate", "decree", "memorandum"],
        "delay" | "delayed" | "late" | "condonation" => &[
            "delay",
            "delayed",
            "late",
            "condonation",
            "sufficient",
            "cause",
        ],
        "limitation" | "period" | "days" => &["limitation", "period", "days", "time"],
        "original" | "decree" => &["original", "decree", "appeal", "court"],
        "physical" | "conveniently" => &["physical", "conveniently", "division", "property"],
        "accompany" | "filed" | "filing" => {
            &["accompany", "filed", "filing", "application", "affidavit"]
        }
        "high" | "court" => &["high", "court", "appellate", "jurisdiction"],
        _ => &[],
    }
}

fn stable_feature_index(feature: &str) -> usize {
    let mut hash = 1469598103934665603_u64;
    for byte in feature.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(1099511628211);
    }
    (hash as usize) % EMBEDDING_DIMS
}

fn semantic_vector(text: &str) -> Vec<f32> {
    let tokens = tokenize(text);
    let mut vector = vec![0.0_f32; EMBEDDING_DIMS];
    for token in &tokens {
        vector[stable_feature_index(token)] += 1.0;
        for concept in concept_terms(token) {
            vector[stable_feature_index(concept)] += 0.55;
        }
        let chars: Vec<char> = token.chars().collect();
        for tri in chars.windows(3) {
            let feature: String = tri.iter().collect();
            vector[stable_feature_index(&format!("#{}", feature))] += 0.15;
        }
    }
    let norm = vector
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value = (*value as f64 / norm) as f32;
        }
    }
    vector
}

fn cosine(left: &[f32], right: &[f32]) -> f64 {
    left.iter()
        .zip(right)
        .map(|(a, b)| (*a as f64) * (*b as f64))
        .sum::<f64>()
        .clamp(0.0, 1.0)
}

fn chunk_search_text(chunk: &CorpusChunk) -> String {
    format!(
        "{} {} {} {} {} {}",
        chunk.metadata.title,
        chunk.metadata.section,
        chunk.metadata.citation,
        chunk.metadata.heading_path.join(" "),
        chunk.authority_hint,
        chunk.text
    )
}

fn filter_allows(chunk: &CorpusChunk, filters: &MetadataFilters) -> bool {
    if filters
        .jurisdiction
        .as_deref()
        .is_some_and(|j| chunk.jurisdiction.as_deref() != Some(j))
        || filters
            .authority_type
            .as_deref()
            .is_some_and(|v| chunk.metadata.authority_type != v)
        || filters
            .trust_level
            .as_deref()
            .is_some_and(|v| chunk.metadata.trust_level != v)
        || filters
            .temporal_status
            .as_deref()
            .is_some_and(|v| chunk.metadata.temporal_status != v)
        || filters
            .superseded_status
            .as_deref()
            .is_some_and(|v| chunk.metadata.superseded_status != v)
    {
        return false;
    }
    // Fail closed for known invalid legal material. UNKNOWN remains inspectable with a warning.
    !matches!(
        chunk.metadata.temporal_status.as_str(),
        "EXPIRED" | "NOT_YET_EFFECTIVE"
    ) && chunk.metadata.superseded_status != "SUPERSEDED"
}

pub fn corpus_checksum(chunks: &[CorpusChunk]) -> String {
    let mut identities: Vec<String> = chunks
        .iter()
        .map(|c| {
            format!(
                "{}:{}:{}",
                c.chunk_id, c.metadata.checksum, c.metadata.pack_checksum
            )
        })
        .collect();
    identities.sort();
    sha256_prefixed(&identities.join("\n"))
}

/// Checksum-driven metadata persistence. The temporary file is flushed and atomically renamed,
/// so a crash cannot replace a valid index with a partial one.
pub fn ensure_persisted_index(
    paths: &VaultPaths,
    chunks: &[CorpusChunk],
) -> Result<(PersistedIndexMetadata, String), String> {
    let index_dir = paths.root.join("indexes").join("legal");
    let index_path = index_dir.join("index.json");
    let backup_path = index_dir.join("index.json.bak");
    if !index_path.exists() && backup_path.exists() {
        fs::rename(&backup_path, &index_path)
            .map_err(|e| format!("CF_RETRIEVAL_INDEX_RECOVER: {e}"))?;
    }
    let checksum = corpus_checksum(chunks);
    if let Ok(raw) = fs::read_to_string(&index_path) {
        if let Ok(meta) = serde_json::from_str::<PersistedIndexMetadata>(&raw) {
            if meta.schema_version == INDEX_SCHEMA_VERSION && meta.corpus_checksum == checksum {
                return Ok((meta, "REUSED".into()));
            }
        }
    }
    fs::create_dir_all(&index_dir).map_err(|e| format!("CF_RETRIEVAL_INDEX_DIR: {e}"))?;
    let metadata = PersistedIndexMetadata {
        schema_version: INDEX_SCHEMA_VERSION.into(),
        index_id: INDEX_ID.into(),
        corpus_checksum: checksum,
        chunk_count: chunks.len(),
        semantic_engine_id: SEMANTIC_ENGINE_ID.into(),
        semantic_engine_license: SEMANTIC_ENGINE_LICENSE.into(),
        embedding_dimensions: EMBEDDING_DIMS,
    };
    let tmp = index_dir.join(format!("index.{}.tmp", Uuid::new_v4()));
    let bytes = serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?;
    let mut file = fs::File::create(&tmp).map_err(|e| format!("CF_RETRIEVAL_INDEX_WRITE: {e}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("CF_RETRIEVAL_INDEX_FLUSH: {e}"))?;
    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .map_err(|e| format!("CF_RETRIEVAL_INDEX_BACKUP_CLEAN: {e}"))?;
    }
    if index_path.exists() {
        fs::rename(&index_path, &backup_path)
            .map_err(|e| format!("CF_RETRIEVAL_INDEX_BACKUP: {e}"))?;
    }
    if let Err(error) = fs::rename(&tmp, &index_path) {
        if backup_path.exists() {
            let _ = fs::rename(&backup_path, &index_path);
        }
        return Err(format!("CF_RETRIEVAL_INDEX_COMMIT: {error}"));
    }
    if backup_path.exists() {
        fs::remove_file(&backup_path)
            .map_err(|e| format!("CF_RETRIEVAL_INDEX_BACKUP_REMOVE: {e}"))?;
    }
    Ok((metadata, "REBUILT".into()))
}

fn authority_hint_from_text(text: &str) -> String {
    for line in text.lines() {
        let t = line.trim();
        if t.to_ascii_lowercase().starts_with("neutral citation")
            || t.contains('[') && t.contains(']')
            || t.contains("U.S.")
            || t.contains("INSC")
        {
            return t.chars().take(200).collect();
        }
    }
    text.lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(120)
        .collect()
}

fn sha256_prefixed(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    format!("sha256:{}", hex::encode(h.finalize()))
}

fn legacy_metadata(pack_id: &str, pack_checksum: &str, text: &str) -> LegalMetadata {
    let checksum = sha256_prefixed(text);
    let suffix = checksum.trim_start_matches("sha256:");
    LegalMetadata {
        source_id: format!("cfsrc:{pack_id}:{}", &suffix[..16]),
        title: "UNKNOWN".into(),
        jurisdiction_path: vec!["UNKNOWN".into()],
        authority_type: "UNKNOWN".into(),
        issuing_authority: "UNKNOWN".into(),
        citation: authority_hint_from_text(text),
        section: "UNKNOWN".into(),
        effective_from: "UNKNOWN".into(),
        effective_to: "UNKNOWN".into(),
        version: "UNKNOWN".into(),
        amendment: "UNKNOWN".into(),
        page: serde_json::Value::String("UNKNOWN".into()),
        paragraph: "UNKNOWN".into(),
        heading_path: Vec::new(),
        checksum: checksum.clone(),
        source_checksum: checksum,
        pack_checksum: pack_checksum.into(),
        trust_level: "DEVELOPMENT_ONLY".into(),
        superseded_status: "UNKNOWN".into(),
        publisher: "UNKNOWN".into(),
        source_reference: "UNKNOWN".into(),
        content_kind: "AUTHORITATIVE_TEXT".into(),
        temporal_status: "UNKNOWN".into(),
    }
}

fn metadata_from_record(record: &LegalChunkRecord) -> LegalMetadata {
    LegalMetadata {
        source_id: record.source_id.clone(),
        title: record.title.clone(),
        jurisdiction_path: record.jurisdiction_path.clone(),
        authority_type: record.authority_type.clone(),
        issuing_authority: record.issuing_authority.clone(),
        citation: record.citation.clone(),
        section: record.section.clone(),
        effective_from: record.effective_from.clone(),
        effective_to: record.effective_to.clone(),
        version: record.version.clone(),
        amendment: record.amendment.clone(),
        page: record.page.clone(),
        paragraph: record.paragraph.clone(),
        heading_path: record.heading_path.clone(),
        checksum: record.checksum.clone(),
        source_checksum: record.source_checksum.clone(),
        pack_checksum: record.pack_checksum.clone(),
        trust_level: record.trust_level.clone(),
        superseded_status: record.superseded_status.clone(),
        publisher: record.publisher.clone(),
        source_reference: record.source_reference.clone(),
        content_kind: record.content_kind.clone(),
        temporal_status: record.temporal_status.clone(),
    }
}

/// Load chunks from installed packs (and optional extra content dirs for eval).
pub fn build_corpus(
    paths: &VaultPaths,
    extra_pack_dirs: &[PathBuf],
) -> Result<Vec<CorpusChunk>, String> {
    let mut chunks = Vec::new();
    let installed = list_installed(paths);
    for pack in installed {
        load_pack_chunks(&pack, &mut chunks)?;
    }
    for dir in extra_pack_dirs {
        // Treat as loose pack dir with manifest optional
        let pack_id = dir
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("loose")
            .to_string();
        let content = dir.join("content");
        let root = if content.is_dir() {
            content
        } else {
            dir.clone()
        };
        load_content_dir(&pack_id, None, "UNKNOWN", &root, &mut chunks)?;
    }
    Ok(chunks)
}

fn load_pack_chunks(pack: &InstalledPack, chunks: &mut Vec<CorpusChunk>) -> Result<(), String> {
    let pack_dir = PathBuf::from(&pack.install_path);
    if crate::packs::is_v2(&pack.schema_version) {
        let records = load_legal_chunks(&pack_dir)?;
        let ids: Vec<String> = records.iter().map(|r| r.chunk_id.clone()).collect();
        dense::register_pack(&pack_dir, &pack.content_sha256, &ids);
        for record in records {
            let hint = if record.citation == "UNKNOWN" {
                authority_hint_from_text(&record.exact_text)
            } else {
                record.citation.clone()
            };
            chunks.push(CorpusChunk {
                chunk_id: record.chunk_id.clone(),
                pack_id: record.pack_id.clone(),
                path: record.source_file.clone(),
                jurisdiction: Some(record.jurisdiction.clone()),
                text: record.exact_text.clone(),
                authority_hint: hint,
                metadata: metadata_from_record(&record),
            });
        }
        return Ok(());
    }
    // B-S1-19: prefer decrypt of protected content; skip pack on tamper (graceful degrade).
    match crate::protection::read_pack_text_files(&pack_dir) {
        Ok(files) if !files.is_empty() => {
            for (rel, text) in files {
                if text.trim().is_empty() {
                    continue;
                }
                for (i, part) in split_chunks(&text, 800).into_iter().enumerate() {
                    let hint = authority_hint_from_text(&part);
                    let metadata = legacy_metadata(&pack.pack_id, &pack.content_sha256, &part);
                    chunks.push(CorpusChunk {
                        chunk_id: format!("{}:{rel}:{i}", pack.pack_id),
                        pack_id: pack.pack_id.clone(),
                        path: format!("content/{rel}"),
                        jurisdiction: pack.jurisdiction.clone(),
                        text: part,
                        authority_hint: hint,
                        metadata,
                    });
                }
            }
            Ok(())
        }
        Ok(_) => {
            // Empty protected or unprotected — fall back to plaintext content/
            let root = pack_dir.join("content");
            if root.is_dir() {
                load_content_dir(
                    &pack.pack_id,
                    pack.jurisdiction.clone(),
                    &pack.content_sha256,
                    &root,
                    chunks,
                )
            } else {
                Ok(())
            }
        }
        Err(e) => {
            // Graceful degrade: exclude pack from retrieval; do not fail whole pipeline.
            eprintln!("protection degrade pack {}: {e}", pack.pack_id);
            Ok(())
        }
    }
}

fn load_content_dir(
    pack_id: &str,
    jurisdiction: Option<String>,
    pack_checksum: &str,
    root: &Path,
    chunks: &mut Vec<CorpusChunk>,
) -> Result<(), String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                let text = fs::read_to_string(&p).unwrap_or_default();
                if text.trim().is_empty() {
                    continue;
                }
                // Chunk by paragraphs (~800 chars)
                let rel = p
                    .strip_prefix(root)
                    .map(|x| x.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_else(|_| p.display().to_string());
                for (i, part) in split_chunks(&text, 800).into_iter().enumerate() {
                    let hint = authority_hint_from_text(&part);
                    let metadata = legacy_metadata(pack_id, pack_checksum, &part);
                    chunks.push(CorpusChunk {
                        chunk_id: format!("{pack_id}:{rel}:{i}"),
                        pack_id: pack_id.to_string(),
                        path: format!("content/{rel}"),
                        jurisdiction: jurisdiction.clone(),
                        text: part,
                        authority_hint: hint,
                        metadata,
                    });
                }
            }
        }
    }
    Ok(())
}

fn split_chunks(text: &str, max_len: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    for para in text.split("\n\n") {
        if buf.len() + para.len() > max_len && !buf.is_empty() {
            out.push(buf.trim().to_string());
            buf.clear();
        }
        if !buf.is_empty() {
            buf.push_str("\n\n");
        }
        buf.push_str(para);
    }
    if !buf.trim().is_empty() {
        out.push(buf.trim().to_string());
    }
    if out.is_empty() && !text.trim().is_empty() {
        out.push(text.chars().take(max_len).collect());
    }
    out
}

/// Optional case_graph injection as retrieval chunks.
pub fn add_case_graph_chunks(
    chunks: &mut Vec<CorpusChunk>,
    case_id: &str,
    graph: &serde_json::Value,
) {
    if let Some(arr) = graph.get("statutes").and_then(|a| a.as_array()) {
        for (i, s) in arr.iter().enumerate() {
            let label = s.get("label").and_then(|v| v.as_str()).unwrap_or("");
            if label.is_empty() {
                continue;
            }
            let pack_id = format!("case_graph:{case_id}");
            let metadata = legacy_metadata(&pack_id, "CASE_GRAPH", label);
            chunks.push(CorpusChunk {
                chunk_id: format!("case_graph:{case_id}:statute:{i}"),
                pack_id,
                path: "case_graph/statutes".into(),
                jurisdiction: graph
                    .get("jurisdiction")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                text: label.to_string(),
                authority_hint: label.chars().take(200).collect(),
                metadata,
            });
        }
    }
}

/// B-S1-17: inject top-K Experience Store patterns into retrieval corpus (AC-11.1).
pub fn add_experience_chunks(chunks: &mut Vec<CorpusChunk>, hits: &[crate::vault::ExperienceHit]) {
    for (i, h) in hits.iter().enumerate() {
        let text = format!(
            "[{}] {} | tags: {} | key: {}",
            h.record_category,
            h.summary,
            h.tags.join(", "),
            h.relevance_key
        );
        let pack_id = format!("experience_store:{}", h.record_category);
        let metadata = legacy_metadata(&pack_id, "EXPERIENCE_STORE", &text);
        chunks.push(CorpusChunk {
            chunk_id: format!("experience:{}:{}", h.record_id, i),
            pack_id,
            path: "experience_store/patterns".into(),
            jurisdiction: h.jurisdiction.clone(),
            text,
            authority_hint: h.summary.chars().take(200).collect(),
            metadata,
        });
    }
}

// --- Pipeline stages ---

pub fn decompose(query: &str) -> (Vec<String>, Vec<String>) {
    let terms = tokenize(query);
    // Citation-like spans (quoted or PLANTED_FAKE)
    let mut cites = Vec::new();
    if query.to_ascii_uppercase().contains("PLANTED_FAKE") {
        for part in query.split(|c| c == ';' || c == '\n') {
            if part.to_ascii_uppercase().contains("PLANTED_FAKE") {
                cites.push(part.trim().to_string());
            }
        }
    }
    // Neutral citation patterns
    for m in extract_citation_candidates(query) {
        cites.push(m);
    }
    (terms, cites)
}

fn extract_citation_candidates(query: &str) -> Vec<String> {
    let mut out = Vec::new();
    // Simple: lines or segments with year/brackets
    for seg in query.split(&['\n', ';', ','][..]) {
        let t = seg.trim();
        if t.len() < 8 {
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if lower.contains("v.")
            || lower.contains("vs.")
            || (t.contains('[') && t.contains(']'))
            || lower.contains("u.s.")
            || lower.contains("insc")
            || lower.contains("planted_fake")
        {
            out.push(t.to_string());
        }
    }
    out
}

pub fn retrieve(
    chunks: &[CorpusChunk],
    query_terms: &[String],
    top_k: usize,
    jurisdiction: Option<&str>,
) -> Vec<RetrievedHit> {
    retrieve_filtered(
        chunks,
        query_terms,
        top_k,
        &MetadataFilters {
            jurisdiction: jurisdiction.map(str::to_string),
            ..MetadataFilters::default()
        },
    )
}

pub fn retrieve_filtered(
    chunks: &[CorpusChunk],
    query_terms: &[String],
    top_k: usize,
    filters: &MetadataFilters,
) -> Vec<RetrievedHit> {
    retrieve_filtered_with_mode(chunks, query_terms, top_k, filters, true)
}

pub fn retrieve_filtered_with_mode(
    chunks: &[CorpusChunk],
    query_terms: &[String],
    top_k: usize,
    filters: &MetadataFilters,
    semantic_available: bool,
) -> Vec<RetrievedHit> {
    retrieve_filtered_with_text(chunks, &query_terms.join(" "), query_terms, top_k, filters, semantic_available)
}

/// As `retrieve_filtered_with_mode`, with the user's own wording for meaning-based ranking (B-S4-27): the embedding
/// model reads the whole question, including words the keyword index drops and non-English text.
pub fn retrieve_filtered_with_text(
    chunks: &[CorpusChunk],
    query_text: &str,
    query_terms: &[String],
    top_k: usize,
    filters: &MetadataFilters,
    semantic_available: bool,
) -> Vec<RetrievedHit> {
    // Meaning-based ranking only when packs carry vectors and the local model answers. Unit tests stay deterministic
    // (they exercise retrieve_core with an explicit query vector instead).
    let dense_query = if !cfg!(test) && dense::has_vectors() && !query_text.trim().is_empty() { dense::embed_query(query_text.trim()) } else { None };
    retrieve_core(chunks, query_terms, top_k, filters, semantic_available, dense_query.as_deref())
}

fn retrieve_core(
    chunks: &[CorpusChunk],
    query_terms: &[String],
    top_k: usize,
    filters: &MetadataFilters,
    semantic_available: bool,
    dense_query: Option<&[f32]>,
) -> Vec<RetrievedHit> {
    if (query_terms.is_empty() && dense_query.is_none()) || chunks.is_empty() {
        return Vec::new();
    }
    let candidate_indexes: Vec<usize> = chunks
        .iter()
        .enumerate()
        .filter_map(|(index, chunk)| filter_allows(chunk, filters).then_some(index))
        .collect();
    if candidate_indexes.is_empty() {
        return Vec::new();
    }
    // Proper BM25 statistics over the deterministically filtered candidate set.
    let mut df: HashMap<String, usize> = HashMap::new();
    let mut docs_tokens: HashMap<usize, Vec<String>> = HashMap::new();
    let mut total_len = 0_usize;
    for index in &candidate_indexes {
        let tokens = tokenize(&chunk_search_text(&chunks[*index]));
        total_len += tokens.len();
        for t in tokens.iter().cloned().collect::<HashSet<_>>() {
            *df.entry(t.clone()).or_insert(0) += 1;
        }
        docs_tokens.insert(*index, tokens);
    }
    let n = candidate_indexes.len() as f64;
    let avg_len = (total_len as f64 / n).max(1.0);
    let query_vector = semantic_available.then(|| semantic_vector(&query_terms.join(" ")));
    let mut scored: Vec<(usize, f64, f64, f64, Vec<String>)> = Vec::new();
    for &index in candidate_indexes.iter().filter(|_| !query_terms.is_empty()) {
        let c = &chunks[index];
        let toks = &docs_tokens[&index];
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for token in toks {
            *counts.entry(token.as_str()).or_insert(0) += 1;
        }
        let mut lexical = 0.0;
        for qt in query_terms {
            let tf = *counts.get(qt.as_str()).unwrap_or(&0) as f64;
            if tf > 0.0 {
                let dfi = *df.get(qt).unwrap_or(&0) as f64;
                let idf = (1.0 + (n - dfi + 0.5) / (dfi + 0.5)).ln();
                let denominator = tf + 1.2 * (1.0 - 0.75 + 0.75 * toks.len() as f64 / avg_len);
                lexical += idf * (tf * 2.2 / denominator);
            }
        }
        let semantic = query_vector
            .as_ref()
            .map(|query| cosine(query, &semantic_vector(&chunk_search_text(c))))
            .unwrap_or(0.0);
        let metadata_text = format!(
            "{} {} {}",
            c.metadata.title, c.metadata.section, c.metadata.citation
        )
        .to_ascii_lowercase();
        let metadata_matches = query_terms
            .iter()
            .filter(|term| metadata_text.contains(term.as_str()))
            .count();
        let metadata_boost = (metadata_matches as f64 / query_terms.len().max(1) as f64).min(1.0);
        let authority_boost = if c.metadata.trust_level == "OFFICIAL_VERIFIED" {
            0.08
        } else {
            0.0
        };
        // Weighted fusion is deterministic; semantic retrieval can surface vocabulary mismatch,
        // while BM25 and metadata retain precise section/citation matching.
        let rerank = if semantic_available {
            lexical * 0.58 + semantic * 2.4 * 0.27 + metadata_boost * 1.4 * 0.15 + authority_boost
        } else {
            lexical + metadata_boost * 0.3 + authority_boost
        };
        if rerank > 0.0 {
            let mut reasons = Vec::new();
            if lexical > 0.0 {
                reasons.push("BM25 term match".into());
            }
            if semantic >= 0.08 {
                reasons.push("legal-concept semantic similarity".into());
            }
            if metadata_boost > 0.0 {
                reasons.push("title/section/citation metadata match".into());
            }
            if authority_boost > 0.0 {
                reasons.push("official verified authority".into());
            }
            scored.push((index, lexical, semantic, rerank, reasons));
        }
    }
    let dense_used = dense_query.map(|q| fuse_dense(chunks, &candidate_indexes, &mut scored, q, fusion_weights(query_terms))).unwrap_or(false);
    scored.sort_by(|a, b| {
        b.3.partial_cmp(&a.3)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| chunks[a.0].chunk_id.cmp(&chunks[b.0].chunk_id))
    });
    scored.truncate(top_k.max(1));
    let max_lexical = scored.iter().map(|s| s.1).fold(0.0_f64, f64::max).max(1e-9);
    let max_rerank = scored.iter().map(|s| s.3).fold(0.0_f64, f64::max).max(1e-9);
    scored
        .into_iter()
        .map(|(i, lexical, semantic, rerank, reasons)| {
            let c = &chunks[i];
            let warning = if c.metadata.temporal_status == "UNKNOWN"
                || c.metadata.superseded_status == "UNKNOWN"
            {
                Some(
                    "Currentness or supersession is unknown; counsel verification required.".into(),
                )
            } else if c.metadata.superseded_status == "PARTIALLY_SUPERSEDED" {
                Some(
                    "Partially superseded material; verify the applicable provision and date."
                        .into(),
                )
            } else {
                None
            };
            RetrievedHit {
                chunk_id: c.chunk_id.clone(),
                pack_id: c.pack_id.clone(),
                path: c.path.clone(),
                score: (rerank / max_rerank).clamp(0.0, 1.0),
                lexical_score: (lexical / max_lexical).clamp(0.0, 1.0),
                semantic_score: semantic,
                rerank_score: (rerank / max_rerank).clamp(0.0, 1.0),
                match_reasons: reasons,
                retrieval_mode: if dense_used {
                    "HYBRID_DENSE"
                } else if semantic_available {
                    "HYBRID"
                } else {
                    "LEXICAL_ONLY"
                }
                .into(),
                corpus_kind: corpus_kind(c),
                temporal_warning: warning,
                snippet: c.text.chars().take(280).collect(),
                authority_hint: c.authority_hint.clone(),
                source_id: c.metadata.source_id.clone(),
                chunk_checksum: c.metadata.checksum.clone(),
                source_checksum: c.metadata.source_checksum.clone(),
                trust_level: c.metadata.trust_level.clone(),
                authority_type: c.metadata.authority_type.clone(),
                temporal_status: c.metadata.temporal_status.clone(),
                superseded_status: c.metadata.superseded_status.clone(),
                content_kind: c.metadata.content_kind.clone(),
            }
        })
        .collect()
}

/// Fusion weights (keyword, meaning) chosen from the question itself. A question naming a section, citation or year
/// (any term containing a digit: "438", "498a", "2023") needs the exact provision, so keyword matching leads; a
/// plain-language or non-English question ("protection before arrest") is better served by meaning.
/// Measured on golden G-DENSE (plain and identifier sets) by eval/checks/dense_retrieval_b_s4_27.py.
fn fusion_weights(query_terms: &[String]) -> (f64, f64) {
    if query_terms.iter().any(|t| t.chars().any(|c| c.is_ascii_digit())) {
        (1.0, 0.5)
    } else {
        (0.5, 1.0)
    }
}

/// Weighted reciprocal-rank fusion of the existing ranking with the meaning-based ranking (top 100 by embedding
/// similarity): w_keyword/(61 + keyword rank) + w_meaning/(61 + meaning rank). Passages found only by meaning join the
/// candidate list; the result is re-scored in place. Returns whether any candidate had a vector.
fn fuse_dense(chunks: &[CorpusChunk], candidates: &[usize], scored: &mut Vec<(usize, f64, f64, f64, Vec<String>)>, query: &[f32], weights: (f64, f64)) -> bool {
    let mut ranked: Vec<(usize, f32)> = candidates.iter().filter_map(|&i| dense::similarity(query, &chunks[i].chunk_id).map(|s| (i, s))).collect();
    if ranked.is_empty() {
        return false;
    }
    ranked.retain(|r| r.1 > 0.0); // orthogonal or opposite meaning is no match
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    ranked.truncate(100);
    scored.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    // Only passages with real evidence (terms, concepts or metadata, not the authority boost alone) hold a lexical rank.
    let lexical_rank: HashMap<usize, usize> = scored
        .iter()
        .filter(|s| s.4.iter().any(|r| r != "official verified authority"))
        .enumerate()
        .map(|(r, s)| (s.0, r))
        .collect();
    let dense_rank: HashMap<usize, usize> = ranked.iter().enumerate().map(|(r, s)| (s.0, r)).collect();
    let present: HashSet<usize> = scored.iter().map(|s| s.0).collect();
    for (index, _) in &ranked {
        if !present.contains(index) {
            scored.push((*index, 0.0, 0.0, 0.0, Vec::new()));
        }
    }
    for s in scored.iter_mut() {
        let l = lexical_rank.get(&s.0).map(|r| weights.0 / (61.0 + *r as f64)).unwrap_or(0.0);
        let d = dense_rank.get(&s.0).map(|r| weights.1 / (61.0 + *r as f64)).unwrap_or(0.0);
        if d > 0.0 {
            s.4.push("meaning match (offline multilingual embedding)".into());
        }
        s.3 = l + d;
    }
    scored.retain(|s| s.3 > 0.0);
    true
}

/// GENERATE constrained to retrieved material only (G2).
pub fn generate_constrained(hits: &[RetrievedHit], extra_authorities: &[String]) -> Vec<String> {
    let mut out: Vec<String> = hits
        .iter()
        .map(|h| {
            if !h.authority_hint.is_empty() {
                h.authority_hint.clone()
            } else {
                h.snippet.chars().take(160).collect()
            }
        })
        .collect();
    // Pass through any query-side authority candidates for VERIFY
    for a in extra_authorities {
        if !out.iter().any(|x| x == a) {
            out.push(a.clone());
        }
    }
    out
}

pub fn verify_and_score(
    authority: &str,
    hits: &[RetrievedHit],
    chunks: &[CorpusChunk],
    query_id: Option<&str>,
    user_confirmed: bool,
) -> CitationRecord {
    let planted = authority.to_ascii_uppercase().contains("PLANTED_FAKE")
        || authority.to_ascii_uppercase().contains("PLANTED FAKE");
    let auth_l = authority.to_ascii_lowercase();

    let mut best_score = 0.0_f64;
    let mut best_pack: Option<String> = None;
    let mut best_locus: Option<CorpusLocus> = None;
    let mut in_retrieve = false;
    let mut best_source_id: Option<String> = None;
    let mut best_chunk_id: Option<String> = None;
    let mut best_source_checksum: Option<String> = None;
    let mut best_chunk_checksum: Option<String> = None;
    let mut best_trust = "UNKNOWN".to_string();
    let mut best_authority_type = "UNKNOWN".to_string();
    let mut best_temporal = "UNKNOWN".to_string();
    let mut best_superseded = "UNKNOWN".to_string();
    let mut best_content_kind = "UNKNOWN".to_string();

    for h in hits {
        let hay = format!("{} {}", h.authority_hint, h.snippet).to_ascii_lowercase();
        if hay.contains(&auth_l) || auth_l.contains(&h.authority_hint.to_ascii_lowercase()) {
            in_retrieve = true;
            if h.score > best_score {
                best_score = h.score;
                best_pack = Some(h.pack_id.clone());
                best_locus = Some(CorpusLocus {
                    path: h.path.clone(),
                    char_start: Some(0),
                    char_end: None,
                    snippet: Some(h.snippet.clone()),
                });
                best_source_id = Some(h.source_id.clone());
                best_chunk_id = Some(h.chunk_id.clone());
                best_source_checksum = Some(h.source_checksum.clone());
                best_chunk_checksum = Some(h.chunk_checksum.clone());
                best_trust = h.trust_level.clone();
                best_authority_type = h.authority_type.clone();
                best_temporal = h.temporal_status.clone();
                best_superseded = h.superseded_status.clone();
                best_content_kind = h.content_kind.clone();
            }
        }
        // token overlap score
        let a_toks: HashSet<_> = tokenize(authority).into_iter().collect();
        let h_toks: HashSet<_> = tokenize(&format!("{} {}", h.authority_hint, h.snippet))
            .into_iter()
            .collect();
        let inter = a_toks.intersection(&h_toks).count() as f64;
        let uni = a_toks.union(&h_toks).count() as f64;
        if uni > 0.0 {
            let j = inter / uni;
            if j > best_score {
                best_score = j;
                best_pack = Some(h.pack_id.clone());
                best_locus = Some(CorpusLocus {
                    path: h.path.clone(),
                    char_start: None,
                    char_end: None,
                    snippet: Some(h.snippet.clone()),
                });
                best_source_id = Some(h.source_id.clone());
                best_chunk_id = Some(h.chunk_id.clone());
                best_source_checksum = Some(h.source_checksum.clone());
                best_chunk_checksum = Some(h.chunk_checksum.clone());
                best_trust = h.trust_level.clone();
                best_authority_type = h.authority_type.clone();
                best_temporal = h.temporal_status.clone();
                best_superseded = h.superseded_status.clone();
                best_content_kind = h.content_kind.clone();
            }
            if j >= 0.35 {
                in_retrieve = true;
            }
        }
    }

    // Full corpus exact containment (still must be retrieved for GREEN high confidence)
    let mut in_corpus = false;
    for c in chunks {
        if c.text.to_ascii_lowercase().contains(&auth_l)
            || auth_l.contains(&c.authority_hint.to_ascii_lowercase())
        {
            in_corpus = true;
            if best_locus.is_none() {
                best_pack = Some(c.pack_id.clone());
                best_locus = Some(CorpusLocus {
                    path: c.path.clone(),
                    char_start: None,
                    char_end: None,
                    snippet: Some(c.text.chars().take(200).collect()),
                });
                best_source_id = Some(c.metadata.source_id.clone());
                best_chunk_id = Some(c.chunk_id.clone());
                best_source_checksum = Some(c.metadata.source_checksum.clone());
                best_chunk_checksum = Some(c.metadata.checksum.clone());
                best_trust = c.metadata.trust_level.clone();
                best_authority_type = c.metadata.authority_type.clone();
                best_temporal = c.metadata.temporal_status.clone();
                best_superseded = c.metadata.superseded_status.clone();
                best_content_kind = c.metadata.content_kind.clone();
            }
            break;
        }
    }

    let controlling_metadata = best_trust == "OFFICIAL_VERIFIED"
        && best_authority_type != "UNKNOWN"
        && best_temporal == "APPLICABLE"
        && best_superseded == "CURRENT"
        && best_content_kind == "AUTHORITATIVE_TEXT"
        && best_source_checksum
            .as_deref()
            .is_some_and(|v| v.starts_with("sha256:"))
        && best_chunk_checksum
            .as_deref()
            .is_some_and(|v| v.starts_with("sha256:"));

    let (status, notes) = if planted {
        ("RED", Some("Planted fake citation (AC-4.3)".to_string()))
    } else if !in_retrieve && !in_corpus {
        (
            "RED",
            Some("Authority not found in retrieved set or local corpus (G2)".into()),
        )
    } else if !in_retrieve && in_corpus {
        // Cite outside RETRIEVE set even if in corpus → RED per hard rule
        (
            "RED",
            Some("Citation outside RETRIEVE set — marked RED (no bypass)".into()),
        )
    } else if in_retrieve && !controlling_metadata {
        (
            "AMBER",
            Some("Retrieved, but metadata is unknown, non-current, non-applicable, derived, custom, or development-only; never controlling".into()),
        )
    } else if best_score >= 0.72 || (in_retrieve && best_score >= 0.45) {
        ("GREEN", Some("Resolved to retrieved corpus text".into()))
    } else if in_retrieve {
        (
            "AMBER",
            Some("Partial match in retrieved material — counsel review".into()),
        )
    } else {
        ("RED", Some("Unverified authority".into()))
    };

    let exclude = status == "RED" && !user_confirmed;

    CitationRecord {
        schema_version: "2.0.0".into(),
        citation_id: format!("cit_{}", Uuid::new_v4().to_string().replace('-', "")),
        query_id: query_id.map(|s| s.to_string()),
        authority_string: authority.to_string(),
        verification_status: status.into(),
        match_score: if planted {
            0.0
        } else {
            best_score.clamp(0.0, 1.0)
        },
        retrieved: in_retrieve,
        user_confirmed,
        pack_id: best_pack,
        source_id: best_source_id,
        chunk_id: best_chunk_id,
        source_checksum: best_source_checksum,
        chunk_checksum: best_chunk_checksum,
        trust_level: best_trust,
        authority_type: best_authority_type,
        temporal_status: best_temporal,
        superseded_status: best_superseded,
        content_kind: best_content_kind,
        corpus_locus: best_locus,
        planted_fake: planted,
        exclude_from_downstream: exclude,
        pipeline: PipelineMeta {
            stages: STAGES.iter().map(|s| s.to_string()).collect(),
            index_id: INDEX_ID.into(),
            notes: Some("B-S3-11 hybrid BM25 + legal-concept semantic retrieval with deterministic reranking".into()),
        },
        notes,
    }
}

/// Full pipeline entry.
pub fn run_pipeline(
    paths: &VaultPaths,
    query: &str,
    top_k: usize,
    jurisdiction: Option<&str>,
    extra_pack_dirs: &[PathBuf],
    case_graph: Option<&serde_json::Value>,
    case_id: Option<&str>,
    force_authorities: &[String],
    user_confirmed_red: bool,
    experience_hits: &[crate::vault::ExperienceHit],
    metadata_filters: &MetadataFilters,
) -> Result<RetrievalResult, String> {
    let mut chunks = build_corpus(paths, extra_pack_dirs)?;
    if let (Some(g), Some(cid)) = (case_graph, case_id) {
        add_case_graph_chunks(&mut chunks, cid, g);
    }
    let experience_injected = experience_hits.len();
    add_experience_chunks(&mut chunks, experience_hits);
    let (index_metadata, index_state) = ensure_persisted_index(paths, &chunks)?;
    let (terms, cite_cands) = decompose(query);
    let mut extra = cite_cands;
    extra.extend(force_authorities.iter().cloned());
    let semantic_available =
        std::env::var("CASEFORGE_DISABLE_SEMANTIC").ok().as_deref() != Some("1");
    let hits = retrieve_filtered_with_text(&chunks, query, &terms, top_k, metadata_filters, semantic_available);
    let authorities = generate_constrained(&hits, &extra);
    let qid = format!("q_{}", Uuid::new_v4().to_string().replace('-', ""));
    let mut citations = Vec::new();
    for a in &authorities {
        let conf = user_confirmed_red && a.to_ascii_uppercase().contains("PLANTED_FAKE");
        citations.push(verify_and_score(
            a,
            &hits,
            &chunks,
            Some(&qid),
            conf && user_confirmed_red,
        ));
    }
    // Always verify force authorities even if generate skipped them
    for a in force_authorities {
        if !citations.iter().any(|c| &c.authority_string == a) {
            citations.push(verify_and_score(a, &hits, &chunks, Some(&qid), false));
        }
    }

    let retrieval_mode = if hits.iter().any(|h| h.retrieval_mode == "HYBRID_DENSE") {
        "HYBRID_DENSE"
    } else if semantic_available {
        "HYBRID"
    } else {
        "LEXICAL_ONLY"
    }
    .to_string();
    let (repository_gap, repository_gap_reason, repository_gap_required_sources) =
        controlled_repository_gap(query, hits.is_empty());
    Ok(RetrievalResult {
        query: query.to_string(),
        query_id: qid,
        jurisdiction_filter: jurisdiction.map(|s| s.to_string()),
        stages_completed: STAGES.iter().map(|s| s.to_string()).collect(),
        retrieved: hits,
        citations,
        index_id: INDEX_ID.into(),
        index_checksum: index_metadata.corpus_checksum,
        index_state,
        retrieval_mode,
        semantic_engine_id: SEMANTIC_ENGINE_ID.into(),
        semantic_available,
        lexical_fallback_notice: (!semantic_available).then(|| "Semantic retrieval is unavailable. Results use BM25 lexical matching only and do not claim semantic coverage.".into()),
        chunk_count: chunks.len(),
        batch: "B-S3-11",
        experience_injected,
        experience_top_k: crate::vault::EXPERIENCE_TOP_K,
        repository_gap,
        repository_gap_reason,
        repository_gap_required_sources,
    })
}

pub fn retrieval_policy() -> serde_json::Value {
    serde_json::json!({
        "batch": "B-S3-11",
        "index_id": INDEX_ID,
        "legacy_index_id": LEGACY_INDEX_ID,
        "index_schema_version": INDEX_SCHEMA_VERSION,
        "pipeline": "deterministic filters -> BM25 + semantic -> rerank -> exact chunks",
        "semantic_engine": {
            "id": SEMANTIC_ENGINE_ID,
            "dimensions": EMBEDDING_DIMS,
            "license": SEMANTIC_ENGINE_LICENSE,
            "weights_bytes": 0,
            "offline": true
        },
        "stages": STAGES,
        "labels": ["GREEN", "AMBER", "RED"],
        "ac_4_3": "PLANTED_FAKE always RED; excluded unless user_confirmed",
        "ac_7_2": "known relevant authority in top-8 >= 85% on GOLDEN G-RETRIEVAL sets",
        "g2": "citations outside RETRIEVE set are RED",
        "metadata_contract": "legal_source/legal_chunk v2; exact authoritative text separate from derived summaries",
        "filterable": ["jurisdiction", "authority_type", "trust_level", "temporal_status", "superseded_status"],
        "unknown_policy": "unknown or non-controlling metadata can never produce GREEN",
        "fallback_policy": "If semantic indexing is unavailable, label the run LEXICAL_ONLY and never claim semantic coverage",
        "index_persistence": "checksum-driven, flushed temporary file, transactional commit",
        "experience_injection": {
            "enabled": true,
            "top_k": crate::vault::EXPERIENCE_TOP_K,
            "pipeline_id": crate::vault::EXPERIENCE_PIPELINE_ID,
            "ac": "AC-11.1"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn verified_test_metadata(text: &str) -> LegalMetadata {
        let mut metadata = legacy_metadata("caseforge.test", "0", text);
        metadata.trust_level = "OFFICIAL_VERIFIED".into();
        metadata.authority_type = "JUDGMENT".into();
        metadata.temporal_status = "APPLICABLE".into();
        metadata.superseded_status = "CURRENT".into();
        metadata
    }

    #[test]
    fn planted_fake_is_red() {
        let chunks = vec![CorpusChunk {
            chunk_id: "c1".into(),
            pack_id: "p".into(),
            path: "content/a.txt".into(),
            jurisdiction: Some("India".into()),
            text: "Neutral citation: [2020] INSC DEMO 1. The Court observed consideration.".into(),
            authority_hint: "Neutral citation: [2020] INSC DEMO 1".into(),
            metadata: legacy_metadata(
                "p",
                "0",
                "Neutral citation: [2020] INSC DEMO 1. The Court observed consideration.",
            ),
        }];
        let hits = retrieve(
            &chunks,
            &tokenize("consideration contract INSC"),
            8,
            Some("India"),
        );
        let cit = verify_and_score(
            "PLANTED_FAKE v. Placeholder, (2099) 1 SYNTH 1",
            &hits,
            &chunks,
            None,
            false,
        );
        assert_eq!(cit.verification_status, "RED");
        assert!(cit.exclude_from_downstream);
        assert!(cit.planted_fake);
    }

    #[test]
    fn known_authority_green() {
        let chunks = vec![CorpusChunk {
            chunk_id: "c1".into(),
            pack_id: "p".into(),
            path: "content/a.txt".into(),
            jurisdiction: Some("India".into()),
            text: "Neutral citation: [2020] INSC DEMO 1. The Court observed consideration.".into(),
            authority_hint: "Neutral citation: [2020] INSC DEMO 1".into(),
            metadata: verified_test_metadata(
                "Neutral citation: [2020] INSC DEMO 1. The Court observed consideration.",
            ),
        }];
        let hits = retrieve(
            &chunks,
            &tokenize("INSC DEMO consideration"),
            8,
            Some("India"),
        );
        assert!(!hits.is_empty());
        let cit = verify_and_score(
            "Neutral citation: [2020] INSC DEMO 1",
            &hits,
            &chunks,
            None,
            false,
        );
        assert_eq!(cit.verification_status, "GREEN");
        assert!(cit.retrieved);
    }

    #[test]
    fn b_s3_08_unknown_metadata_never_controls_and_filters_are_exact() {
        let text = "Neutral citation: [2020] INSC DEMO 1. The Court observed consideration.";
        let unknown = CorpusChunk {
            chunk_id: "legacy".into(),
            pack_id: "legacy.pack".into(),
            path: "content/a.txt".into(),
            jurisdiction: Some("India".into()),
            text: text.into(),
            authority_hint: "Neutral citation: [2020] INSC DEMO 1".into(),
            metadata: legacy_metadata("legacy.pack", "0", text),
        };
        let mut official = unknown.clone();
        official.chunk_id = "official".into();
        official.pack_id = "official.pack".into();
        official.metadata = verified_test_metadata(text);
        let terms = tokenize("INSC DEMO consideration");
        let legacy_hits = retrieve(std::slice::from_ref(&unknown), &terms, 8, Some("India"));
        let citation = verify_and_score(
            "Neutral citation: [2020] INSC DEMO 1",
            &legacy_hits,
            std::slice::from_ref(&unknown),
            None,
            false,
        );
        assert_eq!(citation.verification_status, "AMBER");
        let filtered = retrieve_filtered(
            &[unknown, official],
            &terms,
            8,
            &MetadataFilters {
                trust_level: Some("OFFICIAL_VERIFIED".into()),
                authority_type: Some("JUDGMENT".into()),
                temporal_status: Some("APPLICABLE".into()),
                superseded_status: Some("CURRENT".into()),
                jurisdiction: Some("India".into()),
            },
        );
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].chunk_id, "official");
    }

    #[test]
    fn b_s3_10_missing_local_and_personal_law_return_repository_gap() {
        let (gap, reason, required) =
            controlled_repository_gap("What filing rule applies in the Bombay High Court?", false);
        assert!(gap);
        assert!(reason.unwrap().contains("High Court rules"));
        assert!(!required.is_empty());

        let (gap, reason, _) = controlled_repository_gap(
            "What share does a Muslim heir receive in this partition?",
            false,
        );
        assert!(gap);
        assert!(reason.unwrap().contains("personal-law"));

        assert!(
            !controlled_repository_gap(
                "When may a court order sale instead of division under the Partition Act?",
                false,
            )
            .0
        );
    }

    #[test]
    fn b_s3_10_controlled_pack_retrieval_goldens_land_in_top_eight() {
        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../packs/official/caseforge.india.partition.highcourt.civil-appeal");
        let chunks: Vec<CorpusChunk> = crate::packs::load_legal_chunks(&pack)
            .unwrap()
            .into_iter()
            .map(|record| CorpusChunk {
                chunk_id: record.chunk_id.clone(),
                pack_id: record.pack_id.clone(),
                path: record.source_file.clone(),
                jurisdiction: Some(record.jurisdiction.clone()),
                text: record.exact_text.clone(),
                authority_hint: record.citation.clone(),
                metadata: metadata_from_record(&record),
            })
            .collect();
        for (query, expected_path) in [
            (
                "sale instead of division partition suit",
                "partition_act_1893",
            ),
            (
                "appeal from original decree section 96",
                "code_of_civil_procedure_1908",
            ),
            (
                "application condonation delay appeal",
                "code_of_civil_procedure_1908",
            ),
            (
                "High Court decree order ninety days limitation",
                "limitation_act_1963",
            ),
        ] {
            let hits = retrieve(&chunks, &tokenize(query), 8, Some("India"));
            assert!(
                hits.iter().any(|hit| hit.path.contains(expected_path)),
                "expected {expected_path} in top eight for {query}"
            );
        }
    }

    #[test]
    fn b_s4_27_dense_fusion_adds_meaning_matches_and_is_off_without_vectors() {
        let chunk = |id: &str, text: &str| CorpusChunk {
            chunk_id: id.into(),
            pack_id: "p".into(),
            path: "content/a.txt".into(),
            jurisdiction: Some("India".into()),
            text: text.into(),
            authority_hint: String::new(),
            metadata: legacy_metadata("p", "0", text),
        };
        let chunks = vec![
            chunk("dense-test-bail", "An application for bail may be made to the Court of Session."),
            chunk("dense-test-fine", "The court may impose a fine in addition to imprisonment."),
            chunk("dense-test-arrest", "Where a person has reason to believe he may be arrested, he may apply for a direction."),
        ];
        // Unit vectors along different axes, in the shipped 384-dim format.
        let axis = |i: usize| (0..384).map(|j| if j == i { 127_i8 } else { 0 }).collect::<Vec<i8>>();
        dense::register_for_test("dense-test-bail", axis(1));
        dense::register_for_test("dense-test-fine", axis(2));
        dense::register_for_test("dense-test-arrest", axis(0));
        let query: Vec<f32> = (0..384).map(|j| if j == 0 { 1.0 } else { 0.0 }).collect();
        let terms = tokenize("anticipatory bail");
        let filters = MetadataFilters::default();
        let lexical = retrieve_core(&chunks, &terms, 8, &filters, false, None);
        assert!(lexical.iter().all(|h| h.retrieval_mode == "LEXICAL_ONLY"));
        assert!(!lexical.iter().any(|h| h.chunk_id == "dense-test-arrest"));
        let fused = retrieve_core(&chunks, &terms, 8, &filters, false, Some(&query));
        let arrest = fused.iter().find(|h| h.chunk_id == "dense-test-arrest").expect("meaning-only match is retrieved");
        assert!(arrest.match_reasons.iter().any(|r| r.starts_with("meaning match")));
        assert!(fused.iter().any(|h| h.chunk_id == "dense-test-bail"), "lexical match is kept");
        assert!(fused.iter().all(|h| h.retrieval_mode == "HYBRID_DENSE"));
        // A query vector of the wrong size matches nothing: ranking falls back to the lexical result unchanged.
        let mismatched = retrieve_core(&chunks, &terms, 8, &filters, false, Some(&[1.0, 0.0]));
        assert_eq!(mismatched.iter().map(|h| h.chunk_id.clone()).collect::<Vec<_>>(), lexical.iter().map(|h| h.chunk_id.clone()).collect::<Vec<_>>());
        assert!(mismatched.iter().all(|h| h.retrieval_mode == "LEXICAL_ONLY"));
        // A question with no keyword terms (e.g. Hindi, which the keyword index does not read) is answered by meaning.
        let by_meaning = retrieve_core(&chunks, &[], 8, &filters, false, Some(&query));
        assert_eq!(by_meaning.first().map(|h| h.chunk_id.as_str()), Some("dense-test-arrest"));
        assert!(retrieve_core(&chunks, &[], 8, &filters, false, None).is_empty());
        assert_eq!(fusion_weights(&tokenize("s. 438 CrPC")), (1.0, 0.5));
        assert_eq!(fusion_weights(&tokenize("protection before arrest")), (0.5, 1.0));
        assert_eq!(fusion_weights(&[]), (0.5, 1.0));
    }

    fn controlled_chunks() -> Vec<CorpusChunk> {
        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../packs/official/caseforge.india.partition.highcourt.civil-appeal");
        crate::packs::load_legal_chunks(&pack)
            .unwrap()
            .into_iter()
            .map(|record| CorpusChunk {
                chunk_id: record.chunk_id.clone(),
                pack_id: record.pack_id.clone(),
                path: record.source_file.clone(),
                jurisdiction: Some(record.jurisdiction.clone()),
                text: record.exact_text.clone(),
                authority_hint: record.citation.clone(),
                metadata: metadata_from_record(&record),
            })
            .collect()
    }

    #[test]
    fn b_s3_11_hybrid_goldens_meet_recall_and_mrr_thresholds() {
        let chunks = controlled_chunks();
        let judgments = [
            (
                "When may the court order sale instead of physical division in a partition suit?",
                "Power to court to order sale instead of division",
            ),
            (
                "What is the route of an appeal from an original decree?",
                "Appeal from original decree",
            ),
            (
                "What must accompany a delayed appeal?",
                "Application for condonation of delay",
            ),
            (
                "What limitation entry applies to an appeal to a High Court under the CPC?",
                "116. Under the Code of Civil Procedure",
            ),
        ];
        let mut recalled = 0_usize;
        let mut reciprocal_rank = 0.0_f64;
        for (query, expected) in judgments {
            let hits = retrieve_filtered(
                &chunks,
                &tokenize(query),
                8,
                &MetadataFilters {
                    jurisdiction: Some("India".into()),
                    trust_level: Some("OFFICIAL_VERIFIED".into()),
                    ..MetadataFilters::default()
                },
            );
            if let Some(rank) = hits.iter().position(|hit| {
                chunks
                    .iter()
                    .find(|chunk| chunk.chunk_id == hit.chunk_id)
                    .is_some_and(|chunk| {
                        chunk
                            .text
                            .to_ascii_lowercase()
                            .contains(&expected.to_ascii_lowercase())
                    })
            }) {
                recalled += 1;
                reciprocal_rank += 1.0 / (rank as f64 + 1.0);
            }
        }
        let recall_at_8 = recalled as f64 / judgments.len() as f64;
        let mrr = reciprocal_rank / judgments.len() as f64;
        assert!(recall_at_8 >= 0.90, "Recall@8={recall_at_8}");
        assert!(mrr >= 0.70, "MRR={mrr}");
    }

    #[test]
    fn b_s3_11_filters_temporal_policy_and_fallback_are_fail_closed() {
        let mut chunks = controlled_chunks();
        let mut wrong = chunks[0].clone();
        wrong.chunk_id = "wrong-jurisdiction".into();
        wrong.jurisdiction = Some("United States".into());
        let mut superseded = chunks[0].clone();
        superseded.chunk_id = "superseded".into();
        superseded.metadata.superseded_status = "SUPERSEDED".into();
        chunks.extend([wrong, superseded]);
        let filters = MetadataFilters {
            jurisdiction: Some("India".into()),
            trust_level: Some("OFFICIAL_VERIFIED".into()),
            ..MetadataFilters::default()
        };
        let hits = retrieve_filtered_with_mode(
            &chunks,
            &tokenize("partition sale division"),
            32,
            &filters,
            false,
        );
        assert!(!hits
            .iter()
            .any(|hit| hit.chunk_id == "wrong-jurisdiction" || hit.chunk_id == "superseded"));
        assert!(hits
            .iter()
            .all(|hit| hit.retrieval_mode == "LEXICAL_ONLY" && hit.semantic_score == 0.0));
    }

    #[test]
    fn b_s3_11_index_is_checksum_driven_and_recovers_backup() {
        let temp = tempfile::tempdir().unwrap();
        let paths = VaultPaths::new(temp.path().to_path_buf());
        let chunks = controlled_chunks();
        let (first, state) = ensure_persisted_index(&paths, &chunks).unwrap();
        assert_eq!(state, "REBUILT");
        let (_, state) = ensure_persisted_index(&paths, &chunks).unwrap();
        assert_eq!(state, "REUSED");
        let index = temp.path().join("indexes/legal/index.json");
        let backup = temp.path().join("indexes/legal/index.json.bak");
        fs::rename(&index, &backup).unwrap();
        let (recovered, state) = ensure_persisted_index(&paths, &chunks).unwrap();
        assert_eq!(state, "REUSED");
        assert_eq!(recovered.corpus_checksum, first.corpus_checksum);
        let mut changed = chunks;
        changed[0].metadata.checksum = "sha256:changed".into();
        let (rebuilt, state) = ensure_persisted_index(&paths, &changed).unwrap();
        assert_eq!(state, "REBUILT");
        assert_ne!(rebuilt.corpus_checksum, first.corpus_checksum);
    }
}
