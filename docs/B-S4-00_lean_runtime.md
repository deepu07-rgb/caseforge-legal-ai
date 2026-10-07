# B-S4-00 — Lean runtime (default model tier, optional add-on, ≤ 4 GiB base install)

**Status (2026-10-08): PARTIAL — infrastructure done; no small default model has qualified, so the base install is still over the owner's 4 GiB cap.**

**Update (round 2, same day):** phi4-mini (MIT, 2.32 GiB) passes B-S3-13 at 1.0 but scores 0.80 on the new held-out set G-MODEL-HELDOUT (AC-S4-00.6) and 3/5 in Mock Court; llama3.2:3b rejected (3 critical fails, removed). The small-model prompt v2 gave no measurable gain and is not adopted. The held-out set showed the incumbent 7B also fails a novel injection case (it advised citing a planted fake authority) — a shared weakness for a deterministic host guard, not a prompt fix. Rust toolchain repaired (Strawberry Perl reinstalled; `msvc-env.ps1` now auto-discovers MSVC 14.44 in VS 18); `cargo test --lib` 82/82 pass incl. the new tier test; host tier selector (`llm_tiers`, `llm_set_tier`, UI) built and type-checked. Full record: `eval/bakeoff/default_model_b_s4_00.json`. Awaiting owner choice of default (A: phi4-mini ≈ 3.0 GiB base; B: keep 7B, 5.08 GiB).

## Implemented

- **Tiered model contracts.** `contracts/supported_models.json` = DEFAULT tier (base install); `contracts/supported_models_addon.json` = OPTIONAL_ADDON (7B, ≥ 8 GB RAM, outside the base cap). Both pin Ollama 0.35.1, digest, licence-text SHA-256, prompt hash.
- **Sidecar (`infer_llm.py` 0.4.0).** Tier chosen by `CASEFORGE_LLM_TIER` (`default`/`addon`); unknown tier fails closed; add-on refused under 8 GB RAM; new `llm.tiers` method (`auto_download: false`); generic `<arch>.context_length` (no longer Qwen2-only); licence names normalised; optional `think` setting from the manifest; hidden `<think>` reasoning is always stripped from output.
- **Fitness gate** `model_fitness_b_s3_13.py --tier default|addon` (add-on report: `b_s3_13_model_eval_addon.json`).
- **Honest size gate** `scripts/check_size_budget.py`: measures the **base install** (app + default model + speech + Ollama core/CPU/Vulkan) against `stage4_resource_budget.json` and FAILs above the cap; reports the add-on model and CUDA/ROCm accelerators separately. It previously counted only `ollama.exe` (25 MB); the real Ollama install is 2.79 GiB, of which 2.65 GiB is optional GPU libraries.
- **Gate** `eval/checks/lean_runtime_b_s4_00.py` (`npm run test:s4:00`): ACs from measured evidence, deterministic-engine model-free scan, tier-logic adversarial tests.

## Bake-off (details: `eval/bakeoff/default_model_b_s4_00.json`)

| Model | Disk | Fitness | Mock Court live | Warm turn | RAM | Result |
|---|---|---|---|---|---|---|
| qwen3:4b | 2.33 GiB | 0.67, critical FAIL | — | 3.5 s (27 s if thinking) | 4.1 GB | Rejected: thinking-only build |
| qwen3:4b-instruct | 2.33 GiB | 0.92, critical FAIL (prompt injection) | 3/5 | 3.3 s | 4.1 GB | HALT, not allowlisted |
| qwen2.5:7b | 4.38 GiB | 1.00, all critical pass | 5/5 | 4.9 s | 5.4 GB | Qualified (interim default + add-on) |

## Gate results

| AC | Status | Evidence |
|---|---|---|
| AC-S4-00.1 base install ≤ 4 GiB | **FAIL** | 5.08 GiB with the interim 7B default |
| AC-S4-00.2 licence verified | PASS | licence text hash from the model package |
| AC-S4-00.3 default passes fitness | PASS | 7B score 1.0 (default tier) |
| AC-S4-00.4 RAM ceiling | BLOCKED | Q-S4-12 ceiling not approved; measured 5.4 GB (7B), 4.1 GB (4B) |
| AC-S4-00.5 Stage 3 gates on default | PASS | B-S3-14 21/21; B-S3-16 13/13 structural + live |
| Deterministic engines model-free | PASS | 6 engines scanned |
| Tier contracts distinct | FAIL | default and add-on are the same model until a small default qualifies |

Regression: every previously passing gate still passes, except `encoding_b_s3_01`, whose size sub-gate now enforces the owner cap (encoding and Unicode sub-gates pass). `guardrails_b_s1_21` fails as before this batch.

## Known limitations / environment

- **Rust toolchain broken on this machine**: `C:\Strawberry` is empty (Perl missing) and `msvc-env.ps1` targets an absent VS 2022 BuildTools path (VS "18" Community is installed). Vendored OpenSSL cannot compile, so no Rust tests or host UI changes were possible. The host needs no change for tiers (the sidecar inherits `CASEFORGE_LLM_TIER`); a UI tier selector is deferred until the toolchain is repaired.
- `src-tauri/target` was removed by owner request; a regression check that invokes cargo partially rebuilt it (1.4 GiB), which was cleaned again. Checks that invoke cargo (B-S3-08, B-S3-09) were not re-run.
- Runtime packaging (ship Ollama CPU + Vulkan only) is a proposal in `stage4_resource_budget.json`; it must be proven on the built installer (B-S4-23).

## Verification

```powershell
npm run test:s4:00
npm run test:s3:13            # default tier, live
npm run test:s3:13:addon      # add-on tier, live
npm run test:size
```
