//! Citation & Quote Audit (B-S4-04, PRD v4.0 §7.1).
//!
//! Finds every citation and quotation in a draft and checks it against the INSTALLED legal packs:
//!   VERIFIED            official, applicable, current source; quotation is an exact passage (whitespace-normalised)
//!   MISQUOTED           the cited source exists but the quoted words differ (nearest passage + word diff)
//!   NOT_IN_CORPUS       not in the installed packs (a repository gap — never called "fake")
//!   SUPERSEDED / NOT_APPLICABLE / CUSTOM_SOURCE   per the source's legal metadata
//! Pinpoints (para / section) are checked against the source text. Deterministic: no model.
//! Contract: contracts/audit_report.schema.json. Golden: golden/G-AUDIT.

use crate::retrieval::store::{DocRef, GroupRec, Store};
use crate::retrieval::CorpusChunk;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap};

pub const PIPELINE_ID: &str = "caseforge.audit.v1";
pub const SCHEMA_VERSION: &str = "1.0.0";
const RESULTS: [&str; 7] = ["VERIFIED", "VERIFIED_WITH_NOTE", "MISQUOTED", "NOT_IN_CORPUS", "SUPERSEDED", "NOT_APPLICABLE", "CUSTOM_SOURCE"];
const MIN_QUOTE_CHARS: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LegalLocator {
    pub pack_id: String,
    pub source_id: String,
    pub chunk_id: String,
    pub char_start: usize,
    pub char_end: usize,
    pub page_index: Option<i64>,
    pub paragraph: Option<String>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pinpoint {
    pub claimed: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOp {
    pub op: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Misquote {
    pub nearest: LegalLocator,
    pub diff_ops: Vec<DiffOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub note_kind: String,
    pub text: String,
    pub ref_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditItem {
    pub item_id: String,
    pub kind: String,
    pub citation_kind: Option<String>,
    pub extraction_rule_id: Option<String>,
    pub draft_char_start: usize,
    pub draft_char_end: usize,
    pub draft_text: String,
    pub attributed_citation_item_id: Option<String>,
    pub result: String,
    pub resolved: Option<LegalLocator>,
    pub pinpoint: Pinpoint,
    pub notes: Vec<Note>,
    pub misquote: Option<Misquote>,
    pub proposition_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Subject {
    pub subject_kind: String,
    pub doc_id: Option<String>,
    pub filing_party_id: Option<String>,
    pub sha256: String,
    pub filename: String,
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackSnapshot {
    pub pack_id: String,
    pub version: String,
    pub currency_date: Option<String>,
    pub trust_level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    pub schema_version: String,
    pub audit_id: String,
    pub pipeline_id: String,
    pub generated_at: String,
    pub case_id: Option<String>,
    pub subject: Subject,
    pub pack_snapshot: Vec<PackSnapshot>,
    pub model_fingerprint: Option<String>,
    pub items: Vec<AuditItem>,
    pub counts: BTreeMap<String, usize>,
}

/// One installed source (all its chunks), with the metadata that decides the audit result.
struct Source<'a> {
    source_id: String,
    title: String,
    citation: String,
    authority_type: String,
    trust_level: String,
    temporal_status: String,
    superseded_status: String,
    chunks: Vec<&'a CorpusChunk>,
}

static REPORTER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
          \(\d{4}\)\s*\d{1,3}\s*SCC\s+(?:OnLine\s+[A-Za-z.]+\s+)?\d{1,5}
        | \d{4}\s+SCC\s+OnLine\s+[A-Za-z.]+\s+\d{1,6}
        | AIR\s+\d{4}\s+[A-Za-z.]{2,12}\s+\d{1,5}
        | \d{4}\s+INSC\s+\d{1,5}
        | \[\d{4}\]\s*\d{0,3}\s*[A-Z][A-Za-z.]{1,10}\s+\d{1,5}
        | \d{1,4}\s+(?:U\.\s?S\.|S\.\s?Ct\.|L\.\s?Ed\.(?:\s?2d)?|F\.\s?(?:2d|3d|4th)|F\.\s?Supp\.(?:\s?(?:2d|3d))?)\s+\d{1,5}",
    )
    .unwrap()
});
static AFTER_PINPOINT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*,?\s*(?:at\s+|paras?\.?\s*|paragraph\s+|¶\s*)(\d{1,4})").unwrap());
static SECTION_OF_ACT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b(?:[Ss]ection|[Ss]ec\.|s\.)\s*(\d{1,4}[A-Z]?)(?:\s*\(\d+\))?\s+of\s+(?:the\s+)?([A-Z][A-Za-z]*(?:\s+[A-Z][A-Za-z]*)*\s+Act,?\s+\d{4})").unwrap()
});
static ACT_SECTION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b([A-Z][A-Za-z]*(?:\s+[A-Z][A-Za-z]*)*\s+Act,?\s+\d{4}),?\s*(?:ss?\.|[Ss]ection|[Ss]ec\.)\s*(\d{1,4}[A-Z]?)").unwrap()
});
static BARE_ACT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b([A-Z][A-Za-z]*(?:\s+[A-Z][A-Za-z]*)*\s+Act,?\s+\d{4})").unwrap());
static CURLY_QUOTE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\u{201C}([^\u{201D}]+)\u{201D}").unwrap());
static STRAIGHT_QUOTE: Lazy<Regex> = Lazy::new(|| Regex::new(r#""([^"]+)""#).unwrap());
static WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"\S+").unwrap());

fn norm(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

fn norm_title(text: &str) -> String {
    let t = text.trim();
    let t = t.strip_prefix("The ").or_else(|| t.strip_prefix("the ")).unwrap_or(t);
    norm(t)
}

fn char_index(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

fn classify(s: &Source) -> &'static str {
    if s.trust_level != "OFFICIAL_VERIFIED" {
        "CUSTOM_SOURCE"
    } else if s.superseded_status == "SUPERSEDED" || s.superseded_status == "PARTIALLY_SUPERSEDED" {
        "SUPERSEDED"
    } else if s.temporal_status != "APPLICABLE" || s.superseded_status != "CURRENT" {
        // UNKNOWN status can never be presented as controlling (B-S3-08).
        "NOT_APPLICABLE"
    } else {
        "VERIFIED"
    }
}

/// Group chunks into authorities. Packs may store one source per PDF page, so pages of the same work
/// (same pack, title and citation) form ONE authority; the most restrictive status across its pages applies.
fn group_sources(corpus: &[CorpusChunk]) -> Vec<Source<'_>> {
    let mut by_id: BTreeMap<(String, String, String), Source> = BTreeMap::new();
    for c in corpus {
        let m = &c.metadata;
        let entry = by_id
            .entry((c.pack_id.clone(), m.title.clone(), m.citation.clone()))
            .or_insert_with(|| Source {
                source_id: m.source_id.clone(),
                title: m.title.clone(),
                citation: m.citation.clone(),
                authority_type: m.authority_type.clone(),
                trust_level: m.trust_level.clone(),
                temporal_status: m.temporal_status.clone(),
                superseded_status: m.superseded_status.clone(),
                chunks: Vec::new(),
            });
        if m.trust_level != "OFFICIAL_VERIFIED" {
            entry.trust_level = m.trust_level.clone();
        }
        if m.superseded_status != "CURRENT" && entry.superseded_status != "SUPERSEDED" {
            entry.superseded_status = m.superseded_status.clone();
        }
        if m.temporal_status != "APPLICABLE" {
            entry.temporal_status = m.temporal_status.clone();
        }
        entry.chunks.push(c);
    }
    by_id.into_values().collect()
}

fn page_of(c: &CorpusChunk) -> Option<i64> {
    c.metadata.page.as_i64()
}

fn locator(c: &CorpusChunk, byte_start: usize, byte_end: usize) -> LegalLocator {
    LegalLocator {
        pack_id: c.pack_id.clone(),
        source_id: c.metadata.source_id.clone(),
        chunk_id: c.chunk_id.clone(),
        char_start: char_index(&c.text, byte_start),
        char_end: char_index(&c.text, byte_end),
        page_index: page_of(c),
        paragraph: (!c.metadata.paragraph.is_empty()).then(|| c.metadata.paragraph.clone()),
        excerpt: c.text[byte_start..byte_end].to_string(),
    }
}

/// Section / paragraph heading "N." at the start of a line in any chunk of the source.
/// Of several "N." headings (e.g. arrangement-of-sections list vs. the section itself), prefer the one followed by
/// the most text before the next heading - the operative provision rather than its table-of-contents line.
fn find_heading<'a>(s: &Source<'a>, number: &str) -> Option<(&'a CorpusChunk, usize, usize)> {
    static NEXT_HEADING: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^\s*\d{1,4}[A-Z]?\.\s").unwrap());
    let re = Regex::new(&format!(r"(?m)^\s*{}\.\s", regex::escape(number))).ok()?;
    s.chunks
        .iter()
        .flat_map(|c| re.find_iter(&c.text).map(move |m| (*c, m.start(), m.end())))
        .max_by_key(|(c, _, end)| NEXT_HEADING.find(&c.text[*end..]).map(|n| n.start()).unwrap_or(c.text.len() - end).min(1500))
}

fn citation_locator(s: &Source, pinpoint: Option<&str>) -> (LegalLocator, String) {
    if let Some(n) = pinpoint {
        if let Some((c, a, b)) = find_heading(s, n) {
            let end = c.text[b..].find('\n').map(|i| b + i).unwrap_or(c.text.len()).min(a + 300);
            let mut end_ok = end;
            while !c.text.is_char_boundary(end_ok) {
                end_ok -= 1;
            }
            return (locator(c, a, end_ok), "MATCH".into());
        }
    }
    let c = s.chunks[0];
    let mut end = c.text.len().min(160);
    while !c.text.is_char_boundary(end) {
        end -= 1;
    }
    (locator(c, 0, end), if pinpoint.is_some() { "MISMATCH".into() } else { "NO_PINPOINT".into() })
}

/// Whitespace-normalised exact search; returns original byte offsets in the chunk.
fn find_exact(chunk: &str, quote: &str) -> Option<(usize, usize)> {
    let q: Vec<&str> = quote.split_whitespace().collect();
    if q.is_empty() {
        return None;
    }
    let words: Vec<(usize, usize, &str)> = WORD.find_iter(chunk).map(|m| (m.start(), m.end(), m.as_str())).collect();
    (0..words.len().saturating_sub(q.len() - 1))
        .find(|&i| words[i..i + q.len()].iter().zip(&q).all(|(w, qw)| w.2 == *qw))
        .map(|i| (words[i].0, words[i + q.len() - 1].1))
}

fn lcs_table(a: &[&str], b: &[&str]) -> Vec<Vec<usize>> {
    let mut t = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            t[i][j] = if a[i] == b[j] { t[i + 1][j + 1] + 1 } else { t[i + 1][j].max(t[i][j + 1]) };
        }
    }
    t
}

/// Word diff from the source passage (a) to the quotation (b): DELETE = in source only, INSERT = in quote only.
fn word_diff(a: &[&str], b: &[&str]) -> Vec<DiffOp> {
    let t = lcs_table(a, b);
    let (mut i, mut j) = (0, 0);
    let mut ops: Vec<DiffOp> = Vec::new();
    let mut push = |op: &str, w: &str| match ops.last_mut() {
        Some(last) if last.op == op => {
            last.text.push(' ');
            last.text.push_str(w);
        }
        _ => ops.push(DiffOp { op: op.into(), text: w.into() }),
    };
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            push("EQUAL", a[i]);
            i += 1;
            j += 1;
        } else if t[i + 1][j] >= t[i][j + 1] {
            push("DELETE", a[i]);
            i += 1;
        } else {
            push("INSERT", b[j]);
            j += 1;
        }
    }
    while i < a.len() {
        push("DELETE", a[i]);
        i += 1;
    }
    while j < b.len() {
        push("INSERT", b[j]);
        j += 1;
    }
    ops
}

/// Passage of the source most similar to the quotation (same word count, best LCS).
fn nearest_passage(s: &Source, quote: &str) -> Option<Misquote> {
    let q: Vec<&str> = quote.split_whitespace().collect();
    let mut best: Option<(usize, &CorpusChunk, usize, usize)> = None;
    for c in &s.chunks {
        let words: Vec<(usize, usize, &str)> = WORD.find_iter(&c.text).map(|m| (m.start(), m.end(), m.as_str())).collect();
        if words.is_empty() {
            continue;
        }
        let n = q.len().min(words.len());
        for start in 0..=words.len() - n {
            let window: Vec<&str> = words[start..start + n].iter().map(|w| w.2).collect();
            // Rank by shared words, then by words in the same position (so a window shifted by a word loses).
            let score = lcs_table(&window, &q)[0][0] * 1000 + window.iter().zip(&q).filter(|(a, b)| a == b).count();
            if best.map(|b| score > b.0).unwrap_or(true) {
                best = Some((score, c, start, start + n));
            }
        }
    }
    let (_, c, ws, we) = best?;
    let words: Vec<(usize, usize, &str)> = WORD.find_iter(&c.text).map(|m| (m.start(), m.end(), m.as_str())).collect();
    let window: Vec<&str> = words[ws..we].iter().map(|w| w.2).collect();
    Some(Misquote { nearest: locator(c, words[ws].0, words[we - 1].1), diff_ops: word_diff(&window, &q) })
}

struct Candidate {
    start: usize,
    end: usize,
    rule: &'static str,
    citation_kind: &'static str,
    name: String,
    pinpoint: Option<String>,
}

fn overlaps(list: &[Candidate], s: usize, e: usize) -> bool {
    list.iter().any(|c| s < c.end && c.start < e)
}

fn extract_citations(text: &str, sources: &[Source]) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for c in SECTION_OF_ACT.captures_iter(text) {
        let m = c.get(0).unwrap();
        out.push(Candidate { start: m.start(), end: m.end(), rule: "statute.section_of_act", citation_kind: "STATUTE", name: c[2].to_string(), pinpoint: Some(c[1].to_string()) });
    }
    for c in ACT_SECTION.captures_iter(text) {
        let m = c.get(0).unwrap();
        if !overlaps(&out, m.start(), m.end()) {
            out.push(Candidate { start: m.start(), end: m.end(), rule: "statute.act_section", citation_kind: "STATUTE", name: c[1].to_string(), pinpoint: Some(c[2].to_string()) });
        }
    }
    for c in BARE_ACT.captures_iter(text) {
        let m = c.get(0).unwrap();
        if !overlaps(&out, m.start(), m.end()) {
            out.push(Candidate { start: m.start(), end: m.end(), rule: "statute.act", citation_kind: "STATUTE", name: c[1].to_string(), pinpoint: None });
        }
    }
    for m in REPORTER.find_iter(text) {
        if overlaps(&out, m.start(), m.end()) {
            continue;
        }
        let (end, pinpoint) = match AFTER_PINPOINT.captures(&text[m.end()..]) {
            Some(p) => (m.end() + p.get(0).unwrap().end(), Some(p[1].to_string())),
            None => (m.end(), None),
        };
        let kind = if m.as_str().contains("INSC") { "NEUTRAL_CITATION" } else if m.as_str().starts_with('(') || m.as_str().starts_with("AIR") || m.as_str().starts_with('[') { "CASE" } else { "REPORTER" };
        out.push(Candidate { start: m.start(), end, rule: "reporter", citation_kind: kind, name: m.as_str().to_string(), pinpoint });
    }
    // Corpus-driven: literal citation strings of installed sources ("Act No. 4 of 1893", "CF-MEMO-7", ...).
    for s in sources {
        let needle = s.citation.trim();
        if needle.chars().count() < 5 {
            continue;
        }
        let Ok(re) = Regex::new(&format!("(?i){}", regex::escape(needle))) else { continue };
        for m in re.find_iter(text) {
            if !overlaps(&out, m.start(), m.end()) {
                let kind = if s.authority_type == "STATUTE" { "STATUTE" } else { "OTHER" };
                out.push(Candidate { start: m.start(), end: m.end(), rule: "corpus.literal_citation", citation_kind: kind, name: m.as_str().to_string(), pinpoint: None });
            }
        }
    }
    out.sort_by_key(|c| c.start);
    out
}

fn resolve<'s, 'a>(cand: &Candidate, sources: &'s [Source<'a>]) -> Option<&'s Source<'a>> {
    let n = norm(&cand.name);
    if cand.citation_kind == "STATUTE" {
        let t = norm_title(&cand.name);
        sources.iter().find(|s| norm_title(&s.title) == t || (!n.is_empty() && norm(&s.citation) == n))
    } else {
        sources.iter().find(|s| {
            let sc = norm(&s.citation);
            !sc.is_empty() && (sc == n || sc.contains(&n) || n.contains(&sc))
        })
    }
}

fn hash_hex(text: &str) -> String {
    let mut h = Sha256::new();
    h.update(text.as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn audit(text: &str, corpus: &[CorpusChunk], filename: &str, format: &str, case_id: Option<String>) -> AuditReport {
    let mut packs: BTreeMap<String, PackSnapshot> = BTreeMap::new();
    for c in corpus {
        packs.entry(c.pack_id.clone()).or_insert_with(|| pack_snapshot(&c.pack_id, &c.metadata.version, &c.metadata.trust_level));
    }
    audit_core(text, group_sources(corpus), packs, filename, format, case_id)
}

fn pack_snapshot(pack_id: &str, version: &str, trust_level: &str) -> PackSnapshot {
    PackSnapshot {
        pack_id: pack_id.to_string(),
        version: version.to_string(),
        currency_date: None,
        trust_level: match trust_level {
            "OFFICIAL_VERIFIED" => "OFFICIAL_VERIFIED",
            "CUSTOM" => "CUSTOM",
            _ => "DEVELOPMENT_ONLY",
        }
        .into(),
    }
}

/// B-S4-29: the same audit over the on-disk store. Only the passages the draft needs are read: every passage of each
/// authority the draft cites, and the best keyword matches for each quotation (to find an unattributed quotation
/// anywhere in the installed law). Results equal `audit` over the whole corpus except that an unattributed quotation
/// is looked for among those best matches, not in every passage.
pub fn audit_with_store(store: &Store, small: &[CorpusChunk], text: &str, filename: &str, format: &str, case_id: Option<String>) -> Result<AuditReport, String> {
    struct G {
        rec: GroupRec,
        docs: Vec<DocRef>,
        members: Vec<usize>,
    }
    let mut arena: Vec<CorpusChunk> = small.to_vec();
    let mut groups: BTreeMap<(String, String, String), G> = BTreeMap::new();
    for v in store.group_views() {
        groups.insert((v.pack_id.clone(), v.rec.title.clone(), v.rec.citation.clone()), G { rec: v.rec.clone(), docs: v.docs.clone(), members: Vec::new() });
    }
    for (i, c) in small.iter().enumerate() {
        let g = groups
            .entry((c.pack_id.clone(), c.metadata.title.clone(), c.metadata.citation.clone()))
            .or_insert_with(|| G { rec: GroupRec::new(&c.metadata), docs: Vec::new(), members: Vec::new() });
        g.rec.absorb(&c.metadata);
        g.members.push(i);
    }
    let position: HashMap<(String, String, String), usize> = groups.keys().cloned().enumerate().map(|(i, k)| (k, i)).collect();
    let mut groups: Vec<G> = groups.into_values().collect();
    let light = |groups: &[G]| -> Vec<Source<'static>> {
        groups
            .iter()
            .map(|g| Source {
                source_id: g.rec.source_id.clone(),
                title: g.rec.title.clone(),
                citation: g.rec.citation.clone(),
                authority_type: g.rec.authority_type.clone(),
                trust_level: g.rec.trust_level.clone(),
                temporal_status: g.rec.temporal_status.clone(),
                superseded_status: g.rec.superseded_status.clone(),
                chunks: Vec::new(),
            })
            .collect()
    };
    let mut loaded: HashMap<DocRef, usize> = HashMap::new();
    // 1. Authorities the draft cites: all their passages.
    let sources = light(&groups);
    let mut cited: Vec<usize> = Vec::new();
    for cand in extract_citations(text, &sources) {
        if let Some(s) = resolve(&cand, &sources) {
            if let Some(i) = sources.iter().position(|x| x.source_id == s.source_id) {
                if !cited.contains(&i) {
                    cited.push(i);
                }
            }
        }
    }
    for i in cited {
        for r in groups[i].docs.clone() {
            if !loaded.contains_key(&r) {
                arena.push(store.load_doc(r)?);
                loaded.insert(r, arena.len() - 1);
                groups[i].members.push(arena.len() - 1);
            }
        }
    }
    // 2. Best keyword matches for each quotation.
    let mut quotes: Vec<String> = CURLY_QUOTE
        .captures_iter(text)
        .chain(STRAIGHT_QUOTE.captures_iter(text))
        .filter_map(|c| c.get(1).map(|m| text[m.start()..m.end()].trim().to_string()))
        .filter(|q| q.chars().count() >= MIN_QUOTE_CHARS)
        .collect();
    quotes.sort();
    quotes.dedup();
    for quote in quotes {
        for r in store.quote_candidates(&quote, 20) {
            if loaded.contains_key(&r) {
                continue;
            }
            let Some(vi) = store.group_of_doc(r) else { continue };
            let v = &store.group_views()[vi];
            let Some(&gi) = position.get(&(v.pack_id.clone(), v.rec.title.clone(), v.rec.citation.clone())) else { continue };
            arena.push(store.load_doc(r)?);
            loaded.insert(r, arena.len() - 1);
            groups[gi].members.push(arena.len() - 1);
        }
    }
    let sources: Vec<Source<'_>> = groups
        .iter()
        .map(|g| Source {
            source_id: g.rec.source_id.clone(),
            title: g.rec.title.clone(),
            citation: g.rec.citation.clone(),
            authority_type: g.rec.authority_type.clone(),
            trust_level: g.rec.trust_level.clone(),
            temporal_status: g.rec.temporal_status.clone(),
            superseded_status: g.rec.superseded_status.clone(),
            chunks: g.members.iter().map(|&i| &arena[i]).collect(),
        })
        .collect();
    let mut packs: BTreeMap<String, PackSnapshot> = store.pack_snapshots().into_iter().map(|(id, version, trust)| (id.clone(), pack_snapshot(&id, &version, &trust))).collect();
    for c in small {
        packs.entry(c.pack_id.clone()).or_insert_with(|| pack_snapshot(&c.pack_id, &c.metadata.version, &c.metadata.trust_level));
    }
    Ok(audit_core(text, sources, packs, filename, format, case_id))
}

fn audit_core(text: &str, sources: Vec<Source<'_>>, packs: BTreeMap<String, PackSnapshot>, filename: &str, format: &str, case_id: Option<String>) -> AuditReport {
    let mut items: Vec<AuditItem> = Vec::new();
    // Citations first so quotations can be attributed to them.
    let mut cite_index: Vec<(usize, usize, String, Option<usize>)> = Vec::new(); // (start, end, item_id, source index)
    for cand in extract_citations(text, &sources) {
        let id = format!("i{}", items.len() + 1);
        let src_idx = resolve(&cand, &sources).map(|s| sources.iter().position(|x| x.source_id == s.source_id).unwrap());
        let (result, resolved, pinpoint) = match src_idx {
            None => ("NOT_IN_CORPUS".to_string(), None, Pinpoint { claimed: cand.pinpoint.clone(), status: if cand.pinpoint.is_some() { "NOT_CHECKED".into() } else { "NO_PINPOINT".into() } }),
            Some(i) => {
                let (loc, status) = citation_locator(&sources[i], cand.pinpoint.as_deref());
                (classify(&sources[i]).to_string(), Some(loc), Pinpoint { claimed: cand.pinpoint.clone(), status })
            }
        };
        cite_index.push((cand.start, cand.end, id.clone(), src_idx));
        items.push(AuditItem {
            item_id: id,
            kind: "CITATION".into(),
            citation_kind: Some(cand.citation_kind.into()),
            extraction_rule_id: Some(cand.rule.into()),
            draft_char_start: char_index(text, cand.start),
            draft_char_end: char_index(text, cand.end),
            draft_text: text[cand.start..cand.end].to_string(),
            attributed_citation_item_id: None,
            result,
            resolved,
            pinpoint,
            notes: Vec::new(),
            misquote: None,
            proposition_ids: Vec::new(),
        });
    }

    let mut quotes: Vec<(usize, usize)> = CURLY_QUOTE
        .captures_iter(text)
        .chain(STRAIGHT_QUOTE.captures_iter(text))
        .filter_map(|c| c.get(1).map(|m| (m.start(), m.end())))
        .filter(|(a, b)| text[*a..*b].trim().chars().count() >= MIN_QUOTE_CHARS)
        .collect();
    quotes.sort();
    for (qs, qe) in quotes {
        let quote = text[qs..qe].trim();
        // Nearest citation before the quote (within 300 bytes), else just after it (within 120 bytes).
        let attributed = cite_index
            .iter()
            .filter(|c| c.1 <= qs && qs - c.1 <= 300)
            .max_by_key(|c| c.1)
            .or_else(|| cite_index.iter().filter(|c| c.0 >= qe && c.0 - qe <= 120).min_by_key(|c| c.0));
        let (result, resolved, misquote) = match attributed {
            Some((_, _, _, None)) => ("NOT_IN_CORPUS".to_string(), None, None),
            Some((_, _, _, Some(i))) => {
                let s = &sources[*i];
                match s.chunks.iter().find_map(|c| find_exact(&c.text, quote).map(|(a, b)| locator(c, a, b))) {
                    Some(loc) => (classify(s).to_string(), Some(loc), None),
                    None => match nearest_passage(s, quote) {
                        Some(mq) => ("MISQUOTED".to_string(), Some(mq.nearest.clone()), Some(mq)),
                        None => ("NOT_IN_CORPUS".to_string(), None, None),
                    },
                }
            }
            None => match sources.iter().find_map(|s| {
                s.chunks.iter().find_map(|c| find_exact(&c.text, quote).map(|(a, b)| (s, locator(c, a, b))))
            }) {
                Some((s, loc)) => (classify(s).to_string(), Some(loc), None),
                None => ("NOT_IN_CORPUS".to_string(), None, None),
            },
        };
        items.push(AuditItem {
            item_id: format!("i{}", items.len() + 1),
            kind: "QUOTATION".into(),
            citation_kind: None,
            extraction_rule_id: Some("quotation.marks".into()),
            draft_char_start: char_index(text, qs),
            draft_char_end: char_index(text, qe),
            draft_text: text[qs..qe].to_string(),
            attributed_citation_item_id: attributed.map(|c| c.2.clone()),
            result,
            resolved,
            pinpoint: Pinpoint { claimed: None, status: "NO_PINPOINT".into() },
            notes: Vec::new(),
            misquote,
            proposition_ids: Vec::new(),
        });
    }
    items.sort_by_key(|i| i.draft_char_start);

    let mut counts: BTreeMap<String, usize> = RESULTS.iter().map(|r| (r.to_string(), 0)).collect();
    for i in &items {
        *counts.entry(i.result.clone()).or_insert(0) += 1;
    }
    let sha = hash_hex(text);
    AuditReport {
        schema_version: SCHEMA_VERSION.into(),
        audit_id: format!("audit-{}", &sha[..16]),
        pipeline_id: PIPELINE_ID.into(),
        generated_at: crate::docpipe::now_rfc3339(),
        case_id,
        subject: Subject { subject_kind: "OWN_DRAFT".into(), doc_id: None, filing_party_id: None, sha256: sha, filename: filename.into(), format: format.into() },
        pack_snapshot: packs.into_values().collect(),
        model_fingerprint: None,
        items,
        counts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval::LegalMetadata;
    use serde_json::Value;

    const FIXTURE: &str = include_str!("../../golden/G-AUDIT/fixtures/audit_core.json");

    fn chunk(v: &Value) -> CorpusChunk {
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        CorpusChunk {
            chunk_id: s("chunk_id"),
            pack_id: s("pack_id"),
            path: String::new(),
            jurisdiction: None,
            text: s("text"),
            authority_hint: s("title"),
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
    }

    fn check(id: &str, text: &str, report: &AuditReport, expected: &[Value]) {
        for e in expected {
            let m = e["match"].as_str().unwrap();
            let pos = text.find(m).unwrap_or_else(|| panic!("{id}: match text '{m}' not in draft"));
            let (ms, me) = (char_index(text, pos), char_index(text, pos + m.len()));
            let kind = e["kind"].as_str().unwrap();
            let item = report
                .items
                .iter()
                .find(|i| i.kind == kind && i.draft_char_start < me && ms < i.draft_char_end)
                .unwrap_or_else(|| panic!("{id}: no {kind} item covering '{m}' in {:#?}", report.items));
            assert_eq!(item.result, e["result"].as_str().unwrap(), "{id}: '{m}' -> {item:#?}");
            if let Some(p) = e.get("pinpoint").and_then(Value::as_str) {
                assert_eq!(item.pinpoint.status, p, "{id}: pinpoint for '{m}' -> {item:#?}");
            }
            if let Some(del) = e.get("diff_delete").and_then(Value::as_str) {
                let mq = item.misquote.as_ref().expect("misquote diff");
                assert!(mq.diff_ops.iter().any(|o| o.op == "DELETE" && o.text.contains(del)), "{id}: diff {:?}", mq.diff_ops);
                let ins = e["diff_insert"].as_str().unwrap();
                assert!(mq.diff_ops.iter().any(|o| o.op == "INSERT" && o.text.contains(ins)), "{id}: diff {:?}", mq.diff_ops);
            }
            if item.result == "NOT_IN_CORPUS" {
                assert!(item.resolved.is_none(), "{id}: NOT_IN_CORPUS must not carry a resolved source");
            } else {
                assert!(item.resolved.is_some(), "{id}: {} must carry a resolved source", item.result);
            }
        }
        let total: usize = report.counts.values().sum();
        assert_eq!(total, report.items.len(), "{id}: counts must add up");
    }

    #[test]
    fn golden_synthetic_drafts() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let corpus: Vec<CorpusChunk> = fx["corpus"].as_array().unwrap().iter().map(chunk).collect();
        let mut planted = 0usize;
        let mut detected = 0usize;
        for d in fx["drafts"].as_array().unwrap() {
            let id = d["draft_id"].as_str().unwrap();
            let text = d["text"].as_str().unwrap();
            let report = audit(text, &corpus, "fixture.txt", "txt", None);
            check(id, text, &report, d["expected"].as_array().unwrap());
            planted += d["planted_citations"].as_u64().unwrap() as usize;
            detected += report.items.iter().filter(|i| i.kind == "CITATION").count();
            // AC-S4-01.1: nothing fabricated can ever be VERIFIED.
            for i in &report.items {
                if i.draft_text.contains("2099") || i.draft_text.contains("999 U.S.") || i.draft_text.contains("metes and bounds") {
                    assert_eq!(i.result, "NOT_IN_CORPUS", "{id}: fabricated item {i:#?}");
                }
            }
        }
        assert_eq!(detected, planted, "citation extraction must find every planted citation");
    }

    #[test]
    fn report_matches_frozen_contract_shape() {
        let contract: Value = serde_json::from_str(include_str!("../../contracts/audit_report.schema.json")).unwrap();
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let corpus: Vec<CorpusChunk> = fx["corpus"].as_array().unwrap().iter().map(chunk).collect();
        let report = audit(fx["drafts"][0]["text"].as_str().unwrap(), &corpus, "fixture.txt", "txt", None);
        let v = serde_json::to_value(&report).unwrap();
        let keys = |path: &Value| -> Vec<String> { path.as_array().unwrap().iter().map(|k| k.as_str().unwrap().to_string()).collect() };
        for k in keys(&contract["required"]) {
            assert!(v.get(&k).is_some(), "report missing required key {k}");
        }
        for k in keys(&contract["properties"]["counts"]["required"]) {
            assert!(v["counts"].get(&k).is_some(), "counts missing {k}");
        }
        assert_eq!(v["pipeline_id"], contract["properties"]["pipeline_id"]["const"]);
        let item_required = keys(&contract["$defs"]["Item"]["required"]);
        for item in v["items"].as_array().unwrap() {
            for k in &item_required {
                assert!(item.get(k).is_some(), "item missing required key {k}: {item}");
            }
            let result = item["result"].as_str().unwrap();
            assert_eq!(item["resolved"].is_null(), result == "NOT_IN_CORPUS", "resolved/result mismatch: {item}");
            assert_eq!(item["misquote"].is_null(), result != "MISQUOTED", "misquote/result mismatch: {item}");
        }
    }

    /// B-S4-29: the store-backed audit reports exactly what the whole-corpus audit reports.
    #[test]
    fn store_backed_audit_equals_whole_corpus_audit_on_the_real_pack() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let rp = &fx["real_pack"];
        let pack = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rp["pack_dir"].as_str().unwrap());
        let vault = tempfile::tempdir().unwrap();
        let paths = crate::vault::VaultPaths::new(vault.path().to_path_buf());
        crate::packs::install_pack_from_dir(&paths, &pack).expect("official pack installs");
        let corpus = crate::retrieval::build_corpus(&paths, &[]).unwrap();
        let store = crate::retrieval::store::open(&paths).unwrap();
        // The golden draft (citations, pinpoints, quotations) plus a quotation with no attributed citation, an altered
        // quotation, and a reference to an authority that is not installed.
        let passage = corpus.iter().map(|c| c.text.as_str()).find(|t| t.chars().count() > 200).unwrap();
        let quote: String = passage.chars().skip(60).take(90).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
        let altered = quote.replacen(' ', " certainly ", 1);
        let drafts = [
            rp["draft"].as_str().unwrap().to_string(),
            format!("The rule is stated plainly: \u{201c}{quote}\u{201d} and counsel adds \u{201c}{altered}\u{201d}. See also Section 99 of the Imaginary Act, 2099."),
        ];
        for text in drafts {
            let whole = serde_json::to_value(audit(&text, &corpus, "x.txt", "txt", None)).unwrap();
            let stored = serde_json::to_value(audit_with_store(&store, &[], &text, "x.txt", "txt", None).unwrap()).unwrap();
            let strip = |mut v: Value| {
                v["generated_at"] = Value::Null;
                v
            };
            assert_eq!(strip(whole), strip(stored), "draft: {text}");
        }
    }

    #[test]
    fn group_records_match_group_sources() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let corpus: Vec<CorpusChunk> = fx["corpus"].as_array().unwrap().iter().map(chunk).collect();
        let mut by_key: BTreeMap<(String, String, String), GroupRec> = BTreeMap::new();
        for c in &corpus {
            let g = by_key.entry((c.pack_id.clone(), c.metadata.title.clone(), c.metadata.citation.clone())).or_insert_with(|| GroupRec::new(&c.metadata));
            g.absorb(&c.metadata);
        }
        let sources = group_sources(&corpus);
        assert_eq!(sources.len(), by_key.len());
        for (s, g) in sources.iter().zip(by_key.values()) {
            assert_eq!((&s.source_id, &s.title, &s.citation, &s.authority_type), (&g.source_id, &g.title, &g.citation, &g.authority_type));
            assert_eq!((&s.trust_level, &s.temporal_status, &s.superseded_status), (&g.trust_level, &g.temporal_status, &g.superseded_status));
        }
    }

    #[test]
    fn golden_real_india_partition_pack() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let rp = &fx["real_pack"];
        let pack = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join(rp["pack_dir"].as_str().unwrap());
        let chunks: Value = serde_json::from_str(&std::fs::read_to_string(pack.join("metadata/legal_chunks.json")).unwrap()).unwrap();
        let list = chunks.get("chunks").unwrap_or(&chunks).as_array().unwrap().clone();
        let corpus: Vec<CorpusChunk> = list
            .iter()
            .map(|c| {
                let mut v = c.clone();
                v["text"] = c["exact_text"].clone();
                v["pack_id"] = c["pack_id"].clone();
                chunk(&v)
            })
            .collect();
        let text = rp["draft"].as_str().unwrap();
        let report = audit(text, &corpus, "real.txt", "txt", None);
        check("REAL", text, &report, rp["expected"].as_array().unwrap());
    }
}
