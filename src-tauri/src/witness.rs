//! Witness resolution and Statement Matrix (B-S4-07, PRD v4.0 §7.3).
//!
//! Every statement of a witness (FIR, s.161/s.164 statements, examination-in-chief, cross; declarations, deposition
//! transcripts) is registered against a witness, ordered, and segmented into topic-tagged assertions, each an EXACT
//! excerpt of the stored text with a page/span or page:line locator. Deterministic: no model.
//!
//! Statements come from the statement register (counsel-reviewed proposals stored in the case vault), so counsel's
//! edits, merges and splits survive every rebuild. Statement kinds are data (packs/builtin/statement_kinds.v1.json).
//! Contract: contracts/witness_matrix.schema.json. Golden: golden/G-IMPEACH.

use crate::answer_feedback::{extract_amounts, extract_dates};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const PIPELINE_ID: &str = "caseforge.witness-matrix.v1";
const VOCAB_JSON: &str = include_str!("../../packs/builtin/statement_kinds.v1.json");

// ---------------------------------------------------------------------------------------------------------------
// Statement-kind vocabulary (data)

#[derive(Debug, Clone, Deserialize)]
pub struct KindDef {
    pub kind: String,
    pub label: String,
    pub jurisdiction: String,
    pub procedural_rank: i64,
    pub prior_statement: bool,
    pub testimony: bool,
    header_patterns: Vec<String>,
}

#[derive(Deserialize)]
struct VocabFile {
    pack_id: String,
    table_id: String,
    kinds: Vec<KindDef>,
    metadata_markers: Vec<String>,
}

pub struct Vocabulary {
    pub pack_id: String,
    pub table_id: String,
    kinds: Vec<(KindDef, Vec<Regex>)>,
    markers: Vec<String>,
}

impl Vocabulary {
    pub fn kind(&self, kind: &str) -> Option<&KindDef> {
        self.kinds.iter().map(|(k, _)| k).find(|k| k.kind == kind)
    }
    pub fn rank(&self, kind: &str) -> i64 {
        self.kind(kind).map(|k| k.procedural_rank).unwrap_or(90)
    }
    pub fn label(&self, kind: &str) -> String {
        self.kind(kind).map(|k| k.label.clone()).unwrap_or_else(|| "Other statement".into())
    }
    pub fn kinds(&self) -> Vec<&KindDef> {
        self.kinds.iter().map(|(k, _)| k).collect()
    }
}

pub static VOCAB: Lazy<Vocabulary> = Lazy::new(|| {
    let file: VocabFile = serde_json::from_str(VOCAB_JSON).expect("statement_kinds.v1.json is valid");
    Vocabulary {
        pack_id: file.pack_id,
        table_id: file.table_id,
        kinds: file
            .kinds
            .into_iter()
            .map(|k| {
                let res = k.header_patterns.iter().map(|p| Regex::new(&format!("(?im){p}")).expect("vocabulary regex")).collect();
                (k, res)
            })
            .collect(),
        markers: file.metadata_markers.into_iter().map(|m| m.to_lowercase()).collect(),
    }
});

pub fn is_us(jurisdiction: &str) -> bool {
    let j = jurisdiction.to_ascii_lowercase();
    j.contains("united states") || j == "us" || j == "usa" || j.starts_with("us-") || j.starts_with("u.s")
}

fn jurisdiction_ok(kind: &KindDef, jurisdiction: &str) -> bool {
    kind.jurisdiction == "*" || (is_us(jurisdiction) == is_us(&kind.jurisdiction))
}

// ---------------------------------------------------------------------------------------------------------------
// Statement register

#[derive(Debug, Clone)]
pub struct StatementDoc {
    pub doc_id: String,
    pub pages: Vec<String>,
}

/// One row of the statement register. `origin` is PROPOSED (auto) or COUNSEL_CONFIRMED (counsel edited/confirmed).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RegisterEntry {
    pub doc_id: String,
    pub witness_name: String,
    #[serde(default)]
    pub relation: Option<String>,
    pub statement_kind: String,
    pub recorded_on: Option<String>,
    pub locator_kind: String,
    #[serde(default)]
    pub sequence_override: Option<i64>,
    pub origin: String,
}

pub fn statement_id(doc_id: &str) -> String {
    format!("st:{doc_id}")
}

/// A re-proposal never overwrites counsel's version of the row (survives rerun).
pub fn merge_proposal(existing: Option<&RegisterEntry>, proposal: RegisterEntry) -> RegisterEntry {
    match existing {
        Some(e) if e.origin == "COUNSEL_CONFIRMED" => e.clone(),
        _ => proposal,
    }
}

const NAME: &str = r"((?:[A-Z]\.[ \t]?)*[A-Z][a-z]+(?:[ \t]+(?:[A-Z]\.[ \t]?)*[A-Z][a-z]+){0,3})";
static NAME_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    [
        format!(r"(?:[Ss]tatement|STATEMENT) of[ \t]+(?:(?:Smt|Shri|Sh|Mr|Mrs|Ms|Dr|Km)\.?[ \t]+)?{NAME}"),
        format!(r"\bP\.?W\.?[- ]?\d+[ \t]+(?:(?:Smt|Shri|Sh|Mr|Mrs|Ms|Dr)\.?[ \t]+)?{NAME}"),
        format!(r"(?i:informant|complainant)[ \t]*:[ \t]*(?:(?:Smt|Shri|Sh|Mr|Mrs|Ms|Dr)\.?[ \t]+)?{NAME}"),
        r"(?:DECLARATION|DEPOSITION|AFFIDAVIT|TESTIMONY) OF[ \t]+([A-Z][A-Z.'-]+(?:[ \t]+[A-Z][A-Z.'-]+){0,3})".to_string(),
        format!(r"\bI,[ \t]+{NAME},"),
    ]
    .iter()
    .map(|p| Regex::new(p).unwrap())
    .collect()
});
static RELATION: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?:\b[sSwWdD]/[oO]|son of|wife of|daughter of)[ \t]+((?:Shri[ \t]+|Sh\.[ \t]*)?[A-Z][a-z]+(?:[ \t]+[A-Z][a-z]+){0,2})").unwrap());
static RECORDED_ON: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(?:recorded on|date and time of report|date of report|executed on|taken on|sworn on|affirmed on|dated)\s*:?\s*([^\n]{0,40})").unwrap()
});
static NUMERIC_DATE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{1,2})[./-](\d{1,2})[./-](\d{4})\b").unwrap());
static TRANSCRIPT_LINE: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\s{0,4}(\d{1,2})\s{2,}(\S.*)$").unwrap());

fn title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|w| {
            let lower = w.to_lowercase();
            let mut c = lower.chars();
            c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Dates written in the text as ISO strings with byte spans: worded dates plus numeric d.m.y (India) / m/d/y (US).
pub fn dates_in(text: &str, us: bool) -> Vec<(String, usize, usize)> {
    let mut out = extract_dates(text);
    for c in NUMERIC_DATE.captures_iter(text) {
        let m = c.get(0).unwrap();
        if out.iter().any(|(_, s, e)| m.start() < *e && *s < m.end()) {
            continue;
        }
        let (a, b): (u32, u32) = (c[1].parse().unwrap_or(0), c[2].parse().unwrap_or(0));
        let (day, month) = if us { (b, a) } else { (a, b) };
        if (1..=31).contains(&day) && (1..=12).contains(&month) {
            out.push((format!("{}-{month:02}-{day:02}", &c[3]), m.start(), m.end()));
        }
    }
    out.sort_by_key(|(_, s, _)| *s);
    out
}

fn head(text: &str, chars: usize) -> &str {
    match text.char_indices().nth(chars) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

pub fn is_transcript_page(page: &str) -> bool {
    let lines: Vec<&str> = page.lines().filter(|l| !l.trim().is_empty()).collect();
    let numbered = lines.iter().filter(|l| TRANSCRIPT_LINE.is_match(l)).count();
    numbered >= 3 && numbered * 10 >= lines.len().saturating_sub(1) * 6
}

/// Proposes the register row for one document from its header. None if it does not look like a witness statement.
pub fn propose(doc: &StatementDoc, jurisdiction: &str) -> Option<RegisterEntry> {
    let first = doc.pages.iter().find(|p| !p.trim().is_empty())?;
    let header = head(first, 900);
    let us = is_us(jurisdiction);
    let kind = VOCAB
        .kinds
        .iter()
        .filter(|(k, _)| jurisdiction_ok(k, jurisdiction))
        .filter_map(|(k, res)| res.iter().filter_map(|r| r.find(header).map(|m| m.start())).min().map(|pos| (pos, k.kind.clone())))
        .min()
        .map(|(_, k)| k);
    let name = NAME_PATTERNS.iter().find_map(|r| r.captures(header).map(|c| c[1].trim().to_string()));
    if kind.is_none() && name.is_none() {
        return None;
    }
    let name = name.map(|n| if n.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase()) { title_case(&n) } else { n });
    let recorded_on = RECORDED_ON
        .captures_iter(header)
        .find_map(|c| dates_in(&c[1], us).into_iter().next().map(|d| d.0))
        .or_else(|| dates_in(header, us).into_iter().next().map(|d| d.0));
    let locator_kind = if doc.pages.iter().any(|p| is_transcript_page(p)) { "PAGE_LINE" } else { "PAGE_SPAN" };
    Some(RegisterEntry {
        doc_id: doc.doc_id.clone(),
        witness_name: name.unwrap_or_else(|| "Unnamed witness".into()),
        relation: RELATION.captures(header).map(|c| c[1].trim().to_string()),
        statement_kind: kind.unwrap_or_else(|| "OTHER".into()),
        recorded_on,
        locator_kind: locator_kind.into(),
        sequence_override: None,
        origin: "PROPOSED".into(),
    })
}

// ---------------------------------------------------------------------------------------------------------------
// Witness resolution

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Decision {
    Merge { witness_id: String, into_witness_id: String },
    Split { statement_id: String, new_display_name: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct Witness {
    pub witness_id: String,
    pub display_name: String,
    pub alias_node_ids: Vec<String>,
    pub resolution: String,
    pub statements: Vec<RegisterEntry>,
}

static HONORIFIC: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:smt|shri|sh|mr|mrs|ms|dr|km|pw-?\s?\d+|dw-?\s?\d+)\b\.?").unwrap());

/// Normalised witness key: lower-case, honorifics and PW/DW numbers removed, punctuation collapsed.
pub fn name_key(name: &str) -> String {
    let stripped = HONORIFIC.replace_all(name, " ");
    stripped
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn witness_id_for(name: &str) -> String {
    format!("w:{}", name_key(name).replace(' ', "-"))
}

fn order_statements(statements: &mut [RegisterEntry]) {
    statements.sort_by(|a, b| {
        let key = |e: &RegisterEntry| (e.recorded_on.is_none(), e.recorded_on.clone(), e.sequence_override.unwrap_or_else(|| VOCAB.rank(&e.statement_kind)), e.doc_id.clone());
        key(a).cmp(&key(b))
    });
}

/// Auto-resolution (same normalised name, no conflicting relation) followed by counsel decisions in order.
/// Returns the witnesses and any decision that no longer applies (stale after a register change).
pub fn resolve(entries: &[RegisterEntry], decisions: &[Decision]) -> (Vec<Witness>, Vec<String>) {
    let mut groups: BTreeMap<String, Vec<RegisterEntry>> = BTreeMap::new();
    for e in entries {
        groups.entry(name_key(&e.witness_name)).or_default().push(e.clone());
    }
    let mut witnesses: Vec<Witness> = Vec::new();
    for (key, group) in groups {
        // Different relations (s/o X vs s/o Y) under one name are different people; unrelated rows join the first.
        let mut relations: Vec<String> = Vec::new();
        for e in &group {
            if let Some(r) = &e.relation {
                let r = name_key(r);
                if !relations.contains(&r) {
                    relations.push(r);
                }
            }
        }
        let buckets = relations.len().max(1);
        let mut split: Vec<Vec<RegisterEntry>> = vec![Vec::new(); buckets];
        for e in group {
            let idx = e.relation.as_ref().and_then(|r| relations.iter().position(|x| *x == name_key(r))).unwrap_or(0);
            split[idx].push(e);
        }
        for (i, statements) in split.into_iter().enumerate() {
            let id = if i == 0 { format!("w:{}", key.replace(' ', "-")) } else { format!("w:{}-{}", key.replace(' ', "-"), i + 1) };
            witnesses.push(Witness { witness_id: id, display_name: String::new(), alias_node_ids: vec![], resolution: "PROPOSED".into(), statements });
        }
    }
    let mut stale = Vec::new();
    let mut touched: HashSet<String> = HashSet::new();
    for d in decisions {
        match d {
            Decision::Merge { witness_id, into_witness_id } => {
                let (Some(from), Some(_)) = (witnesses.iter().position(|w| &w.witness_id == witness_id), witnesses.iter().position(|w| &w.witness_id == into_witness_id)) else {
                    stale.push(format!("merge {witness_id} into {into_witness_id}: a witness no longer exists"));
                    continue;
                };
                if witness_id == into_witness_id {
                    continue;
                }
                let moved = witnesses.remove(from);
                let into = witnesses.iter_mut().find(|w| &w.witness_id == into_witness_id).unwrap();
                into.statements.extend(moved.statements);
                into.alias_node_ids.push(moved.witness_id);
                into.alias_node_ids.extend(moved.alias_node_ids);
                touched.insert(into_witness_id.clone());
            }
            Decision::Split { statement_id: sid, new_display_name } => {
                let Some(wi) = witnesses.iter().position(|w| w.statements.iter().any(|s| statement_id(&s.doc_id) == *sid)) else {
                    stale.push(format!("split {sid}: statement no longer registered"));
                    continue;
                };
                let si = witnesses[wi].statements.iter().position(|s| statement_id(&s.doc_id) == *sid).unwrap();
                let entry = witnesses[wi].statements.remove(si);
                touched.insert(witnesses[wi].witness_id.clone());
                if witnesses[wi].statements.is_empty() {
                    witnesses.remove(wi);
                }
                let new_id = witness_id_for(new_display_name);
                match witnesses.iter_mut().find(|w| w.witness_id == new_id) {
                    Some(w) => w.statements.push(entry),
                    None => witnesses.push(Witness { witness_id: new_id.clone(), display_name: new_display_name.clone(), alias_node_ids: vec![], resolution: "PROPOSED".into(), statements: vec![entry] }),
                }
                touched.insert(new_id);
            }
        }
    }
    for w in &mut witnesses {
        order_statements(&mut w.statements);
        if w.display_name.is_empty() {
            let mut counts: HashMap<&str, usize> = HashMap::new();
            for s in &w.statements {
                *counts.entry(s.witness_name.as_str()).or_default() += 1;
            }
            w.display_name = counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(a.0.len().cmp(&b.0.len())).then(b.0.cmp(a.0))).map(|(n, _)| n.to_string()).unwrap_or_default();
        }
        if touched.contains(&w.witness_id) || (!w.statements.is_empty() && w.statements.iter().all(|s| s.origin == "COUNSEL_CONFIRMED")) {
            w.resolution = "COUNSEL_CONFIRMED".into();
        }
    }
    witnesses.sort_by(|a, b| a.witness_id.cmp(&b.witness_id));
    (witnesses, stale)
}

// ---------------------------------------------------------------------------------------------------------------
// Segmentation into assertions with exact locators

#[derive(Debug, Clone, Default, Serialize)]
pub struct Facets {
    pub dates: Vec<String>,
    /// Minutes after midnight.
    pub times: Vec<u32>,
    pub amounts: Vec<String>,
    /// Inclusive ranges ("two or three persons" = 2..=3).
    pub counts: Vec<(u32, u32)>,
    pub places: Vec<Vec<String>>,
    /// (actor, normalised act)
    pub agents: Vec<(String, String)>,
    /// (named person or None for "anyone else", present?)
    pub presence: Vec<(Option<String>, bool)>,
    pub sequence: Option<(String, String)>,
    pub material: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Assertion {
    pub assertion_id: String,
    pub statement_id: String,
    pub doc_id: String,
    pub page_index: usize,
    pub char_start: usize,
    pub char_end: usize,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub excerpt: String,
    pub clean: String,
    pub topic_kind: String,
    pub facets: Facets,
    #[serde(skip)]
    pub keywords: HashSet<String>,
}

impl Assertion {
    pub fn locator(&self) -> Value {
        json!({"doc_id": self.doc_id, "page_index": self.page_index, "char_start": self.char_start, "char_end": self.char_end,
               "line_start": self.line_start, "line_end": self.line_end, "excerpt": self.excerpt})
    }
}

const ABBREVIATIONS: &[&str] = &["mr", "mrs", "ms", "dr", "smt", "shri", "sh", "no", "nos", "rs", "st", "vs", "v", "sr", "jr", "ltd", "co", "inc", "km", "approx", "viz", "etc", "ex"];

fn char_pos(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

/// Sentence byte ranges inside [start, end) of `text`.
fn sentences(text: &str, start: usize, end: usize) -> Vec<(usize, usize)> {
    let block = &text[start..end];
    let chars: Vec<(usize, char)> = block.char_indices().collect();
    let mut out = Vec::new();
    let mut s = 0usize;
    for (i, &(bi, c)) in chars.iter().enumerate() {
        if !matches!(c, '.' | '?' | '!') {
            continue;
        }
        let Some(&(_, next)) = chars.get(i + 1) else { continue };
        if !next.is_whitespace() {
            continue;
        }
        let Some(&(_, after)) = chars[i + 1..].iter().find(|(_, ch)| !ch.is_whitespace()) else { continue };
        if !(after.is_uppercase() || matches!(after, '"' | '“' | '\'')) {
            continue;
        }
        if c == '.' {
            let token_start = block[..bi].rfind(|ch: char| ch.is_whitespace() || ch == '(').map(|p| p + 1).unwrap_or(0);
            let token = block[token_start..bi].to_lowercase();
            let initials = token.split('.').all(|part| part.chars().count() == 1 && part.chars().all(|ch| ch.is_alphabetic()));
            if ABBREVIATIONS.contains(&token.as_str()) || (initials && !token.is_empty()) {
                continue;
            }
        }
        let e = bi + c.len_utf8();
        out.push((start + s, start + e));
        s = e;
    }
    if s < block.len() {
        out.push((start + s, end));
    }
    out
}

/// Paragraph blocks (separated by blank lines) as byte ranges.
fn blocks(page: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut last_end = 0usize;
    let mut pos = 0usize;
    for line in page.split_inclusive('\n') {
        let blank = line.trim().is_empty();
        if blank {
            if let Some(s) = start.take() {
                out.push((s, last_end));
            }
        } else {
            if start.is_none() {
                start = Some(pos);
            }
            last_end = pos + line.trim_end_matches(['\n', '\r']).len();
        }
        pos += line.len();
    }
    if let Some(s) = start {
        out.push((s, last_end));
    }
    out
}

static PARA_NUMBER: Lazy<Regex> = Lazy::new(|| Regex::new(r"^\(?\d{1,3}[.)]\s+").unwrap());
static QA_MARK: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(?:Q|A)[.:]\s+").unwrap());

fn collapse(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn is_metadata(clean: &str) -> bool {
    let lower = clean.to_lowercase();
    VOCAB.markers.iter().any(|m| lower.contains(m.as_str()))
}

fn is_heading(clean: &str) -> bool {
    let letters: Vec<char> = clean.chars().filter(|c| c.is_alphabetic()).collect();
    letters.len() >= 4 && letters.iter().filter(|c| c.is_uppercase()).count() * 10 > letters.len() * 8
}

/// Splits one statement into assertions with exact excerpts. Transcript pages become Q/A units with page:line.
pub fn segment(doc: &StatementDoc, us: bool) -> Vec<Assertion> {
    let sid = statement_id(&doc.doc_id);
    let mut out = Vec::new();
    for (page_index, page) in doc.pages.iter().enumerate() {
        let mut push = |bs: usize, be: usize, clean: String, lines: Option<(u32, u32)>| {
            if clean.split_whitespace().count() < 3 || is_metadata(&clean) || is_heading(&clean) {
                return;
            }
            let (cs, ce) = (char_pos(page, bs), char_pos(page, be));
            let facets = facets(&clean, us);
            out.push(Assertion {
                assertion_id: format!("{sid}#p{page_index}c{cs}"),
                statement_id: sid.clone(),
                doc_id: doc.doc_id.clone(),
                page_index,
                char_start: cs,
                char_end: ce,
                line_start: lines.map(|l| l.0),
                line_end: lines.map(|l| l.1),
                excerpt: page[bs..be].to_string(),
                topic_kind: topic_kind(&facets).into(),
                keywords: keywords(&clean),
                clean,
                facets,
            });
        };
        if is_transcript_page(page) {
            // (line number, content byte start, content byte end, content)
            let mut lines: Vec<(u32, usize, usize, String)> = Vec::new();
            let mut pos = 0usize;
            for raw in page.split_inclusive('\n') {
                let line = raw.trim_end_matches(['\n', '\r']);
                if let Some(c) = TRANSCRIPT_LINE.captures(line) {
                    let content = c.get(2).unwrap();
                    let trimmed = content.as_str().trim_end();
                    lines.push((c[1].parse().unwrap_or(0), pos + content.start(), pos + content.start() + trimmed.len(), trimmed.to_string()));
                }
                pos += raw.len();
            }
            let starts: Vec<usize> = lines.iter().enumerate().filter(|(_, l)| l.3.starts_with("Q.") || l.3.starts_with("Q:")).map(|(i, _)| i).collect();
            for (k, &i) in starts.iter().enumerate() {
                let j = starts.get(k + 1).copied().unwrap_or(lines.len());
                let unit = &lines[i..j];
                let clean = collapse(&unit.iter().map(|l| QA_MARK.replace(&l.3, "").to_string()).collect::<Vec<_>>().join(" "));
                push(unit[0].1, unit[unit.len() - 1].2, clean, Some((unit[0].0, unit[unit.len() - 1].0)));
            }
            continue;
        }
        for (b0, b1) in blocks(page) {
            for (s0, s1) in sentences(page, b0, b1) {
                let raw = &page[s0..s1];
                let lead = raw.len() - raw.trim_start().len();
                let mut bs = s0 + lead;
                let be = s0 + raw.trim_end().len();
                if bs >= be {
                    continue;
                }
                if let Some(m) = PARA_NUMBER.find(&page[bs..be]) {
                    bs += m.end();
                }
                push(bs, be, collapse(&page[bs..be]), None);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------
// Facets and keywords

static TIME_AMPM: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(\d{1,2})(?:[.:](\d{2}))?\s*([ap])\.?\s?m\b\.?").unwrap());
static TIME_PART_OF_DAY: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(\d{1,2})(?:[.:](\d{2}))?\s+(?:o'clock\s+)?in the (morning|afternoon|evening|night)\b").unwrap());
static TIME_24H: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b([01]?\d|2[0-3]):([0-5]\d)\b(?:[^-]|$)").unwrap());
static COUNT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|\d{1,2})(?:\s+or\s+(one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve|\d{1,2}))?\s+(?:\w+\s+)?(persons|people|men|women|boys|girls|assailants|attackers|accused|individuals|youths|strangers|vehicles|cars)\b").unwrap()
});
static PLACE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(?:near|at|outside|inside|behind|opposite|in front of)\s+").unwrap());
const PLACE_CUT: &[&str] = &["when", "while", "where", "with", "who", "which", "because", "but", "i", "we", "he", "she", "they", "it", "and then", "on", "at", "after", "before", "till", "until"];
const PLACE_SKIP_HEAD: &[&str] = &["about", "around", "approximately", "least", "last", "once", "that", "this", "night", "home", "times", "all", "first", "present", "the time"];
const ACT: &str = "hit|hits|struck|stabbed|attacked|assaulted|snatched|fired|shot|pushed|slapped|threatened|beat|kicked|robbed|grabbed|abused|dragged";
static AGENT: Lazy<Regex> = Lazy::new(|| Regex::new(&format!(r"\b([A-Z][a-z]+),?\s+(?:then\s+|also\s+)?({ACT})\b")).unwrap());
static PRESENCE_NAMED: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b([A-Z][a-z]+)\s+was\s+(not\s+)?(?:present|there|with me|with us|beside me|next to me|standing next to me|standing beside me|standing with me)\b").unwrap()
});
static PRESENCE_NOBODY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(?:did not see anyone else|didn't see anyone else|did not see anybody else|no one else was|nobody else was|there was no one else|i was alone|was all alone)\b").unwrap()
});
static SEQUENCE: Lazy<Regex> = Lazy::new(|| Regex::new(&format!(r"(?i)\bfirst\s+(?:\w+\s+)?({ACT})\b.{{0,80}}?\bthen\s+(?:[A-Z][a-z]+\s+)?(?:\w+\s+)?({ACT})\b")).unwrap());
const NON_NAMES: &[&str] = &["They", "He", "She", "It", "We", "I", "Then", "One", "The", "Who", "Someone", "Somebody", "Thereafter", "Both", "Accused", "All", "Yes", "No"];
const MATERIAL_STEMS: &[&str] = &[
    "identif", "recogni", "face", "knife", "lathi", "danda", "stick", "rod", "pistol", "revolver", "gun", "firearm", "sword", "axe", "blade", "dagger",
    "injur", "blood", "bleed", "wound", "fractur",
];
static WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"[a-z]+").unwrap());
const STOP: &[&str] = &[
    "that", "this", "with", "from", "have", "were", "which", "their", "there", "been", "also", "will", "would", "shall", "about", "around",
    "approximately", "into", "upon", "your", "what", "when", "where", "does", "them", "they", "then", "than", "such", "made", "over", "more",
    "some", "very", "just", "these", "those", "here", "being", "after", "before", "because", "while", "other", "each", "every", "both", "said",
    "says", "told", "time", "night", "morning", "evening", "afternoon", "first", "thereafter", "clearly", "near", "january", "february",
    "march", "april", "june", "july", "august", "september", "october", "november", "december", "three", "four", "five", "seven", "eight",
    "nine", "eleven", "twelve", "persons", "people", "believe",
];

pub fn keywords(text: &str) -> HashSet<String> {
    let lower = text.to_lowercase();
    WORD.find_iter(&lower).map(|m| m.as_str()).filter(|w| w.len() >= 4 && !STOP.contains(w)).map(str::to_string).collect()
}

fn number(word: &str) -> Option<u32> {
    let words = ["zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven", "twelve"];
    words.iter().position(|w| w.eq_ignore_ascii_case(word)).map(|n| n as u32).or_else(|| word.parse().ok())
}

fn act(verb: &str) -> String {
    match verb.to_lowercase().as_str() {
        "hits" | "struck" | "beat" => "hit".into(),
        "fired" | "shot" => "shoot".into(),
        v => v.to_string(),
    }
}

fn overlaps(spans: &[(usize, usize)], s: usize, e: usize) -> bool {
    spans.iter().any(|(a, b)| s < *b && *a < e)
}

pub fn facets(clean: &str, us: bool) -> Facets {
    let mut f = Facets::default();
    let mut date_spans = Vec::new();
    for (d, s, e) in dates_in(clean, us) {
        date_spans.push((s, e));
        if !f.dates.contains(&d) {
            f.dates.push(d);
        }
    }
    let mut time_spans: Vec<(usize, usize)> = date_spans.clone();
    for c in TIME_AMPM.captures_iter(clean) {
        let m = c.get(0).unwrap();
        if overlaps(&time_spans, m.start(), m.end()) {
            continue;
        }
        let (h, min): (u32, u32) = (c[1].parse().unwrap_or(0), c.get(2).and_then(|x| x.as_str().parse().ok()).unwrap_or(0));
        if h == 0 || h > 12 || min > 59 {
            continue;
        }
        let pm = c[3].eq_ignore_ascii_case("p");
        f.times.push(((h % 12) + if pm { 12 } else { 0 }) * 60 + min);
        time_spans.push((m.start(), m.end()));
    }
    for c in TIME_PART_OF_DAY.captures_iter(clean) {
        let m = c.get(0).unwrap();
        if overlaps(&time_spans, m.start(), m.end()) {
            continue;
        }
        let (h, min): (u32, u32) = (c[1].parse().unwrap_or(0), c.get(2).and_then(|x| x.as_str().parse().ok()).unwrap_or(0));
        if h == 0 || h > 12 || min > 59 {
            continue;
        }
        let later = match c[3].to_lowercase().as_str() {
            "afternoon" | "evening" => h < 12,
            "night" => h >= 6 && h < 12,
            _ => false,
        };
        f.times.push(((h % 12) + if later { 12 } else { 0 }) * 60 + min);
        time_spans.push((m.start(), m.end()));
    }
    for c in TIME_24H.captures_iter(clean) {
        let m = c.get(1).unwrap();
        let end = c.get(2).unwrap().end();
        if overlaps(&time_spans, m.start(), end) {
            continue;
        }
        f.times.push(c[1].parse::<u32>().unwrap_or(0) * 60 + c[2].parse::<u32>().unwrap_or(0));
        time_spans.push((m.start(), end));
    }
    for a in extract_amounts(clean) {
        if !f.amounts.contains(&a) {
            f.amounts.push(a);
        }
    }
    for c in COUNT.captures_iter(clean) {
        if let Some(a) = number(&c[1]) {
            let b = c.get(2).and_then(|x| number(x.as_str())).unwrap_or(a);
            f.counts.push((a.min(b), a.max(b)));
        }
    }
    // Each preposition is read on its own (a long earlier phrase must not swallow "near the temple").
    for m in PLACE.find_iter(clean) {
        let rest = &clean[m.end()..];
        let stop = rest.find([',', '.', ';', ':', '?', '!', '\n']).unwrap_or(rest.len());
        let mut phrase = rest[..stop].trim().to_lowercase();
        if phrase.len() < 2 {
            continue;
        }
        for cut in PLACE_CUT {
            if let Some(p) = phrase.find(&format!(" {cut} ")) {
                phrase.truncate(p);
            }
        }
        let tokens: Vec<String> = phrase
            .split(|ch: char| !ch.is_alphanumeric())
            .filter(|w| !w.is_empty() && !["the", "a", "an", "of", "and", "my", "his", "her", "our", "their", "your"].contains(w))
            .map(str::to_string)
            .collect();
        let first = tokens.first().map(String::as_str).unwrap_or("");
        if tokens.is_empty() || PLACE_SKIP_HEAD.contains(&first) || first.chars().next().map(|ch| ch.is_ascii_digit()).unwrap_or(true) {
            continue;
        }
        f.places.push(tokens);
    }
    for c in AGENT.captures_iter(clean) {
        if !NON_NAMES.contains(&&c[1]) {
            f.agents.push((c[1].to_string(), act(&c[2])));
        }
    }
    for c in PRESENCE_NAMED.captures_iter(clean) {
        if !NON_NAMES.contains(&&c[1]) {
            f.presence.push((Some(c[1].to_string()), c.get(2).is_none()));
        }
    }
    if PRESENCE_NOBODY.is_match(clean) {
        f.presence.push((None, false));
    }
    if let Some(c) = SEQUENCE.captures(clean) {
        let (a, b) = (act(&c[1]), act(&c[2]));
        if a != b {
            f.sequence = Some((a, b));
        }
    }
    let lower = clean.to_lowercase();
    for w in WORD.find_iter(&lower) {
        if let Some(stem) = MATERIAL_STEMS.iter().find(|s| w.as_str().starts_with(**s)) {
            if !f.material.contains(&stem.to_string()) {
                f.material.push(stem.to_string());
            }
        }
    }
    f
}

fn topic_kind(f: &Facets) -> &'static str {
    if f.material.iter().any(|m| ["identif", "recogni", "face"].contains(&m.as_str())) {
        "IDENTIFICATION"
    } else if !f.presence.is_empty() || !f.agents.is_empty() {
        "WHO"
    } else if !f.dates.is_empty() || !f.times.is_empty() {
        "WHEN"
    } else if !f.places.is_empty() {
        "WHERE"
    } else if !f.amounts.is_empty() || !f.counts.is_empty() {
        "AMOUNT"
    } else if f.sequence.is_some() {
        "SEQUENCE"
    } else {
        "WHAT"
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Matrix

pub struct BuiltWitness {
    pub witness: Witness,
    /// statement_id -> sequence index
    pub order: HashMap<String, usize>,
    pub assertions: Vec<Assertion>,
    pub topic_of: HashMap<String, String>,
    pub topics: Vec<Value>,
    pub texts: HashMap<String, Vec<String>>,
}

impl BuiltWitness {
    pub fn seq(&self, statement_id: &str) -> usize {
        self.order.get(statement_id).copied().unwrap_or(usize::MAX)
    }
}

fn find(parent: &mut Vec<usize>, i: usize) -> usize {
    let mut r = i;
    while parent[r] != r {
        r = parent[r];
    }
    let mut c = i;
    while parent[c] != r {
        let n = parent[c];
        parent[c] = r;
        c = n;
    }
    r
}

pub fn build_witness(witness: &Witness, docs: &HashMap<String, StatementDoc>, us: bool) -> BuiltWitness {
    let mut assertions = Vec::new();
    let mut texts = HashMap::new();
    let mut order = HashMap::new();
    for (i, s) in witness.statements.iter().enumerate() {
        order.insert(statement_id(&s.doc_id), i);
        if let Some(doc) = docs.get(&s.doc_id) {
            assertions.extend(segment(doc, us));
            texts.insert(statement_id(&s.doc_id), doc.pages.clone());
        }
    }
    // Topics: assertions from different statements that share >= 2 specific words fall in one topic.
    let n = assertions.len();
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in i + 1..n {
            if assertions[i].statement_id != assertions[j].statement_id && assertions[i].keywords.intersection(&assertions[j].keywords).count() >= 2 {
                let (a, b) = (find(&mut parent, i), find(&mut parent, j));
                parent[b] = a;
            }
        }
    }
    let mut topic_ids: BTreeMap<usize, String> = BTreeMap::new();
    let mut members: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut topic_order: Vec<String> = Vec::new();
    let mut topic_of = HashMap::new();
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by_key(|&i| (order.get(&assertions[i].statement_id).copied().unwrap_or(usize::MAX), assertions[i].page_index, assertions[i].char_start));
    for i in idx {
        let root = find(&mut parent, i);
        let next = format!("t{}", topic_ids.len() + 1);
        let tid = topic_ids.entry(root).or_insert(next).clone();
        if !members.contains_key(&tid) {
            topic_order.push(tid.clone());
        }
        members.entry(tid.clone()).or_default().push(i);
        topic_of.insert(assertions[i].assertion_id.clone(), tid);
    }
    let priority = ["IDENTIFICATION", "WHO", "WHEN", "WHERE", "AMOUNT", "SEQUENCE", "WHAT"];
    let topics = topic_order
        .iter()
        .map(|tid| {
            let m = &members[tid];
            let mut freq: BTreeMap<&str, usize> = BTreeMap::new();
            let mut kinds: HashMap<&str, usize> = HashMap::new();
            for &i in m {
                for k in &assertions[i].keywords {
                    *freq.entry(k.as_str()).or_default() += 1;
                }
                *kinds.entry(assertions[i].topic_kind.as_str()).or_default() += 1;
            }
            let mut words: Vec<(&str, usize)> = freq.into_iter().collect();
            words.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            let label = words.iter().take(3).map(|w| w.0).collect::<Vec<_>>().join(" · ");
            let kind = priority.iter().max_by_key(|k| (kinds.get(**k).copied().unwrap_or(0), std::cmp::Reverse(priority.iter().position(|p| p == *k)))).copied().unwrap_or("OTHER");
            json!({"topic_id": tid, "topic_kind": kind, "label": if label.is_empty() { "statement".to_string() } else { label }})
        })
        .collect();
    BuiltWitness { witness: witness.clone(), order, assertions, topic_of, topics, texts }
}

pub fn matrix_json(case_id: &str, graph_version: &str, built_at: &str, built: &[BuiltWitness]) -> Value {
    let witnesses: Vec<Value> = built
        .iter()
        .map(|b| {
            let statements: Vec<Value> = b
                .witness
                .statements
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    json!({"statement_id": statement_id(&s.doc_id), "statement_kind": s.statement_kind, "recorded_on": s.recorded_on,
                           "doc_id": s.doc_id, "locator_kind": s.locator_kind, "sequence_index": i})
                })
                .collect();
            let mut cells: BTreeMap<(String, usize), Vec<Value>> = BTreeMap::new();
            for a in &b.assertions {
                let tid = b.topic_of[&a.assertion_id].clone();
                cells.entry((tid, b.seq(&a.statement_id))).or_default().push(json!({"assertion_id": a.assertion_id, "text": a.clean, "locator": a.locator()}));
            }
            let order_of = |t: &str| b.topics.iter().position(|x| x["topic_id"] == t).unwrap_or(usize::MAX);
            let mut keys: Vec<&(String, usize)> = cells.keys().collect();
            keys.sort_by_key(|(t, s)| (order_of(t), *s));
            let cells: Vec<Value> = keys
                .into_iter()
                .map(|k| json!({"topic_id": k.0, "statement_id": statement_id(&b.witness.statements[k.1].doc_id), "assertions": cells[k]}))
                .collect();
            json!({"witness_id": b.witness.witness_id, "display_name": b.witness.display_name, "alias_node_ids": b.witness.alias_node_ids,
                   "resolution": b.witness.resolution, "statements": statements, "topics": b.topics, "cells": cells})
        })
        .collect();
    let digest = crate::certificate::sha256_hex(serde_json::to_string(&witnesses).unwrap_or_default().as_bytes());
    json!({
        "schema_version": "1.0.0",
        "matrix_id": format!("wm-{}", &digest[..16]),
        "case_id": case_id,
        "built_at": built_at,
        "graph_version": graph_version,
        "statement_kind_vocabulary": {"pack_id": VOCAB.pack_id, "table_id": VOCAB.table_id},
        "witnesses": witnesses,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub const FIXTURE: &str = include_str!("../../golden/G-IMPEACH/fixtures/impeach_core.json");

    pub fn docs_of(case: &Value) -> Vec<StatementDoc> {
        case["documents"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| StatementDoc {
                doc_id: d["doc_id"].as_str().unwrap().into(),
                pages: d["pages"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect(),
            })
            .collect()
    }

    pub fn decisions(fx: &Value) -> Vec<Decision> {
        serde_json::from_value(fx["expected"]["counsel_decisions"].clone()).unwrap()
    }

    /// Proposals for both fixture cases, with the docs and jurisdiction flag.
    pub fn register(fx: &Value) -> (Vec<RegisterEntry>, HashMap<String, StatementDoc>, HashMap<String, bool>) {
        let mut entries = Vec::new();
        let mut docs = HashMap::new();
        let mut us = HashMap::new();
        for case in fx["cases"].as_array().unwrap() {
            let j = case["jurisdiction"].as_str().unwrap();
            for d in docs_of(case) {
                entries.push(propose(&d, j).unwrap_or_else(|| panic!("no proposal for {}", d.doc_id)));
                us.insert(d.doc_id.clone(), is_us(j));
                docs.insert(d.doc_id.clone(), d);
            }
        }
        (entries, docs, us)
    }

    pub fn built(fx: &Value) -> Vec<BuiltWitness> {
        let (entries, docs, us) = register(fx);
        let (witnesses, stale) = resolve(&entries, &decisions(fx));
        assert!(stale.is_empty(), "{stale:?}");
        witnesses.iter().map(|w| build_witness(w, &docs, us[&w.statements[0].doc_id])).collect()
    }

    #[test]
    fn proposals_match_headers() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (entries, _, _) = register(&fx);
        for e in fx["expected"]["proposals"].as_array().unwrap() {
            let got = entries.iter().find(|x| x.doc_id == e["doc_id"].as_str().unwrap()).unwrap();
            assert_eq!(got.witness_name, e["witness_name"].as_str().unwrap(), "{got:?}");
            assert_eq!(got.statement_kind, e["statement_kind"].as_str().unwrap(), "{got:?}");
            assert_eq!(got.recorded_on.as_deref(), e["recorded_on"].as_str(), "{got:?}");
            assert_eq!(got.locator_kind, e["locator_kind"].as_str().unwrap(), "{got:?}");
        }
    }

    #[test]
    fn auto_resolution_then_counsel_merge_and_split() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (entries, _, _) = register(&fx);
        let (auto, _) = resolve(&entries, &[]);
        let exp = fx["expected"]["auto_witnesses"].as_object().unwrap();
        assert_eq!(auto.len(), exp.len(), "{:#?}", auto.iter().map(|w| &w.witness_id).collect::<Vec<_>>());
        for (id, docs) in exp {
            let w = auto.iter().find(|w| &w.witness_id == id).unwrap_or_else(|| panic!("missing {id}"));
            let mut got: Vec<&str> = w.statements.iter().map(|s| s.doc_id.as_str()).collect();
            let mut want: Vec<&str> = docs.as_array().unwrap().iter().map(|d| d.as_str().unwrap()).collect();
            got.sort();
            want.sort();
            assert_eq!(got, want, "{id}");
        }
        let (resolved, stale) = resolve(&entries, &decisions(&fx));
        assert!(stale.is_empty());
        let exp = fx["expected"]["resolved_witnesses"].as_object().unwrap();
        assert_eq!(resolved.len(), exp.len());
        for (id, e) in exp {
            let w = resolved.iter().find(|w| &w.witness_id == id).unwrap_or_else(|| panic!("missing {id}"));
            let got: Vec<&str> = w.statements.iter().map(|s| s.doc_id.as_str()).collect();
            let want: Vec<&str> = e["statements_in_order"].as_array().unwrap().iter().map(|d| d.as_str().unwrap()).collect();
            assert_eq!(got, want, "{id} order");
            assert_eq!(w.resolution, e["resolution"].as_str().unwrap(), "{id}");
            assert_eq!(serde_json::to_value(&w.alias_node_ids).unwrap(), e["alias_node_ids"], "{id}");
        }
    }

    #[test]
    fn counsel_decisions_survive_rerun() {
        // Rerun = proposals regenerated from the same documents; counsel's edited row and decisions are re-applied.
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let (entries, _, _) = register(&fx);
        let mut edited = entries.iter().find(|e| e.doc_id == "doc-161-pw4").unwrap().clone();
        edited.witness_name = "Ramesh Yadav (PW-4)".into();
        edited.recorded_on = Some("2023-03-14".into());
        edited.origin = "COUNSEL_CONFIRMED".into();
        let (rerun, _, _) = register(&fx);
        let merged: Vec<RegisterEntry> = rerun.into_iter().map(|p| { let e = if p.doc_id == edited.doc_id { Some(&edited) } else { None }; merge_proposal(e, p) }).collect();
        assert_eq!(merged.iter().find(|e| e.doc_id == "doc-161-pw4").unwrap(), &edited, "counsel edit survives re-proposal");
        let (first, _) = resolve(&entries, &decisions(&fx));
        let (second, stale) = resolve(&merged, &decisions(&fx));
        assert!(stale.is_empty());
        let shape = |ws: &[Witness]| ws.iter().map(|w| (w.witness_id.clone(), w.statements.iter().map(|s| s.doc_id.clone()).collect::<Vec<_>>())).collect::<Vec<_>>();
        assert_eq!(shape(&first), shape(&second), "merge/split survive rerun");
        assert_eq!(second.iter().find(|w| w.witness_id == "w:ramesh-yadav").unwrap().resolution, "COUNSEL_CONFIRMED");
    }

    #[test]
    fn every_matrix_cell_is_an_exact_located_excerpt() {
        // AC-S4-03.2 (matrix): each assertion text is the exact stored substring at its locator.
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let built = built(&fx);
        let mut cells = 0;
        for b in &built {
            for a in &b.assertions {
                let page = &b.texts[&a.statement_id][a.page_index];
                let slice: String = page.chars().skip(a.char_start).take(a.char_end - a.char_start).collect();
                assert_eq!(slice, a.excerpt, "{}", a.assertion_id);
                assert!(!a.excerpt.trim().is_empty());
                let kind = &b.witness.statements[b.seq(&a.statement_id)].locator_kind;
                if kind == "PAGE_LINE" && is_transcript_page(page) {
                    let (s, e) = (a.line_start.unwrap(), a.line_end.unwrap());
                    assert!(s >= 1 && e >= s, "{}", a.assertion_id);
                }
                cells += 1;
            }
        }
        assert!(cells > 20, "matrix too small: {cells}");
        let matrix = matrix_json("case-imp", "graph-v", "2026-10-08T00:00:00Z", &built);
        let contract: Value = serde_json::from_str(include_str!("../../contracts/witness_matrix.schema.json")).unwrap();
        for k in contract["required"].as_array().unwrap() {
            assert!(matrix.get(k.as_str().unwrap()).is_some(), "missing {k}");
        }
        if let Ok(dir) = std::env::var("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("witness_matrix.json"), serde_json::to_string_pretty(&matrix).unwrap()).unwrap();
        }
    }

    #[test]
    fn similar_names_are_never_merged_automatically() {
        let (ws, _) = resolve(
            &[
                RegisterEntry { doc_id: "a".into(), witness_name: "Ramesh Kumar".into(), relation: Some("Shyam Lal".into()), statement_kind: "FIR".into(), recorded_on: None, locator_kind: "PAGE_SPAN".into(), sequence_override: None, origin: "PROPOSED".into() },
                RegisterEntry { doc_id: "b".into(), witness_name: "Ramesh Kumar".into(), relation: Some("Hari Om".into()), statement_kind: "FIR".into(), recorded_on: None, locator_kind: "PAGE_SPAN".into(), sequence_override: None, origin: "PROPOSED".into() },
                RegisterEntry { doc_id: "c".into(), witness_name: "R. Kumar".into(), relation: None, statement_kind: "FIR".into(), recorded_on: None, locator_kind: "PAGE_SPAN".into(), sequence_override: None, origin: "PROPOSED".into() },
            ],
            &[],
        );
        assert_eq!(ws.len(), 3, "different fathers and an initial-only name stay separate until counsel merges");
    }
}
