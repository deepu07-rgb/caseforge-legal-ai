# B-S4-12 — Timed Oral Rehearsal

PRD v4.0 §7.7. Acceptance: AC-S4-07.1 (timer accuracy, graceful end), AC-S4-07.2 (timeline lists every limb, reached or not),
AC-S4-07.3 (interruptions grounded); AC-3.3 latency budget still met. Gate: `eval/checks/timed_rehearsal_b_s4_12.py`
(report `eval/reports/b_s4_12_timed_rehearsal.json`). Golden, pinned before the engine (commit `949a7c6`):
`golden/G-SCRIPTS/fixtures/timed_rehearsal_core.json` with `contracts/rehearsal_policy.json`. Record:
`contracts/rehearsal_timeline.schema.json` (frozen in B-S4-01, unchanged).

**Done when:** a lawyer sees which limbs they never reached in a 5-minute slot. The five-minute golden session does exactly that: the
oral-partition point is named as never reached while the other three show when they were reached and for how long.

## What the lawyer does

On Mock Court, with the matter's argument tree built, choose a slot (2, 5, 10, 15 or 30 minutes, or any number of minutes up to 120) and how
often the bench interrupts (from the bench style, or none / occasional / regular / frequent), then *Start the clock*. Answers are typed or
spoken as before; while the clock runs each answer is timed by the app and the bench either lets counsel continue or interrupts. When the
time is up the session ends by itself: the unsent text stays in the box, and the **timeline** appears in the Debrief tab and is kept with the
matter.

## How it works

* **The host keeps the clock** on a monotonic timer (`Instant`), never the screen's timers. Events are stamped at the moment they became
  due: warnings 60 s and 30 s before the end (a warning that would land at the start is skipped, so the 60 s slot warns once), and the
  expiry at the limit itself. So the timeline is exact however often the screen asks; how soon the screen *notices* is how often it asks
  (every 0.5 s). A response arriving at or after the limit is refused (`CF_REHEARSAL_TIME_UP`) and nothing is lost.
* **Interruptions** come from the argument's **Missing Links** in the integrity scorer's own rank order, preferring one on the point counsel
  is on; when those are used up, the limbs with no Missing Link, weakest first. Each target is used once. The rate sets the spacing: about
  every 180 s, 90 s or 45 s (LOW / MEDIUM / HIGH), none for NONE; the bench style's *interruption* setting picks the default rate.
* **Wording is the host's.** An interruption reads, for example, *Counsel, on "<point>": the evidence is weak (35 of 100; the threshold is
  60). What is your answer?* It is built from the scorer's record and carries the Missing Link's id (`grounding_ref`) and the inputs it cites,
  shown under the question. No model writes it, so it adds no latency and cannot invent.
* **Reached.** A point counts as reached when the answer uses enough of that point's *own* words (its label, theory and anchors; words that
  belong to several points count less). Time between answers goes to the points reached, split equally, or to the point counsel was last on.
  At the end, the time since the last answer goes to the point counsel was last on. Rules: `contracts/rehearsal_policy.json`.
* **Debrief.** The timeline (every limb: reached or not, when first, time on it, interruptions) and observations tied to it: *You did not
  reach "…" in this slot*, *You spent 130 s of 300 s (43%) on "…"*, *The bench interrupted you at 01:40 on "…" (from your Missing Links:
  evidence, 35 of 100)*. Each statement names the part of the timeline it comes from; none predicts an outcome.
* **Text and voice.** Both go through the same timed path (a confirmed voice transcript is just a response). The bench's interruption is
  shown as text; it is not spoken.
* **Licence.** The commands need the `timed_rehearsal` feature (Pro and Chambers (firm) in the placeholder edition map).

## What it does not do — read before relying on it

* **Interruptions are not model-worded.** The playbook asks for interruptions "via the grounded path" (B-S3-16). That path has the model
  write a challenge that the host then reconstructs from verified propositions. Here the host builds the interruption from the Missing
  Link record, which is grounded in the matter's own tree and cites its inputs, but it does not go through the model prompt. Putting a
  model-worded interruption through the B-S3-16 prompt would need a prompt re-freeze and live measurement on the Full machine, and I did
  not do it. The result: interruptions are fast and checkable, but plainer than a model's.
* **"Reached" is a wording match.** A point made in other words can be missed (the timeline says "never reached" for what was not
  detected), and a response that borrows a point's words can be credited. Time per point is estimated from when each answer was sent.
  The screen says both.
* **The timer's accuracy is the host's, not the window's.** If the app is suspended (laptop sleep) the monotonic clock may not count
  the sleep; a session is a sitting, not an alarm.
* The golden is synthetic and hand-derived; the "reached" rule was cross-checked with a throwaway prototype of itself, not with real
  lawyers' arguments. Nothing was measured with the real 7B model; nothing here needs it.
* Slot presets and the timer accuracy are the PROPOSED thresholds `S4.rehearsal.slot_presets_min` and
  `S4.rehearsal.timer_accuracy_max_s` (Q-S4-01, unapproved). The rates, warning marks and reach rule are this batch's proposals.
* Not yet checked on the screen by a person: the new panel type-checks and its wording is scanned, but no one has run it in the app.

## Halt-if

"Turn latency makes short presets unusable on the reference machine." Not triggered: a timed response makes no model call (50 responses in
under 200 ms in a test). The cost of the untimed grounded turn, which timed mode does not use, is unchanged.

## How it was checked

* Rust tests (`timed_rehearsal`): the Missing Link order and exact interruption wording; which limbs each scripted answer reaches; the
  five-minute session's events, limb table and findings equal to the golden; every interruption names a Missing Link of the tree and cites
  the scorer's own inputs; the warnings and expiry at their marks with refusals after; the same on a real clock; rates and the order in
  which targets are used (HIGH: six interruptions then silence; NONE: none in ten minutes); slot and rate validation; ending early; the
  timeline validated against the frozen JSON schema (by the gate); no model or network in the module.
* Gate: pins, the policy against the proposed thresholds, the edition map and first-statement feature checks for the six commands, the
  screen's text and voice paths, that the unsent text is kept on time-up, and that the bench's words come from the host only.
