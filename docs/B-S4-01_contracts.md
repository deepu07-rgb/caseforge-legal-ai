# B-S4-01 — Stage 4 contracts, constants, guardrails freeze

## Outcome

Every Stage 4 artefact has a frozen contract before any producer or consumer exists. Frozen Stage 1–3 contracts are untouched and hash-pinned. Every Stage 4 gate is inventoried with an honest computed status — none can pass yet.

## What was frozen

| Area | Contract(s) |
|---|---|
| Signed legal data in packs | `rule_table`, `treatment_edge`, `disclosure_template`, `akpkg_v2_1` (coexists with akpkg v2.0.0) |
| Lawyer features | `audit_report`, `audit_certificate`, `external_filing`, `witness_matrix`, `impeachment_kit`, `statute_mapping`, `foundation_check`, `deadline_record`, `rehearsal_timeline`, `translation_segment` |
| Collaboration | `bundle_manifest` (X25519 + HKDF-SHA256 + AES-256-GCM + Ed25519, counts-only redaction log) |
| Chambers Edition | `issue_matrix`, `cause_list_brief`, `judgment_scaffold` |
| Licensing | `licence_payload_v2` (coexists with v1.0.0) |
| Policy | `guardrails_stage4.json` (G7–G10), appended `errors.json` codes (GUARDRAIL, PACK, AUDIT, BUNDLE, CHAMBERS, LICENCE) |
| Owner decisions / proposals | `stage4_resource_budget.json` (4 GiB cap + tiered model approved), `stage4_thresholds.json` (proposed) |

## Design decisions

- **Versioned side-by-side, never in-place.** The Rust installer, `scripts/legal_pack_tool.py`, and Stage 1–3 gates pin `akpkg` 2.0.0, licence 1.0.0 and `guardrails.json`. Stage 4 adds successors next to them; originals are byte-identical and hash-pinned in the gate.
- **Separate Stage 4 inventory** (`eval/stage4_ac_inventory.json`), following the Stage 3 precedent, so `eval/run_eval.py` results for Stage 1/2 are unchanged.
- **Computed, not stored, statuses.** The inventory records dependencies (thresholds, legal-editor sign-off, open questions); the gate computes BLOCKED/SKIP. PASS is impossible from B-S4-01.
- **Planned graph amendment.** `ADVERSE_ASSERTION` / `TRANSLATED` origins and `asserting_party_id` are recorded in `MANIFEST.json` `planned_amendments` (case_graph v2.1.0) and applied with their producers in B-S4-06 / B-S4-15, so B-S3-06 stays green meanwhile.
- **Schemas enforce the product promises structurally**, e.g. the certificate scope statement is a `const`; ANALYSIS/FINDINGS/OPERATIVE_ORDER only accept judge-authored text; translation segments are `quotation_verifiable: false`; NEEDS_COUNSEL_INPUT cannot carry a governing code; custom packs cannot carry rule tables.

## Owner request handled in this batch (2026-10-08)

- Size/RAM: base install cap 4 GiB (target 3 GiB); default 3B/4B model, 7B optional add-on — recorded in `stage4_resource_budget.json`; new batch **B-S4-00 Lean runtime** added to the playbook and PRD §11.1. Measured baseline was 4.96 GiB (7B model = 4.38 GiB).
- Deterministic-first intelligence principle added to PRD §11.2.
- Cleanup: 44 build log files and `eval/__pycache__` sent to the Recycle Bin; `src-tauri/target` (1.9 GiB debug cache) removed with `cargo clean`. Rebuild with `. .\src-tauri\msvc-env.ps1` then `cargo build`.
- Licence caution recorded: Qwen2.5-3B is reported as non-commercial; default-model candidates must pass a verified licence check in B-S4-00.

## Verification

```powershell
npm run test:s4:01
```

Result (2026-10-08): `PASS` implementation — 22 contracts; 36/36 adversarial fixtures rejected; 58 ACs inventoried (BLOCKED 17, SKIP 41, PASS 0). Evidence: `eval/reports/b_s4_01_contracts.json`.

**Formal acceptance: BLOCKED** on Q-S4-01 (thresholds, `docs/worksheets/Q-S4-01_thresholds.md`), Q-S4-02 (legal editors), Q-S4-12 (allocations/RAM ceiling).

## Known limitations

- Schemas are self-contained (no cross-file `$ref`, matching repo convention), so shared shapes such as locators are duplicated per schema.
- Rust structs/bindings for these contracts are created by each implementing batch, not here.
- Pre-existing failures unrelated to this batch (seen in the baseline run before any change): `guardrails_b_s1_21` (infer_llm marker strings), `packs_b_s1_12` (expects akpkg status `FROZEN`), and B-S3-08/09 cargo steps (OpenSSL build needs `msvc-env.ps1`).
