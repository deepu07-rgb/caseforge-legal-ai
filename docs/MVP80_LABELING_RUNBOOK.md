# MVP-80 labeling runbook (Track B)

**Goal:** Lock **80 GOLDEN** G-DOCS pages for bake-off measurement, then scale to **500** for AC-6.1.  
**Mode:** Option A — humans label and lock GOLDEN; agents scaffold DRAFT only.  
**Does not unlock B-S1-06** until AC-6.1 PASS on ≥ 500 GOLDEN @ ≥ 92%.

| Doc | Role |
|-----|------|
| [`../golden/G-DOCS/MVP_80_PLAN.md`](../golden/G-DOCS/MVP_80_PLAN.md) | Quotas + schedule |
| [`../golden/G-DOCS/LABELING_RUBRIC.md`](../golden/G-DOCS/LABELING_RUBRIC.md) | Class definitions |
| [`../golden/G-DOCS/AUTHORING.md`](../golden/G-DOCS/AUTHORING.md) | Fixture format + lock rules |
| [`../golden/POLICY.md`](../golden/POLICY.md) | Who may lock GOLDEN |
| [`OPTION_A_runbook.md`](./OPTION_A_runbook.md) | Full Option A path |

---

## Gate 0 — People & rights (do this before scanning 80 pages)

1. Copy roster template and fill names:

```text
golden/G-DOCS/mvp80/ROSTER.template.json
  → save as golden/G-DOCS/mvp80/ROSTER.json  (local; may stay uncommitted if private)
```

2. Copy source list template:

```text
golden/G-DOCS/mvp80/SOURCES.template.md
  → golden/G-DOCS/mvp80/SOURCES.md
```

3. Checklist:

- [ ] Labeler A named  
- [ ] Labeler B named  
- [ ] Adjudicator named  
- [ ] Rights-cleared sources listed (no confidential client dumps without redaction sign-off)  
- [ ] Rubric walkthrough done (both labelers + adjudicator, 1–2 h)

---

## Gate 1 — Stage pages on disk

Put **one page per file** (PNG/JPG or single-page PDF preferred) in:

```text
golden/G-DOCS/mvp80/staging/
  typed_english/
  clean_handwriting/
  messy_handwriting/
  mixed/
  other/
  non_text/
```

Folder names are **hints only** — final class is dual-labeled, not taken from the folder.

### Class quotas (80)

| Class | Count |
|-------|------:|
| typed_english | 32 |
| clean_handwriting | 12 |
| messy_handwriting | 10 |
| mixed | 10 |
| other | 10 |
| non_text | 6 |

---

## Gate 2 — Scaffold DRAFT fixtures

From **repo root**:

```powershell
# Example — one page
python eval/tools/scaffold_gdocs_fixture.py `
  --file "golden\G-DOCS\mvp80\staging\typed_english\order_01.png" `
  --id page_0001 `
  --author "alice"

# Optional proposed class (still DRAFT, not GOLDEN)
python eval/tools/scaffold_gdocs_fixture.py `
  --file "golden\G-DOCS\mvp80\staging\non_text\blank_01.png" `
  --id page_0002 `
  --class non_text `
  --author "alice"
```

Or via npm:

```powershell
npm run gdocs:scaffold -- --file path\to\page.png --id page_0001 --author "alice"
```

**Rules:**

- ID scheme: `page_0001` … `page_0080` for MVP-80 (then continue toward 500).  
- Scaffold creates `status: DRAFT`, `authoritative: false`.  
- **Never** set `GOLDEN` until dual review + adjudicator lock.

---

## Gate 3 — Blind dual label

1. Labeler A opens each fixture `known_answer.json` → sets `triage.class` only after viewing the page image.  
2. Labeler B works **blind** (no access to A’s class) — record B’s labels in `golden/G-DOCS/mvp80/labels_B.csv` (template below) **or** a private sheet.  
3. Do not edit `status` yet.

### Labeler B CSV template (`labels_B.csv`)

```csv
fixture_id,class,notes,labeler
G-DOCS/page_0001,typed_english,,bob
```

Valid `class` values only:

`typed_english` · `clean_handwriting` · `messy_handwriting` · `mixed` · `other` · `non_text`

---

## Gate 4 — Adjudicate & lock GOLDEN (human only)

For each page:

1. If A and B agree → adjudicator may lock.  
2. If they disagree → adjudicator decides using [`LABELING_RUBRIC.md`](../golden/G-DOCS/LABELING_RUBRIC.md); record reason in description/notes.  
3. Lock fields in **both** `meta.json` and `known_answer.json`:

| Field | Value |
|-------|--------|
| `status` | `GOLDEN` |
| `authoritative` | `true` |
| `authored_by` | labeler A (or primary) |
| `reviewed_by` | labeler B or adjudicator |
| `locked_at` | ISO-8601 UTC |
| `content_hash` | already set by scaffold; recompute if file replaced |
| `eval_policy` | `SHIP` (known_answer) |

**Coding agents must not lock GOLDEN.**

---

## Gate 5 — Measure progress & score

```powershell
# Page / quota inventory
npm run gdocs:progress
python eval/tools/gdocs_progress.py --json --out eval/reports/gdocs_progress.json

# Ship scorer (FAIL on page count until 500 — expected)
python eval/checks/ac_6_1_triage_score.py
python eval/run_eval.py --ac AC-6.1

# After 80 GOLDEN locked — bake-off
npm run bakeoff
python eval/bakeoff/run_bakeoff.py --json --out eval/bakeoff/reports/last_bakeoff.json
```

---

## Gate 6 — After MVP-80

1. Freeze provisional vision candidate (Q-VISION-01) on MVP-80.  
2. Scale labeling to 500 with same rubric.  
3. Re-check bake-off winner at ~200 and 500.  
4. When AC-6.1 **PASS** → tell eng: **“AC-6.1 green — go ahead B-S1-06”**.

---

## Agent vs human

| Task | Agent | Human |
|------|-------|-------|
| Scaffold DRAFT | Yes | Yes |
| Propose class on DRAFT | Yes (optional) | Yes |
| Dual label | No | **Yes** |
| Lock GOLDEN | **No** | **Yes** |
| Progress / scorer / bake-off | Yes | Yes |
| Invent rights-cleared case pages | **No** | Collect real or cleared material |

---

## Quick commands cheat sheet

```powershell
cd "c:\Users\puroh\Downloads\Case Forge- Legal AI app"

# Track B pack already generated: 80 synthetic DRAFT pages (page_0001..page_0080)
npm run gdocs:progress
# Dual-label sheets: golden/G-DOCS/mvp80/labels_A.csv + labels_B.csv
python eval/tools/apply_gdocs_labels.py --labels golden/G-DOCS/mvp80/labels_A.csv --labels-b golden/G-DOCS/mvp80/labels_B.csv
# After adjudicated labels_final.csv — apply DRAFT classes (never skip human review):
# python eval/tools/apply_gdocs_labels.py --labels golden/G-DOCS/mvp80/labels_final.csv
# Lock GOLDEN (human adjudicator name required):
# python eval/tools/apply_gdocs_labels.py --adjudicated golden/G-DOCS/mvp80/labels_final.csv --labels golden/G-DOCS/mvp80/labels_final.csv --lock-golden --adjudicator "Name"

npm run gdocs:scaffold -- --file C:\path\to\page.png --id page_0081 --author "you"
npm run test:ac61
npm run bakeoff
```

**Live status:** [`../golden/G-DOCS/mvp80/STATUS.md`](../golden/G-DOCS/mvp80/STATUS.md)
