# B-S2-02 — Mock Court text formats

**Status:** DONE (2026-08-08)  
**Pipeline:** `caseforge.mock_court.v1`  
**Schema:** `contracts/session_turn.schema.json` v1.0.0 (Q-ST-01)

## Goal

Five text-only rehearsal formats, sequential ship order, adversary constrained to **case_graph + retrieved law** (never raw documents).

## Formats (order)

| # | id | Label |
|---|-----|--------|
| 1 | `Bail_Interim_Motion` | Bail / Interim-Motion |
| 2 | `Final_Arguments` | Final Arguments |
| 3 | `Witness_Cross_Examination` | Witness Cross-Examination |
| 4 | `Appellate` | Appellate |
| 5 | `Motion_Generic` | Generic Motion |

Eval runs **1 → 5** and stops on first FAIL.

## Pipeline per turn

`DECOMPOSE → RETRIEVE → GENERATE → VERIFY → SCORE → RENDER`

G5 provenance on every turn: `model_hash`, `prompt_hash`, `retrieval_ids`.

## AC gates

| AC | Gate |
|----|------|
| **AC-2.1** | mock_court never imports/parses raw docs; docpipe never imports mock_court |
| **AC-3.3** | Turn ≤ 15 s (offline deterministic path ≪ budget; live models when ready) |
| Scripts | `golden/G-SCRIPTS/fixtures/format_0N_*` |

## IPC

| Command | Purpose |
|---------|---------|
| `mock_court_policy` | Frozen policy |
| `mock_court_formats` | Five format metas |
| `mock_court_turn` | One turn with optional graph JSON |
| `mock_court_turn_for_case` | Load vault `case_graph` only (no `case_llm_context`) |
| `mock_court_run_script` | Multi-turn scripted session |

## UI

**Mock Court** nav — format cards 1–5; Send uses `mockCourtTurnForCase` when a case is open.

## Eval

```powershell
npm run test:mockcourt
```

## Out of scope

- Voice (B-S2-04)
- Named-judge profiles (never)
- Adaptive bench archetypes (B-S2-03)
