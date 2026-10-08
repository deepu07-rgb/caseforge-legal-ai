//! Step-by-step reasoning with best-of-n verification (B-S4-28).
//!
//! A 7B local model reasons less well than a large cloud model, but its mistakes are mechanically checkable: every legal
//! or factual step must copy an exact excerpt from a supplied source, and every inference must name the earlier steps it
//! rests on. The host verifier (`proposition::verify`) checks that without any model. This module uses that check to buy
//! quality with compute: it asks for the reasoning as a ledger of numbered propositions, samples several candidates
//! (deterministic seed variants), verifies each, keeps the best-verified one, and gives a failing one a single targeted
//! repair. If nothing verifies, it says so: the result is labelled and never presented as a verified answer.
//!
//! No part of selection trusts the model: scores come only from the host's verification, plus agreement between the
//! candidates' conclusions as a tie-breaker. Deterministic given the model's outputs. Golden: golden/G-REASON.

use crate::proposition::{self, PropositionInput, PropositionLedger, SupportSource, VerifyRequest};
use serde::Serialize;
use std::collections::HashSet;

pub const SCHEMA_VERSION: &str = "caseforge.reasoning-run.v1";
/// The variant number reported for the repair candidate.
pub const REPAIR_VARIANT: u32 = 100;
const MAX_PROPOSITIONS: usize = 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Depth {
    /// One candidate, no repair (fastest).
    Quick,
    /// Up to three candidates, stop at the first fully verified one, one repair.
    Careful,
    /// Five candidates, all of them (agreement between conclusions is measured), one repair.
    Thorough,
}

impl Depth {
    pub fn parse(s: &str) -> Option<Depth> {
        match s.to_ascii_uppercase().as_str() {
            "QUICK" => Some(Depth::Quick),
            "CAREFUL" => Some(Depth::Careful),
            "THOROUGH" => Some(Depth::Thorough),
            _ => None,
        }
    }
    pub fn candidates(self) -> usize {
        match self {
            Depth::Quick => 1,
            Depth::Careful => 3,
            Depth::Thorough => 5,
        }
    }
    fn stops_at_first_verified(self) -> bool {
        self == Depth::Careful
    }
    fn repairs(self) -> bool {
        self != Depth::Quick
    }
    /// Wall-clock allowance for the whole run.
    pub fn budget_ms(self) -> u64 {
        match self {
            Depth::Quick => 300_000,
            Depth::Careful => 900_000,
            Depth::Thorough => 1_800_000,
        }
    }
}

/// What the generator is asked for: sample number `variant` (0 is the model's default settings), or a repair.
pub struct Attempt<'a> {
    pub variant: u32,
    pub repair: Option<RepairNote<'a>>,
}

pub struct RepairNote<'a> {
    pub previous: &'a [PropositionInput],
    /// (proposition id, why the host rejected it)
    pub problems: Vec<(String, String)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateSummary {
    pub variant: u32,
    pub points: i32,
    pub trusted: usize,
    pub labelled: usize,
    pub red: usize,
    pub conclusion_ok: bool,
    /// Mean agreement of this candidate's conclusion with the other usable candidates' (0 to 1).
    pub agreement: f64,
    pub repair: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Discarded {
    pub variant: u32,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct ReasoningResult {
    pub schema_version: &'static str,
    pub depth: Depth,
    pub candidates_requested: usize,
    pub candidates_run: usize,
    pub chosen_variant: Option<u32>,
    /// VERIFIED: every step verified or visibly labelled and the conclusion stands. PARTIAL: usable steps, but some step
    /// failed verification (shown as a gap). NO_VERIFIED_ANSWER: nothing the host could stand behind.
    pub status: String,
    pub verified_answer: bool,
    pub agreement: f64,
    pub stopped_early: bool,
    pub stopped_for_time: bool,
    pub repair_attempted: bool,
    pub repair_accepted: bool,
    pub candidates: Vec<CandidateSummary>,
    pub discarded: Vec<Discarded>,
    pub ledger: Option<PropositionLedger>,
    pub rendered_text: String,
}

struct Cand {
    variant: u32,
    props: Vec<PropositionInput>,
    ledger: PropositionLedger,
    trusted: usize,
    labelled: usize,
    red: usize,
    conclusion_ok: bool,
    tokens: HashSet<String>,
    base_points: i32,
    points: i32,
    agreement: f64,
    repair: bool,
}

impl Cand {
    fn fully_verified(&self) -> bool {
        self.red == 0 && self.conclusion_ok && self.trusted >= 1
    }
}

fn tokens(text: &str) -> HashSet<String> {
    const STOP: &[&str] = &["that", "this", "with", "from", "which", "their", "there", "therefore", "because", "should", "would", "could", "have", "been", "were", "also"];
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.chars().count() >= 4 && !STOP.contains(t))
        .map(str::to_string)
        .collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        a.intersection(b).count() as f64 / union as f64
    }
}

fn evaluate(variant: u32, props: Vec<PropositionInput>, sources: &[SupportSource], permitted: &[String], repair: bool) -> Result<Cand, String> {
    if props.is_empty() {
        return Err("the model returned no usable propositions".into());
    }
    if props.len() > MAX_PROPOSITIONS {
        return Err(format!("more than {MAX_PROPOSITIONS} propositions"));
    }
    let ledger = proposition::verify(VerifyRequest {
        output_kind: "REASONING".into(),
        propositions: props.clone(),
        support_sources: sources.to_vec(),
        permitted_authority_ids: permitted.to_vec(),
    })?;
    let trusted = ledger.propositions.iter().filter(|p| p.render_action == "TRUSTED").count();
    let labelled = ledger.propositions.iter().filter(|p| p.render_action == "LABELLED").count();
    let red = ledger.propositions.iter().filter(|p| p.verification_status == "RED").count();
    let last = ledger.propositions.last().ok_or("empty ledger")?;
    let conclusion_ok = last.verification_status != "RED";
    let base_points = 3 * trusted as i32 + labelled as i32 - 6 * red as i32 - if conclusion_ok { 0 } else { 8 };
    Ok(Cand { variant, tokens: tokens(&last.text), props, ledger, trusted, labelled, red, conclusion_ok, base_points, points: base_points, agreement: 0.0, repair })
}

/// Mean Jaccard agreement of each usable candidate's conclusion with the others'; adds up to 4 points.
fn apply_agreement(cands: &mut [Cand]) -> f64 {
    let usable: Vec<usize> = (0..cands.len()).filter(|&i| cands[i].conclusion_ok && !cands[i].repair).collect();
    if usable.len() < 2 {
        return 0.0;
    }
    let mut total = 0.0;
    for &i in &usable {
        let mean = usable.iter().filter(|&&j| j != i).map(|&j| jaccard(&cands[i].tokens, &cands[j].tokens)).sum::<f64>() / (usable.len() - 1) as f64;
        cands[i].agreement = mean;
        cands[i].points = cands[i].base_points + (4.0 * mean).round() as i32;
        total += mean;
    }
    total / usable.len() as f64
}

/// Runs the reasoning. `now_ms` is a monotonic clock in milliseconds; `generate` asks the model for one candidate
/// ledger (or a repair) and returns its propositions, or an error that discards just that candidate.
pub fn run(
    sources: &[SupportSource],
    permitted: &[String],
    depth: Depth,
    budget_ms: u64,
    now_ms: &mut dyn FnMut() -> u64,
    generate: &mut dyn FnMut(Attempt) -> Result<Vec<PropositionInput>, String>,
) -> ReasoningResult {
    let start = now_ms();
    let n = depth.candidates();
    let (mut cands, mut discarded): (Vec<Cand>, Vec<Discarded>) = (Vec::new(), Vec::new());
    let (mut run_count, mut stopped_early, mut stopped_for_time) = (0usize, false, false);
    let mut last_call_ms = 0u64;
    let remaining = |now: u64| budget_ms.saturating_sub(now.saturating_sub(start));

    for variant in 0..n as u32 {
        if variant > 0 && last_call_ms.saturating_mul(12) / 10 > remaining(now_ms()) {
            stopped_for_time = true; // the next sample would probably not finish inside the allowance
            break;
        }
        let t0 = now_ms();
        let outcome = generate(Attempt { variant, repair: None });
        last_call_ms = now_ms().saturating_sub(t0);
        run_count += 1;
        match outcome.and_then(|props| evaluate(variant, props, sources, permitted, false)) {
            Err(reason) => discarded.push(Discarded { variant, reason }),
            Ok(c) => {
                let done = c.fully_verified();
                cands.push(c);
                if done && depth.stops_at_first_verified() && (variant as usize + 1) < n {
                    stopped_early = true;
                    break;
                }
            }
        }
    }

    let agreement = apply_agreement(&mut cands);
    let best_index = |cands: &[Cand]| (0..cands.len()).max_by_key(|&i| (cands[i].points, std::cmp::Reverse(cands[i].variant)));
    let mut chosen = best_index(&cands);

    // One targeted repair of the best candidate if it still has steps the host rejected.
    let (mut repair_attempted, mut repair_accepted) = (false, false);
    if let Some(bi) = chosen {
        if depth.repairs() && cands[bi].red > 0 && last_call_ms.saturating_mul(12) / 10 <= remaining(now_ms()) {
            let problems: Vec<(String, String)> = cands[bi]
                .ledger
                .propositions
                .iter()
                .filter(|p| p.verification_status == "RED")
                .map(|p| (p.proposition_id.clone(), p.reason.clone()))
                .collect();
            let previous = cands[bi].props.clone();
            repair_attempted = true;
            match generate(Attempt { variant: REPAIR_VARIANT, repair: Some(RepairNote { previous: &previous, problems }) }).and_then(|props| evaluate(REPAIR_VARIANT, props, sources, permitted, true)) {
                Err(reason) => discarded.push(Discarded { variant: REPAIR_VARIANT, reason }),
                Ok(r) => {
                    let better = r.base_points > cands[bi].points;
                    cands.push(r);
                    if better {
                        repair_accepted = true;
                        chosen = Some(cands.len() - 1);
                    }
                }
            }
        }
    }

    let summaries: Vec<CandidateSummary> = cands
        .iter()
        .map(|c| CandidateSummary { variant: c.variant, points: c.points, trusted: c.trusted, labelled: c.labelled, red: c.red, conclusion_ok: c.conclusion_ok, agreement: c.agreement, repair: c.repair })
        .collect();
    let (status, chosen_variant, ledger, rendered_text) = match chosen {
        None => ("NO_VERIFIED_ANSWER".to_string(), None, None, String::new()),
        Some(i) => {
            let c = &cands[i];
            let status = if c.fully_verified() {
                "VERIFIED"
            } else if c.conclusion_ok && c.trusted + c.labelled >= 1 {
                "PARTIAL"
            } else {
                "NO_VERIFIED_ANSWER"
            };
            (status.to_string(), Some(c.variant), Some(c.ledger.clone()), c.ledger.rendered_text.clone())
        }
    };
    ReasoningResult {
        schema_version: SCHEMA_VERSION,
        depth,
        candidates_requested: n,
        candidates_run: run_count,
        chosen_variant,
        verified_answer: status == "VERIFIED",
        status,
        agreement,
        stopped_early,
        stopped_for_time,
        repair_attempted,
        repair_accepted,
        candidates: summaries,
        discarded,
        ledger,
        rendered_text,
    }
}

/// The request text for one candidate. The sidecar appends the proposition-ledger output contract.
pub fn prompt(question: &str) -> String {
    format!(
        "QUESTION: {question}\n\nReason step by step and return the proposition ledger as JSON. Order the propositions like this: \
(1) a FACTUAL_BASIS for each material fact, its excerpt copied verbatim from the matter support; \
(2) a LEGAL_BASIS for each rule you rely on, its excerpt copied verbatim from a permitted authority support; \
(3) INFERENCE steps that apply each rule to the facts, each naming in basis_ids the earlier propositions it rests on; \
(4) the last proposition is the conclusion: an INFERENCE whose basis_ids name the steps it follows from. \
If a rule needed for the conclusion is not in the supplied authorities, add an UNSUPPORTED_OR_GAP proposition for it instead of stating it, \
and let the conclusion say what remains to be verified. Do not predict outcomes, verdicts or probabilities."
    )
}

/// The request text for the one repair: the failed steps and why, the previous ledger, and the fix to make.
pub fn repair_prompt(question: &str, previous: &[PropositionInput], problems: &[(String, String)]) -> String {
    let failed = problems.iter().map(|(id, why)| format!("- {id}: {why}")).collect::<Vec<_>>().join("\n");
    let ledger = serde_json::to_string(&serde_json::json!({ "propositions": previous })).unwrap_or_default();
    format!(
        "QUESTION: {question}\n\nREPAIR ONLY. The host rejected these propositions of your previous ledger:\n{failed}\n\nPrevious ledger:\n{ledger}\n\n\
Return the complete ledger again. For each rejected FACTUAL_BASIS or LEGAL_BASIS, either copy a verbatim excerpt from the named support, or replace it with an \
UNSUPPORTED_OR_GAP proposition; then rebuild the inferences and the conclusion that depended on it. Keep every proposition that was accepted. Add no new facts, law or sources."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::cell::Cell;

    const FIXTURE: &str = include_str!("../../golden/G-REASON/fixtures/best_of_n.json");
    const FIXTURE_SHA256: &str = "24cb84dbf711cc54a3a85012598db1cb31fa3cd8618f4a6f1f1ed34f8492c194"; // committed before the engine (be3c43b)

    fn candidate(fx: &Value, name: &Value) -> Result<Vec<PropositionInput>, String> {
        match name {
            Value::String(n) => serde_json::from_value(fx["candidates"][n.as_str()].clone()).map_err(|e| e.to_string()),
            Value::Array(_) => Ok(vec![]),
            other => Err(other["error"].as_str().unwrap_or("error").to_string()),
        }
    }

    #[test]
    fn golden_best_of_n_scenarios() {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "G-REASON fixture changed since it was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let sources: Vec<SupportSource> = serde_json::from_value(fx["sources"].clone()).unwrap();
        let permitted: Vec<String> = serde_json::from_value(fx["permitted_authority_ids"].clone()).unwrap();
        for sc in fx["scenarios"].as_array().unwrap() {
            let name = sc["name"].as_str().unwrap();
            let depth = Depth::parse(sc["depth"].as_str().unwrap()).unwrap();
            let calls = Cell::new(0u64);
            let step = sc["clock_ms_per_call"].as_u64().unwrap();
            let mut clock = || calls.get() * step;
            let attempts = sc["attempts"].as_array().unwrap();
            let mut generate = |a: Attempt| -> Result<Vec<PropositionInput>, String> {
                calls.set(calls.get() + 1);
                match a.repair {
                    Some(note) => {
                        assert!(!note.problems.is_empty(), "{name}: a repair names the failed steps");
                        candidate(&fx, &sc["repair"])
                    }
                    None => candidate(&fx, attempts.get(a.variant as usize).unwrap_or(&Value::Null)),
                }
            };
            let r = run(&sources, &permitted, depth, sc["budget_ms"].as_u64().unwrap(), &mut clock, &mut generate);
            let e = &sc["expect"];
            assert_eq!(r.chosen_variant.map(u64::from), e["chosen_variant"].as_u64(), "{name}: chosen variant");
            assert_eq!(r.status, e["status"].as_str().unwrap(), "{name}: status");
            assert_eq!(r.verified_answer, e["verified_answer"].as_bool().unwrap(), "{name}: verified_answer");
            assert_eq!(r.candidates_run as u64, e["n_run"].as_u64().unwrap(), "{name}: candidates run");
            assert_eq!(r.stopped_early, e["stopped_early"].as_bool().unwrap(), "{name}: stopped_early");
            assert_eq!(r.stopped_for_time, e["stopped_for_time"].as_bool().unwrap(), "{name}: stopped_for_time");
            assert_eq!(r.repair_attempted, e["repair_attempted"].as_bool().unwrap(), "{name}: repair_attempted");
            assert_eq!(r.repair_accepted, e["repair_accepted"].as_bool().unwrap(), "{name}: repair_accepted");
            assert_eq!(r.discarded.len() as u64, e["discarded"].as_u64().unwrap(), "{name}: discarded");
            if e["agreement_positive"].as_bool() == Some(true) {
                assert!(r.agreement > 0.0, "{name}: conclusions of agreeing candidates register agreement");
            }
            // Whatever is chosen, an answer is only called verified when the host verified every step of it.
            if r.verified_answer {
                let l = r.ledger.as_ref().unwrap();
                assert!(l.propositions.iter().all(|p| p.verification_status != "RED"), "{name}");
                assert!(r.rendered_text.contains("[VERIFIED:"), "{name}");
            } else if let Some(l) = &r.ledger {
                assert!(l.propositions.iter().any(|p| p.verification_status == "RED") || r.status != "VERIFIED", "{name}");
            }
        }
    }

    #[test]
    fn a_fabricated_excerpt_never_reaches_a_verified_answer() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let sources: Vec<SupportSource> = serde_json::from_value(fx["sources"].clone()).unwrap();
        let bad: Vec<PropositionInput> = serde_json::from_value(fx["candidates"]["BAD"].clone()).unwrap();
        for depth in [Depth::Quick, Depth::Careful, Depth::Thorough] {
            let mut clock = || 0u64;
            let mut generate = |_: Attempt| Ok(bad.clone());
            let r = run(&sources, &["A1".to_string()], depth, depth.budget_ms(), &mut clock, &mut generate);
            assert!(!r.verified_answer && r.status == "NO_VERIFIED_ANSWER", "{depth:?}: {}", r.status);
            assert!(r.rendered_text.contains("NOT VERIFIED") || r.rendered_text.contains("BLOCKED"), "{depth:?}: the failure is shown, not hidden");
        }
    }

    #[test]
    fn prompts_carry_the_question_the_order_and_the_failures() {
        let p = prompt("Is the lease enforceable?");
        assert!(p.contains("Is the lease enforceable?") && p.contains("FACTUAL_BASIS") && p.contains("UNSUPPORTED_OR_GAP") && p.contains("Do not predict outcomes"));
        let prev: Vec<PropositionInput> = vec![];
        let r = repair_prompt("Q?", &prev, &[("l1".into(), "No permitted verified authority".into())]);
        assert!(r.contains("l1: No permitted verified authority") && r.contains("REPAIR ONLY") && r.contains("Add no new facts"));
    }

    #[test]
    fn depth_parses_and_sets_the_number_of_candidates() {
        assert_eq!(Depth::parse("careful"), Some(Depth::Careful));
        assert_eq!(Depth::parse("nope"), None);
        assert_eq!((Depth::Quick.candidates(), Depth::Careful.candidates(), Depth::Thorough.candidates()), (1, 3, 5));
    }
}
