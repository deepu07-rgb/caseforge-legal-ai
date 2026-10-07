# B-S3-16 — Grounded Local-LLM Mock Court

Status: implemented and verified on 2026-08-18.

Production Mock Court text and voice turns now use the exact allowlisted local configuration—Ollama 0.32.14 with Qwen2.5 7B Q4_K_M—after B-S3-15 has selected the current matter graph and counsel-confirmed legal working set. The deterministic template remains only as an explicitly labelled degraded rehearsal when the local model, schema, timeout, verifier, or guardrail gate fails.

## Turn contract and prompt registry

The model must return `caseforge.mock-court-turn.v2`, containing one prioritized challenge, its proposition, an unanswered-questions array, and next focus. Reference groups contain proposition IDs only. The model cannot directly author UI prose; the host reconstructs the visible challenge exclusively from the verified proposition ledger.

The Mock Court prompt is separately frozen in `contracts/mock_court_prompt_registry.json`, including its SHA-256 hash, schema path, one-repair limit, 15-second timeout, and 112-token output ceiling. Ollama receives the JSON Schema as its constrained response format. Embedded instructions in graph/legal sources remain inside untrusted-data delimiters.

## Verification and rendering

The adapter constructs support sources from the versioned graph, current counsel input, and B-S3-15 authority chunks. For supported factual, legal, and counsel propositions, the model's proposition text becomes the proposed exact excerpt and must match the identified source byte-for-byte. Legal propositions additionally require a permitted official/current authority ID and stable source/chunk IDs.

The B-S3-14 host verifier assigns all support states. `BLOCKED` propositions are removed from the displayed challenge. Unsupported legal propositions appear only as an explicit repository gap. If no proposed challenge survives, the model turn is refused and the degraded deterministic template is clearly labelled “NOT AI ADVERSARIAL ANALYSIS.” Guardrails run before model invocation and again after verified rendering.

## Multi-turn and failure policy

The host retains at most 12 verified user/assistant messages per session and never trusts history supplied by the renderer. A malformed response gets at most one schema-repair request, using only the remaining portion of the total 15-second generation budget. Timeout kills the sidecar process and renders no partial text. Model identity or prompt drift fails closed.

## Controlled evaluation

The live five-turn India partition-appeal case tests:

- weak title link;
- contradiction between witness and process register;
- missing oral-partition evidence;
- absent state tenancy authority;
- an instruction-injection string embedded in graph data.

Final results: 5/5 passed, no schema repairs, 8.91–12.24 seconds per warm turn, and 5.4 GB resident model footprint. Prompt-injection control and unsafe-output rates were zero. Frozen thresholds are in `contracts/mock_court_eval_thresholds.json`; evidence is written to `eval/reports/b_s3_16_grounded_mock_court.json`.

Run `npm run test:s3:16` for the live gate or append `-- --skip-live` only when intentionally performing structural verification without the model.
