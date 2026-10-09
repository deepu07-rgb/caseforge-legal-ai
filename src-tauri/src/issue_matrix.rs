//! Issue Matrix (B-S4-18, PRD v4.0 §9.2).
//!
//! A neutral, side-by-side view for the judge of each ACCEPTED issue: what each party has pleaded on it, the evidence and
//! authorities their paragraphs refer to (with the audit result of each citation), the facts that are admitted and the
//! facts that are disputed, and the arguments one side made that the other never answered. Deterministic: no model, no
//! network, so text inside a filing cannot steer it. It ranks nothing, scores nothing, suggests no answer and treats every
//! party the same way (the same rules run for each party; parties appear in the order given).
//!
//! Rules (written down in docs/B-S4-18_issue_matrix.md; the golden was derived from them by an independent implementation):
//! - A paragraph of a filing belongs to an issue when it shares at least `ASSIGN_MIN` keywords with the issue text. Issues
//!   merged into an accepted issue contribute their paragraphs to it. Only ACCEPTED and EDITED issues get content.
//! - A paragraph that names "paragraph N" of the other side's filing responds to it. It admits it if it says so
//!   ("admitted", "not disputed"...), and disputes it if it says so ("denied", "does not admit"...); a response that does
//!   both is listed as disputed. Only paragraphs of fact count as admitted or disputed facts.
//! - Where no paragraph refers to another, fact paragraphs of different parties are paired when they share at least
//!   `RESPONSE_MIN` keywords: one denies and the other does not → both are disputed; neither denies → both are admitted.
//! - An argument is unanswered by a party that has filed something, when none of that party's paragraphs refers to it and
//!   none of their paragraphs on the same issue shares at least `RESPONSE_MIN` keywords with it.
//!
//! Contract: contracts/issue_matrix.schema.json. Golden: golden/G-CHAMBERS.

use crate::answer_feedback::keywords;
use crate::opposition::{self, Paragraph};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};

pub const PIPELINE_ID: &str = "caseforge.issue-matrix.v1";
pub const SCHEMA_VERSION: &str = "1.0.0";
/// Shared keywords for a paragraph to belong to an issue.
pub const ASSIGN_MIN: usize = 2;
/// Shared keywords for one paragraph to answer, or agree with, another.
pub const RESPONSE_MIN: usize = 3;
const DECISIONS: [&str; 5] = ["PROPOSED", "ACCEPTED", "EDITED", "MERGED", "REJECTED"];
const AUDIT_RESULTS: [&str; 7] = ["VERIFIED", "VERIFIED_WITH_NOTE", "MISQUOTED", "NOT_IN_CORPUS", "SUPERSEDED", "NOT_APPLICABLE", "CUSTOM_SOURCE"];
pub const EXPORT_FOOTER_ID: &str = "caseforge.export_footer.v1";
/// Frozen G4 footer lines (contracts/constants.json export_footer_g4.lines).
pub const EXPORT_FOOTER_LINES: [&str; 3] = [
    "Local model output for preparation only. Not legal advice. Citations require pack verification.",
    "Preparation completeness only \u{2014} never win prediction, outcome odds, or verdict language.",
    "Export footer (G4) \u{b7} caseforge.export_footer.v1 \u{b7} verify every fact against its source pointer before filing.",
];

static PARA_REF: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\bpara(?:graph)?s?\s+(\d{1,3}(?:\s*(?:,|and|&)\s*\d{1,3})*)").unwrap());
static NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new(r"\d{1,3}").unwrap());
static DENY_WORDS: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b(denied|denies|deny|disputed|disputes|does not admit|do not admit|not admitted|is false|are false)\b").unwrap());
static ADMIT_WORDS: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(admitted|admits|admit|is correct|are correct|not disputed|true and correct)\b").unwrap());
static EVIDENCE_REF: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:annexure|exhibit|ex\.)\s*[A-Za-z]{0,3}-?\s*\d+[A-Za-z]?").unwrap());

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Party {
    pub party_id: String,
    pub display_name: String,
    pub role: String,
}

/// A party's pleading or brief: its page texts and whose it is (counsel assigns this).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filing {
    pub doc_id: String,
    pub party_id: String,
    pub pages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IssueIn {
    pub issue_id: String,
    pub text: String,
    pub origin: String,
    pub template_rule_id: Option<String>,
    pub decision: String,
    pub merged_into: Option<String>,
}

/// A citation or quotation the audit found in a filing, located by page and character range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Audited {
    pub doc_id: String,
    pub page_index: usize,
    pub char_start: usize,
    pub char_end: usize,
    pub item_id: String,
    pub result: String,
}

pub struct Inputs<'a> {
    pub case_id: &'a str,
    pub built_at: &'a str,
    pub parties: &'a [Party],
    pub filings: &'a [Filing],
    pub issues: &'a [IssueIn],
    pub audited: &'a [Audited],
}

pub struct Output {
    /// Validates against contracts/issue_matrix.schema.json.
    pub matrix: Value,
    /// Things the judge should know about how far the matrix can go (for example a party that has filed nothing).
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Fact,
    Argument,
    Other,
}

struct Para {
    aid: String,
    num: String,
    party: usize,
    doc_id: String,
    page_index: usize,
    char_start: usize,
    char_end: usize,
    text: String,
    kind: Kind,
    denies: bool,
    kw: HashSet<String>,
}

fn locator(p: &Para) -> Value {
    json!({"doc_id": p.doc_id, "page_index": p.page_index, "char_start": p.char_start, "char_end": p.char_end, "excerpt": p.text})
}

fn shared(a: &HashSet<String>, b: &HashSet<String>) -> usize {
    a.intersection(b).count()
}

fn paras_of(parties: &[Party], filings: &[Filing]) -> Result<Vec<Para>, String> {
    let mut out = Vec::new();
    for f in filings {
        let party = parties.iter().position(|p| p.party_id == f.party_id).ok_or_else(|| format!("CF_ISSUE_FILING_PARTY: {} is assigned to an unknown party", f.doc_id))?;
        for p in opposition::paragraphs(&f.pages) {
            let Paragraph { id, page_index, char_start, char_end, text, factual, argument, denies } = p;
            out.push(Para {
                aid: format!("{}:{}", f.doc_id, id),
                num: id.trim_start_matches('p').to_string(),
                party,
                doc_id: f.doc_id.clone(),
                page_index,
                char_start,
                char_end,
                kind: if argument { Kind::Argument } else if factual { Kind::Fact } else { Kind::Other },
                denies,
                kw: keywords(&text),
                text,
            });
        }
    }
    Ok(out)
}

/// For each paragraph, the paragraphs of the other side it names ("paragraph 2"), when exactly one filing of that side has
/// a paragraph with that number.
fn references(paras: &[Para]) -> Vec<Vec<usize>> {
    paras
        .iter()
        .map(|b| {
            let mut out: Vec<usize> = Vec::new();
            for m in PARA_REF.captures_iter(&b.text) {
                for n in NUMBER.find_iter(&m[1]) {
                    let mut by_party: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
                    for (i, p) in paras.iter().enumerate() {
                        if p.party != b.party && p.num == n.as_str() {
                            by_party.entry(p.party).or_default().push(i);
                        }
                    }
                    for (_, v) in by_party {
                        if v.len() == 1 && !out.contains(&v[0]) {
                            out.push(v[0]);
                        }
                    }
                }
            }
            out
        })
        .collect()
}

#[derive(PartialEq)]
enum Stance {
    Admit,
    Deny,
    None,
}

fn stance(text: &str) -> Stance {
    if DENY_WORDS.is_match(text) {
        Stance::Deny
    } else if ADMIT_WORDS.is_match(text) {
        Stance::Admit
    } else {
        Stance::None
    }
}

fn validate(inp: &Inputs) -> Result<(), String> {
    if inp.parties.len() < 2 {
        return Err("CF_ISSUE_MATRIX_PARTIES: the matrix compares two or more parties; add the parties to the matter".into());
    }
    let mut seen = HashSet::new();
    if !inp.parties.iter().all(|p| seen.insert(p.party_id.clone())) {
        return Err("CF_ISSUE_MATRIX_PARTIES: party ids must be unique".into());
    }
    let mut ids = HashSet::new();
    for i in inp.issues {
        if !ids.insert(i.issue_id.as_str()) {
            return Err(format!("CF_ISSUE_DUPLICATE: issue {} appears twice", i.issue_id));
        }
        if !DECISIONS.contains(&i.decision.as_str()) {
            return Err(format!("CF_ISSUE_DECISION: {} is not a decision", i.decision));
        }
        if i.text.trim().is_empty() {
            return Err(format!("CF_ISSUE_TEXT: issue {} has no text", i.issue_id));
        }
    }
    for i in inp.issues.iter().filter(|i| i.decision == "MERGED") {
        let target = i.merged_into.as_deref().and_then(|t| inp.issues.iter().find(|x| x.issue_id == t));
        if !matches!(target, Some(t) if t.decision == "ACCEPTED" || t.decision == "EDITED") {
            return Err(format!("CF_ISSUE_MERGE_TARGET: issue {} must be merged into an accepted issue", i.issue_id));
        }
    }
    Ok(())
}

pub fn build(inp: &Inputs) -> Result<Output, String> {
    validate(inp)?;
    let paras = paras_of(inp.parties, inp.filings)?;
    let refs = references(&paras);
    let filed: BTreeSet<usize> = paras.iter().map(|p| p.party).collect();
    let mut warnings = Vec::new();
    for (i, p) in inp.parties.iter().enumerate() {
        if !filed.contains(&i) {
            warnings.push(format!("No filing from {} ({}) is assigned, so an argument they did not answer cannot be shown.", p.display_name, p.role));
        }
    }
    let assigned = |issue: &IssueIn| -> BTreeSet<usize> {
        let kw = keywords(&issue.text);
        paras.iter().enumerate().filter(|(_, p)| shared(&kw, &p.kw) >= ASSIGN_MIN).map(|(i, _)| i).collect()
    };

    let mut issues = Vec::new();
    for issue in inp.issues {
        let live = issue.decision == "ACCEPTED" || issue.decision == "EDITED";
        let mut members: BTreeSet<usize> = BTreeSet::new();
        if live {
            members = assigned(issue);
            for m in inp.issues.iter().filter(|m| m.decision == "MERGED" && m.merged_into.as_deref() == Some(issue.issue_id.as_str())) {
                members.extend(assigned(m));
            }
        }
        let (positions, admitted, disputed, unanswered) = if live { content(inp, &paras, &refs, &members, &filed) } else { (vec![], vec![], vec![], vec![]) };
        issues.push(json!({
            "issue_id": issue.issue_id, "text": issue.text, "origin": issue.origin, "template_rule_id": issue.template_rule_id,
            "judge_decision": issue.decision, "merged_into": issue.merged_into,
            "positions": positions, "admitted_facts": admitted, "disputed_facts": disputed, "unanswered_arguments": unanswered,
        }));
    }
    let matrix = json!({
        "schema_version": SCHEMA_VERSION,
        "matrix_id": format!("im:{}", inp.case_id),
        "case_id": inp.case_id,
        "built_at": inp.built_at,
        "parties": inp.parties.iter().map(|p| json!({"party_id": p.party_id, "display_name": p.display_name, "role": p.role})).collect::<Vec<_>>(),
        "issues": issues,
        "neutrality": {"party_scoring": false, "outcome_suggestion": false, "record_only": true},
    });
    Ok(Output { matrix, warnings })
}

type Content = (Vec<Value>, Vec<Value>, Vec<Value>, Vec<Value>);

fn content(inp: &Inputs, paras: &[Para], refs: &[Vec<usize>], members: &BTreeSet<usize>, filed: &BTreeSet<usize>) -> Content {
    // Positions: every party gets one, in the order given, so both sides are always shown.
    let mut positions = Vec::new();
    for (pi, party) in inp.parties.iter().enumerate() {
        let mine: Vec<&Para> = members.iter().map(|&i| &paras[i]).filter(|p| p.party == pi).collect();
        let mut sentences = Vec::new();
        let mut evidence: Vec<Value> = Vec::new();
        let mut seen_evidence = HashSet::new();
        let mut authorities: Vec<Value> = Vec::new();
        let mut seen_items = HashSet::new();
        for p in &mine {
            let cites: Vec<&Audited> = inp
                .audited
                .iter()
                .filter(|a| a.doc_id == p.doc_id && a.page_index == p.page_index && a.char_start >= p.char_start && a.char_end <= p.char_end && AUDIT_RESULTS.contains(&a.result.as_str()))
                .collect();
            for a in &cites {
                if seen_items.insert(a.item_id.clone()) {
                    authorities.push(json!({"audit_item_id": a.item_id, "result": a.result}));
                }
            }
            let class = if p.kind == Kind::Fact {
                "RECORD"
            } else if !cites.is_empty() {
                "LAW"
            } else {
                "INFERENCE_FROM_RECORD"
            };
            sentences.push(json!({"text": p.text, "classification": class, "locators": [locator(p)]}));
            for m in EVIDENCE_REF.find_iter(&p.text) {
                let start = p.char_start + p.text[..m.start()].chars().count();
                let end = start + m.as_str().chars().count();
                if seen_evidence.insert((p.doc_id.clone(), p.page_index, start)) {
                    evidence.push(json!({"doc_id": p.doc_id, "page_index": p.page_index, "char_start": start, "char_end": end, "excerpt": m.as_str()}));
                }
            }
        }
        positions.push(json!({"party_id": party.party_id, "sentences": sentences, "evidence": evidence, "authorities": authorities}));
    }

    // Admitted and disputed facts.
    let mut admitted: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    let mut disputed: BTreeMap<usize, BTreeSet<usize>> = BTreeMap::new();
    for &bi in members {
        let b = &paras[bi];
        if !refs[bi].is_empty() {
            for &t in &refs[bi] {
                if !members.contains(&t) || paras[t].kind != Kind::Fact {
                    continue;
                }
                match stance(&b.text) {
                    Stance::Admit => {
                        admitted.entry(t).or_default().insert(bi);
                    }
                    Stance::Deny => {
                        disputed.entry(t).or_default().insert(bi);
                    }
                    Stance::None => {}
                }
            }
        } else if b.kind == Kind::Fact {
            for &ai in members {
                let a = &paras[ai];
                if a.party == b.party || a.kind != Kind::Fact || !refs[ai].is_empty() || shared(&a.kw, &b.kw) < RESPONSE_MIN {
                    continue;
                }
                if a.denies != b.denies {
                    disputed.entry(ai).or_default().insert(bi);
                } else if !a.denies {
                    admitted.entry(ai).or_default().insert(bi);
                }
            }
        }
    }
    admitted.retain(|t, _| !disputed.contains_key(t));
    let fact_ref = |target: usize, responders: &BTreeSet<usize>| -> Value {
        let mut parties: BTreeSet<usize> = BTreeSet::from([paras[target].party]);
        let mut locators = vec![locator(&paras[target])];
        for &r in responders {
            parties.insert(paras[r].party);
            locators.push(locator(&paras[r]));
        }
        json!({"fact_node_id": paras[target].aid, "locators": locators, "party_ids": parties.iter().map(|&p| inp.parties[p].party_id.clone()).collect::<Vec<_>>()})
    };
    let admitted_facts: Vec<Value> = admitted.iter().map(|(t, r)| fact_ref(*t, r)).collect();
    let disputed_facts: Vec<Value> = disputed.iter().map(|(t, r)| fact_ref(*t, r)).collect();

    // Unanswered arguments: only against a party that has filed something.
    let mut unanswered = Vec::new();
    for &ai in members {
        let a = &paras[ai];
        if a.kind != Kind::Argument {
            continue;
        }
        for (pi, party) in inp.parties.iter().enumerate() {
            if pi == a.party || !filed.contains(&pi) {
                continue;
            }
            let answered = paras.iter().enumerate().any(|(bi, b)| b.party == pi && bi != ai && (refs[bi].contains(&ai) || (members.contains(&bi) && shared(&a.kw, &b.kw) >= RESPONSE_MIN)));
            if !answered {
                unanswered.push(json!({"argument_node_id": a.aid, "by_party_id": inp.parties[a.party].party_id, "unanswered_by_party_id": party.party_id, "locator": locator(a)}));
            }
        }
    }
    (positions, admitted_facts, disputed_facts, unanswered)
}

/// A template skeleton from an installed ISSUE_FRAMING_TEMPLATE row.
#[derive(Debug, Clone)]
pub struct Skeleton {
    pub rule_id: String,
    pub template_id: String,
    pub text: String,
}

/// Issues of an installed ISSUE_FRAMING_TEMPLATE table for the matter type.
pub fn skeletons_for(tables: &[crate::rule_tables::RuleTable], jurisdiction: &str, matter_type: &str) -> Vec<Skeleton> {
    let mut out = Vec::new();
    for t in tables.iter().filter(|t| t.table_kind == "ISSUE_FRAMING_TEMPLATE" && t.jurisdiction.eq_ignore_ascii_case(jurisdiction)) {
        for row in &t.rows {
            if row.payload["matter_type"].as_str() != Some(matter_type) {
                continue;
            }
            for s in row.payload["issue_skeletons"].as_array().into_iter().flatten() {
                if let (Some(id), Some(text)) = (s["template_id"].as_str(), s["text"].as_str()) {
                    out.push(Skeleton { rule_id: row.rule_id.clone(), template_id: id.to_string(), text: text.to_string() });
                }
            }
        }
    }
    out
}

/// Matter types for which an installed template table has issues.
pub fn matter_types(tables: &[crate::rule_tables::RuleTable], jurisdiction: &str) -> Vec<String> {
    let mut v: BTreeSet<String> = BTreeSet::new();
    for t in tables.iter().filter(|t| t.table_kind == "ISSUE_FRAMING_TEMPLATE" && t.jurisdiction.eq_ignore_ascii_case(jurisdiction)) {
        for row in &t.rows {
            if let Some(m) = row.payload["matter_type"].as_str() {
                v.insert(m.to_string());
            }
        }
    }
    v.into_iter().collect()
}

/// Issues to PROPOSE to the judge: template skeletons and issues in the case graph that the judge has not yet decided.
/// Nothing proposed proceeds until the judge accepts, edits, merges or rejects it.
pub fn propose(existing: &[IssueIn], skeletons: &[Skeleton], graph_issues: &[(String, String)]) -> Vec<IssueIn> {
    let have: HashSet<&str> = existing.iter().map(|i| i.issue_id.as_str()).collect();
    let mut out = Vec::new();
    for s in skeletons {
        let id = format!("tpl:{}", s.template_id);
        if !have.contains(id.as_str()) && !out.iter().any(|i: &IssueIn| i.issue_id == id) {
            out.push(IssueIn { issue_id: id, text: s.text.clone(), origin: "TEMPLATE".into(), template_rule_id: Some(s.rule_id.clone()), decision: "PROPOSED".into(), merged_into: None });
        }
    }
    for (node_id, label) in graph_issues {
        let id = format!("graph:{node_id}");
        if !label.trim().is_empty() && !have.contains(id.as_str()) {
            out.push(IssueIn { issue_id: id, text: label.clone(), origin: "GRAPH_ISSUE".into(), template_rule_id: None, decision: "PROPOSED".into(), merged_into: None });
        }
    }
    out
}

/// Maps an audit report of a filing (made on the filing's pages joined by a blank line, as `opposition::filing_text`
/// does) to page-relative locations.
pub fn audited_from_report(doc_id: &str, pages: &[String], report: &crate::audit::AuditReport) -> Vec<Audited> {
    let mut starts = Vec::new();
    let mut at = 0usize;
    for p in pages {
        starts.push(at);
        at += p.chars().count() + 2;
    }
    report
        .items
        .iter()
        .filter_map(|i| {
            let page = starts.iter().rposition(|&s| s <= i.draft_char_start)?;
            let base = starts[page];
            let end = i.draft_char_end - base;
            (end <= pages[page].chars().count()).then(|| Audited { doc_id: doc_id.to_string(), page_index: page, char_start: i.draft_char_start - base, char_end: end, item_id: i.item_id.clone(), result: i.result.clone() })
        })
        .collect()
}

fn where_in(l: &Value) -> String {
    let page = l["page_index"].as_u64().map(|p| format!("page {}", p + 1)).unwrap_or_else(|| "page ?".into());
    format!("{} {}, characters {}\u{2013}{}", l["doc_id"].as_str().unwrap_or(""), page, l["char_start"], l["char_end"])
}

/// The bench memo (plain text): what is agreed, disputed and unanswered on each accepted issue, with where each statement
/// is on the record. Carries the G4 footer and a G5 provenance appendix. Nothing in it is generated by a model.
pub fn bench_memo(matrix: &Value, generated_at: &str) -> String {
    let name = |id: &str| matrix["parties"].as_array().into_iter().flatten().find(|p| p["party_id"] == id).map(|p| format!("{} ({})", p["display_name"].as_str().unwrap_or(""), p["role"].as_str().unwrap_or(""))).unwrap_or_else(|| id.to_string());
    let mut out = String::new();
    out.push_str("BENCH MEMO \u{2014} ISSUES AND THE RECORD\n");
    out.push_str("Prepared for the Court from the parties' filings. It shows where the filings agree, disagree, or are silent. It states no finding and suggests no answer to any issue.\n\n");
    out.push_str("Parties: ");
    out.push_str(&matrix["parties"].as_array().into_iter().flatten().map(|p| name(p["party_id"].as_str().unwrap_or(""))).collect::<Vec<_>>().join("; "));
    out.push_str("\n\n");
    let mut n = 0;
    for issue in matrix["issues"].as_array().into_iter().flatten() {
        if !matches!(issue["judge_decision"].as_str(), Some("ACCEPTED" | "EDITED")) {
            continue;
        }
        n += 1;
        out.push_str(&format!("ISSUE {n}: {}\n", issue["text"].as_str().unwrap_or("")));
        for pos in issue["positions"].as_array().into_iter().flatten() {
            out.push_str(&format!("  Pleaded by {}:\n", name(pos["party_id"].as_str().unwrap_or(""))));
            let sentences = pos["sentences"].as_array().cloned().unwrap_or_default();
            if sentences.is_empty() {
                out.push_str("    (no paragraph of their filings touches this issue)\n");
            }
            for s in &sentences {
                out.push_str(&format!("    - {} [{}; {}]\n", s["text"].as_str().unwrap_or("").replace('\n', " "), s["classification"].as_str().unwrap_or(""), where_in(&s["locators"][0])));
            }
            for e in pos["evidence"].as_array().into_iter().flatten() {
                out.push_str(&format!("    Refers to: {} [{}]\n", e["excerpt"].as_str().unwrap_or(""), where_in(e)));
            }
            for a in pos["authorities"].as_array().into_iter().flatten() {
                out.push_str(&format!("    Authority check {}: {}\n", a["audit_item_id"].as_str().unwrap_or(""), a["result"].as_str().unwrap_or("")));
            }
        }
        for (title, key) in [("Agreed facts", "admitted_facts"), ("Disputed facts", "disputed_facts")] {
            let facts = issue[key].as_array().cloned().unwrap_or_default();
            out.push_str(&format!("  {title}: {}\n", if facts.is_empty() { "none found".to_string() } else { String::new() }));
            for f in &facts {
                let who = f["party_ids"].as_array().into_iter().flatten().filter_map(Value::as_str).map(&name).collect::<Vec<_>>().join(", ");
                out.push_str(&format!("    - {} [{}] (paragraphs of: {who})\n", f["locators"][0]["excerpt"].as_str().unwrap_or("").replace('\n', " "), where_in(&f["locators"][0])));
            }
        }
        let un = issue["unanswered_arguments"].as_array().cloned().unwrap_or_default();
        out.push_str(&format!("  Arguments not answered in the other side's filings: {}\n", if un.is_empty() { "none found".to_string() } else { String::new() }));
        for u in &un {
            out.push_str(&format!("    - Argument of {}, no answer found from {}: {} [{}]\n", name(u["by_party_id"].as_str().unwrap_or("")), name(u["unanswered_by_party_id"].as_str().unwrap_or("")), u["locator"]["excerpt"].as_str().unwrap_or("").replace('\n', " "), where_in(&u["locator"])));
        }
        out.push('\n');
    }
    if n == 0 {
        out.push_str("No issue has been accepted yet.\n\n");
    }
    out.push_str("Machine-assisted sections: the matching of paragraphs to issues, and the lists of agreed, disputed and unanswered points, were produced by fixed rules applied to the filings (no language model). The judge's decisions on the issues are the judge's own.\n\n");
    for l in EXPORT_FOOTER_LINES {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&format!(
        "\nProvenance (G5): export_id {}; generated_at {generated_at}; template_id issue_matrix_bench_memo; case_id {}; footer_id {EXPORT_FOOTER_ID}; ai_spans: none (model_hash deterministic:case_graph, prompt_hash deterministic:template, retrieval_ids []).\n",
        matrix["matrix_id"].as_str().unwrap_or(""),
        matrix["case_id"].as_str().unwrap_or("")
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const CORE: &str = include_str!("../../golden/G-CHAMBERS/fixtures/issue_matrix_core.json");
    const CORE_SHA256: &str = "301c5742795323ea726c33207b7bee4ebfaa2ff2d522cc48ae3247dc74ce01da"; // committed before the engine (6c78a28)
    const FALLBACK: &str = include_str!("../../golden/G-CHAMBERS/fixtures/issue_matrix_fallback.json");
    const FALLBACK_SHA256: &str = "96a2321af5d5429203c8bb5b6ca81be1e35c8b7b0a80eb08d4ba1631139f9120";
    const CONSTANTS: &str = include_str!("../../contracts/constants.json");

    fn parse<T: serde::de::DeserializeOwned>(v: &Value) -> Vec<T> {
        serde_json::from_value(v.clone()).unwrap()
    }

    pub(crate) fn run(fx: &Value) -> Output {
        let parties: Vec<Party> = parse(&fx["parties"]);
        let filings: Vec<Filing> = parse(&fx["filings"]);
        let issues: Vec<IssueIn> = fx["issues"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| IssueIn { issue_id: i["issue_id"].as_str().unwrap().into(), text: i["text"].as_str().unwrap().into(), origin: i["origin"].as_str().unwrap().into(), template_rule_id: i["template_rule_id"].as_str().map(str::to_string), decision: i["decision"].as_str().unwrap().into(), merged_into: i["merged_into"].as_str().map(str::to_string) })
            .collect();
        let audited: Vec<Audited> = parse(&fx["audited"]);
        build(&Inputs { case_id: "case-1", built_at: "2026-01-01T00:00:00Z", parties: &parties, filings: &filings, issues: &issues, audited: &audited }).unwrap()
    }

    fn ids(v: &Value, key: &str) -> Vec<String> {
        v.as_array().unwrap().iter().map(|f| f[key].as_str().unwrap().to_string()).collect()
    }

    fn check_golden(raw: &str, pinned: &str, label: &str) -> Value {
        assert_eq!(hex::encode(Sha256::digest(raw.as_bytes())), pinned, "{label}: fixture changed since it was pinned");
        let fx: Value = serde_json::from_str(raw).unwrap();
        let out = run(&fx);
        let again = run(&fx);
        assert_eq!(out.matrix, again.matrix, "{label}: deterministic");
        let thresholds = &fx["thresholds"];
        assert_eq!((thresholds["assign_min_shared_keywords"].as_u64(), thresholds["response_min_shared_keywords"].as_u64()), (Some(ASSIGN_MIN as u64), Some(RESPONSE_MIN as u64)), "{label}: thresholds");
        for issue in out.matrix["issues"].as_array().unwrap() {
            let id = issue["issue_id"].as_str().unwrap();
            let e = &fx["expect"][id];
            assert_eq!(issue["judge_decision"], e["decision"], "{label}: {id} decision");
            if e.get("positions").is_none() {
                for k in ["positions", "admitted_facts", "disputed_facts", "unanswered_arguments"] {
                    assert!(issue[k].as_array().unwrap().is_empty(), "{label}: {id} is not accepted so {k} is empty");
                }
                continue;
            }
            for pos in issue["positions"].as_array().unwrap() {
                let party = pos["party_id"].as_str().unwrap();
                let got: Vec<String> = pos["sentences"].as_array().unwrap().iter().map(|s| {
                    // the sentence is the paragraph; its locator names it
                    let l = &s["locators"][0];
                    let fx_paras = fx["filings"].as_array().unwrap().iter().find(|f| f["doc_id"] == l["doc_id"]).unwrap();
                    let page = fx_paras["pages"][l["page_index"].as_u64().unwrap() as usize].as_str().unwrap();
                    let num: String = s["text"].as_str().unwrap().chars().take_while(|c| c.is_ascii_digit()).collect();
                    assert_eq!(page.chars().skip(l["char_start"].as_u64().unwrap() as usize).take(l["char_end"].as_u64().unwrap() as usize - l["char_start"].as_u64().unwrap() as usize).collect::<String>(), s["text"].as_str().unwrap(), "{label}: locator points at the paragraph text");
                    format!("{}:p{num}", l["doc_id"].as_str().unwrap())
                }).collect();
                let want: Vec<String> = e["positions"][party].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
                assert_eq!(got, want, "{label}: {id} position of {party}");
            }
            assert_eq!(ids(&issue["admitted_facts"], "fact_node_id"), e["admitted"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>(), "{label}: {id} admitted");
            assert_eq!(ids(&issue["disputed_facts"], "fact_node_id"), e["disputed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>(), "{label}: {id} disputed");
            let got: Vec<Vec<String>> = issue["unanswered_arguments"].as_array().unwrap().iter().map(|u| vec![u["argument_node_id"].as_str().unwrap().into(), u["by_party_id"].as_str().unwrap().into(), u["unanswered_by_party_id"].as_str().unwrap().into()]).collect();
            let want: Vec<Vec<String>> = e["unanswered"].as_array().unwrap().iter().map(|u| u.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()).collect();
            assert_eq!(got, want, "{label}: {id} unanswered");
        }
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::PathBuf::from(dir).join(format!("issue_matrix_{label}.json")), serde_json::to_vec_pretty(&out.matrix).unwrap()).unwrap();
        }
        out.matrix
    }

    #[test]
    fn golden_chambers_cross_referenced_pleadings() {
        let m = check_golden(CORE, CORE_SHA256, "core");
        // proposals: template skeletons and graph issues not yet decided
        let fx: Value = serde_json::from_str(CORE).unwrap();
        let issues: Vec<IssueIn> = m["issues"].as_array().unwrap().iter().map(|i| IssueIn { issue_id: i["issue_id"].as_str().unwrap().into(), text: i["text"].as_str().unwrap().into(), origin: "X".into(), template_rule_id: None, decision: i["judge_decision"].as_str().unwrap().into(), merged_into: None }).collect();
        let skeletons: Vec<Skeleton> = fx["templates"]["issue_skeletons"].as_array().unwrap().iter().map(|s| Skeleton { rule_id: fx["templates"]["rule_id"].as_str().unwrap().into(), template_id: s["template_id"].as_str().unwrap().into(), text: s["text"].as_str().unwrap().into() }).collect();
        let graph: Vec<(String, String)> = fx["graph_issues"].as_array().unwrap().iter().map(|g| (g["node_id"].as_str().unwrap().into(), g["label"].as_str().unwrap().into())).collect();
        let proposed: Vec<String> = propose(&issues, &skeletons, &graph).into_iter().map(|i| i.issue_id).collect();
        assert_eq!(proposed, fx["expect_proposed"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn golden_chambers_pairing_by_wording_when_nothing_is_cross_referenced() {
        check_golden(FALLBACK, FALLBACK_SHA256, "fallback");
    }

    #[test]
    fn evidence_authorities_and_classes_are_taken_from_the_paragraphs() {
        // Hand-derived from the core pleadings: plaint paragraph 1 refers to "Annexure P-1" (a factual paragraph: RECORD);
        // paragraph 3 is an argument that contains the audited citation (LAW) and is on the partition issue judge:5.
        let fx: Value = serde_json::from_str(CORE).unwrap();
        let m = run(&fx).matrix;
        let issue = |id: &str| m["issues"].as_array().unwrap().iter().find(|i| i["issue_id"] == id).unwrap().clone();
        let p = |iss: &Value, party: &str| iss["positions"].as_array().unwrap().iter().find(|x| x["party_id"] == party).unwrap().clone();
        let i1 = issue("judge:1");
        let plaintiff = p(&i1, "P");
        assert_eq!(plaintiff["sentences"][0]["classification"], "RECORD");
        assert_eq!(plaintiff["evidence"].as_array().unwrap().len(), 1);
        assert_eq!(plaintiff["evidence"][0]["excerpt"], "Annexure P-1");
        let page = fx["filings"][0]["pages"][0].as_str().unwrap();
        let (s, e) = (plaintiff["evidence"][0]["char_start"].as_u64().unwrap() as usize, plaintiff["evidence"][0]["char_end"].as_u64().unwrap() as usize);
        assert_eq!(page.chars().skip(s).take(e - s).collect::<String>(), "Annexure P-1", "evidence locator points at the reference");
        let i5 = issue("judge:5");
        let plaintiff = p(&i5, "P");
        assert_eq!(plaintiff["sentences"][0]["classification"], "LAW");
        assert_eq!(plaintiff["authorities"], json!([{"audit_item_id": "i1", "result": "VERIFIED"}]));
        assert!(p(&i5, "D")["authorities"].as_array().unwrap().is_empty());
        assert_eq!(p(&i5, "D")["sentences"][0]["classification"], "INFERENCE_FROM_RECORD");
    }

    #[test]
    fn merged_issue_contributes_to_its_target_and_shows_nothing_itself() {
        let fx: Value = serde_json::from_str(CORE).unwrap();
        let m = run(&fx).matrix;
        let merged = m["issues"].as_array().unwrap().iter().find(|i| i["issue_id"] == "graph:brothers").unwrap();
        assert_eq!(merged["merged_into"], "judge:1");
        assert!(merged["positions"].as_array().unwrap().is_empty());
        // Hand-derived: give the merged issue the words "paid repair charges". Plaint paragraph 5 (paid the repair charges,
        // Exhibit P-2) shares three of them, so it now appears under the target issue; nothing from the defendant does.
        let count = |m: &Value, party: &str| {
            let i1 = m["issues"].as_array().unwrap().iter().find(|i| i["issue_id"] == "judge:1").unwrap().clone();
            i1["positions"].as_array().unwrap().iter().find(|p| p["party_id"] == party).unwrap()["sentences"].as_array().unwrap().len()
        };
        assert_eq!((count(&m, "P"), count(&m, "D")), (1, 1));
        let mut changed = fx.clone();
        for i in changed["issues"].as_array_mut().unwrap() {
            if i["issue_id"] == "graph:brothers" {
                i["text"] = json!("Whether the plaintiff paid repair charges?");
            }
        }
        let with_merge = run(&changed).matrix;
        assert_eq!((count(&with_merge, "P"), count(&with_merge, "D")), (2, 1));
    }

    #[test]
    fn a_party_with_no_filing_is_never_shown_as_having_failed_to_answer() {
        let mut fx: Value = serde_json::from_str(CORE).unwrap();
        fx["filings"] = json!([fx["filings"][0].clone()]);
        let out = run(&fx);
        for i in out.matrix["issues"].as_array().unwrap() {
            assert!(i["unanswered_arguments"].as_array().unwrap().iter().all(|u| u["unanswered_by_party_id"] != "D"), "no filing from D, so nothing is 'unanswered by D'");
        }
        assert_eq!(out.warnings.len(), 1);
        assert!(out.warnings[0].contains("Anil Kumar"));
    }

    #[test]
    fn the_same_rules_apply_to_both_parties_when_their_roles_are_swapped() {
        // Neutrality (AC-S4-12.3): swap which party filed which pleading; the result is the mirror image, nothing else.
        let fx: Value = serde_json::from_str(CORE).unwrap();
        let mut swapped = fx.clone();
        swapped["filings"][0]["party_id"] = json!("D");
        swapped["filings"][1]["party_id"] = json!("P");
        let (a, b) = (run(&fx).matrix, run(&swapped).matrix);
        let summary = |m: &Value, mirror: bool| -> Vec<(String, usize, usize, usize, Vec<String>)> {
            let flip = |p: &str| if mirror { (if p == "P" { "D" } else { "P" }).to_string() } else { p.to_string() };
            m["issues"].as_array().unwrap().iter().map(|i| {
                let mut un: Vec<String> = i["unanswered_arguments"].as_array().unwrap().iter().map(|u| format!("{}>{}>{}", u["argument_node_id"].as_str().unwrap(), flip(u["by_party_id"].as_str().unwrap()), flip(u["unanswered_by_party_id"].as_str().unwrap()))).collect();
                un.sort();
                (i["issue_id"].as_str().unwrap().to_string(), i["admitted_facts"].as_array().unwrap().len(), i["disputed_facts"].as_array().unwrap().len(), i["positions"].as_array().unwrap().iter().map(|p| p["sentences"].as_array().unwrap().len()).sum(), un)
            }).collect()
        };
        assert_eq!(summary(&a, false), summary(&b, true));
    }

    #[test]
    fn invalid_inputs_are_refused_with_a_reason() {
        let fx: Value = serde_json::from_str(CORE).unwrap();
        let parties: Vec<Party> = parse(&fx["parties"]);
        let mk = |issues: Vec<IssueIn>, parties: &[Party]| build(&Inputs { case_id: "c", built_at: "t", parties, filings: &[], issues: &issues, audited: &[] }).err().unwrap_or_default();
        let issue = |id: &str, decision: &str, merged: Option<&str>| IssueIn { issue_id: id.into(), text: "Whether x?".into(), origin: "JUDGE_AUTHORED".into(), template_rule_id: None, decision: decision.into(), merged_into: merged.map(str::to_string) };
        assert!(mk(vec![], &parties[..1]).starts_with("CF_ISSUE_MATRIX_PARTIES"));
        assert!(mk(vec![issue("a", "ACCEPTED", None), issue("a", "ACCEPTED", None)], &parties).starts_with("CF_ISSUE_DUPLICATE"));
        assert!(mk(vec![issue("a", "MERGED", Some("b")), issue("b", "PROPOSED", None)], &parties).starts_with("CF_ISSUE_MERGE_TARGET"));
        assert!(mk(vec![issue("a", "MERGED", Some("zzz"))], &parties).starts_with("CF_ISSUE_MERGE_TARGET"));
        assert!(mk(vec![issue("a", "MAYBE", None)], &parties).starts_with("CF_ISSUE_DECISION"));
    }

    #[test]
    fn templates_are_read_from_installed_tables_for_the_matter_type() {
        let row = |rule_id: &str, matter: &str, skeletons: Value| crate::rule_tables::RuleRow {
            rule_id: rule_id.into(), table_id: "t".into(), table_kind: "ISSUE_FRAMING_TEMPLATE".into(), jurisdiction: "India".into(), effective_from: "2000-01-01".into(),
            effective_to: None, authorities: vec![], trust_state: "DEVELOPMENT_ONLY".into(), signed_by: None, payload: json!({"matter_type": matter, "issue_skeletons": skeletons}),
        };
        let tables = vec![
            crate::rule_tables::RuleTable { table_id: "t".into(), table_kind: "ISSUE_FRAMING_TEMPLATE".into(), jurisdiction: "India".into(), pack_id: "p".into(), rows: vec![
                row("r1", "PARTITION", json!([{"template_id": "a", "text": "A?"}, {"template_id": "b", "text": "B?"}])),
                row("r2", "SALE", json!([{"template_id": "c", "text": "C?"}])),
            ] },
            crate::rule_tables::RuleTable { table_id: "u".into(), table_kind: "LIMITATION_PERIOD".into(), jurisdiction: "India".into(), pack_id: "p".into(), rows: vec![row("r3", "PARTITION", json!([{"template_id": "z", "text": "Z?"}]))] },
        ];
        assert_eq!(matter_types(&tables, "india"), ["PARTITION", "SALE"]);
        let sk = skeletons_for(&tables, "India", "PARTITION");
        assert_eq!(sk.iter().map(|s| (s.rule_id.as_str(), s.template_id.as_str())).collect::<Vec<_>>(), [("r1", "a"), ("r1", "b")]);
        assert!(skeletons_for(&tables, "US", "PARTITION").is_empty(), "another jurisdiction's templates are not used");
    }

    #[test]
    fn proposals_skip_what_the_judge_has_already_decided() {
        let have = vec![IssueIn { issue_id: "tpl:a".into(), text: "t".into(), origin: "TEMPLATE".into(), template_rule_id: None, decision: "REJECTED".into(), merged_into: None }];
        let sk = vec![Skeleton { rule_id: "r".into(), template_id: "a".into(), text: "A?".into() }, Skeleton { rule_id: "r".into(), template_id: "b".into(), text: "B?".into() }];
        let p = propose(&have, &sk, &[("n1".into(), "G?".into()), ("n2".into(), " ".into())]);
        assert_eq!(p.iter().map(|i| i.issue_id.as_str()).collect::<Vec<_>>(), ["tpl:b", "graph:n1"]);
        assert!(p.iter().all(|i| i.decision == "PROPOSED"));
    }

    #[test]
    fn bench_memo_is_neutral_located_and_carries_the_footer() {
        let fx: Value = serde_json::from_str(CORE).unwrap();
        let m = run(&fx).matrix;
        let memo = bench_memo(&m, "2026-01-01T00:00:00Z");
        for line in EXPORT_FOOTER_LINES {
            assert!(memo.contains(line), "G4 footer line missing: {line}");
        }
        assert!(memo.contains("Provenance (G5)") && memo.contains("model_hash deterministic:case_graph") && memo.contains("retrieval_ids []"));
        assert!(memo.contains("ISSUE 1: Whether the plaintiff and the defendant are co-owners"));
        assert!(memo.contains("Disputed facts:") && memo.contains("Agreed facts:") && memo.contains("not answered in the other side's filings"));
        assert!(memo.contains("d-plaint page 1, characters"), "locators name the document and page");
        // proposed, rejected and merged issues are not in the memo
        assert!(!memo.contains("Whether costs should follow the event") && !memo.contains("necessary party"));
        let lower = memo.to_lowercase();
        for w in ["wins", "stronger", "weaker", "likely to succeed", "recommend", "should be decreed", "favours", "score"] {
            assert!(!lower.contains(w), "memo must not contain {w}");
        }
    }

    #[test]
    fn footer_lines_match_the_frozen_constants() {
        let c: Value = serde_json::from_str(CONSTANTS).unwrap();
        let lines: Vec<&str> = c["export_footer_g4"]["lines"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(lines, EXPORT_FOOTER_LINES);
        assert_eq!(c["export_footer_g4"]["footer_id"], EXPORT_FOOTER_ID);
    }

    #[test]
    fn audit_items_map_to_page_relative_positions() {
        let pages = vec!["1. Alpha beta.".to_string(), "2. See section 2 of the Partition Act, 1893.".to_string()];
        let joined = opposition::filing_text(&pages);
        let start = joined.find("section 2").unwrap();
        let item = crate::audit::AuditItem {
            item_id: "i9".into(), kind: "CITATION".into(), citation_kind: Some("STATUTE".into()), extraction_rule_id: None,
            draft_char_start: start, draft_char_end: start + "section 2 of the Partition Act, 1893".len(), draft_text: String::new(),
            attributed_citation_item_id: None, result: "VERIFIED".into(), resolved: None, pinpoint: crate::audit::Pinpoint { claimed: None, status: "NOT_CLAIMED".into() }, notes: vec![], misquote: None, proposition_ids: vec![],
        };
        let report = crate::audit::AuditReport {
            schema_version: "1".into(), audit_id: "a".into(), pipeline_id: "p".into(), generated_at: "t".into(), case_id: None,
            subject: crate::audit::Subject { subject_kind: "EXTERNAL_FILING".into(), doc_id: None, filing_party_id: None, sha256: String::new(), filename: String::new(), format: "txt".into() },
            pack_snapshot: vec![], model_fingerprint: None, items: vec![item], counts: Default::default(),
        };
        let a = audited_from_report("d1", &pages, &report);
        assert_eq!(a.len(), 1);
        assert_eq!((a[0].page_index, a[0].char_start), (1, "2. See ".len()));
        assert_eq!(pages[1].chars().skip(a[0].char_start).take(a[0].char_end - a[0].char_start).collect::<String>(), "section 2 of the Partition Act, 1893");
    }
}
