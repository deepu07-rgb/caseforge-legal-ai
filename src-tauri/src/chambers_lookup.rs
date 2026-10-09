//! Record lookup for the Chambers Edition (B-S4-19, PRD v4.0 §9.3).
//!
//! "Where in the record ...?" is answered with passages of the record and nothing else: ranked verbatim excerpts, each with
//! its document, page, paragraph label and character range, and the words that matched marked inside the excerpt. No model
//! is involved and no sentence is written: a hit is a slice of a document on the record. The index is built from the
//! matter's own documents only (filings, evidence, orders) - never from installed law or anything outside the matter.
//!
//! Passages are numbered pleading paragraphs ("p4"), or blocks separated by a blank line ("b2") on pages without numbered
//! paragraphs; a very long passage is cut into sentence windows ("p4.2"). Ranking is BM25 over lightly stemmed words, with
//! a small vocabulary of near-synonyms (packs/builtin/record_synonyms.v1.json, data not code) counting at half weight, a
//! bonus for adjacent question words appearing together, and a floor on how many of the question's words must appear, so
//! a question the record cannot answer returns nothing instead of the nearest noise.
//! Golden: golden/G-CHAMBERS (record_lookup).

use crate::opposition;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

pub const SYNONYMS_JSON: &str = include_str!("../../packs/builtin/record_synonyms.v1.json");
const K1: f64 = 1.2;
const B: f64 = 0.75;
const SYNONYM_WEIGHT: f64 = 0.5;
const ADJACENT_BONUS: f64 = 0.35;
/// Fraction of the question's distinct words that must appear in a passage for it to be returned.
const MIN_COVERAGE: f64 = 0.4;
const LONG_PASSAGE: usize = 1500;
const WINDOW: usize = 1000;
pub const DEFAULT_K: usize = 5;
pub const MAX_K: usize = 10;

static WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"[A-Za-z0-9]+").unwrap());
static BLANK: Lazy<Regex> = Lazy::new(|| Regex::new(r"\r?\n[ \t]*(?:\r?\n[ \t]*)+").unwrap());
static SENTENCE_END: Lazy<Regex> = Lazy::new(|| Regex::new(r"[.!?]\s+").unwrap());
const STOP: &[&str] = &[
    "the", "a", "an", "and", "or", "of", "to", "in", "on", "for", "from", "by", "is", "are", "was", "were", "be", "been", "being", "what", "when", "where", "which", "who", "whom", "whose", "how",
    "why", "does", "do", "did", "this", "that", "these", "those", "with", "into", "any", "it", "its", "as", "at", "if", "not", "has", "have", "had", "there", "their", "he", "she", "his", "her", "they",
    "them", "i", "me", "my", "we", "our", "you", "your", "about", "than", "then", "so", "but", "also", "can", "could", "would", "should", "will", "shall",
];

/// Lightly stemmed lower-case form of a word; the same function reads the record and the question.
pub fn stem(word: &str) -> String {
    let mut s = word.to_ascii_lowercase();
    if s.len() > 4 && s.ends_with("ies") {
        s.truncate(s.len() - 3);
        s.push('y');
    } else if s.len() > 3 && s.ends_with('s') && !s.ends_with("ss") {
        s.pop();
    }
    if s.len() > 5 && s.ends_with("ing") {
        s.truncate(s.len() - 3);
    } else if s.len() > 4 && s.ends_with("ed") {
        s.truncate(s.len() - 2);
    }
    if s.len() > 3 && s.ends_with('e') {
        s.pop();
    }
    s
}

/// Near-synonym groups, stemmed: a stem maps to the other stems in its groups.
pub struct Synonyms(HashMap<String, Vec<String>>);

impl Synonyms {
    pub fn from_json(raw: &str) -> Synonyms {
        let v: Value = serde_json::from_str(raw).unwrap_or(Value::Null);
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for g in v["groups"].as_array().into_iter().flatten() {
            let stems: Vec<String> = g.as_array().into_iter().flatten().filter_map(Value::as_str).map(stem).collect();
            for s in &stems {
                let e = map.entry(s.clone()).or_default();
                for o in &stems {
                    if o != s && !e.contains(o) {
                        e.push(o.clone());
                    }
                }
            }
        }
        Synonyms(map)
    }
    pub fn builtin() -> &'static Synonyms {
        static S: Lazy<Synonyms> = Lazy::new(|| Synonyms::from_json(SYNONYMS_JSON));
        &S
    }
    fn of(&self, stem: &str) -> &[String] {
        self.0.get(stem).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// A document of the matter as page texts.
pub struct DocPages {
    pub doc_id: String,
    pub filename: String,
    pub pages: Vec<String>,
}

pub struct Passage {
    pub doc: usize,
    /// 1-based.
    pub page: usize,
    pub label: String,
    /// Character range inside the page text.
    pub char_start: usize,
    pub char_end: usize,
    pub text: String,
    tokens: Vec<(String, usize, usize)>, // stem, char start, char end within `text`
}

pub struct Index {
    docs: Vec<(String, String)>,
    pub passages: Vec<Passage>,
    df: HashMap<String, usize>,
    avg_len: f64,
}

fn char_at(text: &str, byte: usize) -> usize {
    text[..byte].chars().count()
}

/// (label, char_start, char_end) of the passages of one page.
fn split_page(page: &str) -> Vec<(String, usize, usize)> {
    let paras = opposition::paragraphs(std::slice::from_ref(&page.to_string()));
    let mut units: Vec<(String, usize, usize)> = Vec::new();
    if !paras.is_empty() {
        let first = paras[0].char_start;
        if page.chars().take(first).any(|c| !c.is_whitespace()) {
            units.push(("b1".into(), 0, first));
        }
        units.extend(paras.iter().map(|p| (format!("p{}", p.id.trim_start_matches('p')), p.char_start, p.char_end)));
    } else {
        let mut start = 0usize;
        let mut k = 0;
        let mut spans: Vec<(usize, usize)> = BLANK.find_iter(page).map(|m| (m.start(), m.end())).collect();
        spans.push((page.len(), page.len()));
        for (s, e) in spans {
            if page[start..s].trim().is_empty() {
                start = e;
                continue;
            }
            k += 1;
            units.push((format!("b{k}"), char_at(page, start), char_at(page, s)));
            start = e;
        }
    }
    // cut a very long passage into sentence windows
    let chars: Vec<char> = page.chars().collect();
    let mut out = Vec::new();
    for (label, s, e) in units {
        // trim surrounding whitespace so the excerpt starts and ends on text
        let (mut s, mut e) = (s, e.min(chars.len()));
        while s < e && chars[s].is_whitespace() {
            s += 1;
        }
        while e > s && chars[e - 1].is_whitespace() {
            e -= 1;
        }
        if e <= s {
            continue;
        }
        if e - s <= LONG_PASSAGE {
            out.push((label, s, e));
            continue;
        }
        let text: String = chars[s..e].iter().collect();
        let mut cuts = vec![0usize];
        let mut last = 0usize;
        for m in SENTENCE_END.find_iter(&text) {
            let at = char_at(&text, m.end());
            if at - last >= WINDOW {
                cuts.push(at);
                last = at;
            }
        }
        cuts.push(e - s);
        cuts.dedup();
        for (k, w) in cuts.windows(2).enumerate() {
            let (a, b) = (s + w[0], s + w[1]);
            if b > a {
                out.push((format!("{label}.{}", k + 1), a, b));
            }
        }
    }
    out
}

impl Index {
    pub fn build(docs: &[DocPages]) -> Index {
        let mut passages = Vec::new();
        for (di, d) in docs.iter().enumerate() {
            for (pi, page) in d.pages.iter().enumerate() {
                let chars: Vec<char> = page.chars().collect();
                for (label, s, e) in split_page(page) {
                    let text: String = chars[s..e].iter().collect();
                    let tokens = WORD.find_iter(&text).map(|m| (stem(m.as_str()), char_at(&text, m.start()), char_at(&text, m.end()))).collect();
                    passages.push(Passage { doc: di, page: pi + 1, label, char_start: s, char_end: e, text, tokens });
                }
            }
        }
        let mut df: HashMap<String, usize> = HashMap::new();
        let mut total = 0usize;
        for p in &passages {
            total += p.tokens.len();
            for t in p.tokens.iter().map(|t| t.0.as_str()).collect::<HashSet<_>>() {
                *df.entry(t.to_string()).or_default() += 1;
            }
        }
        let avg_len = (total as f64 / passages.len().max(1) as f64).max(1.0);
        Index { docs: docs.iter().map(|d| (d.doc_id.clone(), d.filename.clone())).collect(), passages, df, avg_len }
    }

    fn idf(&self, stem: &str) -> f64 {
        let n = self.passages.len() as f64;
        let d = *self.df.get(stem).unwrap_or(&0) as f64;
        (1.0 + (n - d + 0.5) / (d + 0.5)).ln()
    }
}

/// One passage of the record that answers the question. Every field is stored text or a position in it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Hit {
    pub rank: usize,
    pub score: f64,
    pub doc_id: String,
    pub filename: String,
    pub page: usize,
    pub label: String,
    pub char_start: usize,
    pub char_end: usize,
    /// The passage, verbatim.
    pub excerpt: String,
    /// Character ranges inside `excerpt` of the words that matched the question.
    pub matched: Vec<(usize, usize)>,
}

/// The question's distinct content words, stemmed, in order.
pub fn question_terms(question: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    WORD.find_iter(question)
        .map(|m| m.as_str().to_ascii_lowercase())
        .filter(|w| !STOP.contains(&w.as_str()) && (w.len() >= 2 || w.chars().all(|c| c.is_ascii_digit())))
        .map(|w| stem(&w))
        .filter(|s| seen.insert(s.clone()))
        .collect()
}

pub fn lookup(index: &Index, syn: &Synonyms, question: &str, k: usize) -> Vec<Hit> {
    let terms = question_terms(question);
    if terms.is_empty() || index.passages.is_empty() {
        return vec![];
    }
    let need = ((terms.len() as f64) * MIN_COVERAGE).ceil() as usize;
    let sat = |tf: f64, len: usize| tf * (K1 + 1.0) / (tf + K1 * (1.0 - B + B * len as f64 / index.avg_len));
    let mut scored: Vec<(usize, f64, HashSet<String>)> = Vec::new();
    for (pi, p) in index.passages.iter().enumerate() {
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for (t, _, _) in &p.tokens {
            *counts.entry(t.as_str()).or_default() += 1;
        }
        let (mut score, mut covered) = (0.0, 0usize);
        let mut matched: HashSet<String> = HashSet::new();
        for t in &terms {
            let exact = *counts.get(t.as_str()).unwrap_or(&0);
            let mut s = if exact > 0 { index.idf(t) * sat(exact as f64, p.tokens.len()) } else { 0.0 };
            let mut hit = exact > 0;
            if exact > 0 {
                matched.insert(t.clone());
            }
            let mut best = 0.0f64;
            let mut best_term: Option<&String> = None;
            for e in syn.of(t) {
                if let Some(&c) = counts.get(e.as_str()) {
                    let v = SYNONYM_WEIGHT * index.idf(e) * sat(c as f64, p.tokens.len());
                    if v > best {
                        best = v;
                        best_term = Some(e);
                    }
                }
            }
            if best > 0.0 {
                hit = true;
                s += best;
                if let Some(e) = best_term {
                    matched.insert(e.clone());
                }
            }
            if hit {
                covered += 1;
                score += s;
            }
        }
        if covered < need.max(1) {
            continue;
        }
        // adjacent question words appearing together
        let stems: Vec<&str> = p.tokens.iter().map(|t| t.0.as_str()).collect();
        for w in terms.windows(2) {
            if stems.windows(2).any(|x| x[0] == w[0] && x[1] == w[1]) {
                score += ADJACENT_BONUS * (index.idf(&w[0]) + index.idf(&w[1])) / 2.0;
            }
        }
        if score > 0.0 {
            scored.push((pi, score, matched));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
    scored
        .into_iter()
        .take(k.clamp(1, MAX_K))
        .enumerate()
        .map(|(i, (pi, score, matched))| {
            let p = &index.passages[pi];
            let (doc_id, filename) = &index.docs[p.doc];
            Hit {
                rank: i + 1,
                score: (score * 1000.0).round() / 1000.0,
                doc_id: doc_id.clone(),
                filename: filename.clone(),
                page: p.page,
                label: p.label.clone(),
                char_start: p.char_start,
                char_end: p.char_end,
                excerpt: p.text.clone(),
                matched: p.tokens.iter().filter(|(t, _, _)| matched.contains(t)).map(|(_, s, e)| (*s, *e)).collect(),
            }
        })
        .collect()
}

static CACHE: Lazy<Mutex<HashMap<String, Arc<Index>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// The index for a matter's documents, rebuilt when a document is added, removed or changed. `key` names the exact set.
pub fn cached(key: &str, build: impl FnOnce() -> Result<Vec<DocPages>, String>) -> Result<Arc<Index>, String> {
    if let Some(i) = CACHE.lock().ok().and_then(|m| m.get(key).cloned()) {
        return Ok(i);
    }
    let index = Arc::new(Index::build(&build()?));
    if let Ok(mut m) = CACHE.lock() {
        if m.len() >= 8 {
            m.clear();
        }
        m.insert(key.to_string(), index.clone());
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../golden/G-CHAMBERS/fixtures/record_lookup_core.json");
    const FIXTURE_SHA256: &str = "b8655d2333ef8343e23df0b0176e7fd5f17ec086c0ef78fd8d921547c7d90cdf"; // committed before the engine (2c300d5)

    fn docs(fx: &Value) -> Vec<DocPages> {
        fx["documents"].as_array().unwrap().iter().map(|d| DocPages { doc_id: d["doc_id"].as_str().unwrap().into(), filename: d["filename"].as_str().unwrap().into(), pages: d["pages"].as_array().unwrap().iter().map(|p| p.as_str().unwrap().to_string()).collect() }).collect()
    }

    fn load() -> (Value, Vec<DocPages>, Index) {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "record-lookup golden changed since it was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let d = docs(&fx);
        let idx = Index::build(&d);
        (fx, d, idx)
    }

    #[test]
    fn golden_top_five_contains_the_answering_passage() {
        let (fx, _, idx) = load();
        let syn = Synonyms::builtin();
        let (mut hits, mut misses) = (0usize, Vec::new());
        let queries = fx["queries"].as_array().unwrap();
        for q in queries {
            let got = lookup(&idx, syn, q["q"].as_str().unwrap(), DEFAULT_K);
            let ok = q["expect"].as_array().unwrap().iter().any(|e| got.iter().any(|h| h.doc_id == e["doc_id"].as_str().unwrap() && h.page as u64 == e["page"].as_u64().unwrap() && h.label == e["label"].as_str().unwrap()));
            if ok {
                hits += 1;
            } else {
                misses.push(format!("{}: {} -> {:?}", q["id"], q["q"], got.iter().map(|h| format!("{}:{}:{}", h.doc_id, h.page, h.label)).collect::<Vec<_>>()));
            }
        }
        let rate = hits as f64 / queries.len() as f64;
        eprintln!("record lookup top-5 hit rate {hits}/{} = {rate:.3}; misses {misses:?}", queries.len());
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::PathBuf::from(dir).join("record_lookup.json"), serde_json::to_vec_pretty(&serde_json::json!({"queries": queries.len(), "hits": hits, "rate": rate, "misses": misses})).unwrap()).unwrap();
        }
        assert!(rate >= fx["threshold"]["proposed"].as_f64().unwrap(), "AC-S4-13.1: {rate:.3} below the proposed threshold; misses {misses:?}");
    }

    #[test]
    fn a_question_the_record_cannot_answer_returns_nothing() {
        let (fx, _, idx) = load();
        for q in fx["no_answer"].as_array().unwrap() {
            assert!(lookup(&idx, Synonyms::builtin(), q["q"].as_str().unwrap(), DEFAULT_K).is_empty(), "{}", q["q"]);
        }
        assert!(lookup(&idx, Synonyms::builtin(), "   ", DEFAULT_K).is_empty());
        assert!(lookup(&idx, Synonyms::builtin(), "the of and", DEFAULT_K).is_empty(), "only filler words");
    }

    #[test]
    fn every_hit_is_a_verbatim_slice_of_the_record_with_a_locator_and_nothing_else() {
        // AC-S4-13.2: zero generated prose. The excerpt is the document text at the stated place, character for character.
        let (fx, d, idx) = load();
        let mut total = 0;
        for q in fx["queries"].as_array().unwrap() {
            for h in lookup(&idx, Synonyms::builtin(), q["q"].as_str().unwrap(), MAX_K) {
                total += 1;
                let doc = d.iter().find(|x| x.doc_id == h.doc_id).unwrap();
                let page: Vec<char> = doc.pages[h.page - 1].chars().collect();
                assert_eq!(page[h.char_start..h.char_end].iter().collect::<String>(), h.excerpt, "{}: excerpt is the record text at its locator", q["id"]);
                assert!(!h.label.is_empty() && h.page >= 1 && h.rank >= 1);
                let ex: Vec<char> = h.excerpt.chars().collect();
                assert!(h.matched.iter().all(|(s, e)| s < e && *e <= ex.len()), "highlights lie inside the excerpt");
                assert!(!h.matched.is_empty(), "a hit shows which words matched");
            }
        }
        assert!(total > 50);
        // the serialised hit has exactly these fields: stored text, positions and numbers
        let h = lookup(&idx, Synonyms::builtin(), "who changed the lock", 1).remove(0);
        let keys: Vec<String> = serde_json::to_value(&h).unwrap().as_object().unwrap().keys().cloned().collect();
        let mut want = vec!["rank", "score", "doc_id", "filename", "page", "label", "char_start", "char_end", "excerpt", "matched"];
        want.sort();
        let mut got = keys;
        got.sort();
        assert_eq!(got, want);
    }

    #[test]
    fn passages_are_pleading_paragraphs_or_blank_line_blocks() {
        let (_, _, idx) = load();
        let labels = |doc: &str, page: usize| idx.passages.iter().filter(|p| idx.docs[p.doc].0 == doc && p.page == page).map(|p| p.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels("d-plaint", 1), ["p1", "p2", "p3"]);
        assert_eq!(labels("d-plaint", 2), ["p4", "p5"]);
        assert_eq!(labels("d-pw1", 1), ["b1", "b2", "b3", "b4"]);
        assert_eq!(labels("d-orders", 1), ["b1", "b2", "b3"]);
    }

    #[test]
    fn a_very_long_passage_is_cut_at_sentence_ends_and_nothing_is_lost() {
        let sentence = "The witness described the events of that evening in some detail and did not waver. ";
        let page = sentence.repeat(40);
        let d = vec![DocPages { doc_id: "d".into(), filename: "f".into(), pages: vec![page.clone()] }];
        let idx = Index::build(&d);
        assert!(idx.passages.len() >= 3 && idx.passages.iter().all(|p| p.label.starts_with("b1.") && p.text.chars().count() <= LONG_PASSAGE));
        let chars: Vec<char> = page.chars().collect();
        for p in &idx.passages {
            assert_eq!(chars[p.char_start..p.char_end].iter().collect::<String>(), p.text);
        }
        assert_eq!(idx.passages[0].char_start, 0);
        assert!(idx.passages.windows(2).all(|w| w[0].char_end <= w[1].char_start + 1));
    }

    #[test]
    fn lookup_is_deterministic_and_synonyms_count_for_less_than_the_word_itself() {
        let (_, _, idx) = load();
        let syn = Synonyms::builtin();
        let a = lookup(&idx, syn, "red car", 5);
        assert_eq!(a, lookup(&idx, syn, "red car", 5));
        // "vehicle" is a near-synonym of "car": the passage with the red car is found, but ranked by the synonym's half weight
        let v = lookup(&idx, syn, "vehicle", 5);
        assert!(v.iter().any(|h| h.label == "b3" && h.doc_id == "d-pw1"));
        let exact = lookup(&idx, syn, "car", 1)[0].score;
        assert!(v[0].score < exact);
        assert_eq!(stem("registered"), stem("register"));
        assert_eq!(stem("houses"), stem("house"));
        assert_eq!(stem("parties"), "party");
    }

    #[test]
    fn the_engine_writes_no_prose() {
        let src = include_str!("chambers_lookup.rs").split("#[cfg(test)]").next().unwrap();
        for t in ["infer_llm", "ollama", "chat_with_timeout", "llm_chat", "reqwest", "http://", "https://"] {
            assert!(!src.contains(t), "the lookup engine must not contain {t}");
        }
    }
}
