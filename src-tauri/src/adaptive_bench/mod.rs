//! Per-Case Adaptive Bench (B-S2-03 / PRD §10).
//!
//! Six generic archetypes + case-order signals + user dial-in sliders.
//! **No named-judge profiles ever** (§20 non-goal).
//! Signals come only from this case's case_graph (AC-2.1 / AC-10.2).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const PIPELINE_ID: &str = "caseforge.adaptive_bench.v1";
pub const SCHEMA_VERSION: &str = "1.0.0";
pub const NAMED_JUDGE_ERROR: &str = "CF_BENCH_NO_NAMED_JUDGE";

pub const ARCHETYPE_IDS: [&str; 6] = [
    "procedure_first",
    "evidence_heavy",
    "textualist",
    "relief_focused",
    "skeptical_probe",
    "settlement_lens",
];

pub const SLIDER_KEYS: [&str; 5] = [
    "tempo",
    "formality",
    "opposition_aggression",
    "technicality",
    "interruption",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchetypeMeta {
    pub id: String,
    pub label: String,
    pub baseline: SliderSet,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SliderSet {
    pub tempo: u8,
    pub formality: u8,
    pub opposition_aggression: u8,
    pub technicality: u8,
    pub interruption: u8,
}

impl Default for SliderSet {
    fn default() -> Self {
        Self {
            tempo: 50,
            formality: 50,
            opposition_aggression: 50,
            technicality: 50,
            interruption: 50,
        }
    }
}

impl SliderSet {
    pub fn clamp_all(mut self) -> Self {
        self.tempo = self.tempo.min(100);
        self.formality = self.formality.min(100);
        self.opposition_aggression = self.opposition_aggression.min(100);
        self.technicality = self.technicality.min(100);
        self.interruption = self.interruption.min(100);
        self
    }

    pub fn get(&self, key: &str) -> Option<u8> {
        match key {
            "tempo" => Some(self.tempo),
            "formality" => Some(self.formality),
            "opposition_aggression" => Some(self.opposition_aggression),
            "technicality" => Some(self.technicality),
            "interruption" => Some(self.interruption),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseOrderSignals {
    pub jurisdiction: String,
    pub relief_count: usize,
    pub open_gap_count: usize,
    pub evidence_count: usize,
    pub statute_count: usize,
    pub chronology_density: usize,
    pub has_procedural_posture: bool,
    pub urgency_hint: bool,
    pub suggested_archetype: String,
    /// Deterministic fingerprint of signal vector (AC-10.2).
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchProfile {
    pub pipeline_id: String,
    pub schema_version: String,
    pub case_id: String,
    pub archetype_id: String,
    pub archetype_label: String,
    pub sliders: SliderSet,
    pub signals: CaseOrderSignals,
    pub behaviour_brief: String,
    pub named_judge_profiles: bool,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResolveRequest {
    pub case_id: String,
    pub case_graph: Option<Value>,
    pub archetype_id: Option<String>,
    pub sliders: Option<SliderSet>,
    /// Forbidden: any attempt to pass a real judge name.
    pub judge_name: Option<String>,
    pub named_judge_id: Option<String>,
}

pub fn policy() -> Value {
    json!({
        "pipeline_id": PIPELINE_ID,
        "schema_version": SCHEMA_VERSION,
        "batch": "B-S2-03",
        "named_judge_profiles": false,
        "named_judge_error": NAMED_JUDGE_ERROR,
        "archetypes": archetypes_meta(),
        "sliders": SLIDER_KEYS,
        "slider_min": 0,
        "slider_max": 100,
        "consumes_only": ["case_graph", "integrity_tree", "user_sliders"],
        "forbidden": [
            "named_judge_profiles",
            "judge_databases",
            "real_person_judicial_profiling",
            "raw_document_parse"
        ],
        "prd_refs": ["§10", "AC-10.1", "AC-10.2", "§20"],
        "acceptance": {
            "AC-10.1": [
                "exactly_six_generic_archetypes",
                "named_judge_profiles_false",
                "refuse_named_judge_fields",
                "no_judge_database"
            ],
            "AC-10.2": [
                "signals_from_this_case_graph_only",
                "sliders_change_behaviour_brief",
                "deterministic_fingerprint",
                "opposition_aggression_control"
            ]
        }
    })
}

pub fn archetypes_meta() -> Vec<ArchetypeMeta> {
    vec![
        ArchetypeMeta {
            id: "procedure_first".into(),
            label: "Procedure-first".into(),
            baseline: SliderSet {
                tempo: 45,
                formality: 75,
                opposition_aggression: 40,
                technicality: 55,
                interruption: 35,
            },
        },
        ArchetypeMeta {
            id: "evidence_heavy".into(),
            label: "Evidence-heavy".into(),
            baseline: SliderSet {
                tempo: 50,
                formality: 55,
                opposition_aggression: 55,
                technicality: 60,
                interruption: 45,
            },
        },
        ArchetypeMeta {
            id: "textualist".into(),
            label: "Textualist / statutory".into(),
            baseline: SliderSet {
                tempo: 40,
                formality: 70,
                opposition_aggression: 45,
                technicality: 85,
                interruption: 30,
            },
        },
        ArchetypeMeta {
            id: "relief_focused".into(),
            label: "Relief-focused".into(),
            baseline: SliderSet {
                tempo: 55,
                formality: 50,
                opposition_aggression: 50,
                technicality: 45,
                interruption: 40,
            },
        },
        ArchetypeMeta {
            id: "skeptical_probe".into(),
            label: "Skeptical probe".into(),
            baseline: SliderSet {
                tempo: 65,
                formality: 45,
                opposition_aggression: 80,
                technicality: 55,
                interruption: 70,
            },
        },
        ArchetypeMeta {
            id: "settlement_lens".into(),
            label: "Settlement lens".into(),
            baseline: SliderSet {
                tempo: 35,
                formality: 50,
                opposition_aggression: 25,
                technicality: 40,
                interruption: 25,
            },
        },
    ]
}

fn archetype_label(id: &str) -> String {
    archetypes_meta()
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.label)
        .unwrap_or_else(|| id.to_string())
}

fn baseline_for(id: &str) -> SliderSet {
    archetypes_meta()
        .into_iter()
        .find(|a| a.id == id)
        .map(|a| a.baseline)
        .unwrap_or_default()
}

fn validate_archetype(id: &str) -> Result<(), String> {
    if ARCHETYPE_IDS.contains(&id) {
        Ok(())
    } else {
        Err(format!(
            "CF_BENCH_BAD_ARCHETYPE: {id}; expected one of {}",
            ARCHETYPE_IDS.join(", ")
        ))
    }
}

/// Extract case-order signals from this case's graph only.
pub fn extract_signals(graph: Option<&Value>) -> CaseOrderSignals {
    let g = graph.unwrap_or(&Value::Null);
    let jurisdiction = g
        .get("jurisdiction")
        .and_then(|v| v.as_str())
        .unwrap_or("Other")
        .to_string();
    let relief_count = g
        .get("prayer_relief")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let open_gap_count = g
        .get("gaps")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter(|it| it.get("status").and_then(|s| s.as_str()).unwrap_or("open") == "open")
                .count()
        })
        .unwrap_or(0);
    let evidence_count = g
        .get("evidence")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let statute_count = g
        .get("statutes")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let chronology_density = g
        .get("chronology")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let has_procedural_posture = g
        .get("procedural_posture")
        .map(|v| !v.is_null())
        .unwrap_or(false);

    // Urgency from this case's relief/posture text only
    let mut urgency_blob = String::new();
    if let Some(arr) = g.get("prayer_relief").and_then(|v| v.as_array()) {
        for it in arr {
            if let Some(t) = it
                .get("text")
                .or_else(|| it.get("label"))
                .and_then(|v| v.as_str())
            {
                urgency_blob.push_str(t);
                urgency_blob.push(' ');
            }
        }
    }
    if let Some(pp) = g.get("procedural_posture") {
        urgency_blob.push_str(&pp.to_string());
    }
    let low = urgency_blob.to_lowercase();
    let urgency_hint = [
        "interim",
        "bail",
        "injunction",
        "stay",
        "urgent",
        "ex-parte",
        "ex parte",
    ]
    .iter()
    .any(|k| low.contains(k));

    let suggested = suggest_archetype(
        open_gap_count,
        evidence_count,
        statute_count,
        urgency_hint,
        relief_count,
    );

    let mut h = Sha256::new();
    h.update(jurisdiction.as_bytes());
    h.update([0]);
    h.update(relief_count.to_string().as_bytes());
    h.update(open_gap_count.to_string().as_bytes());
    h.update(evidence_count.to_string().as_bytes());
    h.update(statute_count.to_string().as_bytes());
    h.update(chronology_density.to_string().as_bytes());
    h.update([has_procedural_posture as u8]);
    h.update([urgency_hint as u8]);
    h.update(suggested.as_bytes());
    let fingerprint = hex::encode(h.finalize())[..16].to_string();

    CaseOrderSignals {
        jurisdiction,
        relief_count,
        open_gap_count,
        evidence_count,
        statute_count,
        chronology_density,
        has_procedural_posture,
        urgency_hint,
        suggested_archetype: suggested,
        fingerprint,
    }
}

fn suggest_archetype(
    gaps: usize,
    evidence: usize,
    statutes: usize,
    urgency: bool,
    relief: usize,
) -> String {
    if urgency {
        return "relief_focused".into();
    }
    if gaps >= 2 {
        return "skeptical_probe".into();
    }
    if evidence == 0 {
        return "evidence_heavy".into();
    }
    if statutes >= 2 {
        return "textualist".into();
    }
    if relief >= 2 {
        return "procedure_first".into();
    }
    if gaps == 0 && evidence >= 2 {
        return "settlement_lens".into();
    }
    "procedure_first".into()
}

/// Merge archetype baseline with user slider overrides (user wins when provided as full set).
pub fn merge_sliders(archetype_id: &str, user: Option<&SliderSet>) -> SliderSet {
    let base = baseline_for(archetype_id);
    match user {
        None => base,
        Some(u) => SliderSet {
            // If user sends a full set, use it; still clamp.
            tempo: u.tempo,
            formality: u.formality,
            opposition_aggression: u.opposition_aggression,
            technicality: u.technicality,
            interruption: u.interruption,
        }
        .clamp_all(),
    }
}

pub fn build_behaviour_brief(
    archetype_id: &str,
    label: &str,
    sliders: &SliderSet,
    signals: &CaseOrderSignals,
) -> String {
    let agg = sliders.opposition_aggression;
    let aggression_band = if agg >= 70 {
        "high pressure — short follow-ups, demand concessions"
    } else if agg >= 40 {
        "moderate pressure — probe weak limbs, allow structured answers"
    } else {
        "low pressure — issue-narrowing, settlement-compatible tone"
    };
    let tempo_band = if sliders.tempo >= 65 {
        "rapid multi-question bursts"
    } else if sliders.tempo >= 40 {
        "measured one-issue turns"
    } else {
        "deliberate, few questions per turn"
    };
    let formality_band = if sliders.formality >= 65 {
        "high formality (structured, courtly register)"
    } else if sliders.formality >= 40 {
        "professional neutral register"
    } else {
        "plain, conversational bench register"
    };
    let tech = if sliders.technicality >= 65 {
        "insist on elements / authority on the graph"
    } else {
        "keep elements light; prefer facts and relief"
    };
    let interrupt = if sliders.interruption >= 65 {
        "redirect often when counsel digresses"
    } else {
        "allow fuller submissions before redirect"
    };

    format!(
        "Adaptive Bench profile (generic archetype — not a named judge)\n\
         Archetype: {label} (`{archetype_id}`)\n\
         Case signals: jurisdiction={jur}; relief={rc}; gaps={gc}; evidence={ec}; \
statutes={sc}; chronology={cd}; procedural_posture={pp}; urgency={uh}\n\
         Fingerprint: {fp}\n\
         Sliders: tempo={t} formality={f} opposition_aggression={oa} technicality={te} interruption={i}\n\
         Behaviour:\n\
         - Opposition aggression: {aggression_band}\n\
         - Tempo: {tempo_band}\n\
         - Formality: {formality_band}\n\
         - Technicality: {tech}\n\
         - Interruption: {interrupt}\n\
         Rules: use only this case's case_graph anchors; never invent a judicial identity; never predict verdicts.",
        label = label,
        archetype_id = archetype_id,
        jur = signals.jurisdiction,
        rc = signals.relief_count,
        gc = signals.open_gap_count,
        ec = signals.evidence_count,
        sc = signals.statute_count,
        cd = signals.chronology_density,
        pp = signals.has_procedural_posture,
        uh = signals.urgency_hint,
        fp = signals.fingerprint,
        t = sliders.tempo,
        f = sliders.formality,
        oa = sliders.opposition_aggression,
        te = sliders.technicality,
        i = sliders.interruption,
        aggression_band = aggression_band,
        tempo_band = tempo_band,
        formality_band = formality_band,
        tech = tech,
        interrupt = interrupt,
    )
}

pub fn resolve(req: ResolveRequest) -> Result<BenchProfile, String> {
    // AC-10.1: hard refuse named-judge fields
    if req
        .judge_name
        .as_ref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
        || req
            .named_judge_id
            .as_ref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false)
    {
        return Err(format!(
            "{NAMED_JUDGE_ERROR}: named-judge profiles are a non-goal (PRD §20). Use generic archetypes only."
        ));
    }

    let signals = extract_signals(req.case_graph.as_ref());
    let archetype_id = req
        .archetype_id
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| signals.suggested_archetype.clone());
    validate_archetype(&archetype_id)?;
    let label = archetype_label(&archetype_id);
    let sliders = merge_sliders(&archetype_id, req.sliders.as_ref());
    let brief = build_behaviour_brief(&archetype_id, &label, &sliders, &signals);

    Ok(BenchProfile {
        pipeline_id: PIPELINE_ID.into(),
        schema_version: SCHEMA_VERSION.into(),
        case_id: req.case_id,
        archetype_id,
        archetype_label: label,
        sliders,
        signals,
        behaviour_brief: brief,
        named_judge_profiles: false,
        sources: vec!["case_graph".into(), "user_sliders".into()],
    })
}

/// Apply profile to a mock-court style prompt fragment for bench role.
pub fn inject_into_prompt(base: &str, profile: &BenchProfile) -> String {
    format!(
        "{base}\n\n---\n[ADAPTIVE BENCH]\n{}\n[end adaptive bench]",
        profile.behaviour_brief
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_graph(gaps: usize, urgency: bool) -> Value {
        let mut gap_arr = Vec::new();
        for i in 0..gaps {
            gap_arr.push(json!({"id": format!("g{i}"), "status": "open", "description": "gap"}));
        }
        let relief = if urgency {
            json!([{"id": "r1", "text": "Interim injunction and stay pending hearing"}])
        } else {
            json!([{"id": "r1", "text": "Damages as pleaded"}])
        };
        json!({
            "jurisdiction": "India",
            "prayer_relief": relief,
            "gaps": gap_arr,
            "evidence": [{"id": "e1", "text": "Exhibit A"}],
            "statutes": [{"id": "s1", "label": "Act s.1"}],
            "chronology": [{"date": "2025-01-01", "text": "Event"}],
            "procedural_posture": null,
            "parties": [],
            "contradictions": [],
            "counsel_facts": []
        })
    }

    #[test]
    fn six_archetypes() {
        assert_eq!(ARCHETYPE_IDS.len(), 6);
        assert_eq!(archetypes_meta().len(), 6);
    }

    #[test]
    fn refuses_named_judge() {
        let err = resolve(ResolveRequest {
            case_id: "c".into(),
            case_graph: None,
            archetype_id: Some("procedure_first".into()),
            sliders: None,
            judge_name: Some("Hon'ble Justice Example".into()),
            named_judge_id: None,
        })
        .unwrap_err();
        assert!(err.contains("CF_BENCH_NO_NAMED_JUDGE"));
    }

    #[test]
    fn sliders_change_brief() {
        let g = sample_graph(0, false);
        let p1 = resolve(ResolveRequest {
            case_id: "c".into(),
            case_graph: Some(g.clone()),
            archetype_id: Some("skeptical_probe".into()),
            sliders: Some(SliderSet {
                tempo: 50,
                formality: 50,
                opposition_aggression: 20,
                technicality: 50,
                interruption: 50,
            }),
            judge_name: None,
            named_judge_id: None,
        })
        .unwrap();
        let p2 = resolve(ResolveRequest {
            case_id: "c".into(),
            case_graph: Some(g),
            archetype_id: Some("skeptical_probe".into()),
            sliders: Some(SliderSet {
                tempo: 50,
                formality: 50,
                opposition_aggression: 90,
                technicality: 50,
                interruption: 50,
            }),
            judge_name: None,
            named_judge_id: None,
        })
        .unwrap();
        assert_ne!(p1.behaviour_brief, p2.behaviour_brief);
        assert!(
            p2.behaviour_brief.contains("high pressure") || p2.sliders.opposition_aggression == 90
        );
        assert!(
            p1.behaviour_brief.contains("low pressure") || p1.sliders.opposition_aggression == 20
        );
    }

    #[test]
    fn fingerprint_deterministic() {
        let g = sample_graph(1, false);
        let a = extract_signals(Some(&g));
        let b = extract_signals(Some(&g));
        assert_eq!(a.fingerprint, b.fingerprint);
    }

    #[test]
    fn different_graphs_different_suggestion() {
        let g1 = sample_graph(3, false);
        let g2 = sample_graph(0, true);
        let s1 = extract_signals(Some(&g1));
        let s2 = extract_signals(Some(&g2));
        assert_eq!(s1.suggested_archetype, "skeptical_probe");
        assert_eq!(s2.suggested_archetype, "relief_focused");
    }
}
