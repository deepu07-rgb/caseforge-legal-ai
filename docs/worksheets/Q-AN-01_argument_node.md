# Q-AN-01 — Argument node property inventory worksheet

**Status:** DECIDED (2026-08-08) — frozen in `contracts/argument_node.schema.json` v1.0.0  
**Unblocks:** B-S2-01 Argument Tree + Integrity Engine  
**Related:** PRD §8 · AC-8.1 · AC-8.2 · AC-8.3 · `constants.json` → `vulnerability_index`

---

## 1. Owner decision block

| Field | Answer |
|-------|--------|
| Owner name | CaseForge Product Owner |
| Date | 2026-08-08 |
| Decision status | **DECIDED** |
| Schema version | **1.0.0** |
| Scorer id | `caseforge.integrity.v1` |

---

## 2. Tree shape (PRD §8)

| Role | Meaning |
|------|---------|
| `relief_root` | Root = relief sought |
| `argument_limb` | Argument limbs under the root |
| `anchor_leaf` | Leaves = anchors (fact / evidence / law / exposure pointers) |

Hierarchy: **root → limbs → anchors**. One tree per case (`argument_tree.json`).

---

## 3. Four links (normative)

| Link | Score | Meaning |
|------|-------|---------|
| Fact | integer 0–100 | Factual foundation strength |
| Evidence | integer 0–100 | Exhibit / proof support |
| Law | integer 0–100 | Legal theory / authority support |
| Exposure | integer 0–100 | How well counter-exposure is addressed (higher = better prepared) |

**VI (Vulnerability Index):** `VI = 100 − min(Fact, Evidence, Law, Exposure)`  
Higher VI = weaker limb (PRD §8).

**Missing link threshold (frozen):** **60** — a link score **&lt; 60** is a Missing Link.

---

## 4. Property inventory (v1.0.0)

### ArgumentTree (document root)

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| schema_version | string const `1.0.0` | yes | |
| case_id | string | yes | Vault case id |
| generated_at | string | yes | ISO / epoch-ms string |
| pipeline | object | yes | `scorer_id`, optional `builder_id` |
| scoring_rules | object | yes | formula, threshold, link scale |
| relief_root | ReliefRoot | yes | |
| limbs | Limb[] | yes | May be empty only for draft |
| meta | object \| null | no | demo/golden flags |

### ReliefRoot

| Field | Type | Required |
|-------|------|----------|
| node_id | string | yes |
| node_role | `"relief_root"` | yes |
| label | string | yes |
| jurisdiction | string \| null | no |
| relief_ref | string \| null | no — case_graph prayer_relief item id |

### Limb

| Field | Type | Required |
|-------|------|----------|
| node_id | string | yes |
| node_role | `"argument_limb"` | yes |
| parent | string | yes — root node_id |
| label | string | yes |
| theory | string \| null | no |
| links | Links | yes |
| anchors | Anchor[] | yes (may be empty) |
| planted_gaps | PlantedGap[] | no — GOLDEN / AC-8.1 |
| expected_VI | integer \| null | no — eval only |

### Links

`Fact`, `Evidence`, `Law`, `Exposure` — each integer 0–100.

### Anchor

| Field | Type | Required |
|-------|------|----------|
| node_id | string | yes |
| node_role | `"anchor_leaf"` | yes |
| kind | `fact` \| `evidence` \| `law` \| `exposure` | yes |
| ref | string | yes — case_graph item id or label |
| text | string | yes |
| case_graph_path | string \| null | no — e.g. `evidence[0]` |

### PlantedGap (AC-8.1)

| Field | Type | Required |
|-------|------|----------|
| gap_id | string | yes |
| link | Fact \| Evidence \| Law \| Exposure | yes |
| description | string | yes |
| severity | `critical` \| `material` \| `minor` | yes |

### MissingLinkRecord (scorer **output** — always cites inputs)

| Field | Type | Notes |
|-------|------|-------|
| limb_id | string | |
| link | Fact…Exposure | |
| score | 0–100 | |
| threshold | 60 | |
| rank | 1-based in panel | weaker first |
| inputs_cited | string[] | anchor refs + planted gap ids + case_graph paths |
| planted_gap_id | string \| null | when from planted_gaps |
| description | string \| null | |

---

## 5. Normative invariants

| Invariant | OK |
|-----------|----|
| Scoring is pure deterministic code; **no LLM** in scoring path (AC-8.3) | yes |
| VI formula exact: `100 − min(four links)` | yes |
| Missing Links panel always cites its inputs | yes |
| Planted gap appears in top-2 Missing Links (AC-8.1) | yes |
| VI recompute ≤ 5 s (AC-8.2) | yes |
| Stage 2 consumes only case_graph (+ this tree); never re-parses documents (AC-2.1) | yes |
| VI is preparation vulnerability — never win prediction (G6) | yes |

---

## 6. Out of scope for v1

| Excluded | Lives instead |
|----------|----------------|
| Live adversary dialogue | B-S2-02 |
| Voice court | B-S2-04 |
| Bench archetypes | B-S2-03 |
| LLM argument drafting | Optional UI aid only; never feeds scorer |
