# Q-ST-01 — Session turn property inventory (Stage 2 Mock Court)

**Status:** DECIDED (2026-08-08) for Stage 2 text Mock Court — `session_turn.schema.json` v1.0.0  
**Unblocks:** B-S2-02 Mock Court text formats  
**Related:** PRD §2 Stage 2 · §4.3 · §15 G5 · AC-3.3 · AC-2.1

---

## 1. Owner decision

| Field | Answer |
|-------|--------|
| Owner | CaseForge Product Owner |
| Date | 2026-08-08 |
| Decision | **DECIDED** (v1.0.0) |
| Pipeline id | `caseforge.mock_court.v1` |

---

## 2. Five rehearsal formats (PRD §2 — order is sequential ship order)

| Order | id | Label |
|-------|-----|--------|
| 1 | `Bail_Interim_Motion` | Bail / Interim-Motion |
| 2 | `Final_Arguments` | Final Arguments |
| 3 | `Witness_Cross_Examination` | Witness Cross-Examination |
| 4 | `Appellate` | Appellate |
| 5 | `Motion_Generic` | Generic Motion |

**Rule:** Format N ships only after format N−1 scripted eval PASSes.

---

## 3. SessionTurn fields (v1.0.0)

| Field | Required | Notes |
|-------|----------|-------|
| schema_version | yes | `1.0.0` |
| turn_id | yes | uuid / stable id |
| session_id | yes | |
| case_id | yes | |
| format | yes | one of five enums |
| role | yes | `counsel` \| `adversary` \| `bench` \| `witness` \| `system` |
| stage | yes | DECOMPOSE…RENDER |
| user_text | no | counsel input |
| assistant_text | no | generated / scripted output |
| pipeline_stages_completed | yes | list of stages done this turn |
| retrieval_ids | yes | G5; may be empty if graph-only |
| model_hash | yes | `deterministic` or model id hash |
| prompt_hash | yes | hash of constrained prompt material |
| elapsed_ms | yes | for AC-3.3 |
| sources | yes | `case_graph` and/or `retrieval` only — never `raw_document` |
| disclaimer | yes | preparation only; no verdict |

---

## 4. Invariants

| Invariant | OK |
|-----------|----|
| Adversary / bench consume **only** case_graph + retrieved law | yes |
| No raw PDF / page_text_layer parse in mock_court | yes |
| No named-judge profiles | yes |
| Text-only this batch (voice = B-S2-04) | yes |
| Turn latency budget ≤ 15 s when models ready (AC-3.3) | yes |
| G6: no win/verdict prediction language in system templates | yes |
