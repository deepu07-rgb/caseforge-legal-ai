# CaseForge demo cases (synthetic · not law · not GOLDEN)

**Purpose:** Three hand-authored practice matters for dogfooding skeleton completeness and **Argument Integrity** logic (Fact / Evidence / Law / Exposure → Vulnerability Index).

| Rule | Status |
|------|--------|
| `authoritative` | **false** always |
| Ship gate AC-6.1 / AC-6.2 / AC-8.x | **Never** uses these as GOLDEN |
| Real client facts | **Forbidden** |
| Legal citations | Synthetic teaching labels only (`SYNTHETIC_DEMO_NOT_LAW`) |

**Why not under `/golden` as GOLDEN?**  
PRD + Option A forbid inventing legal truth for ship metrics. These fixtures are **DEMO_SYNTHETIC** pedagogical packs.

**Why not B-S1-06?**  
B-S1-06 (recognition) remains **HALT** until AC-6.1 PASS on 500 GOLDEN G-DOCS pages. Demo packs do **not** unlock that gate.

---

## Inventory

| ID | Difficulty | Jurisdiction | Theme | Critical VI (weakest limb) |
|----|------------|--------------|-------|----------------------------|
| `demo_01_easy_dishonour` | **Easy** | India | Cheque dishonour recovery | **15** (strong case; Exposure min) |
| `demo_02_medium_wrongful_term` | **Medium** | United States | At-will termination + contract bonus | **62** (L2 Evidence gap) |
| `demo_03_hard_partition` | **Hard** | India | Partition + oral gift + limitation | **78** (L2 oral settlement collapse) |

---

## Files per case

```text
demo/cases/<id>/
  meta.json              # difficulty, jurisdiction, teaching goals
  case_packet.txt        # synthetic pleadings / notes for intake
  known_answer.json      # filled concept_shell (DEMO only)
  argument_tree.json     # relief root, limbs, anchors, link scores, pedagogy
  narrative.md           # human-readable walkthrough
```

---

## Score the argument trees

```powershell
# All three demos
npm run demo:score

# One case
python demo/tools/score_argument_tree.py --case demo_01_easy_dishonour
python demo/tools/score_argument_tree.py --all --json
```

Formula (PRD §8, deterministic):

```text
VI = 100 − min(Fact, Evidence, Law, Exposure)
```

Higher VI = more vulnerable limb. Missing Links panel lists links below the threshold (default 60).

---

## How to use in the live app

### Argument Integrity panel (no vault required)

`tauri:dev` or `npm run dev` shows a **Demo — Argument Integrity** panel with easy / medium / hard
tabs. It scores limbs with the PRD formula, highlights Missing Links, and lists where each argument
needs more strength. Works in Vite-only mode (JSON bundled into the UI).

### Manual intake dogfood

1. `npm run tauri:dev` (with `. .\src-tauri\msvc-env.ps1` on Windows).
2. Unlock vault → **New Case** with jurisdiction matching the demo.
3. Save `case_packet.txt` as a `.txt` (or PDF wrapper) and drag into Document intake.
4. Run triage (heuristic).
5. Compare UI / mental model against `known_answer.json` + the Integrity panel.

Automatic vault seeding is **out of scope** for this pack (no invented IPC batch).

---

## Teaching goals

| Case | What you should practice |
|------|--------------------------|
| Easy | Clean four-link limb; low VI; little Missing Links noise |
| Medium | One limb looks good until **Evidence** is scored honestly |
| Hard | Multi-limb tree; Law + Exposure failures; where counsel must strengthen |

---

*CaseForge demo layer — parallel to product batches; not a substitute for GOLDEN authorship.*
