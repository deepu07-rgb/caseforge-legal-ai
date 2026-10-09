//! Opposing Filing Scan (B-S4-06, PRD v4.0 §7.2).
//!
//! The opponent's pleading/brief/reply is UNTRUSTED ADVERSE ASSERTION — never matter fact. The scan:
//!   1. audits every citation and quotation in it (same engine as B-S4-04);
//!   2. finds record conflicts: a date, an amount or a denial that contradicts the matter record, with BOTH locators;
//!   3. lists factual assertions that cite no record support;
//!   4. lists arguments with no counterpart in counsel's Argument Tree.
//! Deterministic: no model, so text inside the filing (incl. instructions aimed at AI systems) cannot steer it.
//! Nothing here writes to the case graph (leak barrier: docpipe::exclude_external_pages).
//! Contract: contracts/external_filing.schema.json. Golden: golden/G-OPPOSING.

use crate::answer_feedback::{extract_amounts, extract_dates, keywords};
use crate::audit::{self, AuditReport};
use crate::retrieval::CorpusChunk;
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::{json, Value};
use std::collections::HashSet;

pub const PIPELINE_ID: &str = "caseforge.opposition-scan.v1";
const FILING_KINDS: [&str; 6] = ["PLEADING", "BRIEF", "REPLY", "WRITTEN_SUBMISSION", "AFFIDAVIT", "OTHER"];

static PARA_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*(\d{1,3})\.\s+").unwrap());
static ARGUMENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(it is (?:further |also )?submitted|submitted that|contended|urged that|barred by|is barred|not maintainable|estopped|is liable|are liable|is entitled|are entitled)\b").unwrap()
});
static AUTHORITY_PARA: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(reliance is placed|relied upon|the court held|held that|see also)\b").unwrap());
static FACT_VERB: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(executed|served|paid|payment|possession|signed|received|delivered|attested|registered|mutated|transferred|occupied|sold|purchased)\b").unwrap()
});
static DENIAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(never|not|no|denied|denies|deny|neither|nor)\b").unwrap());
static RECORD_SUPPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(annexure|exhibit|ex\.|document no|marked as|page \d|p\.\s?\d|paragraph \d|para \d)").unwrap()
});

pub(crate) struct Paragraph {
    pub(crate) id: String,
    pub(crate) page_index: usize,
    pub(crate) char_start: usize,
    pub(crate) char_end: usize,
    pub(crate) text: String,
    pub(crate) factual: bool,
    pub(crate) argument: bool,
    pub(crate) denies: bool,
}

fn char_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

pub(crate) fn paragraphs(pages: &[String]) -> Vec<Paragraph> {
    let mut out = Vec::new();
    for (page_index, page) in pages.iter().enumerate() {
        let starts: Vec<(usize, String)> = PARA_START.captures_iter(page).map(|c| (c.get(0).unwrap().start(), c[1].to_string())).collect();
        for (i, (start, number)) in starts.iter().enumerate() {
            let end = starts.get(i + 1).map(|n| n.0).unwrap_or(page.len());
            let raw = &page[*start..end];
            let lead = raw.len() - raw.trim_start().len();
            let text = raw.trim().to_string();
            let argument = ARGUMENT.is_match(&text);
            let authority = AUTHORITY_PARA.is_match(&text);
            let factual = !argument && !authority && (!extract_dates(&text).is_empty() || !extract_amounts(&text).is_empty() || FACT_VERB.is_match(&text));
            out.push(Paragraph {
                id: format!("p{number}"),
                page_index,
                char_start: char_index(page, start + lead),
                char_end: char_index(page, start + lead + text.len()),
                denies: factual && DENIAL.is_match(&text),
                factual,
                argument,
                text,
            });
        }
    }
    out
}

fn filing_locator(doc_id: &str, p: &Paragraph) -> Value {
    json!({"doc_id": doc_id, "page_index": p.page_index, "char_start": p.char_start, "char_end": p.char_end, "excerpt": p.text})
}

/// A matter-record entry (chronology event or evidence item) that a filing assertion can contradict.
struct RecordEntry {
    id: String,
    text: String,
    date: Option<String>,
    amounts: Vec<String>,
    origin: String,
    locator: Value,
}

fn record_entries(graph: &Value) -> Vec<RecordEntry> {
    let mut out = Vec::new();
    for (key, id_keys, text_keys) in [("chronology", ["event_id", "id"], ["summary", "label"]), ("evidence", ["evidence_id", "id"], ["label", "summary"])] {
        for e in graph.get(key).and_then(Value::as_array).into_iter().flatten() {
            let id = id_keys.iter().find_map(|k| e.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
            let text = text_keys.iter().find_map(|k| e.get(*k).and_then(Value::as_str)).unwrap_or("").to_string();
            let prov = e.get("provenance").cloned().unwrap_or(Value::Null);
            let origin = match prov.get("origin").and_then(Value::as_str) {
                Some(o @ ("SOURCED_BY_COUNSEL" | "EXTRACTED" | "USER_EDIT")) => o.to_string(),
                _ => "EXTRACTED".to_string(),
            };
            let src = prov.get("sources").and_then(|s| s.get(0)).cloned().unwrap_or(Value::Null);
            let locator = json!({
                "doc_id": src.get("doc_id").and_then(Value::as_str).unwrap_or("record"),
                "page_index": src.get("page").and_then(Value::as_i64),
                "char_start": src.get("char_start").and_then(Value::as_u64).unwrap_or(0),
                "char_end": src.get("char_end").and_then(Value::as_u64).unwrap_or(0),
                "excerpt": text,
            });
            out.push(RecordEntry {
                date: e.get("date").and_then(Value::as_str).map(str::to_string),
                amounts: extract_amounts(&text),
                id,
                text,
                origin,
                locator,
            });
        }
    }
    out
}

fn tree_limbs(tree: Option<&Value>) -> Vec<(String, HashSet<String>)> {
    tree.and_then(|t| t.get("limbs"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|l| {
            let mut text = String::new();
            for k in ["label", "theory"] {
                text.push_str(l.get(k).and_then(Value::as_str).unwrap_or(""));
                text.push(' ');
            }
            for a in l.get("anchors").and_then(Value::as_array).into_iter().flatten() {
                text.push_str(a.get("text").and_then(Value::as_str).unwrap_or(""));
                text.push(' ');
            }
            (l.get("node_id").and_then(Value::as_str).unwrap_or("").to_string(), keywords(&text))
        })
        .collect()
}

/// True when the case graph already cites `doc_id` as a source — i.e. the document was tagged EXTERNAL_FILING
/// after the graph was built, so its assertions may already sit among matter facts until the graph is rebuilt.
pub fn graph_cites_doc(graph: &Value, doc_id: &str) -> bool {
    match graph {
        Value::Object(map) => map.iter().any(|(k, v)| (k == "doc_id" && v.as_str() == Some(doc_id)) || graph_cites_doc(v, doc_id)),
        Value::Array(items) => items.iter().any(|v| graph_cites_doc(v, doc_id)),
        _ => false,
    }
}

pub struct ScanInput<'a> {
    pub doc_id: &'a str,
    pub filing_party_id: &'a str,
    pub filing_kind: &'a str,
    pub filename: &'a str,
    pub pages: &'a [String],
    pub graph: &'a Value,
    pub argument_tree: Option<&'a Value>,
    pub corpus: &'a [CorpusChunk],
    pub case_id: Option<String>,
}

/// The text of a filing as the audit reads it.
pub fn filing_text(pages: &[String]) -> String {
    pages.join("\n\n")
}

/// Returns (external_filing record per contracts/external_filing.schema.json, the audit report of its authorities).
/// The authorities are audited against `input.corpus`; `scan_with_report` takes an audit made another way (the on-disk
/// store, B-S4-29).
pub fn scan(input: ScanInput) -> (Value, AuditReport) {
    let report = audit::audit(&filing_text(input.pages), input.corpus, input.filename, "txt", input.case_id.clone());
    scan_with_report(input, report)
}

pub fn scan_with_report(input: ScanInput, mut report: AuditReport) -> (Value, AuditReport) {
    report.subject.subject_kind = "EXTERNAL_FILING".into();
    report.subject.doc_id = Some(input.doc_id.to_string());
    report.subject.filing_party_id = Some(input.filing_party_id.to_string());

    let paras = paragraphs(input.pages);
    let records = record_entries(input.graph);
    let limbs = tree_limbs(input.argument_tree);

    let assertions: Vec<Value> = paras
        .iter()
        .map(|p| {
            json!({
                "assertion_id": p.id,
                "polarity": if p.denies { "DENIES" } else { "ASSERTS" },
                "text": p.text,
                "locator": filing_locator(input.doc_id, p),
                "record_support_cited": RECORD_SUPPORT.is_match(&p.text),
                "graph_edge_id": Value::Null,
            })
        })
        .collect();

    let mut conflicts: Vec<Value> = Vec::new();
    for p in paras.iter().filter(|p| p.factual) {
        let words = keywords(&p.text);
        let overlap = |r: &RecordEntry| keywords(&r.text).intersection(&words).count();
        let mut push = |r: &RecordEntry, basis: &str| {
            conflicts.push(json!({
                "conflict_id": format!("c{}", conflicts.len() + 1),
                "assertion_id": p.id,
                "record_fact_node_id": r.id,
                "filing_locator": filing_locator(input.doc_id, p),
                "record_locator": r.locator,
                "record_origin": r.origin,
                "basis": basis,
                "proposition_ids": [],
            }));
        };
        // DATE: the filing dates an event the record dates differently (same event = >= 2 shared specific words).
        for (date, _, _) in extract_dates(&p.text) {
            if records.iter().any(|r| r.date.as_deref() == Some(date.as_str())) {
                continue;
            }
            if let Some(r) = records.iter().filter(|r| r.date.is_some() && overlap(r) >= 2).max_by_key(|r| overlap(r)) {
                push(r, "DATE");
            }
        }
        // AMOUNT: the filing states an amount the record states differently for the same subject.
        for amount in extract_amounts(&p.text) {
            if records.iter().any(|r| r.amounts.contains(&amount)) {
                continue;
            }
            if let Some(r) = records.iter().filter(|r| !r.amounts.is_empty() && overlap(r) >= 1).max_by_key(|r| overlap(r)) {
                push(r, "AMOUNT");
            }
        }
        // PRESENCE: the filing denies something the record records (strong overlap required).
        if p.denies {
            if let Some(r) = records.iter().filter(|r| overlap(r) >= 3).max_by_key(|r| overlap(r)) {
                push(r, "PRESENCE");
            }
        }
    }

    let unsupported: Vec<&str> = paras.iter().filter(|p| p.factual && !RECORD_SUPPORT.is_match(&p.text)).map(|p| p.id.as_str()).collect();

    let new_arguments: Vec<Value> = paras
        .iter()
        .filter(|p| p.argument)
        .map(|p| {
            let words = keywords(&p.text);
            let matched = limbs
                .iter()
                .map(|(id, kw)| (id, kw.intersection(&words).count()))
                .filter(|(_, n)| *n >= 2)
                .max_by_key(|(_, n)| *n)
                .map(|(id, _)| id.clone());
            let summary: String = p.text.chars().take(240).collect();
            json!({"argument_id": format!("arg-{}", p.id), "summary": summary, "locator": filing_locator(input.doc_id, p), "matched_tree_node_id": matched})
        })
        .collect();

    let kind = if FILING_KINDS.contains(&input.filing_kind) { input.filing_kind } else { "OTHER" };
    let record = json!({
        "schema_version": "1.0.0",
        "doc_id": input.doc_id,
        "type_tag": crate::docpipe::EXTERNAL_FILING_TAG,
        "filing_party_id": input.filing_party_id,
        "filing_kind": kind,
        "filed_on": Value::Null,
        "trust": "UNTRUSTED_ADVERSE",
        "assertions": assertions,
        "scan": {
            "audit_report_id": report.audit_id,
            "record_conflicts": conflicts,
            "unsupported_assertion_ids": unsupported,
            "new_arguments": new_arguments,
        }
    });
    (record, report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::LegalMetadata;

    const FIXTURE: &str = include_str!("../../golden/G-OPPOSING/fixtures/respondent_reply.json");

    fn corpus(fx: &Value) -> Vec<CorpusChunk> {
        fx["corpus"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
                CorpusChunk {
                    chunk_id: s("chunk_id"), pack_id: s("pack_id"), path: String::new(), jurisdiction: None, text: s("text"), authority_hint: s("title"),
                    metadata: LegalMetadata {
                        source_id: s("source_id"), title: s("title"), jurisdiction_path: vec![], authority_type: s("authority_type"),
                        issuing_authority: String::new(), citation: s("citation"), section: String::new(), effective_from: String::new(),
                        effective_to: String::new(), version: "fixture".into(), amendment: String::new(), page: v["page"].clone(),
                        paragraph: s("paragraph"), heading_path: vec![], checksum: String::new(), source_checksum: String::new(),
                        pack_checksum: String::new(), trust_level: s("trust_level"), superseded_status: s("superseded_status"),
                        publisher: String::new(), source_reference: String::new(), content_kind: "AUTHORITATIVE_TEXT".into(),
                        temporal_status: s("temporal_status"),
                    },
                }
            })
            .collect()
    }

    fn run(fx: &Value, pages: &[String]) -> (Value, AuditReport) {
        let corpus = corpus(fx);
        scan(ScanInput {
            doc_id: fx["filing"]["doc_id"].as_str().unwrap(),
            filing_party_id: fx["filing"]["filing_party_id"].as_str().unwrap(),
            filing_kind: fx["filing"]["filing_kind"].as_str().unwrap(),
            filename: "reply.txt",
            pages,
            graph: &fx["graph"],
            argument_tree: Some(&fx["argument_tree"]),
            corpus: &corpus,
            case_id: None,
        })
    }

    fn pages(fx: &Value) -> Vec<String> {
        fx["filing"]["pages"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect()
    }

    /// Findings only (what the system concludes), for the injection-invariance check.
    fn findings(record: &Value, report: &AuditReport) -> Value {
        let conflicts: Vec<Value> = record["scan"]["record_conflicts"].as_array().unwrap().iter()
            .map(|c| json!([c["assertion_id"], c["basis"], c["record_fact_node_id"]])).collect();
        let args: Vec<Value> = record["scan"]["new_arguments"].as_array().unwrap().iter()
            .map(|a| json!([a["argument_id"], a["matched_tree_node_id"]])).collect();
        let auth: Vec<Value> = report.items.iter().map(|i| json!([i.kind, i.draft_text, i.result])).collect();
        json!({"conflicts": conflicts, "unsupported": record["scan"]["unsupported_assertion_ids"], "arguments": args, "authorities": auth})
    }

    fn input<'a>(fx: &'a Value, pages: &'a [String], corpus: &'a [CorpusChunk]) -> ScanInput<'a> {
        ScanInput {
            doc_id: fx["filing"]["doc_id"].as_str().unwrap(),
            filing_party_id: fx["filing"]["filing_party_id"].as_str().unwrap(),
            filing_kind: fx["filing"]["filing_kind"].as_str().unwrap(),
            filename: "reply.txt",
            pages,
            graph: &fx["graph"],
            argument_tree: Some(&fx["argument_tree"]),
            corpus,
            case_id: None,
        }
    }

    /// B-S4-29: a scan whose authorities are audited against the on-disk store equals the whole-corpus scan.
    #[test]
    fn store_backed_scan_equals_whole_corpus_scan_on_the_real_pack() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let pack = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packs/official/caseforge.india.partition.highcourt.civil-appeal");
        let vault = tempfile::tempdir().unwrap();
        let paths = crate::vault::VaultPaths::new(vault.path().to_path_buf());
        crate::packs::install_pack_from_dir(&paths, &pack).expect("official pack installs");
        let whole = crate::retrieval::build_corpus(&paths, &[]).unwrap();
        let store = crate::retrieval::store::open(&paths).unwrap();
        let passage = whole.iter().map(|c| c.text.as_str()).find(|t| t.chars().count() > 200).unwrap();
        let quote: String = passage.chars().skip(60).take(90).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        let pages = vec![
            format!("1. The respondent relies on Section 2 of the Partition Act, 1893 and on \u{201c}{quote}\u{201d} without more.\n\n2. The appellant never paid rent in 2019. See also Section 99 of the Imaginary Act, 2099."),
        ];
        let strip = |mut v: Value| {
            v["generated_at"] = Value::Null;
            v
        };
        let (rec_a, rep_a) = scan(input(&fx, &pages, &whole));
        let report = audit::audit_with_store(&store, &[], &filing_text(&pages), "reply.txt", "txt", None).unwrap();
        let (rec_b, rep_b) = scan_with_report(input(&fx, &pages, &[]), report);
        assert_eq!(rec_a, rec_b);
        assert_eq!(strip(serde_json::to_value(&rep_a).unwrap()), strip(serde_json::to_value(&rep_b).unwrap()));
        assert_eq!(rep_b.subject.subject_kind, "EXTERNAL_FILING");
        assert!(!rep_b.items.is_empty());
    }

    #[test]
    fn golden_respondent_reply() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (record, report) = run(&fx, &pages(&fx));
        let exp = &fx["expected"];
        // AC-S4-02.1: every planted fabricated / misquoted authority detected
        for a in exp["authorities"].as_array().unwrap() {
            let m = a["match"].as_str().unwrap();
            let item = report.items.iter().find(|i| i.kind == a["kind"].as_str().unwrap() && (i.draft_text.contains(m) || m.contains(i.draft_text.as_str())))
                .unwrap_or_else(|| panic!("no {} item for '{m}': {:#?}", a["kind"], report.items));
            assert_eq!(item.result, a["result"].as_str().unwrap(), "'{m}' -> {item:#?}");
        }
        assert_eq!(report.subject.subject_kind, "EXTERNAL_FILING");
        // AC-S4-02.2: planted record conflicts, each with two locators
        let conflicts = record["scan"]["record_conflicts"].as_array().unwrap();
        for c in exp["conflicts"].as_array().unwrap() {
            let id = format!("p{}", c["paragraph"].as_str().unwrap());
            let hit = conflicts.iter().find(|x| x["assertion_id"] == json!(id) && x["basis"] == c["basis"])
                .unwrap_or_else(|| panic!("missing conflict {c} in {conflicts:#?}"));
            assert_eq!(hit["record_fact_node_id"], c["record_ref"]);
            assert_eq!(hit["record_origin"], c["record_origin"]);
            for side in ["filing_locator", "record_locator"] {
                assert!(hit[side]["doc_id"].as_str().map(|d| !d.is_empty()).unwrap_or(false), "{side} lacks doc_id: {hit}");
                assert!(hit[side]["excerpt"].as_str().map(|e| !e.is_empty()).unwrap_or(false), "{side} lacks excerpt: {hit}");
            }
        }
        assert_eq!(conflicts.len(), exp["planted_conflicts"].as_u64().unwrap() as usize, "no extra conflicts beyond the planted ones: {conflicts:#?}");
        // Unsupported vs supported assertions
        let unsupported: Vec<String> = record["scan"]["unsupported_assertion_ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        let want: Vec<String> = exp["unsupported_paragraphs"].as_array().unwrap().iter().map(|v| format!("p{}", v.as_str().unwrap())).collect();
        assert_eq!(unsupported, want);
        // New arguments vs the Argument Tree
        let args = record["scan"]["new_arguments"].as_array().unwrap();
        for a in exp["new_arguments"].as_array().unwrap() {
            let id = format!("arg-p{}", a["paragraph"].as_str().unwrap());
            let hit = args.iter().find(|x| x["argument_id"] == json!(id)).unwrap_or_else(|| panic!("missing argument {id}: {args:#?}"));
            assert_eq!(hit["matched_tree_node_id"], a["matched_tree_node_id"], "{id}");
        }
        // Contract invariants
        assert_eq!(record["trust"], json!("UNTRUSTED_ADVERSE"));
        assert!(record["assertions"].as_array().unwrap().iter().all(|a| a["graph_edge_id"].is_null()), "scan never writes graph edges");
    }

    #[test]
    fn instruction_inside_the_filing_changes_nothing() {
        // AC-S4-02.4: identical findings with and without the embedded instruction.
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let with = pages(&fx);
        let injection = fx["filing"]["injection_text"].as_str().unwrap();
        let without: Vec<String> = with.iter().map(|p| p.replace(injection, "")).collect();
        assert_ne!(with, without, "fixture must actually contain the instruction");
        let (r1, a1) = run(&fx, &with);
        let (r2, a2) = run(&fx, &without);
        assert_eq!(findings(&r1, &a1), findings(&r2, &a2));
        assert!(!a1.items.iter().any(|i| i.draft_text.contains("2099") && i.result == "VERIFIED"));
    }

    #[test]
    fn stale_graph_is_detected() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        assert!(graph_cites_doc(&fx["graph"], "doc-deed"));
        assert!(!graph_cites_doc(&fx["graph"], "doc-reply"), "the opponent's reply must not be a graph source");
    }

    #[test]
    fn record_matches_frozen_contract_shape() {
        let contract: Value = serde_json::from_str(include_str!("../../contracts/external_filing.schema.json")).unwrap();
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (record, _) = run(&fx, &pages(&fx));
        for k in contract["required"].as_array().unwrap() {
            assert!(record.get(k.as_str().unwrap()).is_some(), "missing {k}");
        }
        for k in contract["properties"]["scan"]["required"].as_array().unwrap() {
            assert!(record["scan"].get(k.as_str().unwrap()).is_some(), "scan missing {k}");
        }
        // The gate validates this against the full JSON schema (eval/checks/opposing_scan_b_s4_06.py).
        if let Ok(dir) = std::env::var("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("opposing_scan_record.json"), serde_json::to_string_pretty(&record).unwrap()).unwrap();
        }
    }
}
