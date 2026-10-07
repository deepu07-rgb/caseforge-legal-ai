# G-DOCS — Document triage & intake typing

**Purpose:** Golden labels for page triage (AC-6.1) and document auto-type suggestion (AC-5.1).  
**PRD:** §5, §6.1  

## Target metrics (from `contracts/constants.json`)

| AC | Target |
|----|--------|
| AC-6.1 | ≥ 92% accuracy on 500-page golden triage set |
| AC-5.1 | ≥ 80% correct auto-type on 60-file intake set |

## Triage classes (frozen)

`typed_english` | `other` | `mixed` | `clean_handwriting` | `messy_handwriting` | `non_text`

## Authorship

See [`../POLICY.md`](../POLICY.md). Ship-scale content is **not** in this scaffold.

## Fixtures (scaffold / synthetic — not GOLDEN)

| ID | Status | Role |
|----|--------|------|
| `fixtures/micro_001` | `SYNTHETIC_PLACEHOLDER` | typed_english marker plumbing |
| `fixtures/micro_002` | `SYNTHETIC_PLACEHOLDER` | other |
| `fixtures/micro_003` | `SYNTHETIC_PLACEHOLDER` | mixed |
| `fixtures/micro_004` | `SYNTHETIC_PLACEHOLDER` | clean_handwriting |
| `fixtures/micro_005` | `SYNTHETIC_PLACEHOLDER` | messy_handwriting |
| `fixtures/micro_006` | `SYNTHETIC_PLACEHOLDER` | non_text |

**Ship gate:** AC-6.1 requires ~500 **GOLDEN** pages (authoritative labels). Synthetic fixtures are for harness + pipeline plumbing only (`SKIP_IF_STATUS_NOT_GOLDEN`).

## Path to green (humans)

1. [`LABELING_RUBRIC.md`](./LABELING_RUBRIC.md) — class definitions  
2. [`MVP_80_PLAN.md`](./MVP_80_PLAN.md) — first 80 pages  
3. [`AUTHORING.md`](./AUTHORING.md) — fixture lock format  
4. Bake-off: `python eval/bakeoff/run_bakeoff.py`
