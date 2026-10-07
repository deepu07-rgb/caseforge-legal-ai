//! Voice-in-court + structured debrief (B-S2-04 / PRD §2 Stage 2, §9).
//!
//! Full-duplex English path: counsel STT (lexicon) → Mock Court turn → role TTS.
//! Network off. Lite tier: honesty notice + refuse live simulation.
//! No named judges. No non-English court voice.

use crate::adaptive_bench;
use crate::choreography;
use crate::installer;
use crate::mock_court;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Instant;

pub const PIPELINE_ID: &str = "caseforge.voice_court.v1";
pub const SCHEMA_VERSION: &str = "1.0.0";
pub const BATCH: &str = "B-S2-04";
pub const PRIMARY_LANGUAGE: &str = "English";
pub const TURN_MAX_S: u64 = 15; // AC-3.3
pub const LITE_REFUSE_CODE: &str = "CF_VOICE_COURT_LITE_REFUSE";

pub const TTS_ROLES: [&str; 3] = ["bench", "opposition", "witness"];

const LITE_HONESTY: &str = "Lite tier honesty notice: this machine is under 8 GB RAM. \
CaseForge Lite supports Digest vault, drafting, citation checking, and lexicon voice dictation only. \
Live Mock Court voice simulation is not available. Upgrade to Full tier (≥8 GB) for voice-in-court.";

const DISCLAIMER: &str = "Voice Mock Court is preparation only. Not a real hearing. \
No verdict prediction (G6). English primary. Offline only.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceGate {
    pub allowed: bool,
    pub tier: String,
    pub live_simulation_allowed: bool,
    pub honesty_notice: Option<String>,
    pub refuse_code: Option<String>,
    pub refuse_reason: Option<String>,
    pub messaging: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceTurnResult {
    pub pipeline_id: String,
    pub session_id: String,
    pub case_id: String,
    pub format: String,
    pub turn_index: u32,
    pub counsel_raw: String,
    pub counsel_staged: String,
    pub lexicon_applied: bool,
    pub court_turn: mock_court::SessionTurn,
    pub tts_role: String,
    pub tts_spoken_text: String,
    pub stt_network_used: bool,
    pub tts_network_used: bool,
    pub network_used: bool,
    pub cloud: bool,
    pub elapsed_ms: u64,
    pub within_ac33: bool,
    pub language: String,
    pub debrief_partial: Option<String>,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebriefReport {
    pub pipeline_id: String,
    pub session_id: String,
    pub case_id: String,
    pub format: String,
    pub turns_completed: usize,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub missing_links_focus: Vec<String>,
    pub recommended_actions: Vec<String>,
    pub lexicon_hits: usize,
    pub network_used_any: bool,
    pub max_turn_ms: u64,
    pub within_ac33: bool,
    pub tier: String,
    pub live_simulation_allowed: bool,
    pub fingerprint: String,
    pub disclaimer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptedVoiceReport {
    pub pipeline_id: String,
    pub format: String,
    pub script_id: String,
    pub case_id: String,
    pub turns: Vec<VoiceTurnResult>,
    pub debrief: DebriefReport,
    pub pass: bool,
    pub reasons: Vec<String>,
    pub network_used: bool,
    pub lite_refused: bool,
    pub llm_required: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VoiceTurnRequest {
    pub session_id: String,
    pub case_id: String,
    pub format: String,
    pub turn_index: Option<u32>,
    /// Offline STT path (provisional).
    pub transcript_stub: Option<String>,
    pub counsel_text: Option<String>,
    /// Mock Court role for reply: adversary | bench
    pub court_role: Option<String>,
    pub tts_role: Option<String>,
    pub case_graph: Option<Value>,
    pub retrieval_ids: Option<Vec<String>>,
    pub retrieval_snippets: Option<Vec<String>>,
    pub bench_archetype_id: Option<String>,
    pub bench_sliders: Option<adaptive_bench::SliderSet>,
    /// Optional lexicon pairs for offline apply without vault: [[surface, corrected], ...]
    pub lexicon_pairs: Option<Vec<(String, String)>>,
    /// Eval only: force tier ("lite" | "full" | "unknown")
    pub force_tier: Option<String>,
    pub language: Option<String>,
}

pub fn policy() -> Value {
    json!({
        "pipeline_id": PIPELINE_ID,
        "schema_version": SCHEMA_VERSION,
        "batch": BATCH,
        "primary_language": PRIMARY_LANGUAGE,
        "language_pinning": true,
        "tts_roles": TTS_ROLES,
        "network_required": false,
        "turn_max_s": TURN_MAX_S,
        "lite_honesty_notice": LITE_HONESTY,
        "live_simulation_requires_full_tier": true,
        "named_judge_profiles": false,
        "mode": "full_duplex_english",
        "stt": "caseforge.speech.provisional.v1",
        "court": "caseforge.mock_court.v1",
        "bench": "caseforge.adaptive_bench.v1",
        "debrief": "structured",
        "disclaimer": DISCLAIMER,
        "prd_refs": ["§2 Stage 2", "§9", "AC-3.3", "AC-9.3", "AC-9.4"],
        "acceptance": {
            "voice_multi_turn": true,
            "network_off": true,
            "lite_honesty_refuse": true,
            "ac33_turn_budget_s": TURN_MAX_S
        }
    })
}

/// Resolve whether live voice Mock Court is allowed.
pub fn gate(force_tier: Option<&str>) -> VoiceGate {
    let (tier, live) = if let Some(ft) = force_tier {
        match ft {
            "lite" => ("lite".into(), false),
            "full" => ("full".into(), true),
            "not_supported" => ("not_supported".into(), false),
            _ => ("unknown".into(), true),
        }
    } else {
        choreography::detect_tier()
    };

    if tier == "lite" || (!live && tier != "unknown" && tier != "full") {
        // Lite: refuse live voice simulation with honesty notice
        if tier == "lite" {
            return VoiceGate {
                allowed: false,
                tier,
                live_simulation_allowed: false,
                honesty_notice: Some(LITE_HONESTY.into()),
                refuse_code: Some(LITE_REFUSE_CODE.into()),
                refuse_reason: Some(LITE_HONESTY.into()),
                messaging: LITE_HONESTY.into(),
            };
        }
    }

    if tier == "not_supported" {
        let hw = installer::evaluate();
        return VoiceGate {
            allowed: false,
            tier,
            live_simulation_allowed: false,
            honesty_notice: hw.refuse_reason.clone(),
            refuse_code: Some("CF_HARDWARE_REFUSE".into()),
            refuse_reason: hw.refuse_reason,
            messaging: hw.messaging,
        };
    }

    VoiceGate {
        allowed: true,
        tier: tier.clone(),
        live_simulation_allowed: live,
        honesty_notice: None,
        refuse_code: None,
        refuse_reason: None,
        messaging: if live {
            "Full tier: voice-in-court full-duplex practice allowed (offline English).".into()
        } else {
            "Voice-in-court available with honesty constraints; live sim policy follows hardware gate.".into()
        },
    }
}

fn pin_language(lang: Option<&str>) -> Result<(), String> {
    let l = lang.unwrap_or(PRIMARY_LANGUAGE).trim();
    let low = l.to_lowercase();
    if low == "english" || low == "en" || low.starts_with("en-") || low.starts_with("en_") {
        Ok(())
    } else {
        Err(format!(
            "CF_SPEECH_LANGUAGE_PINNED: court voice is English primary; got '{l}'"
        ))
    }
}

fn apply_lexicon(raw: &str, pairs: &[(String, String)]) -> (String, bool) {
    let mut out = raw.to_string();
    let mut hit = false;
    let mut ordered = pairs.to_vec();
    ordered.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    for (surface, corrected) in ordered {
        if surface.is_empty() {
            continue;
        }
        let low_out = out.to_lowercase();
        let low_s = surface.to_lowercase();
        if let Some(pos) = low_out.find(&low_s) {
            let end = pos + surface.len();
            // preserve length from original slice carefully
            let (before, rest) = out.split_at(pos);
            let after = if rest.len() >= surface.len() {
                &rest[surface.len()..]
            } else {
                &rest[rest.len()..]
            };
            out = format!("{before}{corrected}{after}");
            hit = true;
            let _ = end;
        }
    }
    (out, hit)
}

fn tts_role_for_court_role(court_role: &str, override_role: Option<&str>) -> String {
    if let Some(r) = override_role {
        if TTS_ROLES.contains(&r) {
            return r.into();
        }
    }
    match court_role {
        "bench" => "bench".into(),
        "adversary" => "opposition".into(),
        "witness" => "witness".into(),
        _ => "opposition".into(),
    }
}

/// Offline duplex turn without live sidecar (deterministic eval path).
pub fn run_voice_turn(req: VoiceTurnRequest) -> Result<VoiceTurnResult, String> {
    let start = Instant::now();
    let g = gate(req.force_tier.as_deref());
    if !g.allowed {
        return Err(format!(
            "{}: {}",
            g.refuse_code.as_deref().unwrap_or(LITE_REFUSE_CODE),
            g.honesty_notice
                .as_deref()
                .or(g.refuse_reason.as_deref())
                .unwrap_or(LITE_HONESTY)
        ));
    }
    pin_language(req.language.as_deref())?;

    let counsel_raw = req.counsel_text.or(req.transcript_stub).unwrap_or_default();
    if counsel_raw.trim().is_empty() {
        return Err("CF_VOICE_COURT_EMPTY: counsel speech empty".into());
    }

    let pairs = req.lexicon_pairs.unwrap_or_default();
    let (counsel_staged, lexicon_applied) = apply_lexicon(&counsel_raw, &pairs);

    let court_role = req.court_role.unwrap_or_else(|| "adversary".into());
    let tts_role = tts_role_for_court_role(&court_role, req.tts_role.as_deref());

    let court_turn = mock_court::run_turn(mock_court::TurnRequest {
        session_id: req.session_id.clone(),
        case_id: req.case_id.clone(),
        format: req.format.clone(),
        role: Some(court_role),
        user_text: counsel_staged.clone(),
        retrieval_ids: req.retrieval_ids,
        retrieval_snippets: req.retrieval_snippets,
        case_graph: req.case_graph,
        script_id: Some(format!("voice-{}", req.session_id)),
        deterministic_only: Some(true),
        bench_archetype_id: req.bench_archetype_id,
        bench_sliders: req.bench_sliders,
        judge_name: None,
        named_judge_id: None,
    })?;

    let reply = court_turn
        .assistant_text
        .clone()
        .unwrap_or_else(|| "(no reply)".into());
    // TTS spoken text: apply lexicon pronunciation-style lightly (same pairs as STT for offline)
    let (tts_spoken, _) = apply_lexicon(&reply, &pairs);
    // Truncate for ~300 token adversary budget display
    let tts_spoken = tts_spoken.chars().take(1200).collect::<String>();

    let elapsed_ms = start.elapsed().as_millis() as u64;
    let within = elapsed_ms <= TURN_MAX_S * 1000;

    Ok(VoiceTurnResult {
        pipeline_id: PIPELINE_ID.into(),
        session_id: req.session_id,
        case_id: req.case_id,
        format: req.format,
        turn_index: req.turn_index.unwrap_or(0),
        counsel_raw,
        counsel_staged,
        lexicon_applied,
        court_turn,
        tts_role,
        tts_spoken_text: tts_spoken,
        stt_network_used: false,
        tts_network_used: false,
        network_used: false,
        cloud: false,
        elapsed_ms,
        within_ac33: within,
        language: PRIMARY_LANGUAGE.into(),
        debrief_partial: None,
        disclaimer: DISCLAIMER.into(),
    })
}

pub fn build_debrief(
    session_id: &str,
    case_id: &str,
    format: &str,
    turns: &[VoiceTurnResult],
    tier: &str,
    live: bool,
) -> DebriefReport {
    let mut strengths = Vec::new();
    let mut weaknesses = Vec::new();
    let mut missing = Vec::new();
    let mut actions = Vec::new();
    let mut lexicon_hits = 0usize;
    let mut network_any = false;
    let mut max_ms = 0u64;

    for t in turns {
        max_ms = max_ms.max(t.elapsed_ms);
        if t.lexicon_applied {
            lexicon_hits += 1;
        }
        if t.network_used {
            network_any = true;
        }
        if t.counsel_staged.split_whitespace().count() >= 12 {
            strengths.push(format!(
                "Turn {}: counsel submission had usable length for the format.",
                t.turn_index
            ));
        } else {
            weaknesses.push(format!(
                "Turn {}: submission was thin — expand relief and anchors.",
                t.turn_index
            ));
        }
        if t.court_turn
            .assistant_text
            .as_ref()
            .map(|s| s.to_lowercase().contains("gap"))
            .unwrap_or(false)
        {
            missing.push(format!(
                "Turn {}: adversary/bench pressed an open gap from the case_graph.",
                t.turn_index
            ));
            actions.push(
                "Close the cited case_graph gap with an exhibit or counsel fact before next drill."
                    .into(),
            );
        }
    }

    if strengths.is_empty() {
        strengths.push("Session completed offline without cloud path.".into());
    }
    if actions.is_empty() {
        actions.push("Re-run Argument Tree VI and re-drill the weakest limb.".into());
    }
    if missing.is_empty() {
        missing.push(
            "No explicit gap language in replies — still re-check Missing Links panel.".into(),
        );
    }

    let mut h = Sha256::new();
    h.update(session_id.as_bytes());
    h.update(format.as_bytes());
    h.update(turns.len().to_string().as_bytes());
    h.update(max_ms.to_string().as_bytes());
    let fingerprint = hex::encode(h.finalize())[..16].to_string();

    DebriefReport {
        pipeline_id: PIPELINE_ID.into(),
        session_id: session_id.into(),
        case_id: case_id.into(),
        format: format.into(),
        turns_completed: turns.len(),
        strengths,
        weaknesses,
        missing_links_focus: missing,
        recommended_actions: actions,
        lexicon_hits,
        network_used_any: network_any,
        max_turn_ms: max_ms,
        within_ac33: max_ms <= TURN_MAX_S * 1000,
        tier: tier.into(),
        live_simulation_allowed: live,
        fingerprint,
        disclaimer: DISCLAIMER.into(),
    }
}

pub fn run_scripted_voice_session(
    format: &str,
    case_id: &str,
    case_graph: Option<Value>,
    script_id: &str,
    user_lines: &[String],
    lexicon_pairs: Vec<(String, String)>,
    force_tier: Option<&str>,
) -> Result<ScriptedVoiceReport, String> {
    // Lite path: single refuse, no turns
    let g = gate(force_tier);
    if !g.allowed {
        return Ok(ScriptedVoiceReport {
            pipeline_id: PIPELINE_ID.into(),
            format: format.into(),
            script_id: script_id.into(),
            case_id: case_id.into(),
            turns: vec![],
            debrief: build_debrief(script_id, case_id, format, &[], &g.tier, false),
            pass: true, // refuse path is correct behaviour
            reasons: vec![
                format!(
                    "lite_or_unsupported_refused: {}",
                    g.refuse_code.as_deref().unwrap_or("?")
                ),
                "honesty_notice_shown".into(),
            ],
            network_used: false,
            lite_refused: g.tier == "lite" || g.refuse_code.as_deref() == Some(LITE_REFUSE_CODE),
            llm_required: false,
        });
    }

    let session_id = format!("voice-sess-{}", script_id);
    let lines: Vec<String> = if user_lines.is_empty() {
        default_voice_lines(format)
    } else {
        user_lines.to_vec()
    };

    let mut turns = Vec::new();
    let mut reasons = Vec::new();
    let mut pass = true;
    let mut network = false;

    for (i, line) in lines.iter().enumerate() {
        let role = if i % 2 == 0 { "adversary" } else { "bench" };
        let t = run_voice_turn(VoiceTurnRequest {
            session_id: session_id.clone(),
            case_id: case_id.into(),
            format: format.into(),
            turn_index: Some(i as u32 + 1),
            transcript_stub: Some(line.clone()),
            counsel_text: None,
            court_role: Some(role.into()),
            tts_role: None,
            case_graph: case_graph.clone(),
            retrieval_ids: Some(vec!["case_graph:chronology[0]".into()]),
            retrieval_snippets: None,
            bench_archetype_id: None,
            bench_sliders: None,
            lexicon_pairs: Some(lexicon_pairs.clone()),
            force_tier: force_tier.map(|s| s.to_string()),
            language: Some(PRIMARY_LANGUAGE.into()),
        })?;

        if t.network_used || t.cloud {
            pass = false;
            network = true;
            reasons.push("AC-9.3 network/cloud on voice path".into());
        }
        if !t.within_ac33 {
            pass = false;
            reasons.push(format!("AC-3.3 turn {} ms > {}s", t.elapsed_ms, TURN_MAX_S));
        }
        if t.court_turn.sources.iter().any(|s| s == "raw_document") {
            pass = false;
            reasons.push("AC-2.1 raw_document in court turn".into());
        }
        turns.push(t);
    }

    let debrief = build_debrief(
        &session_id,
        case_id,
        format,
        &turns,
        &g.tier,
        g.live_simulation_allowed,
    );
    if debrief.network_used_any {
        pass = false;
        reasons.push("debrief reports network".into());
    }
    if turns.len() < 2 {
        pass = false;
        reasons.push("need multi-turn voice rehearsal (≥2)".into());
    }
    if pass {
        reasons.push("voice multi-turn + debrief ok (offline)".into());
    }

    Ok(ScriptedVoiceReport {
        pipeline_id: PIPELINE_ID.into(),
        format: format.into(),
        script_id: script_id.into(),
        case_id: case_id.into(),
        turns,
        debrief,
        pass,
        reasons,
        network_used: network,
        lite_refused: false,
        llm_required: false,
    })
}

fn default_voice_lines(format: &str) -> Vec<String> {
    vec![
        format!(
            "My Lord, on this {format} application I seek the relief as pleaded on the case graph."
        ),
        "The principal exhibit and chronology on the graph support urgency and balance of convenience.".into(),
        "I close without predicting the outcome and invite the court to mark the open gap for counsel to cure.".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> Value {
        mock_court::sample_case_graph("voice-c1")
    }

    #[test]
    fn lite_refuses_with_honesty() {
        let g = gate(Some("lite"));
        assert!(!g.allowed);
        assert!(g.honesty_notice.unwrap().contains("Lite tier"));
        assert_eq!(g.refuse_code.as_deref(), Some(LITE_REFUSE_CODE));
    }

    #[test]
    fn full_allows() {
        let g = gate(Some("full"));
        assert!(g.allowed);
        assert!(g.live_simulation_allowed);
    }

    #[test]
    fn multi_turn_offline_pass() {
        let rep = run_scripted_voice_session(
            "Bail_Interim_Motion",
            "voice-c1",
            Some(graph()),
            "script-voice-01",
            &[],
            vec![("my lord".into(), "My Lord".into())],
            Some("full"),
        )
        .unwrap();
        assert!(rep.pass, "{:?}", rep.reasons);
        assert!(!rep.network_used);
        assert!(rep.turns.len() >= 2);
        assert!(rep.debrief.turns_completed >= 2);
        for t in &rep.turns {
            assert!(!t.network_used);
            assert!(t.within_ac33);
            assert!(TTS_ROLES.contains(&t.tts_role.as_str()));
        }
    }

    #[test]
    fn language_pin() {
        let err = run_voice_turn(VoiceTurnRequest {
            session_id: "s".into(),
            case_id: "c".into(),
            format: "Final_Arguments".into(),
            turn_index: Some(1),
            transcript_stub: Some("hello".into()),
            counsel_text: None,
            court_role: None,
            tts_role: None,
            case_graph: Some(graph()),
            retrieval_ids: None,
            retrieval_snippets: None,
            bench_archetype_id: None,
            bench_sliders: None,
            lexicon_pairs: None,
            force_tier: Some("full".into()),
            language: Some("Hindi".into()),
        })
        .unwrap_err();
        assert!(err.contains("CF_SPEECH_LANGUAGE_PINNED"));
    }

    #[test]
    fn lite_script_reports_refuse() {
        let rep = run_scripted_voice_session(
            "Motion_Generic",
            "c",
            Some(graph()),
            "lite-script",
            &[],
            vec![],
            Some("lite"),
        )
        .unwrap();
        assert!(rep.lite_refused);
        assert!(rep.pass);
        assert!(rep.turns.is_empty());
    }
}
