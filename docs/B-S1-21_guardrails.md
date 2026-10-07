# B-S1-21 — Guardrails G1–G6 (Stage 1)

**Status:** DONE (2026-08-08)

## Goal

Hard-enforce PRD §15 guardrails on Stage 1 surfaces: system prompts, input/output filters, export footer/provenance, preparation language. Pipeline **`caseforge.guardrails.v1`**. Stage-1 red-team subset with **zero G1–G3 violations**.

## Rules

| ID | Rule | Enforcement |
|----|------|-------------|
| **G1** | No verdicts / guilt / sentencing | Input refuse + output scrub |
| **G2** | No citation bypassing retrieval | Input refuse + output scrub; retrieval GREEN/AMBER/RED |
| **G3** | Refuse fabrication / perjury coaching | Input refuse + output scrub |
| **G4** | Export footer required | `caseforge.export_footer.v1` (B-S1-15) |
| **G5** | Provenance on AI output | model_hash, prompt_hash, retrieval_ids |
| **G6** | Preparation language only | Digest copy + output note |

## Stage-1 red-team (partial AC-15.1)

| | |
|--|--|
| Suite | `caseforge.redteam.stage1.v1` |
| Fixture | `golden/G-REDTEAM/fixtures/stage1_024` (**GOLDEN**) |
| Size | **24** prompts (8× G1 / G2 / G3) |
| Pass | zero misses on must-refuse |
| Full 200 | Deferred to **B-S2-06** |

## Architecture

```
User prompt
  → guardrails.check_user_input  (G1–G3 hard refuse)
  → choreography.acquire primary_llm
  → infer-llm (system prompt includes G1–G6)
  → guardrails.check_model_output (G1–G3 scrub, G6 note)
  → UI
```

| Layer | Path |
|-------|------|
| Contract | `contracts/guardrails.json` |
| Host | `src-tauri/src/guardrails/mod.rs` |
| Sidecar prompt | `sidecars/infer_llm/infer_llm.py` |
| UI | **Guardrails** nav |

## Eval

```powershell
npm run test:guardrails
```

## Out of scope

- Full ≥200-prompt AC-15.1 suite (Stage 2)
- Soft “style” filtering beyond frozen patterns
