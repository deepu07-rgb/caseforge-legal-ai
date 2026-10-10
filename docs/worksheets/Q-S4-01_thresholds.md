# Q-S4-01 / Q-S4-12 — Stage 4 thresholds and resource allocations (owner worksheet)

**Status:** AWAITING OWNER APPROVAL  
**Contracts:** `contracts/stage4_thresholds.json`, `contracts/stage4_resource_budget.json`  
**Effect of approval:** approved values move into `contracts/constants.json` (`stage4` block) with an approval record; the dependent ACs change from BLOCKED to SKIP and become measurable in their batches.

Mark each row **Approve**, **Change to …**, or **Reject (drop the gate)**.

## Q-S4-01 — numeric gates

| Threshold id | Gate | Proposed | Why this number | Decision |
|---|---|---|---|---|
| S4.audit.citation_extraction_recall_min | AC-S4-01.3 | 98 % per jurisdiction | A missed citation is an unaudited citation; must be near-complete | |
| S4.opposing.record_conflict_recall_min | AC-S4-02.2 | 90 % | Conflicts include semantic ones; precision protected by two-locator rule | |
| S4.impeach.deterministic_recall_min | AC-S4-03.1 | 95 % | Date/amount/name conflicts are mechanical; misses are bugs | |
| S4.impeach.deterministic_precision_min | AC-S4-03.1 | 90 % | False contradictions waste cross-examination time | |
| S4.foundation.planted_gap_rank_max | AC-S4-05.1 | top 3 | Same spirit as AC-8.1 (top 2) with more link types | |
| S4.rehearsal.timer_accuracy_max_s | AC-S4-07.1 | ±1 s | Short Indian hearing slots | |
| S4.rehearsal.slot_presets_min | AC-S4-07.1 | 2, 5, 10, 15, 30 min | Mention/interim through final arguments | |
| S4.indic.ocr_cer_max | AC-S4-09.1 | *agent proposal, not frozen:* 0.02 clean / 0.05 degraded | B-S4-14 bake-off on G-INDIC measured 0.0056 clean / 0.0024 degraded on rendered pages (a floor; real scans will be worse). Freeze after a real-scan measurement if you can. | |
| S4.indic.critical_field_accuracy_min | AC-S4-09.2 | 98 % or flagged | Dates/amounts/sections must be right or visibly unconfirmed | |
| S4.chambers.admitted_disputed_agreement_min | AC-S4-12.2 | 95 % | Judges must trust the admitted/disputed split | |
| S4.chambers.lookup_top5_hit_rate_min | AC-S4-13.1 | 90 % | Lookup is excerpt-only, so misses are the main risk | |
| S4.chambers.cause_list_100_render_max_s | AC-S4-14.2 | 60 s | A full board before court sits | |
| S4.chambers.redteam_min_prompts | AC-S4-15.2 | 100 | Decision-solicitation coverage | |
| S4.redteam.full_suite_min_prompts | AC-S4-RT-RED | 300 | Stage 2 suite was 210; Stage 4 adds G7-G10 | |
| S4.licence.seat_transfers_per_year | AC-S4-16.1 | 2 (Q-S4-07) | Laptop replacement without abuse | |
| S4.packs.official_delta_cadence_days | policy | 30 (Q-S4-05) | Indian law changes faster than quarterly | |

## Q-S4-12 — disk allocations and RAM ceiling

Owner already approved (2026-10-08): **base install ≤ 4 GiB (target 3 GiB)**; **3B/4B default model, 7B optional add-on**.

| Component | Proposed max (GiB) | Decision |
|---|---|---|
| Default LLM (Q4) | 2.5 | |
| Speech (STT + TTS) | 0.5 | |
| App + docpipe + OCR runtime | 0.3 | |
| Embeddings | 0.15 | |
| Stage 4 Indic models (Devanagari OCR data + translation) | 0.35 | |
| Packs + indexes at install | 0.2 | |
| **Total** | **4.0** | |
| Peak RSS with default model | 4.5 | |

Note: hitting the 3 GiB *target* needs a ≈ 2.0 GiB default model (3B class); a 4B-class model lands near 3.5 GiB, still under the 4 GiB cap.

## Approval record

```text
approved_by:
approved_on:
changes:
```
