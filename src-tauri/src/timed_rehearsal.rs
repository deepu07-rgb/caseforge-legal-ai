//! Timed Oral Rehearsal (B-S4-12, PRD v4.0 section 7.7).
//!
//! Mock Court gains a timer mode: counsel sets a slot, the bench interrupts at a rate, and the debrief shows a timeline of which
//! limbs of the argument were reached, when, for how long, and which were never reached.
//!
//! Everything here is deterministic and runs in the host; there is no model in this module.
//!   - The host keeps the clock. A session is a state machine over a time the caller reads from a monotonic clock
//!     (`Instant`, never the wall clock and never the screen's timers). Events are stamped with the exact moment they became due
//!     (a warning at its mark, the expiry at the limit), not with the moment somebody looked, so the timeline is accurate to the
//!     millisecond whatever the polling interval; how soon the screen *learns* of the expiry depends on how often it asks.
//!   - At the limit the session ends gracefully: the expiry and the end are recorded, later responses are refused with
//!     CF_REHEARSAL_TIME_UP, the unsent text stays on the screen, and the timeline is complete.
//!   - Interruptions come from the argument's Missing Links (then its weakest limbs), in the integrity engine's own rank order.
//!     The wording is reconstructed by the host from the Missing Link record - the limb's label, the link, its score and the
//!     threshold, and the inputs the scorer cited - never written by a model, and every interruption carries the id of the
//!     Missing Link it came from (`grounding_ref`) and the inputs it rests on. This is the host-built counterpart of the
//!     B-S3-16 rule (the visible challenge is reconstructed from verified material); it does not go through the model prompt.
//!   - A limb is "reached" by a response when the response uses enough of that limb's own vocabulary (rules in
//!     contracts/rehearsal_policy.json). That is a wording match, not an understanding of the argument: it can miss a point made
//!     in other words, so the timeline says "reached" for what was detected and the screen says how it was detected.
//! Rules are data (contracts/rehearsal_policy.json), the record is contracts/rehearsal_timeline.schema.json, the golden is
//! golden/G-SCRIPTS/fixtures/timed_rehearsal_core.json.

use crate::integrity::{MissingLinkRecord, ScoreReport};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Mutex;
use std::time::Instant;

pub const POLICY_JSON: &str = include_str!("../../contracts/rehearsal_policy.json");
pub const TIME_UP: &str = "CF_REHEARSAL_TIME_UP";

pub struct Policy {
    pub presets_seconds: Vec<i64>,
    pub min_seconds: i64,
    pub max_seconds: i64,
    pub warnings_remaining_s: Vec<i64>,
    pub interval_ms: HashMap<String, Option<i64>>,
    pub default_rates: Vec<(i64, String)>,
    pub min_token_len: usize,
    pub w_two: f64,
    pub w_three: f64,
    pub min_score: f64,
    pub stopwords: HashSet<String>,
}

pub fn policy() -> &'static Policy {
    static P: Lazy<Policy> = Lazy::new(|| {
        let v: Value = serde_json::from_str(POLICY_JSON).expect("contracts/rehearsal_policy.json parses");
        let ints = |p: &str| v.pointer(p).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
        Policy {
            presets_seconds: ints("/slot/presets_seconds"),
            min_seconds: v.pointer("/slot/min_seconds").and_then(Value::as_i64).unwrap_or(60),
            max_seconds: v.pointer("/slot/max_seconds").and_then(Value::as_i64).unwrap_or(7200),
            warnings_remaining_s: ints("/warnings_remaining_seconds"),
            interval_ms: v["interruption_interval_ms"].as_object().map(|o| o.iter().map(|(k, x)| (k.clone(), x.as_i64())).collect()).unwrap_or_default(),
            default_rates: v["default_rate_by_archetype_interruption"].as_array().map(|a| a.iter().map(|r| (r["min_interruption"].as_i64().unwrap_or(0), r["rate"].as_str().unwrap_or("NONE").to_string())).collect()).unwrap_or_default(),
            min_token_len: v.pointer("/reach/min_token_length").and_then(Value::as_u64).unwrap_or(4) as usize,
            w_two: v.pointer("/reach/weight_two_limbs").and_then(Value::as_f64).unwrap_or(0.5),
            w_three: v.pointer("/reach/weight_three_or_more").and_then(Value::as_f64).unwrap_or(0.0),
            min_score: v.pointer("/reach/min_score").and_then(Value::as_f64).unwrap_or(2.0),
            stopwords: v.pointer("/reach/stopwords").and_then(Value::as_array).map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        }
    });
    &P
}

pub const RATES: [&str; 4] = ["NONE", "LOW", "MEDIUM", "HIGH"];

pub fn parse_rate(rate: &str) -> Result<&'static str, String> {
    RATES.iter().copied().find(|r| *r == rate).ok_or_else(|| format!("CF_REHEARSAL_BAD_RATE: {rate:?} is not one of {RATES:?}"))
}

pub fn validate_slot(seconds: i64) -> Result<(), String> {
    let p = policy();
    if seconds < p.min_seconds || seconds > p.max_seconds {
        return Err(format!("CF_REHEARSAL_BAD_SLOT: the slot must be between {} and {} seconds", p.min_seconds, p.max_seconds));
    }
    Ok(())
}

pub fn interval_ms(rate: &str) -> Option<i64> {
    policy().interval_ms.get(rate).copied().flatten()
}

/// The rate an archetype's interruption setting (0-100) suggests; counsel can change it.
pub fn default_rate(interruption: i64) -> &'static str {
    let mut rows = policy().default_rates.clone();
    rows.sort_by_key(|r| std::cmp::Reverse(r.0));
    let name = rows.iter().find(|(min, _)| interruption >= *min).map(|(_, r)| r.as_str()).unwrap_or("NONE");
    RATES.iter().copied().find(|r| *r == name).unwrap_or("NONE")
}

// ---------- reaching a limb ----------

fn tokens(text: &str) -> BTreeSet<String> {
    let p = policy();
    let mut out = BTreeSet::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut BTreeSet<String>| {
        if cur.chars().count() >= p.min_token_len && !p.stopwords.contains(cur.as_str()) {
            out.insert(std::mem::take(cur));
        } else {
            cur.clear();
        }
    };
    for c in text.chars() {
        if c.is_alphanumeric() {
            cur.extend(c.to_lowercase());
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

#[derive(Debug, Clone)]
pub struct LimbInfo {
    pub node_id: String,
    pub label: String,
    pub vi: i64,
    vocab: BTreeSet<String>,
}

/// The limbs of an argument tree, in tree order, with their vocabulary: label, theory, and every anchor's text and ref.
pub fn limbs_of(tree: &Value, report: &ScoreReport) -> Vec<LimbInfo> {
    let vi: HashMap<&str, i64> = report.limbs.iter().map(|l| (l.node_id.as_str(), l.VI as i64)).collect();
    tree["limbs"]
        .as_array()
        .map(|a| a.iter().filter_map(|l| {
            let id = l["node_id"].as_str()?;
            let mut vocab = tokens(l["label"].as_str().unwrap_or(""));
            vocab.extend(tokens(l["theory"].as_str().unwrap_or("")));
            for an in l["anchors"].as_array().into_iter().flatten() {
                vocab.extend(tokens(an["text"].as_str().unwrap_or("")));
                vocab.extend(tokens(an["ref"].as_str().unwrap_or("")));
            }
            Some(LimbInfo { node_id: id.to_string(), label: l["label"].as_str().unwrap_or(id).to_string(), vi: vi.get(id).copied().unwrap_or(0), vocab })
        }).collect())
        .unwrap_or_default()
}

/// (limb index, score) for every limb a response reaches, in tree order.
pub fn reached(text: &str, limbs: &[LimbInfo]) -> Vec<(usize, f64)> {
    let p = policy();
    let words = tokens(text);
    let weight = |tok: &str| match limbs.iter().filter(|l| l.vocab.contains(tok)).count() {
        1 => 1.0,
        2 => p.w_two,
        n if n >= 3 => p.w_three,
        _ => 0.0,
    };
    limbs
        .iter()
        .enumerate()
        .filter_map(|(i, l)| {
            let score: f64 = words.iter().filter(|w| l.vocab.contains(*w)).map(|w| weight(w)).sum();
            (score >= p.min_score).then_some((i, score))
        })
        .collect()
}

// ---------- interruption targets ----------

#[derive(Debug, Clone, Serialize)]
pub struct Target {
    pub id: String,
    pub limb_id: String,
    /// MISSING_LINK or WEAK_LIMB
    pub kind: &'static str,
    pub link: Option<String>,
    pub score: Option<i64>,
    pub vi: Option<i64>,
    pub inputs_cited: Vec<String>,
    pub text: String,
    pub used: bool,
}

fn link_phrase(link: &str) -> &'static str {
    match link {
        "Fact" => "factual basis",
        "Evidence" => "evidence",
        "Law" => "legal authority",
        _ => "exposure to the other side's answer",
    }
}

fn missing_link_text(label: &str, m: &MissingLinkRecord) -> String {
    let desc = m.description.as_deref().map(|d| format!(" {}{}", d.trim_end_matches('.'), ".")).unwrap_or_default();
    format!("Counsel, on \"{label}\": the {} is weak ({} of 100; the threshold is {}).{desc} What is your answer?", link_phrase(&m.link), m.score, m.threshold)
}

/// Missing Links in the scorer's rank order, then the limbs that have none, weakest first.
pub fn targets_of(report: &ScoreReport, limbs: &[LimbInfo]) -> Vec<Target> {
    let label = |id: &str| limbs.iter().find(|l| l.node_id == id).map(|l| l.label.clone()).unwrap_or_else(|| id.to_string());
    let mut t: Vec<Target> = report
        .missing_links_panel
        .iter()
        .map(|m| Target {
            id: format!("ml:{}:{}", m.limb_id, m.link),
            limb_id: m.limb_id.clone(),
            kind: "MISSING_LINK",
            link: Some(m.link.clone()),
            score: Some(m.score as i64),
            vi: None,
            inputs_cited: m.inputs_cited.clone(),
            text: missing_link_text(&label(&m.limb_id), m),
            used: false,
        })
        .collect();
    let with_ml: HashSet<&str> = report.missing_links_panel.iter().map(|m| m.limb_id.as_str()).collect();
    let mut weak: Vec<&LimbInfo> = limbs.iter().filter(|l| !with_ml.contains(l.node_id.as_str())).collect();
    weak.sort_by_key(|l| std::cmp::Reverse(l.vi)); // stable: ties keep tree order
    for l in weak {
        t.push(Target {
            id: format!("limb:{}", l.node_id),
            limb_id: l.node_id.clone(),
            kind: "WEAK_LIMB",
            link: None,
            score: None,
            vi: Some(l.vi),
            inputs_cited: vec![format!("limb:{}", l.node_id), format!("vulnerability_index:{}", l.vi)],
            text: format!("Counsel, on \"{}\": this is one of your weaker points (vulnerability {} of 100). What is your answer?", l.label, l.vi),
            used: false,
        });
    }
    t
}

// ---------- the session ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Event {
    pub t_ms: i64,
    pub kind: String,
    pub limb_node_id: Option<String>,
    pub turn_id: Option<String>,
    pub grounding_ref: Option<String>,
}

#[derive(Debug, Clone)]
struct LimbState {
    reached: bool,
    first_reached_ms: Option<i64>,
    time_spent_ms: i64,
    interruptions: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Interruption {
    pub grounding_ref: String,
    pub limb_node_id: String,
    pub kind: &'static str,
    pub text: String,
    pub inputs_cited: Vec<String>,
    pub t_ms: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub text: String,
    pub source_kind: &'static str,
    pub limb_node_id: Option<String>,
    pub event_index: Option<usize>,
}

#[derive(Debug)]
pub struct Session {
    pub session_id: String,
    pub case_id: String,
    pub slot_ms: i64,
    pub rate: &'static str,
    pub archetype_id: Option<String>,
    limbs: Vec<LimbInfo>,
    state: Vec<LimbState>,
    targets: Vec<Target>,
    pub events: Vec<Event>,
    warnings_done: HashSet<i64>,
    last_interruption_ms: i64,
    last_response_ms: i64,
    current_limb: Option<usize>,
    turns: usize,
    pub ended_by: Option<&'static str>,
    ended_at_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Response {
    pub t_ms: i64,
    pub remaining_ms: i64,
    pub turn_id: String,
    pub limbs_reached_now: Vec<String>,
    pub newly_reached: Vec<String>,
    pub interruption: Option<Interruption>,
    pub new_events: Vec<Event>,
}

impl Session {
    pub fn start(session_id: &str, case_id: &str, slot_seconds: i64, rate: &str, archetype_id: Option<&str>, limbs: Vec<LimbInfo>, targets: Vec<Target>) -> Result<Session, String> {
        validate_slot(slot_seconds)?;
        Self::start_unchecked(session_id, case_id, slot_seconds * 1000, rate, archetype_id, limbs, targets)
    }

    /// Without the slot range check (tests of the timer use slots shorter than the shortest real one).
    pub(crate) fn start_unchecked(session_id: &str, case_id: &str, slot_ms: i64, rate: &str, archetype_id: Option<&str>, limbs: Vec<LimbInfo>, targets: Vec<Target>) -> Result<Session, String> {
        let rate = parse_rate(rate)?;
        if limbs.is_empty() {
            return Err("CF_REHEARSAL_NO_TREE: the argument tree has no limbs to rehearse; build it first".into());
        }
        let state = vec![LimbState { reached: false, first_reached_ms: None, time_spent_ms: 0, interruptions: 0 }; limbs.len()];
        Ok(Session {
            session_id: session_id.to_string(),
            case_id: case_id.to_string(),
            slot_ms,
            rate,
            archetype_id: archetype_id.map(str::to_string),
            limbs,
            state,
            targets,
            events: vec![Event { t_ms: 0, kind: "SESSION_START".into(), limb_node_id: None, turn_id: None, grounding_ref: None }],
            warnings_done: HashSet::new(),
            last_interruption_ms: 0,
            last_response_ms: 0,
            current_limb: None,
            turns: 0,
            ended_by: None,
            ended_at_ms: None,
        })
    }

    pub fn ended(&self) -> bool {
        self.ended_by.is_some()
    }

    fn push(&mut self, out: &mut Vec<Event>, t_ms: i64, kind: &str, limb: Option<usize>, turn: Option<&str>, gref: Option<&str>) {
        let e = Event { t_ms, kind: kind.into(), limb_node_id: limb.map(|i| self.limbs[i].node_id.clone()), turn_id: turn.map(str::to_string), grounding_ref: gref.map(str::to_string) };
        self.events.push(e.clone());
        out.push(e);
    }

    /// Time since the last response or interruption goes to the limb the user was last on.
    fn credit_tail(&mut self, until_ms: i64) {
        let from = self.last_response_ms.max(self.last_interruption_ms);
        if let (Some(i), true) = (self.current_limb, until_ms > from) {
            self.state[i].time_spent_ms += until_ms - from;
        }
    }

    /// Advances the timer to `now_ms` and returns the events that became due, each stamped with the moment it was due.
    pub fn observe(&mut self, now_ms: i64) -> Vec<Event> {
        let mut out = vec![];
        if self.ended() {
            return out;
        }
        let mut marks: Vec<i64> = policy().warnings_remaining_s.iter().map(|r| self.slot_ms - r * 1000).filter(|m| *m > 0).collect();
        marks.sort_unstable();
        marks.dedup();
        for m in marks {
            if now_ms >= m && self.warnings_done.insert(m) {
                self.push(&mut out, m, "TIME_WARNING", None, None, None);
            }
        }
        if now_ms >= self.slot_ms {
            let end = self.slot_ms;
            self.credit_tail(end);
            self.push(&mut out, end, "TIME_EXPIRED", None, None, None);
            self.push(&mut out, end, "SESSION_END", None, None, None);
            self.ended_by = Some("TIME_EXPIRED");
            self.ended_at_ms = Some(end);
        }
        out
    }

    pub fn remaining_ms(&self, now_ms: i64) -> i64 {
        (self.slot_ms - now_ms).max(0)
    }

    /// Counsel ends the session before the limit.
    pub fn end_early(&mut self, now_ms: i64) -> Vec<Event> {
        let mut out = self.observe(now_ms);
        if self.ended() {
            return out;
        }
        self.credit_tail(now_ms);
        self.push(&mut out, now_ms, "SESSION_END", None, None, None);
        self.ended_by = Some("USER_ENDED");
        self.ended_at_ms = Some(now_ms);
        out
    }

    /// A response (text typed, or a confirmed voice transcript) submitted at `now_ms`.
    pub fn respond(&mut self, now_ms: i64, text: &str) -> Result<Response, String> {
        let mut new_events = self.observe(now_ms);
        if self.ended() {
            return Err(format!("{TIME_UP}: the slot has ended; your text was not sent and is still on the screen"));
        }
        self.turns += 1;
        let turn = format!("t{}", self.turns);
        let hits = reached(text, &self.limbs);
        let primary = hits.iter().copied().fold(None, |best: Option<(usize, f64)>, h| match best {
            Some(b) if b.1 >= h.1 => Some(b),
            _ => Some(h),
        }).map(|h| h.0);
        // the time spent speaking since the last response or interruption
        let span = now_ms - self.last_response_ms.max(self.last_interruption_ms);
        let credited: Vec<usize> = if hits.is_empty() { self.current_limb.into_iter().collect() } else { hits.iter().map(|h| h.0).collect() };
        if span > 0 && !credited.is_empty() {
            let share = span / credited.len() as i64;
            let rest = span - share * credited.len() as i64;
            for (k, i) in credited.iter().enumerate() {
                self.state[*i].time_spent_ms += share + if k == 0 { rest } else { 0 };
            }
        }
        let shown = primary.or(self.current_limb);
        self.push(&mut new_events, now_ms, "USER_RESPONSE", shown, Some(&turn), None);
        let mut newly = vec![];
        for (i, _) in &hits {
            if !self.state[*i].reached {
                self.state[*i].reached = true;
                self.state[*i].first_reached_ms = Some(now_ms);
                newly.push(self.limbs[*i].node_id.clone());
                self.push(&mut new_events, now_ms, "LIMB_REACHED", Some(*i), Some(&turn), None);
            }
        }
        if primary.is_some() {
            self.current_limb = primary;
        }
        self.last_response_ms = now_ms;
        // does the bench interrupt?
        let mut interruption = None;
        if let Some(every) = interval_ms(self.rate) {
            if now_ms - self.last_interruption_ms >= every {
                let matched: HashSet<&str> = hits.iter().map(|h| self.limbs[h.0].node_id.as_str()).chain(self.current_limb.map(|i| self.limbs[i].node_id.as_str())).collect();
                let pick = self.targets.iter().position(|t| !t.used && t.kind == "MISSING_LINK" && matched.contains(t.limb_id.as_str()))
                    .or_else(|| self.targets.iter().position(|t| !t.used && t.kind == "MISSING_LINK"))
                    .or_else(|| self.targets.iter().position(|t| !t.used));
                if let Some(k) = pick {
                    self.targets[k].used = true;
                    let t = self.targets[k].clone();
                    let li = self.limbs.iter().position(|l| l.node_id == t.limb_id);
                    if let Some(li) = li {
                        self.state[li].interruptions += 1;
                    }
                    self.last_interruption_ms = now_ms;
                    self.push(&mut new_events, now_ms, "INTERRUPTION", li, Some(&turn), Some(&t.id));
                    interruption = Some(Interruption { grounding_ref: t.id.clone(), limb_node_id: t.limb_id.clone(), kind: t.kind, text: t.text.clone(), inputs_cited: t.inputs_cited.clone(), t_ms: now_ms });
                }
            }
        }
        Ok(Response {
            t_ms: now_ms,
            remaining_ms: self.remaining_ms(now_ms),
            turn_id: turn,
            limbs_reached_now: hits.iter().map(|h| self.limbs[h.0].node_id.clone()).collect(),
            newly_reached: newly,
            interruption,
            new_events,
        })
    }

    /// Node id -> label of every limb (kept beside the timeline so it can be read later without the tree).
    pub fn limb_labels(&self) -> Value {
        Value::Object(self.limbs.iter().map(|l| (l.node_id.clone(), Value::String(l.label.clone()))).collect())
    }

    /// The record of the session, per contracts/rehearsal_timeline.schema.json. Only a finished session has one.
    pub fn timeline(&self, debrief_id: Option<&str>) -> Result<Value, String> {
        let ended_by = self.ended_by.ok_or("CF_REHEARSAL_NOT_ENDED: end the session before reading its timeline")?;
        Ok(json!({
            "schema_version": "1.0.0",
            "timeline_id": format!("tl:{}", self.session_id),
            "session_id": self.session_id,
            "debrief_id": debrief_id,
            "slot_seconds": self.slot_ms / 1000,
            "interruption_rate": self.rate,
            "bench_archetype_id": self.archetype_id,
            "ended_by": ended_by,
            "events": self.events,
            "limbs": self.limbs.iter().zip(&self.state).map(|(l, s)| json!({
                "limb_node_id": l.node_id, "reached": s.reached, "first_reached_ms": s.first_reached_ms, "time_spent_ms": s.time_spent_ms, "interruptions": s.interruptions
            })).collect::<Vec<_>>(),
        }))
    }

    /// What the debrief says, each statement tied to the part of the timeline it comes from. Observations only.
    pub fn findings(&self) -> Vec<Finding> {
        let mut out = vec![];
        for (l, s) in self.limbs.iter().zip(&self.state) {
            if !s.reached {
                out.push(Finding { text: format!("You did not reach \"{}\" in this slot.", l.label), source_kind: "TIMELINE_LIMB", limb_node_id: Some(l.node_id.clone()), event_index: None });
            }
        }
        let slot_s = (self.slot_ms as f64 / 1000.0).round() as i64;
        for (l, s) in self.limbs.iter().zip(&self.state) {
            if s.reached {
                let secs = (s.time_spent_ms as f64 / 1000.0).round() as i64;
                let pct = (100.0 * s.time_spent_ms as f64 / self.slot_ms.max(1) as f64).round() as i64;
                out.push(Finding { text: format!("You spent {secs} s of {slot_s} s ({pct}%) on \"{}\".", l.label), source_kind: "TIMELINE_LIMB", limb_node_id: Some(l.node_id.clone()), event_index: None });
            }
        }
        for (i, e) in self.events.iter().enumerate().filter(|(_, e)| e.kind == "INTERRUPTION") {
            let Some(t) = e.grounding_ref.as_deref().and_then(|g| self.targets.iter().find(|t| t.id == g)) else { continue };
            let label = self.limbs.iter().find(|l| l.node_id == t.limb_id).map(|l| l.label.as_str()).unwrap_or("");
            let at = format!("{:02}:{:02}", e.t_ms / 60_000, (e.t_ms / 1000) % 60);
            let why = match (t.kind, &t.link, t.score, t.vi) {
                ("MISSING_LINK", Some(link), Some(score), _) => format!("from your Missing Links: {}, {score} of 100", link_phrase(link)),
                (_, _, _, Some(vi)) => format!("one of your weaker points, vulnerability {vi} of 100"),
                _ => "from your Missing Links".into(),
            };
            out.push(Finding { text: format!("The bench interrupted you at {at} on \"{label}\" ({why})."), source_kind: "TIMELINE_EVENT", limb_node_id: Some(t.limb_id.clone()), event_index: Some(i) });
        }
        out
    }
}

// ---------- live sessions, held by the host ----------

pub struct Live {
    pub session: Session,
    pub started: Instant,
}

impl Live {
    pub fn now_ms(&self) -> i64 {
        self.started.elapsed().as_millis() as i64
    }
}

pub static SESSIONS: Lazy<Mutex<HashMap<String, Live>>> = Lazy::new(|| Mutex::new(HashMap::new()));

pub fn policy_json() -> Value {
    let p = policy();
    json!({
        "batch": "B-S4-12",
        "status": "PROPOSED",
        "presets_seconds": p.presets_seconds,
        "min_seconds": p.min_seconds,
        "max_seconds": p.max_seconds,
        "rates": RATES,
        "interval_ms": RATES.iter().map(|r| (r.to_string(), json!(interval_ms(r)))).collect::<serde_json::Map<_, _>>(),
        "warnings_remaining_seconds": p.warnings_remaining_s,
        "reached_by": "wording: a limb counts as reached when the response uses enough of that limb's own words; a point made in other words can be missed",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrity::score_tree;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../golden/G-SCRIPTS/fixtures/timed_rehearsal_core.json");
    const FIXTURE_SHA256: &str = "2579e4b7786abe06dcebdb76f2c9d4a6c05157b8ad301f1954de6d2b7cd5ec25"; // committed before the engine (949a7c6)
    const POLICY_SHA256: &str = "47218694b4de9cba2dedd375d29b9385f64297a16f7d04efa473ee907b4102b7"; // committed with the golden, before the engine (949a7c6)

    fn fx() -> Value {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "the timed-rehearsal golden changed since it was pinned");
        assert_eq!(hex::encode(Sha256::digest(POLICY_JSON.as_bytes())), POLICY_SHA256, "the rehearsal policy changed since it was pinned");
        serde_json::from_str(FIXTURE).unwrap()
    }

    fn setup(f: &Value) -> (Vec<LimbInfo>, Vec<Target>, ScoreReport) {
        let report = score_tree(&f["tree"]).unwrap();
        let limbs = limbs_of(&f["tree"], &report);
        let targets = targets_of(&report, &limbs);
        (limbs, targets, report)
    }

    fn new_session(f: &Value, slot_s: i64, rate: &str) -> Session {
        let (limbs, targets, _) = setup(f);
        Session::start("s1", "c1", slot_s, rate, Some("evidence_heavy"), limbs, targets).unwrap()
    }

    #[test]
    fn golden_the_missing_link_order_and_the_interruption_wording() {
        let f = fx();
        let (_, targets, _) = setup(&f);
        let ids: Vec<&str> = targets.iter().map(|t| t.id.as_str()).collect();
        let mut want: Vec<String> = f["missing_link_rank"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
        want.extend(f["limb_level_targets"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()));
        assert_eq!(ids, want.iter().map(String::as_str).collect::<Vec<_>>());
        for (id, text) in f["interruption_text"].as_object().unwrap() {
            let t = targets.iter().find(|t| &t.id == id).unwrap();
            assert_eq!(&t.text, text.as_str().unwrap(), "{id}");
            assert!(!t.inputs_cited.is_empty(), "{id} cites the inputs it rests on");
        }
    }

    #[test]
    fn golden_a_limb_is_reached_by_its_own_words() {
        let f = fx();
        let (limbs, _, _) = setup(&f);
        for c in f["reach_cases"].as_array().unwrap() {
            let got: Vec<&str> = reached(c["text"].as_str().unwrap(), &limbs).iter().map(|h| limbs[h.0].node_id.as_str()).collect();
            let want: Vec<&str> = c["reaches"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
            assert_eq!(got, want, "{}", c["text"]);
        }
    }

    #[test]
    fn golden_a_five_minute_session_gives_the_expected_timeline_and_findings() {
        let f = fx();
        let s = &f["session_five_minutes"];
        let mut sess = new_session(&f, s["slot_seconds"].as_i64().unwrap(), s["rate"].as_str().unwrap());
        for r in s["responses"].as_array().unwrap() {
            let text = f["responses"][r["key"].as_str().unwrap()].as_str().unwrap();
            sess.respond(r["at_s"].as_i64().unwrap() * 1000, text).unwrap();
        }
        sess.observe(s["final_poll_ms"].as_i64().unwrap());
        let want = &s["expect"];
        assert_eq!(sess.ended_by, Some("TIME_EXPIRED"));
        let tl = sess.timeline(None).unwrap();
        let want_events: Vec<Value> = want["events"].as_array().unwrap().clone();
        assert_eq!(tl["events"], Value::Array(want_events), "events");
        assert_eq!(tl["limbs"], want["limbs"], "limbs");
        assert_eq!(tl["ended_by"], "TIME_EXPIRED");
        // AC-S4-07.2: every limb of the tree is listed, reached or not
        assert_eq!(tl["limbs"].as_array().unwrap().len(), f["tree"]["limbs"].as_array().unwrap().len());
        // the time spent adds up to the slot
        let total: i64 = tl["limbs"].as_array().unwrap().iter().map(|l| l["time_spent_ms"].as_i64().unwrap()).sum();
        assert_eq!(total, 300_000);
        let got: Vec<Value> = sess.findings().iter().map(|g| json!({"text": g.text, "source": {"kind": g.source_kind, "limb_node_id": g.limb_node_id, "event_index": g.event_index}})).collect();
        assert_eq!(Value::Array(got), want["findings"], "findings");
        // the limb never reached is named
        let never: Vec<&str> = tl["limbs"].as_array().unwrap().iter().filter(|l| !l["reached"].as_bool().unwrap()).map(|l| l["limb_node_id"].as_str().unwrap()).collect();
        assert_eq!(never, want["never_reached"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect::<Vec<_>>());
    }

    #[test]
    fn golden_every_interruption_is_grounded_in_a_missing_link_of_the_tree() {
        // AC-S4-07.3
        let f = fx();
        let (limbs, targets, report) = setup(&f);
        let s = &f["session_five_minutes"];
        let mut sess = new_session(&f, 300, "MEDIUM");
        let mut seen = 0;
        for r in s["responses"].as_array().unwrap() {
            let text = f["responses"][r["key"].as_str().unwrap()].as_str().unwrap();
            if let Some(i) = sess.respond(r["at_s"].as_i64().unwrap() * 1000, text).unwrap().interruption {
                seen += 1;
                let t = targets.iter().find(|t| t.id == i.grounding_ref).expect("the grounding_ref names a target of this tree");
                assert_eq!(t.text, i.text, "the wording is the host's, built from the record");
                assert!(!i.inputs_cited.is_empty());
                match t.kind {
                    "MISSING_LINK" => {
                        let m = report.missing_links_panel.iter().find(|m| format!("ml:{}:{}", m.limb_id, m.link) == i.grounding_ref).expect("a Missing Link of the scorer");
                        assert_eq!(i.inputs_cited, m.inputs_cited, "cites the scorer's own inputs");
                    }
                    _ => assert!(limbs.iter().any(|l| l.node_id == i.limb_node_id)),
                }
            }
        }
        assert_eq!(seen, 2);
        // an interruption is never worded by anything but the host: no target text mentions a prediction or a ruling
        for t in &targets {
            let low = t.text.to_lowercase();
            assert!(!["will win", "likely to", "should be allowed", "the court holds", "guilty"].iter().any(|w| low.contains(w)), "{}", t.text);
        }
    }

    #[test]
    fn golden_the_timer_warns_expires_exactly_at_the_limit_and_refuses_late_responses() {
        // AC-S4-07.1
        let f = fx();
        let t = &f["session_timer"];
        let mut sess = new_session(&f, t["slot_seconds"].as_i64().unwrap(), t["rate"].as_str().unwrap());
        for p in t["polls_ms"].as_array().unwrap() {
            let got: Vec<Value> = sess.observe(p.as_i64().unwrap()).iter().map(|e| json!({"t_ms": e.t_ms, "kind": e.kind})).collect();
            assert_eq!(Value::Array(got), t["expect_events_by_poll"][p.to_string()], "poll at {p} ms");
        }
        assert_eq!(sess.ended_by, Some("TIME_EXPIRED"));
        for at in t["refused_at_ms"].as_array().unwrap() {
            let e = sess.respond(at.as_i64().unwrap(), "May I add one more point about the sale deed?").unwrap_err();
            assert!(e.starts_with(t["refusal_code"].as_str().unwrap()), "{e}");
        }
        // a response a millisecond before the limit is accepted, and the session then ends at the limit
        let mut s2 = new_session(&f, 120, "NONE");
        let r = s2.respond(t["accepted_at_ms"].as_i64().unwrap(), "The registered sale deed conveyed the whole house.").unwrap();
        assert_eq!(r.remaining_ms, 1);
        s2.observe(120_000);
        assert_eq!(s2.ended_by, Some("TIME_EXPIRED"));
        // the shortest real slot warns once (a warning 60 s before the end would be at the start)
        let mut s3 = new_session(&f, f["session_shortest_slot"]["slot_seconds"].as_i64().unwrap(), "NONE");
        s3.observe(60_000);
        let w: Vec<i64> = s3.events.iter().filter(|e| e.kind == "TIME_WARNING").map(|e| e.t_ms).collect();
        assert_eq!(w, f["session_shortest_slot"]["expect_warning_times_ms"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect::<Vec<_>>());
    }

    #[test]
    fn the_timer_is_accurate_on_a_real_monotonic_clock() {
        // AC-S4-07.1: the expiry is stamped at the limit; the screen learns of it as soon as it asks.
        let f = fx();
        let (limbs, targets, _) = setup(&f);
        let mut sess = Session::start_unchecked("s", "c", 1_500, "NONE", None, limbs, targets).unwrap();
        let started = Instant::now();
        let mut lag_ms = None;
        loop {
            let now = started.elapsed().as_millis() as i64;
            let ev = sess.observe(now);
            if let Some(e) = ev.iter().find(|e| e.kind == "TIME_EXPIRED") {
                assert_eq!(e.t_ms, 1_500, "stamped at the limit, not at the moment it was noticed");
                lag_ms = Some(now - e.t_ms);
                break;
            }
            assert!(now < 5_000, "the timer never expired");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let lag = lag_ms.unwrap();
        assert!((0..=policy_accuracy_ms()).contains(&lag), "noticed {lag} ms after the limit with a 10 ms poll");
    }

    fn policy_accuracy_ms() -> i64 {
        serde_json::from_str::<Value>(POLICY_JSON).unwrap()["timer_accuracy_max_ms"].as_i64().unwrap()
    }

    #[test]
    fn golden_interruption_rates_and_the_order_in_which_targets_are_used() {
        let f = fx();
        // none: never
        let n = &f["session_rate_none"];
        let mut none = new_session(&f, n["slot_seconds"].as_i64().unwrap(), "NONE");
        let oral = f["responses"]["oral"].as_str().unwrap();
        for t in n["response_times_s"].as_array().unwrap() {
            assert!(none.respond(t.as_i64().unwrap() * 1000, oral).unwrap().interruption.is_none());
        }
        assert_eq!(none.events.iter().filter(|e| e.kind == "INTERRUPTION").count() as i64, n["expect_interruptions"].as_i64().unwrap());
        // high: each target once, the matched limb first, then by rank, then the weak limbs; then silence
        let h = &f["session_high"];
        let mut high = new_session(&f, h["slot_seconds"].as_i64().unwrap(), "HIGH");
        let mut got = std::collections::BTreeMap::new();
        for t in h["response_times_s"].as_array().unwrap() {
            if let Some(i) = high.respond(t.as_i64().unwrap() * 1000, oral).unwrap().interruption {
                got.insert(t.as_i64().unwrap().to_string(), i.grounding_ref);
            }
        }
        let want: std::collections::BTreeMap<String, String> = h["expect_interruptions_at_s"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string())).collect();
        assert_eq!(got, want);
        assert_eq!(got.len() as i64, h["expect_total_interruptions"].as_i64().unwrap());
        // the rates and the archetype defaults
        for (rate, ms) in f["interval_ms"].as_object().unwrap() {
            assert_eq!(interval_ms(rate), ms.as_i64(), "{rate}");
        }
        for c in f["default_rate_by_archetype_interruption"].as_array().unwrap() {
            assert_eq!(default_rate(c["interruption"].as_i64().unwrap()), c["rate"].as_str().unwrap(), "{c}");
        }
        // the archetypes the app really has map to a rate
        for a in crate::adaptive_bench::archetypes_meta() {
            assert!(RATES.contains(&default_rate(a.baseline.interruption as i64)), "{}", a.id);
        }
    }

    #[test]
    fn golden_slots_and_starts_are_validated() {
        let f = fx();
        for s in f["slot_validation"]["ok_seconds"].as_array().unwrap() {
            assert!(validate_slot(s.as_i64().unwrap()).is_ok(), "{s}");
        }
        for s in f["slot_validation"]["refused_seconds"].as_array().unwrap() {
            assert!(validate_slot(s.as_i64().unwrap()).unwrap_err().starts_with("CF_REHEARSAL_BAD_SLOT"), "{s}");
        }
        assert!(parse_rate("EXTREME").unwrap_err().starts_with("CF_REHEARSAL_BAD_RATE"));
        let (limbs, targets, _) = setup(&f);
        assert!(Session::start("s", "c", 30, "LOW", None, limbs.clone(), targets.clone()).unwrap_err().starts_with("CF_REHEARSAL_BAD_SLOT"));
        assert!(Session::start("s", "c", 300, "SOMETIMES", None, limbs, targets).unwrap_err().starts_with("CF_REHEARSAL_BAD_RATE"));
        assert!(Session::start("s", "c", 300, "LOW", None, vec![], vec![]).unwrap_err().starts_with("CF_REHEARSAL_NO_TREE"));
        // the presets in the policy are the proposed threshold's, in seconds
        let thr: Value = serde_json::from_str(include_str!("../../contracts/stage4_thresholds.json")).unwrap();
        let proposed: Vec<i64> = thr["thresholds"].as_array().unwrap().iter().find(|t| t["id"] == "S4.rehearsal.slot_presets_min").unwrap()["proposed"].as_array().unwrap().iter().map(|m| m.as_i64().unwrap() * 60).collect();
        assert_eq!(policy().presets_seconds, proposed);
    }

    #[test]
    fn ending_early_closes_the_timeline_and_a_session_without_an_end_has_none() {
        let f = fx();
        let mut sess = new_session(&f, 300, "NONE");
        assert!(sess.timeline(None).unwrap_err().starts_with("CF_REHEARSAL_NOT_ENDED"));
        sess.respond(30_000, f["responses"]["r1"].as_str().unwrap()).unwrap();
        let ev = sess.end_early(50_000);
        assert_eq!(ev.last().unwrap().kind, "SESSION_END");
        let tl = sess.timeline(Some("debrief-1")).unwrap();
        assert_eq!((tl["ended_by"].as_str(), tl["debrief_id"].as_str()), (Some("USER_ENDED"), Some("debrief-1")));
        // the 20 s after the last response go to the limb counsel was on
        assert_eq!(tl["limbs"][0]["time_spent_ms"], 50_000);
        // ending twice, or responding afterwards, changes nothing
        assert!(sess.end_early(60_000).is_empty());
        assert!(sess.respond(70_000, "More about the sale deed.").unwrap_err().starts_with(TIME_UP));
    }

    #[test]
    fn a_response_costs_the_screen_no_perceptible_time() {
        // AC-3.3: nothing here calls a model, so a response adds microseconds, not seconds
        let f = fx();
        let mut sess = new_session(&f, 600, "HIGH");
        let oral = f["responses"]["oral"].as_str().unwrap();
        let start = Instant::now();
        for i in 1..=50 {
            sess.respond(i * 1000, oral).unwrap();
        }
        assert!(start.elapsed().as_millis() < 200, "{} ms for 50 responses", start.elapsed().as_millis());
    }

    #[test]
    fn the_timeline_validates_against_the_frozen_schema_and_nothing_here_can_reach_a_model() {
        let f = fx();
        let mut sess = new_session(&f, 300, "MEDIUM");
        for r in f["session_five_minutes"]["responses"].as_array().unwrap() {
            sess.respond(r["at_s"].as_i64().unwrap() * 1000, f["responses"][r["key"].as_str().unwrap()].as_str().unwrap()).unwrap();
        }
        sess.observe(301_000);
        let tl = sess.timeline(None).unwrap();
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(std::path::Path::new(&dir).join("rehearsal_timeline.json"), serde_json::to_string_pretty(&tl).unwrap()).unwrap();
            let findings: Vec<Value> = sess.findings().iter().map(|g| json!({"text": g.text, "source_kind": g.source_kind, "limb_node_id": g.limb_node_id, "event_index": g.event_index})).collect();
            std::fs::write(std::path::Path::new(&dir).join("rehearsal_findings.json"), serde_json::to_string_pretty(&findings).unwrap()).unwrap();
        }
        let src = include_str!("timed_rehearsal.rs");
        let code = src.split("#[cfg(test)]").next().unwrap();
        for tok in ["infer_llm", "chat_with_timeout", "llm_chat", "reqwest", "TcpStream", "ollama"] {
            assert!(!code.replace("no model in this module", "").lines().filter(|l| !l.trim_start().starts_with("//")).any(|l| l.contains(tok)), "{tok}");
        }
    }
}
