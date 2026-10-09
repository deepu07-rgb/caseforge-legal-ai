//! Deadline & Limitation Watch date engine (B-S4-11, PRD v4.0 §7.6, G9).
//!
//! Computes a deadline from three things only: a signed rule row (period, counting convention, exclusions, calendar),
//! a trigger event with a locator in the matter, and a court calendar. No model, no network, no hidden defaults: every
//! rule used, every exclusion applied and every roll-forward is reported, so a lawyer can re-derive the date by hand.
//!
//! - Periods in days, months or years. A month or year ends on the corresponding date; where the target month is
//!   shorter the last day of that month is used (31 Jan + 1 month = 28/29 Feb; 29 Feb + 1 year = 28 Feb).
//! - `exclude_trigger_day`: the trigger day is not counted (day 1 is the next day), so the period ends n days / months /
//!   years after it; otherwise the trigger day is day 1 and the period ends one day earlier.
//! - Exclusions (time spent obtaining a copy, ...) apply only when their triggering fact exists in the matter and is
//!   confirmed; each one lengthens the period by its stated days. A present but unconfirmed fact gives a range.
//! - If the rule says so and the last day falls on a day the court is closed, the date rolls forward to the next open day
//!   (consecutive holidays, across a year boundary), using only the calendar rows supplied.
//! - Anything uncertain (imprecise or conflicting trigger date, unconfirmed exclusion, missing calendar year, a rule that
//!   changes inside the trigger range) gives a date RANGE with the reasons, never a single date (AC-S4-06.2).
//! - No deadline without a rule, an authority and a trigger locator (AC-S4-06.3); every record is labelled VERIFY (G9).
//!
//! Contract: contracts/deadline_record.schema.json. Golden: golden/G-DEADLINE (synthetic rows: the arithmetic and refusals
//! are proven; legal-editor agreement on real rows, AC-S4-06.1, needs signed rows and stays open).

use crate::rule_tables::RuleRow;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const ENGINE_VERSION: &str = "caseforge.deadlines.v1";
/// When a calendar year is missing the latest the date could fall is shown as this many days after the unrolled date.
/// An upper bound for display, not a legal rule: the true date depends on the court's calendar.
pub const UNKNOWN_CALENDAR_SPAN_DAYS: i64 = 14;
const MAX_ROLL_DAYS: i64 = 400;
const REASON_ORDER: [&str; 5] = ["TRIGGER_DATE_IMPRECISE", "TRIGGER_DATE_CONFLICT", "CALENDAR_MISSING", "EXCLUSION_FACT_UNCONFIRMED", "RULE_WINDOW_AMBIGUOUS"];

// ---------------------------------------------------------------------------------------------------------------------
// Civil dates (proleptic Gregorian) — no external date crate.

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Date {
    y: i32,
    m: u32,
    d: u32,
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i32, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ => {
            if is_leap(y) {
                29
            } else {
                28
            }
        }
    }
}

impl Date {
    pub fn ymd(y: i32, m: u32, d: u32) -> Option<Date> {
        ((1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m) && (1..=9999).contains(&y)).then_some(Date { y, m, d })
    }

    /// Strict `YYYY-MM-DD`.
    pub fn parse(s: &str) -> Option<Date> {
        let b = s.as_bytes();
        if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !s.is_ascii() {
            return None;
        }
        Date::ymd(s.get(0..4)?.parse().ok()?, s.get(5..7)?.parse().ok()?, s.get(8..10)?.parse().ok()?)
    }

    pub fn year(self) -> i32 {
        self.y
    }

    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.y, self.m, self.d)
    }

    /// Days since 1970-01-01.
    fn ordinal(self) -> i64 {
        let y = i64::from(self.y) - i64::from(self.m <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (i64::from(self.m) + 9) % 12;
        let doy = (153 * mp + 2) / 5 + i64::from(self.d) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    fn from_ordinal(z: i64) -> Date {
        let z = z + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let y = (yoe + era * 400 + i64::from(m <= 2)) as i32;
        Date { y, m, d }
    }

    pub fn add_days(self, n: i64) -> Date {
        Date::from_ordinal(self.ordinal() + n)
    }

    /// The corresponding date `n` months later; the last day of the month when that day does not exist there.
    pub fn add_months(self, n: i64) -> Date {
        let total = i64::from(self.y) * 12 + i64::from(self.m) - 1 + n;
        let (y, m) = (total.div_euclid(12) as i32, (total.rem_euclid(12) + 1) as u32);
        Date { y, m, d: self.d.min(days_in_month(y, m)) }
    }

    /// 0 = Monday ... 6 = Sunday.
    pub fn weekday(self) -> u32 {
        (self.ordinal() + 3).rem_euclid(7) as u32
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Inputs

#[derive(Clone, Debug)]
pub struct Locator {
    pub doc_id: String,
    pub excerpt: String,
}

#[derive(Clone, Debug)]
pub struct Trigger {
    pub event_node_id: String,
    pub event_type: String,
    pub date: Option<String>,
    /// DAY, MONTH, YEAR, RANGE or UNKNOWN.
    pub precision: String,
    pub range: Option<(String, String)>,
    /// Other dates the matter's sources give for the same event.
    pub conflicting: Vec<String>,
    pub locator: Locator,
    pub origin: String,
}

#[derive(Clone, Debug)]
pub struct Fact {
    pub fact_type: String,
    pub node_id: String,
    pub days: Option<u32>,
    pub confirmed: bool,
    pub locator: Locator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Days,
    Months,
    Years,
}

impl Unit {
    fn name(self) -> &'static str {
        match self {
            Unit::Days => "DAYS",
            Unit::Months => "MONTHS",
            Unit::Years => "YEARS",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExclusionRule {
    pub id: String,
    pub fact_type: String,
}

#[derive(Clone, Debug)]
pub struct Rule {
    pub rule_id: String,
    pub effective_from: String,
    pub effective_to: Option<String>,
    pub authorities: Vec<Value>,
    pub trust_state: String,
    pub label: String,
    pub trigger_event_type: String,
    pub value: i64,
    pub unit: Unit,
    pub exclude_trigger_day: bool,
    pub roll_forward: bool,
    pub calendar_table_id: Option<String>,
    pub exclusions: Vec<ExclusionRule>,
}

impl Rule {
    pub fn from_parts(rule_id: &str, effective_from: &str, effective_to: Option<&str>, authorities: &[Value], trust_state: &str, payload: &Value) -> Result<Rule, String> {
        let bad = |why: &str| format!("CF_DEADLINE_BAD_RULE: {rule_id}: {why}");
        let s = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        let period = payload.get("period").ok_or_else(|| bad("no period"))?;
        let counting = payload.get("counting").ok_or_else(|| bad("no counting"))?;
        let unit = match period.get("unit").and_then(Value::as_str) {
            Some("DAYS") => Unit::Days,
            Some("MONTHS") => Unit::Months,
            Some("YEARS") => Unit::Years,
            _ => return Err(bad("period unit must be DAYS, MONTHS or YEARS")),
        };
        let value = period.get("value").and_then(Value::as_i64).filter(|v| *v >= 1).ok_or_else(|| bad("period value must be at least 1"))?;
        let exclusions: Vec<ExclusionRule> = payload
            .get("exclusions")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(|e| Some(ExclusionRule { id: s(e, "exclusion_id")?, fact_type: s(e, "condition_fact_type")? })).collect())
            .unwrap_or_default();
        Ok(Rule {
            rule_id: rule_id.to_string(),
            effective_from: effective_from.to_string(),
            effective_to: effective_to.map(str::to_string),
            authorities: authorities.to_vec(),
            trust_state: trust_state.to_string(),
            label: s(payload, "deadline_label").ok_or_else(|| bad("no deadline_label"))?,
            trigger_event_type: s(payload, "trigger_event_type").ok_or_else(|| bad("no trigger_event_type"))?,
            value,
            unit,
            exclude_trigger_day: counting.get("exclude_trigger_day").and_then(Value::as_bool).ok_or_else(|| bad("no exclude_trigger_day"))?,
            roll_forward: counting.get("roll_forward_if_court_closed").and_then(Value::as_bool).ok_or_else(|| bad("no roll_forward_if_court_closed"))?,
            calendar_table_id: s(counting, "calendar_table_id"),
            exclusions,
        })
    }

    pub fn from_row(r: &RuleRow) -> Result<Rule, String> {
        Rule::from_parts(&r.rule_id, &r.effective_from, r.effective_to.as_deref(), &r.authorities, &r.trust_state, &r.payload)
    }

    fn in_force(&self, d: Date) -> bool {
        Date::parse(&self.effective_from).map(|f| f <= d).unwrap_or(false) && self.effective_to.as_deref().map(|t| Date::parse(t).map(|t| d <= t).unwrap_or(false)).unwrap_or(true)
    }
}

/// Court calendars by table id: the days a court is closed, and which years are covered at all.
#[derive(Default, Debug)]
pub struct Calendars {
    tables: BTreeMap<String, TableDays>,
}

#[derive(Default, Debug)]
struct TableDays {
    covered_years: BTreeSet<i32>,
    closed: BTreeSet<Date>,
}

impl Calendars {
    /// Adds one COURT_CALENDAR row payload (court_id, year, closed_dates, closed_ranges) to table `table_id`.
    pub fn add(&mut self, table_id: &str, payload: &Value) -> Result<(), String> {
        let t = self.tables.entry(table_id.to_string()).or_default();
        let year = payload.get("year").and_then(Value::as_i64).ok_or("CF_DEADLINE_BAD_CALENDAR: no year")? as i32;
        t.covered_years.insert(year);
        for d in payload.get("closed_dates").and_then(Value::as_array).into_iter().flatten() {
            t.closed.insert(d.get("date").and_then(Value::as_str).and_then(Date::parse).ok_or("CF_DEADLINE_BAD_CALENDAR: bad closed date")?);
        }
        for r in payload.get("closed_ranges").and_then(Value::as_array).into_iter().flatten() {
            let (a, b) = (r.get("from").and_then(Value::as_str).and_then(Date::parse), r.get("to").and_then(Value::as_str).and_then(Date::parse));
            let (a, b) = a.zip(b).filter(|(a, b)| a <= b && b.ordinal() - a.ordinal() <= 400).ok_or("CF_DEADLINE_BAD_CALENDAR: bad closed range")?;
            for n in 0..=(b.ordinal() - a.ordinal()) {
                t.closed.insert(a.add_days(n));
            }
        }
        Ok(())
    }

    /// None when the table or the date's year is not covered.
    fn is_closed(&self, table_id: &str, d: Date) -> Option<bool> {
        let t = self.tables.get(table_id)?;
        t.covered_years.contains(&d.year()).then(|| t.closed.contains(&d))
    }
}

/// First and last day of a `YYYY-MM` or `YYYY` expression.
fn partial_span(s: &str) -> Option<(Date, Date)> {
    let s = s.trim();
    match s.len() {
        7 if s.as_bytes()[4] == b'-' => {
            let (y, m): (i32, u32) = (s.get(0..4)?.parse().ok()?, s.get(5..7)?.parse().ok()?);
            Some((Date::ymd(y, m, 1)?, Date::ymd(y, m, days_in_month(y, m))?))
        }
        4 => {
            let y: i32 = s.parse().ok()?;
            Some((Date::ymd(y, 1, 1)?, Date::ymd(y, 12, 31)?))
        }
        _ => None,
    }
}

/// The trigger a case-graph chronology event gives to a rule that runs from `rule_trigger_type` (counsel assigned the
/// event to that rule). Precision, ambiguity and the source location come from the event itself, never from a guess:
/// an unresolved or relative date is UNKNOWN, a month or year is a range, an ambiguous or disputed day keeps every
/// candidate date so the result becomes a range.
pub fn trigger_from_event(event: &Value, rule_trigger_type: &str) -> Trigger {
    let dx = &event["date_expression"];
    let precision = dx["precision"].as_str().or(event["date_precision"].as_str()).unwrap_or("unknown");
    let status = dx["status"].as_str().unwrap_or("resolved");
    let resolved = dx["resolved"].as_str().filter(|s| !s.is_empty()).or(event["date"].as_str()).unwrap_or("");
    let candidates: Vec<String> = dx["candidates"].as_array().into_iter().flatten().filter_map(Value::as_str).filter(|c| Date::parse(c).is_some()).map(str::to_string).collect();
    let unknown = || ("UNKNOWN", None, None, Vec::new());
    let (prec, date, range, conflicting): (&str, Option<String>, Option<(String, String)>, Vec<String>) = if status == "unresolved" {
        unknown()
    } else {
        match precision {
            "day" => {
                let first = Date::parse(resolved).map(Date::iso).or_else(|| candidates.first().cloned());
                match first {
                    Some(d) => ("DAY", Some(d.clone()), None, candidates.iter().filter(|c| **c != d).cloned().collect()),
                    None => unknown(),
                }
            }
            "month" | "year" => match partial_span(resolved) {
                Some((a, b)) => (if precision == "month" { "MONTH" } else { "YEAR" }, None, Some((a.iso(), b.iso())), Vec::new()),
                None => unknown(),
            },
            "range" => match dx["range_start"].as_str().and_then(Date::parse).zip(dx["range_end"].as_str().and_then(Date::parse)) {
                Some((a, b)) => ("RANGE", None, Some((a.iso(), b.iso())), Vec::new()),
                None => unknown(),
            },
            _ => unknown(),
        }
    };
    let source = &event["provenance"]["sources"][0];
    let quote = source["quote"].as_str().filter(|q| !q.trim().is_empty()).or(event["summary"].as_str()).unwrap_or("");
    let origin = match event["provenance"]["origin"].as_str() {
        Some(o @ ("SOURCED_BY_COUNSEL" | "USER_EDIT")) => o,
        _ => "EXTRACTED",
    };
    Trigger {
        event_node_id: event["event_id"].as_str().unwrap_or("").to_string(),
        event_type: rule_trigger_type.to_string(),
        date,
        precision: prec.to_string(),
        range,
        conflicting,
        locator: Locator { doc_id: source["doc_id"].as_str().unwrap_or("").to_string(), excerpt: quote.to_string() },
        origin: origin.to_string(),
    }
}

pub struct Inputs<'a> {
    pub case_id: &'a str,
    pub rules: &'a [Rule],
    pub trigger: &'a Trigger,
    pub facts: &'a [Fact],
    pub calendars: &'a Calendars,
    pub computed_at: &'a str,
}

// ---------------------------------------------------------------------------------------------------------------------
// Computation

struct Outcome {
    lo: Date,
    hi: Date,
    rolled_from: Option<Date>,
}

struct Applied {
    exclusion_id: String,
    days: i64,
    fact: Fact,
}

fn period_end(rule: &Rule, trigger: Date) -> Date {
    let end = match rule.unit {
        Unit::Days => trigger.add_days(rule.value),
        Unit::Months => trigger.add_months(rule.value),
        Unit::Years => trigger.add_months(12 * rule.value),
    };
    if rule.exclude_trigger_day {
        end
    } else {
        end.add_days(-1)
    }
}

/// Rolls forward over closed days. Returns the open date, the date it started from if it moved, and whether the calendar
/// ran out (then the date is a lower bound and `UNKNOWN_CALENDAR_SPAN_DAYS` bounds the other side).
fn roll(rule: &Rule, cals: &Calendars, end: Date) -> (Date, Option<Date>, bool) {
    if !rule.roll_forward {
        return (end, None, false);
    }
    let Some(table) = rule.calendar_table_id.as_deref() else { return (end, None, true) };
    let mut d = end;
    for _ in 0..=MAX_ROLL_DAYS {
        match cals.is_closed(table, d) {
            None => return (d, (d != end).then_some(end), true),
            Some(false) => return (d, (d != end).then_some(end), false),
            Some(true) => d = d.add_days(1),
        }
    }
    (d, Some(end), true)
}

fn sort_reasons(set: BTreeSet<&'static str>) -> Vec<&'static str> {
    REASON_ORDER.iter().copied().filter(|r| set.contains(r)).collect()
}

/// One deadline record (contracts/deadline_record.schema.json), or the reason there is none.
pub fn compute(i: &Inputs) -> Result<Value, String> {
    let t = i.trigger;
    let mut reasons: BTreeSet<&'static str> = BTreeSet::new();

    // Trigger interval.
    let (mut lo, mut hi) = match t.precision.as_str() {
        "DAY" => {
            let d = t.date.as_deref().and_then(Date::parse).ok_or("CF_DEADLINE_TRIGGER_UNKNOWN: the trigger date is missing or not a valid date")?;
            (d, d)
        }
        "MONTH" | "YEAR" | "RANGE" => {
            let (a, b) = t.range.as_ref().and_then(|(a, b)| Date::parse(a).zip(Date::parse(b))).filter(|(a, b)| a <= b).ok_or("CF_DEADLINE_TRIGGER_UNKNOWN: the trigger date range is missing or invalid")?;
            reasons.insert("TRIGGER_DATE_IMPRECISE");
            (a, b)
        }
        _ => return Err("CF_DEADLINE_TRIGGER_UNKNOWN: the trigger date is not known".into()),
    };
    for other in &t.conflicting {
        let d = Date::parse(other).ok_or("CF_DEADLINE_TRIGGER_UNKNOWN: a conflicting trigger date is not a valid date")?;
        if d < lo || d > hi {
            reasons.insert("TRIGGER_DATE_CONFLICT");
        }
        lo = lo.min(d);
        hi = hi.max(d);
    }
    if t.locator.doc_id.trim().is_empty() || t.locator.excerpt.trim().is_empty() {
        return Err("CF_DEADLINE_NO_LOCATOR: a deadline needs the document and excerpt that give the trigger date".into());
    }
    if i.rules.is_empty() {
        return Err("CF_DEADLINE_NO_RULE: no rule applies to this trigger".into());
    }
    if i.rules.iter().any(|r| r.authorities.is_empty() || r.authorities.iter().any(|a| ["pack_id", "source_id", "chunk_id"].iter().any(|k| a.get(*k).and_then(Value::as_str).unwrap_or("").is_empty()))) {
        return Err("CF_DEADLINE_NO_AUTHORITY: the rule cites no installed authority".into());
    }

    // The rule in force on each end of the interval.
    let pick = |d: Date| i.rules.iter().find(|r| r.in_force(d));
    let (r_lo, r_hi) = (pick(lo).ok_or("CF_DEADLINE_NO_RULE: no rule is in force on the trigger date")?, pick(hi).ok_or("CF_DEADLINE_NO_RULE: no rule is in force on the trigger date")?);
    for r in [r_lo, r_hi] {
        if r.trigger_event_type != t.event_type {
            return Err(format!("CF_DEADLINE_TRIGGER_TYPE: the rule runs from a {} but the trigger is a {}", r.trigger_event_type, t.event_type));
        }
    }
    if !std::ptr::eq(r_lo, r_hi) {
        reasons.insert("RULE_WINDOW_AMBIGUOUS");
    }

    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut applied_first: Vec<Applied> = Vec::new();
    for (n, (rule, date)) in [(r_lo, lo), (r_hi, hi)].into_iter().enumerate() {
        if n == 1 && lo == hi && std::ptr::eq(r_lo, r_hi) {
            break;
        }
        // Each exclusion: absent -> not applied; confirmed -> applied; present but unconfirmed -> both possibilities.
        let mut variants: Vec<(i64, Vec<Applied>)> = vec![(0, Vec::new())];
        for ex in &rule.exclusions {
            let Some(f) = i.facts.iter().find(|f| f.fact_type == ex.fact_type) else { continue };
            let days = f.days.map(i64::from).ok_or_else(|| format!("CF_DEADLINE_EXCLUSION_DAYS_MISSING: {} is recorded without the number of days", ex.fact_type))?;
            let mut next = Vec::new();
            for (extra, applied) in variants {
                if f.confirmed {
                    let mut a = applied;
                    a.push(Applied { exclusion_id: ex.id.clone(), days, fact: f.clone() });
                    next.push((extra + days, a));
                } else {
                    reasons.insert("EXCLUSION_FACT_UNCONFIRMED");
                    next.push((extra + days, applied.iter().map(|a| Applied { exclusion_id: a.exclusion_id.clone(), days: a.days, fact: a.fact.clone() }).collect()));
                    next.push((extra, applied));
                }
            }
            variants = next;
        }
        let base = period_end(rule, date);
        for (extra, applied) in variants {
            let (open, rolled, missing) = roll(rule, i.calendars, base.add_days(extra));
            if missing {
                reasons.insert("CALENDAR_MISSING");
            }
            outcomes.push(Outcome { lo: open, hi: if missing { open.add_days(UNKNOWN_CALENDAR_SPAN_DAYS) } else { open }, rolled_from: rolled });
            if n == 0 && applied_first.is_empty() {
                applied_first = applied.into_iter().filter(|a| a.fact.confirmed).collect();
            }
        }
    }
    let first = outcomes.iter().map(|o| o.lo).min().ok_or("CF_DEADLINE_NO_RULE: nothing to compute")?;
    let last = outcomes.iter().map(|o| o.hi).max().ok_or("CF_DEADLINE_NO_RULE: nothing to compute")?;
    let reasons = sort_reasons(reasons);
    let is_range = !reasons.is_empty();

    let authority = {
        let mut a = r_lo.authorities[0].clone();
        a["rule_trust_state"] = json!(r_lo.trust_state);
        a
    };
    let trigger_json = json!({
        "event_node_id": t.event_node_id, "event_type": t.event_type,
        "date": if t.precision == "DAY" { json!(t.date) } else { Value::Null },
        "date_precision": t.precision,
        "date_range": if t.precision == "DAY" { Value::Null } else { json!({"from": lo.iso(), "to": hi.iso()}) },
        "locator": {"doc_id": t.locator.doc_id, "excerpt": t.locator.excerpt}, "origin": t.origin,
    });
    let id_seed = format!("{}|{}|{}|{}|{}|{}", i.case_id, r_lo.rule_id, t.event_node_id, first.iso(), last.iso(), reasons.join(","));
    Ok(json!({
        "schema_version": "1.0.0",
        "deadline_id": format!("dl_{}", &hex::encode(Sha256::digest(id_seed.as_bytes()))[..16]),
        "case_id": i.case_id,
        "label": r_lo.label,
        "rule_id": r_lo.rule_id,
        "authority": authority,
        "trigger": trigger_json,
        "computation": {
            "period": {"value": r_lo.value, "unit": r_lo.unit.name()},
            "exclude_trigger_day": r_lo.exclude_trigger_day,
            "exclusions_applied": applied_first.iter().map(|a| json!({"exclusion_id": a.exclusion_id, "rule_id": r_lo.rule_id, "days_excluded": a.days, "triggering_fact_node_id": a.fact.node_id, "locator": {"doc_id": a.fact.locator.doc_id, "excerpt": a.fact.locator.excerpt}})).collect::<Vec<_>>(),
            "calendar_rule_ids": if r_lo.roll_forward { r_lo.calendar_table_id.iter().cloned().collect::<Vec<_>>() } else { vec![] },
            "rolled_forward_from": if is_range { Value::Null } else { json!(outcomes.first().and_then(|o| o.rolled_from).map(Date::iso)) },
        },
        "result": if is_range {
            json!({"kind": "RANGE", "date": Value::Null, "range_from": first.iso(), "range_to": last.iso(), "uncertainty_reasons": reasons})
        } else {
            json!({"kind": "DATE", "date": first.iso(), "range_from": Value::Null, "range_to": Value::Null, "uncertainty_reasons": []})
        },
        "verify_label": "VERIFY",
        "guardrail": "G9",
        "computed_at": i.computed_at,
        "engine_version": ENGINE_VERSION,
    }))
}

// ---------------------------------------------------------------------------------------------------------------------
// iCalendar export (RFC 5545), local file only

fn ics_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace(';', "\\;").replace(',', "\\,").replace("\r\n", "\\n").replace('\n', "\\n").replace('\r', "")
}

/// Folds a content line at 75 octets, continuing with a leading space, never splitting a UTF-8 character.
fn ics_fold(line: &str) -> String {
    let (mut out, mut width, mut limit) = (String::new(), 0usize, 75usize);
    for ch in line.chars() {
        let w = ch.len_utf8();
        if width + w > limit {
            out.push_str("\r\n ");
            width = 1;
            limit = 75;
        }
        out.push(ch);
        width += w;
    }
    out
}

fn ics_stamp(rfc3339: &str) -> String {
    rfc3339.chars().filter(|c| c.is_ascii_digit() || *c == 'T').take(15).collect::<String>() + "Z"
}

/// All-day calendar events for the given deadline records; a range becomes one event spanning it. Two display alarms
/// (7 days and 1 day before the earliest possible date) make the file a local reminder. Contains no network address.
pub fn to_ics(records: &[Value], now_rfc3339: &str) -> String {
    let mut lines: Vec<String> = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//CaseForge//Deadline Watch//EN", "CALSCALE:GREGORIAN", "METHOD:PUBLISH"].iter().map(|s| s.to_string()).collect();
    for r in records {
        let res = &r["result"];
        let (from, to) = if res["kind"] == "DATE" { (res["date"].as_str(), res["date"].as_str()) } else { (res["range_from"].as_str(), res["range_to"].as_str()) };
        let (Some(from_d), Some(to_d)) = (from.and_then(Date::parse), to.and_then(Date::parse)) else { continue };
        let range = res["kind"] == "RANGE";
        let reasons = res["uncertainty_reasons"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let provision = r["authority"]["provision_label"].as_str().filter(|s| !s.is_empty()).unwrap_or(r["authority"]["chunk_id"].as_str().unwrap_or(""));
        let when = if range { format!("between {} and {} ({})", from_d.iso(), to_d.iso(), reasons) } else { format!("on {}", from_d.iso()) };
        let desc = format!(
            "VERIFY before relying on this date. {} falls {}. Rule: {}. Authority: {}. Trigger: {} ({}). Computed by CaseForge from the rule, the trigger and the court calendar; a computation to check, not legal advice.",
            r["label"].as_str().unwrap_or(""),
            when,
            r["rule_id"].as_str().unwrap_or(""),
            provision,
            r["trigger"]["event_type"].as_str().unwrap_or(""),
            r["trigger"]["locator"]["doc_id"].as_str().unwrap_or(""),
        );
        lines.push("BEGIN:VEVENT".into());
        lines.push(format!("UID:{}@caseforge.local", r["deadline_id"].as_str().unwrap_or("deadline")));
        lines.push(format!("DTSTAMP:{}", ics_stamp(now_rfc3339)));
        lines.push(format!("DTSTART;VALUE=DATE:{}", from_d.iso().replace('-', "")));
        lines.push(format!("DTEND;VALUE=DATE:{}", to_d.add_days(1).iso().replace('-', "")));
        lines.push(format!("SUMMARY:{}", ics_escape(&format!("VERIFY: {}{}", r["label"].as_str().unwrap_or("Deadline"), if range { " (date range)" } else { "" }))));
        lines.push(format!("DESCRIPTION:{}", ics_escape(&desc)));
        lines.push("CATEGORIES:DEADLINE".into());
        lines.push("TRANSP:TRANSPARENT".into());
        for (trigger, text) in [("-P7D", "in 7 days"), ("-P1D", "tomorrow")] {
            lines.extend(["BEGIN:VALARM".to_string(), "ACTION:DISPLAY".to_string(), format!("DESCRIPTION:{}", ics_escape(&format!("VERIFY deadline {text}: {}", r["label"].as_str().unwrap_or("")))), format!("TRIGGER:{trigger}"), "END:VALARM".to_string()]);
        }
        lines.push("END:VEVENT".into());
    }
    lines.push("END:VCALENDAR".into());
    lines.iter().map(|l| ics_fold(l)).collect::<Vec<_>>().join("\r\n") + "\r\n"
}

// ---------------------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const FIXTURE: &str = include_str!("../../golden/G-DEADLINE/fixtures/deadline_core.json");
    const FIXTURE_SHA256: &str = "ae4ac55da440f46a928ee5b4a5065561a1ff89ee1bf8dbd9beb017e81b675fbb"; // committed before the engine (810a929)

    fn locator(v: &Value) -> Locator {
        Locator { doc_id: v["doc_id"].as_str().unwrap_or("").into(), excerpt: v["excerpt"].as_str().unwrap_or("").into() }
    }

    fn rules(fx: &Value, key: &str) -> Vec<Rule> {
        fx["rules"][key]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| Rule::from_parts(r["rule_id"].as_str().unwrap(), r["effective_from"].as_str().unwrap(), r["effective_to"].as_str(), r["authorities"].as_array().unwrap(), r["trust_state"].as_str().unwrap(), &r["payload"]).unwrap())
            .collect()
    }

    fn calendars(fx: &Value) -> Calendars {
        let mut c = Calendars::default();
        for (table, list) in fx["calendars"].as_object().unwrap() {
            for p in list.as_array().unwrap() {
                c.add(table, p).unwrap();
            }
        }
        c
    }

    fn trigger(v: &Value) -> Trigger {
        Trigger {
            event_node_id: v["event_node_id"].as_str().unwrap().into(),
            event_type: v["event_type"].as_str().unwrap().into(),
            date: v["date"].as_str().map(str::to_string),
            precision: v["date_precision"].as_str().unwrap().into(),
            range: v["date_range"].as_object().map(|r| (r["from"].as_str().unwrap().into(), r["to"].as_str().unwrap().into())),
            conflicting: v["conflicting_dates"].as_array().unwrap().iter().map(|d| d.as_str().unwrap().into()).collect(),
            locator: locator(&v["locator"]),
            origin: v["origin"].as_str().unwrap().into(),
        }
    }

    fn facts(v: &Value) -> Vec<Fact> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|f| Fact { fact_type: f["fact_type"].as_str().unwrap().into(), node_id: f["node_id"].as_str().unwrap().into(), days: f["days"].as_u64().map(|d| d as u32), confirmed: f["confirmed"].as_bool().unwrap(), locator: locator(&f["locator"]) })
            .collect()
    }

    #[test]
    fn golden_deadline_cases() {
        assert_eq!(hex::encode(Sha256::digest(FIXTURE.as_bytes())), FIXTURE_SHA256, "G-DEADLINE fixture changed since it was pinned");
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let cals = calendars(&fx);
        let mut records = Vec::new();
        for c in fx["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let rs = rules(&fx, c["rules"].as_str().unwrap());
            let (trig, fs) = (trigger(&c["trigger"]), facts(&c["facts"]));
            let out = compute(&Inputs { case_id: "case-1", rules: &rs, trigger: &trig, facts: &fs, calendars: &cals, computed_at: "2026-10-09T09:00:00Z" });
            if let Some(code) = c["error"].as_str() {
                let e = out.expect_err(name);
                assert!(e.starts_with(code), "{name}: expected {code}, got {e}");
                continue;
            }
            let rec = out.unwrap_or_else(|e| panic!("{name}: {e}"));
            let x = &c["expect"];
            let r = &rec["result"];
            assert_eq!(r["kind"], x["kind"], "{name}: kind");
            assert_eq!(r["date"], x["date"], "{name}: date");
            assert_eq!(r["range_from"], x["range_from"], "{name}: range_from");
            assert_eq!(r["range_to"], x["range_to"], "{name}: range_to");
            assert_eq!(r["uncertainty_reasons"], x["reasons"], "{name}: reasons");
            assert_eq!(rec["computation"]["rolled_forward_from"], x["rolled_forward_from"], "{name}: rolled_forward_from");
            let days: Vec<Value> = rec["computation"]["exclusions_applied"].as_array().unwrap().iter().map(|e| e["days_excluded"].clone()).collect();
            assert_eq!(Value::Array(days), x["exclusion_days"], "{name}: exclusions");
            // Contract invariants (AC-S4-06.2 / 06.3, G9).
            assert_eq!((rec["verify_label"].as_str(), rec["guardrail"].as_str(), rec["engine_version"].as_str()), (Some("VERIFY"), Some("G9"), Some(ENGINE_VERSION)), "{name}");
            assert!(!rec["rule_id"].as_str().unwrap().is_empty() && !rec["authority"]["chunk_id"].as_str().unwrap().is_empty() && !rec["trigger"]["locator"]["doc_id"].as_str().unwrap().is_empty(), "{name}: rule, authority and locator");
            if r["kind"] == "RANGE" {
                assert!(r["range_from"].as_str().unwrap() <= r["range_to"].as_str().unwrap() && !r["uncertainty_reasons"].as_array().unwrap().is_empty(), "{name}");
            } else {
                assert!(r["uncertainty_reasons"].as_array().unwrap().is_empty(), "{name}: a single date carries no uncertainty");
            }
            records.push(rec);
        }
        if let Some(dir) = std::env::var_os("CASEFORGE_EVIDENCE_DIR") {
            std::fs::write(PathBuf::from(dir).join("deadline_records.json"), serde_json::to_vec_pretty(&records).unwrap()).unwrap();
        }
    }

    #[test]
    fn chronology_events_become_triggers_without_guessing() {
        let ev = |dx: Value, date: &str, origin: &str| json!({"event_id": "event_0001", "date": date, "date_precision": dx["precision"], "date_expression": dx, "summary": "The order was passed.", "provenance": {"sources": [{"doc_id": "doc-9", "page_index": 2, "quote": "order passed on 15 January 2024"}], "origin": origin, "counsel_confirmed": false}});
        let dx = |precision: &str, status: &str, resolved: Value, cands: Vec<&str>| json!({"raw": "x", "kind": "exact", "precision": precision, "status": status, "candidates": cands, "resolved": resolved, "range_start": null, "range_end": null});
        let t = trigger_from_event(&ev(dx("day", "resolved", json!("2024-01-15"), vec![]), "2024-01-15", "EXTRACTED"), "ORDER_PASSED");
        assert_eq!((t.precision.as_str(), t.date.as_deref(), t.event_type.as_str(), t.locator.doc_id.as_str(), t.locator.excerpt.as_str()), ("DAY", Some("2024-01-15"), "ORDER_PASSED", "doc-9", "order passed on 15 January 2024"));
        let t = trigger_from_event(&ev(dx("day", "ambiguous", json!(null), vec!["2024-03-04", "2024-04-03"]), "04/03/2024", "EXTRACTED"), "E");
        assert_eq!((t.precision.as_str(), t.date.as_deref(), t.conflicting.as_slice()), ("DAY", Some("2024-03-04"), &["2024-04-03".to_string()][..]), "both readings are kept");
        let t = trigger_from_event(&ev(dx("month", "resolved", json!("2024-02"), vec![]), "2024-02", "EXTRACTED"), "E");
        assert_eq!((t.precision.as_str(), t.range.clone()), ("MONTH", Some(("2024-02-01".into(), "2024-02-29".into()))));
        let t = trigger_from_event(&ev(dx("year", "resolved", json!("2023"), vec![]), "2023", "EXTRACTED"), "E");
        assert_eq!((t.precision.as_str(), t.range.clone()), ("YEAR", Some(("2023-01-01".into(), "2023-12-31".into()))));
        let mut d = dx("range", "resolved", json!(null), vec![]);
        d["range_start"] = json!("2024-05-01");
        d["range_end"] = json!("2024-05-09");
        assert_eq!(trigger_from_event(&ev(d, "x", "EXTRACTED"), "E").range, Some(("2024-05-01".into(), "2024-05-09".into())));
        for (precision, status) in [("relative", "resolved"), ("unknown", "unresolved"), ("day", "unresolved")] {
            let t = trigger_from_event(&ev(dx(precision, status, json!("2024-01-15"), vec![]), "later", "EXTRACTED"), "E");
            assert_eq!(t.precision, "UNKNOWN", "{precision}/{status}");
        }
        assert_eq!(trigger_from_event(&ev(dx("day", "resolved", json!("2024-01-15"), vec![]), "2024-01-15", "SOURCED_BY_COUNSEL"), "E").origin, "SOURCED_BY_COUNSEL");
        assert_eq!(trigger_from_event(&ev(dx("day", "resolved", json!("2024-01-15"), vec![]), "2024-01-15", "INFERRED"), "E").origin, "EXTRACTED", "only counsel origins are kept as such");
        // An event with no source document yields a trigger that compute() refuses (no locator), not a guessed one.
        let mut bare = ev(dx("day", "resolved", json!("2024-01-15"), vec![]), "2024-01-15", "EXTRACTED");
        bare["provenance"]["sources"] = json!([]);
        assert_eq!(trigger_from_event(&bare, "E").locator.doc_id, "");
    }

    #[test]
    fn shipped_limitation_rules_install_cite_the_right_lines_and_compute() {
        // The real development pack through the real installer, loader and store; no stand-ins.
        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../packs/official/caseforge.india.limitation-rules");
        let vault = tempfile::tempdir().unwrap();
        let paths = crate::vault::VaultPaths::new(vault.path().to_path_buf());
        crate::packs::install_pack_from_dir(&paths, &pack).expect("the rules pack installs");
        let (tables, rejected) = crate::rule_tables::load_installed(&paths);
        assert!(rejected.is_empty(), "{rejected:?}");
        let table = tables.iter().find(|t| t.table_kind == "LIMITATION_PERIOD").expect("limitation table");
        assert_eq!(table.rows.len(), 7);
        assert!(table.rows.iter().all(|r| r.trust_state == "DEVELOPMENT_ONLY" && r.signed_by.is_none()), "no row counts as signed off");
        // Each rule's first authority is the exact line of the Schedule it takes its period from.
        let store = crate::retrieval::store::open(&paths).unwrap();
        for r in &table.rows {
            let a = &r.authorities[0];
            let chunk = store.get_by_chunk_id(a["chunk_id"].as_str().unwrap()).expect("cited chunk is installed");
            let cited: String = chunk.text.chars().skip(a["char_start"].as_u64().unwrap() as usize).take((a["char_end"].as_u64().unwrap() - a["char_start"].as_u64().unwrap()) as usize).collect();
            let period = r.payload["period"]["value"].as_u64().unwrap();
            let word = match period {
                10 => "Ten days",
                30 => "Thirty days",
                60 => "Sixty days",
                90 => "Ninety days",
                other => panic!("unexpected period {other}"),
            };
            assert!(cited.contains(word), "{}: '{cited}' should state {word}", r.rule_id);
        }
        // A High Court appeal: 90 days from the order, the time to obtain the copy excluded once confirmed. No court
        // calendar is installed, so the answer is a range whose earliest date is the safe one.
        let rules: Vec<Rule> = table.rows.iter().filter(|r| r.rule_id == "in.limitation.cpc.appeal.high-court").map(|r| Rule::from_row(r).unwrap()).collect();
        let trig = Trigger { event_node_id: "ev-1".into(), event_type: "DECREE_OR_ORDER".into(), date: Some("2024-01-15".into()), precision: "DAY".into(), range: None, conflicting: vec![], locator: Locator { doc_id: "doc-order".into(), excerpt: "Decree dated 15 January 2024".into() }, origin: "EXTRACTED".into() };
        let none = Calendars::default();
        let without = compute(&Inputs { case_id: "c", rules: &rules, trigger: &trig, facts: &[], calendars: &none, computed_at: "2026-10-09T09:00:00Z" }).unwrap();
        assert_eq!((without["result"]["kind"].as_str(), without["result"]["range_from"].as_str(), without["result"]["range_to"].as_str()), (Some("RANGE"), Some("2024-04-14"), Some("2024-04-28")));
        assert_eq!(without["result"]["uncertainty_reasons"], json!(["CALENDAR_MISSING"]));
        assert_eq!(without["authority"]["provision_label"], "Limitation Act Schedule, Art. 116(a)");
        assert_eq!(without["authority"]["rule_trust_state"], "DEVELOPMENT_ONLY");
        let copy = Fact { fact_type: "COPY_OBTAINING_TIME".into(), node_id: "fact-1".into(), days: Some(12), confirmed: true, locator: Locator { doc_id: "doc-copy".into(), excerpt: "Copy applied 2 Feb, ready 14 Feb".into() } };
        let with = compute(&Inputs { case_id: "c", rules: &rules, trigger: &trig, facts: &[copy], calendars: &none, computed_at: "2026-10-09T09:00:00Z" }).unwrap();
        assert_eq!((with["result"]["range_from"].as_str(), with["computation"]["exclusions_applied"][0]["days_excluded"].as_u64()), (Some("2024-04-26"), Some(12)));
        // With the court's calendar the answer is a single date; a Sunday end rolls to the next open day.
        let mut cal = Calendars::default();
        cal.add("t", &json!({"court_id": "C", "year": 2024, "closed_dates": [{"date": "2024-04-14", "label": "Sunday"}], "closed_ranges": []})).unwrap();
        let mut rule = rules[0].clone();
        rule.calendar_table_id = Some("t".into());
        let dated = compute(&Inputs { case_id: "c", rules: &[rule], trigger: &trig, facts: &[], calendars: &cal, computed_at: "2026-10-09T09:00:00Z" }).unwrap();
        assert_eq!((dated["result"]["kind"].as_str(), dated["result"]["date"].as_str(), dated["computation"]["rolled_forward_from"].as_str()), (Some("DATE"), Some("2024-04-15"), Some("2024-04-14")));
    }

    #[test]
    fn date_arithmetic_matches_day_by_day_counting() {
        // Independent check: count day by day with month lengths, never through ordinals.
        let step = |d: Date| if d.d < days_in_month(d.y, d.m) { Date { d: d.d + 1, ..d } } else if d.m < 12 { Date { m: d.m + 1, d: 1, ..d } } else { Date { y: d.y + 1, m: 1, d: 1 } };
        let mut d = Date::ymd(1996, 1, 1).unwrap();
        let mut seen = 0;
        while d.y < 2033 {
            if d.d == 1 || d.d == 28 || d.d == 29 || d.d == 31 || d.d == days_in_month(d.y, d.m) {
                for n in [0i64, 1, 30, 31, 90, 365, 366, 1461] {
                    let mut walk = d;
                    for _ in 0..n {
                        walk = step(walk);
                    }
                    assert_eq!(d.add_days(n), walk, "{} + {n} days", d.iso());
                    assert_eq!(walk.add_days(-n), d, "{} - {n} days", walk.iso());
                }
                seen += 1;
            }
            d = step(d);
        }
        assert!(seen > 1000);
        assert_eq!((Date::ymd(1900, 2, 29), Date::ymd(2000, 2, 29).map(Date::iso), Date::ymd(2023, 2, 29), Date::ymd(2024, 2, 29).map(Date::iso)), (None, Some("2000-02-29".into()), None, Some("2024-02-29".into())));
        assert_eq!(Date::parse("2024-02-30"), None);
        assert_eq!(Date::parse("2024-2-3"), None);
        assert_eq!(Date::parse("2024-13-01"), None);
        assert_eq!(Date::ymd(2024, 7, 4).unwrap().weekday(), 3, "4 July 2024 was a Thursday");
        assert_eq!(Date::ymd(1970, 1, 1).unwrap().ordinal(), 0);
    }

    #[test]
    fn month_arithmetic_clamps_and_is_monotonic() {
        for y in [1999, 2000, 2023, 2024, 2100] {
            for m in 1..=12u32 {
                for day in [1u32, 15, 28, 29, 30, 31] {
                    let Some(d) = Date::ymd(y, m, day) else { continue };
                    for n in [1i64, 2, 11, 12, 13, 24, 36, 120] {
                        let r = d.add_months(n);
                        assert_eq!(r.d, day.min(days_in_month(r.y, r.m)), "{} + {n} months", d.iso());
                        assert!(r > d);
                        assert_eq!((i64::from(r.y) * 12 + i64::from(r.m)) - (i64::from(d.y) * 12 + i64::from(d.m)), n);
                    }
                }
            }
        }
        // Later trigger, never an earlier result.
        let mut prev = Date::ymd(2023, 1, 1).unwrap().add_months(1);
        let mut d = Date::ymd(2023, 1, 1).unwrap();
        for _ in 0..800 {
            d = d.add_days(1);
            let r = d.add_months(1);
            assert!(r >= prev, "{} -> {}", d.iso(), r.iso());
            prev = r;
        }
    }

    #[test]
    fn roll_forward_never_lands_on_a_closed_day_or_before_the_start() {
        let mut cals = Calendars::default();
        cals.add("t", &json!({"court_id": "C", "year": 2024, "closed_dates": [], "closed_ranges": [{"from": "2024-03-01", "to": "2024-03-20", "label": "long vacation"}, {"from": "2024-12-25", "to": "2025-01-05", "label": "winter"}]})).unwrap();
        cals.add("t", &json!({"court_id": "C", "year": 2025, "closed_dates": [{"date": "2025-01-06", "label": "extra"}], "closed_ranges": []})).unwrap();
        let rule = Rule::from_parts("r", "2000-01-01", None, &[json!({"pack_id": "p", "source_id": "s", "chunk_id": "c"})], "DEVELOPMENT_ONLY", &json!({"deadline_label": "x", "trigger_event_type": "E", "period": {"value": 1, "unit": "DAYS"}, "counting": {"exclude_trigger_day": true, "roll_forward_if_court_closed": true, "calendar_table_id": "t"}, "exclusions": []})).unwrap();
        let mut d = Date::ymd(2024, 1, 1).unwrap();
        while d < Date::ymd(2024, 12, 31).unwrap() {
            let (open, from, missing) = roll(&rule, &cals, d);
            if !missing {
                assert_eq!(cals.is_closed("t", open), Some(false), "{} rolled to a closed day {}", d.iso(), open.iso());
                assert!(open >= d && (from.is_none() || from == Some(d)));
            }
            d = d.add_days(1);
        }
        assert_eq!(roll(&rule, &cals, Date::ymd(2024, 12, 24).unwrap()).0, Date::ymd(2024, 12, 24).unwrap());
        assert_eq!(roll(&rule, &cals, Date::ymd(2024, 12, 27).unwrap()).0, Date::ymd(2025, 1, 7).unwrap(), "over a closure that spans the year and a further closed day");
        assert!(roll(&rule, &cals, Date::ymd(2026, 5, 5).unwrap()).2, "no calendar for 2026");
    }

    #[test]
    fn calendar_export_is_a_local_valid_looking_ics() {
        let fx: Value = serde_json::from_str(FIXTURE).unwrap();
        let cals = calendars(&fx);
        let rs = rules(&fx, "R30");
        let t1 = trigger(&json!({"event_node_id": "ev-1", "event_type": "ORDER_PASSED", "date": "2024-01-15", "date_precision": "DAY", "date_range": null, "conflicting_dates": [], "locator": {"doc_id": "doc-é", "excerpt": "x"}, "origin": "EXTRACTED"}));
        let t2 = trigger(&json!({"event_node_id": "ev-2", "event_type": "ORDER_PASSED", "date": null, "date_precision": "MONTH", "date_range": {"from": "2024-03-01", "to": "2024-03-31"}, "conflicting_dates": [], "locator": {"doc_id": "d2", "excerpt": "x"}, "origin": "EXTRACTED"}));
        let recs: Vec<Value> = [&t1, &t2].iter().map(|t| compute(&Inputs { case_id: "c", rules: &rs, trigger: t, facts: &[], calendars: &cals, computed_at: "2026-10-09T09:00:00Z" }).unwrap()).collect();
        let ics = to_ics(&recs, "2026-10-09T09:00:00Z");
        assert!(ics.starts_with("BEGIN:VCALENDAR\r\n") && ics.ends_with("END:VCALENDAR\r\n"));
        assert!(!ics.contains("\r\r") && ics.replace("\r\n", "").find(|c| c == '\r' || c == '\n').is_none(), "CRLF line ends only");
        assert!(ics.lines().all(|l| l.len() <= 75), "lines are folded at 75 octets");
        assert!(!ics.contains("http"), "no network address in the file");
        let unfolded = ics.replace("\r\n ", "");
        assert!(unfolded.contains("DTSTART;VALUE=DATE:20240214\r\nDTEND;VALUE=DATE:20240215"), "a single date is a one-day event (DTEND exclusive)");
        assert!(unfolded.contains("DTSTART;VALUE=DATE:20240401\r\nDTEND;VALUE=DATE:20240501"), "a range spans first to last possible day (DTEND exclusive)");
        assert_eq!(unfolded.matches("BEGIN:VEVENT").count(), 2);
        assert_eq!(unfolded.matches("TRIGGER:-P7D").count(), 2);
        assert!(unfolded.contains("SUMMARY:VERIFY: Appeal within 30 days") && unfolded.contains("(date range)") && unfolded.contains("DTSTAMP:20261009T090000Z"));
        assert!(unfolded.contains("TRIGGER_DATE_IMPRECISE"), "a range says why");
        assert_eq!(to_ics(&recs, "2026-10-09T09:00:00Z"), ics, "deterministic");
        // Escaping and folding never split a multi-byte character.
        let folded = ics_fold(&format!("DESCRIPTION:{}", "é".repeat(60)));
        assert!(folded.split("\r\n").all(|l| l.len() <= 75) && folded.replace("\r\n ", "").chars().filter(|c| *c == 'é').count() == 60);
        assert_eq!(ics_escape("a,b;c\\d\ne"), "a\\,b\\;c\\\\d\\ne");
    }
}
