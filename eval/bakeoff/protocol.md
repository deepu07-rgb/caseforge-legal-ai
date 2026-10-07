# Bake-off protocol (CaseForge triage)

**PRD:** §4.2 (8 GB choreography), §6.1 (classes), §21 #3 (vision bake-off before M2 freeze).

---

## 1. Fairness rules

1. **Same pages** for every candidate (MVP-80 GOLDEN recommended).  
2. **No peeking** at known_answer during inference (adapters receive image/text only).  
3. **One large model resident** at a time (never concurrent with primary reasoner).  
4. **Pin versions** (GGUF hash / HF revision / git commit) in the report.  
5. **Disable marker shortcuts** for vision candidates (heuristic may still use markers only on synthetic — synthetic excluded by default).

---

## 2. Hardware profile (record in report)

| Field | Example |
|-------|---------|
| Machine class | Full / Mid / Lite (PRD §3) |
| RAM | 16 GB |
| CPU | … |
| GPU | none (offline CPU-only preferred for ship path) |
| OS | Windows 11 |

Ship path should prefer candidates that fit **Full tier offline CPU** unless product explicitly accepts GPU.

---

## 3. Metrics

| Metric | Definition |
|--------|------------|
| Accuracy | Correct page labels / pages |
| Per-class recall | For each of 6 classes |
| Confusion | 6×6 matrix |
| p50 / p95 latency | Per page |
| Peak process RSS | During batch |
| Load time | Cold start of model |

**Primary decision metric:** accuracy on GOLDEN MVP-80, with no class recall &lt; 70% on classes with ≥ 5 pages (soft gate for bake-off winner).

---

## 4. Candidate slots (wired in repo)

| Slot | Class | Notes |
|------|-------|-------|
| A | Heuristic baseline | `caseforge.triage.heuristic.v1` — enabled in `candidates.json` |
| B | 3B-class VLM #1 | `caseforge.triage.vision.slot_b` — disabled until CLI + weights |
| C | 3B-class VLM #2 | `caseforge.triage.vision.slot_c` — disabled |
| D | Optional classical CV + layout | `caseforge.triage.vision.slot_d` — disabled |

Human shortlist + enable steps: [`CANDIDATES.md`](./CANDIDATES.md).  
Machine config: [`candidates.json`](./candidates.json).

Do **not** commit multi-GB weights into this git repo. Path via env var or local packs only.

---

## 5. Decision record (template)

```text
BAKEOFF DECISION
- date:
- set: G-DOCS MVP-80 (commit / content hashes)
- winner: <classifier_id>
- accuracy:
- license / redistribution:
- peak RSS:
- rejected alternatives:
- frozen_in: contracts (when ready) + model pack version
- open: re-validate at 200 and 500 pages
```

Store the filled decision under `eval/bakeoff/reports/` (git-ok if no weights).
