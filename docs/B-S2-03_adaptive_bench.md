# B-S2-03 — Per-Case Adaptive Bench

**Status:** DONE (2026-08-08)  
**Pipeline:** `caseforge.adaptive_bench.v1`  
**Contract:** `contracts/adaptive_bench.json`  
**Worksheet:** `docs/worksheets/Q-AB-01_adaptive_bench.md`

## Goal

Six **generic** bench archetypes + case-order signals from **this case’s** `case_graph` + user dial-in sliders.  
**Never** named-judge profiles (PRD §10, §20).

## Archetypes

| id | Label |
|----|--------|
| `procedure_first` | Procedure-first |
| `evidence_heavy` | Evidence-heavy |
| `textualist` | Textualist / statutory |
| `relief_focused` | Relief-focused |
| `skeptical_probe` | Skeptical probe |
| `settlement_lens` | Settlement lens |

## Sliders (0–100)

`tempo` · `formality` · `opposition_aggression` · `technicality` · `interruption`

## AC gates

| AC | Procedure |
|----|-----------|
| **AC-10.1** | Six archetypes; `named_judge_profiles=false`; refuse `judge_name` / `named_judge_id` |
| **AC-10.2** | Signals from this case_graph only; slider changes behaviour brief; deterministic fingerprint |

## IPC

| Command | Purpose |
|---------|---------|
| `adaptive_bench_policy` | Frozen policy |
| `adaptive_bench_archetypes` | Six metas + baselines |
| `adaptive_bench_resolve` | Profile from graph JSON + dials |
| `adaptive_bench_resolve_case` | Load vault case_graph |

Mock Court turns inject the behaviour brief for bench/adversary roles.

## UI

Mock Court → **Adaptive Bench** tab: archetype cards + five sliders + case-order signal summary.

## Eval

```powershell
npm run test:bench
```
