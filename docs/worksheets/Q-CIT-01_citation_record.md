# Q-CIT-01 — Citation record property inventory worksheet

**Status:** DECIDED (2026-08-07) — frozen in `contracts/citation_record.schema.json` v1.0.0  
**Unblocks:** B-S1-13 retrieval + GREEN/AMBER/RED  
**Related:** AC-4.3 · AC-7.2 · Guardrail G2  

---

## 1. Owner decision block

| Field | Answer |
|-------|--------|
| Owner name | CaseForge Product Owner |
| Date | 2026-08-07 |
| Decision status | **DECIDED** |
| Schema version | **1.0.0** |

---

## 2. Property inventory (v1.0.0)

| Field | Type | Notes |
|-------|------|--------|
| schema_version | const `1.0.0` | Freeze tag |
| citation_id | string | Stable id |
| query_id | string \| null | Pipeline query |
| authority_string | string | Shown to counsel |
| verification_status | GREEN \| AMBER \| RED | **Every authority labeled** |
| match_score | number 0–1 | Best retrieve/corpus match |
| retrieved | boolean | In RETRIEVE set (G2) |
| user_confirmed | boolean | Allows RED into downstream |
| pack_id | string \| null | Source pack |
| corpus_locus | object \| null | path + optional char span + snippet |
| planted_fake | boolean | AC-4.3 marker |
| exclude_from_downstream | boolean | RED && !user_confirmed |
| pipeline | stages + index_id | Provenance |
| notes | string \| null | Human-readable reason |

---

## 3. Status rules

| Label | Rule |
|-------|------|
| **GREEN** | Authority resolves to retrieved corpus text (exact or high match, score ≥ ~0.45–0.72 band) |
| **AMBER** | Partial / fuzzy match in retrieved material — counsel review |
| **RED** | Not in corpus, planted fake, or cite **outside RETRIEVE set** (G2 hard fail) |

### State transitions (user confirm)

```text
any → VERIFY → GREEN | AMBER | RED
RED + user_confirmed=true → still labeled RED, exclude_from_downstream=false
RED + user_confirmed=false → exclude_from_downstream=true
```

Planted fakes (**PLANTED_FAKE**) are always RED (AC-4.3).

---

## 4. Normative invariants

| Invariant | OK |
|-----------|----|
| Every authority labeled GREEN/AMBER/RED | yes |
| Planted fake always RED (AC-4.3) | yes |
| RED excluded unless user confirms | yes |
| No citation bypassing retrieval (G2) | yes |
| Official packs: raw judgment + neutral citation only | yes |

---

## 5. Index note (Stage 1)

| Item | Value |
|------|--------|
| Stage-1 index | `caseforge.retrieve.lexical.v1` (TF-IDF lexical) |
| FAISS | Drop-in when embedding model bake-off freezes — same citation labels |

---

## 6. Out of scope for citation_record v1

| Excluded | Lives instead |
|----------|----------------|
| Full session turn provenance | session_turn / Q-ST-01 |
| Integrity VI scores | argument_node Stage 2 |
| Mock Court adversary cites | Stage 2 |
