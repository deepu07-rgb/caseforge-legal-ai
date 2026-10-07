# B-S1-22 — Stage 1 ship gate (v1.0)

**Status:** DONE (2026-08-08)  
**Release candidate:** `v1.0.0-rc1`  
**Product surface:** Digest Engine only (Stage 2 Mock Court not default-on)

## Goal

Commercial **Stage 1 Digest** ship only when the Stage 1 matrix is green (PASS) or explicitly **waived** in `eval/waivers/stage1_ship_waivers.json`.

## Ship command

```powershell
npm run ship:stage1
```

Writes `eval/reports/stage1_ship_gate.json`.

## What the gate runs

1. **All B-S1-01 … B-S1-21** batch check scripts  
2. **AC-2.1** Stage-2 dependency audit (docpipe free of Stage 2 imports)  
3. **AC-6.1 / AC-6.2** content scorers  
4. **G6** copy audit (no win/verdict prediction language)  
5. **Artefacts:** changelog, release checklist, waivers, installer hardware gate  

## Hardware refuse (PRD §3)

| Condition | Behaviour |
|-----------|-----------|
| 32-bit OS | Refuse with clear message |
| RAM &lt; 4 GB | Refuse |
| 4–&lt;8 GB | **Lite** messaging (no live simulation) |
| ≥ 8 GB | **Full** messaging |

Host: `hardware_gate` / `hardware_policy` (`src-tauri/src/installer/mod.rs`).

## AC-2.1

Static audit: document pipeline (`docpipe` + `sidecars/docpipe`) must not import Stage 2 modules (Mock Court, Argument Tree engine, Bench Prep). Stage 2 UI may exist for preview but **must not** be wired into doc parsing.

## Waivers

See [`eval/waivers/stage1_ship_waivers.json`](../eval/waivers/stage1_ship_waivers.json).

Notable deferred lab measurements: AC-3.2, AC-3.4, AC-4.2 wall-clock/RSS; AC-5.1 auto-type GOLDEN; AC-9.1 WER; AC-16.3 `.akcase`; full AC-15.1 200-prompt (Stage 2).

## Stage 2

`stage2_default_on: false`. Mock Court remains non-default until B-S2-07.

## Outputs

| Artefact | Path |
|----------|------|
| Ship report | `eval/reports/stage1_ship_gate.json` |
| Changelog | `CHANGELOG.md` |
| Checklist | `docs/RELEASE_CHECKLIST_v1.0.md` |
| This doc | `docs/B-S1-22_ship_gate.md` |

## Eval

```powershell
npm run test:ship
npm run ship:stage1
```
