//! Admissibility & Foundation Gaps (B-S4-10, PRD v4.0 §7.5).
//!
//! Each evidence or exhibit item in the matter is matched, by evidence type, to the `ADMISSIBILITY_REQUIREMENT` rows in
//! force; every foundation element a row requires is marked PRESENT (with where it is), ABSENT, or UNKNOWN. Gaps cite the
//! rule row and its authority and lead the Missing Links panel. Deterministic: no model, no network.
//!
//! - The evidence type is PROPOSED from the words in the item's label, kind and document name (vocabulary:
//!   packs/builtin/evidence_types.v1.json, data not code) and is COUNSEL_CONFIRMED once counsel sets it.
//! - An element is PRESENT when counsel linked a document to it, when a graph node of a required type is connected to the
//!   item by an edge that has not been rejected, or when a document carrying one of the element's type tags exists and
//!   there is no doubt which item it belongs to (the only item of that type). If such a document exists but could belong
//!   to several items it is UNKNOWN (link it). Otherwise it is ABSENT.
//! - Rows are checked for each date given (today and, if known, the date proceedings were instituted), so a matter caught
//!   between an old and a new code sees the requirement under both.
//!
//! The integrity v2 *score* change (a foundation factor in the Evidence link) is frozen in the contract as needing owner
//! approval and a schema bump before any scorer code; this module therefore changes no score. Contract:
//! contracts/foundation_check.schema.json. Golden: golden/G-ADMISS.

use crate::deadlines::Date;
use crate::rule_tables::RuleRow;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const VOCAB_JSON: &str = include_str!("../../packs/builtin/evidence_types.v1.json");
pub const SCORER_VERSION: &str = "caseforge.integrity.v1";

// ---------------------------------------------------------------------------------------------------------------------
// Inputs

#[derive(Clone, Debug)]
pub struct Evidence {
    pub evidence_id: String,
    pub label: String,
    pub kind: String,
    pub doc_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct DocInfo {
    pub doc_id: String,
    pub filename: String,
    pub type_tag: String,
}

#[derive(Clone, Debug)]
pub struct NodeRef {
    pub node_id: String,
    pub node_type: String,
    pub label: String,
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub status: String,
}

/// Counsel linked a document to one element of one evidence item.
#[derive(Clone, Debug)]
pub struct Link {
    pub evidence_id: String,
    pub element_id: String,
    pub doc_id: String,
}

/// Counsel set the evidence type of one item.
#[derive(Clone, Debug)]
pub struct TypeChoice {
    pub evidence_id: String,
    pub evidence_type: String,
}

#[derive(Clone, Debug)]
pub struct Element {
    pub element_id: String,
    pub label: String,
    pub node_types: Vec<String>,
    pub type_tags: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub rule_id: String,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub authorities: Vec<Value>,
    pub trust_state: String,
    pub evidence_type: String,
    pub elements: Vec<Element>,
}

impl Rule {
    pub fn from_parts(rule_id: &str, effective_from: &str, effective_to: Option<&str>, authorities: &[Value], trust_state: &str, payload: &Value) -> Result<Rule, String> {
        let bad = |why: &str| format!("CF_FOUNDATION_BAD_RULE: {rule_id}: {why}");
        let strings = |v: &Value, k: &str| -> Vec<String> { v.get(k).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect() };
        let elements: Vec<Element> = payload
            .get("elements")
            .and_then(Value::as_array)
            .ok_or_else(|| bad("no elements"))?
            .iter()
            .map(|e| {
                Some(Element {
                    element_id: e.get("element_id")?.as_str()?.to_string(),
                    label: e.get("label")?.as_str()?.to_string(),
                    node_types: strings(e, "satisfied_by_node_types"),
                    type_tags: strings(e, "satisfied_by_type_tags"),
                })
            })
            .collect::<Option<_>>()
            .ok_or_else(|| bad("an element lacks an id or label"))?;
        if elements.is_empty() || authorities.is_empty() {
            return Err(bad("a rule needs at least one element and one authority"));
        }
        Ok(Rule {
            rule_id: rule_id.to_string(),
            effective_from: effective_from.to_string(),
            effective_to: effective_to.map(str::to_string),
            authorities: authorities.to_vec(),
            trust_state: trust_state.to_string(),
            evidence_type: payload.get("evidence_type").and_then(Value::as_str).ok_or_else(|| bad("no evidence_type"))?.to_string(),
            elements,
        })
    }

    pub fn from_row(r: &RuleRow) -> Result<Rule, String> {
        Rule::from_parts(&r.rule_id, &r.effective_from, r.effective_to.as_deref(), &r.authorities, &r.trust_state, &r.payload)
    }

    fn in_force(&self, d: Date) -> bool {
        Date::parse(&self.effective_from).map(|f| f <= d).unwrap_or(false) && self.effective_to.as_deref().map(|t| Date::parse(t).map(|t| d <= t).unwrap_or(false)).unwrap_or(true)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Evidence types

pub struct VocabType {
    pub evidence_type: String,
    pub label: String,
    keywords: Vec<Regex>,
}

pub struct Vocab {
    pub types: Vec<VocabType>,
}

pub fn vocabulary() -> &'static Vocab {
    static V: Lazy<Vocab> = Lazy::new(|| {
        let v: Value = serde_json::from_str(VOCAB_JSON).expect("evidence_types.v1.json is valid");
        let types = v["types"]
            .as_array()
            .expect("types")
            .iter()
            .map(|t| VocabType {
                evidence_type: t["evidence_type"].as_str().expect("evidence_type").to_string(),
                label: t["label"].as_str().unwrap_or("").to_string(),
                keywords: t["keywords"]
                    .as_array()
                    .expect("keywords")
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|k| Regex::new(&format!(r"(?i)\b{}\b", regex::escape(k).replace("\\ ", r"\s+").replace(' ', r"\s+"))).expect("keyword pattern"))
                    .collect(),
            })
            .collect();
        Vocab { types }
    });
    &V
}

/// The type counsel chose, else the vocabulary type with the most distinct keyword hits (ties go to the earlier type).
/// None when nothing matches: no foundation rule applies to the item.
pub fn classify(ev: &Evidence, docs: &[DocInfo], vocab: &Vocab, chosen: &[TypeChoice]) -> Option<(String, &'static str)> {
    if let Some(c) = chosen.iter().find(|c| c.evidence_id == ev.evidence_id) {
        return Some((c.evidence_type.clone(), "COUNSEL_CONFIRMED"));
    }
    let doc_name = ev.doc_id.as_deref().and_then(|id| docs.iter().find(|d| d.doc_id == id)).map(|d| d.filename.as_str()).unwrap_or("");
    let text = format!("{} {} {}", ev.label, ev.kind, doc_name.replace(['_', '.'], " "));
    let mut best: Option<(usize, &VocabType)> = None;
    for t in &vocab.types {
        let hits = t.keywords.iter().filter(|k| k.is_match(&text)).count();
        if hits > 0 && best.map(|(h, _)| hits > h).unwrap_or(true) {
            best = Some((hits, t));
        }
    }
    best.map(|(_, t)| (t.evidence_type.clone(), "PROPOSED"))
}

// ---------------------------------------------------------------------------------------------------------------------
// Checking

pub struct Inputs<'a> {
    pub case_id: &'a str,
    pub evidence: &'a [Evidence],
    pub docs: &'a [DocInfo],
    pub nodes: &'a [NodeRef],
    pub edges: &'a [Edge],
    pub links: &'a [Link],
    pub types: &'a [TypeChoice],
    pub rules: &'a [Rule],
    /// Dates to check the rows against (today, and the institution date if known); each row is checked once.
    pub as_of: &'a [String],
    pub vocab: &'a Vocab,
}

pub struct Outcome {
    /// One record per evidence item and row, as contracts/foundation_check.schema.json.
    pub checks: Vec<Value>,
    /// One entry per element that is ABSENT or UNKNOWN, citing the row and its authority.
    pub gaps: Vec<Value>,
}

fn short_hash(seed: &str) -> String {
    hex::encode(Sha256::digest(seed.as_bytes()))[..16].to_string()
}

pub fn check_all(i: &Inputs) -> Outcome {
    let (mut checks, mut gaps) = (Vec::new(), Vec::new());
    // Items of each type in the matter: decides whether a lone matching document can be attributed to an item.
    let typed: Vec<(&Evidence, Option<(String, &'static str)>)> = i.evidence.iter().map(|e| (e, classify(e, i.docs, i.vocab, i.types))).collect();
    for (ev, ty) in &typed {
        let Some((etype, status)) = ty else { continue };
        let same_type = typed.iter().filter(|(_, t)| t.as_ref().map(|t| &t.0) == Some(etype)).count();
        let mut done: HashSet<&str> = HashSet::new();
        for as_of in i.as_of {
            let Some(date) = Date::parse(as_of) else { continue };
            for rule in i.rules.iter().filter(|r| &r.evidence_type == etype && r.in_force(date)) {
                if !done.insert(rule.rule_id.as_str()) {
                    continue;
                }
                let check_id = format!("fc_{}", short_hash(&format!("{}|{}|{}", i.case_id, ev.evidence_id, rule.rule_id)));
                let mut elements = Vec::new();
                for (n, el) in rule.elements.iter().enumerate() {
                    let authority = rule.authorities.get(n).unwrap_or(&rule.authorities[0]).clone();
                    let (state, locators) = element_state(i, ev, el, same_type);
                    if state != "PRESENT" {
                        let what = if state == "ABSENT" { "not on file" } else { "a possible document is on file but is not linked to this item" };
                        gaps.push(json!({
                            "gap_id": format!("fg_{}", short_hash(&format!("{check_id}|{}", el.element_id))),
                            "check_id": check_id, "evidence_node_id": ev.evidence_id, "evidence_label": ev.label, "evidence_type": etype,
                            "evidence_type_status": status, "element_id": el.element_id, "element_label": el.label, "status": state,
                            "rule_id": rule.rule_id, "rule_trust_state": rule.trust_state, "authority": authority, "candidates": locators,
                            "message": format!("{}: {} — {}", ev.label, el.label, what),
                        }));
                    }
                    elements.push(json!({"element_id": el.element_id, "label": el.label, "status": state, "locators": locators, "authority": authority}));
                }
                checks.push(json!({
                    "schema_version": "1.0.0", "check_id": check_id, "evidence_node_id": ev.evidence_id, "evidence_type": etype, "evidence_type_status": status,
                    "rule_id": rule.rule_id, "elements": elements, "scorer_version": SCORER_VERSION,
                }));
            }
        }
    }
    Outcome { checks, gaps }
}

fn element_state(i: &Inputs, ev: &Evidence, el: &Element, same_type: usize) -> (&'static str, Vec<Value>) {
    // 1. Counsel linked a document to this element of this item.
    if let Some(d) = i.links.iter().find(|l| l.evidence_id == ev.evidence_id && l.element_id == el.element_id).and_then(|l| i.docs.iter().find(|d| d.doc_id == l.doc_id)) {
        return ("PRESENT", vec![json!({"doc_id": d.doc_id, "excerpt": format!("{} — linked by counsel", d.filename)})]);
    }
    // 2. A node of a required type is connected to the item by an edge that has not been rejected.
    let connected: Vec<&NodeRef> = i
        .nodes
        .iter()
        .filter(|n| el.node_types.contains(&n.node_type) && n.node_id != ev.evidence_id)
        .filter(|n| i.edges.iter().any(|e| e.status != "rejected" && ((e.from == n.node_id && e.to == ev.evidence_id) || (e.to == n.node_id && e.from == ev.evidence_id))))
        .collect();
    if !connected.is_empty() {
        return ("PRESENT", connected.iter().map(|n| json!({"doc_id": n.node_id, "excerpt": n.label})).collect());
    }
    // 3. Documents carrying one of the element's type tags.
    let candidates: Vec<&DocInfo> = i.docs.iter().filter(|d| el.type_tags.iter().any(|t| t.eq_ignore_ascii_case(&d.type_tag))).collect();
    if candidates.is_empty() {
        return ("ABSENT", Vec::new());
    }
    if same_type == 1 {
        return ("PRESENT", candidates.iter().map(|d| json!({"doc_id": d.doc_id, "excerpt": format!("{} — the document tagged {} in this matter", d.filename, d.type_tag)})).collect());
    }
    ("UNKNOWN", candidates.iter().map(|d| json!({"doc_id": d.doc_id, "excerpt": format!("{} — tagged {}; link it to this item if it belongs to it", d.filename, d.type_tag)})).collect())
}

/// The Missing Links panel with foundation gaps first: a missing mandatory foundation can bar the evidence outright.
/// Only ABSENT elements enter the panel (UNKNOWN ones are listed separately for counsel to resolve); gaps on a counsel-
/// confirmed evidence type come before gaps on a proposed type. The existing link-score entries keep their order. Display
/// ordering only: no score changes.
pub fn merge_panel(v1: &[Value], gaps: &[Value]) -> Vec<Value> {
    let mut absent: Vec<&Value> = gaps.iter().filter(|g| g["status"] == "ABSENT").collect();
    absent.sort_by(|a, b| {
        let key = |g: &Value| (g["evidence_type_status"] != "COUNSEL_CONFIRMED", g["evidence_node_id"].as_str().unwrap_or("").to_string(), g["rule_id"].as_str().unwrap_or("").to_string(), g["element_id"].as_str().unwrap_or("").to_string());
        key(a).cmp(&key(b))
    });
    let mut out: Vec<Value> = Vec::new();
    for g in absent {
        let mut e = g.clone();
        e["panel_id"] = json!(g["gap_id"]);
        e["kind"] = json!("FOUNDATION");
        out.push(e);
    }
    for m in v1 {
        let mut e = m.clone();
        e["panel_id"] = json!(format!("{}:{}", m["limb_id"].as_str().unwrap_or(""), m["link"].as_str().unwrap_or("")));
        e["kind"] = json!("LINK");
        out.push(e);
    }
    for (n, e) in out.iter_mut().enumerate() {
        e["rank"] = json!(n + 1);
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FIXTURE: &str = include_str!("../../golden/G-ADMISS/fixtures/foundation_core.json");
    const FIXTURE_SHA256: &str = "140b5ea0e5c04a5ab1a46425810b26b87ec70c65317e9b2f0f51c0ea6a0a3c79"; // committed before the engine (fb502dc)
    const VOCAB_SHA256: &str = "0c03e4c28c466c360ffc109e14416dfb8fe46f17ee81621443d9d0fd6a6df67b";

    fn s(v: &Value, k: &str) -> String {
        v[k].as_str().unwrap_or("").to_string()
    }

    fn rules(fx: &Value) -> Vec<Rule> {
        fx["rules"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| Rule::from_parts(r["rule_id"].as_str().unwrap(), r["effective_from"].as_str().unwrap(), r["effective_to"].as_str(), r["authorities"].as_array().unwrap(), r["trust_state"].as_str().unwrap(), &r["payload"]).unwrap())
            .collect()
    }

    #[test]
    fn golden_foundation_cases() {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "G-ADMISS fixture changed since it was pinned");
        assert_eq!(hex::encode(Sha256::digest(VOCAB_JSON.as_bytes())), VOCAB_SHA256, "the evidence-type vocabulary changed since the golden was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let mut all_checks = Vec::new();
        for c in fx["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let rs = rules(&json!({"rules": c["rules"]}));
            let evidence: Vec<Evidence> = c["evidence"].as_array().unwrap().iter().map(|e| Evidence { evidence_id: s(e, "evidence_id"), label: s(e, "label"), kind: s(e, "kind"), doc_id: e["doc_id"].as_str().map(str::to_string) }).collect();
            let docs: Vec<DocInfo> = c["docs"].as_array().unwrap().iter().map(|d| DocInfo { doc_id: s(d, "doc_id"), filename: s(d, "filename"), type_tag: s(d, "type_tag") }).collect();
            let nodes: Vec<NodeRef> = c["nodes"].as_array().unwrap().iter().map(|n| NodeRef { node_id: s(n, "node_id"), node_type: s(n, "node_type"), label: s(n, "label") }).collect();
            let edges: Vec<Edge> = c["edges"].as_array().unwrap().iter().map(|e| Edge { from: s(e, "from_node_id"), to: s(e, "to_node_id"), status: s(e, "status") }).collect();
            let links: Vec<Link> = c["links"].as_array().unwrap().iter().map(|l| Link { evidence_id: s(l, "evidence_id"), element_id: s(l, "element_id"), doc_id: s(l, "doc_id") }).collect();
            let types: Vec<TypeChoice> = c["types"].as_array().unwrap().iter().map(|t| TypeChoice { evidence_id: s(t, "evidence_id"), evidence_type: s(t, "evidence_type") }).collect();
            let as_of: Vec<String> = c["as_of"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().to_string()).collect();
            let inputs = Inputs { case_id: "case-1", evidence: &evidence, docs: &docs, nodes: &nodes, edges: &edges, links: &links, types: &types, rules: &rs, as_of: &as_of, vocab: vocabulary() };
            let out = check_all(&inputs);
            let again = check_all(&inputs);
            assert_eq!(out.checks, again.checks, "{name}: deterministic");
            let expect = c["expect"].as_array().unwrap();
            assert_eq!(out.checks.len(), expect.len(), "{name}: number of checks");
            let mut unresolved = 0;
            for (chk, e) in out.checks.iter().zip(expect) {
                assert_eq!((chk["evidence_node_id"].as_str(), chk["evidence_type"].as_str(), chk["evidence_type_status"].as_str(), chk["rule_id"].as_str()),
                           (e["evidence_id"].as_str(), e["evidence_type"].as_str(), e["evidence_type_status"].as_str(), e["rule_id"].as_str()), "{name}: check header");
                let elements = chk["elements"].as_array().unwrap();
                assert_eq!(elements.len(), e["elements"].as_object().unwrap().len(), "{name}: elements");
                for el in elements {
                    let id = el["element_id"].as_str().unwrap();
                    assert_eq!(el["status"], e["elements"][id], "{name}: {} / {id}", chk["evidence_node_id"]);
                    if el["status"] == "PRESENT" {
                        assert!(!el["locators"].as_array().unwrap().is_empty(), "{name}: PRESENT carries a locator");
                    } else {
                        unresolved += 1;
                    }
                    // AC-S4-05.3: every element cites an authority.
                    for k in ["pack_id", "source_id", "chunk_id"] {
                        assert!(!el["authority"][k].as_str().unwrap_or("").is_empty(), "{name}: authority {k}");
                    }
                }
                assert_eq!((chk["scorer_version"].as_str(), chk["schema_version"].as_str()), (Some(SCORER_VERSION), Some("1.0.0")));
            }
            assert_eq!(out.gaps.len(), unresolved, "{name}: one gap per element that is not PRESENT");
            for g in &out.gaps {
                assert!(!g["rule_id"].as_str().unwrap().is_empty() && !g["authority"]["chunk_id"].as_str().unwrap().is_empty() && !g["message"].as_str().unwrap().is_empty(), "{name}: a gap cites its rule and authority");
            }
            all_checks.extend(out.checks);
        }
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(PathBuf::from(dir).join("foundation_checks.json"), serde_json::to_vec_pretty(&all_checks).unwrap()).unwrap();
        }
    }

    #[test]
    fn foundation_gaps_lead_the_panel_and_planted_gaps_are_in_the_top_three() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let m = &fx["merge"];
        let merged = merge_panel(m["v1_panel"].as_array().unwrap(), m["gaps"].as_array().unwrap());
        let order: Vec<&str> = merged.iter().map(|e| e["panel_id"].as_str().unwrap()).collect();
        let expect: Vec<&str> = m["expect_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(order, expect);
        assert!(merged.iter().enumerate().all(|(n, e)| e["rank"] == json!(n + 1)));
        assert_eq!(merged[0]["kind"], "FOUNDATION");
        assert!(order.iter().take(3).any(|id| *id == "g-confirmed") && !order.contains(&"g-unknown"), "an UNKNOWN element is not a Missing Link");
        // v1 entries are carried unchanged (nothing is rescored).
        assert_eq!(merged[2]["score"], m["v1_panel"][0]["score"]);
    }

    #[test]
    fn shipped_admissibility_rules_install_cite_the_right_text_and_check_a_matter() {
        // The real development pack through the real installer, loader and store.
        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packs/official/caseforge.india.admissibility-rules");
        let vault = tempfile::tempdir().unwrap();
        let paths = crate::vault::VaultPaths::new(vault.path().to_path_buf());
        crate::packs::install_pack_from_dir(&paths, &pack).expect("the rules pack installs");
        let (tables, rejected) = crate::rule_tables::load_installed(&paths);
        assert!(rejected.is_empty(), "{rejected:?}");
        let table = tables.iter().find(|t| t.table_kind == "ADMISSIBILITY_REQUIREMENT").expect("admissibility table");
        assert_eq!(table.rows.len(), 6);
        assert!(table.rows.iter().all(|r| r.trust_state == "DEVELOPMENT_ONLY" && r.signed_by.is_none()));
        let rs: Vec<Rule> = table.rows.iter().map(|r| Rule::from_row(r).unwrap()).collect();
        // Each rule's first citation is the sentence of the section it rests on.
        let store = crate::retrieval::store::open(&paths).unwrap();
        let expect = [("in.adm.electronic.iea", "a certificate doing any of the following things"), ("in.adm.electronic.bsa", "submitted along with the electronic record"), ("in.adm.secondary-copy.iea", "contents of a document"), ("in.adm.secondary-copy.bsa", "contents of a document"), ("in.adm.expert.iea", "specially skilled"), ("in.adm.expert.bsa", "specially skilled")];
        for (id, phrase) in expect {
            let r = rs.iter().find(|r| r.rule_id == id).unwrap_or_else(|| panic!("{id}"));
            let a = &r.authorities[0];
            let chunk = store.get_by_chunk_id(a["chunk_id"].as_str().unwrap()).expect("cited chunk installed");
            let (s0, e0) = (a["char_start"].as_u64().unwrap() as usize, a["char_end"].as_u64().unwrap() as usize);
            let cited: String = chunk.text.chars().skip(s0).take(e0 - s0).collect();
            assert!(cited.contains(phrase), "{id}: '{cited}' should contain '{phrase}'");
        }
        // A matter with a WhatsApp screenshot and no certificate: a gap under the Sakshya Adhiniyam today, under the Evidence Act on an earlier date.
        let ev = vec![Evidence { evidence_id: "evidence_0001".into(), label: "Annexure P-1: WhatsApp chat screenshot".into(), kind: "annexure".into(), doc_id: None }];
        let run = |docs: &[DocInfo], as_of: &[&str]| {
            let dates: Vec<String> = as_of.iter().map(|d| d.to_string()).collect();
            check_all(&Inputs { case_id: "c", evidence: &ev, docs, nodes: &[], edges: &[], links: &[], types: &[], rules: &rs, as_of: &dates, vocab: vocabulary() })
        };
        let today = run(&[], &["2026-10-09"]);
        assert_eq!((today.checks.len(), today.checks[0]["rule_id"].as_str(), today.checks[0]["elements"][0]["status"].as_str()), (1, Some("in.adm.electronic.bsa"), Some("ABSENT")));
        assert_eq!(today.gaps.len(), 1);
        assert!(today.gaps[0]["message"].as_str().unwrap().contains("not on file") && today.gaps[0]["authority"]["provision_label"] == "Sakshya Adhiniyam s.63(4)");
        let both = run(&[], &["2026-10-09", "2023-05-01"]);
        assert_eq!(both.checks.iter().map(|c| c["rule_id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["in.adm.electronic.bsa", "in.adm.electronic.iea"]);
        let with_cert = run(&[DocInfo { doc_id: "d9".into(), filename: "certificate.pdf".into(), type_tag: "CERTIFICATE_63".into() }], &["2026-10-09"]);
        assert_eq!(with_cert.checks[0]["elements"][0]["status"], "PRESENT");
        assert!(with_cert.gaps.is_empty());
    }

    #[test]
    fn evidence_types_are_matched_on_whole_words_and_ties_go_to_the_earlier_type() {
        let v = vocabulary();
        let ev = |label: &str| Evidence { evidence_id: "e".into(), label: label.into(), kind: "exhibit".into(), doc_id: None };
        let t = |label: &str| classify(&ev(label), &[], v, &[]).map(|(t, _)| t);
        assert_eq!(t("Photocopy of the agreement").as_deref(), Some("SECONDARY_COPY"));
        assert_eq!(t("A copy of the agreement").as_deref(), Some("SECONDARY_COPY"));
        assert_eq!(t("Registered sale deed"), None, "no keyword, no type");
        assert_eq!(t("Digitally signed sale deed"), None, "'digital' is not a whole word in 'digitally'");
        assert_eq!(t("WhatsApp chat export").as_deref(), Some("ELECTRONIC_RECORD"));
        assert_eq!(t("Photocopy of an e-mail").as_deref(), Some("ELECTRONIC_RECORD"), "tie: the earlier type wins; counsel can change it");
        assert_eq!(t("Handwriting expert's FSL report").as_deref(), Some("EXPERT_OPINION"));
        let chosen = [TypeChoice { evidence_id: "e".into(), evidence_type: "SECONDARY_COPY".into() }];
        assert_eq!(classify(&ev("WhatsApp chat export"), &[], v, &chosen), Some(("SECONDARY_COPY".to_string(), "COUNSEL_CONFIRMED")));
    }
}
