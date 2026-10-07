# CaseForge Stage 2 — Release checklist v2.0

**Product:** CaseForge Digest + Mock Court  
**RC:** v2.0.0-rc1  
**Batch:** B-S2-07  
**Date:** 2026-08-09

## Pre-release

- [x] B-S1-22 Stage 1 ship gate green (prerequisite)
- [x] B-S2-01 … B-S2-06 complete
- [x] `npm run ship:stage2` PASS
- [x] AC-2.1 docpipe dependency audit PASS
- [x] AC-8.1–8.3 Integrity PASS
- [x] AC-10.1–10.2 Adaptive Bench PASS
- [x] AC-15.1 full red-team ≥200 zero G1–G3 violations PASS
- [x] Five Mock Court formats scripted PASS
- [x] Voice-in-court Lite honesty refuse PASS
- [x] Bench Prep non-decision-maker + G1 red-team PASS
- [x] Experience Store limb_outcome / bench_setting writable
- [x] Full-tier messaging for Mock Court present
- [x] Stage 2 ship waivers documented

## Installer / runtime

- [x] Refuse 32-bit / &lt;4 GB (Stage 1 gate retained)
- [x] Lite: no live Mock Court voice simulation
- [x] Full: sequential models + Mock Court paths
- [x] Offline licence path retained
- [x] Case data has zero network path (AC-13.1 policy)

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Product Owner | | | |
| Engineering | CaseForge agent build | 2026-08-09 | ship gate automated |
| QA | | | |

**Rule:** Do not ship Stage 2 if `eval/reports/stage2_ship_gate.json` has `ship_ok: false`.
