//! Code Bridge — statute succession and temporal routing (B-S4-09, PRD v4.0 §7.4).
//!
//! Every statutory reference ("section 302 IPC", "u/s 161 Cr.P.C.", "Rule 801 of the Federal Rules of Evidence") is
//! annotated ONLY from installed STATUTE_SUCCESSION rule rows: equivalence class, counterpart provision, and the
//! governing code computed from the rows' transition rules and the case's key dates. A missing basis date gives
//! NEEDS_COUNSEL_INPUT with an interview question — never a guess. No matching row gives UNKNOWN. MODIFIED mappings
//! carry the old and new texts side by side from the authoritative chunks. Deterministic; no model; no legal value is
//! written in code. Contract: contracts/statute_mapping.schema.json. Golden: golden/G-CODEBRIDGE.

use crate::audit::{AuditItem, Note};
use crate::rule_tables::{ChunkIndex, RuleRow};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

pub const PIPELINE_ID: &str = "caseforge.code-bridge.v1";
pub const DATE_KINDS: [&str; 5] = ["OFFENCE_DATE", "INSTITUTION_DATE", "FILING_DATE", "CAUSE_OF_ACTION_DATE", "EVENT_DATE"];

/// A case key date entered or confirmed by counsel, with where it came from.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyDate {
    pub kind: String,
    pub date: String,
    /// Counsel-input record or MatterLocator (contract: basis_locator).
    pub locator: Value,
}

#[derive(Debug, Clone)]
pub struct Reference {
    pub ref_text: String,
    pub code: String,
    pub provision: String,
    pub char_start: usize,
}

const SECTION_WORD: &str = r"(?i:sections?|secs?\.?|s\.|u/s\.?|rules?|articles?|art\.|§)";
const PROVISION: &str = r"(\d+[A-Z]{0,2}(?:\(\d+[A-Za-z]?\))*)";
static WITH_SECTION_WORD: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"\b{SECTION_WORD}\s*{PROVISION}\s+(?:of\s+(?:the\s+)?)?([A-Z][A-Za-z.]*(?:[ \t]+(?:of[ \t]+(?:the[ \t]+)?|and[ \t]+)?[A-Z][A-Za-z.]*){{0,6}}(?:,?[ \t]+\d{{4}})?)")).unwrap()
});
static SENTENCE_END: Lazy<Regex> = Lazy::new(|| Regex::new(r"\.\s+").unwrap());
static TRAILING_YEAR: Lazy<Regex> = Lazy::new(|| Regex::new(r",?\s+\d{4}$").unwrap());
static STATUTE_WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(act|code|sanhita|adhiniyam|rules|regulations|ordinance|constitution)\b").unwrap());

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// (full normalised name without year, acronym of a multi-word name). "Indian Penal Code, 1860" ->
/// ("indianpenalcode", Some("ipc")); "I.P.C." -> ("ipc", None). Purely lexical; the codes come from the rows.
fn code_forms(code: &str) -> (String, Option<String>) {
    let base = TRAILING_YEAR.replace(code.trim(), "").to_string();
    let words: Vec<&str> = base.split_whitespace().filter(|w| !matches!(w.to_lowercase().as_str(), "of" | "the" | "and")).collect();
    let acronym = (words.len() >= 2).then(|| words.iter().filter_map(|w| w.chars().next()).collect::<String>().to_lowercase());
    (norm(&base), acronym)
}

/// Code-name aliases shipped by installed official packs (`metadata/code_aliases.json`), e.g. "CrPC" =
/// "Code of Criminal Procedure". Each group holds full normalised names only (acronyms of long names collide:
/// "Code of Civil Procedure" and "Code of Criminal Procedure" are both "ccp").
static ALIASES: Lazy<std::sync::RwLock<Vec<Vec<String>>>> = Lazy::new(|| std::sync::RwLock::new(Vec::new()));

pub fn add_code_aliases(groups: &[Vec<String>]) {
    if let Ok(mut all) = ALIASES.write() {
        for names in groups {
            let mut g: Vec<String> = names.iter().map(|n| code_forms(n).0).filter(|n| !n.is_empty()).collect();
            g.sort();
            g.dedup();
            if g.len() > 1 && !all.contains(&g) {
                all.push(g);
            }
        }
    }
}

/// Same code? Full names must match exactly; an acronym only matches a one-word code ("Indian Penal Code" ~ "IPC"),
/// never another long name. Pack aliases join names a lawyer writes differently ("Code of Criminal Procedure" ~ "CrPC").
pub fn same_code(a: &str, b: &str) -> bool {
    let ((fa, aa), (fb, ab)) = (code_forms(a), code_forms(b));
    if fa.is_empty() || fb.is_empty() {
        return false;
    }
    if fa == fb || aa.as_deref() == Some(fb.as_str()) && ab.is_none() || ab.as_deref() == Some(fa.as_str()) && aa.is_none() {
        return true;
    }
    ALIASES.read().map(|all| all.iter().any(|g| g.contains(&fa) && g.contains(&fb))).unwrap_or(false)
}

fn provision_base(p: &str) -> String {
    norm(p.split('(').next().unwrap_or(p))
}

fn same_provision(a: &str, b: &str) -> bool {
    norm(a) == norm(b) || provision_base(a) == norm(b) || norm(a) == provision_base(b)
}

/// Statutory references in `text`. `known_codes` (from installed rows) also allows the short form "302 IPC".
pub fn find_references(text: &str, known_codes: &[String]) -> Vec<Reference> {
    let mut out: Vec<Reference> = Vec::new();
    // Scan from the end of each accepted reference, so a code cut short at a sentence end or "and" does not hide
    // the next reference.
    let mut pos = 0usize;
    while pos < text.len() {
        let Some(c) = WITH_SECTION_WORD.captures(&text[pos..]) else { break };
        let m = c.get(0).unwrap();
        let raw_code = c.get(2).unwrap();
        let (ms, rs) = (pos + m.start(), pos + raw_code.start());
        // A code never runs across a sentence end ("... DPC. Sedition ...") or into a following "and Section ...".
        let mut code = raw_code.as_str().to_string();
        if let Some(p) = SENTENCE_END.find(&code) {
            code.truncate(p.start());
        }
        if let Some(p) = code.find(" and ") {
            if !STATUTE_WORD.is_match(&code[p..]) || code[p..].contains(" and Section") {
                code.truncate(p);
            }
        }
        let code = code.trim_end_matches(['.', ',']).trim().to_string();
        let known = known_codes.iter().any(|k| same_code(k, &code));
        let acronym = code.len() >= 2 && code.chars().filter(|ch| ch.is_alphabetic()).all(|ch| ch.is_uppercase() || ch == 'r');
        if code.is_empty() || !(known || STATUTE_WORD.is_match(&code) || acronym) {
            pos = ms + text[ms..].chars().next().map(char::len_utf8).unwrap_or(1);
            continue;
        }
        let end = rs + code.len();
        out.push(Reference { ref_text: text[ms..end].to_string(), code, provision: c[1].to_string(), char_start: text[..ms].chars().count() });
        pos = end;
    }
    // Short form "302 IPC" for codes the installed rows know.
    for k in known_codes {
        let pattern = format!(r"\b{PROVISION}\s+({})\b", regex::escape(k));
        let Ok(re) = Regex::new(&pattern) else { continue };
        for c in re.captures_iter(text) {
            let m = c.get(0).unwrap();
            let start = text[..m.start()].chars().count();
            if out.iter().any(|r| start >= r.char_start && start < r.char_start + r.ref_text.chars().count()) {
                continue;
            }
            out.push(Reference { ref_text: m.as_str().to_string(), code: c[2].to_string(), provision: c[1].to_string(), char_start: start });
        }
    }
    out.sort_by_key(|r| r.char_start);
    out
}

fn provision_of(v: &Value) -> Option<(String, String)> {
    Some((v.get("code")?.as_str()?.to_string(), v.get("provision")?.as_str()?.to_string()))
}

/// Codes named anywhere in the rows (for reference matching).
pub fn known_codes(rows: &[&RuleRow]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for r in rows {
        let p = &r.payload;
        for c in [p.get("predecessor").and_then(provision_of).map(|x| x.0), p.get("successor").and_then(provision_of).map(|x| x.0), p.get("before_code").and_then(Value::as_str).map(str::to_string), p.get("after_code").and_then(Value::as_str).map(str::to_string)].into_iter().flatten() {
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out
}

fn excerpt(chunks: &ChunkIndex, a: &Value) -> Option<Value> {
    let chunk_id = a.get("chunk_id")?.as_str()?;
    let c = chunks.get(chunk_id)?;
    let text: String = match (a.get("char_start").and_then(Value::as_u64), a.get("char_end").and_then(Value::as_u64)) {
        (Some(s), Some(e)) => c.text.chars().skip(s as usize).take((e - s) as usize).collect(),
        _ => c.text.clone(),
    };
    Some(json!({"pack_id": c.pack_id, "source_id": c.source_id, "chunk_id": chunk_id, "excerpt": text}))
}

fn side_by_side(row: &RuleRow, chunks: &ChunkIndex) -> Option<Value> {
    let label_of = |side: &str| row.payload.get(side).and_then(provision_of).map(|(c, p)| format!("{c} {p}"));
    let find = |label: Option<String>| {
        let label = label?;
        row.authorities.iter().find(|a| a.get("provision_label").and_then(Value::as_str).map(|l| norm(l) == norm(&label) || provision_base(l) == provision_base(&label)).unwrap_or(false)).cloned()
    };
    let old = find(label_of("predecessor")).or_else(|| row.authorities.first().cloned())?;
    let new = find(label_of("successor")).or_else(|| row.authorities.get(1).cloned())?;
    if old == new {
        return None;
    }
    Some(json!({"old": excerpt(chunks, &old)?, "new": excerpt(chunks, &new)?}))
}

fn governing(code: &str, mapping: Option<&RuleRow>, rows: &[&RuleRow], key_dates: &HashMap<String, KeyDate>) -> (Value, Option<String>) {
    let listed: Vec<String> = mapping
        .and_then(|m| m.payload.get("transition_rule_ids"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    let transitions: Vec<&&RuleRow> = rows
        .iter()
        .filter(|r| r.payload.get("entry_type").and_then(Value::as_str) == Some("TRANSITION"))
        .filter(|r| {
            let p = &r.payload;
            listed.contains(&r.rule_id) || [p["before_code"].as_str(), p["after_code"].as_str()].into_iter().flatten().any(|c| same_code(c, code))
        })
        .collect();
    let unknown = json!({"status": "UNKNOWN", "governing_code": null, "basis_date_kind": null, "basis_date": null, "basis_locator": null, "transition_rule_id": null, "interview_question_id": null});
    let Some(first) = transitions.first() else { return (unknown, None) };
    let mut decided: Vec<(String, &KeyDate, &RuleRow)> = Vec::new();
    for t in &transitions {
        let kind = t.payload["governed_by"].as_str().unwrap_or("");
        let Some(date) = key_dates.get(kind) else {
            return (
                json!({"status": "NEEDS_COUNSEL_INPUT", "governing_code": null, "basis_date_kind": kind, "basis_date": null, "basis_locator": null,
                       "transition_rule_id": t.rule_id, "interview_question_id": format!("keydate:{kind}")}),
                Some(t.rule_id.clone()),
            );
        };
        let switch = t.payload["switch_date"].as_str().unwrap_or("");
        let code = if date.date.as_str() >= switch { &t.payload["after_code"] } else { &t.payload["before_code"] };
        decided.push((code.as_str().unwrap_or("").to_string(), date, **t));
    }
    if decided.iter().any(|d| d.0 != decided[0].0) {
        // Two transition rules point to different codes: counsel decides which basis applies.
        let kind = first.payload["governed_by"].as_str().unwrap_or("");
        return (
            json!({"status": "NEEDS_COUNSEL_INPUT", "governing_code": null, "basis_date_kind": kind, "basis_date": null, "basis_locator": null,
                   "transition_rule_id": first.rule_id, "interview_question_id": "keydate:conflicting-transition-rules"}),
            Some(first.rule_id.clone()),
        );
    }
    let (code, date, t) = &decided[0];
    (
        json!({"status": "DETERMINED", "governing_code": code, "basis_date_kind": date.kind, "basis_date": date.date, "basis_locator": date.locator,
               "transition_rule_id": t.rule_id, "interview_question_id": null}),
        Some(t.rule_id.clone()),
    )
}

/// One annotation per contracts/statute_mapping.schema.json, plus the trust of the rows it relied on
/// (SIGNED_OFF, DEVELOPMENT_ONLY, or NONE for UNKNOWN) — unsigned rows never make anything verified (AC-S4-RT.2).
#[derive(Debug, Clone, Serialize)]
pub struct Annotated {
    pub annotation: Value,
    pub trust: String,
    pub char_start: usize,
}

pub fn annotate(text: &str, context: &str, context_ref_id: Option<&str>, rows: &[&RuleRow], chunks: &ChunkIndex, key_dates: &HashMap<String, KeyDate>) -> Vec<Annotated> {
    let codes = known_codes(rows);
    find_references(text, &codes)
        .into_iter()
        .map(|r| {
            let mapping = rows
                .iter()
                .filter(|row| row.payload.get("entry_type").and_then(Value::as_str) == Some("MAPPING"))
                .filter_map(|row| {
                    let pred = row.payload.get("predecessor").and_then(provision_of);
                    let succ = row.payload.get("successor").and_then(provision_of);
                    let hit = |p: &Option<(String, String)>| p.as_ref().map(|(c, pr)| same_code(c, &r.code) && same_provision(pr, &r.provision)).unwrap_or(false);
                    if hit(&pred) {
                        Some((*row, "SUCCESSOR", pred.unwrap(), succ))
                    } else if hit(&succ) {
                        Some((*row, "PREDECESSOR", succ.unwrap(), pred))
                    } else {
                        None
                    }
                })
                .max_by_key(|(row, ..)| row.signed());
            let (code, provision) = mapping.as_ref().map(|m| (m.2 .0.clone(), r.provision.clone())).unwrap_or_else(|| (r.code.clone(), r.provision.clone()));
            let (equivalence, counterpart, mut rule_ids, sbs, trust) = match &mapping {
                Some((row, direction, _, other)) => {
                    let eq = row.payload["equivalence"].as_str().unwrap_or("UNKNOWN").to_string();
                    let counterpart = other.as_ref().map(|(c, p)| json!({"direction": direction, "code": c, "provision": p})).unwrap_or(Value::Null);
                    let sbs = if eq == "MODIFIED" { side_by_side(row, chunks).unwrap_or(Value::Null) } else { Value::Null };
                    (eq, counterpart, vec![row.rule_id.clone()], sbs, if row.signed() { "SIGNED_OFF" } else { "DEVELOPMENT_ONLY" })
                }
                None => ("UNKNOWN".to_string(), Value::Null, vec![], Value::Null, "NONE"),
            };
            let (gov, transition) = governing(&code, mapping.as_ref().map(|m| m.0), rows, key_dates);
            if equivalence != "UNKNOWN" {
                if let Some(t) = transition {
                    if !rule_ids.contains(&t) {
                        rule_ids.push(t);
                    }
                }
            }
            let trust = if trust != "NONE" && rows.iter().filter(|row| rule_ids.contains(&row.rule_id)).any(|row| !row.signed()) { "DEVELOPMENT_ONLY" } else { trust };
            let id = crate::certificate::sha256_hex(format!("{context}|{}|{}|{}", context_ref_id.unwrap_or(""), r.char_start, r.ref_text).as_bytes());
            Annotated {
                annotation: json!({
                    "schema_version": "1.0.0",
                    "reference_id": format!("cb-{}", &id[..12]),
                    "context": context,
                    "context_ref_id": context_ref_id,
                    "ref_text": r.ref_text,
                    "code": code,
                    "provision": provision,
                    "equivalence": equivalence,
                    "counterpart": counterpart,
                    "rule_ids": rule_ids,
                    "governing": gov,
                    "side_by_side": sbs,
                }),
                trust: trust.to_string(),
                char_start: r.char_start,
            }
        })
        .collect()
}

/// AUDIT annotation: a VERIFIED statute citation whose provision has a SIGNED_OFF renumbering, modification or
/// omission becomes VERIFIED_WITH_NOTE with a CODE_BRIDGE note. Development-only rows change nothing.
pub fn add_audit_notes(items: &mut [AuditItem], counts: &mut BTreeMap<String, usize>, rows: &[&RuleRow], chunks: &ChunkIndex, key_dates: &HashMap<String, KeyDate>) {
    for item in items.iter_mut().filter(|i| i.kind == "CITATION" && i.result == "VERIFIED") {
        let notes: Vec<Note> = annotate(&item.draft_text, "AUDIT_ITEM", Some(&item.item_id), rows, chunks, key_dates)
            .into_iter()
            .filter(|a| a.trust == "SIGNED_OFF" && matches!(a.annotation["equivalence"].as_str(), Some("RENUMBERED" | "MODIFIED" | "OMITTED")))
            .map(|a| {
                let n = &a.annotation;
                let now = match (n["counterpart"]["code"].as_str(), n["counterpart"]["provision"].as_str()) {
                    (Some(c), Some(p)) => format!("{} {} ({})", c, p, n["equivalence"].as_str().unwrap_or("")),
                    _ => format!("no counterpart ({})", n["equivalence"].as_str().unwrap_or("")),
                };
                Note {
                    note_kind: "CODE_BRIDGE".into(),
                    text: format!("{} {}: {}{}", n["code"].as_str().unwrap_or(""), n["provision"].as_str().unwrap_or(""), if n["counterpart"]["direction"] == "PREDECESSOR" { "earlier " } else { "now " }, now),
                    ref_id: n["rule_ids"][0].as_str().map(str::to_string),
                }
            })
            .collect();
        if !notes.is_empty() {
            item.notes.extend(notes);
            item.result = "VERIFIED_WITH_NOTE".into();
            let verified = counts.get("VERIFIED").copied().unwrap_or(1).saturating_sub(1);
            counts.insert("VERIFIED".into(), verified);
            *counts.entry("VERIFIED_WITH_NOTE".into()).or_default() += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rule_tables::tests::{dev_only, index, FIXTURE};
    use crate::rule_tables::{query, validate_table, RuleTable};

    fn table(fx: &Value, signed: bool) -> RuleTable {
        let t = if signed { fx["rule_table"].clone() } else { dev_only(fx) };
        validate_table(&t, "official", &index(fx)).unwrap()
    }

    fn dates(v: &Value) -> HashMap<String, KeyDate> {
        v.as_object()
            .unwrap()
            .iter()
            .map(|(k, d)| (k.clone(), KeyDate { kind: k.clone(), date: d.as_str().unwrap().into(), locator: json!({"kind": "COUNSEL_INPUT", "entered_by": "counsel"}) }))
            .collect()
    }

    fn run(fx: &Value, scenario: usize, signed: bool) -> Vec<Annotated> {
        let tables = vec![table(fx, signed)];
        let rows = query(&tables, "STATUTE_SUCCESSION", Some("Demoland"), "2026-10-08");
        annotate(fx["draft"].as_str().unwrap(), "DRAFT", None, &rows, &index(fx), &dates(&fx["scenarios"][scenario]["key_dates"]))
    }

    fn find<'a>(got: &'a [Annotated], m: &str) -> &'a Annotated {
        got.iter().find(|a| a.annotation["ref_text"].as_str().map(|t| t.contains(m) || m.contains(t)).unwrap_or(false)).unwrap_or_else(|| panic!("no annotation for '{m}': {:#?}", got.iter().map(|a| &a.annotation["ref_text"]).collect::<Vec<_>>()))
    }

    #[test]
    fn golden_mappings_with_known_dates() {
        // AC-S4-04.1 (engine side, against fixture sign-off) and AC-S4-04.3
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let got = run(&fx, 0, true);
        let expected = fx["scenarios"][0]["expected"].as_array().unwrap();
        assert_eq!(got.len(), expected.len(), "{:#?}", got.iter().map(|a| &a.annotation["ref_text"]).collect::<Vec<_>>());
        for e in expected {
            let a = &find(&got, e["match"].as_str().unwrap()).annotation;
            let m = e["match"].as_str().unwrap();
            assert_eq!(a["code"], e["code"], "{m}");
            assert_eq!(a["provision"], e["provision"], "{m}");
            assert_eq!(a["equivalence"], e["equivalence"], "{m}");
            assert_eq!(a["counterpart"], e["counterpart"], "{m}");
            assert_eq!(a["governing"]["governing_code"], e["governing_code"], "{m}: {}", a["governing"]);
            assert_eq!(a["governing"]["basis_date_kind"], e["basis_date_kind"], "{m}");
            assert_eq!(a["rule_ids"], e["rule_ids"], "{m}");
            if e["side_by_side"].as_bool().unwrap() {
                let sbs = &a["side_by_side"];
                for (side, chunk) in [("old", "dpc-420"), ("new", "dns-318")] {
                    let want = fx["chunks"].as_array().unwrap().iter().find(|c| c["chunk_id"].as_str().unwrap().ends_with(chunk)).unwrap();
                    assert_eq!(sbs[side]["excerpt"], want["text"], "{m} {side}");
                }
            } else {
                assert!(a["side_by_side"].is_null(), "{m}");
            }
            let status = if e["governing_code"].is_null() { "UNKNOWN" } else { "DETERMINED" };
            assert_eq!(a["governing"]["status"], status, "{m}");
        }
    }

    #[test]
    fn missing_key_date_always_needs_counsel_input() {
        // AC-S4-04.2
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let got = run(&fx, 1, true);
        let needs: Vec<&Annotated> = got.iter().filter(|a| a.annotation["governing"]["status"] == "NEEDS_COUNSEL_INPUT").collect();
        assert_eq!(needs.len() as u64, fx["scenarios"][1]["planted_missing_date_refs"].as_u64().unwrap());
        for a in &needs {
            let g = &a.annotation["governing"];
            assert!(g["governing_code"].is_null(), "never a guess: {g}");
            assert_eq!(g["interview_question_id"], json!(format!("keydate:{}", g["basis_date_kind"].as_str().unwrap())));
        }
        assert!(got.iter().filter(|a| a.annotation["governing"]["status"] != "NEEDS_COUNSEL_INPUT").all(|a| a.annotation["equivalence"] == "UNKNOWN"));
    }

    #[test]
    fn switch_date_boundary() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let got = run(&fx, 2, true);
        for (m, code) in fx["scenarios"][2]["expected_governing"].as_object().unwrap() {
            assert_eq!(&find(&got, m).annotation["governing"]["governing_code"], code, "{m}");
        }
    }

    #[test]
    fn unsigned_rows_never_verify_and_signed_rows_annotate_audit() {
        // AC-S4-RT.2 + AUDIT integration (VERIFIED_WITH_NOTE only from signed rows).
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let cite = fx["audit_integration"]["citation"].as_str().unwrap();
        let item = || -> AuditItem {
            serde_json::from_value(json!({"item_id": "i1", "kind": "CITATION", "citation_kind": "STATUTE", "extraction_rule_id": null, "draft_char_start": 0,
                "draft_char_end": cite.len(), "draft_text": cite, "attributed_citation_item_id": null, "result": "VERIFIED", "resolved": null,
                "pinpoint": {"claimed": null, "status": "NO_PINPOINT"}, "notes": [], "misquote": null, "proposition_ids": []}))
            .unwrap()
        };
        let chunks = index(&fx);
        for signed in [false, true] {
            let tables = vec![table(&fx, signed)];
            let rows = query(&tables, "STATUTE_SUCCESSION", None, "2026-10-08");
            let mut items = vec![item()];
            let mut counts = BTreeMap::from([("VERIFIED".to_string(), 1usize)]);
            add_audit_notes(&mut items, &mut counts, &rows, &chunks, &HashMap::new());
            let all = annotate(fx["draft"].as_str().unwrap(), "DRAFT", None, &rows, &chunks, &HashMap::new());
            if signed {
                assert_eq!(items[0].result, "VERIFIED_WITH_NOTE");
                let note = &items[0].notes[0];
                assert_eq!(note.note_kind, "CODE_BRIDGE");
                for want in fx["audit_integration"]["expected_note_contains"].as_array().unwrap() {
                    assert!(note.text.contains(want.as_str().unwrap()), "{}", note.text);
                }
                assert_eq!(counts["VERIFIED_WITH_NOTE"], 1);
                assert!(all.iter().filter(|a| a.annotation["equivalence"] != "UNKNOWN").all(|a| a.trust == "SIGNED_OFF"));
            } else {
                assert_eq!(items[0].result, "VERIFIED", "development-only rows never upgrade an audit result");
                assert!(items[0].notes.is_empty());
                assert!(all.iter().filter(|a| a.annotation["equivalence"] != "UNKNOWN").all(|a| a.trust == "DEVELOPMENT_ONLY"));
            }
        }
    }

    #[test]
    fn annotations_match_frozen_contract_shape() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let contract: Value = serde_json::from_str(include_str!("../../contracts/statute_mapping.schema.json")).unwrap();
        let mut all = Vec::new();
        for scenario in 0..3 {
            for a in run(&fx, scenario, true) {
                for k in contract["required"].as_array().unwrap() {
                    assert!(a.annotation.get(k.as_str().unwrap()).is_some(), "missing {k}");
                }
                all.push(a.annotation);
            }
        }
        if let Ok(dir) = std::env::var("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("code_bridge_annotations.json"), serde_json::to_string_pretty(&all).unwrap()).unwrap();
        }
    }

    #[test]
    fn real_world_reference_forms_are_found() {
        let refs = find_references("Charged u/s 302 I.P.C. and Section 65B of the Indian Evidence Act, 1872; see Rule 801 of the Federal Rules of Evidence and 420 IPC.", &["IPC".into()]);
        let got: Vec<(String, String)> = refs.iter().map(|r| (r.code.clone(), r.provision.clone())).collect();
        assert!(got.contains(&("I.P.C".into(), "302".into())), "{got:?}");
        assert!(got.contains(&("Indian Evidence Act, 1872".into(), "65B".into())), "{got:?}");
        assert!(got.contains(&("Federal Rules of Evidence".into(), "801".into())), "{got:?}");
        assert!(got.contains(&("IPC".into(), "420".into())), "{got:?}");
        assert!(same_code("I.P.C", "IPC") && same_code("Indian Penal Code, 1860", "IPC") && !same_code("IPC", "CrPC"));
    }

    #[test]
    fn shipped_criminal_codes_pack_installs_and_bridges() {
        // The real official pack (IPC/CrPC/IEA -> BNS/BNSS/BSA) through the real installer and loader.
        let pack = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../packs/official/caseforge.india.criminal-codes");
        assert!(pack.exists(), "criminal-codes pack is shipped in packs/official");
        let vault = tempfile::tempdir().unwrap();
        let paths = crate::vault::VaultPaths::new(vault.path().to_path_buf());
        crate::packs::install_pack_from_dir(&paths, &pack).expect("official pack installs");
        let (tables, rejected) = crate::rule_tables::load_installed(&paths);
        assert!(rejected.is_empty(), "{rejected:?}");
        let rows = query(&tables, "STATUTE_SUCCESSION", Some("India"), "2026-10-08");
        assert!(rows.len() > 1000, "{} rows", rows.len());
        assert!(rows.iter().all(|r| !r.signed()), "no row counts as signed until a named legal editor signs it");
        let corpus = crate::retrieval::build_corpus(&paths, &[]).unwrap();
        let chunks = crate::rule_tables::index_corpus(&corpus);
        let dates = HashMap::from([("OFFENCE_DATE".to_string(), KeyDate { kind: "OFFENCE_DATE".into(), date: "2024-03-01".into(), locator: json!({"kind": "COUNSEL_INPUT"}) })]);
        let got = annotate("He is charged under section 302 IPC. His statement was recorded u/s 161 of the Code of Criminal Procedure, 1973.", "DRAFT", None, &rows, &chunks, &dates);
        let a302 = &got.iter().find(|a| a.annotation["provision"] == "302").expect("302 IPC annotated").annotation;
        assert_eq!(a302["counterpart"], json!({"direction": "SUCCESSOR", "code": "BNS", "provision": "103(1)"}), "{a302}");
        assert_eq!(a302["equivalence"], "RENUMBERED");
        assert_eq!(a302["governing"]["governing_code"], "IPC", "offence before 1 July 2024");
        let a161 = got.iter().find(|a| a.annotation["provision"] == "161").expect("161 CrPC annotated (long name via pack aliases)");
        assert_eq!(a161.annotation["code"], "CrPC");
        assert_eq!(a161.annotation["counterpart"]["code"], "BNSS");
        assert_eq!(a161.annotation["counterpart"]["provision"], "180");
        assert_eq!(a161.annotation["governing"]["status"], "NEEDS_COUNSEL_INPUT", "institution date not given");
        assert!(got.iter().all(|a| a.trust == "DEVELOPMENT_ONLY"));
    }

    #[test]
    fn pack_aliases_join_long_and_short_code_names() {
        assert!(!same_code("Code of Criminal Procedure, 1973", "TstCrPC"));
        add_code_aliases(&[vec!["TstCrPC".into(), "Code of Criminal Procedure".into(), "Criminal Procedure Code".into()]]);
        assert!(same_code("Code of Criminal Procedure, 1973", "TstCrPC"));
        assert!(same_code("Criminal Procedure Code", "TstCrPC"));
        assert!(!same_code("Code of Civil Procedure", "TstCrPC"));
        assert!(!same_code("Code of Civil Procedure, 1908", "Code of Criminal Procedure, 1973"), "long-name acronyms never match each other");
    }
}
