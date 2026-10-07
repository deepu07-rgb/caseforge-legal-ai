# G-DOCS authoring guide — path to AC-6.1 green

**Ship gate:** ≥ 92% accuracy on **500 GOLDEN pages** (`contracts/constants.json` → `document_pipeline`).

Coding agents **must not** flip fixtures to `GOLDEN` or invent real case law labels. Humans (product / legal-content owner) lock GOLDEN per [`../POLICY.md`](../POLICY.md).

## Companion docs (start here)

| Doc | Use |
|-----|-----|
| [`LABELING_RUBRIC.md`](./LABELING_RUBRIC.md) | Class definitions + dual-review rules |
| [`MVP_80_PLAN.md`](./MVP_80_PLAN.md) | First 80 pages before scaling to 500 |
| [`../../docs/B-S1-05_path_to_green.md`](../../docs/B-S1-05_path_to_green.md) | End-to-end critical path |
| [`../../eval/bakeoff/README.md`](../../eval/bakeoff/README.md) | Vision bake-off harness |

### Scaffold a DRAFT fixture from a page file

```powershell
python eval/tools/scaffold_gdocs_fixture.py --file C:\path\to\page.png --id page_0001 --author "alice"
# optional proposed class (still DRAFT):
python eval/tools/scaffold_gdocs_fixture.py --file notes.jpg --id page_0002 --class clean_handwriting --author "alice"
```

---

## 1. Minimum ship set

| Requirement | Value |
|-------------|--------|
| Page count | ≥ 500 labeled pages |
| Classes covered | All 6 triage classes (balanced as practical) |
| Status | `meta.json` + `known_answer.json` → `status: "GOLDEN"` |
| Review | `authored_by`, `reviewed_by`, `locked_at` filled |
| Rights | Rights-cleared or synthetic-but-reviewed material only |

Suggested mix (guidance, not PRD-normative):

| Class | Approx share |
|-------|----------------|
| typed_english | 40–50% |
| clean_handwriting | 15–20% |
| messy_handwriting | 10–15% |
| mixed | 10% |
| other | 10% |
| non_text | 5–10% |

---

## 2. Fixture layout

```text
golden/G-DOCS/fixtures/<fixture_id>/
  meta.json
  known_answer.json
  page_stub.txt          # optional text layer
  page.png | page.pdf    # optional binary (set inputs.content_file)
```

### `meta.json` (GOLDEN)

```json
{
  "fixture_id": "G-DOCS/page_0042",
  "set_id": "G-DOCS",
  "fixture_version": "1.0.0",
  "status": "GOLDEN",
  "authoritative": true,
  "batch": "B-S1-05",
  "primary_acs": ["AC-6.1"],
  "jurisdiction": null,
  "description": "Clean typed English order page",
  "inputs": {
    "page_text_stub": "page_stub.txt",
    "content_file": "page.png"
  },
  "known_answer": "known_answer.json",
  "authored_by": "name",
  "reviewed_by": "name",
  "locked_at": "2026-…",
  "content_hash": "sha256:…"
}
```

### `known_answer.json` (single page)

```json
{
  "schema_version": "0.1.0",
  "authoritative": true,
  "status": "GOLDEN",
  "fixture_id": "G-DOCS/page_0042",
  "triage": {
    "page_index": 0,
    "class": "typed_english"
  },
  "eval_policy": "SHIP"
}
```

### Multi-page document

Use `triage` as an array:

```json
"triage": [
  { "page_index": 0, "class": "typed_english" },
  { "page_index": 1, "class": "non_text" }
]
```

---

## 3. Score the set

```powershell
python eval/checks/ac_6_1_triage_score.py
python eval/checks/ac_6_1_triage_score.py --json
python eval/run_eval.py --ac AC-6.1
```

| Result | Meaning |
|--------|---------|
| **SKIP** | No GOLDEN pages yet |
| **FAIL** | GOLDEN pages exist but count &lt; 500 or accuracy &lt; 92% |
| **PASS** | Ready for B-S1-06 |

---

## 4. Classifier note

Until vision bake-off (PRD §21 #3 / Q-VISION-01), the product uses  
`caseforge.triage.heuristic.v1` (provisional). Real scanned pages may fail AC-6.1 until a vision model is selected and the classifier upgraded — that is expected; stay in B-S1-05 until PASS.

---

## 5. What is already in repo

| Fixtures | Status | Role |
|----------|--------|------|
| `micro_001` … `micro_006` | `SYNTHETIC_PLACEHOLDER` | Plumbing + marker self-test only |

Do **not** count these toward AC-6.1 ship.
