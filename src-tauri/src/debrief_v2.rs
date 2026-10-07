//! B-S3-18 source-specific Mock Court debrief and preparation tasks.
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const SCHEMA_VERSION: &str = "caseforge.mock-court-debrief.v2";
pub const TASK_SCHEMA_VERSION: &str = "caseforge.preparation-task.v1";
pub const RUBRIC_VERSION: &str = "caseforge.debrief-usefulness.v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebriefTurnInput {
    pub turn_id: String,
    pub counsel_text: String,
    pub assistant_text: String,
    pub proposition_ledger: Option<crate::proposition::PropositionLedger>,
    #[serde(default)]
    pub authorities: Vec<crate::mock_court_retrieval::MockCourtAuthority>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub case_id: String,
    pub session_id: String,
    pub format: String,
    pub turns: Vec<DebriefTurnInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub source_kind: String,
    pub stable_id: String,
    pub locator: String,
    pub title: String,
    pub excerpt: Option<String>,
    pub verification_status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub finding_id: String,
    pub category: String,
    pub title: String,
    pub detail: String,
    pub turn_ids: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub epistemic_status: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparationTask {
    pub task_id: String,
    pub debrief_id: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub turn_ids: Vec<String>,
    pub finding_ids: Vec<String>,
    pub source_refs: Vec<SourceRef>,
    pub epistemic_status: String,
    pub usefulness_score: u8,
    pub counsel_note: Option<String>,
    pub updated_at_unix_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionComparison {
    pub previous_debrief_id: Option<String>,
    pub observed_change: String,
    pub supported_rate_previous: Option<f64>,
    pub supported_rate_current: f64,
    pub outcome_prediction: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebriefReportV2 {
    pub schema_version: String,
    pub debrief_id: String,
    pub case_id: String,
    pub session_id: String,
    pub format: String,
    pub created_at_unix_ms: i64,
    pub turns_completed: usize,
    pub findings: Vec<Finding>,
    pub tasks: Vec<PreparationTask>,
    pub comparison: SessionComparison,
    pub source_fingerprint: String,
    pub rubric_version: String,
    pub generic_tasks_rejected: usize,
    pub disclaimer: String,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn legal_ref(a: &crate::mock_court_retrieval::MockCourtAuthority) -> SourceRef {
    SourceRef {
        source_kind: "LEGAL_AUTHORITY".into(),
        stable_id: a.chunk_id.clone(),
        locator: format!(
            "legal://{}/{}/{}#{}",
            a.pack_id, a.source_id, a.chunk_id, a.section_label
        ),
        title: format!("{} · {}", a.citation, a.section_label),
        excerpt: Some(a.exact_text.clone()),
        verification_status: if a.trust_level == "OFFICIAL_VERIFIED"
            && a.temporal_status == "APPLICABLE"
            && a.superseded_status == "CURRENT"
        {
            "GREEN".into()
        } else {
            "AMBER".into()
        },
    }
}

fn prop_refs(
    turn: &DebriefTurnInput,
    p: &crate::proposition::VerifiedProposition,
) -> Vec<SourceRef> {
    let mut refs = Vec::new();
    for support in &p.verified_support_ids {
        if let Some(a) = turn
            .authorities
            .iter()
            .find(|a| support.contains(&a.authority_id) && support.contains(&a.chunk_id))
        {
            refs.push(legal_ref(a));
        } else {
            refs.push(SourceRef {
                source_kind: if p.class == "COUNSEL_INPUT" {
                    "COUNSEL_INPUT"
                } else {
                    "MATTER_EVIDENCE"
                }
                .into(),
                stable_id: support.clone(),
                locator: format!("matter-ledger://{support}"),
                title: support.clone(),
                excerpt: None,
                verification_status: p.verification_status.clone(),
            });
        }
    }
    refs
}

fn category_for(text: &str, p: &crate::proposition::VerifiedProposition) -> &'static str {
    let low = text.to_ascii_lowercase();
    if low.contains("but ") || low.contains("contradict") {
        "CONTRADICTION_PRESSED"
    } else if p.class == "LEGAL_BASIS" && p.verification_status != "GREEN" {
        "LEGAL_GAP_OR_MISUSE"
    } else if p.class == "INFERENCE" || p.class == "UNSUPPORTED_OR_GAP" {
        "UNSUPPORTED_PERSUASION"
    } else if low.contains("no ") || low.contains("missing") || low.contains("not linked") {
        "FACTUAL_OR_EVIDENTIARY_GAP"
    } else {
        "WEAKNESS_DISCOVERED"
    }
}

fn task_title(category: &str, text: &str) -> String {
    let subject: String = text.chars().take(150).collect();
    match category {
        "CONTRADICTION_PRESSED" => format!("Reconcile and source the contradiction: {subject}"),
        "LEGAL_GAP_OR_MISUSE" => format!("Add or correct the controlling authority for: {subject}"),
        "UNSUPPORTED_PERSUASION" => {
            format!("Supply verified bases or remove the unsupported submission: {subject}")
        }
        "UNANSWERED_QUESTION" => format!("Prepare a sourced answer to: {subject}"),
        "MISSED_AUTHORITY" => {
            format!("Review and decide how to use the unaddressed authority: {subject}")
        }
        _ => format!("Close the source-specific preparation gap: {subject}"),
    }
}

fn usefulness(title: &str, turns: &[String], sources: &[SourceRef], epistemic: &str) -> u8 {
    let mut score = 0;
    if title.len() >= 24 {
        score += 1;
    }
    if !turns.is_empty() {
        score += 1;
    }
    if !sources.is_empty() {
        score += 1;
    }
    if matches!(epistemic, "OBSERVED" | "EXPLICIT_GAP" | "INFERENCE") {
        score += 1;
    }
    score
}

pub fn generate(
    request: GenerateRequest,
    integrity: Option<&crate::integrity::ScoreReport>,
    previous: Option<&Value>,
) -> Result<DebriefReportV2, String> {
    if request.case_id.trim().is_empty()
        || request.session_id.trim().is_empty()
        || request.turns.is_empty()
    {
        return Err(
            "CF_DEBRIEF_V2_EMPTY: case, session, and at least one recorded turn are required"
                .into(),
        );
    }
    let now = now_ms();
    let mut h = Sha256::new();
    h.update(serde_json::to_vec(&request).map_err(|e| e.to_string())?);
    let fingerprint = hex::encode(h.finalize());
    let debrief_id = format!("debrief:{}", &fingerprint[..20]);
    let mut findings = Vec::new();
    let mut used_authorities = HashSet::new();
    let mut supported = 0usize;
    let mut total = 0usize;

    for (turn_index, turn) in request.turns.iter().enumerate() {
        if let Some(ledger) = &turn.proposition_ledger {
            for p in &ledger.propositions {
                total += 1;
                if p.verification_status == "GREEN" {
                    supported += 1;
                }
                used_authorities.extend(p.verified_support_ids.iter().cloned());
                let category = category_for(&p.text, p);
                let refs = prop_refs(turn, p);
                let epistemic = if p.render_action == "TRUSTED"
                    || (p.class == "COUNSEL_INPUT" && p.render_action == "LABELLED")
                {
                    "OBSERVED"
                } else if p.class == "INFERENCE" {
                    "INFERENCE"
                } else {
                    "EXPLICIT_GAP"
                };
                findings.push(Finding {
                    finding_id: format!("finding:{}:{}", turn.turn_id, p.proposition_id),
                    category: category.into(),
                    title: p.text.clone(),
                    detail: format!("{} Host verifier: {}", turn.assistant_text, p.reason),
                    turn_ids: vec![turn.turn_id.clone()],
                    source_refs: refs,
                    epistemic_status: epistemic.into(),
                    severity: if p.verification_status == "RED" {
                        "HIGH"
                    } else {
                        "MEDIUM"
                    }
                    .into(),
                });
            }
        } else {
            findings.push(Finding {
                finding_id: format!("finding:{}:ledger-gap", turn.turn_id),
                category: "UNSUPPORTED_PERSUASION".into(),
                title: "Turn has no verified proposition ledger".into(),
                detail: "The rehearsal turn cannot be treated as grounded analysis.".into(),
                turn_ids: vec![turn.turn_id.clone()],
                source_refs: vec![],
                epistemic_status: "EXPLICIT_GAP".into(),
                severity: "HIGH".into(),
            });
        }
        let next_answered = request
            .turns
            .get(turn_index + 1)
            .is_some_and(|next| next.counsel_text.split_whitespace().count() >= 4);
        if turn.assistant_text.contains('?') && !next_answered {
            findings.push(Finding {
                finding_id: format!("finding:{}:unanswered", turn.turn_id),
                category: "UNANSWERED_QUESTION".into(),
                title: turn
                    .assistant_text
                    .split('?')
                    .next()
                    .unwrap_or("Question")
                    .trim()
                    .to_string()
                    + "?",
                detail: "Question remained open at the end of the recorded turn.".into(),
                turn_ids: vec![turn.turn_id.clone()],
                source_refs: vec![],
                epistemic_status: "EXPLICIT_GAP".into(),
                severity: "MEDIUM".into(),
            });
        }
    }

    let all_authorities: Vec<_> = request
        .turns
        .iter()
        .flat_map(|t| t.authorities.iter())
        .collect();
    let mut seen_chunks = HashSet::new();
    for a in all_authorities {
        if seen_chunks.insert(a.chunk_id.clone())
            && !used_authorities.iter().any(|id| id.contains(&a.chunk_id))
        {
            findings.push(Finding { finding_id: format!("finding:missed:{}", a.chunk_id), category: "MISSED_AUTHORITY".into(), title: format!("{} · {}", a.citation, a.section_label), detail: "Verified working-set passage was available but no verified session proposition relied on it.".into(), turn_ids: request.turns.iter().map(|t| t.turn_id.clone()).collect(), source_refs: vec![legal_ref(a)], epistemic_status: "OBSERVED".into(), severity: "LOW".into() });
        }
    }
    if let Some(score) = integrity {
        for gap in score.missing_links_panel.iter().take(5) {
            findings.push(Finding {
                finding_id: format!("finding:integrity:{}:{}", gap.limb_id, gap.link),
                category: "FACTUAL_OR_EVIDENTIARY_GAP".into(),
                title: format!(
                    "{} — {} link scored {}/{}",
                    gap.limb_id, gap.link, gap.score, gap.threshold
                ),
                detail: gap
                    .description
                    .clone()
                    .unwrap_or_else(|| "Deterministic Integrity missing link.".into()),
                turn_ids: request.turns.iter().map(|t| t.turn_id.clone()).collect(),
                source_refs: gap
                    .inputs_cited
                    .iter()
                    .map(|id| SourceRef {
                        source_kind: "INTEGRITY_INPUT".into(),
                        stable_id: id.clone(),
                        locator: format!("integrity://{id}"),
                        title: id.clone(),
                        excerpt: None,
                        verification_status: "UNSUPPORTED".into(),
                    })
                    .collect(),
                epistemic_status: "EXPLICIT_GAP".into(),
                severity: "HIGH".into(),
            });
        }
    }

    let mut generic_tasks_rejected = 0;
    let mut tasks = Vec::new();
    for f in &findings {
        let title = task_title(&f.category, &f.title);
        let score = usefulness(&title, &f.turn_ids, &f.source_refs, &f.epistemic_status);
        if score < 3 {
            generic_tasks_rejected += 1;
            continue;
        }
        tasks.push(PreparationTask {
            task_id: format!(
                "task:{}",
                &hex::encode(Sha256::digest(
                    format!("{}:{}", debrief_id, f.finding_id).as_bytes()
                ))[..20]
            ),
            debrief_id: debrief_id.clone(),
            title,
            detail: f.detail.clone(),
            status: "PROPOSED".into(),
            turn_ids: f.turn_ids.clone(),
            finding_ids: vec![f.finding_id.clone()],
            source_refs: f.source_refs.clone(),
            epistemic_status: f.epistemic_status.clone(),
            usefulness_score: score,
            counsel_note: None,
            updated_at_unix_ms: now,
        });
    }
    let current_rate = if total == 0 {
        0.0
    } else {
        supported as f64 / total as f64
    };
    let previous_id = previous
        .and_then(|v| v.get("debrief_id"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let previous_rate = previous
        .and_then(|v| v.pointer("/comparison/supported_rate_current"))
        .and_then(Value::as_f64);
    let observed_change = match previous_rate {
        Some(p) if current_rate > p => format!(
            "Verified-support rate increased from {:.0}% to {:.0}% across recorded sessions.",
            p * 100.0,
            current_rate * 100.0
        ),
        Some(p) if current_rate < p => format!(
            "Verified-support rate decreased from {:.0}% to {:.0}%; review source use.",
            p * 100.0,
            current_rate * 100.0
        ),
        Some(_) => "Verified-support rate was unchanged across recorded sessions.".into(),
        None => "First comparable Debrief v2 session; no earlier baseline.".into(),
    };
    Ok(DebriefReportV2 { schema_version: SCHEMA_VERSION.into(), debrief_id, case_id: request.case_id, session_id: request.session_id, format: request.format, created_at_unix_ms: now, turns_completed: request.turns.len(), findings, tasks, comparison: SessionComparison { previous_debrief_id: previous_id, observed_change, supported_rate_previous: previous_rate, supported_rate_current: current_rate, outcome_prediction: false }, source_fingerprint: fingerprint, rubric_version: RUBRIC_VERSION.into(), generic_tasks_rejected, disclaimer: "Preparation review only. Observations describe recorded rehearsal performance and source coverage; they do not predict outcome, guilt, sentence, or judge behavior.".into() })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_session_fails_closed() {
        assert!(generate(
            GenerateRequest {
                case_id: "c".into(),
                session_id: "s".into(),
                format: "f".into(),
                turns: vec![]
            },
            None,
            None
        )
        .is_err());
    }
    #[test]
    fn usefulness_rejects_generic() {
        assert!(usefulness("Review case", &[], &[], "INFERENCE") < 3);
    }
    #[test]
    fn planted_weaknesses_become_traceable_specific_tasks() {
        let props = vec![
            crate::proposition::VerifiedProposition {
                proposition_id: "p1".into(),
                class: "FACTUAL_BASIS".into(),
                text: "Witness A says notice was served, but the register contradicts it.".into(),
                support_ids: vec!["matter:doc:p1".into()],
                basis_ids: vec![],
                support_claims: vec![],
                quotation: None,
                verification_status: "GREEN".into(),
                render_action: "TRUSTED".into(),
                verified_support_ids: vec!["matter:doc:p1".into()],
                rejected_support_ids: vec![],
                quote_offsets: None,
                reason: "exact matter source".into(),
            },
            crate::proposition::VerifiedProposition {
                proposition_id: "p2".into(),
                class: "UNSUPPORTED_OR_GAP".into(),
                text: "No partition instrument is linked.".into(),
                support_ids: vec![],
                basis_ids: vec![],
                support_claims: vec![],
                quotation: None,
                verification_status: "RED".into(),
                render_action: "REPOSITORY_GAP".into(),
                verified_support_ids: vec![],
                rejected_support_ids: vec![],
                quote_offsets: None,
                reason: "explicit gap".into(),
            },
        ];
        let ledger = crate::proposition::PropositionLedger {
            schema_version: crate::proposition::SCHEMA_VERSION.into(),
            batch: "B-S3-14".into(),
            output_kind: "MOCK_COURT".into(),
            propositions: props,
            rendered_text: "".into(),
            trusted_count: 1,
            gap_count: 1,
            blocked_count: 0,
            all_legal_grounded: true,
            all_quotes_exact: true,
        };
        let report = generate(
            GenerateRequest {
                case_id: "case".into(),
                session_id: "session".into(),
                format: "Civil_Appeal".into(),
                turns: vec![DebriefTurnInput {
                    turn_id: "turn-1".into(),
                    counsel_text: "submission".into(),
                    assistant_text: "challenge".into(),
                    proposition_ledger: Some(ledger),
                    authorities: vec![],
                }],
            },
            None,
            None,
        )
        .unwrap();
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == "CONTRADICTION_PRESSED"));
        assert!(report
            .findings
            .iter()
            .any(|f| f.category == "UNSUPPORTED_PERSUASION"));
        assert!(report.tasks.iter().all(|t| t.usefulness_score >= 3
            && !t.turn_ids.is_empty()
            && (!t.source_refs.is_empty() || t.epistemic_status != "OBSERVED")));
        assert!(!report.comparison.outcome_prediction);
    }
}
