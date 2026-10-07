//! Versioned, host-owned legal retrieval for Mock Court (B-S3-15).

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub const SCHEMA_VERSION: &str = "caseforge.mock-court-retrieval.v1";
const MAX_AUTHORITIES: usize = 8;
const MAX_CACHE_ENTRIES: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MockCourtAuthority {
    pub authority_id: String,
    pub source_id: String,
    pub chunk_id: String,
    pub pack_id: String,
    pub title: String,
    pub citation: String,
    pub section_label: String,
    pub jurisdiction: String,
    pub authority_type: String,
    pub trust_level: String,
    pub temporal_status: String,
    pub superseded_status: String,
    pub source_reference: String,
    pub source_checksum: String,
    pub chunk_checksum: String,
    pub exact_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockCourtReadiness {
    pub schema_version: String,
    pub case_id: String,
    pub ready: bool,
    pub graph_available: bool,
    pub graph_version: String,
    pub graph_counsel_reviewed: bool,
    pub working_set_version: i64,
    pub regime_status: String,
    pub confirmed_regime_id: Option<String>,
    pub usable_authority_count: usize,
    pub authorities: Vec<MockCourtAuthority>,
    pub blocking_reasons: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GroundedTurnRequest {
    pub session_id: String,
    pub case_id: String,
    pub format: String,
    pub role: Option<String>,
    pub user_text: String,
    pub expected_graph_version: Option<String>,
    pub expected_working_set_version: Option<i64>,
    pub accept_version_change: Option<bool>,
    pub bench_archetype_id: Option<String>,
    pub bench_sliders: Option<crate::adaptive_bench::SliderSet>,
    pub judge_name: Option<String>,
    pub named_judge_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroundedTurnResponse {
    pub schema_version: String,
    pub turn: crate::mock_court::SessionTurn,
    pub graph_version: String,
    pub working_set_version: i64,
    pub retrieval_query: String,
    pub retrieval_cache_key: String,
    pub cache_hit: bool,
    pub authorities: Vec<MockCourtAuthority>,
    pub generation_mode: String,
    pub degraded_reason: Option<String>,
    pub model: String,
    pub backend: String,
    pub model_digest: String,
    pub prompt_version: String,
    pub prompt_hash: String,
    pub system_prompt_hash: String,
    pub model_elapsed_ms: u64,
    pub tokens_per_second: Option<f64>,
    pub schema_repair_used: bool,
    pub structured_turn: Option<crate::grounded_mock_court::StructuredTurnCandidate>,
    pub proposition_ledger: Option<crate::proposition::PropositionLedger>,
    /// B-S4-00b: deterministic feedback on counsel's answer to the previous challenge.
    pub answer_feedback: Option<crate::answer_feedback::AnswerFeedback>,
}

#[derive(Debug, Clone)]
pub struct RetrievalBundle {
    pub query: String,
    pub cache_key: String,
    pub cache_hit: bool,
    pub authorities: Vec<MockCourtAuthority>,
}

static CACHE: Lazy<Mutex<HashMap<String, Vec<MockCourtAuthority>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn digest(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    hex::encode(Sha256::digest(bytes))
}

pub fn graph_version(graph: Option<&Value>) -> String {
    graph.map(digest).unwrap_or_else(|| "missing".into())
}

fn nonempty(item: &Value, key: &str) -> Option<String> {
    item.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn jurisdiction_allowed(case_jurisdiction: &str, source_jurisdiction: &str) -> bool {
    let case = case_jurisdiction.trim().to_ascii_lowercase();
    let source = source_jurisdiction.trim().to_ascii_lowercase();
    !case.is_empty()
        && !source.is_empty()
        && (case == source
            || case.starts_with(&format!("{source} "))
            || case.starts_with(&format!("{source},"))
            || source.starts_with(&format!("{case} "))
            || source.starts_with(&format!("{case},")))
}

fn authority_from_item(
    item: &Value,
    case_jurisdiction: &str,
    usable: &HashSet<String>,
) -> Option<MockCourtAuthority> {
    let authority_id = nonempty(item, "authority_id")?;
    let permitted = usable.contains(&authority_id)
        && item.get("decision").and_then(Value::as_str) == Some("INCLUDED")
        && item.get("controlling").and_then(Value::as_bool) == Some(true)
        && item.get("trust_level").and_then(Value::as_str) == Some("OFFICIAL_VERIFIED")
        && item.get("temporal_status").and_then(Value::as_str) == Some("APPLICABLE")
        && item.get("superseded_status").and_then(Value::as_str) == Some("CURRENT")
        && jurisdiction_allowed(
            case_jurisdiction,
            item.get("jurisdiction")
                .and_then(Value::as_str)
                .unwrap_or(""),
        );
    if !permitted {
        return None;
    }
    Some(MockCourtAuthority {
        authority_id,
        source_id: nonempty(item, "source_id")?,
        chunk_id: nonempty(item, "chunk_id")?,
        pack_id: nonempty(item, "pack_id")?,
        title: nonempty(item, "title")?,
        citation: nonempty(item, "citation")?,
        section_label: nonempty(item, "section_label")?,
        jurisdiction: nonempty(item, "jurisdiction")?,
        authority_type: nonempty(item, "authority_type")?,
        trust_level: nonempty(item, "trust_level")?,
        temporal_status: nonempty(item, "temporal_status")?,
        superseded_status: nonempty(item, "superseded_status")?,
        source_reference: nonempty(item, "source_reference")?,
        source_checksum: nonempty(item, "source_checksum")?,
        chunk_checksum: nonempty(item, "chunk_checksum")?,
        exact_text: nonempty(item, "exact_text")?,
    })
}

fn graph_reviewed(graph: Option<&Value>) -> bool {
    let Some(graph) = graph else {
        return false;
    };
    let mut saw_provenance = false;
    for collection in ["nodes", "edges"] {
        for item in graph
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if let Some(reviewed) = item
                .pointer("/provenance/counsel_confirmed")
                .and_then(Value::as_bool)
            {
                saw_provenance = true;
                if !reviewed {
                    return false;
                }
            }
        }
    }
    // Legacy graphs cannot prove per-proposition counsel review, so surface a warning.
    saw_provenance
}

pub fn readiness(
    case_id: &str,
    case_jurisdiction: &str,
    graph: Option<&Value>,
    working_set: &Value,
) -> MockCourtReadiness {
    let graph_available = graph.is_some();
    let reviewed = graph_reviewed(graph);
    let working_set_version = working_set
        .get("version")
        .and_then(Value::as_i64)
        .unwrap_or(0);
    let regime_status = working_set
        .get("regime_status")
        .and_then(Value::as_str)
        .unwrap_or("PENDING_CONFIRMATION")
        .to_string();
    let confirmed_regime_id = working_set
        .get("confirmed_regime_id")
        .and_then(Value::as_str)
        .map(str::to_string);
    let usable: HashSet<String> = working_set
        .get("usable_authority_ids")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    let authorities: Vec<_> = working_set
        .get("items")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| authority_from_item(item, case_jurisdiction, &usable))
        .collect();
    let mut blocking_reasons = Vec::new();
    let mut warnings = Vec::new();
    if !graph_available {
        blocking_reasons.push("CF_MOCK_COURT_NO_GRAPH: run the case skeleton first".into());
    }
    if regime_status != "COUNSEL_CONFIRMED" {
        blocking_reasons.push(
            "CF_WORKING_SET_REGIME_UNCONFIRMED: counsel must confirm the applicable-law regime"
                .into(),
        );
    }
    if confirmed_regime_id
        .as_deref()
        .is_some_and(|id| id.ends_with("_REQUIRED"))
    {
        blocking_reasons.push(
            "CF_WORKING_SET_REGIME_SOURCE_GAP: required authoritative pack is not installed".into(),
        );
    }
    if authorities.is_empty() {
        blocking_reasons.push(
            "CF_MOCK_COURT_AUTHORITY_ABSENT: no permitted current authority for this jurisdiction"
                .into(),
        );
    }
    if graph_available && !reviewed {
        warnings.push("Case graph contains unconfirmed propositions; review them before relying on factual challenges.".into());
    }
    MockCourtReadiness {
        schema_version: SCHEMA_VERSION.into(),
        case_id: case_id.into(),
        ready: blocking_reasons.is_empty(),
        graph_available,
        graph_version: graph_version(graph),
        graph_counsel_reviewed: reviewed,
        working_set_version,
        regime_status,
        confirmed_regime_id,
        usable_authority_count: authorities.len(),
        authorities,
        blocking_reasons,
        warnings,
    }
}

fn push_graph_terms(parts: &mut Vec<String>, graph: &Value) {
    for collection in ["issues", "statutes", "prayer_relief", "gaps", "nodes"] {
        for item in graph
            .get(collection)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .take(12)
        {
            for key in ["label", "title", "issue", "relief", "section", "text"] {
                if let Some(value) = item
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|v| !v.is_empty())
                {
                    parts.push(value.to_string());
                }
            }
        }
    }
}

pub fn decompose_query(format: &str, role: &str, user_text: &str, graph: &Value) -> String {
    let mut parts = vec![format.replace('_', " "), role.to_string()];
    let user = user_text.trim();
    if !user.is_empty() {
        parts.push(user.to_string());
    }
    push_graph_terms(&mut parts, graph);
    let query = parts
        .join(" ")
        .split_whitespace()
        .take(160)
        .collect::<Vec<_>>()
        .join(" ");
    if query.is_empty() {
        "legal authority applicable relief procedure".into()
    } else {
        query
    }
}

fn tokens(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|s| s.len() > 2)
        .map(|s| s.to_ascii_lowercase())
        .collect()
}

pub fn retrieve(
    readiness: &MockCourtReadiness,
    format: &str,
    role: &str,
    user_text: &str,
    graph: &Value,
) -> Result<RetrievalBundle, String> {
    if !readiness.ready {
        return Err(format!(
            "CF_MOCK_COURT_NOT_READY: {}",
            readiness.blocking_reasons.join(" | ")
        ));
    }
    let query = decompose_query(format, role, user_text, graph);
    let cache_key = hex::encode(Sha256::digest(
        format!(
            "{}\0{}\0{}\0{}",
            readiness.case_id, readiness.graph_version, readiness.working_set_version, query
        )
        .as_bytes(),
    ));
    if let Ok(cache) = CACHE.lock() {
        if let Some(authorities) = cache.get(&cache_key) {
            return Ok(RetrievalBundle {
                query,
                cache_key,
                cache_hit: true,
                authorities: authorities.clone(),
            });
        }
    }
    let query_tokens = tokens(&query);
    let mut ranked: Vec<(usize, MockCourtAuthority)> = readiness
        .authorities
        .iter()
        .cloned()
        .map(|a| {
            let haystack = format!(
                "{} {} {} {}",
                a.title, a.citation, a.section_label, a.exact_text
            );
            let score = tokens(&haystack).intersection(&query_tokens).count();
            (score, a)
        })
        .collect();
    ranked.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1.authority_id.cmp(&b.1.authority_id))
    });
    let authorities: Vec<_> = ranked
        .into_iter()
        .take(MAX_AUTHORITIES)
        .map(|(_, a)| a)
        .collect();
    if authorities.is_empty() {
        return Err(
            "CF_MOCK_COURT_AUTHORITY_ABSENT: legal turn cannot run with empty retrieval".into(),
        );
    }
    if let Ok(mut cache) = CACHE.lock() {
        if cache.len() >= MAX_CACHE_ENTRIES {
            cache.clear();
        }
        cache.insert(cache_key.clone(), authorities.clone());
    }
    Ok(RetrievalBundle {
        query,
        cache_key,
        cache_hit: false,
        authorities,
    })
}

pub fn enforce_versions(
    request: &GroundedTurnRequest,
    readiness: &MockCourtReadiness,
) -> Result<(), String> {
    if request.accept_version_change.unwrap_or(false) {
        return Ok(());
    }
    let graph_stale = request
        .expected_graph_version
        .as_deref()
        .is_some_and(|v| v != readiness.graph_version);
    let set_stale = request
        .expected_working_set_version
        .is_some_and(|v| v != readiness.working_set_version);
    if graph_stale || set_stale {
        return Err(format!("CF_MOCK_COURT_SESSION_STALE: graph={} working_set={}; explicitly resume against current versions", readiness.graph_version, readiness.working_set_version));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn item(id: &str, jurisdiction: &str) -> Value {
        json!({
            "authority_id":id,"source_id":"source-1","chunk_id":format!("chunk-{id}"),"pack_id":"pack-1",
            "title":"Partition Act","citation":"Act IV of 1893","section_label":"Section 2","jurisdiction":jurisdiction,
            "authority_type":"statute","trust_level":"OFFICIAL_VERIFIED","temporal_status":"APPLICABLE","superseded_status":"CURRENT",
            "source_reference":"official://act/2","source_checksum":"abc","chunk_checksum":"def","exact_text":"The Court may direct a sale.",
            "decision":"INCLUDED","controlling":true
        })
    }
    fn set(items: Vec<Value>, ids: Vec<&str>) -> Value {
        json!({"version":3,"regime_status":"COUNSEL_CONFIRMED","confirmed_regime_id":"CENTRAL_CIVIL_PROCEDURE","usable_authority_ids":ids,"items":items})
    }
    fn graph() -> Value {
        json!({"schema_version":"1.0.0","issues":[{"issue":"partition by sale"}]})
    }

    #[test]
    fn excludes_wrong_jurisdiction_and_noncurrent_sources() {
        let mut superseded = item("old", "India");
        superseded["superseded_status"] = json!("SUPERSEDED");
        let mut removed = item("removed", "India");
        removed["decision"] = json!("REMOVED");
        let mut red = item("red", "India");
        red["trust_level"] = json!("RED");
        let ws = set(
            vec![
                item("good", "India"),
                item("wrong", "United States"),
                superseded,
                removed,
                red,
            ],
            vec!["good", "wrong", "old", "removed", "red"],
        );
        let ready = readiness("case-1", "India", Some(&graph()), &ws);
        assert!(ready.ready);
        assert_eq!(ready.authorities.len(), 1);
        assert_eq!(ready.authorities[0].authority_id, "good");
    }

    #[test]
    fn blocks_unconfirmed_or_empty_working_set() {
        let ws = json!({"version":1,"regime_status":"PENDING_CONFIRMATION","usable_authority_ids":[],"items":[]});
        let ready = readiness("case-1", "India", Some(&graph()), &ws);
        assert!(!ready.ready);
        assert!(ready
            .blocking_reasons
            .iter()
            .any(|r| r.contains("REGIME_UNCONFIRMED")));
        assert!(ready
            .blocking_reasons
            .iter()
            .any(|r| r.contains("AUTHORITY_ABSENT")));
    }

    #[test]
    fn version_or_query_change_invalidates_cache_key() {
        let ws = set(vec![item("good", "India")], vec!["good"]);
        let g = graph();
        let ready = readiness("case-1", "India", Some(&g), &ws);
        let a = retrieve(&ready, "Appellate", "adversary", "limitation", &g).unwrap();
        let b = retrieve(&ready, "Appellate", "adversary", "title", &g).unwrap();
        assert_ne!(a.cache_key, b.cache_key);
        let mut newer = ready.clone();
        newer.working_set_version += 1;
        let c = retrieve(&newer, "Appellate", "adversary", "limitation", &g).unwrap();
        assert_ne!(a.cache_key, c.cache_key);
    }

    #[test]
    fn stale_session_requires_explicit_resume() {
        let ws = set(vec![item("good", "India")], vec!["good"]);
        let ready = readiness("case-1", "India", Some(&graph()), &ws);
        let req = GroundedTurnRequest {
            session_id: "s".into(),
            case_id: "case-1".into(),
            format: "Appellate".into(),
            role: None,
            user_text: "x".into(),
            expected_graph_version: Some("old".into()),
            expected_working_set_version: Some(3),
            accept_version_change: None,
            bench_archetype_id: None,
            bench_sliders: None,
            judge_name: None,
            named_judge_id: None,
        };
        assert!(enforce_versions(&req, &ready)
            .unwrap_err()
            .contains("SESSION_STALE"));
    }
}
