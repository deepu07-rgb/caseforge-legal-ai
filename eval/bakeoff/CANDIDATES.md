# Vision bake-off candidate shortlist (Q-VISION-01)

**Purpose:** Human-readable shortlist of **offline** triage candidates.  
**Does not:** download weights, freeze the winner, or PASS AC-6.1.  
**Machine config:** `candidates.json` (wired from this list).  
**Protocol:** [`protocol.md`](./protocol.md)

---

## Constraints (from PRD / contracts)

| Constraint | Detail |
|------------|--------|
| Class | Strong **~2B–3B** vision-language preferred for docpipe |
| Concurrency | Vision **never** concurrent with primary 7–9B reasoner on 8 GB |
| Network | Ship path = offline; no cloud APIs at inference |
| License | Must allow product redistribution / local install (owner legal check) |
| Classes | Exact 6: typed_english, clean_handwriting, messy_handwriting, mixed, other, non_text |
| Fairness | Same GOLDEN (MVP-80) pages for every candidate |

---

## Slot registry

| Slot | classifier_id | Adapter | Role | Default enabled |
|------|---------------|---------|------|-----------------|
| A | `caseforge.triage.heuristic.v1` | `heuristic` | Baseline (always) | **yes** |
| B | `caseforge.triage.vision.slot_b` | `external_cli` | 3B-class VLM #1 | no until weights + CLI ready |
| C | `caseforge.triage.vision.slot_c` | `external_cli` | 3B-class VLM #2 | no |
| D | `caseforge.triage.vision.slot_d` | `external_cli` | Optional classical / layout OCR hybrid | no |

---

## Suggested models for owner research (not pre-selected)

Product owner must verify **license**, **RAM**, and **CPU-only** feasibility before enabling a slot.

| Research ID | Approx class | Why considered | License check | Notes |
|-------------|--------------|----------------|---------------|-------|
| Qwen2-VL / Qwen2.5-VL ~2B | 2B VLM | Strong doc/VQA family; common GGUF/llama.cpp paths | ☐ | Prefer Q4 quant for 8 GB choreography |
| InternVL2 ~2B | 2B VLM | Document-oriented multimodal | ☐ | Confirm offline packaging |
| MiniCPM-V ~2–3B | 2–3B VLM | Mobile/edge friendly | ☐ | Check export path |
| Florence-2 base/large | Encoder VLM | Microsoft doc/OCR-adjacent tasks | ☐ | License + offline packaging |
| Layout / classical hybrid | Heuristic + detector | No LLM; lower RAM | ☐ | May underperform handwriting |

**Do not commit multi-GB weights into git.** Place models under a local packs directory and point `command` / env vars at them.

---

## Enabling a vision slot

1. Install model **outside** the repo (or git-lfs private pack).  
2. Implement or point CLI at stdin JSON pages → stdout JSON results (see `adapters/external_cli.py`).  
3. Edit `candidates.json`: set `enabled: true`, real `command`, `model_path` / notes.  
4. Record GGUF hash / revision in notes.  
5. Run:

```powershell
# After MVP-80 GOLDEN exists:
python eval/bakeoff/run_bakeoff.py --candidates eval/bakeoff/candidates.json

# Pre-lock experiments only:
python eval/bakeoff/run_bakeoff.py --candidates eval/bakeoff/candidates.json --include-draft
```

6. Write decision under `eval/bakeoff/reports/` using protocol template.  
7. Freeze winner `classifier_id` with product owner (Q-VISION-01).

---

## Owner decision log (fill after bake-off)

| Field | Value |
|-------|-------|
| Date | |
| Set | MVP-80 GOLDEN / other |
| Winner classifier_id | |
| Accuracy | |
| Peak RSS | |
| License OK | ☐ |
| Re-validate @200 | ☐ |
| Re-validate @500 | ☐ |
| Frozen in contracts/pack version | |

Until winner is frozen, production triage remains **`caseforge.triage.heuristic.v1` (provisional)**.
