//! Matter-specific, counsel-controlled legal working set (B-S3-12).

use crate::retrieval::CorpusChunk;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const DECISIONS: &[&str] = &[
    "PROPOSED",
    "INCLUDED",
    "EXCLUDED",
    "IRRELEVANT",
    "REMOVED",
    "SUPERSEDED",
];
pub const OUTPUT_KINDS: &[&str] = &["DIGEST", "DRAFTING", "ARGUMENT_INTEGRITY", "MOCK_COURT", "REASONING"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeCandidate {
    pub regime_id: String,
    pub label: String,
    pub reason: String,
    pub confirmation_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingSetItemInput {
    pub item_id: String,
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
    pub source_path: String,
    pub source_reference: String,
    pub source_checksum: String,
    pub chunk_checksum: String,
    pub exact_text: String,
    pub last_seen_index_checksum: String,
}

pub fn item_from_chunk(chunk: &CorpusChunk, index_checksum: &str) -> WorkingSetItemInput {
    let identity = format!(
        "{}|{}|{}|{}",
        chunk.jurisdiction.as_deref().unwrap_or("UNKNOWN"),
        chunk.metadata.title,
        chunk.metadata.citation,
        chunk.metadata.section
    )
    .to_ascii_lowercase();
    let authority_id = format!(
        "cfauth:{}",
        hex::encode(Sha256::digest(identity.as_bytes()))
    );
    WorkingSetItemInput {
        item_id: format!("ws:{authority_id}"),
        authority_id,
        source_id: chunk.metadata.source_id.clone(),
        chunk_id: chunk.chunk_id.clone(),
        pack_id: chunk.pack_id.clone(),
        title: chunk.metadata.title.clone(),
        citation: chunk.metadata.citation.clone(),
        section_label: chunk.metadata.section.clone(),
        jurisdiction: chunk
            .jurisdiction
            .clone()
            .unwrap_or_else(|| "UNKNOWN".into()),
        authority_type: chunk.metadata.authority_type.clone(),
        trust_level: chunk.metadata.trust_level.clone(),
        temporal_status: chunk.metadata.temporal_status.clone(),
        superseded_status: chunk.metadata.superseded_status.clone(),
        source_path: chunk.path.clone(),
        source_reference: chunk.metadata.source_reference.clone(),
        source_checksum: chunk.metadata.source_checksum.clone(),
        chunk_checksum: chunk.metadata.checksum.clone(),
        exact_text: chunk.text.clone(),
        last_seen_index_checksum: index_checksum.into(),
    }
}

pub fn suggestion_query(
    profile: &crate::vault::CaseSummary,
    graph: Option<&serde_json::Value>,
) -> String {
    let mut parts = vec![
        profile.jurisdiction.clone(),
        profile.court.clone(),
        profile.matter_type.clone(),
        profile.code_era.clone(),
    ];
    if let Some(graph) = graph {
        for collection in ["statutes", "issues", "prayer_relief"] {
            if let Some(items) = graph.get(collection).and_then(|v| v.as_array()) {
                for item in items.iter().take(12) {
                    for field in ["label", "title", "issue", "relief", "section"] {
                        if let Some(value) = item.get(field).and_then(|v| v.as_str()) {
                            parts.push(value.into());
                        }
                    }
                }
            }
        }
    }
    parts.join(" ")
}

pub fn regime_candidates(
    profile: &crate::vault::CaseSummary,
    graph: Option<&serde_json::Value>,
) -> Vec<RegimeCandidate> {
    let context = format!(
        "{} {} {}",
        profile.court,
        profile.matter_type,
        graph.map(|g| g.to_string()).unwrap_or_default()
    )
    .to_ascii_lowercase();
    let mut out = vec![RegimeCandidate {
        regime_id: "CENTRAL_CIVIL_PROCEDURE".into(),
        label: "Central civil procedure and general partition law".into(),
        reason: "CPC, Limitation Act, and Partition Act framework for the selected civil appeal."
            .into(),
        confirmation_required: true,
    }];
    if context.contains("partition") || context.contains("property") {
        out.push(RegimeCandidate {
            regime_id: "PERSONAL_OR_SUCCESSION_LAW_REQUIRED".into(),
            label: "Personal/succession entitlement law may also govern shares".into(),
            reason: "A property partition can depend on succession, coparcenary, religion, or title facts not established by the generic matter profile.".into(),
            confirmation_required: true,
        });
    }
    if context.contains("agricultural")
        || context.contains("revenue land")
        || context.contains("tenancy")
    {
        out.push(RegimeCandidate {
            regime_id: "STATE_LAND_REVENUE_LAW_REQUIRED".into(),
            label: "State land/revenue/tenancy law required".into(),
            reason: "The matter context indicates land governed by state-specific legislation outside the controlled pack.".into(),
            confirmation_required: true,
        });
    }
    out
}

pub fn policy() -> serde_json::Value {
    serde_json::json!({
        "batch": "B-S3-12", "schema_version": SCHEMA_VERSION,
        "decisions": DECISIONS, "output_kinds": OUTPUT_KINDS,
        "confirmation_semantics": "PROPOSED is never controlling. Only explicit counsel INCLUDED plus controlling=true is controlling; ambiguous regime confirmation remains separately visible.",
        "reentry_policy": "EXCLUDED, IRRELEVANT, REMOVED, and SUPERSEDED decisions are never overwritten by suggestion/rerun",
        "text_policy": "Exact source chunk is retained only with stable IDs, checksums, pack, path, and source reference"
    })
}

/// Codes that govern criminal proceedings only. (Evidence law is not listed: it serves civil and criminal matters alike.)
const CRIMINAL_ONLY_TITLES: &[&str] = &["nyaya sanhita", "nagarik suraksha", "indian penal code", "code of criminal procedure"];
const CRIMINAL_MATTER_WORDS: &[&str] = &["criminal", "bail", "sessions", "offence", "accused", "magistrate", "prosecution", "fir ", "habeas", "anticipatory"];
const CIVIL_MATTER_WORDS: &[&str] = &["civil", "partition", "property", "contract", "suit", "decree", "tenancy", "commercial", "probate", "injunction", "specific performance", "money recovery", "succession"];

/// Why a suggested authority does not fit this matter, or None when it fits or we cannot tell. Found by running the real app: for a civil
/// partition appeal the suggestions led with sections of the criminal procedure code. A suggestion is only ever a proposal, and counsel can
/// still search for and add anything by hand, so a clear mismatch is left out of the list and *named* beside it, never silently dropped.
pub fn misfit(profile: &crate::vault::CaseSummary, graph: Option<&serde_json::Value>, title: &str) -> Option<String> {
    let nature = format!("{} {} {}", profile.matter_type, profile.cause_title, profile.court).to_ascii_lowercase();
    let civil = CIVIL_MATTER_WORDS.iter().any(|w| nature.contains(w));
    let criminal = CRIMINAL_MATTER_WORDS.iter().any(|w| nature.contains(w));
    let t = title.to_ascii_lowercase();
    if !(civil && !criminal) || !CRIMINAL_ONLY_TITLES.iter().any(|c| t.contains(c)) {
        return None;
    }
    // the matter's own papers cite a criminal code: then it is relevant after all
    let cited = graph
        .map(|g| g["statutes"].to_string().to_ascii_lowercase())
        .unwrap_or_default();
    if CRIMINAL_ONLY_TITLES.iter().any(|c| cited.contains(c)) || ["ipc", "crpc", "bnss", "bns "].iter().any(|c| cited.contains(c)) {
        return None;
    }
    Some("a criminal code, and this is a civil matter whose papers cite none".into())
}

#[cfg(test)]
mod misfit_tests {
    use super::*;
    fn profile(matter: &str, title: &str) -> crate::vault::CaseSummary {
        crate::vault::CaseSummary { case_id: "c".into(), created_at_unix_ms: 0, label: "l".into(), cause_title: title.into(), court: "High Court".into(), jurisdiction: "India".into(), matter_type: matter.into(), user_side: "Appellant".into(), code_era: "".into(), language: "en".into(), is_open: true }
    }
    #[test]
    fn a_civil_matter_is_not_offered_criminal_codes_but_a_criminal_one_is() {
        let civil = profile("Civil", "Ravi v. Suresh (partition appeal)");
        assert!(misfit(&civil, None, "The Bharatiya Nagarik Suraksha Sanhita, 2023").is_some());
        assert!(misfit(&civil, None, "The Bharatiya Nyaya Sanhita, 2023").is_some());
        assert!(misfit(&civil, None, "The Limitation Act, 1963").is_none(), "civil law is offered");
        assert!(misfit(&civil, None, "The Bharatiya Sakshya Adhiniyam, 2023").is_none(), "evidence law serves both");
        let criminal = profile("Criminal", "State v. Mohan");
        assert!(misfit(&criminal, None, "The Bharatiya Nagarik Suraksha Sanhita, 2023").is_none());
        let mixed = profile("Civil and criminal complaint", "Acme v. Northern");
        assert!(misfit(&mixed, None, "The Bharatiya Nyaya Sanhita, 2023").is_none(), "when the matter says both, nothing is left out");
    }
    #[test]
    fn a_civil_matter_whose_papers_cite_a_criminal_code_still_gets_it() {
        let civil = profile("Civil", "Acme v. Northern");
        let graph = serde_json::json!({"statutes": [{"label": "Section 420 of the Indian Penal Code"}]});
        assert!(misfit(&civil, Some(&graph), "The Bharatiya Nyaya Sanhita, 2023").is_none());
        let graph = serde_json::json!({"statutes": [{"label": "Article 116 of the Limitation Act, 1963"}]});
        assert!(misfit(&civil, Some(&graph), "The Bharatiya Nyaya Sanhita, 2023").is_some());
    }
    #[test]
    fn an_unclear_matter_is_never_filtered() {
        assert!(misfit(&profile("", "A v. B"), None, "The Bharatiya Nyaya Sanhita, 2023").is_none());
    }
}
