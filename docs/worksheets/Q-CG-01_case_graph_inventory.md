# Q-CG-01 — Case Graph property inventory worksheet

**Status:** DECIDED (2026-08-06) — frozen in `contracts/case_graph.schema.json` v1.0.0  
**Unblocks:** B-S1-07 skeleton extraction  
**Related:** Q-CG-02 (source pointer) · Q-CG-03 (confidence) · Q-CONST-01  

---

## 1. Owner decision block

| Field | Answer |
|-------|--------|
| Owner name | CaseForge Product Owner |
| Date | 2026-08-06 |
| Decision status | **DECIDED** |
| Schema version | **1.0.0** |

---

## 2–4. Inventories

Adopted as proposed in the worksheet draft (all concept areas).  
Full normative schema: `contracts/case_graph.schema.json`.

### SourcePointer (Q-CG-02)

Primary span model: **both** — `pdf_span` (char_start/char_end) preferred; `image_bbox` optional; `page_only` and `counsel` allowed.

### Confidence (Q-CG-03)

| Decision | Owner pick |
|----------|------------|
| Item confidence type | **float 0–1** |
| Same as word confidence? | **yes** |
| Missing confidence means | **force review** (do not ship EXTRACTED item without confidence) |

---

## 5. Normative invariants

| Invariant | OK |
|-----------|----|
| Stage 2 never parses raw documents — only this graph | yes |
| Every EXTRACTED item has source pointer + confidence | yes |
| Guided interview answers use SOURCED_BY_COUNSEL | yes |
| Jurisdiction matches case wizard freeze | yes |
| Completeness metrics never imply win prediction | yes |

---

## 6. Out of scope for v1 graph

| Excluded | Lives instead |
|----------|----------------|
| Argument limbs / VI | argument_node / Stage 2 |
| Citation GREEN/AMBER/RED | citation_record / B-S1-13 |
