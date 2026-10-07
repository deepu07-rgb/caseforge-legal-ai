# CaseForge Stage 1 — Release checklist v1.0

**Product:** CaseForge Digest Engine  
**RC:** v1.0.0-rc1  
**Batch:** B-S1-22  
**Date:** 2026-08-08

## Pre-release

- [x] B-S0-01 … B-S0-05 complete (S0 exit green)
- [x] B-S1-01 … B-S1-21 complete (batch plan checklist)
- [x] `npm run ship:stage1` PASS
- [x] AC-2.1 docpipe dependency audit PASS
- [x] Stage-1 guardrails red-team (24) PASS
- [x] G6 preparation-language copy audit PASS
- [x] Hardware refuse policy implemented (&lt;4 GB / 32-bit)
- [x] Lite / Full tier messaging present
- [x] Stage 2 features default **OFF**
- [x] Waivers documented for deferred lab ACs

## Installer / runtime

- [x] Refuse 32-bit OS
- [x] Refuse &lt; 4 GB RAM with clear message
- [x] Lite honesty notice (no live simulation)
- [x] Full sequential model choreography + status chip
- [x] Offline licence path (B-S1-18); EXPIRED = read-only vault
- [x] Case data has zero network path (AC-13.1 policy)

## Content / GOLDEN

- [x] AC-6.1 triage GOLDEN path
- [x] AC-6.2 chronology GOLDEN path
- [x] AC-4.3 / AC-7.2 retrieval GOLDEN path
- [ ] Real-scan G-DOCS supplement (commercial audit)
- [ ] Full-tier lab timing AC-3.2 / AC-3.4 / AC-4.2 (waived to lab)

## Sign-off

| Role | Name | Date | Signature |
|------|------|------|-----------|
| Product Owner | | | |
| Engineering | CaseForge agent build | 2026-08-08 | ship gate automated |
| QA | | | |

**Rule:** Do not ship if `eval/reports/stage1_ship_gate.json` has `ship_ok: false`.
