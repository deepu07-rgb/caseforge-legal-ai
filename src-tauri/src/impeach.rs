//! Contradiction engine and Impeachment Kit (B-S4-08, PRD v4.0 §7.3).
//!
//! Two layers over one witness's statements (built by `witness`):
//!   * Deterministic: date, time, amount, count, place, person, presence and sequence conflicts. Each later statement
//!     is compared with the MOST RECENT earlier statement that speaks to the same matter; OMISSION flags a material
//!     fact (identification, weapon, injury) in testimony that no earlier statement mentions — a counsel-judgement
//!     candidate, never a conclusion.
//!   * Model-proposed (SEMANTIC): candidates from the local model render ONLY if both quotes pass the B-S3-14
//!     exact-substring verifier against two different statements of the same witness; failures are discarded.
//! Every rendered finding carries exact quotes and locators on both sides (AC-S4-03.2).
//! Contract: contracts/impeachment_kit.schema.json. Golden: golden/G-IMPEACH.

use crate::proposition::{self, PropositionInput, SupportSource, VerifyRequest};
use crate::witness::{statement_id, Assertion, BuiltWitness, VOCAB};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};

pub const PIPELINE_ID: &str = "caseforge.impeachment.v1";
pub const EXPORT_FOOTER_ID: &str = "caseforge.export_footer.v1";
const DIMENSIONS: [&str; 8] = ["DATE", "TIME", "AMOUNT", "COUNT", "PLACE", "PERSON", "PRESENCE", "SEQUENCE"];
/// Suggested confrontation order (preparation guidance): identity and presence first, peripheral detail last.
const CONFRONT_ORDER: [&str; 11] = ["PERSON", "PRESENCE", "MATERIAL_FACT", "DATE", "TIME", "PLACE", "SEQUENCE", "COUNT", "AMOUNT", "SEMANTIC", "OTHER"];
/// Times this far apart or more are a contradiction ("about 9 pm" vs "9.20 pm" is not).
const TIME_TOLERANCE_MIN: u32 = 45;

fn has(a: &Assertion, d: &str) -> bool {
    let f = &a.facets;
    match d {
        "DATE" => !f.dates.is_empty(),
        "TIME" => !f.times.is_empty(),
        "AMOUNT" => !f.amounts.is_empty(),
        "COUNT" => !f.counts.is_empty(),
        "PLACE" => !f.places.is_empty(),
        "PERSON" => !f.agents.is_empty(),
        "PRESENCE" => !f.presence.is_empty(),
        "SEQUENCE" => f.sequence.is_some(),
        _ => false,
    }
}

fn subset(a: &[String], b: &[String]) -> bool {
    a.iter().all(|x| b.contains(x))
}

fn conflict(e: &Assertion, l: &Assertion, d: &str) -> bool {
    let (fe, fl) = (&e.facets, &l.facets);
    match d {
        "DATE" => !fe.dates.iter().any(|x| fl.dates.contains(x)),
        "TIME" => fe.times.iter().all(|a| fl.times.iter().all(|b| a.abs_diff(*b) >= TIME_TOLERANCE_MIN)),
        "AMOUNT" => !fe.amounts.iter().any(|x| fl.amounts.contains(x)),
        "COUNT" => !fe.counts.iter().any(|(a0, a1)| fl.counts.iter().any(|(b0, b1)| a0 <= b1 && b0 <= a1)),
        "PLACE" => !fe.places.iter().any(|p| fl.places.iter().any(|q| subset(p, q) || subset(q, p))),
        "PERSON" => fl.agents.iter().any(|(n2, v)| {
            fe.agents.iter().any(|(n1, v1)| v1 == v && n1 != n2) && !fe.agents.iter().any(|(n1, v1)| v1 == v && n1 == n2)
        }),
        "PRESENCE" => fl.presence.iter().any(|(n2, p2)| {
            fe.presence.iter().any(|(n1, p1)| match (n1, n2) {
                (Some(a), Some(b)) => a == b && p1 != p2,
                (None, Some(_)) => !*p1 && *p2,
                (Some(_), None) => *p1 && !*p2,
                (None, None) => false,
            })
        }),
        "SEQUENCE" => matches!((&fe.sequence, &fl.sequence), (Some((a, b)), Some((c, d))) if a == d && b == c),
        _ => false,
    }
}

fn overlap(a: &Assertion, b: &Assertion) -> usize {
    a.keywords.intersection(&b.keywords).count()
}

/// Does earlier assertion `e` speak to the same matter as later assertion `l` for dimension `d`?
fn topical(e: &Assertion, l: &Assertion, d: &str, single_time: &dyn Fn(&str) -> bool) -> bool {
    match d {
        "SEQUENCE" => {
            let (Some((a, b)), Some((c, dd))) = (&e.facets.sequence, &l.facets.sequence) else { return false };
            (a == c && b == dd) || (a == dd && b == c)
        }
        "PERSON" => overlap(e, l) >= 2 && e.facets.agents.iter().any(|(_, v)| l.facets.agents.iter().any(|(_, w)| v == w)),
        // A witness usually gives one time for the incident: if each statement has exactly one, they are compared.
        "TIME" => overlap(e, l) >= 2 || (single_time(&e.statement_id) && single_time(&l.statement_id)),
        _ => overlap(e, l) >= 2,
    }
}

fn finding_id(witness_id: &str, kind: &str, dimension: &str, earlier: &str, later: &str) -> String {
    let digest = crate::certificate::sha256_hex(format!("{witness_id}|{kind}|{dimension}|{earlier}|{later}").as_bytes());
    format!("f-{}", &digest[..12])
}

fn side(a: &Assertion) -> Value {
    json!({"statement_id": a.statement_id, "quote": a.excerpt, "locator": a.locator(), "absence_note": Value::Null})
}

fn statement_label(b: &BuiltWitness, sid: &str) -> String {
    b.witness
        .statements
        .iter()
        .find(|s| statement_id(&s.doc_id) == sid)
        .map(|s| format!("{} ({})", VOCAB.label(&s.statement_kind), s.recorded_on.clone().unwrap_or_else(|| "undated".into())))
        .unwrap_or_else(|| sid.to_string())
}

/// Deterministic contradictions and omissions for one witness (unordered; `kit` orders them).
pub fn detect(b: &BuiltWitness) -> Vec<Value> {
    let wid = &b.witness.witness_id;
    let mut time_counts: HashMap<&str, usize> = HashMap::new();
    for a in b.assertions.iter().filter(|a| !a.facets.times.is_empty()) {
        *time_counts.entry(a.statement_id.as_str()).or_default() += 1;
    }
    let single_time = |sid: &str| time_counts.get(sid).copied() == Some(1);
    let mut out = Vec::new();
    for l in &b.assertions {
        let li = b.seq(&l.statement_id);
        if li == 0 || li == usize::MAX {
            continue;
        }
        for d in DIMENSIONS {
            if !has(l, d) {
                continue;
            }
            let best = b
                .assertions
                .iter()
                .filter(|e| b.seq(&e.statement_id) < li && has(e, d) && topical(e, l, d, &single_time))
                .max_by_key(|e| (b.seq(&e.statement_id), overlap(e, l), std::cmp::Reverse(e.char_start)));
            if let Some(e) = best {
                if conflict(e, l, d) {
                    out.push(json!({
                        "finding_id": finding_id(wid, "CONTRADICTION", d, &e.assertion_id, &l.assertion_id),
                        "finding_kind": "CONTRADICTION",
                        "detection": "DETERMINISTIC",
                        "dimension": d,
                        "topic_id": b.topic_of[&l.assertion_id],
                        "earlier": side(e),
                        "later": side(l),
                        "proposition_ids": [],
                        "counsel_judgement_required": false,
                    }));
                }
            }
        }
    }
    // OMISSION: a material fact in testimony absent from every earlier prior statement.
    for l in &b.assertions {
        let li = b.seq(&l.statement_id);
        let Some(entry) = b.witness.statements.get(li) else { continue };
        if l.facets.material.is_empty() || !VOCAB.kind(&entry.statement_kind).map(|k| k.testimony).unwrap_or(false) {
            continue;
        }
        let prior: Vec<&crate::witness::RegisterEntry> = b.witness.statements[..li]
            .iter()
            .filter(|s| VOCAB.kind(&s.statement_kind).map(|k| k.prior_statement).unwrap_or(false))
            .collect();
        let Some(latest) = prior.last() else { continue };
        let prior_words: Vec<String> = prior
            .iter()
            .flat_map(|s| b.texts.get(&statement_id(&s.doc_id)).cloned().unwrap_or_default())
            .flat_map(|page| page.to_lowercase().split(|c: char| !c.is_alphabetic()).filter(|w| !w.is_empty()).map(str::to_string).collect::<Vec<_>>())
            .collect();
        if l.facets.material.iter().any(|stem| prior_words.iter().any(|w| w.starts_with(stem.as_str()))) {
            continue;
        }
        let lower = l.clean.to_lowercase();
        let terms: Vec<&str> = lower
            .split(|c: char| !c.is_alphabetic())
            .filter(|w| l.facets.material.iter().any(|s| w.starts_with(s.as_str())))
            .collect();
        let note = format!(
            "Not mentioned in {}: no reference to {}. Candidate for counsel judgement (omission / improvement).",
            prior.iter().map(|s| statement_label(b, &statement_id(&s.doc_id))).collect::<Vec<_>>().join(", "),
            terms.join(", ")
        );
        out.push(json!({
            "finding_id": finding_id(wid, "OMISSION", "MATERIAL_FACT", &statement_id(&latest.doc_id), &l.assertion_id),
            "finding_kind": "OMISSION",
            "detection": "DETERMINISTIC",
            "dimension": "MATERIAL_FACT",
            "topic_id": b.topic_of[&l.assertion_id],
            "earlier": {"statement_id": statement_id(&latest.doc_id), "quote": Value::Null, "locator": Value::Null, "absence_note": note},
            "later": side(l),
            "proposition_ids": [],
            "counsel_judgement_required": true,
        }));
    }
    out
}

/// Printed transcript line numbers covering a char range of a transcript page.
fn transcript_lines(page: &str, char_start: usize, char_end: usize) -> Option<(u32, u32)> {
    if !crate::witness::is_transcript_page(page) {
        return None;
    }
    let mut pos = 0usize;
    let (mut first, mut last) = (None, None);
    for raw in page.split_inclusive('\n') {
        let len = raw.chars().count();
        let (s, e) = (pos, pos + len);
        if s < char_end && char_start < e {
            let n: Option<u32> = raw.trim_start().split_whitespace().next().and_then(|t| t.parse().ok());
            if let Some(n) = n {
                first = first.or(Some(n));
                last = Some(n);
            }
        }
        pos = e;
    }
    first.zip(last)
}

/// AC-S4-03.3: verify model-proposed SEMANTIC candidates. Pairs `<id>-a` / `<id>-b` render only if both quotes are exact
/// substrings (B-S3-14 verifier) of two different statements of this witness, each within one page.
pub fn verify_model_candidates(b: &BuiltWitness, props: &[PropositionInput], existing: &[Value]) -> (Vec<Value>, Vec<(String, String)>) {
    let mut pairs: BTreeMap<String, (Option<&PropositionInput>, Option<&PropositionInput>)> = BTreeMap::new();
    for p in props {
        if let Some(base) = p.proposition_id.strip_suffix("-a") {
            pairs.entry(base.to_string()).or_default().0 = Some(p);
        } else if let Some(base) = p.proposition_id.strip_suffix("-b") {
            pairs.entry(base.to_string()).or_default().1 = Some(p);
        }
    }
    let sources: Vec<SupportSource> = b
        .texts
        .iter()
        .map(|(sid, pages)| SupportSource {
            support_id: sid.clone(),
            kind: "MATTER_SOURCE".into(),
            exact_text: pages.join("\u{c}"),
            source_id: None,
            chunk_id: None,
            authority_id: None,
            doc_id: Some(sid.trim_start_matches("st:").to_string()),
            page: None,
            paragraph: None,
            trust_level: None,
            temporal_status: None,
            superseded_status: None,
            permitted: false,
        })
        .collect();
    let mut rendered = Vec::new();
    let mut discarded = Vec::new();
    for (id, pair) in pairs {
        let (Some(pa), Some(pb)) = pair else {
            discarded.push((id, "incomplete pair".into()));
            continue;
        };
        let claim = |p: &PropositionInput| -> Option<(String, String)> {
            (p.class == "FACTUAL_BASIS" && p.support_ids.len() == 1 && p.support_claims.len() == 1 && p.support_claims[0].support_id == p.support_ids[0])
                .then(|| (p.support_ids[0].clone(), p.support_claims[0].exact_excerpt.clone()))
        };
        let (Some((sa, qa)), Some((sb, qb))) = (claim(pa), claim(pb)) else {
            discarded.push((id, "not a single exact factual quote per side".into()));
            continue;
        };
        if !b.texts.contains_key(&sa) || !b.texts.contains_key(&sb) {
            discarded.push((id, "quotes a statement that is not this witness's".into()));
            continue;
        }
        if sa == sb {
            discarded.push((id, "both quotes from the same statement".into()));
            continue;
        }
        let ledger = proposition::verify(VerifyRequest {
            output_kind: "REASONING".into(),
            propositions: vec![pa.clone(), pb.clone()],
            support_sources: sources.clone(),
            permitted_authority_ids: vec![],
        });
        let trusted = ledger.map(|l| l.propositions.iter().all(|p| p.render_action == "TRUSTED")).unwrap_or(false);
        if !trusted {
            discarded.push((id, "a quote is not an exact substring of the stored statement".into()));
            continue;
        }
        let locate = |sid: &str, quote: &str| -> Option<Value> {
            let pages = &b.texts[sid];
            pages.iter().enumerate().find_map(|(pi, page)| {
                page.find(quote).map(|byte| {
                    let cs = page[..byte].chars().count();
                    let ce = cs + quote.chars().count();
                    let lines = transcript_lines(page, cs, ce);
                    json!({"doc_id": sid.trim_start_matches("st:"), "page_index": pi, "char_start": cs, "char_end": ce,
                           "line_start": lines.map(|l| l.0), "line_end": lines.map(|l| l.1), "excerpt": quote})
                })
            })
        };
        let (Some(la), Some(lb)) = (locate(&sa, &qa), locate(&sb, &qb)) else {
            discarded.push((id, "a quote spans a page break".into()));
            continue;
        };
        let (mut e, mut l) = ((sa, qa, la, pa), (sb, qb, lb, pb));
        if b.seq(&e.0) > b.seq(&l.0) {
            std::mem::swap(&mut e, &mut l);
        }
        if existing.iter().any(|f| f["earlier"]["quote"] == json!(e.1) && f["later"]["quote"] == json!(l.1)) {
            discarded.push((id, "duplicates a finding already shown".into()));
            continue;
        }
        let page = l.2["page_index"].as_u64().unwrap_or(0) as usize;
        let (cs, ce) = (l.2["char_start"].as_u64().unwrap_or(0) as usize, l.2["char_end"].as_u64().unwrap_or(0) as usize);
        let topic = b
            .assertions
            .iter()
            .find(|a| a.statement_id == l.0 && a.page_index == page && a.char_start < ce && cs < a.char_end)
            .or_else(|| b.assertions.iter().find(|a| a.statement_id == l.0))
            .and_then(|a| b.topic_of.get(&a.assertion_id).cloned())
            .unwrap_or_else(|| "t1".into());
        rendered.push(json!({
            "finding_id": finding_id(&b.witness.witness_id, "CONTRADICTION", "SEMANTIC", &format!("{}:{}", e.0, e.1), &format!("{}:{}", l.0, l.1)),
            "finding_kind": "CONTRADICTION",
            "detection": "MODEL_VERIFIED",
            "dimension": "SEMANTIC",
            "topic_id": topic,
            "earlier": {"statement_id": e.0, "quote": e.1, "locator": e.2, "absence_note": Value::Null},
            "later": {"statement_id": l.0, "quote": l.1, "locator": l.2, "absence_note": Value::Null},
            "proposition_ids": [e.3.proposition_id, l.3.proposition_id],
            "counsel_judgement_required": true,
        }));
    }
    (rendered, discarded)
}

/// Request for the local model: the witness's statements as support sources, asking for quote-backed pairs only.
pub fn model_request(b: &BuiltWitness) -> Result<(String, Vec<SupportSource>), String> {
    let mut sources = Vec::new();
    let mut listing = Vec::new();
    for s in &b.witness.statements {
        let sid = statement_id(&s.doc_id);
        let Some(pages) = b.texts.get(&sid) else { continue };
        listing.push(format!("{sid} = {}", statement_label(b, &sid)));
        sources.push(SupportSource {
            support_id: sid,
            kind: "MATTER_SOURCE".into(),
            exact_text: pages.join("\n\n"),
            source_id: None,
            chunk_id: None,
            authority_id: None,
            doc_id: Some(s.doc_id.clone()),
            page: None,
            paragraph: None,
            trust_level: None,
            temporal_status: None,
            superseded_status: None,
            permitted: false,
        });
    }
    let chars: usize = sources.iter().map(|s| s.exact_text.chars().count()).sum();
    if sources.len() < 2 {
        return Err("CF_IMPEACH_NEEDS_TWO_STATEMENTS: this witness has fewer than two statements".into());
    }
    if chars > 30_000 || sources.len() > 40 {
        return Err(format!("CF_LLM_SUPPORT_CONTEXT_TOO_LONG: {chars} characters across {} statements; the local model takes at most 30000. Nothing was truncated.", sources.len()));
    }
    let user = format!(
        "Statements of one witness, earliest first: {}.\n\
         Find contradictions IN MEANING between an earlier and a later statement that a careful advocate could put to this witness \
         in cross-examination (for example: saw the accused vs. could not see in the dark; truck was red vs. truck was blue). \
         Ignore plain date, time, amount, count or place differences (already found by rules).\n\
         For each contradiction N output two FACTUAL_BASIS propositions: 'cN-a' quoting the EARLIER statement and 'cN-b' quoting the \
         LATER statement. Each has exactly one support_id and one support_claim whose exact_excerpt is copied character for character \
         from that statement. At most 6 contradictions. If there are none, return an empty propositions list. \
         Do not suggest what the witness should say.",
        listing.join("; ")
    );
    Ok((user, sources))
}

/// Orders findings (suggested confrontation sequence), applies counsel status, and wraps them per the kit contract.
pub fn kit(case_id: &str, matrix_id: &str, generated_at: &str, b: &BuiltWitness, mut findings: Vec<Value>, statuses: &HashMap<String, String>, model_fingerprint: Option<String>) -> Value {
    let rank = |f: &Value| CONFRONT_ORDER.iter().position(|d| f["dimension"] == *d).unwrap_or(CONFRONT_ORDER.len());
    let pos = |f: &Value| {
        let sid = f["later"]["statement_id"].as_str().unwrap_or("");
        (b.seq(sid), f["later"]["locator"]["page_index"].as_u64().unwrap_or(0), f["later"]["locator"]["char_start"].as_u64().unwrap_or(0))
    };
    findings.sort_by(|x, y| rank(x).cmp(&rank(y)).then(pos(x).cmp(&pos(y))));
    for (i, f) in findings.iter_mut().enumerate() {
        f["confrontation_order"] = json!(i + 1);
        let id = f["finding_id"].as_str().unwrap_or("").to_string();
        f["status"] = json!(statuses.get(&id).cloned().unwrap_or_else(|| "PROPOSED".into()));
    }
    let digest = crate::certificate::sha256_hex(format!("{matrix_id}|{}", b.witness.witness_id).as_bytes());
    json!({
        "schema_version": "1.0.0",
        "kit_id": format!("kit-{}", &digest[..16]),
        "case_id": case_id,
        "witness_id": b.witness.witness_id,
        "matrix_id": matrix_id,
        "generated_at": generated_at,
        "model_fingerprint": model_fingerprint,
        "findings": findings,
        "export_footer_id": EXPORT_FOOTER_ID,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::witness::tests::{built, FIXTURE};

    fn all_findings(fx: &Value) -> (Vec<BuiltWitness>, Vec<Value>) {
        let built = built(fx);
        let findings = built.iter().flat_map(|b| detect(b).into_iter().map(move |mut f| { f["witness_id"] = json!(b.witness.witness_id); f })).collect();
        (built, findings)
    }

    fn doc_of(sid: &Value) -> String {
        sid.as_str().unwrap().trim_start_matches("st:").to_string()
    }

    #[test]
    fn golden_deterministic_recall_and_precision() {
        // AC-S4-03.1: every planted contradiction found, nothing else reported.
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (_, findings) = all_findings(&fx);
        let expected = fx["expected"]["findings"].as_array().unwrap();
        let mut matched = vec![false; findings.len()];
        for e in expected {
            let hit = findings.iter().position(|f| {
                f["witness_id"] == e["witness_id"]
                    && f["finding_kind"] == e["finding_kind"]
                    && f["dimension"] == e["dimension"]
                    && doc_of(&f["earlier"]["statement_id"]) == e["earlier_doc"].as_str().unwrap()
                    && doc_of(&f["later"]["statement_id"]) == e["later_doc"].as_str().unwrap()
                    && e["earlier_quote_contains"].as_str().map(|q| f["earlier"]["quote"].as_str().unwrap_or("").contains(q)).unwrap_or(f["earlier"]["quote"].is_null())
                    && f["later"]["quote"].as_str().unwrap_or("").contains(e["later_quote_contains"].as_str().unwrap())
            });
            let i = hit.unwrap_or_else(|| panic!("missed planted finding {e}\nproduced: {}", serde_json::to_string_pretty(&findings).unwrap()));
            matched[i] = true;
            if let Some(lines) = e.get("later_lines") {
                let loc = &findings[i]["later"]["locator"];
                assert_eq!(loc["page_index"], e["later_page_index"], "{e}");
                assert_eq!(json!([loc["line_start"], loc["line_end"]]), *lines, "{e}");
            }
        }
        let extras: Vec<&Value> = findings.iter().zip(&matched).filter(|(_, m)| !**m).map(|(f, _)| f).collect();
        assert!(extras.is_empty(), "false positives: {}", serde_json::to_string_pretty(&extras).unwrap());
        let planted = fx["expected"]["planted_contradictions"].as_u64().unwrap() as usize;
        assert_eq!(findings.iter().filter(|f| f["finding_kind"] == "CONTRADICTION").count(), planted);
    }

    #[test]
    fn every_finding_quotes_both_sides_exactly() {
        // AC-S4-03.2: exact quote + valid locator on both sides (OMISSION: later side + absence note).
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (built, findings) = all_findings(&fx);
        for f in &findings {
            let b = built.iter().find(|b| b.witness.witness_id == f["witness_id"]).unwrap();
            for s in ["earlier", "later"] {
                let side = &f[s];
                if side["quote"].is_null() {
                    assert_eq!(f["finding_kind"], "OMISSION");
                    assert_eq!(s, "earlier");
                    assert!(side["absence_note"].as_str().map(|n| !n.is_empty()).unwrap_or(false));
                    continue;
                }
                let loc = &side["locator"];
                let page = &b.texts[side["statement_id"].as_str().unwrap()][loc["page_index"].as_u64().unwrap() as usize];
                let (cs, ce) = (loc["char_start"].as_u64().unwrap() as usize, loc["char_end"].as_u64().unwrap() as usize);
                let slice: String = page.chars().skip(cs).take(ce - cs).collect();
                assert_eq!(slice, side["quote"].as_str().unwrap(), "{f}");
                assert_eq!(loc["excerpt"], side["quote"]);
            }
        }
    }

    #[test]
    fn unverified_model_candidates_never_render() {
        // AC-S4-03.3
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (built, _) = all_findings(&fx);
        let mc = &fx["model_candidates"];
        let b = built.iter().find(|b| b.witness.witness_id == mc["witness_id"]).unwrap();
        let props: Vec<PropositionInput> = serde_json::from_value(mc["propositions"].clone()).unwrap();
        let existing = detect(b);
        let (rendered, discarded) = verify_model_candidates(b, &props, &existing);
        let ids: Vec<String> = rendered.iter().map(|f| f["proposition_ids"][0].as_str().unwrap().trim_end_matches("-a").to_string()).collect();
        let want: Vec<String> = mc["expected_rendered"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(ids, want, "discarded: {discarded:?}");
        let gone: Vec<&str> = discarded.iter().map(|d| d.0.as_str()).collect();
        for k in mc["expected_discarded"].as_object().unwrap().keys() {
            assert!(gone.contains(&k.as_str()), "{k} should be discarded: {discarded:?}");
        }
        for f in &rendered {
            assert_eq!(f["detection"], "MODEL_VERIFIED");
            assert_eq!(f["proposition_ids"].as_array().unwrap().len(), 2);
            for s in ["earlier", "later"] {
                let loc = &f[s]["locator"];
                let page = &b.texts[f[s]["statement_id"].as_str().unwrap()][loc["page_index"].as_u64().unwrap() as usize];
                let slice: String = page.chars().skip(loc["char_start"].as_u64().unwrap() as usize).take((loc["char_end"].as_u64().unwrap() - loc["char_start"].as_u64().unwrap()) as usize).collect();
                assert_eq!(slice, f[s]["quote"].as_str().unwrap());
            }
            assert_eq!(f["earlier"]["statement_id"], "st:doc-decl", "earlier side is the earlier statement");
        }
        // The deposition quote of c6 is on transcript page 13, line 2.
        let c6 = rendered.iter().find(|f| f["proposition_ids"][0] == "c6-a").unwrap();
        assert_eq!(json!([c6["later"]["locator"]["page_index"], c6["later"]["locator"]["line_start"], c6["later"]["locator"]["line_end"]]), json!([2, 2, 2]));
    }

    #[test]
    fn kit_matches_frozen_contract_shape() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let built = built(&fx);
        let matrix = crate::witness::matrix_json("case-imp", "graph-v", "2026-10-08T00:00:00Z", &built);
        let b = built.iter().find(|b| b.witness.witness_id == "w:ramesh-kumar").unwrap();
        let mut statuses = HashMap::new();
        let findings = detect(b);
        statuses.insert(findings[0]["finding_id"].as_str().unwrap().to_string(), "COUNSEL_ACCEPTED".to_string());
        let kit = kit("case-imp", matrix["matrix_id"].as_str().unwrap(), "2026-10-08T00:00:00Z", b, findings, &statuses, None);
        let contract: Value = serde_json::from_str(include_str!("../../contracts/impeachment_kit.schema.json")).unwrap();
        for k in contract["required"].as_array().unwrap() {
            assert!(kit.get(k.as_str().unwrap()).is_some(), "missing {k}");
        }
        let orders: Vec<u64> = kit["findings"].as_array().unwrap().iter().map(|f| f["confrontation_order"].as_u64().unwrap()).collect();
        assert_eq!(orders, (1..=orders.len() as u64).collect::<Vec<_>>());
        assert_eq!(kit["findings"][0]["dimension"], "PERSON", "identity first");
        assert!(kit["findings"].as_array().unwrap().iter().any(|f| f["status"] == "COUNSEL_ACCEPTED"));
        // Every finding's topic exists in the matrix for this witness.
        let w = matrix["witnesses"].as_array().unwrap().iter().find(|w| w["witness_id"] == "w:ramesh-kumar").unwrap();
        for f in kit["findings"].as_array().unwrap() {
            assert!(w["topics"].as_array().unwrap().iter().any(|t| t["topic_id"] == f["topic_id"]), "{f}");
        }
        if let Ok(dir) = std::env::var("CASEFORGE_EVIDENCE_DIR") {
            let us = built.iter().find(|b| b.witness.witness_id == "w:john-miller").unwrap();
            let props: Vec<PropositionInput> = serde_json::from_value(fx["model_candidates"]["propositions"].clone()).unwrap();
            let det = detect(us);
            let (sem, _) = verify_model_candidates(us, &props, &det);
            let us_kit = super::kit("case-imp", matrix["matrix_id"].as_str().unwrap(), "2026-10-08T00:00:00Z", us, det.into_iter().chain(sem).collect(), &HashMap::new(), Some("sha256:fixture".into()));
            std::fs::write(std::path::Path::new(&dir).join("impeachment_kits.json"), serde_json::to_string_pretty(&json!([kit, us_kit])).unwrap()).unwrap();
        }
    }
}
