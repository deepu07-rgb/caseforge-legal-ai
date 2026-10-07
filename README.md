# CaseForge

**Offline Legal Intelligence for Litigators**  
Dual-market architecture (India + United States + global expansion ready)

Stage 1: **The Digest Engine** · Stage 2: **The Mock Court**

---

## Source documents

| Document | Role |
|----------|------|
| [`CaseForge_PRD_v1.0.md`](./CaseForge_PRD_v1.0.md) | **Layer 1** — Product Requirements (normative). What to build, acceptance criteria (AC-x.y), construction sequence. |
| [`CaseForge_Dev_Batches_v1.0.md`](./CaseForge_Dev_Batches_v1.0.md) | **Development batch plan** — Ordered work batches (B-S0-*, B-S1-*, B-S2-*). Implement only the current batch; halt if truth is missing. |
| [`CaseForge_PRD_v4.0.md`](./CaseForge_PRD_v4.0.md) | **Stage 4 PRD (DRAFT)** — Verified Advocacy & Chambers Edition. Extends v1.0; starts after the Stage 3 ship gate. |
| [`docs/CASEFORGE_STAGE4_AGENTIC_PLAYBOOK.md`](./docs/CASEFORGE_STAGE4_AGENTIC_PLAYBOOK.md) | **Stage 4 batch plan (DRAFT)** — B-S4-01 … B-S4-23 in three waves. |

**Prime rule:** If a detail is missing from the PRD, contracts, golden data, or batch plan — **HALT** and raise a `QUESTION`. Never invent.

---

## Repository layout (Phase 0)

```
/contracts   Layer 2 — frozen JSON schemas, IPC contracts, constants (B-S0-02+)
/golden      Layer 4 — golden datasets and known-answer fixtures (B-S0-04+)
/eval        Layer 4 — evaluation harness; every AC-x.y is a script (B-S0-05+)
/src         Application source (starts at B-S1-01; empty until then)
/docs        Project documentation
```

Product feature code does **not** begin until the S0 exit gate is green (batches B-S0-01 through B-S0-05 complete).

---

## Development process

1. Work batches **in order** as listed in `CaseForge_Dev_Batches_v1.0.md`.
2. A batch is **DONE** only when its listed AC / gates pass.
3. Do not implement later-batch features early.
4. Stage 2 work starts only after the Stage 1 ship gate (B-S1-22).

## Run the shell (B-S1-01)

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
npm install
npm run test:shell
npm run build
# MSVC Build Tools + Strawberry Perl required for SQLCipher/OpenSSL:
. .\src-tauri\msvc-env.ps1
cargo check --manifest-path src-tauri\Cargo.toml
npm run tauri:dev
```

See [`docs/B-S1-01_shell.md`](./docs/B-S1-01_shell.md).

## Stage 2 ship gate (B-S2-07 / v2.0)

```powershell
npm run ship:stage2
# writes eval/reports/stage2_ship_gate.json
```

See [`docs/B-S2-07_ship_gate.md`](./docs/B-S2-07_ship_gate.md) and [`docs/RELEASE_CHECKLIST_v2.0.md`](./docs/RELEASE_CHECKLIST_v2.0.md).

**Full tier (≥8 GB)** is required for live Mock Court voice simulation. Lite tier refuses with an honesty notice.

## Experience Store Stage 2 + full red-team (B-S2-06)

Unlocks `limb_outcome` / `bench_setting`. AC-15.1 full suite ≥200 prompts, zero G1–G3 violations.

```powershell
npm run test:s2redteam
python eval/tools/generate_stage2_redteam.py
```

See [`docs/B-S2-06_experience_redteam.md`](./docs/B-S2-06_experience_redteam.md).

## Bench Preparation Mode (B-S2-05)

AI plays **both counsel** only — never the decision-maker (G1).

```powershell
npm run test:benchprep
```

See [`docs/B-S2-05_bench_prep.md`](./docs/B-S2-05_bench_prep.md).

## Voice-in-court (B-S2-04)

Full-duplex English Mock Court + structured debrief. Lite tier is refused with an honesty notice.

```powershell
npm run test:voicecourt
```

See [`docs/B-S2-04_voice_court.md`](./docs/B-S2-04_voice_court.md).

## Adaptive Bench (B-S2-03)

Six generic archetypes + case-order signals + sliders — **no named judges**.

```powershell
npm run test:bench
```

See [`docs/B-S2-03_adaptive_bench.md`](./docs/B-S2-03_adaptive_bench.md).

## Mock Court text formats (B-S2-02)

Five sequential rehearsal formats (text-only). Adversary uses **case_graph + retrieval only**.

```powershell
npm run test:mockcourt
```

See [`docs/B-S2-02_mock_court.md`](./docs/B-S2-02_mock_court.md).

## Argument Integrity Engine (B-S2-01)

Deterministic Vulnerability Index + Missing Links — **no LLM in scoring**.

```powershell
npm run test:integrity
```

In the app: **Argument Tree** builds from `case_graph` only (AC-2.1). See [`docs/B-S2-01_integrity.md`](./docs/B-S2-01_integrity.md).

## Local LLM brain (offline reasoner)

CaseForge uses a **local** infer-llm sidecar (no cloud). Models are tiered (B-S4-00): `contracts/supported_models.json` is the **default** tier shipped in the base install and `contracts/supported_models_addon.json` the **optional add-on** (≥ 8 GB RAM). Select with `CASEFORGE_LLM_TIER=default|addon`. Until a small model passes the fitness gate, the qualified `qwen2.5:7b` (Ollama 0.35.1) is both — see `docs/B-S4-00_lean_runtime.md`.

```powershell
# Ensure Ollama is running and the model is pulled
ollama pull qwen2.5:7b

# Probe sidecar
python -c "import json,subprocess,sys; print(subprocess.check_output([sys.executable,'sidecars/infer_llm/infer_llm.py','--once',json.dumps({'jsonrpc':'2.0','id':1,'method':'llm.status','params':{}})],text=True))"
```

| Env | Purpose |
|-----|---------|
| `CASEFORGE_LLM_MODEL` | Model name (default `qwen2.5:7b`) |
| `CASEFORGE_OLLAMA_HOST` | Ollama base URL |
| `CASEFORGE_OPENAI_BASE` | Local OpenAI-compatible server (LM Studio / llama.cpp server) |
| `CASEFORGE_GGUF_PATH` | Direct GGUF via `llama-cpp-python` |

In the app: **Mock Court** uses the live model when ready; **System → Local LLM brain** shows status.

## Document triage (B-S1-05)

```powershell
npm run test:triage
npm run test:ac61
python eval/run_eval.py --ac AC-6.1
# Optional stdio sidecar:
# echo '{"jsonrpc":"2.0","id":1,"method":"docpipe.ping","params":{}}' | npm run docpipe
```

| Doc | Purpose |
|-----|---------|
| [`golden/G-DOCS/LABELING_RUBRIC.md`](./golden/G-DOCS/LABELING_RUBRIC.md) | How humans label the 6 classes |
| [`golden/G-DOCS/MVP_80_PLAN.md`](./golden/G-DOCS/MVP_80_PLAN.md) | First 80 GOLDEN pages + schedule |
| [`golden/G-DOCS/AUTHORING.md`](./golden/G-DOCS/AUTHORING.md) | Fixture format + lock rules |
| [`docs/B-S1-05_path_to_green.md`](./docs/B-S1-05_path_to_green.md) | Critical path to AC-6.1 |
| [`docs/MVP80_LABELING_RUNBOOK.md`](./docs/MVP80_LABELING_RUNBOOK.md) | Track B — human labeling ops for MVP-80 |
| [`docs/worksheets/Q-CONST-01_confidence_thresholds.md`](./docs/worksheets/Q-CONST-01_confidence_thresholds.md) | Product owner: freeze confidence cutoffs |
| [`docs/worksheets/Q-CG-01_case_graph_inventory.md`](./docs/worksheets/Q-CG-01_case_graph_inventory.md) | Product owner: freeze case_graph fields |
| [`eval/bakeoff/CANDIDATES.md`](./eval/bakeoff/CANDIDATES.md) | Vision bake-off shortlist (Q-VISION-01) |
| [`eval/bakeoff/README.md`](./eval/bakeoff/README.md) | Vision bake-off harness |

```powershell
npm run bakeoff
npm run gdocs:progress
python eval/tools/scaffold_gdocs_fixture.py --file page.png --id page_0001 --author "you"
# India / US case skeleton DRAFT shells (empty known-answers — no invented law):
npm run gcase-in:scaffold -- --id matter_0001 --author "you"
npm run gcase-us:scaffold -- --id matter_0001 --author "you"
```

**Stage 1 Digest RC `v1.0.0-rc1`** — B-S0 + B-S1-01…22 green (2026-08-08). Ship: `npm run ship:stage1`. Stage 2 starts at B-S2-01 after sign-off.

**Development mode: Option A (strict).** See [`docs/OPTION_A_runbook.md`](./docs/OPTION_A_runbook.md).

## Demo cases (synthetic · argument integrity practice)

Three pedagogical matters (easy / medium / hard) with full concept shells, argument trees, and “where to strengthen” notes. **Not GOLDEN. Not law. Does not unlock B-S1-06.**

| Case | Difficulty | Jurisdiction |
|------|------------|--------------|
| `demo/cases/demo_01_easy_dishonour` | Easy | India |
| `demo/cases/demo_02_medium_wrongful_term` | Medium | United States |
| `demo/cases/demo_03_hard_partition` | Hard | India |

```powershell
npm run demo:score
# or: python demo/tools/score_argument_tree.py --all --verbose
```

See [`demo/README.md`](./demo/README.md).

---

## Status

| Batch | Status |
|-------|--------|
| B-S0-01 Repository skeleton | Done |
| B-S0-02 Contracts freeze | Done (paths reserved; field inventory mostly BLOCKED — see `contracts/QUESTIONS.md`) |
| B-S0-03 Constants & errors | Done (PRD numbers frozen; confidence thresholds still Q-CONST-01) |
| B-S0-04 Golden scaffolds | Done (8 sets + micro-fixtures; placeholders only — not GOLDEN) |
| B-S0-05 Eval harness | Done (`python eval/run_eval.py`; full AC inventory) |
| S0 exit gate | Green |
| B-S1-01 Shell | Source complete; `cargo check` / `cargo test` green with MSVC + Perl (`src-tauri/msvc-env.ps1`) |
| B-S1-02 Vault | SQLCipher vault module + Argon2id freeze; `npm run test:vault` |
| B-S1-03 New Case wizard | Done — jurisdiction-first §5 wizard; `npm run test:wizard` |
| B-S1-04 Document intake | Done — encrypted originals + type tags; `npm run test:docs` |
| B-S1-05 docpipe triage | **Done** — AC-6.1 PASS (500 GOLDEN @ 100%); Q-VISION-01 Stage-1 freeze `caseforge.triage.heuristic.v1` |
| B-S1-06 docpipe recognition | **Done** — word confidences + Q-CONST-01 thresholds; Text layer UI (`npm run test:recognize`) |
| B-S1-07 skeleton / case_graph | **Done** — case_graph v1.0.0; AC-6.2 scorer; Skeleton tab (`npm run test:skeleton`) |
| B-S1-08 review UI | **Done** — side-by-side Review; AC-6.3/6.4 (`npm run test:review`) |
| B-S1-09 guided interview | **Done** — gaps → SOURCED_BY_COUNSEL; AC-6.5/6.6 (`npm run test:interview`) |
| B-S1-10 overnight batch | **Done** — checkpoints + kill/resume AC-6.8; AC-3.2 SKIP+hardware note (`npm run test:batch`) |
| B-S1-11 phone capture | **Done** — LAN-only QR/URL companion; AC-6.7 (`npm run test:companion`) |
| B-S1-12 knowledge packs | **Done** — Ed25519 .akpkg + AC-7.1 (`npm run test:packs`) |
| B-S1-13 retrieval / citations | **Done** — lexical index + GREEN/AMBER/RED (`npm run test:retrieval`) |
| B-S1-14 Digest / completeness | **Done** — CASEBOOK·CHAMBER·DIGEST + prep score (`npm run test:digest`) |
| Demo argument packs | **Done (parallel)** — 3 synthetic cases + VI scorer (`npm run demo:score`) + in-app Integrity panel. Not a batch unlock. |

---

## Non-goals (see PRD §20)

No cloud case management, no CLM, no generalist chatbot, no verdict prediction, no marketplace, no named-judge profiling, no automatic decision-making.

---

*CaseForge — formerly Aakhada (name retired).*
