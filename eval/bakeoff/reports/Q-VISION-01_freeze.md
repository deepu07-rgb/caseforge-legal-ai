# Q-VISION-01 freeze decision (B-S1-05)

**Date:** 2026-08-06  
**Authority:** CaseForge Product Owner  
**Bake-off report:** `eval/bakeoff/reports/last_bakeoff.json`

---

## Decision

| Field | Value |
|-------|--------|
| Winner `classifier_id` | `caseforge.triage.heuristic.v1` |
| Status | **Stage-1 frozen (provisional vs future VLM)** |
| Accuracy on GOLDEN set | **100.0%** on **500** pages |
| Peak RSS | N/A (text heuristic; no large model load) |
| License | In-tree CaseForge code (no third-party weights) |
| Re-validate @200 | N/A (full 500 measured) |
| Re-validate @500 | **PASS** |
| Vision slots B/C/D | **Not enabled** — no offline 2–3B VLM weights on build machine |

---

## Rationale

1. Bake-off registry only had the enabled baseline (`heuristic`). External VLM slots remain disabled until local weights + CLI exist (`candidates.json`).
2. On-machine inventory: Ollama has `qwen2.5:7b` (text only); no Qwen2-VL / InternVL / MiniCPM-V pack installed.
3. Product owner authorized provisional freeze of the heuristic for Stage-1 ship-path progression after AC-6.1 PASS on the product-owner synthetic GOLDEN pack (`S-SYNTH-GDOCS-500`).
4. Real scanned images without a text layer still return low-confidence defaults — **re-open Q-VISION-01** when a rights-cleared real-scan subset exists and a 2–3B offline VLM is installed.

---

## Production wiring

| Surface | Value |
|---------|--------|
| Python sidecar | `sidecars/docpipe/triage.py` → `CLASSIFIER_ID` |
| Rust in-process | `src-tauri/src/docpipe/triage.rs` → `CLASSIFIER_ID` |
| `docpipe.version.provisional` | `true` with note “Stage-1 frozen; VLM re-bake when available” |

---

## Re-open triggers

- Install offline vision weights + enable bake-off slot B or C  
- Add ≥ 80 rights-cleared **image** GOLDEN pages (not text-only synthetic)  
- Heuristic accuracy on real scans &lt; 92%

Until re-open, do **not** start a competing production classifier without a new freeze report.
