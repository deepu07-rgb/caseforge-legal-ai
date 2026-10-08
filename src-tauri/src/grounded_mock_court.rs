//! Verified local-model Mock Court adapter (B-S3-16).

use crate::infer_llm::{LlmChatRequest, LlmMessage};
use crate::mock_court::SessionTurn;
use crate::mock_court_retrieval::{MockCourtAuthority, MockCourtReadiness, RetrievalBundle};
use crate::proposition::{PropositionLedger, SupportSource, VerifyRequest};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

pub const SCHEMA_VERSION: &str = "caseforge.mock-court-turn.v2";
pub const PROMPT_VERSION: &str = "caseforge.mock-court.v2@2.0.0";
pub const TURN_TIMEOUT_SECONDS: u64 = 15;
const MAX_HISTORY_MESSAGES: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceGroup {
    pub id: String,
    pub proposition_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredTurnCandidate {
    pub schema_version: String,
    pub challenges: Vec<ReferenceGroup>,
    pub propositions: Vec<crate::proposition::PropositionInput>,
    pub unanswered_questions: Vec<ReferenceGroup>,
    pub next_focus: ReferenceGroup,
}

#[derive(Debug, Clone, Serialize)]
pub struct GroundedModelResult {
    pub turn: SessionTurn,
    pub generation_mode: String,
    pub degraded_reason: Option<String>,
    pub model: String,
    pub backend: String,
    pub model_digest: String,
    pub prompt_version: String,
    pub prompt_hash: String,
    pub system_prompt_hash: String,
    pub elapsed_ms: u64,
    pub tokens_per_second: Option<f64>,
    pub schema_repair_used: bool,
    pub structured_turn: Option<StructuredTurnCandidate>,
    pub proposition_ledger: Option<PropositionLedger>,
}

static HISTORY: Lazy<Mutex<HashMap<String, Vec<LlmMessage>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) fn support_sources(
    case_id: &str,
    graph: &Value,
    graph_version: &str,
    user_text: &str,
    authorities: &[MockCourtAuthority],
) -> Vec<SupportSource> {
    let mut sources = vec![SupportSource {
        support_id: format!("matter-graph:{case_id}:{graph_version}"),
        kind: "MATTER_SOURCE".into(),
        exact_text: serde_json::to_string(graph).unwrap_or_default(),
        source_id: Some(format!("case_graph:{graph_version}")),
        chunk_id: None,
        authority_id: None,
        doc_id: Some(format!("case_graph:{case_id}")),
        page: None,
        paragraph: None,
        trust_level: None,
        temporal_status: None,
        superseded_status: None,
        permitted: true,
    }];
    sources.push(SupportSource {
        support_id: format!("counsel-turn:{case_id}"),
        kind: "COUNSEL_INPUT".into(),
        exact_text: user_text.into(),
        source_id: None,
        chunk_id: None,
        authority_id: None,
        doc_id: None,
        page: None,
        paragraph: None,
        trust_level: None,
        temporal_status: None,
        superseded_status: None,
        permitted: true,
    });
    sources.extend(authorities.iter().map(|a| SupportSource {
        support_id: format!("legal:{}:{}", a.authority_id, a.chunk_id),
        kind: "LEGAL_AUTHORITY".into(),
        exact_text: a.exact_text.clone(),
        source_id: Some(a.source_id.clone()),
        chunk_id: Some(a.chunk_id.clone()),
        authority_id: Some(a.authority_id.clone()),
        doc_id: None,
        page: None,
        paragraph: Some(a.section_label.clone()),
        trust_level: Some(a.trust_level.clone()),
        temporal_status: Some(a.temporal_status.clone()),
        superseded_status: Some(a.superseded_status.clone()),
        permitted: true,
    }));
    sources
}

fn validate_references(candidate: &StructuredTurnCandidate) -> Result<(), String> {
    if candidate.schema_version != SCHEMA_VERSION
        || candidate.challenges.is_empty()
        || candidate.challenges.len() > 6
        || candidate.propositions.is_empty()
        || candidate.propositions.len() > 40
        || candidate.unanswered_questions.len() > 6
    {
        return Err("CF_MOCK_COURT_V2_SCHEMA: invalid structured turn bounds".into());
    }
    let ids: HashSet<&str> = candidate
        .propositions
        .iter()
        .map(|p| p.proposition_id.as_str())
        .collect();
    if ids.len() != candidate.propositions.len() {
        return Err("CF_MOCK_COURT_V2_SCHEMA: duplicate proposition IDs".into());
    }
    for group in candidate
        .challenges
        .iter()
        .chain(candidate.unanswered_questions.iter())
        .chain(std::iter::once(&candidate.next_focus))
    {
        if group.id.trim().is_empty()
            || group.proposition_ids.is_empty()
            || group.proposition_ids.len() > 8
            || group
                .proposition_ids
                .iter()
                .any(|id| !ids.contains(id.as_str()))
        {
            return Err(
                "CF_MOCK_COURT_V2_SCHEMA: reference group contains missing proposition IDs".into(),
            );
        }
    }
    Ok(())
}

fn safe_prop_text(ledger: &PropositionLedger, id: &str) -> Option<String> {
    let prop = ledger
        .propositions
        .iter()
        .find(|p| p.proposition_id == id)?;
    match prop.render_action.as_str() {
        "TRUSTED" => Some(format!(
            "{} [VERIFIED: {}]",
            prop.text,
            prop.verified_support_ids.join(", ")
        )),
        "LABELLED" if prop.class == "INFERENCE" => Some(format!(
            "[INFERENCE — bases: {}] {}",
            prop.basis_ids.join(", "),
            prop.text
        )),
        "LABELLED" => Some(format!("[COUNSEL INPUT] {}", prop.text)),
        "REPOSITORY_GAP" => Some(format!("[REPOSITORY GAP — NOT VERIFIED] {}", prop.text)),
        _ => None,
    }
}

pub fn render_verified(
    candidate: &StructuredTurnCandidate,
    ledger: &PropositionLedger,
) -> Result<String, String> {
    validate_references(candidate)?;
    let mut sections = Vec::new();
    let mut rendered_challenges = 0usize;
    for (index, group) in candidate.challenges.iter().enumerate() {
        let lines: Vec<_> = group
            .proposition_ids
            .iter()
            .filter_map(|id| safe_prop_text(ledger, id))
            .collect();
        if !lines.is_empty() {
            rendered_challenges += 1;
            sections.push(format!("Challenge {}\n{}", index + 1, lines.join("\n")));
        }
    }
    if rendered_challenges == 0 {
        return Err("CF_MOCK_COURT_VERIFIER_EMPTY: every proposed challenge was blocked".into());
    }
    let unanswered: Vec<_> = candidate
        .unanswered_questions
        .iter()
        .flat_map(|group| group.proposition_ids.iter())
        .filter_map(|id| safe_prop_text(ledger, id))
        .collect();
    if !unanswered.is_empty() {
        sections.push(format!("Unanswered questions\n{}", unanswered.join("\n")));
    }
    let focus: Vec<_> = candidate
        .next_focus
        .proposition_ids
        .iter()
        .filter_map(|id| safe_prop_text(ledger, id))
        .collect();
    if !focus.is_empty() {
        sections.push(format!("Next focus\n{}", focus.join("\n")));
    }
    sections.push("Preparation / rehearsal only. No verdict or outcome prediction.".into());
    Ok(sections.join("\n\n"))
}

fn history_for(session_id: &str) -> Vec<LlmMessage> {
    HISTORY
        .lock()
        .ok()
        .and_then(|history| history.get(session_id).cloned())
        .unwrap_or_default()
        .into_iter()
        .rev()
        .take(MAX_HISTORY_MESSAGES)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn store_history(session_id: &str, user_text: &str, assistant_text: &str) {
    if let Ok(mut history) = HISTORY.lock() {
        if history.len() > 64 && !history.contains_key(session_id) {
            history.clear();
        }
        let messages = history.entry(session_id.into()).or_default();
        messages.push(LlmMessage {
            role: "user".into(),
            content: user_text.into(),
        });
        messages.push(LlmMessage {
            role: "assistant".into(),
            content: assistant_text.into(),
        });
        if messages.len() > MAX_HISTORY_MESSAGES {
            messages.drain(0..messages.len() - MAX_HISTORY_MESSAGES);
        }
    }
}

pub fn run(
    profile: &crate::vault::CaseSummary,
    graph: &Value,
    readiness: &MockCourtReadiness,
    retrieval: &RetrievalBundle,
    user_text: &str,
    session_id: &str,
    format: &str,
    role: &str,
    mut baseline: SessionTurn,
) -> Result<GroundedModelResult, String> {
    let status = crate::infer_llm::status();
    if !status.controlled_beta_ready {
        return Err(format!(
            "CF_MOCK_COURT_MODEL_NOT_ALLOWLISTED: {}",
            status.detail
        ));
    }
    let input_guard = crate::guardrails::check_user_input(user_text);
    if input_guard.refused {
        return Err("CF_MOCK_COURT_INPUT_REFUSED: safety policy blocked the turn".into());
    }
    let sources = support_sources(
        &profile.case_id,
        graph,
        &readiness.graph_version,
        user_text,
        &retrieval.authorities,
    );
    let permitted: Vec<String> = retrieval
        .authorities
        .iter()
        .map(|a| a.authority_id.clone())
        .collect();
    let authority_context = retrieval
        .authorities
        .iter()
        .map(|a| {
            format!(
                "[{} | {} | {}] {}",
                a.authority_id, a.citation, a.section_label, a.exact_text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let request = LlmChatRequest {
        user_text: user_text.into(),
        role: Some(role.into()),
        format: Some(format.into()),
        case_title: Some(profile.cause_title.clone()),
        jurisdiction: Some(profile.jurisdiction.clone()),
        user_side: Some(profile.user_side.clone()),
        case_context: None,
        authority_context: Some(authority_context),
        authority_ids: Some(permitted.clone()),
        working_set_version: Some(readiness.working_set_version.to_string()),
        structured_propositions: Some(true),
        output_kind: Some("MOCK_COURT".into()),
        support_sources: Some(sources.clone()),
        permitted_authority_ids: Some(permitted.clone()),
        mock_court_schema_v2: Some(true),
        messages: Some(history_for(session_id)),
        max_tokens: Some(112),
        variant: None,
    };
    let generation_started = Instant::now();
    let mut model = crate::infer_llm::chat_with_timeout(request.clone(), TURN_TIMEOUT_SECONDS)?;
    let mut repair_used = false;
    if model.mock_court_turn.is_none() {
        repair_used = true;
        let mut repair = request;
        let mut messages = repair.messages.take().unwrap_or_default();
        messages.push(LlmMessage {
            role: "assistant".into(),
            content: model.content.clone(),
        });
        repair.user_text = "SCHEMA_REPAIR_ONLY: Return the same answer as one valid caseforge.mock-court-turn.v2 JSON object. Do not add facts, law, sources, or prose.".into();
        repair.messages = Some(messages);
        let elapsed = generation_started.elapsed().as_secs();
        if elapsed >= TURN_TIMEOUT_SECONDS {
            return Err(
                "CF_LLM_TIMEOUT: no time remains for schema repair; degraded safely".into(),
            );
        }
        model = crate::infer_llm::chat_with_timeout(repair, TURN_TIMEOUT_SECONDS - elapsed)?;
    }
    if !model.controlled_beta_ready {
        return Err(
            "CF_MOCK_COURT_MODEL_DRIFT: generation did not report controlled-beta readiness".into(),
        );
    }
    let candidate: StructuredTurnCandidate =
        serde_json::from_value(model.mock_court_turn.clone().ok_or_else(|| {
            "CF_MOCK_COURT_V2_SCHEMA: model failed the single schema-repair attempt".to_string()
        })?)
        .map_err(|e| format!("CF_MOCK_COURT_V2_SCHEMA: {e}"))?;
    validate_references(&candidate)?;
    let ledger = crate::proposition::verify(VerifyRequest {
        output_kind: "MOCK_COURT".into(),
        propositions: candidate.propositions.clone(),
        support_sources: sources,
        permitted_authority_ids: permitted,
    })?;
    let rendered = render_verified(&candidate, &ledger)?;
    let guarded = crate::guardrails::check_model_output(user_text, &rendered, &model.model, &[]);
    if guarded.refused {
        return Err(
            "CF_MOCK_COURT_OUTPUT_REFUSED: verified model turn still failed safety policy".into(),
        );
    }
    let prompt_hash = model
        .provenance
        .get("prompt_hash")
        .and_then(Value::as_str)
        .unwrap_or("unverified")
        .to_string();
    let system_prompt_hash = model
        .provenance
        .get("system_prompt_hash")
        .and_then(Value::as_str)
        .unwrap_or("unverified")
        .to_string();
    let model_digest = model
        .provenance
        .get("model_digest")
        .and_then(Value::as_str)
        .unwrap_or("unverified")
        .to_string();
    baseline.schema_version = "2.0.0".into();
    baseline.assistant_text = Some(guarded.content.clone());
    baseline.model_hash = model_digest.clone();
    baseline.prompt_hash = prompt_hash.clone();
    baseline.elapsed_ms = model.elapsed_ms;
    store_history(session_id, user_text, &guarded.content);
    Ok(GroundedModelResult {
        turn: baseline,
        generation_mode: "LOCAL_LLM_VERIFIED".into(),
        degraded_reason: None,
        model: model.model,
        backend: model.backend,
        model_digest,
        prompt_version: PROMPT_VERSION.into(),
        prompt_hash,
        system_prompt_hash,
        elapsed_ms: model.elapsed_ms,
        tokens_per_second: model.tokens_per_second,
        schema_repair_used: repair_used,
        structured_turn: Some(candidate),
        proposition_ledger: Some(ledger),
    })
}

pub fn degraded(mut baseline: SessionTurn, reason: String) -> GroundedModelResult {
    if let Some(text) = baseline.assistant_text.take() {
        baseline.assistant_text = Some(format!("[DEGRADED DETERMINISTIC REHEARSAL — NOT AI ADVERSARIAL ANALYSIS]\nReason: {reason}\n\n{text}"));
    }
    GroundedModelResult {
        turn: baseline,
        generation_mode: "DEGRADED_DETERMINISTIC".into(),
        degraded_reason: Some(reason),
        model: "none".into(),
        backend: "deterministic-template".into(),
        model_digest: "deterministic".into(),
        prompt_version: "caseforge.mock-court.deterministic.v1".into(),
        prompt_hash: "deterministic:template".into(),
        system_prompt_hash: "deterministic:template".into(),
        elapsed_ms: 0,
        tokens_per_second: None,
        schema_repair_used: false,
        structured_turn: None,
        proposition_ledger: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposition::{PropositionInput, SupportClaim, VerifiedProposition};

    fn candidate() -> StructuredTurnCandidate {
        StructuredTurnCandidate {
            schema_version: SCHEMA_VERSION.into(),
            challenges: vec![ReferenceGroup {
                id: "c1".into(),
                proposition_ids: vec!["p1".into(), "bad".into()],
            }],
            propositions: vec![],
            unanswered_questions: vec![],
            next_focus: ReferenceGroup {
                id: "focus".into(),
                proposition_ids: vec!["p1".into()],
            },
        }
    }
    fn verified(id: &str, action: &str) -> VerifiedProposition {
        VerifiedProposition {
            proposition_id: id.into(),
            class: "LEGAL_BASIS".into(),
            text: format!("text-{id}"),
            support_ids: vec![],
            basis_ids: vec![],
            support_claims: Vec::<SupportClaim>::new(),
            quotation: None,
            verification_status: if action == "TRUSTED" { "GREEN" } else { "RED" }.into(),
            render_action: action.into(),
            verified_support_ids: vec!["legal:a:c".into()],
            rejected_support_ids: vec![],
            quote_offsets: None,
            reason: "test".into(),
        }
    }
    #[test]
    fn blocked_model_proposition_never_renders() {
        let mut c = candidate();
        c.propositions = vec![
            PropositionInput {
                proposition_id: "p1".into(),
                class: "LEGAL_BASIS".into(),
                text: "ok".into(),
                support_ids: vec![],
                basis_ids: vec![],
                support_claims: vec![],
                quotation: None,
            },
            PropositionInput {
                proposition_id: "bad".into(),
                class: "LEGAL_BASIS".into(),
                text: "invented holding".into(),
                support_ids: vec![],
                basis_ids: vec![],
                support_claims: vec![],
                quotation: None,
            },
        ];
        let ledger = PropositionLedger {
            schema_version: "x".into(),
            batch: "x".into(),
            output_kind: "MOCK_COURT".into(),
            propositions: vec![verified("p1", "TRUSTED"), verified("bad", "BLOCKED")],
            rendered_text: "".into(),
            trusted_count: 1,
            gap_count: 0,
            blocked_count: 1,
            all_legal_grounded: false,
            all_quotes_exact: true,
        };
        let out = render_verified(&c, &ledger).unwrap();
        assert!(out.contains("text-p1"));
        assert!(!out.contains("invented holding"));
    }
    #[test]
    fn missing_reference_is_rejected() {
        let mut c = candidate();
        c.propositions = vec![PropositionInput {
            proposition_id: "p1".into(),
            class: "UNSUPPORTED_OR_GAP".into(),
            text: "gap".into(),
            support_ids: vec![],
            basis_ids: vec![],
            support_claims: vec![],
            quotation: None,
        }];
        assert!(validate_references(&c).is_err());
    }
}
