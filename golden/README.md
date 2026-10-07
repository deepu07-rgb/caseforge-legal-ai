# CaseForge Layer 4 — Golden datasets

**Batch:** B-S0-04 (scaffolds + micro-fixtures only)  
**PRD:** §17, §18 Phase 0  

Golden datasets are authored **before** production code and **define correctness**.  
An implementer’s work is DONE only when tests against these sets pass — not when output “looks good.”

---

## Sets (required by batch plan)

| Set ID | Path | Purpose | Primary ACs (non-exhaustive) |
|--------|------|---------|--------------------------------|
| **G-DOCS** | [`G-DOCS/`](./G-DOCS/) | Page triage / document type / quality | AC-5.1, AC-6.1 |
| **G-CASE-IN** | [`G-CASE-IN/`](./G-CASE-IN/) | India case chronology / skeleton known-answer | AC-6.2–AC-6.6, AC-6.8 |
| **G-CASE-US** | [`G-CASE-US/`](./G-CASE-US/) | United States case chronology / skeleton known-answer | AC-6.2–AC-6.6, AC-6.8 |

### DRAFT scaffold tools (empty known-answers)

| Set | Command |
|-----|---------|
| G-DOCS | `python eval/tools/scaffold_gdocs_fixture.py --file page.png --id page_0001` |
| G-CASE-IN | `python eval/tools/scaffold_gcase_in_fixture.py --id matter_0001 --author "you"` |
| G-CASE-US | `python eval/tools/scaffold_gcase_us_fixture.py --id matter_0001 --author "you"` |
| Either case | `python eval/tools/scaffold_gcase_fixture.py --jurisdiction India\|US --id …` |

Authoring guides: [`G-CASE-IN/AUTHORING.md`](./G-CASE-IN/AUTHORING.md), [`G-CASE-US/AUTHORING.md`](./G-CASE-US/AUTHORING.md).
| **G-AUDIO** | [`G-AUDIO/`](./G-AUDIO/) | English STT / lexicon / voice rehearsal | AC-9.x |
| **G-RETRIEVAL-IN** | [`G-RETRIEVAL-IN/`](./G-RETRIEVAL-IN/) | India authority retrieval + citation verify | AC-4.3, AC-7.2 |
| **G-RETRIEVAL-US** | [`G-RETRIEVAL-US/`](./G-RETRIEVAL-US/) | US authority retrieval + citation verify | AC-4.3, AC-7.2 |
| **G-REDTEAM** | [`G-REDTEAM/`](./G-REDTEAM/) | Guardrail prompts G1–G3 (+ Stage sets) | AC-15.1 |
| **G-SCRIPTS** | [`G-SCRIPTS/`](./G-SCRIPTS/) | Scripted sessions (perf, Mock Court, dependency) | AC-2.1, AC-3.x, AC-4.2, Stage 2 scripts |

**Inventory:** [`MANIFEST.json`](./MANIFEST.json)  
**Policy (SKIP vs FAIL, authorship):** [`POLICY.md`](./POLICY.md)

---

## Critical rule — no invented legal truth

Micro-fixtures in this batch are **`SYNTHETIC_PLACEHOLDER`** only:

- They exist so the **eval harness can discover sets** and exercise loaders.
- They are **NOT** authoritative known-answers for release gates.
- Do **not** train, tune, or claim AC pass rates against placeholder content.
- Real golden answers must be authored by a qualified legal reviewer (see POLICY.md).

If a test requires real case material that is not yet authored → status **`SKIP`** or **`BLOCKED`**, never silent **`PASS`**.

---

## Fixture layout (every set)

```
G-*/ 
  README.md
  fixtures/
    <fixture_id>/
      meta.json           # status, jurisdiction, AC targets, authorship
      ... input stubs ...
      known_answer.json   # expected outputs (PLACEHOLDER until authored)
```

---

## Status values (`meta.json` → `status`)

| Status | Meaning |
|--------|---------|
| `SYNTHETIC_PLACEHOLDER` | Scaffold only; not valid for AC PASS |
| `DRAFT` | Human-authored draft; not yet locked |
| `GOLDEN` | Locked known-answer; eval may FAIL on mismatch |
| `RETIRED` | Kept for history; harness ignores |

---

## Next batch

**B-S0-05** builds `/eval` to discover these sets and map every `AC-x.y` → PASS / FAIL / SKIP / BLOCKED.
