# B-S2-01 — Argument Tree + Integrity Engine

**Status:** DONE (2026-08-08)  
**Scorer:** `caseforge.integrity.v1`  
**Schema:** `contracts/argument_node.schema.json` v1.0.0 (Q-AN-01 frozen)

## Goal

Build the Argument Tree and Vulnerability Index / Missing Links panel with **zero LLM in the scoring path**.

## Normative rules (PRD §8)

| Rule | Implementation |
|------|----------------|
| Root = relief, limbs = arguments, leaves = anchors | `argument_tree` JSON |
| Four links 0–100: Fact, Evidence, Law, Exposure | `links` object |
| VI = 100 − min(links) | `integrity::Links::vi` |
| Missing Links cite inputs | `MissingLinkRecord.inputs_cited` |
| No LLM in scoring | Module has no infer_llm / network; `llm_used: false` |

## AC gates

| AC | Gate |
|----|------|
| **AC-8.1** | Planted gap id appears in top-2 Missing Links (`golden/G-INTEGRITY/fixtures/planted_gap_evidence`) |
| **AC-8.2** | VI recompute ≤ 5 s |
| **AC-8.3** | Pure deterministic code; static audit bans LLM imports in `integrity/` |

## Commands

| IPC | Purpose |
|-----|---------|
| `integrity_policy` | Frozen constants + scorer id |
| `integrity_score_tree` | Score any tree JSON |
| `integrity_score_case` | Score vault-stored tree |
| `integrity_build_from_graph` | Build tree from `case_graph` only (AC-2.1) |
| `argument_tree_get` / `argument_tree_put` | Vault persistence |

## UI

**Argument Tree** nav → `ArgumentTreeView`  
Shows relief root, limbs with link meters, VI chips, and Missing Links panel with cited inputs.

Optional LLM **Argument Builder** remains a draft aid only — it never feeds the scorer.

## Eval

```powershell
npm run test:integrity
```

## Stage 2 dependency

Document pipeline (`docpipe`) must not import this module (AC-2.1). Stage 2 consumes **only** `case_graph` (+ persisted argument tree).
