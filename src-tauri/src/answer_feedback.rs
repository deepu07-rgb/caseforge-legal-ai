//! Per-answer feedback in Mock Court (B-S4-00b).
//!
//! Counsel's text in each Mock Court turn is their answer to the previous challenge. Before the next challenge is
//! generated, the host checks that answer against the matter record and the counsel-confirmed working set:
//! dates and amounts (supported / conflicting / not found), authorities (in the working set or not), relevant record
//! contradictions and authorities the answer left unused, and whether the answer engages the challenge at all.
//!
//! Deterministic only: no model call, no score, no "right/wrong", no outcome language (G1, G6).
//! Contract: contracts/answer_feedback.schema.json. Golden: golden/G-ANSWER-FEEDBACK.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

pub const SCHEMA_VERSION: &str = "1.0.0";
pub const ENGINE_VERSION: &str = "caseforge.answer-feedback.v1";
/// Share of the challenge's specific terms an answer must engage to count as ADDRESSED (display heuristic only).
const ADDRESSED_RATIO: f64 = 0.4;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeedbackItem {
    pub kind: String,
    pub status: String,
    pub value: Option<String>,
    pub message: String,
    pub record_ref: Option<String>,
    pub locator: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnswerFeedback {
    pub schema_version: String,
    pub engine_version: String,
    pub session_id: String,
    pub engagement: String,
    pub engagement_terms_matched: usize,
    pub engagement_terms_total: usize,
    pub items: Vec<FeedbackItem>,
    pub generated_by: String,
    pub model_used: bool,
    pub outcome_prediction: bool,
}

/// An authority in the counsel-confirmed working set for this turn.
#[derive(Debug, Clone)]
pub struct PermittedAuthority {
    pub authority_id: String,
    pub citation: String,
    pub label: String,
}

/// Last rendered challenge per Mock Court session (host memory only, like the turn history).
static LAST_CHALLENGE: Lazy<Mutex<HashMap<String, String>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn remember_challenge(session_id: &str, challenge: &str) {
    if let Ok(mut map) = LAST_CHALLENGE.lock() {
        map.insert(session_id.to_string(), challenge.to_string());
    }
}

pub fn previous_challenge(session_id: &str) -> Option<String> {
    LAST_CHALLENGE.lock().ok().and_then(|map| map.get(session_id).cloned())
}

const MONTHS: [(&str, u32); 21] = [
    ("january", 1), ("february", 2), ("march", 3), ("april", 4), ("may", 5), ("june", 6), ("july", 7),
    ("august", 8), ("september", 9), ("october", 10), ("november", 11), ("december", 12),
    ("jan", 1), ("feb", 2), ("mar", 3), ("apr", 4), ("jun", 6), ("jul", 7), ("aug", 8), ("sep", 9), ("sept", 9),
];
const MONTH_ALT: &str = "january|february|march|april|may|june|july|august|september|october|november|december|sept|jan|feb|mar|apr|jun|jul|aug|sep|oct|nov|dec";

static DAY_MONTH_YEAR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"(?i)\b(\d{{1,2}})(?:st|nd|rd|th)?\s+({MONTH_ALT})\.?,?\s+(\d{{4}})\b")).unwrap()
});
static MONTH_DAY_YEAR: Lazy<Regex> = Lazy::new(|| {
    Regex::new(&format!(r"(?i)\b({MONTH_ALT})\.?\s+(\d{{1,2}})(?:st|nd|rd|th)?,?\s+(\d{{4}})\b")).unwrap()
});
static ISO_DATE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").unwrap());
static AMOUNT: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)(?:\brs\.?|\binr|₹|\$|\busd)\s*([0-9][0-9,]*)(?:\.[0-9]+)?").unwrap());
static REPORTER_CITATION: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?x)
          \(\d{4}\)\s*\d{1,3}\s*SCC\s+(?:OnLine\s+[A-Za-z.]+\s+)?\d{1,5}
        | \d{4}\s+SCC\s+OnLine\s+[A-Za-z.]+\s+\d{1,6}
        | AIR\s+\d{4}\s+[A-Za-z.]{2,12}\s+\d{1,5}
        | \d{4}\s+INSC\s+\d{1,5}
        | \d{1,4}\s+(?:U\.\s?S\.|S\.\s?Ct\.|F\.\s?(?:2d|3d|4th)|F\.\s?Supp\.)\s+\d{1,5}",
    )
    .unwrap()
});
static STATUTE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\b[A-Z][A-Za-z]+(?:\s+[A-Z][A-Za-z]+)*\s+Act,?\s+\d{4}(?:,?\s*(?:s\.|sec\.|section)\s*\d+[A-Z]?)?").unwrap()
});
static WORD: Lazy<Regex> = Lazy::new(|| Regex::new(r"[a-z]+").unwrap());

/// Words too generic to show that an answer engages a specific challenge.
const STOP: &[&str] = &[
    "that", "this", "with", "from", "have", "were", "which", "their", "there", "been", "also", "will", "would", "shall",
    "about", "into", "upon", "under", "your", "what", "when", "where", "does", "answer", "court", "counsel", "record",
    "says", "said", "show", "shows", "case", "matter", "argue", "submit", "appellant", "respondent", "plaintiff",
    "defendant", "client", "party", "parties", "against", "only", "they", "them", "then", "than", "such", "made",
    "make", "over", "more", "most", "some", "very", "just", "like", "these", "those", "must", "should", "could",
    "here", "being", "after", "before", "because", "while", "other", "each", "every", "both", "either", "neither",
];

fn month_number(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    MONTHS.iter().find(|(m, _)| *m == lower).map(|(_, n)| *n)
}

fn iso(year: &str, month: u32, day: &str) -> Option<String> {
    let d: u32 = day.parse().ok()?;
    (1..=31).contains(&d).then(|| format!("{year}-{month:02}-{d:02}"))
}

/// Dates written in the text as ISO strings, with their byte span.
pub fn extract_dates(text: &str) -> Vec<(String, usize, usize)> {
    let mut out = Vec::new();
    for c in DAY_MONTH_YEAR.captures_iter(text) {
        if let (Some(m), Some(month)) = (c.get(0), month_number(&c[2])) {
            if let Some(d) = iso(&c[3], month, &c[1]) {
                out.push((d, m.start(), m.end()));
            }
        }
    }
    for c in MONTH_DAY_YEAR.captures_iter(text) {
        if let (Some(m), Some(month)) = (c.get(0), month_number(&c[1])) {
            if let Some(d) = iso(&c[3], month, &c[2]) {
                if !out.iter().any(|(_, s, e)| m.start() < *e && *s < m.end()) {
                    out.push((d, m.start(), m.end()));
                }
            }
        }
    }
    for c in ISO_DATE.captures_iter(text) {
        let m = c.get(0).unwrap();
        out.push((m.as_str().to_string(), m.start(), m.end()));
    }
    out.sort_by_key(|(_, s, _)| *s);
    out
}

/// Amounts (Rs/INR/₹/$/USD) as digits only, e.g. "Rs 2,50,000" -> "250000".
pub fn extract_amounts(text: &str) -> Vec<String> {
    AMOUNT.captures_iter(text).map(|c| c[1].replace(',', "")).collect()
}

fn keywords(text: &str) -> HashSet<String> {
    let lower = text.to_lowercase();
    WORD.find_iter(&lower)
        .map(|m| m.as_str())
        .filter(|w| w.len() >= 4 && !STOP.contains(w))
        .map(str::to_string)
        .collect()
}

fn norm(text: &str) -> String {
    text.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_lowercase()
}

fn window(text: &str, start: usize, end: usize, radius: usize) -> &str {
    let mut a = start.saturating_sub(radius);
    while !text.is_char_boundary(a) {
        a -= 1;
    }
    let mut b = (end + radius).min(text.len());
    while !text.is_char_boundary(b) {
        b += 1;
    }
    &text[a..b]
}

fn str_field<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| v.get(*k).and_then(Value::as_str))
}

fn first_source(v: &Value) -> Option<Value> {
    v.get("provenance")
        .and_then(|p| p.get("sources"))
        .and_then(|s| s.get(0))
        .cloned()
        .or_else(|| v.get("left_source").cloned())
}

fn array<'a>(graph: &'a Value, key: &str) -> Vec<&'a Value> {
    graph.get(key).and_then(Value::as_array).map(|a| a.iter().collect()).unwrap_or_default()
}

pub fn evaluate(
    session_id: &str,
    reply: &str,
    previous: Option<&str>,
    graph: &Value,
    permitted: &[PermittedAuthority],
) -> AnswerFeedback {
    let mut items = Vec::new();
    let reply_words = keywords(reply);
    let reply_dates = extract_dates(reply);
    let reply_norm = norm(reply);

    // --- Engagement with the previous challenge -----------------------------------------------------------------------
    let (engagement, matched, total) = match previous {
        None => ("NO_PRIOR_CHALLENGE".to_string(), 0, 0),
        Some(challenge) => {
            let mut terms: Vec<String> = keywords(challenge).into_iter().collect();
            terms.extend(extract_dates(challenge).into_iter().map(|(d, _, _)| format!("date:{d}")));
            let reply_date_set: HashSet<String> = reply_dates.iter().map(|(d, _, _)| format!("date:{d}")).collect();
            let hit = terms.iter().filter(|t| reply_words.contains(*t) || reply_date_set.contains(*t)).count();
            let label = if terms.is_empty() {
                "PARTIAL"
            } else if hit as f64 / terms.len() as f64 >= ADDRESSED_RATIO {
                "ADDRESSED"
            } else if hit > 0 {
                "PARTIAL"
            } else {
                "NOT_ADDRESSED"
            };
            (label.to_string(), hit, terms.len())
        }
    };

    // --- Dates against the chronology ---------------------------------------------------------------------------------
    let chronology = array(graph, "chronology");
    for (date, start, end) in &reply_dates {
        let exact: Vec<&&Value> =
            chronology.iter().filter(|e| str_field(e, &["date"]).map(|d| d == date).unwrap_or(false)).collect();
        let context = keywords(window(reply, *start, *end, 90));
        let overlap = |e: &Value| keywords(str_field(e, &["summary"]).unwrap_or("")).intersection(&context).count();
        if let Some(event) = exact.iter().max_by_key(|e| overlap(e)) {
            items.push(FeedbackItem {
                kind: "DATE".into(),
                status: "SUPPORTED_BY_RECORD".into(),
                value: Some(date.clone()),
                message: format!("Matches the record: '{}' ({date}).", str_field(event, &["summary"]).unwrap_or("")),
                record_ref: str_field(event, &["event_id"]).map(str::to_string),
                locator: first_source(event),
            });
        } else if let Some(event) = chronology.iter().filter(|e| overlap(e) > 0).max_by_key(|e| overlap(e)) {
            items.push(FeedbackItem {
                kind: "DATE".into(),
                status: "CONFLICTS_WITH_RECORD".into(),
                value: Some(date.clone()),
                message: format!(
                    "Check this date: the record dates '{}' to {}, but the answer says {date}.",
                    str_field(event, &["summary"]).unwrap_or(""),
                    str_field(event, &["date"]).unwrap_or("?")
                ),
                record_ref: str_field(event, &["event_id"]).map(str::to_string),
                locator: first_source(event),
            });
        } else {
            items.push(FeedbackItem {
                kind: "DATE".into(),
                status: "NOT_FOUND_IN_RECORD".into(),
                value: Some(date.clone()),
                message: format!("No event on {date} was found in the record; add a source or drop it."),
                record_ref: None,
                locator: None,
            });
        }
    }

    // --- Amounts against chronology, evidence and contradictions ------------------------------------------------------
    let mut record_amounts: Vec<(String, String, String, Option<Value>)> = Vec::new(); // (digits, ref, text, locator)
    for (key, id_keys) in [("chronology", ["event_id", "id"]), ("evidence", ["evidence_id", "id"]), ("contradictions", ["contradiction_id", "id"])] {
        for entry in array(graph, key) {
            let text = str_field(entry, &["summary", "label", "description", "name"]).unwrap_or("");
            for digits in extract_amounts(text) {
                record_amounts.push((
                    digits,
                    str_field(entry, &id_keys).unwrap_or("").to_string(),
                    text.to_string(),
                    first_source(entry),
                ));
            }
        }
    }
    for digits in extract_amounts(reply) {
        match record_amounts.iter().find(|(d, ..)| *d == digits) {
            Some((_, rref, text, loc)) => items.push(FeedbackItem {
                kind: "AMOUNT".into(),
                status: "SUPPORTED_BY_RECORD".into(),
                value: Some(digits.clone()),
                message: format!("Matches the record: '{text}'."),
                record_ref: Some(rref.clone()),
                locator: loc.clone(),
            }),
            None => items.push(FeedbackItem {
                kind: "AMOUNT".into(),
                status: "NOT_FOUND_IN_RECORD".into(),
                value: Some(digits.clone()),
                message: format!("Amount {digits} was not found in the record; add a source or drop it."),
                record_ref: None,
                locator: None,
            }),
        }
    }

    // --- Authorities: working set vs. not verified --------------------------------------------------------------------
    let mut cited: HashSet<String> = HashSet::new();
    for a in permitted {
        let n = norm(&a.citation);
        if !n.is_empty() && reply_norm.contains(&n) {
            cited.insert(a.authority_id.clone());
            items.push(FeedbackItem {
                kind: "AUTHORITY".into(),
                status: "VERIFIED_IN_WORKING_SET".into(),
                value: Some(a.citation.clone()),
                message: format!("In the counsel-confirmed working set ({}).", a.authority_id),
                record_ref: Some(a.authority_id.clone()),
                locator: None,
            });
        }
    }
    let permitted_norms: Vec<String> = permitted.iter().map(|a| norm(&a.citation)).collect();
    let mut seen = HashSet::new();
    for m in REPORTER_CITATION.find_iter(reply).chain(STATUTE.find_iter(reply)) {
        let n = norm(m.as_str());
        let covered = permitted_norms.iter().any(|p| !p.is_empty() && (p.contains(&n) || n.contains(p.as_str())));
        if covered || !seen.insert(n) {
            continue;
        }
        items.push(FeedbackItem {
            kind: "AUTHORITY".into(),
            status: "NOT_IN_WORKING_SET".into(),
            value: Some(m.as_str().trim().to_string()),
            message: "Not in the working set: NOT VERIFIED until it is retrieved from an installed pack and confirmed.".into(),
            record_ref: None,
            locator: None,
        });
    }

    // --- What the record offers that the answer did not use (only relative to an actual challenge) ------------------
    if let Some(challenge) = previous {
        let challenge_words = keywords(challenge);
        for c in array(graph, "contradictions") {
            let summary_words = keywords(str_field(c, &["summary"]).unwrap_or(""));
            if summary_words.intersection(&challenge_words).count() >= 2 && summary_words.intersection(&reply_words).count() == 0 {
                items.push(FeedbackItem {
                    kind: "OPEN_CONTRADICTION".into(),
                    status: "AVAILABLE_NOT_USED".into(),
                    value: None,
                    message: format!(
                        "The record has a contradiction on this point that the answer does not deal with: {}",
                        str_field(c, &["summary"]).unwrap_or("")
                    ),
                    record_ref: str_field(c, &["contradiction_id", "id"]).map(str::to_string),
                    locator: first_source(c),
                });
            }
        }
        for a in permitted.iter().filter(|a| !cited.contains(&a.authority_id)) {
            if keywords(&a.label).intersection(&challenge_words).count() >= 1 {
                items.push(FeedbackItem {
                    kind: "UNUSED_AUTHORITY".into(),
                    status: "AVAILABLE_NOT_USED".into(),
                    value: Some(a.citation.clone()),
                    message: format!("Relevant to the challenge and in the working set, but not used: {} ({}).", a.citation, a.authority_id),
                    record_ref: Some(a.authority_id.clone()),
                    locator: None,
                });
            }
        }
    }

    AnswerFeedback {
        schema_version: SCHEMA_VERSION.into(),
        engine_version: ENGINE_VERSION.into(),
        session_id: session_id.to_string(),
        engagement,
        engagement_terms_matched: matched,
        engagement_terms_total: total,
        items,
        generated_by: "deterministic".into(),
        model_used: false,
        outcome_prediction: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../golden/G-ANSWER-FEEDBACK/fixtures/partition_appeal.json");

    fn permitted(fx: &Value) -> Vec<PermittedAuthority> {
        fx["permitted_authorities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| PermittedAuthority {
                authority_id: a["authority_id"].as_str().unwrap().into(),
                citation: a["citation"].as_str().unwrap().into(),
                label: a["label"].as_str().unwrap().into(),
            })
            .collect()
    }

    #[test]
    fn golden_partition_appeal_turns_match_exactly() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let auth = permitted(&fx);
        let prohibited: Vec<String> =
            fx["prohibited_language"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_lowercase()).collect();
        for turn in fx["turns"].as_array().unwrap() {
            let id = turn["id"].as_str().unwrap();
            let previous = match turn.get("previous_challenge") {
                Some(Value::Null) => None,
                Some(Value::String(s)) => Some(s.clone()),
                _ => fx["previous_challenge"].as_str().map(str::to_string),
            };
            let fb = evaluate("s1", turn["reply"].as_str().unwrap(), previous.as_deref(), &fx["graph"], &auth);
            let expect = &turn["expect"];
            assert_eq!(fb.engagement, expect["engagement"].as_str().unwrap(), "{id}: engagement {fb:?}");
            let expected_items = expect["items"].as_array().unwrap();
            if expected_items.is_empty() {
                assert!(fb.items.is_empty(), "{id}: expected no items, got {:?}", fb.items);
            }
            for e in expected_items {
                let found = fb.items.iter().any(|i| {
                    i.kind == e["kind"].as_str().unwrap()
                        && i.status == e["status"].as_str().unwrap()
                        && e.get("value").and_then(Value::as_str).map(|v| i.value.as_deref() == Some(v)).unwrap_or(true)
                        && e.get("record_ref").and_then(Value::as_str).map(|r| i.record_ref.as_deref() == Some(r)).unwrap_or(true)
                });
                assert!(found, "{id}: missing expected item {e} in {:?}", fb.items);
            }
            if let Some(kinds) = expect.get("must_not_contain_kinds").and_then(Value::as_array) {
                for k in kinds {
                    assert!(!fb.items.iter().any(|i| i.kind == k.as_str().unwrap()), "{id}: unexpected {k}");
                }
            }
            for item in &fb.items {
                let msg = item.message.to_lowercase();
                for word in &prohibited {
                    assert!(!msg.split(|c: char| !c.is_alphanumeric() && c != '%').any(|w| w == word.as_str()) && !(word.contains(' ') && msg.contains(word.as_str())),
                        "{id}: prohibited '{word}' in message '{}'", item.message);
                }
                if ["SUPPORTED_BY_RECORD", "CONFLICTS_WITH_RECORD", "VERIFIED_IN_WORKING_SET", "AVAILABLE_NOT_USED"].contains(&item.status.as_str()) {
                    assert!(item.record_ref.as_deref().map(|r| !r.is_empty()).unwrap_or(false), "{id}: {item:?} lacks record_ref");
                }
            }
            assert!(!fb.model_used && !fb.outcome_prediction && fb.generated_by == "deterministic");
        }
    }

    #[test]
    fn date_and_amount_parsing() {
        let d: Vec<String> = extract_dates("On 2nd March 2020, then March 14, 2019 and 2021-01-05.").into_iter().map(|x| x.0).collect();
        assert_eq!(d, vec!["2020-03-02", "2019-03-14", "2021-01-05"]);
        assert_eq!(extract_amounts("paid Rs 2,50,000 and $1,200.50 and ₹75,000"), vec!["250000", "1200", "75000"]);
        assert!(extract_dates("(2101) 3 SCC 77 and 31 Foo 2020").is_empty());
    }
}
