# B-S4-11 — Deadline & Limitation Watch

Computes a limitation or procedural deadline from a signed rule row, a start event in the matter, and a court
calendar — deterministically, with its working shown, labelled **VERIFY** (G9), and as a **range with the reason**
whenever any input is uncertain (AC-S4-06.2). No model, no network.

## The engine (`src-tauri/src/deadlines.rs`)

| Input | Source |
|---|---|
| Rule | `LIMITATION_PERIOD` / `PROCEDURAL_DEADLINE` rows of installed official packs (period, trigger event type, counting convention, exclusions, calendar table, authorities) |
| Start event | a chronology event counsel assigns to the rule; its date, precision, ambiguity and source document/excerpt come from the event itself (`trigger_from_event`) |
| Exclusion facts | recorded by counsel with the number of days, whether confirmed, and where they are recorded |
| Calendar | `COURT_CALENDAR` rows (closed dates and ranges per year) named by the rule |

Counting (shown in every record): days, months or years; a month or year ends on the corresponding date, or the last day of
the month when that day does not exist (31 Jan + 1 month = 28/29 Feb; 29 Feb + 1 year = 28 Feb). With
`exclude_trigger_day` the first day is not counted; otherwise it is day 1 and the period ends a day earlier. Exclusions
(for example the days taken to obtain a copy, Limitation Act s.12(2)) lengthen the period, and only when their fact is
recorded and confirmed. If the rule says so and the last day is a day the court is closed, the date moves to the next
open day, across consecutive holidays and year boundaries (Limitation Act s.4).

Uncertainty always gives a RANGE with reasons (`TRIGGER_DATE_IMPRECISE`, `TRIGGER_DATE_CONFLICT`, `CALENDAR_MISSING`,
`EXCLUSION_FACT_UNCONFIRMED`, `RULE_WINDOW_AMBIGUOUS`). A missing calendar year gives the unrolled date as the earliest
and that date plus 14 days as the latest (`UNKNOWN_CALENDAR_SPAN_DAYS`): the earliest is the safe date to work to; the
bound is a display aid, not a legal rule.

Refusals (no deadline is shown): `CF_DEADLINE_TRIGGER_UNKNOWN`, `_NO_LOCATOR` (the trigger needs a document and excerpt),
`_NO_AUTHORITY` (the rule must cite an installed authority), `_NO_RULE` (none in force on the date), `_TRIGGER_TYPE`,
`_EXCLUSION_DAYS_MISSING` (AC-S4-06.3).

Records follow the frozen `contracts/deadline_record.schema.json`. `to_ics` writes an iCalendar file (all-day events, a
range as one event spanning it, alarms 7 days and 1 day before the earliest date) — saved on the computer, no network address.

## In the app

`Deadlines` (left menu): for each rule installed for the matter's jurisdiction, pick the chronology event that starts it;
record any excluded time with where it is recorded; read each result with "Show the working" (start event and its source,
period, exclusions, roll-forward, calendar, rule and authority); download the calendar file. Rules that are not signed off
are marked and their dates are reference only. Commands: `deadlines_compute`, `_assign`, `_unassign`, `_fact_save`,
`_fact_remove`, `_export_ics` (assignments and facts are stored in the matter's encrypted vault).

## Data shipped

`packs/official/caseforge.india.limitation-rules` (development): verbatim sections 4 and 12 and Schedule Articles 116–122
of the Limitation Act, 1963 (cut from the signed controlled partition pack's official text) and seven `DEVELOPMENT_ONLY`
rules — appeals under the CPC (Art. 116(a)/(b), 117), leave to defend (118), legal representatives (120), setting aside
abatement (121), restoring a matter dismissed for default (122). Each rule cites the exact line it comes from. No court
calendar is installed, so these give a range until one is. Not covered: other Articles, state amendments, special-law
periods, criminal appeals.

## Evidence

- Golden **G-DEADLINE** (21 synthetic cases, dates derived by hand and cross-checked with Python's calendar, pinned before
  the engine) incl. leap days, month ends, 90 days counting the first day, rolling over an Easter weekend and over a
  winter vacation into the new year, imprecise/conflicting/ambiguous triggers, confirmed vs unconfirmed vs absent
  exclusions, a missing calendar, a rule that changes inside the range, and each refusal.
- Property-style tests: day counting against an independent walk 1996–2032, clamping and monotonic months, roll-forward
  never on a closed day, calendar-file format (CRLF, 75-octet folding, multi-byte safe, deterministic).
- `shipped_limitation_rules_install_cite_the_right_lines_and_compute`: the real pack through the real installer; each
  rule's first citation is the Schedule line stating its period; a High Court appeal computes to the expected range, and
  to a single date once a calendar is supplied.
- `eval/checks/deadlines_b_s4_11.py`: fixture pin, every record validated against the frozen schema, all five
  uncertainty reasons and both result kinds exercised, static properties.
- **AC-S4-06.2 PASS, AC-S4-06.3 PASS.** **AC-S4-06.1 BLOCKED**: agreement with legal-editor answers needs real signed rows
  and real court calendars (Q-S4-02, Q-S4-11). Until then every shipped date is reference only.
