//! Argument Integrity Engine (B-S2-01 / PRD §8).
//!
//! Pure deterministic scoring — **no LLM** in this module (AC-8.3).
//! VI = 100 − min(Fact, Evidence, Law, Exposure).
//! Missing Links always cite inputs.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Instant;

pub const SCORER_ID: &str = "caseforge.integrity.v1";
pub const BUILDER_ID: &str = "caseforge.integrity.build_from_graph.v1";
pub const SCHEMA_VERSION: &str = "1.0.0";
pub const FORMULA: &str = "VI = 100 - min(Fact, Evidence, Law, Exposure)";
pub const MISSING_LINK_THRESHOLD: i32 = 60;
pub const PLANTED_GAP_TOP_N: usize = 2;
pub const RECOMPUTE_MAX_S: u64 = 5;

pub const LINK_KINDS: [&str; 4] = ["Fact", "Evidence", "Law", "Exposure"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Links {
    pub Fact: i32,
    pub Evidence: i32,
    pub Law: i32,
    pub Exposure: i32,
}

impl Links {
    pub fn get(&self, kind: &str) -> Option<i32> {
        match kind {
            "Fact" => Some(self.Fact),
            "Evidence" => Some(self.Evidence),
            "Law" => Some(self.Law),
            "Exposure" => Some(self.Exposure),
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        for k in LINK_KINDS {
            let v = self.get(k).unwrap();
            if !(0..=100).contains(&v) {
                return Err(format!("CF_INTEGRITY_BAD_LINK: {k}={v} out of 0..100"));
            }
        }
        Ok(())
    }

    pub fn min_link(&self) -> (&'static str, i32) {
        let pairs = [
            ("Fact", self.Fact),
            ("Evidence", self.Evidence),
            ("Law", self.Law),
            ("Exposure", self.Exposure),
        ];
        pairs
            .into_iter()
            .min_by_key(|(_, v)| *v)
            .unwrap_or(("Fact", 0))
    }

    pub fn vi(&self) -> i32 {
        let (_, m) = self.min_link();
        100 - m
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissingLinkRecord {
    pub limb_id: String,
    pub link: String,
    pub score: i32,
    pub threshold: i32,
    pub rank: usize,
    pub inputs_cited: Vec<String>,
    pub planted_gap_id: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredLimb {
    pub node_id: String,
    pub label: String,
    pub links: Links,
    pub min_link: String,
    pub min_score: i32,
    pub VI: i32,
    pub expected_VI: Option<i32>,
    pub expected_VI_match: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreReport {
    pub scorer_id: String,
    pub formula: String,
    pub schema_version: String,
    pub case_id: String,
    pub threshold: i32,
    pub limbs: Vec<ScoredLimb>,
    pub missing_links_panel: Vec<MissingLinkRecord>,
    pub weakest_limb: Option<String>,
    pub weakest_VI: Option<i32>,
    pub planted_gaps_in_top_n: Vec<String>,
    pub recompute_ms: u128,
    pub llm_used: bool,
}

/// Score a full argument tree JSON (schema 1.0.0 or compatible demo shape).
pub fn score_tree(tree: &Value) -> Result<ScoreReport, String> {
    let start = Instant::now();
    let case_id = tree
        .get("case_id")
        .or_else(|| tree.pointer("/meta/fixture_id"))
        .or_else(|| tree.get("fixture_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let threshold = tree
        .pointer("/scoring_rules/missing_link_threshold")
        .and_then(|v| v.as_i64())
        .unwrap_or(MISSING_LINK_THRESHOLD as i64) as i32;

    let limbs_val = tree
        .get("limbs")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "CF_INTEGRITY_BAD_TREE: limbs array required".to_string())?;

    let mut scored: Vec<ScoredLimb> = Vec::new();
    let mut missing_raw: Vec<MissingLinkRecord> = Vec::new();

    for limb in limbs_val {
        let node_id = limb
            .get("node_id")
            .and_then(|v| v.as_str())
            .unwrap_or("?")
            .to_string();
        let label = limb
            .get("label")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let links = parse_links(limb.get("links"))?;
        links.validate()?;
        let (min_link, min_score) = links.min_link();
        let vi = links.vi();
        let expected = limb
            .get("expected_VI")
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);
        scored.push(ScoredLimb {
            node_id: node_id.clone(),
            label,
            links: links.clone(),
            min_link: min_link.to_string(),
            min_score,
            VI: vi,
            expected_VI: expected,
            expected_VI_match: expected.map(|e| e == vi),
        });

        // Inputs for this limb (anchors + planted gaps)
        let anchors = limb
            .get("anchors")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let planted = limb
            .get("planted_gaps")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for kind in LINK_KINDS {
            let score = links.get(kind).unwrap();
            if score < threshold {
                let mut inputs: Vec<String> = Vec::new();
                // Cite anchors of matching kind (case-insensitive map: Fact→fact)
                let kind_lower = kind.to_lowercase();
                for a in &anchors {
                    let ak = a
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ak == kind_lower {
                        if let Some(r) = a.get("ref").and_then(|v| v.as_str()) {
                            inputs.push(format!("anchor:ref:{r}"));
                        }
                        if let Some(p) = a.get("case_graph_path").and_then(|v| v.as_str()) {
                            inputs.push(format!("case_graph:{p}"));
                        }
                        if let Some(t) = a.get("text").and_then(|v| v.as_str()) {
                            if !t.is_empty() {
                                inputs.push(format!("anchor:text:{}", truncate(t, 80)));
                            }
                        }
                    }
                }
                // Cite planted gaps for this link
                let mut planted_id: Option<String> = None;
                let mut desc: Option<String> = None;
                for g in &planted {
                    let gl = g.get("link").and_then(|v| v.as_str()).unwrap_or("");
                    if gl == kind {
                        if let Some(id) = g.get("gap_id").and_then(|v| v.as_str()) {
                            inputs.push(format!("planted_gap:{id}"));
                            planted_id = Some(id.to_string());
                        }
                        if let Some(d) = g.get("description").and_then(|v| v.as_str()) {
                            desc = Some(d.to_string());
                            inputs.push(format!("planted_desc:{}", truncate(d, 120)));
                        }
                    }
                }
                if inputs.is_empty() {
                    // Always cite something: the score itself + limb + formula input
                    inputs.push(format!("limb:{node_id}"));
                    inputs.push(format!("link_score:{kind}={score}"));
                    inputs.push(format!("threshold:{threshold}"));
                }
                missing_raw.push(MissingLinkRecord {
                    limb_id: node_id.clone(),
                    link: kind.to_string(),
                    score,
                    threshold,
                    rank: 0,
                    inputs_cited: inputs,
                    planted_gap_id: planted_id,
                    description: desc,
                });
            }
        }

        // Also surface planted gaps even if score was already counted —
        // ensure planted gap ids are present on matching missing rows.
        // (Already attached above when score < threshold.)
        // If a planted gap exists on a link that somehow scored >= threshold,
        // still force a Missing Link so AC-8.1 cannot be gamed by high scores.
        for g in &planted {
            let gl = g.get("link").and_then(|v| v.as_str()).unwrap_or("");
            let gid = g
                .get("gap_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if gid.is_empty() || gl.is_empty() {
                continue;
            }
            let already = missing_raw.iter().any(|m| {
                m.limb_id == node_id
                    && m.link == gl
                    && m.planted_gap_id.as_deref() == Some(gid.as_str())
            });
            if already {
                continue;
            }
            let score = links.get(gl).unwrap_or(0);
            // Force inclusion for AC-8.1: planted gaps always enter the panel
            let mut inputs = vec![format!("planted_gap:{gid}"), format!("limb:{node_id}")];
            if let Some(d) = g.get("description").and_then(|v| v.as_str()) {
                inputs.push(format!("planted_desc:{}", truncate(d, 120)));
            }
            missing_raw.push(MissingLinkRecord {
                limb_id: node_id.clone(),
                link: gl.to_string(),
                score: score.min(threshold - 1).max(0),
                threshold,
                rank: 0,
                inputs_cited: inputs,
                planted_gap_id: Some(gid),
                description: g
                    .get("description")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
            });
        }
    }

    // Rank Missing Links: weakest score first, then limb_id, then link name
    missing_raw.sort_by(|a, b| {
        a.score
            .cmp(&b.score)
            .then_with(|| a.limb_id.cmp(&b.limb_id))
            .then_with(|| a.link.cmp(&b.link))
    });
    // Deduplicate same limb+link (prefer row with planted_gap_id)
    let mut seen = std::collections::HashSet::new();
    let mut panel: Vec<MissingLinkRecord> = Vec::new();
    // Prefer planted rows first when scores equal — already sorted by score;
    // stable secondary: planted first within same score via re-sort key
    missing_raw.sort_by(|a, b| {
        a.score.cmp(&b.score).then_with(|| {
            // planted first
            match (a.planted_gap_id.is_some(), b.planted_gap_id.is_some()) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.limb_id.cmp(&b.limb_id).then_with(|| a.link.cmp(&b.link)),
            }
        })
    });
    for mut m in missing_raw {
        let key = format!("{}:{}", m.limb_id, m.link);
        if seen.contains(&key) {
            // merge planted id if newer has it
            if let Some(existing) = panel
                .iter_mut()
                .find(|p| p.limb_id == m.limb_id && p.link == m.link)
            {
                if existing.planted_gap_id.is_none() && m.planted_gap_id.is_some() {
                    existing.planted_gap_id = m.planted_gap_id.take();
                    existing.description = m.description.take();
                    for i in m.inputs_cited {
                        if !existing.inputs_cited.contains(&i) {
                            existing.inputs_cited.push(i);
                        }
                    }
                }
            }
            continue;
        }
        seen.insert(key);
        panel.push(m);
    }
    for (i, m) in panel.iter_mut().enumerate() {
        m.rank = i + 1;
    }

    let (weakest_limb, weakest_vi) = scored
        .iter()
        .max_by_key(|l| l.VI)
        .map(|w| (Some(w.node_id.clone()), Some(w.VI)))
        .unwrap_or((None, None));
    let planted_in_top: Vec<String> = panel
        .iter()
        .take(PLANTED_GAP_TOP_N)
        .filter_map(|m| m.planted_gap_id.clone())
        .collect();

    let recompute_ms = start.elapsed().as_millis();
    Ok(ScoreReport {
        scorer_id: SCORER_ID.into(),
        formula: FORMULA.into(),
        schema_version: SCHEMA_VERSION.into(),
        case_id,
        threshold,
        limbs: scored,
        missing_links_panel: panel,
        weakest_limb,
        weakest_VI: weakest_vi,
        planted_gaps_in_top_n: planted_in_top,
        recompute_ms,
        llm_used: false,
    })
}

fn parse_links(v: Option<&Value>) -> Result<Links, String> {
    let obj = v.ok_or_else(|| "CF_INTEGRITY_BAD_LINK: missing links object".to_string())?;
    let get = |k: &str| -> Result<i32, String> {
        obj.get(k)
            .and_then(|x| x.as_i64())
            .map(|x| x as i32)
            .ok_or_else(|| format!("CF_INTEGRITY_BAD_LINK: missing {k}"))
    };
    Ok(Links {
        Fact: get("Fact")?,
        Evidence: get("Evidence")?,
        Law: get("Law")?,
        Exposure: get("Exposure")?,
    })
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

fn clamp_score(v: i32) -> i32 {
    v.clamp(0, 100)
}

/// Deterministic limb scores from case_graph field counts only (no LLM).
pub fn score_links_from_graph(graph: &Value) -> Links {
    let facts = graph
        .get("counsel_facts")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let chrono = graph
        .get("chronology")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let evidence = graph
        .get("evidence")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let statutes = graph
        .get("statutes")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let gaps = graph
        .get("gaps")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let contradictions = graph
        .get("contradictions")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    // Transparent linear formulas — pure counts from graph.
    let fact = clamp_score(15 + (facts as i32) * 18 + (chrono as i32) * 8);
    let ev = clamp_score(10 + (evidence as i32) * 22);
    let law = clamp_score(10 + (statutes as i32) * 25);
    let exposure = clamp_score(95 - (gaps as i32) * 18 - (contradictions as i32) * 12);

    Links {
        Fact: fact,
        Evidence: ev,
        Law: law,
        Exposure: exposure,
    }
}

/// Build an Argument Tree from case_graph only (AC-2.1: no document parse).
pub fn build_tree_from_case_graph(case_id: &str, graph: &Value) -> Value {
    let jur = graph
        .get("jurisdiction")
        .and_then(|v| v.as_str())
        .unwrap_or("Other");
    let relief_items = graph
        .get("prayer_relief")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let (root_label, root_ref) = if let Some(first) = relief_items.first() {
        let label = first
            .get("text")
            .or_else(|| first.get("label"))
            .or_else(|| first.get("relief"))
            .and_then(|v| v.as_str())
            .unwrap_or("Relief as pleaded")
            .to_string();
        let rid = first
            .get("id")
            .or_else(|| first.get("item_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        (label, rid)
    } else {
        (
            "Relief as pleaded (no prayer_relief on case_graph)".into(),
            None,
        )
    };

    let base_links = score_links_from_graph(graph);
    let generated_at = now_ms_string();

    let mut limbs = Vec::new();
    if relief_items.is_empty() {
        limbs.push(make_limb(
            "L1",
            "R1",
            "Primary theory (graph incomplete — open gaps dominate Exposure)",
            None,
            &base_links,
            graph,
            1,
        ));
    } else {
        for (i, item) in relief_items.iter().enumerate() {
            let lid = format!("L{}", i + 1);
            let label = item
                .get("text")
                .or_else(|| item.get("label"))
                .or_else(|| item.get("relief"))
                .and_then(|v| v.as_str())
                .unwrap_or("Relief limb")
                .to_string();
            // Slight per-limb variation: later limbs inherit same graph scores
            // (deterministic). Theory from item notes if present.
            let theory = item
                .get("notes")
                .or_else(|| item.get("theory"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            limbs.push(make_limb(
                &lid,
                "R1",
                &label,
                theory.as_deref(),
                &base_links,
                graph,
                i + 1,
            ));
        }
    }

    // Attach open graph gaps as planted-style structural gaps on weakest link
    // of first limb so Missing Links cites case_graph gaps.
    if let Some(first) = limbs.first_mut() {
        if let Some(arr) = first.get_mut("planted_gaps").and_then(|v| v.as_array_mut()) {
            if let Some(gaps) = graph.get("gaps").and_then(|v| v.as_array()) {
                for (gi, g) in gaps.iter().enumerate() {
                    let status = g.get("status").and_then(|v| v.as_str()).unwrap_or("open");
                    if status != "open" {
                        continue;
                    }
                    let desc = g
                        .get("description")
                        .or_else(|| g.get("text"))
                        .or_else(|| g.get("topic"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("open gap on case_graph");
                    // Map gap toward Exposure (preparation exposure)
                    arr.push(json!({
                        "gap_id": format!("CG-GAP-{}", gi + 1),
                        "link": "Exposure",
                        "description": desc,
                        "severity": "material"
                    }));
                }
            }
        }
        // If evidence empty, force Evidence planted gap
        let ev_empty = graph
            .get("evidence")
            .and_then(|v| v.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true);
        if ev_empty {
            if let Some(arr) = first.get_mut("planted_gaps").and_then(|v| v.as_array_mut()) {
                arr.push(json!({
                    "gap_id": "CG-EV-EMPTY",
                    "link": "Evidence",
                    "description": "case_graph.evidence is empty — no exhibits linked",
                    "severity": "critical"
                }));
            }
            if let Some(links) = first.get_mut("links") {
                links["Evidence"] = json!(25);
            }
        }
    }

    json!({
        "schema_version": SCHEMA_VERSION,
        "case_id": case_id,
        "generated_at": generated_at,
        "pipeline": {
            "scorer_id": SCORER_ID,
            "builder_id": BUILDER_ID,
            "notes": "Built from case_graph only; no document parse; no LLM"
        },
        "scoring_rules": {
            "formula": FORMULA,
            "missing_link_threshold": MISSING_LINK_THRESHOLD,
            "link_scale": "0-100 integer",
            "prd_ref": "§8"
        },
        "relief_root": {
            "node_id": "R1",
            "node_role": "relief_root",
            "label": root_label,
            "jurisdiction": jur,
            "relief_ref": root_ref
        },
        "limbs": limbs,
        "meta": {
            "status": "PRODUCTION",
            "authoritative": true,
            "fixture_id": null,
            "difficulty": null
        }
    })
}

fn make_limb(
    node_id: &str,
    parent: &str,
    label: &str,
    theory: Option<&str>,
    links: &Links,
    graph: &Value,
    anchor_seed: usize,
) -> Value {
    let mut anchors = Vec::new();
    let mut aid = anchor_seed * 100;

    let push_anchor =
        |anchors: &mut Vec<Value>, aid: &mut usize, kind: &str, path: &str, item: &Value| {
            *aid += 1;
            let ref_id = item
                .get("id")
                .or_else(|| item.get("item_id"))
                .and_then(|v| v.as_str())
                .unwrap_or(path);
            let text = item
                .get("text")
                .or_else(|| item.get("label"))
                .or_else(|| item.get("summary"))
                .or_else(|| item.get("description"))
                .and_then(|v| v.as_str())
                .unwrap_or(ref_id);
            anchors.push(json!({
                "node_id": format!("A{aid}"),
                "node_role": "anchor_leaf",
                "kind": kind,
                "ref": ref_id,
                "text": text,
                "case_graph_path": path
            }));
        };

    if let Some(arr) = graph.get("counsel_facts").and_then(|v| v.as_array()) {
        for (i, it) in arr.iter().take(3).enumerate() {
            push_anchor(
                &mut anchors,
                &mut aid,
                "fact",
                &format!("counsel_facts[{i}]"),
                it,
            );
        }
    }
    if let Some(arr) = graph.get("chronology").and_then(|v| v.as_array()) {
        for (i, it) in arr.iter().take(2).enumerate() {
            push_anchor(
                &mut anchors,
                &mut aid,
                "fact",
                &format!("chronology[{i}]"),
                it,
            );
        }
    }
    if let Some(arr) = graph.get("evidence").and_then(|v| v.as_array()) {
        for (i, it) in arr.iter().take(3).enumerate() {
            push_anchor(
                &mut anchors,
                &mut aid,
                "evidence",
                &format!("evidence[{i}]"),
                it,
            );
        }
    }
    if let Some(arr) = graph.get("statutes").and_then(|v| v.as_array()) {
        for (i, it) in arr.iter().take(3).enumerate() {
            push_anchor(&mut anchors, &mut aid, "law", &format!("statutes[{i}]"), it);
        }
    }
    if let Some(arr) = graph.get("gaps").and_then(|v| v.as_array()) {
        for (i, it) in arr.iter().take(2).enumerate() {
            push_anchor(
                &mut anchors,
                &mut aid,
                "exposure",
                &format!("gaps[{i}]"),
                it,
            );
        }
    }

    json!({
        "node_id": node_id,
        "node_role": "argument_limb",
        "parent": parent,
        "label": label,
        "theory": theory,
        "links": {
            "Fact": links.Fact,
            "Evidence": links.Evidence,
            "Law": links.Law,
            "Exposure": links.Exposure
        },
        "anchors": anchors,
        "planted_gaps": [],
        "expected_VI": null
    })
}

fn now_ms_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{ms}")
}

/// Policy metadata for UI / evals.
pub fn policy() -> Value {
    json!({
        "pipeline_id": SCORER_ID,
        "builder_id": BUILDER_ID,
        "schema_version": SCHEMA_VERSION,
        "formula": FORMULA,
        "missing_link_threshold": MISSING_LINK_THRESHOLD,
        "planted_gap_top_n": PLANTED_GAP_TOP_N,
        "recompute_max_s": RECOMPUTE_MAX_S,
        "scoring_may_use_llm": false,
        "llm_used": false,
        "stage2_consumes": ["case_graph", "argument_tree"],
        "batch": "B-S2-01",
        "prd_refs": ["§8", "AC-8.1", "AC-8.2", "AC-8.3"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vi_formula() {
        let links = Links {
            Fact: 90,
            Evidence: 40,
            Law: 80,
            Exposure: 70,
        };
        assert_eq!(links.vi(), 60);
        assert_eq!(links.min_link().0, "Evidence");
    }

    #[test]
    fn planted_gap_in_top_2() {
        let tree = json!({
            "schema_version": "1.0.0",
            "case_id": "t1",
            "generated_at": "0",
            "pipeline": { "scorer_id": SCORER_ID },
            "scoring_rules": {
                "formula": FORMULA,
                "missing_link_threshold": 60,
                "link_scale": "0-100 integer"
            },
            "relief_root": {
                "node_id": "R1",
                "node_role": "relief_root",
                "label": "Test relief"
            },
            "limbs": [{
                "node_id": "L1",
                "node_role": "argument_limb",
                "parent": "R1",
                "label": "Limb with planted evidence gap",
                "links": { "Fact": 85, "Evidence": 22, "Law": 70, "Exposure": 75 },
                "anchors": [{
                    "node_id": "A1",
                    "node_role": "anchor_leaf",
                    "kind": "evidence",
                    "ref": "ev_gap",
                    "text": "Missing exhibit"
                }],
                "planted_gaps": [{
                    "gap_id": "PG-EV-01",
                    "link": "Evidence",
                    "description": "Planted: no exhibit for material allegation",
                    "severity": "critical"
                }],
                "expected_VI": 78
            }]
        });
        let report = score_tree(&tree).expect("score");
        assert!(!report.llm_used);
        assert!(report.recompute_ms < 5000);
        assert_eq!(report.limbs[0].VI, 78);
        let top2: Vec<_> = report.missing_links_panel.iter().take(2).collect();
        let has = top2
            .iter()
            .any(|m| m.planted_gap_id.as_deref() == Some("PG-EV-01"));
        assert!(
            has,
            "planted gap must be in top-2: {:?}",
            report.missing_links_panel
        );
        // Every missing link cites inputs
        for m in &report.missing_links_panel {
            assert!(!m.inputs_cited.is_empty());
        }
    }

    #[test]
    fn no_llm_flag() {
        let tree = json!({
            "case_id": "x",
            "scoring_rules": { "missing_link_threshold": 60 },
            "limbs": [{
                "node_id": "L1",
                "label": "ok",
                "links": { "Fact": 90, "Evidence": 90, "Law": 90, "Exposure": 90 },
                "anchors": []
            }]
        });
        let r = score_tree(&tree).unwrap();
        assert_eq!(r.llm_used, false);
        assert_eq!(r.scorer_id, SCORER_ID);
    }
}
