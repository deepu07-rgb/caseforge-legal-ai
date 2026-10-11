# Stage 4 — owner decision brief

**As of 2026-10-11.** Stage 4 ship verdict: **NOT READY**. Nothing is failing: 55 criteria pass, 2 are waived (by you), 9 are blocked. All 9 blocked criteria wait on a decision or a person below, not on code. This page lists each decision once, says what it unlocks, and gives a recommendation. A recommendation is mine; each decision is yours.

How to answer: reply with the decision numbers and "approve", "change to …" or "reject", e.g. "D1 approve all, D2 change to 4.0 GiB peak…". I then record the approvals (with your name and date) in `contracts/stage4_thresholds.json` and `contracts/constants.json` and re-run the gates.

## Already decided (for the record)

| Decision | What | Date |
|---|---|---|
| W-AC-S4-00.1 | Base install may exceed the 4 GiB cap (measured 5.08 GiB with the 7B default model); reported WAIVED | 2026-10-08 |
| W-AC-S4-00.7 | Default model's one failing injection case (`INJ2-US-1`, a date slip) accepted as a recorded limitation; reported WAIVED | 2026-10-11 |
| Mock Court limit | The 15 s turn limit stays; slower machines fall back to the labelled deterministic rehearsal | 2026-10-11 |

## Decisions that unlock blocked criteria

### D1 — Numeric gates (Q-S4-01). Unlocks AC-S4-01.3, 02.2, 03.1 (and the formal status of many others)

All proposed values are in `docs/worksheets/Q-S4-01_thresholds.md`. What was measured against them:

| Gate | Proposed | Measured | Caveat |
|---|---|---|---|
| Citation extraction recall (01.3) | ≥ 98 % | 10 of 10 planted | one small synthetic set |
| Record-conflict recall (02.2) | ≥ 90 % | 3 of 3 | one small synthetic fixture |
| Impeachment recall / precision (03.1) | ≥ 95 % / ≥ 90 % | 12 of 12, 0 false positives | one small synthetic fixture |
| Planted-gap rank (05.1) | top 3 | the engine puts the planted gap in the top 3 | shipped rows are unsigned |
| Admitted/disputed agreement (12.2) | ≥ 95 % | passes on the golden | synthetic golden |
| Lookup top-5 hit rate (13.1) | ≥ 90 % | passes | synthetic record |
| 100-matter cause-list render (14.2) | ≤ 60 s | about 0.6 s | development machine, not the Full reference machine |
| Critical-field accuracy (09.2) | ≥ 98 % or flagged | passes | synthetic scans |

**Recommendation: approve all as proposed.** They are gates, not claims: approving them lets the criteria turn from BLOCKED to PASS on the data we have. Be aware that "PASS" then means "passes on synthetic fixtures"; real filings, statements and scans will miss more, and the gates should be re-run on editor-built data (D2) before any real use. Two need your machine: the cause-list render time (re-measure on the Full reference machine) and the lookup hit rate (needs real matter records).

### D2 — Two named legal editors per jurisdiction (Q-S4-02). Unlocks AC-S4-04.1, 05.1, 06.1

Nobody at the keyboard can supply this; it is a person or two. What they sign (`eval/signoffs/legal_editors.json`, which does not exist yet):
* the real code-succession rows (IPC → BNS, CrPC → BNSS, Evidence Act → BSA, and the US equivalents), so the Code Bridge stops saying "Not mapped" and works on real codes;
* the admissibility rules and the limitation/deadline rule tables (today "development only"; every shipped date is reference-only until signed);
* the golden answer sets G-CODEBRIDGE, G-ADMISS, G-DEADLINE, and the planted-gap rows for foundation gaps.

**Recommendation:** name editors for India first (the shipped packs are India-heavy). Until this exists the app says "reference only" on every date and "Not mapped" on every code mapping, which is the honest state.

### D3 — OCR error limit for Hindi (Q-S4-04, AC-S4-09.1). Unlocks AC-S4-09.1

Measured character error on rendered pages: **0.0056 clean, 0.0024 degraded** (a floor: real scans will be worse). The contract has no value because it was left for you to freeze after the bake-off.

**Recommendation:** freeze provisionally at **0.02 clean / 0.05 degraded**, and re-measure on a handful of real Hindi scans of your own (you pick them; nothing leaves the machine) before treating it as final. Also confirm `hin+eng` from `tessdata_best` (Apache-2.0) as the Hindi reader.

### D4 — RAM ceiling (Q-S4-12, AC-S4-00.4). Unlocks AC-S4-00.4

The worksheet proposes **4.5 GiB peak** with the default model. That number was written when the default was a 3B/4B model. The default is now `qwen2.5:7b`. It has been **measured** (2026-10-11, real app, `qwen2.5:7b`, a Mock Court session on a synthetic matter, memory of every process sampled once a second):

| Part | Peak working set (RAM in use) |
|---|---|
| Desktop app | 0.1 GB |
| Web view (the screen engine) | 0.2-0.5 GB |
| Python sidecars (reading, search, model gateway) | up to 0.55 GB |
| Model runner (`llama-server`) | **2.9 GB**, on top of 2.3 GB held in graphics memory |
| **Whole app, simultaneous peak** | **about 3.1 GB observed in the run that included the model; about 3.7 GB if the biggest observed web-view/sidecar peaks coincide with it** |

What that means:
* **On a machine with a GPU like this one** (4 GB card: the 7B model splits 58 % processor / 42 % graphics memory, 5.4 GB in all), whole-app RAM stays **under** the proposed 4.5 GiB (4.83 GB). The criterion would pass here.
* **On a machine with no usable GPU**, the whole 5.4 GB model sits in RAM, so the same session needs roughly **6.2 GB** (my estimate: 5.4 + the app parts above; not measured, I have no CPU-only machine). 4.5 GiB would fail there, and the Full tier's 8 GB minimum would be tight.
* Caveats: these figures come from an unoptimised development build; the PC was short of free memory (0.4 of 15.7 GB), so Windows had already trimmed idle working sets and the numbers are lower bounds; OCR of a large scan and Hindi translation were not part of this run; the figure "private/commit" memory reads 7+ GB because the graphics runtime reserves address space, and is not a usable measure.

**Recommendation:** state the ceiling per hardware class instead of one number: **4.5 GiB of RAM when a GPU of 4 GB or more carries part of the model, about 6.5 GiB when the model runs on the processor alone** (the second is an estimate and should be measured on a CPU-only machine before you rely on it), and keep 8 GB as the Full-tier minimum. The disk-allocation table in the same worksheet still needs restating (the 7B model alone is 4.4 GiB).

## Decisions that change the product but block no criterion

### D5 — Translation model (Q-S4-04)

Hindi → English currently uses `qwen2.5:7b` with a fixed prompt. It works (about 2.5 s a segment; dates, amounts, section and case numbers are written back by fixed rules so the model cannot change a number; names always wait for counsel; every English segment is labelled "machine translation - wording not verified"). A dedicated Indic translation model would very likely read better but needs a download, which I will not do without your say-so.
**Recommendation:** keep the current model for now; ask again after real Hindi pages have been read by a Hindi-literate lawyer. Also decide which editions get translation: it is the feature "Indic-script ingestion" in the placeholder edition map, which today includes it for the judiciary (Chambers) edition as well; Chambers is record-only, so whether a judge's edition may show machine-translated text is your call.

### D6 — Editions and prices (Q-S4-06) and seat transfers (Q-S4-07)

The mechanism is built and tested; the map of which features belong to which edition is a placeholder (`contracts/edition_entitlements.json`, development only) and no prices exist. Seat transfers per year: proposed **2**.
**Recommendation:** approve 2 transfers a year. Editions and prices are yours; send the list and I will put it in the map.

### D7 — Pilot court (Q-S4-10) and court calendars (Q-S4-11)

Chambers pilot constraints are configuration, not code (`chambers_pilot.json`). The real cause-list and order-sheet formats of the pilot court are not available to me, so order recognition is generic; and court calendars for a real court/year are missing, so deadlines show a range with "VERIFY".
**Recommendation:** pick the pilot court, then share one real (redacted) cause list, one order sheet and that year's court calendar; I will fit the recognisers and tables to them.

## Things I cannot do from here

| What | Why | Unlocks |
|---|---|---|
| Network packet capture across the Stage 4 workflows | needs a machine with capture tooling and your sign-off on the method | AC-S4-10.3 |
| Run the model-speed and render checks on the Full reference machine | this laptop has a 4 GB GPU | AC-S4-14.2 (formal), Mock Court turns inside 15 s |
| Build the installer and measure the real base-install size | the installer workflow is blocked while GitHub Actions billing is blocked; it can be run locally | replaces the 5.08 GiB estimate |
| Sign a release | no release signing key exists; you decide who signs | signed ship report |
| Run the one ignored test (`b_s4_29_scale_benchmark`) | needs a folder of real signed legal packs and its own workflow | "zero required skips" |
| Exercise Chambers/editions, voice and LAN transfer in the real app | Chambers needs a licence signed with your licence key; I will not add a signing key of my own to the app's trust list | confidence in those screens |

## Suggested order

1. **D1** (one message, unlocks three criteria and most of the formal statuses).
2. **D4** (a wrong number here would create a failure).
3. **D3** and **D5** (small, and they close the Indic story).
4. **D2** (the long pole: it needs a person; start it early).
5. **D6**, **D7** when you have the facts.
