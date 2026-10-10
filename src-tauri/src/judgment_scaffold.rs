//! Judgment Scaffold with the Decision-Free Zone (B-S4-21, PRD v4.0 §9.5, guardrails G7 and G8).
//!
//! A judgment has ten sections. CaseForge may pre-fill seven of them, from the record only, deterministically (no model is
//! involved in any section): cause title, appearances, brief facts, procedural history, submissions of each party, the
//! judge's accepted issues, and a summary of the evidence. Every pre-filled sentence is the record's own text, with a
//! locator that is checked here against the documents. **Analysis, findings and the operative order are locked to the
//! judge**: this module has no way to produce text for them, nothing in it calls a model, and the screen has no generate
//! button; only text typed by the judge goes in them.
//!
//!   CAUSE_TITLE         the matter's cause title, court and number
//!   APPEARANCES         sentences of the orders saying who appeared for whom
//!   BRIEF_FACTS         the facts the filings AGREE on (admitted facts of the accepted issues), never one party's allegations
//!   PROCEDURAL_HISTORY  the orders on the record, in date order, as written
//!   SUBMISSIONS         each party's submissions (argument paragraphs of their filings), as written
//!   ISSUES              the issues the judge accepted or edited (the judge's own act, not a machine statement)
//!   EVIDENCE_SUMMARY    evidence items of the matter whose label appears in the document they come from
//! The judge can add their own sentences anywhere, and remove any pre-filled one. Allowed help on the judge's text is
//! spellcheck, formatting and the citation audit - none of it generates text. Contract: contracts/judgment_scaffold.schema.json.
//! Golden: golden/G-CHAMBERS (judgment_scaffold). The existence of a scaffold locks record-only mode on (B-S4-17).

use crate::cause_list::{self, Matter, MatterDoc};
use crate::issue_matrix::{Filing, IssueIn, Party};
use crate::opposition;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

pub const KINDS: [&str; 10] = ["CAUSE_TITLE", "APPEARANCES", "BRIEF_FACTS", "PROCEDURAL_HISTORY", "SUBMISSIONS", "ISSUES", "EVIDENCE_SUMMARY", "ANALYSIS", "FINDINGS", "OPERATIVE_ORDER"];
pub const LOCKED: [&str; 3] = ["ANALYSIS", "FINDINGS", "OPERATIVE_ORDER"];
pub const FOOTER_ID: &str = "caseforge.export_footer.v1";

pub fn is_locked(kind: &str) -> bool {
    LOCKED.contains(&kind)
}

pub struct EvidenceItem {
    pub evidence_id: String,
    pub label: String,
    pub doc_id: Option<String>,
}

pub struct Inputs<'a> {
    pub cause_title: &'a str,
    pub court: &'a str,
    pub label: &'a str,
    pub jurisdiction: &'a str,
    pub parties: &'a [Party],
    pub filings: &'a [Filing],
    pub docs: &'a [MatterDoc],
    pub issues: &'a [IssueIn],
    pub evidence: &'a [EvidenceItem],
    /// The Issue Matrix document (contracts/issue_matrix.schema.json) built over the same filings and issues.
    pub matrix: &'a Value,
}

fn matter_locator(doc_id: &str, page: usize, start: usize, end: usize, excerpt: &str) -> Value {
    json!({"kind": "MATTER", "doc_id": doc_id, "page": page, "char_start": start, "char_end": end, "excerpt": excerpt})
}

fn prefill_sentence(text: &str, class: &str, locators: Vec<Value>) -> Value {
    json!({"text": text, "origin": "MACHINE_PREFILL", "classification": class, "locators": locators})
}

fn judge_sentence(text: &str, locators: Vec<Value>) -> Value {
    json!({"text": text, "origin": "JUDGE_AUTHORED", "classification": "JUDGE_TEXT", "locators": locators})
}

static APPEARANCE: once_cell::sync::Lazy<(regex::Regex, regex::Regex, regex::Regex)> = once_cell::sync::Lazy::new(|| {
    (
        regex::Regex::new(r"(?i)\b(?:appeared|appearing|appearance|present|counsel\s+for)\b").unwrap(),
        regex::Regex::new(r"(?i)\bfor\s+(?:the\s+)?(?:plaintiffs?|defendants?|petitioners?|respondents?|appellants?|accused|state|prosecution|complainants?|applicants?|opponents?)\b").unwrap(),
        regex::Regex::new(r"\b(?:Mr|Ms|Mrs|Sh|Shri|Smt|Dr|Adv|Advocate|Sri)\b").unwrap(),
    )
});

fn matter_of(inp: &Inputs) -> Matter {
    Matter { case_id: String::new(), label: inp.label.into(), cause_title: inp.cause_title.into(), jurisdiction: inp.jurisdiction.into(), parties: inp.parties.to_vec(), docs: inp.docs.to_vec() }
}

/// The machine pre-fill of each section kind. Locked sections are always empty.
pub fn prefill(inp: &Inputs) -> BTreeMap<&'static str, Vec<Value>> {
    let mut out: BTreeMap<&'static str, Vec<Value>> = KINDS.iter().map(|k| (*k, Vec::new())).collect();
    // CAUSE_TITLE
    for (field, value) in [("cause_title", inp.cause_title), ("court", inp.court), ("label", inp.label)] {
        if !value.trim().is_empty() {
            out.get_mut("CAUSE_TITLE").unwrap().push(prefill_sentence(value, "RECORD", vec![json!({"kind": "MATTER_PROFILE", "field": field, "value": value})]));
        }
    }
    // orders: APPEARANCES and PROCEDURAL_HISTORY
    let (orders, _) = cause_list::orders_of(&matter_of(inp));
    for o in &orders {
        for s in cause_list::sentences(&o.text) {
            if APPEARANCE.0.is_match(&s.2) && APPEARANCE.1.is_match(&s.2) && APPEARANCE.2.is_match(&s.2) {
                out.get_mut("APPEARANCES").unwrap().push(prefill_sentence(&s.2, "RECORD", vec![matter_locator(&o.doc_id, o.page, o.start + s.0, o.start + s.1, &s.2)]));
            }
        }
        out.get_mut("PROCEDURAL_HISTORY").unwrap().push(prefill_sentence(&o.text, "RECORD", vec![matter_locator(&o.doc_id, o.page, o.start, o.end, &o.text)]));
    }
    // BRIEF_FACTS: what the filings agree on
    let mut seen = std::collections::HashSet::new();
    for issue in inp.matrix["issues"].as_array().into_iter().flatten().filter(|i| matches!(i["judge_decision"].as_str(), Some("ACCEPTED" | "EDITED"))) {
        for f in issue["admitted_facts"].as_array().into_iter().flatten() {
            if !seen.insert(f["fact_node_id"].as_str().unwrap_or("").to_string()) {
                continue;
            }
            let locs: Vec<Value> = f["locators"].as_array().into_iter().flatten().map(|l| matter_locator(l["doc_id"].as_str().unwrap_or(""), l["page_index"].as_u64().unwrap_or(0) as usize + 1, l["char_start"].as_u64().unwrap_or(0) as usize, l["char_end"].as_u64().unwrap_or(0) as usize, l["excerpt"].as_str().unwrap_or(""))).collect();
            if let Some(first) = f["locators"][0]["excerpt"].as_str() {
                out.get_mut("BRIEF_FACTS").unwrap().push(prefill_sentence(first, "RECORD", locs));
            }
        }
    }
    // SUBMISSIONS: each party's argument paragraphs, parties in the order given
    for p in inp.parties {
        for f in inp.filings.iter().filter(|f| f.party_id == p.party_id) {
            for para in opposition::paragraphs(&f.pages).into_iter().filter(|x| x.argument) {
                let mut loc = matter_locator(&f.doc_id, para.page_index + 1, para.char_start, para.char_end, &para.text);
                loc["party_id"] = json!(p.party_id);
                out.get_mut("SUBMISSIONS").unwrap().push(prefill_sentence(&para.text, "RECORD", vec![loc]));
            }
        }
    }
    // ISSUES: what the judge accepted or edited (their own act)
    for i in inp.issues.iter().filter(|i| matches!(i.decision.as_str(), "ACCEPTED" | "EDITED")) {
        out.get_mut("ISSUES").unwrap().push(judge_sentence(&i.text, vec![json!({"kind": "ISSUE_DECISION", "issue_id": i.issue_id, "decision": i.decision})]));
    }
    // EVIDENCE_SUMMARY: labels that really appear in their document
    for e in inp.evidence {
        let Some(doc) = e.doc_id.as_deref().and_then(|id| inp.docs.iter().find(|d| d.doc_id == id)) else { continue };
        let label = e.label.trim();
        if label.is_empty() {
            continue;
        }
        for (pi, page) in doc.pages.iter().enumerate() {
            if let Some(b) = page.find(label) {
                let start = page[..b].chars().count();
                out.get_mut("EVIDENCE_SUMMARY").unwrap().push(prefill_sentence(label, "RECORD", vec![matter_locator(&doc.doc_id, pi + 1, start, start + label.chars().count(), label)]));
                break;
            }
        }
    }
    out
}

/// Splits the judge's own text into sentences.
fn judge_sentences(text: &str) -> Vec<Value> {
    cause_list::sentences(text).into_iter().map(|s| judge_sentence(&s.2, vec![])).collect()
}

/// A new scaffold from a pre-fill.
pub fn new_scaffold(case_id: &str, created_at: &str, issue_matrix_id: Option<String>, pre: &BTreeMap<&'static str, Vec<Value>>) -> Value {
    let sections: Vec<Value> = KINDS
        .iter()
        .map(|k| {
            let locked = is_locked(k);
            json!({"section_kind": k, "assist_policy": if locked { "LOCKED_JUDGE_ONLY" } else { "PREFILL_ALLOWED" }, "sentences": if locked { vec![] } else { pre.get(k).cloned().unwrap_or_default() }})
        })
        .collect();
    json!({"schema_version": "1.0.0", "scaffold_id": format!("js:{case_id}"), "case_id": case_id, "created_at": created_at, "record_only_locked": true, "issue_matrix_id": issue_matrix_id, "sections": sections, "export_footer_id": FOOTER_ID})
}

fn section_mut<'a>(sc: &'a mut Value, kind: &str) -> Result<&'a mut Vec<Value>, String> {
    sc["sections"].as_array_mut().and_then(|a| a.iter_mut().find(|s| s["section_kind"] == kind)).and_then(|s| s["sentences"].as_array_mut()).ok_or_else(|| format!("CF_SCAFFOLD_SECTION: no section {kind}"))
}

/// Replaces the machine pre-fill of the pre-fillable sections with a fresh one; the judge's own sentences stay, after it.
pub fn refresh(sc: &mut Value, pre: &BTreeMap<&'static str, Vec<Value>>) -> Result<(), String> {
    for k in KINDS.iter().filter(|k| !is_locked(k)) {
        let s = section_mut(sc, k)?;
        let mine: Vec<Value> = s.iter().filter(|x| x["origin"] == "JUDGE_AUTHORED" && x["locators"].as_array().map(|l| l.iter().all(|y| y["kind"] != "ISSUE_DECISION")).unwrap_or(true)).cloned().collect();
        let mut fresh = pre.get(k).cloned().unwrap_or_default();
        fresh.extend(mine);
        *s = fresh;
    }
    Ok(())
}

/// Sets the judge's own text for a section (any section, locked or not): replaces the judge-typed sentences, leaves any
/// machine pre-fill alone. Whatever the judge writes is stored exactly as written.
pub fn set_judge_text(sc: &mut Value, kind: &str, text: &str) -> Result<(), String> {
    if !KINDS.contains(&kind) {
        return Err(format!("CF_SCAFFOLD_SECTION: no section {kind}"));
    }
    let s = section_mut(sc, kind)?;
    // issue sentences accepted in the Issue Matrix are the judge's but come from there; typed text is separate
    s.retain(|x| x["origin"] != "JUDGE_AUTHORED" || x["locators"].as_array().map(|l| l.iter().any(|y| y["kind"] == "ISSUE_DECISION")).unwrap_or(false));
    s.extend(judge_sentences(text));
    Ok(())
}

/// The judge's typed text of a section, as one string.
pub fn judge_text(sc: &Value, kind: &str) -> String {
    sc["sections"].as_array().and_then(|a| a.iter().find(|s| s["section_kind"] == kind)).and_then(|s| s["sentences"].as_array()).map(|a| a.iter().filter(|x| x["origin"] == "JUDGE_AUTHORED" && x["locators"].as_array().map(|l| l.is_empty()).unwrap_or(true)).filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default()
}

pub fn remove_sentence(sc: &mut Value, kind: &str, index: usize) -> Result<(), String> {
    let s = section_mut(sc, kind)?;
    if index >= s.len() {
        return Err("CF_SCAFFOLD_SENTENCE: no such sentence".into());
    }
    s.remove(index);
    Ok(())
}

pub struct Profile<'a> {
    pub cause_title: &'a str,
    pub court: &'a str,
    pub label: &'a str,
}

/// Checks a scaffold: structure (ten unique sections, locked ones judge-only), and that every pre-filled sentence has a valid
/// locator - an exact slice of a document on the record, or a matter-profile field with that value. (AC-S4-15.3)
pub fn validate(sc: &Value, docs: &[MatterDoc], profile: &Profile) -> Result<(), Vec<String>> {
    let mut errs = Vec::new();
    let sections = sc["sections"].as_array().cloned().unwrap_or_default();
    let kinds: Vec<&str> = sections.iter().filter_map(|s| s["section_kind"].as_str()).collect();
    if kinds != KINDS {
        errs.push(format!("the ten sections must appear once each, in order: {kinds:?}"));
    }
    if sc["record_only_locked"] != true || sc["export_footer_id"] != FOOTER_ID {
        errs.push("record_only_locked must be true and the export footer id fixed".into());
    }
    for s in &sections {
        let kind = s["section_kind"].as_str().unwrap_or("");
        let locked = is_locked(kind);
        if s["assist_policy"] != if locked { "LOCKED_JUDGE_ONLY" } else { "PREFILL_ALLOWED" } {
            errs.push(format!("{kind}: wrong assist policy"));
        }
        for (i, x) in s["sentences"].as_array().into_iter().flatten().enumerate() {
            let at = format!("{kind}[{i}]");
            match x["origin"].as_str() {
                Some("JUDGE_AUTHORED") => {
                    if x["classification"] != "JUDGE_TEXT" {
                        errs.push(format!("{at}: judge text must be JUDGE_TEXT"));
                    }
                }
                Some("MACHINE_PREFILL") => {
                    if locked {
                        errs.push(format!("{at}: a locked section holds machine text"));
                    }
                    if !matches!(x["classification"].as_str(), Some("RECORD" | "LAW" | "INFERENCE_FROM_RECORD")) {
                        errs.push(format!("{at}: pre-filled text must be RECORD, LAW or INFERENCE_FROM_RECORD"));
                    }
                    let locs = x["locators"].as_array().cloned().unwrap_or_default();
                    if locs.is_empty() {
                        errs.push(format!("{at}: no locator"));
                    }
                    for l in &locs {
                        if let Err(e) = check_locator(l, docs, profile) {
                            errs.push(format!("{at}: {e}"));
                        }
                    }
                }
                _ => errs.push(format!("{at}: unknown origin")),
            }
        }
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

fn check_locator(l: &Value, docs: &[MatterDoc], p: &Profile) -> Result<(), String> {
    match l["kind"].as_str() {
        Some("MATTER") => {
            let doc = docs.iter().find(|d| Some(d.doc_id.as_str()) == l["doc_id"].as_str()).ok_or("the document is not in the matter")?;
            let page = doc.pages.get((l["page"].as_u64().unwrap_or(0) as usize).wrapping_sub(1)).ok_or("no such page")?;
            let (s, e) = (l["char_start"].as_u64().unwrap_or(0) as usize, l["char_end"].as_u64().unwrap_or(0) as usize);
            let text: String = page.chars().skip(s).take(e.saturating_sub(s)).collect();
            if e <= s || text != l["excerpt"].as_str().unwrap_or("\u{0}") {
                return Err("the excerpt is not the document's text at that place".into());
            }
            Ok(())
        }
        Some("MATTER_PROFILE") => {
            let want = match l["field"].as_str() {
                Some("cause_title") => p.cause_title,
                Some("court") => p.court,
                Some("label") => p.label,
                _ => return Err("unknown profile field".into()),
            };
            if l["value"].as_str() == Some(want) && !want.is_empty() { Ok(()) } else { Err("the profile value differs".into()) }
        }
        _ => Err("unknown locator kind".into()),
    }
}

const HEADINGS: [(&str, &str); 10] = [
    ("CAUSE_TITLE", "Cause title"), ("APPEARANCES", "Appearances"), ("BRIEF_FACTS", "Brief facts"), ("PROCEDURAL_HISTORY", "Procedural history"), ("SUBMISSIONS", "Submissions"),
    ("ISSUES", "Issues"), ("EVIDENCE_SUMMARY", "Evidence summary"), ("ANALYSIS", "Analysis"), ("FINDINGS", "Findings"), ("OPERATIVE_ORDER", "Operative order"),
];
pub const EXPORT_FOOTER_LINES: [&str; 3] = [
    "Local model output for preparation only. Not legal advice. Citations require pack verification.",
    "Preparation completeness only \u{2014} never win prediction, outcome odds, or verdict language.",
    "Export footer (G4) \u{b7} caseforge.export_footer.v1 \u{b7} verify every fact against its source pointer before filing.",
];

/// The scaffold as plain text with the G4 footer, a statement of which sections were machine-assisted, and the G5 line.
pub fn export_text(sc: &Value, doc_names: &HashMap<String, String>, generated_at: &str) -> String {
    let mut out = String::from("JUDGMENT \u{2014} DRAFT SCAFFOLD\nThe sections below marked \"from the record\" were filled from the filings, orders and evidence on the record. Analysis, findings and the operative order are written only by the judge.\n\n");
    let (mut assisted, mut judge_only) = (Vec::new(), Vec::new());
    for (n, (kind, heading)) in HEADINGS.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", n + 1, heading.to_uppercase()));
        let sentences = sc["sections"].as_array().and_then(|a| a.iter().find(|s| s["section_kind"] == *kind)).and_then(|s| s["sentences"].as_array()).cloned().unwrap_or_default();
        if sentences.is_empty() {
            out.push_str(if is_locked(kind) { "(to be written by the judge)\n" } else { "(nothing found on the record)\n" });
        }
        let mut machine = false;
        for s in &sentences {
            out.push_str(s["text"].as_str().unwrap_or(""));
            if s["origin"] == "MACHINE_PREFILL" {
                machine = true;
                let l = &s["locators"][0];
                let name = l["doc_id"].as_str().map(|d| doc_names.get(d).cloned().unwrap_or_else(|| d.to_string()));
                match (l["kind"].as_str(), name) {
                    (Some("MATTER"), Some(n)) => out.push_str(&format!("  [from the record: {n}, page {}]", l["page"])),
                    _ => out.push_str("  [from the matter details]"),
                }
            }
            out.push('\n');
        }
        out.push('\n');
        if machine { assisted.push(*heading) } else { judge_only.push(*heading) }
    }
    out.push_str(&format!("Sections filled from the record by CaseForge: {}.\n", if assisted.is_empty() { "none".to_string() } else { assisted.join(", ") }));
    out.push_str(&format!("Sections written only by the judge: {}. Analysis, Findings and Operative order are never machine-assisted.\n\n", judge_only.join(", ")));
    for l in EXPORT_FOOTER_LINES {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&format!("\nProvenance (G5): export_id {}; generated_at {generated_at}; template_id judgment_scaffold; case_id {}; footer_id {FOOTER_ID}; ai_spans: none (model_hash deterministic:case_graph, prompt_hash deterministic:template, retrieval_ids []).\n", sc["scaffold_id"].as_str().unwrap_or(""), sc["case_id"].as_str().unwrap_or("")));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::issue_matrix;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../golden/G-CHAMBERS/fixtures/judgment_scaffold_core.json");
    const FIXTURE_SHA256: &str = "6339dcb55dbe54f5d56c221269e1aa2732b0b32d6c3469eeb16f923046b20da0"; // committed before the engine (bbef75b)

    struct World {
        fx: Value,
        parties: Vec<Party>,
        filings: Vec<Filing>,
        docs: Vec<MatterDoc>,
        issues: Vec<IssueIn>,
        evidence: Vec<EvidenceItem>,
        matrix: Value,
    }

    fn world() -> World {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "scaffold golden changed since it was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let parties: Vec<Party> = serde_json::from_value(fx["parties"].clone()).unwrap();
        let filings: Vec<Filing> = serde_json::from_value(fx["filings"].clone()).unwrap();
        let docs: Vec<MatterDoc> = serde_json::from_value(fx["documents"].clone()).unwrap();
        let issues: Vec<IssueIn> = fx["issues"].as_array().unwrap().iter().map(|i| IssueIn { issue_id: i["issue_id"].as_str().unwrap().into(), text: i["text"].as_str().unwrap().into(), origin: i["origin"].as_str().unwrap().into(), template_rule_id: i["template_rule_id"].as_str().map(str::to_string), decision: i["decision"].as_str().unwrap().into(), merged_into: None }).collect();
        let evidence = fx["evidence"].as_array().unwrap().iter().map(|e| EvidenceItem { evidence_id: e["evidence_id"].as_str().unwrap().into(), label: e["label"].as_str().unwrap().into(), doc_id: e["doc_id"].as_str().map(str::to_string) }).collect();
        let matrix = issue_matrix::build(&issue_matrix::Inputs { case_id: "case-1", built_at: "2024-10-14T00:00:00Z", parties: &parties, filings: &filings, issues: &issues, audited: &[] }).unwrap().matrix;
        World { fx, parties, filings, docs, issues, evidence, matrix }
    }

    fn inputs(w: &World) -> Inputs<'_> {
        let p = &w.fx["profile"];
        Inputs { cause_title: p["cause_title"].as_str().unwrap(), court: p["court"].as_str().unwrap(), label: p["label"].as_str().unwrap(), jurisdiction: "India", parties: &w.parties, filings: &w.filings, docs: &w.docs, issues: &w.issues, evidence: &w.evidence, matrix: &w.matrix }
    }

    fn profile(w: &World) -> Profile<'_> {
        Profile { cause_title: w.fx["profile"]["cause_title"].as_str().unwrap(), court: w.fx["profile"]["court"].as_str().unwrap(), label: w.fx["profile"]["label"].as_str().unwrap() }
    }

    #[test]
    fn golden_prefill_is_the_record_and_nothing_else() {
        let w = world();
        let pre = prefill(&inputs(&w));
        for kind in KINDS {
            let want = w.fx["expect"][kind].as_array().unwrap();
            let got = &pre[kind];
            assert_eq!(got.iter().map(|s| s["text"].as_str().unwrap().to_string()).collect::<Vec<_>>(), want.iter().map(|s| s["text"].as_str().unwrap().to_string()).collect::<Vec<_>>(), "{kind}: sentences");
            for (g, e) in got.iter().zip(want) {
                if let Some(d) = e.get("doc_id") {
                    assert_eq!(g["locators"][0]["doc_id"], *d, "{kind}: document");
                }
                if let Some(ds) = e.get("doc_ids") {
                    assert_eq!(g["locators"].as_array().unwrap().iter().map(|l| l["doc_id"].clone()).collect::<Vec<_>>(), *ds.as_array().unwrap(), "{kind}: both filings");
                }
                if let Some(p) = e.get("party_id") {
                    assert_eq!(g["locators"][0]["party_id"], *p, "{kind}: party");
                }
                if let Some(i) = e.get("issue_id") {
                    assert_eq!((g["locators"][0]["issue_id"].clone(), g["origin"].clone()), (i.clone(), json!("JUDGE_AUTHORED")), "{kind}: the judge's accepted issue");
                } else {
                    assert_eq!((g["origin"].as_str(), g["classification"].as_str()), (Some("MACHINE_PREFILL"), Some("RECORD")), "{kind}");
                }
            }
        }
        for k in LOCKED {
            assert!(pre[k].is_empty(), "{k}: nothing is ever pre-filled for a locked section");
        }
    }

    #[test]
    fn a_new_scaffold_validates_against_the_documents_and_the_frozen_contract_shape() {
        // AC-S4-15.3
        let w = world();
        let sc = new_scaffold("case-1", "2024-10-14T08:00:00Z", Some("im:case-1".into()), &prefill(&inputs(&w)));
        validate(&sc, &w.docs, &profile(&w)).unwrap();
        assert_eq!(sc["sections"].as_array().unwrap().len(), 10);
        let machine: Vec<&Value> = sc["sections"].as_array().unwrap().iter().flat_map(|s| s["sentences"].as_array().unwrap()).filter(|x| x["origin"] == "MACHINE_PREFILL").collect();
        assert!(machine.len() >= 10 && machine.iter().all(|x| !x["locators"].as_array().unwrap().is_empty()));
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            let mut with_judge = sc.clone();
            for k in LOCKED {
                set_judge_text(&mut with_judge, k, w.fx["judge_text"][k].as_str().unwrap()).unwrap();
            }
            std::fs::write(std::path::PathBuf::from(&dir).join("judgment_scaffold.json"), serde_json::to_vec_pretty(&with_judge).unwrap()).unwrap();
        }
    }

    #[test]
    fn a_bad_locator_a_machine_sentence_in_a_locked_section_or_a_missing_section_is_caught() {
        let w = world();
        let good = new_scaffold("case-1", "t", None, &prefill(&inputs(&w)));
        let broken = |f: &dyn Fn(&mut Value)| {
            let mut sc = good.clone();
            f(&mut sc);
            validate(&sc, &w.docs, &profile(&w)).unwrap_err().join("; ")
        };
        assert!(broken(&|sc| sc["sections"][3]["sentences"][0]["locators"][0]["excerpt"] = json!("something else")).contains("not the document's text"));
        assert!(broken(&|sc| sc["sections"][3]["sentences"][0]["locators"] = json!([])).contains("no locator"));
        assert!(broken(&|sc| sc["sections"][0]["sentences"][0]["locators"][0]["value"] = json!("Another v. Matter")).contains("profile value differs"));
        assert!(broken(&|sc| sc["sections"][7]["sentences"] = json!([prefill_sentence("The suit is dismissed.", "RECORD", vec![json!({"kind": "MATTER", "doc_id": "d-ws", "page": 1, "char_start": 0, "char_end": 1, "excerpt": "1"})])])).contains("a locked section holds machine text"));
        assert!(broken(&|sc| { sc["sections"].as_array_mut().unwrap().pop(); }).contains("the ten sections"));
        assert!(broken(&|sc| sc["sections"][7]["assist_policy"] = json!("PREFILL_ALLOWED")).contains("wrong assist policy"));
        assert!(broken(&|sc| sc["sections"][2]["sentences"][0]["classification"] = json!("EXTRA_RECORD")).contains("RECORD, LAW or INFERENCE_FROM_RECORD"));
    }

    #[test]
    fn the_judges_text_is_stored_exactly_and_never_touched_by_the_machine() {
        let w = world();
        let mut sc = new_scaffold("case-1", "t", None, &prefill(&inputs(&w)));
        for k in LOCKED {
            set_judge_text(&mut sc, k, w.fx["judge_text"][k].as_str().unwrap()).unwrap();
        }
        // the judge may write a decision in their own sections: the decision-free zone binds the machine, not the judge
        let analysis = sc["sections"][7]["sentences"].as_array().unwrap();
        assert_eq!(analysis.iter().map(|s| s["text"].as_str().unwrap()).collect::<Vec<_>>(), ["The plaintiff's case fails on limitation.", "We therefore dismiss the suit."]);
        assert!(analysis.iter().all(|s| s["origin"] == "JUDGE_AUTHORED" && s["classification"] == "JUDGE_TEXT" && s["locators"].as_array().unwrap().is_empty()));
        assert!(crate::g8::check_output("We therefore dismiss the suit.").is_some(), "the same words from the machine would be withheld");
        validate(&sc, &w.docs, &profile(&w)).unwrap();
        // the judge also adds a sentence to a pre-filled section; a refresh from the record keeps it and replaces only machine text
        set_judge_text(&mut sc, "BRIEF_FACTS", "The parties are brothers.").unwrap();
        let before = judge_text(&sc, "ANALYSIS");
        refresh(&mut sc, &prefill(&inputs(&w))).unwrap();
        assert_eq!((judge_text(&sc, "ANALYSIS"), judge_text(&sc, "BRIEF_FACTS")), (before, "The parties are brothers.".to_string()));
        assert_eq!(sc["sections"][2]["sentences"].as_array().unwrap().len(), 2, "one record sentence and the judge's");
        // removing a pre-filled sentence is the judge's choice
        remove_sentence(&mut sc, "APPEARANCES", 0).unwrap();
        assert_eq!(sc["sections"][1]["sentences"].as_array().unwrap().len(), 1);
        assert!(remove_sentence(&mut sc, "APPEARANCES", 9).is_err() && set_judge_text(&mut sc, "VERDICT", "x").is_err());
    }

    #[test]
    fn export_names_the_machine_assisted_sections_and_carries_the_footer() {
        let w = world();
        let mut sc = new_scaffold("case-1", "t", None, &prefill(&inputs(&w)));
        for k in LOCKED {
            set_judge_text(&mut sc, k, w.fx["judge_text"][k].as_str().unwrap()).unwrap();
        }
        let names: HashMap<String, String> = w.docs.iter().map(|d| (d.doc_id.clone(), d.filename.clone())).collect();
        let text = export_text(&sc, &names, "2024-10-14T09:00:00Z");
        for l in EXPORT_FOOTER_LINES {
            assert!(text.contains(l));
        }
        assert!(text.contains("Sections filled from the record by CaseForge: Cause title, Appearances, Brief facts, Procedural history, Submissions, Evidence summary."));
        assert!(text.contains("Sections written only by the judge: Issues, Analysis, Findings, Operative order. Analysis, Findings and Operative order are never machine-assisted."));
        assert!(text.contains("We therefore dismiss the suit.") && text.contains("[from the record: Plaint.pdf, page 1]") && text.contains("ai_spans: none"));
        let analysis = text.split("8. ANALYSIS\n").nth(1).unwrap().split("9. FINDINGS").next().unwrap();
        assert!(!analysis.contains("[from the record") && !analysis.contains("[from the matter"), "nothing in a locked section is marked as machine text");
        // a new matter's export says plainly that the locked sections are for the judge
        let empty = new_scaffold("c", "t", None, &BTreeMap::new());
        assert!(export_text(&empty, &HashMap::new(), "t").contains("(to be written by the judge)"));
    }

    #[test]
    fn nothing_here_can_reach_a_model() {
        // AC-S4-15.1, the module's part: no model, no network, no way to ask for text for a locked section
        let src = include_str!("judgment_scaffold.rs").split("#[cfg(test)]").next().unwrap();
        for t in ["infer_llm", "ollama", "chat_with_timeout", "llm_chat", "reqwest", "http://", "https://", "grounded_mock_court", "reasoning::"] {
            assert!(!src.contains(t), "{t}");
        }
        let w = world();
        assert!(prefill(&inputs(&w)).iter().filter(|(k, _)| is_locked(k)).all(|(_, v)| v.is_empty()));
    }
}
