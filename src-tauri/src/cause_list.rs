//! Daily Cause-List Brief (B-S4-20, PRD v4.0 §9.4).
//!
//! A cause list is imported from a file (CSV, text/PDF lines) or typed in; no court portal is contacted. Each listed matter
//! is matched to a local matter, and a matched matter gets a one-page brief whose every field either cites the order or
//! document it comes from (document, page, character range and the exact excerpt) or says it was **not found in the record**.
//!
//! Everything comes from the orders on the record: an "order" is a block of a document that begins "Order dated <date>" or
//! "Order of <date>" (and the blocks that follow it up to the next such heading). Rule-based and deterministic - no model, so
//! nothing is summarised or inferred beyond the wording of the orders:
//!   stage        the stage named by the newest order that uses stage wording (packs/builtin/case_stages.v1.json), citing it
//!   last order   the newest order, whole, with its date
//!   compliance   each sentence of the last order that directs something ("shall", "directed to", "must"...), and whether a
//!                later dated sentence in a document on the record shows it done (a completion verb, not negated, sharing
//!                at least three content words with the direction); otherwise "not shown on the record" - never "not done"
//!   adjournments every order that adjourns the matter, with who sought it only where the order says ("at the request of
//!                the defendant"); where it does not say, the party is left empty
//!   applications each application number ("IA No. 45/2024") whose latest mention in the orders is not a disposal
//! A listing that matches no matter is "no local record"; one that cannot be told between matters waits for the judge's
//! confirmation. Contract: contracts/cause_list_brief.schema.json. Golden: golden/G-CAUSELIST.

use crate::chambers_lookup::question_terms;
use crate::issue_matrix::Party;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashSet};

pub const STAGES_JSON: &str = include_str!("../../packs/builtin/case_stages.v1.json");
const MATCH_CERTAIN: f64 = 0.8;
const MATCH_MARGIN: f64 = 0.2;
const MATCH_POSSIBLE: f64 = 0.5;
const COMPLETION_MIN_SHARED: usize = 3;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ListEntry {
    pub serial: String,
    pub title: String,
    pub case_number: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MatterDoc {
    pub doc_id: String,
    pub filename: String,
    pub type_tag: String,
    pub pages: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Matter {
    pub case_id: String,
    pub label: String,
    pub cause_title: String,
    pub jurisdiction: String,
    pub parties: Vec<Party>,
    pub docs: Vec<MatterDoc>,
}

// ---------------- import ----------------

static SERIAL_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s*(\d{1,3})[.)]\s+(\S.*)$").unwrap());
static TRAILING_NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+([A-Za-z.]{1,10}\s*\d{1,6}\s*/\s*\d{4})\s*$").unwrap());

/// A case number as written, in a comparable form: "CS 112 / 2023" -> "CS112/2023".
pub fn norm_number(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '/').collect::<String>().to_ascii_uppercase()
}

/// Lines of a cause list given as text (a PDF's text, or typed): "1. Title ... CS 112/2023".
pub fn parse_lines(text: &str) -> Vec<ListEntry> {
    text.lines()
        .filter_map(|l| {
            let c = SERIAL_LINE.captures(l)?;
            let rest = c[2].trim().to_string();
            Some(match TRAILING_NUMBER.captures(&rest) {
                Some(n) => ListEntry { serial: c[1].to_string(), title: rest[..n.get(0).unwrap().start()].trim().to_string(), case_number: Some(n[1].trim().to_string()) },
                None => ListEntry { serial: c[1].to_string(), title: rest, case_number: None },
            })
        })
        .collect()
}

fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let (mut rows, mut row, mut field, mut quoted) = (Vec::new(), Vec::new(), String::new(), false);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => row.push(std::mem::take(&mut field)),
            ('\r', false) => {}
            ('\n', false) => {
                row.push(std::mem::take(&mut field));
                if row.iter().any(|f| !f.trim().is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            _ => field.push(c),
        }
    }
    row.push(field);
    if row.iter().any(|f| !f.trim().is_empty()) {
        rows.push(row);
    }
    rows
}

/// A cause list as CSV. The first row is a header if it names the columns (serial, case number, title); otherwise the
/// columns are serial, case number, title in that order.
pub fn parse_csv(text: &str) -> Result<Vec<ListEntry>, String> {
    let rows = csv_rows(text.trim_start_matches('\u{feff}'));
    let first = rows.first().ok_or("CF_CAUSELIST_EMPTY: the file has no rows")?;
    let find = |names: &[&str]| first.iter().position(|h| names.contains(&h.trim().to_ascii_lowercase().replace(['.', '_'], " ").trim()));
    let (s, n, t) = (find(&["sr no", "sr", "serial", "serial no", "sl no", "s no", "item", "item no", "no"]), find(&["case number", "case no", "number", "case"]), find(&["cause title", "title", "case title", "parties", "cause"]));
    let (header, s, n, t) = if let (Some(t), true) = (t, s.is_some() || n.is_some()) { (true, s, n, t) } else { (false, Some(0), Some(1), 2) };
    let body = rows.iter().skip(usize::from(header));
    let out: Vec<ListEntry> = body
        .filter_map(|r| {
            let title = r.get(t)?.trim().to_string();
            if title.is_empty() {
                return None;
            }
            let get = |i: Option<usize>| i.and_then(|i| r.get(i)).map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
            Some(ListEntry { serial: get(s).unwrap_or_default(), title, case_number: get(n) })
        })
        .collect();
    if out.is_empty() {
        return Err("CF_CAUSELIST_EMPTY: no listed matter was found; the columns should be serial, case number and cause title".into());
    }
    Ok(out)
}

// ---------------- matching ----------------

const TITLE_STOP: &[&str] = &["v", "vs", "versus", "of", "and", "the", "others", "other", "anr", "ors", "another"];

fn title_tokens(s: &str) -> HashSet<String> {
    s.to_ascii_lowercase().split(|c: char| !c.is_ascii_alphanumeric()).filter(|t| !t.is_empty() && !TITLE_STOP.contains(t)).map(str::to_string).collect()
}

fn jaccard(a: &HashSet<String>, b: &HashSet<String>) -> f64 {
    let i = a.intersection(b).count() as f64;
    let u = a.union(b).count() as f64;
    if u == 0.0 { 0.0 } else { i / u }
}

static NUMBER_IN_TEXT: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z.]{1,10}\s*\d{1,6}\s*/\s*\d{4}").unwrap());

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatchResult {
    pub status: &'static str,
    pub case_id: Option<String>,
    /// For an ambiguous listing: the matters it could be, best first.
    pub candidates: Vec<String>,
}

/// Matches one listing to a local matter. A case number that belongs to exactly one matter decides; otherwise the cause
/// titles are compared. A listing is matched only when one matter is clearly the best; when it could be several the judge
/// confirms (`confirmed` is the matter the judge chose, if they have).
pub fn match_entry(entry: &ListEntry, matters: &[Matter], confirmed: Option<&str>) -> MatchResult {
    let numbers = |m: &Matter| -> HashSet<String> { NUMBER_IN_TEXT.find_iter(&format!("{} {}", m.label, m.cause_title)).map(|x| norm_number(x.as_str())).collect() };
    if let Some(n) = entry.case_number.as_deref().map(norm_number).filter(|n| !n.is_empty()) {
        let hit: Vec<&Matter> = matters.iter().filter(|m| numbers(m).contains(&n)).collect();
        if hit.len() == 1 {
            return MatchResult { status: "MATCHED", case_id: Some(hit[0].case_id.clone()), candidates: vec![] };
        }
    }
    let t = title_tokens(&entry.title);
    let mut scored: Vec<(f64, &Matter)> = matters.iter().map(|m| (jaccard(&t, &title_tokens(&m.cause_title)), m)).filter(|(s, _)| *s >= MATCH_POSSIBLE).collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.case_id.cmp(&b.1.case_id)));
    match scored.as_slice() {
        [] => MatchResult { status: "NO_LOCAL_RECORD", case_id: None, candidates: vec![] },
        [(s, m)] if *s >= MATCH_CERTAIN => MatchResult { status: "MATCHED", case_id: Some(m.case_id.clone()), candidates: vec![] },
        [(s0, m), (s1, _), ..] if *s0 >= MATCH_CERTAIN && s0 - s1 >= MATCH_MARGIN => MatchResult { status: "MATCHED", case_id: Some(m.case_id.clone()), candidates: vec![] },
        _ => {
            let candidates: Vec<String> = scored.iter().map(|(_, m)| m.case_id.clone()).collect();
            match confirmed.filter(|c| candidates.iter().any(|x| x == c)) {
                Some(c) => MatchResult { status: "AMBIGUOUS_CONFIRMED", case_id: Some(c.to_string()), candidates },
                None => MatchResult { status: "AMBIGUOUS_UNCONFIRMED", case_id: None, candidates },
            }
        }
    }
}

// ---------------- dates and orders ----------------

const MONTH_RX: &str = r"jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?";
static D_MON_Y: Lazy<Regex> = Lazy::new(|| Regex::new(&format!(r"(?i)\b(\d{{1,2}})(?:st|nd|rd|th)?[\s,-]+({MONTH_RX})\b\.?,?\s+(\d{{4}})\b")).unwrap());
static MON_D_Y: Lazy<Regex> = Lazy::new(|| Regex::new(&format!(r"(?i)\b({MONTH_RX})\b\.?\s+(\d{{1,2}})(?:st|nd|rd|th)?,?\s+(\d{{4}})\b")).unwrap());
static NUMERIC: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{1,2})[./-](\d{1,2})[./-](\d{4})\b").unwrap());
static ISO: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").unwrap());

fn month_no(name: &str) -> u32 {
    ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"].iter().position(|m| name.to_ascii_lowercase().starts_with(m)).map(|p| p as u32 + 1).unwrap_or(0)
}

fn iso(y: &str, m: u32, d: &str) -> Option<String> {
    let d: u32 = d.parse().ok()?;
    ((1..=12).contains(&m) && (1..=31).contains(&d)).then(|| format!("{y}-{m:02}-{d:02}"))
}

/// Dates written in the text (ISO form, byte span), in order of appearance. Numeric dates are day first unless
/// `day_first` is false (month first, as in the US).
pub fn dates_in(text: &str, day_first: bool) -> Vec<(String, usize, usize)> {
    let mut out: Vec<(String, usize, usize)> = Vec::new();
    let mut push = |iso: Option<String>, s: usize, e: usize, out: &mut Vec<(String, usize, usize)>| {
        if let Some(d) = iso {
            if !out.iter().any(|(_, a, b)| s < *b && *a < e) {
                out.push((d, s, e));
            }
        }
    };
    for c in D_MON_Y.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(iso(&c[3], month_no(&c[2]), &c[1]), m.start(), m.end(), &mut out);
    }
    for c in MON_D_Y.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(iso(&c[3], month_no(&c[1]), &c[2]), m.start(), m.end(), &mut out);
    }
    for c in NUMERIC.captures_iter(text) {
        let m = c.get(0).unwrap();
        let (a, b): (u32, u32) = (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0));
        let (day, month) = if day_first { (a, b) } else { (b, a) };
        push(iso(&c[3], month, &day.to_string()), m.start(), m.end(), &mut out);
    }
    for c in ISO.captures_iter(text) {
        let m = c.get(0).unwrap();
        push(iso(&c[1], c[2].parse().unwrap_or(0), &c[3]), m.start(), m.end(), &mut out);
    }
    out.sort_by_key(|(_, s, _)| *s);
    out
}

static HEADER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)^\s*(?:daily\s+)?orders?\s+(?:dated|of|on)\b").unwrap());
static BLANK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\r?\n[ \t]*(?:\r?\n[ \t]*)+").unwrap());

#[derive(Debug, Clone)]
struct Order {
    doc_id: String,
    page: usize,
    /// Character range inside the page.
    start: usize,
    end: usize,
    text: String,
    date: String,
}

fn char_at(t: &str, byte: usize) -> usize {
    t[..byte].chars().count()
}

/// Order entries of one page: blocks beginning "Order dated <date>" / "Order of <date>", with the blocks that follow up to the
/// next heading.
fn orders_of_page(doc_id: &str, page_no: usize, page: &str, day_first: bool) -> Vec<Order> {
    let mut blocks: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    let mut spans: Vec<(usize, usize)> = BLANK.find_iter(page).map(|m| (m.start(), m.end())).collect();
    spans.push((page.len(), page.len()));
    for (s, e) in spans {
        if !page[start..s].trim().is_empty() {
            blocks.push((start, s));
        }
        start = e;
    }
    let mut out: Vec<Order> = Vec::new();
    let mut open: Option<(usize, usize, String)> = None; // byte start, byte end, date
    let mut flush = |open: &mut Option<(usize, usize, String)>, out: &mut Vec<Order>| {
        if let Some((s, e, date)) = open.take() {
            let raw = &page[s..e];
            let lead = raw.len() - raw.trim_start().len();
            let text = raw.trim().to_string();
            out.push(Order { doc_id: doc_id.to_string(), page: page_no, start: char_at(page, s + lead), end: char_at(page, s + lead + text.len()), text, date });
        }
    };
    for (s, e) in blocks {
        let block = &page[s..e];
        let date = HEADER.is_match(block).then(|| dates_in(&block[..block.char_indices().nth(80).map(|(i, _)| i).unwrap_or(block.len())], day_first).into_iter().next()).flatten();
        match date {
            Some((d, _, _)) => {
                flush(&mut open, &mut out);
                open = Some((s, e, d));
            }
            None => {
                if let Some(o) = open.as_mut() {
                    o.1 = e;
                }
            }
        }
    }
    flush(&mut open, &mut out);
    out
}

/// All orders on the record, oldest first, and the ids of the documents they were found in.
fn orders_of(m: &Matter) -> (Vec<Order>, HashSet<String>) {
    let day_first = !m.jurisdiction.eq_ignore_ascii_case("US");
    let mut orders = Vec::new();
    let mut order_docs = HashSet::new();
    for d in &m.docs {
        let before = orders.len();
        for (i, p) in d.pages.iter().enumerate() {
            orders.extend(orders_of_page(&d.doc_id, i + 1, p, day_first));
        }
        if orders.len() > before || d.type_tag == "ORDER_SHEET" {
            order_docs.insert(d.doc_id.clone());
        }
    }
    orders.sort_by(|a, b| a.date.cmp(&b.date).then(a.doc_id.cmp(&b.doc_id)).then(a.page.cmp(&b.page)).then(a.start.cmp(&b.start)));
    (orders, order_docs)
}

// ---------------- sentences ----------------

const ABBREV: &[&str] = &["no", "nos", "rs", "mr", "mrs", "ms", "dr", "sh", "smt", "vs", "v", "ltd", "sec", "art", "ex", "hon", "addl", "distt", "dt", "st", "pvt", "co", "inc", "i", "ia"];

/// Sentences of a text as (char start, char end, text).
fn sentences(text: &str) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    while i < chars.len() {
        if matches!(chars[i], '.' | '!' | '?') {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_whitespace() {
                j += 1;
            }
            let word: String = chars[..i].iter().rev().take_while(|c| c.is_alphabetic()).collect::<Vec<_>>().into_iter().rev().collect();
            let boundary = j >= chars.len() || (j > i + 1 && chars[j].is_uppercase() && !ABBREV.contains(&word.to_ascii_lowercase().as_str()));
            if boundary {
                let seg: String = chars[start..=i].iter().collect();
                let lead = seg.chars().take_while(|c| c.is_whitespace()).count();
                let t = seg.trim().to_string();
                if !t.is_empty() {
                    out.push((start + lead, start + lead + t.chars().count(), t));
                }
                start = j;
                i = j;
                continue;
            }
        }
        i += 1;
    }
    if start < chars.len() {
        let seg: String = chars[start..].iter().collect();
        let lead = seg.chars().take_while(|c| c.is_whitespace()).count();
        let t = seg.trim().to_string();
        if !t.is_empty() {
            out.push((start + lead, start + lead + t.chars().count(), t));
        }
    }
    out
}

// ---------------- the brief ----------------

static DIRECTIVE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:shall|is\s+directed\s+to|are\s+directed\s+to|directed\s+to|ordered\s+to|is\s+to|are\s+to|must)\b").unwrap());
static COMPLETION: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:deposited|filed|produced|paid|complied|submitted|furnished|handed|delivered|executed|appeared|served|returned|supplied)\b").unwrap());
static NEGATION: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:not|never|no|failed|fail|fails|refused|unable|yet\s+to|neither|nor|without)\b").unwrap());
static ADJOURN: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\badjourn(?:ed|ment)\b").unwrap());
static SOUGHT_BY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:at\s+the\s+(?:request|instance)\s+of|on\s+the\s+(?:application|prayer|request|motion)\s+of|as\s+sought\s+by|on\s+behalf\s+of)\s+(?:the\s+)?(?:learned\s+)?(?:counsel\s+for\s+(?:the\s+)?)?([A-Za-z][A-Za-z' -]{0,40}?)(?:[.,;]|\s+(?:and|for|as|since|because|who|which|due)\b|$)").unwrap()
});
static APPLICATION_ID: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(I\.?\s?A\.?|C\.?M\.?A\.?|Crl\.?\s?M\.?P\.?)\s*(?:No\.?|Nos\.?)?\s*(\d+(?:\s*/\s*\d{2,4})?)").unwrap());
static DISPOSAL: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:allowed|dismissed|rejected|disposed|withdrawn|granted|decided)\b").unwrap());
static STAGES: Lazy<Vec<(String, Regex)>> = Lazy::new(|| {
    let v: Value = serde_json::from_str(STAGES_JSON).unwrap_or(Value::Null);
    v["stages"].as_array().into_iter().flatten().filter_map(|s| Some((s["label"].as_str()?.to_string(), Regex::new(s["pattern"].as_str()?).ok()?))).collect()
});

const NOT_FOUND: fn() -> Value = || json!({"state": "NOT_FOUND_IN_RECORD", "value": null, "locator": null});

fn locator(doc_id: &str, page: usize, start: usize, end: usize, excerpt: &str) -> Value {
    json!({"doc_id": doc_id, "page": page, "char_start": start, "char_end": end, "excerpt": excerpt})
}

fn found(value: &str, loc: Value) -> Value {
    json!({"state": "FOUND", "value": value, "locator": loc})
}

fn order_locator(o: &Order) -> Value {
    locator(&o.doc_id, o.page, o.start, o.end, &o.text)
}

fn sentence_locator(o: &Order, s: &(usize, usize, String)) -> Value {
    locator(&o.doc_id, o.page, o.start + s.0, o.start + s.1, &s.2)
}

fn party_of(word: &str, parties: &[Party]) -> Option<String> {
    let w = word.trim().to_ascii_lowercase();
    if w.is_empty() {
        return None;
    }
    parties.iter().find(|p| {
        let (role, name) = (p.role.to_ascii_lowercase(), p.display_name.to_ascii_lowercase());
        role == w || role.contains(&w) || w.contains(&role) && !role.is_empty() || name.contains(&w)
    }).map(|p| p.party_id.clone())
}

fn alpha_stems(text: &str) -> HashSet<String> {
    question_terms(text).into_iter().filter(|s| s.chars().all(|c| c.is_ascii_alphabetic())).collect()
}

/// Does a later dated sentence in a document that is not an order sheet show the direction done?
fn completion_for(directive: &str, order: &Order, m: &Matter, order_docs: &HashSet<String>) -> Option<Value> {
    let want = alpha_stems(directive);
    let day_first = !m.jurisdiction.eq_ignore_ascii_case("US");
    for d in m.docs.iter().filter(|d| !order_docs.contains(&d.doc_id)) {
        for (pi, page) in d.pages.iter().enumerate() {
            for (s, e, text) in sentences(page) {
                let dated_after = dates_in(&text, day_first).iter().any(|(date, _, _)| date.as_str() >= order.date.as_str());
                if dated_after && COMPLETION.is_match(&text) && !NEGATION.is_match(&text) && alpha_stems(&text).intersection(&want).count() >= COMPLETION_MIN_SHARED {
                    return Some(locator(&d.doc_id, pi + 1, s, e, &text));
                }
            }
        }
    }
    None
}

/// The brief of a matched matter (schema: contracts/cause_list_brief.schema.json `Brief`).
pub fn brief(m: &Matter) -> Value {
    let (orders, order_docs) = orders_of(m);
    let Some(last) = orders.last() else {
        return json!({"stage": NOT_FOUND(), "last_order": NOT_FOUND(), "last_order_date": NOT_FOUND(), "compliance": [], "adjournments": {"count": 0, "entries": []}, "pending_applications": []});
    };
    // stage: newest order with stage wording
    let stage = orders
        .iter()
        .rev()
        .find_map(|o| STAGES.iter().find(|(_, rx)| rx.is_match(&o.text)).map(|(label, _)| found(label, order_locator(o))))
        .unwrap_or_else(NOT_FOUND);
    // compliance
    let compliance: Vec<Value> = sentences(&last.text)
        .into_iter()
        .filter(|(_, _, t)| DIRECTIVE.is_match(t))
        .map(|s| {
            let done = completion_for(&s.2, last, m, &order_docs);
            json!({"directed": found(&s.2, sentence_locator(last, &s)), "record_status": if done.is_some() { "DONE_ON_RECORD" } else { "NOT_SHOWN_ON_RECORD" }, "done_locator": done})
        })
        .collect();
    // adjournments
    let adj: Vec<Value> = orders
        .iter()
        .filter(|o| ADJOURN.is_match(&o.text))
        .map(|o| {
            let by = SOUGHT_BY.captures(&o.text).and_then(|c| party_of(&c[1], &m.parties));
            json!({"order_date": o.date, "sought_by_party_id": by, "locator": order_locator(o)})
        })
        .collect();
    // applications: latest mention of each id decides
    let mut first_seen: Vec<String> = Vec::new();
    let mut latest: BTreeMap<String, (usize, (usize, usize, String))> = BTreeMap::new();
    for (oi, o) in orders.iter().enumerate() {
        for s in sentences(&o.text) {
            for c in APPLICATION_ID.captures_iter(&s.2) {
                let id = format!("{} {}", c[1].chars().filter(|x| x.is_alphabetic()).collect::<String>().to_ascii_uppercase(), c[2].replace(' ', ""));
                if !first_seen.contains(&id) {
                    first_seen.push(id.clone());
                }
                latest.insert(id, (oi, s.clone()));
            }
        }
    }
    let pending: Vec<Value> = first_seen
        .iter()
        .filter_map(|id| {
            let (oi, s) = latest.get(id)?;
            (!DISPOSAL.is_match(&s.2)).then(|| found(&s.2, sentence_locator(&orders[*oi], s)))
        })
        .collect();
    json!({
        "stage": stage, "last_order": found(&last.text, order_locator(last)), "last_order_date": found(&last.date, order_locator(last)),
        "compliance": compliance,
        "adjournments": {"count": adj.len(), "entries": adj},
        "pending_applications": pending,
    })
}

pub struct ImportInfo {
    /// PDF, CSV or MANUAL.
    pub format: &'static str,
    pub sha256: Option<String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// The cause-list brief document (the schema's) and, beside it, the candidates of each ambiguous listing.
pub fn render(court_label: &str, list_date: &str, import: &ImportInfo, entries: &[ListEntry], matters: &[Matter], confirmations: &BTreeMap<String, String>, now: &str) -> (Value, BTreeMap<String, Vec<String>>) {
    let mut out = Vec::new();
    let mut cands = BTreeMap::new();
    for e in entries {
        let m = match_entry(e, matters, confirmations.get(&e.serial).map(String::as_str));
        let b = match (&m.status, &m.case_id) {
            (&("MATCHED" | "AMBIGUOUS_CONFIRMED"), Some(id)) => matters.iter().find(|x| &x.case_id == id).map(brief),
            _ => None,
        };
        if !m.candidates.is_empty() {
            cands.insert(e.serial.clone(), m.candidates.clone());
        }
        out.push(json!({"serial": e.serial, "listed_title": e.title, "match": {"status": m.status, "case_id": m.case_id}, "brief": b}));
    }
    let basis = format!("{list_date}|{}", entries.iter().map(|e| format!("{}:{}", e.serial, e.title)).collect::<Vec<_>>().join("|"));
    (
        json!({
            "schema_version": "1.0.0", "list_id": format!("cl:{list_date}:{}", &sha256_hex(basis.as_bytes())[..12]), "court_label": court_label, "list_date": list_date,
            "import": {"format": import.format, "sha256": import.sha256}, "entries": out, "generated_at": now,
        }),
        cands,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../golden/G-CAUSELIST/fixtures/cause_list_core.json");
    const FIXTURE_SHA256: &str = "75b2630d218e78e9dae035bd7a3718825f181c5d67bd4ba959330140df22205d"; // committed before the engine (fd0c765)

    fn fx() -> Value {
        assert_eq!(sha256_hex(FIXTURE.as_bytes()), FIXTURE_SHA256, "G-CAUSELIST fixture changed since it was pinned");
        serde_json::from_str(FIXTURE).unwrap()
    }

    fn matters(fx: &Value) -> Vec<Matter> {
        serde_json::from_value(fx["matters"].clone()).unwrap()
    }

    fn entries(fx: &Value) -> Vec<ListEntry> {
        parse_csv(fx["list"]["csv"].as_str().unwrap()).unwrap()
    }

    fn text_at(ms: &[Matter], loc: &Value) -> String {
        let doc = ms.iter().flat_map(|m| m.docs.iter()).find(|d| d.doc_id == loc["doc_id"].as_str().unwrap()).unwrap();
        let page: Vec<char> = doc.pages[loc["page"].as_u64().unwrap() as usize - 1].chars().collect();
        page[loc["char_start"].as_u64().unwrap() as usize..loc["char_end"].as_u64().unwrap() as usize].iter().collect()
    }

    fn check_field(ms: &[Matter], got: &Value, want: &Value, what: &str) {
        assert_eq!(got["state"], want["state"], "{what}: state");
        assert_eq!(got["value"], want["value"], "{what}: value");
        if want["state"] == "FOUND" {
            assert_eq!(got["locator"]["doc_id"], want["locator"]["doc_id"], "{what}: document");
            assert_eq!(got["locator"]["page"], want["locator"]["page"], "{what}: page");
            assert_eq!(got["locator"]["excerpt"], want["locator"]["excerpt"], "{what}: excerpt");
            assert_eq!(got["locator"]["char_start"], want["locator"]["start"], "{what}: start");
            assert_eq!(text_at(ms, &got["locator"]), got["locator"]["excerpt"].as_str().unwrap(), "{what}: the excerpt is the record's text at its locator");
        } else {
            assert!(got["locator"].is_null() && got["value"].is_null(), "{what}: not found carries nothing");
        }
    }

    #[test]
    fn golden_import_reads_csv_and_lines() {
        let f = fx();
        let want = |k: &str| -> Vec<ListEntry> { serde_json::from_value(f["expect_import"][k].clone()).unwrap() };
        assert_eq!(parse_csv(f["list"]["csv"].as_str().unwrap()).unwrap(), want("csv"));
        assert_eq!(parse_lines(f["list"]["text_lines"].as_str().unwrap()), want("text"));
        // headerless CSV: serial, case number, title
        let h = parse_csv("1,CS 5/2020,A v. B\n2,,C v. D\n").unwrap();
        assert_eq!(h.iter().map(|e| (e.serial.as_str(), e.case_number.as_deref(), e.title.as_str())).collect::<Vec<_>>(), [("1", Some("CS 5/2020"), "A v. B"), ("2", None, "C v. D")]);
        assert!(parse_csv("Sr No,Case Number,Cause Title\n").is_err() && parse_csv("").is_err());
    }

    #[test]
    fn golden_matching_and_briefs() {
        let f = fx();
        let ms = matters(&f);
        let es = entries(&f);
        let info = ImportInfo { format: "CSV", sha256: Some(sha256_hex(f["list"]["csv"].as_str().unwrap().as_bytes())) };
        let (doc, cands) = render("Court No. 3", "2024-10-14", &info, &es, &ms, &BTreeMap::new(), "2024-10-14T08:00:00Z");
        assert_eq!(doc["entries"].as_array().unwrap().len(), 6);
        for e in doc["entries"].as_array().unwrap() {
            let serial = e["serial"].as_str().unwrap();
            let want = &f["expect"][serial];
            assert_eq!((e["match"]["status"].as_str(), e["match"]["case_id"].as_str()), (want["status"].as_str(), want["case_id"].as_str()), "serial {serial}: match");
            if want["brief"].is_null() {
                assert!(e["brief"].is_null(), "serial {serial}: no brief without a matched matter");
                continue;
            }
            let (g, w) = (&e["brief"], &want["brief"]);
            for k in ["stage", "last_order", "last_order_date"] {
                check_field(&ms, &g[k], &w[k], &format!("{serial}.{k}"));
            }
            assert_eq!(g["compliance"].as_array().unwrap().len(), w["compliance"].as_array().unwrap().len(), "{serial}: compliance items");
            for (gc, wc) in g["compliance"].as_array().unwrap().iter().zip(w["compliance"].as_array().unwrap()) {
                check_field(&ms, &gc["directed"], &wc["directed"], &format!("{serial}.directed"));
                assert_eq!(gc["record_status"], wc["record_status"], "{serial}: record status");
                if wc["done"].is_null() {
                    assert!(gc["done_locator"].is_null());
                } else {
                    assert_eq!((&gc["done_locator"]["doc_id"], &gc["done_locator"]["excerpt"]), (&wc["done"]["doc_id"], &wc["done"]["excerpt"]), "{serial}: where it is shown done");
                    assert_eq!(text_at(&ms, &gc["done_locator"]), gc["done_locator"]["excerpt"].as_str().unwrap());
                }
            }
            assert_eq!(g["adjournments"]["count"], w["adjournments"]["count"], "{serial}: adjournment count");
            for (ga, wa) in g["adjournments"]["entries"].as_array().unwrap().iter().zip(w["adjournments"]["entries"].as_array().unwrap()) {
                assert_eq!((&ga["order_date"], &ga["sought_by_party_id"], &ga["locator"]["excerpt"]), (&wa["order_date"], &wa["sought_by_party_id"], &wa["excerpt"]), "{serial}: adjournment");
                assert_eq!(text_at(&ms, &ga["locator"]), ga["locator"]["excerpt"].as_str().unwrap());
            }
            assert_eq!(g["pending_applications"].as_array().unwrap().len(), w["pending_applications"].as_array().unwrap().len(), "{serial}: pending applications");
            for (gp, wp) in g["pending_applications"].as_array().unwrap().iter().zip(w["pending_applications"].as_array().unwrap()) {
                check_field(&ms, gp, wp, &format!("{serial}.application"));
            }
        }
        assert_eq!(cands.get("3").cloned().unwrap_or_default(), f["expect"]["3"]["candidates"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>());
        // the judge confirms the ambiguous listing: it gets a brief, from the matter they chose
        let conf: BTreeMap<String, String> = serde_json::from_value(f["confirmations"].clone()).unwrap();
        let (doc2, _) = render("Court No. 3", "2024-10-14", &info, &es, &ms, &conf, "2024-10-14T08:00:00Z");
        let e3 = &doc2["entries"][2];
        let want = &f["expect_confirmed_3"];
        assert_eq!((e3["match"]["status"].as_str(), e3["match"]["case_id"].as_str()), (want["status"].as_str(), want["case_id"].as_str()));
        for k in ["stage", "last_order", "last_order_date"] {
            check_field(&ms, &e3["brief"][k], &want["brief"][k], &format!("3.{k}"));
        }
        assert!(e3["brief"]["compliance"].as_array().unwrap().is_empty() && e3["brief"]["adjournments"]["count"] == 0);
        // a "confirmation" for a matter that is not a candidate is ignored
        let wrong: BTreeMap<String, String> = [("3".to_string(), "m5".to_string())].into();
        assert_eq!(render("c", "2024-10-14", &info, &es, &ms, &wrong, "t").0["entries"][2]["match"]["status"], "AMBIGUOUS_UNCONFIRMED");
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            let d = std::path::PathBuf::from(dir);
            std::fs::write(d.join("cause_list_unconfirmed.json"), serde_json::to_vec_pretty(&doc).unwrap()).unwrap();
            std::fs::write(d.join("cause_list_confirmed.json"), serde_json::to_vec_pretty(&doc2).unwrap()).unwrap();
        }
    }

    fn walk(v: &Value, ms: &[Matter], cited: &mut i32, absent: &mut i32) {
        match v {
            Value::Object(o) if o.contains_key("state") && o.contains_key("locator") => match o["state"].as_str() {
                Some("FOUND") => {
                    assert!(!o["locator"].is_null() && o["value"].is_string());
                    assert_eq!(text_at(ms, &o["locator"]), o["locator"]["excerpt"].as_str().unwrap());
                    *cited += 1;
                }
                Some("NOT_FOUND_IN_RECORD") => {
                    assert!(o["locator"].is_null() && o["value"].is_null());
                    *absent += 1;
                }
                other => panic!("field state {other:?}"),
            },
            Value::Object(o) => o.values().for_each(|x| walk(x, ms, cited, absent)),
            Value::Array(a) => a.iter().for_each(|x| walk(x, ms, cited, absent)),
            _ => {}
        }
    }

    #[test]
    fn every_brief_field_cites_the_record_or_says_not_found() {
        // AC-S4-14.1
        let f = fx();
        let ms = matters(&f);
        let conf: BTreeMap<String, String> = serde_json::from_value(f["confirmations"].clone()).unwrap();
        let (doc, _) = render("c", "2024-10-14", &ImportInfo { format: "MANUAL", sha256: None }, &entries(&f), &ms, &conf, "t");
        let (mut cited, mut absent) = (0, 0);
        walk(&doc, &ms, &mut cited, &mut absent);
        assert!(cited >= 12 && absent >= 3, "cited {cited}, not found {absent}");
        for e in doc["entries"].as_array().unwrap().iter().filter(|e| !e["brief"].is_null()) {
            for a in e["brief"]["adjournments"]["entries"].as_array().unwrap() {
                assert_eq!(text_at(&ms, &a["locator"]), a["locator"]["excerpt"].as_str().unwrap());
            }
            for c in e["brief"]["compliance"].as_array().unwrap() {
                assert_eq!(c["record_status"] == "DONE_ON_RECORD", !c["done_locator"].is_null());
            }
        }
    }

    #[test]
    fn a_hundred_listed_matters_render_within_the_budget() {
        // AC-S4-14.2 (the Full reference machine is the owner's; this measures the machine the tests run on)
        let f = fx();
        let base = matters(&f);
        let mut ms = Vec::new();
        for i in 0..100 {
            let mut m = base[0].clone();
            m.case_id = format!("bulk-{i}");
            m.label = format!("CS {}/2024", 1000 + i);
            m.cause_title = format!("Party{i} v. Other{i}");
            for d in m.docs.iter_mut() {
                d.doc_id = format!("{}-{i}", d.doc_id);
            }
            ms.push(m);
        }
        let es: Vec<ListEntry> = (0..100).map(|i| ListEntry { serial: (i + 1).to_string(), title: format!("Party{i} vs Other{i}"), case_number: None }).collect();
        let started = std::time::Instant::now();
        let (doc, _) = render("c", "2024-10-14", &ImportInfo { format: "MANUAL", sha256: None }, &es, &ms, &BTreeMap::new(), "t");
        let secs = started.elapsed().as_secs_f64();
        assert_eq!(doc["entries"].as_array().unwrap().len(), 100);
        assert!(doc["entries"].as_array().unwrap().iter().all(|e| e["match"]["status"] == "MATCHED" && !e["brief"].is_null()));
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::PathBuf::from(dir).join("cause_list_budget.json"), serde_json::to_vec_pretty(&json!({"entries": 100, "matters": 100, "seconds": secs})).unwrap()).unwrap();
        }
        assert!(secs <= f["budget"]["max_seconds"].as_f64().unwrap(), "100 briefs took {secs:.2} s");
    }

    #[test]
    fn dates_sentences_and_numbers_follow_the_stated_rules() {
        assert_eq!(dates_in("on 12.03.2024 and 5th April, 2024 and May 3, 2024 and 2024-07-09", true).iter().map(|d| d.0.clone()).collect::<Vec<_>>(), ["2024-03-12", "2024-04-05", "2024-05-03", "2024-07-09"]);
        assert_eq!(dates_in("12/03/2024", true)[0].0, "2024-03-12");
        assert_eq!(dates_in("12/03/2024", false)[0].0, "2024-12-03", "month first for a US matter");
        assert!(dates_in("32.13.2024", true).is_empty());
        let s: Vec<String> = sentences("IA No. 45/2024 filed by Mr. Rao v. Rao is allowed. The Court sat on 5 May 2024. Rs. 5,000 paid.").into_iter().map(|x| x.2).collect();
        assert_eq!(s, ["IA No. 45/2024 filed by Mr. Rao v. Rao is allowed.", "The Court sat on 5 May 2024.", "Rs. 5,000 paid."]);
        assert_eq!(norm_number("CS 112 / 2023"), norm_number("cs112/2023"));
    }

    #[test]
    fn a_direction_is_not_shown_done_by_a_negated_or_undated_or_earlier_sentence() {
        let f = fx();
        let mut ms = matters(&f);
        let m = ms.iter_mut().find(|m| m.case_id == "m2").unwrap();
        for (page, why) in [
            ("On 6 September 2024 the accused did not deposit the fine amount of Rs 5,000 in the court registry.\n", "negated"),
            ("The accused deposited the fine amount of Rs 5,000 in the court registry.\n", "no date"),
            ("On 1 September 2024 the accused deposited the fine amount of Rs 5,000 in the court registry.\n", "dated before the order"),
            ("On 6 September 2024 the accused deposited the registration fee.\n", "too little in common with the direction"),
        ] {
            m.docs.iter_mut().find(|d| d.doc_id == "m2-report").unwrap().pages = vec![page.to_string()];
            assert_eq!(brief(m)["compliance"][0]["record_status"], "NOT_SHOWN_ON_RECORD", "{why}");
        }
    }

    #[test]
    fn the_engine_uses_no_model_or_network() {
        let src = include_str!("cause_list.rs").split("#[cfg(test)]").next().unwrap();
        for t in ["infer_llm", "ollama", "chat_with_timeout", "llm_chat", "reqwest", "http://", "https://"] {
            assert!(!src.contains(t), "{t}");
        }
    }
}
