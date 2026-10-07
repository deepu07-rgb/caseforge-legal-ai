//! B-S3-06 deterministic litigation graph v2 extractor.
//! Extraction, resolution, relationship construction and inference are separate phases.

use regex::Regex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const EXTRACTOR_ID: &str = "caseforge.graph-extractor.deterministic.v2";
pub const ENTITY_RESOLVER_ID: &str = "caseforge.entity-resolver.exact-alias.v2";
pub const RELATIONSHIP_BUILDER_ID: &str = "caseforge.relationship-builder.co-source.v2";
const PIPELINE_VERSION: &str = "B-S3-06.2.0.0";

pub struct PageInput {
    pub doc_id: String,
    pub page_index: i64,
    pub text: String,
    pub mean_confidence: f64,
}

pub(crate) fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Howard Hinnant's civil-from-days algorithm, UTC.
    let days = (seconds / 86_400) as i64;
    let sod = seconds % 86_400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        sod / 3600,
        (sod % 3600) / 60,
        sod % 60
    )
}

fn pointer(doc_id: &str, page_index: i64, start: usize, end: usize, quote: &str) -> Value {
    let mut h = Sha256::new();
    h.update(quote.as_bytes());
    json!({
        "doc_id": doc_id, "page_index": page_index, "kind": "pdf_span",
        "char_start": start as i64, "char_end": end as i64, "bbox": null,
        "quote": quote.chars().take(320).collect::<String>(),
        "content_hash": format!("sha256:{}", hex::encode(h.finalize()))
    })
}

fn provenance(
    source: Value,
    confidence: f64,
    origin: &str,
    recorded_at: &str,
    note: &str,
) -> Value {
    json!({
        "source": source.clone(), "sources": [source], "confidence": confidence.clamp(0.0, 1.0), "origin": origin,
        "pipeline_version": PIPELINE_VERSION, "recorded_at": recorded_at,
        "counsel_confirmed": false, "correction_lineage": [], "reviewed": false, "notes": note
    })
}

fn attr(
    date: Value,
    role: Value,
    side: Value,
    event_type: Value,
    jurisdiction: Value,
    citation: Value,
    doc_id: Value,
    statement_kind: Value,
    alternative_group: Value,
    aliases: Vec<String>,
) -> Value {
    json!({"aliases": aliases, "role": role, "side": side, "event_type": event_type, "date": date,
        "jurisdiction": jurisdiction, "citation": citation, "document_id": doc_id,
        "statement_kind": statement_kind, "alternative_group": alternative_group})
}

fn node(
    id: String,
    kind: &str,
    label: String,
    status: &str,
    attributes: Value,
    prov: Value,
) -> Value {
    json!({"node_id": id, "node_type": kind, "label": label, "status": status, "attributes": attributes, "provenance": prov})
}

fn source_of(value: &Value) -> Value {
    value["provenance"]["sources"][0].clone()
}

fn source_key(value: &Value) -> String {
    let s = &value["provenance"]["sources"][0];
    format!(
        "{}:{}",
        s["doc_id"].as_str().unwrap_or(""),
        s["page_index"].as_i64().unwrap_or(0)
    )
}

fn compact(s: &str, max: usize) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

fn month_num(name: &str) -> Option<u32> {
    [
        "january",
        "february",
        "march",
        "april",
        "may",
        "june",
        "july",
        "august",
        "september",
        "october",
        "november",
        "december",
    ]
    .iter()
    .position(|m| *m == name.to_ascii_lowercase())
    .map(|i| i as u32 + 1)
}

fn date_expr(
    raw: &str,
    kind: &str,
    precision: &str,
    status: &str,
    candidates: Vec<String>,
    resolved: Option<String>,
) -> Value {
    json!({"raw": raw, "kind": kind, "precision": precision, "status": status, "candidates": candidates,
        "resolved": resolved, "range_start": null, "range_end": null, "relative_to": null})
}

fn find_dates(text: &str) -> Vec<(Value, usize, usize)> {
    let mut out = Vec::new();
    let named = Regex::new(r"(?i)\b(\d{1,2})\s+(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{4})\b").unwrap();
    for c in named.captures_iter(text) {
        let m = c.get(0).unwrap();
        let d: u32 = c[1].parse().unwrap_or(0);
        let mo = month_num(&c[2]).unwrap_or(0);
        let y: u32 = c[3].parse().unwrap_or(0);
        if d > 0 && d <= 31 {
            let iso = format!("{y:04}-{mo:02}-{d:02}");
            out.push((
                date_expr(
                    m.as_str(),
                    "exact",
                    "day",
                    "resolved",
                    vec![iso.clone()],
                    Some(iso),
                ),
                m.start(),
                m.end(),
            ));
        }
    }
    let us_named = Regex::new(r"(?i)\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{1,2}),?\s+(\d{4})\b").unwrap();
    for c in us_named.captures_iter(text) {
        let m = c.get(0).unwrap();
        let mo = month_num(&c[1]).unwrap_or(0);
        let d: u32 = c[2].parse().unwrap_or(0);
        let y: u32 = c[3].parse().unwrap_or(0);
        if d > 0 && d <= 31 {
            let iso = format!("{y:04}-{mo:02}-{d:02}");
            out.push((
                date_expr(
                    m.as_str(),
                    "exact",
                    "day",
                    "resolved",
                    vec![iso.clone()],
                    Some(iso),
                ),
                m.start(),
                m.end(),
            ));
        }
    }
    let numeric = Regex::new(r"\b(\d{1,2})[./-](\d{1,2})[./-](\d{4})\b").unwrap();
    for c in numeric.captures_iter(text) {
        let m = c.get(0).unwrap();
        let a: u32 = c[1].parse().unwrap_or(0);
        let b: u32 = c[2].parse().unwrap_or(0);
        let y: u32 = c[3].parse().unwrap_or(0);
        if a == 0 || b == 0 || a > 31 || b > 31 {
            continue;
        }
        if a <= 12 && b <= 12 && a != b {
            let candidates = vec![
                format!("{y:04}-{b:02}-{a:02}"),
                format!("{y:04}-{a:02}-{b:02}"),
            ];
            out.push((
                date_expr(m.as_str(), "exact", "day", "ambiguous", candidates, None),
                m.start(),
                m.end(),
            ));
        } else {
            let (d, mo) = if b > 12 { (b, a) } else { (a, b) };
            if d <= 31 && mo <= 12 {
                let iso = format!("{y:04}-{mo:02}-{d:02}");
                out.push((
                    date_expr(
                        m.as_str(),
                        "exact",
                        "day",
                        "resolved",
                        vec![iso.clone()],
                        Some(iso),
                    ),
                    m.start(),
                    m.end(),
                ));
            }
        }
    }
    let relative = Regex::new(r"(?i)\b(?:the\s+)?(?:same|next|following|previous)\s+(?:day|week|month|year)\b|\b\d+\s+(?:days?|weeks?|months?|years?)\s+(?:later|earlier|after|before)\b").unwrap();
    for m in relative.find_iter(text) {
        out.push((
            date_expr(
                m.as_str(),
                "relative",
                "relative",
                "unresolved",
                vec![],
                None,
            ),
            m.start(),
            m.end(),
        ));
    }
    out.sort_by_key(|(_, s, _)| *s);
    out
}

fn sentence_context(text: &str, start: usize, end: usize) -> String {
    let lo = text[..start].rfind(['\n', '.']).map(|i| i + 1).unwrap_or(0);
    let hi = text[end..]
        .find(['\n', '.'])
        .map(|i| end + i + 1)
        .unwrap_or_else(|| (end + 180).min(text.len()));
    compact(&text[lo..hi], 420)
}

fn status_for_fact(s: &str) -> &'static str {
    let x = s.to_ascii_lowercase();
    if x.contains("admitted") || x.contains("undisputed") {
        "admitted"
    } else if x.contains("disputed") || x.contains("denies") || x.contains("contends") {
        "disputed"
    } else if x.contains("material fact") {
        "material"
    } else {
        "pleaded"
    }
}

fn polarity_key(s: &str) -> (String, bool) {
    let lower = s.to_ascii_lowercase();
    let neg = Regex::new(r"\b(?:not|never|no)\b")
        .unwrap()
        .is_match(&lower);
    let cleaned = Regex::new(
        r"(?i)^(?:admitted|disputed|material)?\s*fact\s*:\s*|\b(?:not|never|no)\b|[^a-z0-9 ]",
    )
    .unwrap()
    .replace_all(&lower, " ");
    (compact(&cleaned, 300), neg)
}

fn push_gap(gaps: &mut Vec<Value>, concept: &str, prompt: &str) {
    gaps.push(json!({"gap_id": format!("gap_{concept}"), "concept": concept, "topic": concept,
        "reason": "not_found", "question_prompt": prompt, "status": "open", "answered_fact_id": null}));
}

pub fn extract_case_graph(
    case_id: &str,
    jurisdiction: &str,
    pages: &[PageInput],
    classifier_id: &str,
    recognizer_id: &str,
) -> Value {
    let at = now_rfc3339();
    let jur = if matches!(jurisdiction, "India" | "United States" | "Other") {
        jurisdiction
    } else {
        "Other"
    };
    let mut nodes = Vec::<Value>::new();
    let mut parties = Vec::<Value>::new();
    let mut chronology = Vec::<Value>::new();
    let mut statutes = Vec::<Value>::new();
    let mut evidence = Vec::<Value>::new();
    let mut relief = Vec::<Value>::new();
    let mut procedural: Option<Value> = None;
    let mut seen = HashSet::<String>::new();

    let patterns: Vec<(&str, &str, Regex)> = vec![
        (
            "claim",
            "pleaded",
            Regex::new(r"(?im)^\s*claim\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "cause_or_ground",
            "pleaded",
            Regex::new(r"(?im)^\s*(?:cause of action|ground)\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "defence",
            "pleaded",
            Regex::new(r"(?im)^\s*(?:defen[cs]e|affirmative defen[cs]e)\s*:\s*(.{4,300})$")
                .unwrap(),
        ),
        (
            "counterclaim",
            "pleaded",
            Regex::new(r"(?im)^\s*counterclaim\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "issue",
            "unknown",
            Regex::new(r"(?im)^\s*(?:issue|question presented)\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "sub_issue",
            "unknown",
            Regex::new(r"(?im)^\s*sub-?issue\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "fact",
            "pleaded",
            Regex::new(r"(?im)^\s*(?:(?:admitted|disputed|material)\s+)?fact\s*:\s*(.{4,400})$")
                .unwrap(),
        ),
        (
            "argument",
            "pleaded",
            Regex::new(r"(?im)^\s*argument\s*:\s*(.{4,400})$").unwrap(),
        ),
        (
            "interpretation",
            "inferred",
            Regex::new(r"(?im)^\s*alternative interpretation\s*:\s*(.{4,400})$").unwrap(),
        ),
        (
            "witness",
            "unknown",
            Regex::new(r"(?im)^\s*witness\s*:\s*(.{3,180})$").unwrap(),
        ),
        (
            "statement",
            "pleaded",
            Regex::new(r"(?im)^\s*(?:statement|testimony)\s*:\s*(.{4,400})$").unwrap(),
        ),
        (
            "application",
            "pleaded",
            Regex::new(r"(?im)^\s*application\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "hearing",
            "unknown",
            Regex::new(r"(?im)^\s*hearing\s*:\s*(.{4,300})$").unwrap(),
        ),
        (
            "order",
            "unknown",
            Regex::new(r"(?im)^\s*order\s*:\s*(.{4,300})$").unwrap(),
        ),
    ];
    let party_re = Regex::new(r"(?im)^\s*(plaintiff|petitioner|applicant|defendant|respondent|appellant|appellee|accused|prosecution)\s*[:\-–]\s*([^\n;,]{2,120})").unwrap();
    let alias_re =
        Regex::new(r"(?i)\b(?:also known as|a\.?k\.?a\.?)\s+([A-Z][A-Za-z0-9 .'-]{2,80})").unwrap();
    let law_re = Regex::new(r"(?i)\b(?:section|rule|article|§)\s*\d+[A-Za-z]?(?:\([0-9A-Za-z]+\))*[^\n.;]{0,90}|\b\d+\s+(?:U\.S\.|F\.\d+d|SCC|AIR)\s+\d+\b|\b(?:IPC|CrPC|CPC|U\.S\.C\.)\b").unwrap();
    let relief_re = Regex::new(r"(?im)^\s*(?:prayer|relief|wherefore)\s*:\s*(.{5,400})$").unwrap();
    let evidence_re =
        Regex::new(r"(?im)^\s*(?:(exhibit|annexure|document|evidence)\s*[:\-]\s*)(.{3,300})$")
            .unwrap();
    let procedure_re =
        Regex::new(r"(?im)^\s*(?:procedural history|procedure|posture)\s*:\s*(.{4,400})$").unwrap();

    for page in pages {
        if page.text.trim().is_empty() {
            continue;
        }
        let text = &page.text;
        let base = page.mean_confidence.clamp(0.0, 1.0);
        for caps in party_re.captures_iter(text) {
            let m = caps.get(0).unwrap();
            let role = caps[1].to_ascii_lowercase();
            let name = compact(caps[2].trim(), 140);
            let key = format!("party:{}", name.to_ascii_lowercase());
            if !seen.insert(key) {
                continue;
            }
            let aliases = alias_re
                .captures(&name)
                .map(|c| vec![compact(&c[1], 100)])
                .unwrap_or_default();
            let side = if matches!(
                role.as_str(),
                "plaintiff" | "petitioner" | "applicant" | "appellant" | "prosecution"
            ) {
                "initiating"
            } else {
                "responding"
            };
            let p = provenance(
                pointer(
                    &page.doc_id,
                    page.page_index,
                    m.start(),
                    m.end(),
                    m.as_str(),
                ),
                base.min(0.94),
                "EXTRACTED",
                &at,
                "party caption pattern",
            );
            let id = format!("party_{:04}", parties.len() + 1);
            nodes.push(node(
                id.clone(),
                "party",
                name.clone(),
                "pleaded",
                attr(
                    Value::Null,
                    json!(role),
                    json!(side),
                    Value::Null,
                    json!(jur),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    aliases.clone(),
                ),
                p.clone(),
            ));
            parties.push(json!({"party_id":id,"display_name":name,"role":role,"side":side,"aliases":aliases,"provenance":p}));
        }
        for (kind, default_status, re) in &patterns {
            for caps in re.captures_iter(text) {
                let m = caps.get(0).unwrap();
                let label = compact(caps.get(1).unwrap().as_str(), 420);
                let key = format!("{kind}:{}", label.to_ascii_lowercase());
                if !seen.insert(key) {
                    continue;
                }
                let status = if *kind == "fact" {
                    status_for_fact(m.as_str())
                } else {
                    default_status
                };
                let p = provenance(
                    pointer(
                        &page.doc_id,
                        page.page_index,
                        m.start(),
                        m.end(),
                        m.as_str(),
                    ),
                    base.min(0.88),
                    "EXTRACTED",
                    &at,
                    &format!("{kind} labelled extraction"),
                );
                let id = format!(
                    "{}_{:04}",
                    kind,
                    nodes.iter().filter(|n| n["node_type"] == *kind).count() + 1
                );
                nodes.push(node(
                    id,
                    *kind,
                    label,
                    status,
                    attr(
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        if *kind == "statement" {
                            json!("document_statement")
                        } else {
                            Value::Null
                        },
                        Value::Null,
                        vec![],
                    ),
                    p,
                ));
            }
        }
        for (dx, start, end) in find_dates(text) {
            let summary = sentence_context(text, start, end);
            let p = provenance(
                pointer(&page.doc_id, page.page_index, start, end, &text[start..end]),
                base.min(0.9),
                "EXTRACTED",
                &at,
                "date expression plus source sentence",
            );
            let id = format!("event_{:04}", chronology.len() + 1);
            let resolved = dx["resolved"].as_str().unwrap_or("").to_string();
            let raw = dx["raw"].as_str().unwrap_or("").to_string();
            let event_type = if summary.to_ascii_lowercase().contains("hearing") {
                "hearing"
            } else if summary.to_ascii_lowercase().contains("order") {
                "order"
            } else if summary.to_ascii_lowercase().contains("filed") {
                "filing"
            } else {
                "event"
            };
            nodes.push(node(
                id.clone(),
                "event",
                summary.clone(),
                "unknown",
                attr(
                    dx.clone(),
                    Value::Null,
                    Value::Null,
                    json!(event_type),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    vec![],
                ),
                p.clone(),
            ));
            chronology.push(json!({"event_id":id,"date":if resolved.is_empty(){raw.clone()}else{resolved.clone()},"date_precision":dx["precision"],"date_expression":dx,"summary":summary,"sort_key":if resolved.is_empty(){format!("~{raw}")}else{resolved},"event_type":event_type,"provenance":p}));
        }
        for m in law_re.find_iter(text) {
            let label = compact(m.as_str(), 220);
            let key = format!("law:{}", label.to_ascii_lowercase());
            if !seen.insert(key) {
                continue;
            }
            let lower = label.to_ascii_lowercase();
            let law_type = if lower.starts_with("rule") {
                "rule"
            } else if Regex::new(r"\d+\s+(?:u\.s\.|f\.|scc|air)")
                .unwrap()
                .is_match(&lower)
            {
                "authority"
            } else if lower.starts_with("section")
                || lower.starts_with("article")
                || lower.starts_with('§')
            {
                "provision"
            } else {
                "statute"
            };
            let p = provenance(
                pointer(
                    &page.doc_id,
                    page.page_index,
                    m.start(),
                    m.end(),
                    m.as_str(),
                ),
                base.min(0.9),
                "EXTRACTED",
                &at,
                "legal reference pattern",
            );
            let id = format!("law_{:04}", statutes.len() + 1);
            nodes.push(node(
                id.clone(),
                law_type,
                label.clone(),
                "pleaded",
                attr(
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    json!(jur),
                    json!(label),
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    vec![],
                ),
                p.clone(),
            ));
            statutes.push(json!({"statute_id":id,"label":label,"law_type":law_type,"jurisdiction":jur,"citation_raw":label,"provenance":p}));
        }
        for caps in relief_re.captures_iter(text) {
            let m = caps.get(0).unwrap();
            let label = compact(&caps[1], 420);
            let p = provenance(
                pointer(
                    &page.doc_id,
                    page.page_index,
                    m.start(),
                    m.end(),
                    m.as_str(),
                ),
                base.min(0.86),
                "EXTRACTED",
                &at,
                "relief labelled extraction",
            );
            let id = format!("relief_{:04}", relief.len() + 1);
            nodes.push(node(
                id.clone(),
                "relief",
                label.clone(),
                "pleaded",
                attr(
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    vec![],
                ),
                p.clone(),
            ));
            relief.push(json!({"relief_id":id,"text":label,"quantum":null,"provenance":p}));
        }
        for caps in evidence_re.captures_iter(text) {
            let m = caps.get(0).unwrap();
            let kind = caps[1].to_ascii_lowercase();
            let label = compact(&caps[2], 320);
            let node_kind = if matches!(kind.as_str(), "exhibit" | "annexure") {
                "exhibit"
            } else if kind == "document" {
                "document"
            } else {
                "evidence"
            };
            let p = provenance(
                pointer(
                    &page.doc_id,
                    page.page_index,
                    m.start(),
                    m.end(),
                    m.as_str(),
                ),
                base.min(0.88),
                "EXTRACTED",
                &at,
                "evidence/document label",
            );
            let id = format!("evidence_{:04}", evidence.len() + 1);
            nodes.push(node(
                id.clone(),
                node_kind,
                label.clone(),
                "unknown",
                attr(
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    json!(page.doc_id),
                    Value::Null,
                    Value::Null,
                    vec![],
                ),
                p.clone(),
            ));
            evidence.push(json!({"evidence_id":id,"label":label,"doc_id":page.doc_id,"kind":node_kind,"provenance":p}));
        }
        if procedural.is_none() {
            if let Some(c) = procedure_re.captures(text) {
                let m = c.get(0).unwrap();
                let label = compact(&c[1], 420);
                let p = provenance(
                    pointer(
                        &page.doc_id,
                        page.page_index,
                        m.start(),
                        m.end(),
                        m.as_str(),
                    ),
                    base.min(0.84),
                    "EXTRACTED",
                    &at,
                    "procedural label",
                );
                let id = "procedure_0001".to_string();
                nodes.push(node(
                    id,
                    "procedure",
                    label.clone(),
                    "unknown",
                    attr(
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        Value::Null,
                        vec![],
                    ),
                    p.clone(),
                ));
                procedural =
                    Some(json!({"court":null,"stage":null,"summary":label,"provenance":p}));
            }
        }
    }

    // Entity resolution phase: exact aliases only; never merge merely similar legal names.
    let mut alias_pairs: Vec<(String, String)> = Vec::new();
    for party in &parties {
        if let Some(aliases) = party["aliases"].as_array() {
            for alias in aliases {
                alias_pairs.push((
                    party["party_id"].as_str().unwrap_or("").to_string(),
                    alias.as_str().unwrap_or("").to_string(),
                ));
            }
        }
    }
    for (party_id, alias) in alias_pairs {
        if alias.is_empty() {
            continue;
        }
        if let Some(p) = nodes.iter().find(|n| n["node_id"] == party_id) {
            let prov = p["provenance"].clone();
            let aid = format!(
                "alias_{:04}",
                nodes
                    .iter()
                    .filter(|n| n["node_type"] == "interpretation")
                    .count()
                    + 1
            );
            nodes.push(node(
                aid.clone(),
                "interpretation",
                alias,
                "inferred",
                attr(
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    Value::Null,
                    json!(party_id),
                    vec![],
                ),
                provenance(
                    source_of(p),
                    prov["confidence"].as_f64().unwrap_or(0.5),
                    "INFERRED",
                    &at,
                    "alias candidate; not a document fact",
                ),
            ));
        }
    }

    // Relationship phase: construct only traceable co-source links.
    let mut edges = Vec::<Value>::new();
    let link_specs = [
        ("fact", "document", "fact_to_document"),
        ("fact", "evidence", "fact_to_evidence"),
        ("fact", "exhibit", "fact_to_evidence"),
        ("issue", "fact", "issue_to_fact"),
        ("issue", "statute", "issue_to_law"),
        ("issue", "rule", "issue_to_law"),
        ("issue", "authority", "issue_to_law"),
        ("issue", "provision", "issue_to_law"),
        ("argument", "fact", "argument_to_support"),
        ("argument", "evidence", "argument_to_support"),
        ("witness", "statement", "witness_to_statement"),
        ("sub_issue", "issue", "sub_issue_of"),
        ("procedure", "order", "procedure_to_order"),
    ];
    for (from_kind, to_kind, edge_kind) in link_specs {
        let froms: Vec<&Value> = nodes
            .iter()
            .filter(|n| n["node_type"] == from_kind)
            .collect();
        let tos: Vec<&Value> = nodes.iter().filter(|n| n["node_type"] == to_kind).collect();
        for f in &froms {
            if let Some(t) = tos.iter().find(|t| source_key(f) == source_key(t)) {
                let srcs = json!([source_of(f), source_of(t)]);
                let confidence = f["provenance"]["confidence"]
                    .as_f64()
                    .unwrap_or(0.5)
                    .min(t["provenance"]["confidence"].as_f64().unwrap_or(0.5))
                    * 0.9;
                let mut ep = provenance(
                    srcs[0].clone(),
                    confidence,
                    "EXTRACTED",
                    &at,
                    "co-source relationship candidate",
                );
                ep["sources"] = srcs;
                edges.push(json!({"edge_id":format!("edge_{:04}",edges.len()+1),"edge_type":edge_kind,"from_node_id":f["node_id"],"to_node_id":t["node_id"],"status":"candidate","provenance":ep}));
            }
        }
    }

    // Contradiction candidates require explicit opposing polarity, never date equality.
    let facts: Vec<&Value> = nodes.iter().filter(|n| n["node_type"] == "fact").collect();
    let mut contradictions = Vec::<Value>::new();
    for i in 0..facts.len() {
        for j in i + 1..facts.len() {
            let (ka, na) = polarity_key(facts[i]["label"].as_str().unwrap_or(""));
            let (kb, nb) = polarity_key(facts[j]["label"].as_str().unwrap_or(""));
            if !ka.is_empty() && ka == kb && na != nb {
                let ls = source_of(facts[i]);
                let rs = source_of(facts[j]);
                let mut cp = provenance(
                    ls.clone(),
                    0.78,
                    "INFERRED",
                    &at,
                    "explicit opposing polarity; counsel review required",
                );
                cp["sources"] = json!([ls.clone(), rs.clone()]);
                let cid = format!("contradiction_{:04}", contradictions.len() + 1);
                contradictions.push(json!({"contradiction_id":cid,"summary":"Explicit affirmative and negative statements concern the same normalized proposition","left_ref":facts[i]["node_id"],"right_ref":facts[j]["node_id"],"left_source":ls,"right_source":rs,"basis":"explicit_negation","status":"candidate","severity":"medium","confidence":0.78,"provenance":cp}));
            }
        }
    }

    chronology.sort_by(|a, b| {
        a["sort_key"]
            .as_str()
            .unwrap_or("")
            .cmp(b["sort_key"].as_str().unwrap_or(""))
    });
    let concepts = [
        (
            "parties",
            "Who are the parties, aliases, roles and sides?",
            nodes.iter().any(|n| n["node_type"] == "party"),
        ),
        (
            "claims",
            "What claims, grounds, defences or counterclaims are pleaded?",
            nodes.iter().any(|n| {
                matches!(
                    n["node_type"].as_str(),
                    Some("claim" | "cause_or_ground" | "defence" | "counterclaim")
                )
            }),
        ),
        (
            "issues",
            "What issues and sub-issues require determination?",
            nodes
                .iter()
                .any(|n| matches!(n["node_type"].as_str(), Some("issue" | "sub_issue"))),
        ),
        (
            "relief",
            "What relief or prayers are sought?",
            !relief.is_empty(),
        ),
        (
            "facts",
            "Which facts are material, admitted or disputed?",
            nodes.iter().any(|n| n["node_type"] == "fact"),
        ),
        (
            "evidence",
            "What evidence, exhibits, documents, witnesses and statements support the matter?",
            !evidence.is_empty(),
        ),
        (
            "chronology",
            "What are the key dates, ranges, relative dates and disputed events?",
            !chronology.is_empty(),
        ),
        (
            "procedure",
            "What applications, hearings and orders form the procedural history?",
            nodes.iter().any(|n| {
                matches!(
                    n["node_type"].as_str(),
                    Some("procedure" | "application" | "hearing" | "order")
                )
            }),
        ),
        (
            "law",
            "What statutes, rules, authorities and pleaded provisions apply?",
            !statutes.is_empty(),
        ),
        (
            "support_links",
            "Which facts, evidence, issues, law and arguments are linked?",
            !edges.is_empty(),
        ),
    ];
    let mut gaps = Vec::new();
    for (concept, prompt, found) in concepts {
        if !found {
            push_gap(&mut gaps, concept, prompt);
        }
    }
    for e in &chronology {
        if e["date_expression"]["status"] == "ambiguous"
            || e["date_expression"]["status"] == "unresolved"
        {
            gaps.push(json!({"gap_id":format!("gap_date_{}",e["event_id"].as_str().unwrap_or("unknown")),"concept":"chronology_date","topic":"chronology","reason":"ambiguous","question_prompt":format!("Resolve date expression '{}' from its source.",e["date_expression"]["raw"].as_str().unwrap_or("")),"status":"open","answered_fact_id":null}));
        }
    }
    let filled = [
        !parties.is_empty(),
        !chronology.is_empty(),
        !statutes.is_empty(),
        !evidence.is_empty(),
        !relief.is_empty(),
    ]
    .iter()
    .filter(|x| **x)
    .count();
    let score = (filled * 20) as i64;
    json!({"schema_version":"2.0.0","case_id":case_id,"jurisdiction":jur,"generated_at":at,
        "migration":{"strategy":"normalized_v2_with_v1_read_projections","legacy_projections":true,"migrated_from":null},
        "pipeline":{"extractor_id":EXTRACTOR_ID,"recognizer_id":recognizer_id,"classifier_id":classifier_id,"entity_resolver_id":ENTITY_RESOLVER_ID,"relationship_builder_id":RELATIONSHIP_BUILDER_ID,"inference_policy":"inferences_are_never_document_facts","notes":"four separated deterministic phases; normalized nodes/edges authoritative"},
        "completeness":{"score_id":"caseforge.prep_completeness.v1","score_0_100":score,"open_gaps":gaps.len(),"formula":"20 points for each non-empty legacy projection among parties, chronology, statutes, evidence, prayer_relief (max 100)."},
        "nodes":nodes,"edges":edges,"parties":parties,"chronology":chronology,"statutes":statutes,"evidence":evidence,"procedural_posture":procedural,"prayer_relief":relief,"contradictions":contradictions,"gaps":gaps,"counsel_facts":[]})
}

pub fn assert_extracted_sources(graph: &Value) -> Vec<String> {
    let mut errors = Vec::new();
    for collection in ["nodes", "edges"] {
        for (i, item) in graph[collection]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .enumerate()
        {
            let p = &item["provenance"];
            if p["sources"]
                .as_array()
                .map(|a| a.is_empty())
                .unwrap_or(true)
            {
                errors.push(format!("{collection}[{i}]: missing sources"));
            }
            for field in [
                "confidence",
                "origin",
                "pipeline_version",
                "recorded_at",
                "counsel_confirmed",
                "correction_lineage",
            ] {
                if p[field].is_null() {
                    errors.push(format!("{collection}[{i}]: missing {field}"));
                }
            }
            if p["origin"] == "EXTRACTED"
                && p["sources"][0]["doc_id"].as_str().unwrap_or("").is_empty()
            {
                errors.push(format!("{collection}[{i}]: extracted item has no document"));
            }
        }
    }
    for (i, c) in graph["contradictions"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .enumerate()
    {
        if c["left_source"].is_null() || c["right_source"].is_null() {
            errors.push(format!("contradictions[{i}]: both source sides required"));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    fn page(text: &str) -> PageInput {
        PageInput {
            doc_id: "doc-a".into(),
            page_index: 0,
            text: text.into(),
            mean_confidence: 0.93,
        }
    }
    #[test]
    fn ambiguous_numeric_date_stays_ambiguous() {
        let g = extract_case_graph(
            "c",
            "India",
            &[page(
                "Fact: filing occurred.\nOn 03/04/2025 the claim was filed.",
            )],
            "triage",
            "recognize",
        );
        assert_eq!(g["chronology"][0]["date_expression"]["status"], "ambiguous");
        assert!(g["chronology"][0]["date_expression"]["resolved"].is_null());
    }
    #[test]
    fn same_date_is_not_a_contradiction() {
        let g = extract_case_graph(
            "c",
            "India",
            &[page(
                "On 3 March 2025 filing occurred. On 3 March 2025 hearing occurred.",
            )],
            "triage",
            "recognize",
        );
        assert_eq!(g["contradictions"].as_array().unwrap().len(), 0);
    }
    #[test]
    fn explicit_negation_has_both_sources() {
        let g = extract_case_graph(
            "c",
            "United States",
            &[page(
                "Fact: payment was made\nDisputed fact: payment was not made",
            )],
            "triage",
            "recognize",
        );
        let c = &g["contradictions"][0];
        assert!(c["left_source"].is_object() && c["right_source"].is_object());
    }
    #[test]
    fn all_nodes_edges_are_traceable() {
        let g=extract_case_graph("c","India",&[page("Plaintiff: Asha Rao\nIssue: whether payment was made\nFact: payment was made\nEvidence: bank statement\nRule 10")],"triage","recognize");
        assert!(assert_extracted_sources(&g).is_empty());
    }

    fn run_controlled_fixture(raw: &str) -> Value {
        let fixture: Value = serde_json::from_str(raw).unwrap();
        let pages: Vec<PageInput> = fixture["pages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| PageInput {
                doc_id: p["doc_id"].as_str().unwrap().into(),
                page_index: p["page_index"].as_i64().unwrap(),
                text: p["text"].as_str().unwrap().into(),
                mean_confidence: p["mean_confidence"].as_f64().unwrap(),
            })
            .collect();
        let graph = extract_case_graph(
            fixture["fixture_id"].as_str().unwrap(),
            fixture["jurisdiction"].as_str().unwrap(),
            &pages,
            "triage-test",
            "recognize-test",
        );
        let mut observed: HashMap<String, usize> = HashMap::new();
        for n in graph["nodes"].as_array().unwrap() {
            *observed
                .entry(n["node_type"].as_str().unwrap().into())
                .or_default() += 1;
        }
        for (kind, expected) in fixture["expected_counts"].as_object().unwrap() {
            assert_eq!(
                observed.get(kind).copied().unwrap_or(0),
                expected.as_u64().unwrap() as usize,
                "{} count mismatch for {}: observed {:?}",
                fixture["fixture_id"],
                kind,
                observed
            );
        }
        assert_eq!(
            observed.len(),
            fixture["expected_counts"].as_object().unwrap().len()
        );
        assert_eq!(
            graph["contradictions"].as_array().unwrap().len(),
            fixture["expected_contradictions"].as_u64().unwrap() as usize
        );
        let ambiguous = graph["chronology"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["date_expression"]["status"] == "ambiguous")
            .count();
        assert_eq!(
            ambiguous,
            fixture["ambiguous_dates"].as_u64().unwrap() as usize
        );
        json!({"fixture_id":fixture["fixture_id"],"observed":observed,"nodes":graph["nodes"].as_array().unwrap().len(),"edges":graph["edges"].as_array().unwrap().len(),"contradictions":graph["contradictions"].as_array().unwrap().len(),"ambiguous_dates":ambiguous,"provenance_defects":assert_extracted_sources(&graph).len()})
    }

    #[test]
    fn graph_golden_metrics() {
        let india = run_controlled_fixture(include_str!(
            "../../../golden/G-CASE-GRAPH/fixtures/india_controlled.json"
        ));
        let us = run_controlled_fixture(include_str!(
            "../../../golden/G-CASE-GRAPH/fixtures/us_controlled.json"
        ));
        println!(
            "B_S3_06_METRICS={}",
            json!({"india":india,"united_states":us})
        );
    }
}
