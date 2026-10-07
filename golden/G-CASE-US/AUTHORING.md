# G-CASE-US authoring — United States case skeleton (DRAFT → GOLDEN)

**Jurisdiction:** United States only (do not mix India material).  
**Primary ACs:** AC-6.2–AC-6.6, AC-6.8  
**Policy:** [`../POLICY.md`](../POLICY.md)

Coding agents **must not** invent parties, dates, rules, or relief as GOLDEN truth.  
`case_graph` field inventory is still **BLOCKED** (Q-CG-01) — known-answers stay empty shells until Layer 2 freezes.

---

## Scaffold a DRAFT fixture

```powershell
# Empty shell
python eval/tools/scaffold_gcase_us_fixture.py --id matter_0001 --author "bob"

# With source documents
python eval/tools/scaffold_gcase_us_fixture.py --id matter_0001 --file complaint.pdf --file order.pdf --author "bob"

# Entire packet folder
python eval/tools/scaffold_gcase_us_fixture.py --id matter_0002 --dir C:\packets\us\matter_y --label "working title" --author "bob"

# Or unified tool
python eval/tools/scaffold_gcase_fixture.py --jurisdiction US --id matter_0003 --author "bob"
```

npm:

```powershell
npm run gcase-us:scaffold -- --id matter_0001 --author "bob"
```

### Output layout

```text
golden/G-CASE-US/fixtures/<id>/
  meta.json              # status=DRAFT, jurisdiction=United States
  known_answer.json      # empty concept_shell — not legal truth
  case_stub.txt
  sources/               # optional copied files
```

---

## What is intentionally empty

Same empty `concept_shell` as G-CASE-IN (PRD concept areas only). Fill only after human review and schema freeze.

---

## Lock to GOLDEN (humans only)

1. Resolve Q-CG-01 or use agreed provisional schema.  
2. Author from rights-cleared US materials.  
3. Dual-review per POLICY.md.  
4. Set `status: GOLDEN` + authorship fields.  
5. Never copy India facts into this set.

---

## Existing micro-fixture

| ID | Status |
|----|--------|
| `fixtures/micro_001` | `SYNTHETIC_PLACEHOLDER` (harness only) |
