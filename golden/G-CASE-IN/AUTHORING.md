# G-CASE-IN authoring — India case skeleton (DRAFT → GOLDEN)

**Jurisdiction:** India only (do not mix US material).  
**Primary ACs:** AC-6.2–AC-6.6, AC-6.8  
**Policy:** [`../POLICY.md`](../POLICY.md)

Coding agents **must not** invent parties, dates, statutes, or relief as GOLDEN truth.  
`case_graph` field inventory is still **BLOCKED** (Q-CG-01) — known-answers stay empty shells until Layer 2 freezes.

---

## Scaffold a DRAFT fixture

```powershell
# Empty shell (meta + empty concept_shell)
python eval/tools/scaffold_gcase_in_fixture.py --id matter_0001 --author "alice"

# With source documents
python eval/tools/scaffold_gcase_in_fixture.py --id matter_0001 --file plaint.pdf --file order.pdf --author "alice"

# Entire packet folder
python eval/tools/scaffold_gcase_in_fixture.py --id matter_0002 --dir C:\packets\india\matter_x --label "working title" --author "alice"

# Or unified tool
python eval/tools/scaffold_gcase_fixture.py --jurisdiction India --id matter_0003 --author "alice"
```

npm:

```powershell
npm run gcase-in:scaffold -- --id matter_0001 --author "alice"
```

### Output layout

```text
golden/G-CASE-IN/fixtures/<id>/
  meta.json              # status=DRAFT, jurisdiction=India
  known_answer.json      # empty concept_shell — not legal truth
  case_stub.txt
  sources/               # optional copied files
```

---

## What is intentionally empty

`known_answer.json` → `concept_shell`:

| Area | Value until human GOLDEN |
|------|---------------------------|
| chronology_dated_events | `[]` |
| parties_and_roles | `[]` |
| statutes_rules_invoked | `[]` |
| evidence_inventory | `[]` |
| procedural_posture | `null` |
| prayer_relief | `null` |
| contradiction_report | `[]` |

---

## Lock to GOLDEN (humans only)

1. Resolve Q-CG-01 (case_graph property freeze) or author against agreed interim schema marked provisional.  
2. Fill known-answer from rights-cleared India materials.  
3. Dual-review per POLICY.md.  
4. Set `status: GOLDEN`, `authoritative: true`, `authored_by`, `reviewed_by`, `locked_at`, `content_hash`.  
5. Never copy US facts into this set.

---

## Existing micro-fixture

| ID | Status |
|----|--------|
| `fixtures/micro_001` | `SYNTHETIC_PLACEHOLDER` (harness only) |
