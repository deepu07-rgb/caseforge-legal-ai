# B-S2-05 — Bench Preparation Mode

**Status:** DONE (2026-08-09)  
**Pipeline:** `caseforge.bench_prep.v1`

## Goal

AI plays **both counsel** for preparation. **Never** the decision-maker.  
G1 enforced: no verdicts, holdings, or court orders as if the software were the court.

## Mode rules

| Rule | Implementation |
|------|----------------|
| Dual counsel | `side_a` (applicant/plaintiff) + `side_b` (respondent/defence) |
| Non-decision-maker | Hard notice on every turn + UI banner |
| G1 | Input/output filters + holding/order detectors |
| Case sources | `case_graph` (+ optional retrieval ids) only |
| Red-team | ≥12 prompts; all refuse decision language |

## Halt condition

If mode outputs a **holding** or **order** as if court → G1 fail.

## IPC

| Command | Purpose |
|---------|---------|
| `bench_prep_policy` | Frozen policy |
| `bench_prep_turn` | One dual-counsel turn |
| `bench_prep_turn_for_case` | Vault case_graph path |
| `bench_prep_run_script` | Multi-turn dual counsel |
| `bench_prep_redteam` | G1 red-team suite |

## UI

Mock Court → **Bench prep** mode:

- Non-decision-maker banner  
- Side A / Side B toggle  
- Run G1 red-team  

## Eval

```powershell
npm run test:benchprep
```
