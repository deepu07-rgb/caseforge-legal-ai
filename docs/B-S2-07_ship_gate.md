# B-S2-07 — Stage 2 ship gate (v2.0)

**Status:** DONE (2026-08-09)  
**Release candidate:** `v2.0.0-rc1`  
**Product surface:** Digest Engine + **Mock Court** (Full tier required for live voice simulation)

## Goal

Ship Stage 2 Mock Court only when Stage 2 ACs + AC-2.1 + AC-15.1 are green.

## Ship command

```powershell
npm run ship:stage2
```

Writes `eval/reports/stage2_ship_gate.json`.

## What the gate runs

| Check | Script |
|-------|--------|
| B-S2-01 Integrity | `integrity_b_s2_01.py` |
| B-S2-02 Mock Court formats | `mock_court_b_s2_02.py` |
| B-S2-03 Adaptive Bench | `adaptive_bench_b_s2_03.py` |
| B-S2-04 Voice court | `voice_court_b_s2_04.py` |
| B-S2-05 Bench Prep | `bench_prep_b_s2_05.py` |
| B-S2-06 Experience + AC-15.1 | `stage2_experience_redteam_b_s2_06.py` |
| AC-2.1 | `ac_2_1_stage2_dependency.py` |

Also:

- G1/G6 surface audit on Mock Court UI  
- Full-tier Mock Court messaging (Lite refuse live sim)  
- Artefacts: changelog, v2 checklist, waivers, version 2.x  

## Hardware / tier (PRD §3)

| Tier | Mock Court |
|------|------------|
| Lite (4–&lt;8 GB) | Digest only; **no live voice simulation** (honesty refuse) |
| Full (≥8 GB) | Text Mock Court + voice duplex + Adaptive Bench + Bench Prep |

## Waivers

See [`eval/waivers/stage2_ship_waivers.json`](../eval/waivers/stage2_ship_waivers.json).

**Not waived:** AC-15.1 full suite, AC-2.1, AC-8.x, AC-10.x.

## Outputs

| Artefact | Path |
|----------|------|
| Ship report | `eval/reports/stage2_ship_gate.json` |
| Checklist | `docs/RELEASE_CHECKLIST_v2.0.md` |
| This doc | `docs/B-S2-07_ship_gate.md` |

## Eval

```powershell
npm run test:ship2
npm run ship:stage2
```
