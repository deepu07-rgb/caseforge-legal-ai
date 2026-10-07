# CaseForge — CTO Technical Manual & Manual Test Playbook

**Product:** CaseForge (formerly Aakhada — name retired)  
**Version covered:** `2.0.0-rc1` (Stage 1 Digest + Stage 2 Mock Court RC)  
**Audience:** CTO, engineering leads, QA, and power users doing **manual dogfood** (create cases → upload documents → run arguments → inspect results)  
**Document status:** Living engineering reference (derived from repo source, contracts, PRD, and batch docs)  
**Last aligned to tree:** 2026-08-12  

**Colleague / first-pass companion (plain language):**  
[`CASEFORGE_COLLEAGUE_REVIEW_OVERVIEW.md`](./CASEFORGE_COLLEAGUE_REVIEW_OVERVIEW.md) — purpose, core structure, and review agenda for non-deep-technical readers. Share that overview **with** this manual when circulating for review.

---

## Table of contents

1. [Executive summary](#1-executive-summary)
2. [Architecture at a glance](#2-architecture-at-a-glance)
3. [Technology stack](#3-technology-stack)
4. [Databases & on-disk layout](#4-databases--on-disk-layout)
5. [LLM models, speech & sidecars](#5-llm-models-speech--sidecars)
6. [Security, licensing & guardrails](#6-security-licensing--guardrails)
7. [Complete application file map](#7-complete-application-file-map)
8. [End-to-end product workflows](#8-end-to-end-product-workflows)
9. [Manual testing playbook (QA / CTO)](#9-manual-testing-playbook-qa--cto)
10. [Automated gates & demos](#10-automated-gates--demos)
11. [Hardware tiers & performance budgets](#11-hardware-tiers--performance-budgets)
12. [Known limitations & non-goals](#12-known-limitations--non-goals)
13. [Quick command reference](#13-quick-command-reference)

---

## 1. Executive summary

CaseForge is an **offline-first desktop legal intelligence product** for litigators. It does **not** replace counsel and does **not** predict verdicts. It provides:

| Stage | Product name | What it delivers |
|-------|----------------|------------------|
| **Stage 1** | **The Digest Engine** | Encrypted vault, case wizard, document intake, page triage, text recognition, case graph (skeleton), review, guided interview, overnight batch, phone capture companion, knowledge packs, retrieval/citations, digest packs, drafting, voice lexicon, experience store, licence, protection, model choreography, guardrails |
| **Stage 2** | **The Mock Court** | Argument integrity (VI + Missing Links), text Mock Court formats, Adaptive Bench, voice-in-court (Full tier), Bench Prep, Stage-2 experience red-team |

**Design laws (non-negotiable):**

- **Nothing leaves the machine** by product default (air-gap ready).
- **User case data** is encrypted with a **local vault password** (SQLCipher + Argon2id). There is **no cloud account / user ID**.
- **No verdict prediction** (G6); **no named-judge profiling**; **no fabrication** assistance (G1–G3).
- Truth is contract-driven: **PRD → contracts → golden/eval → code**. Missing truth ⇒ **HALT** (Option A), never invent law for ship gates.

**Manual test intent:** create real matters (or load demo packs), ingest documents, run the pipeline, inspect argument trees / digests / mock court turns, and verify honesty notices where features refuse (Lite tier, missing models, guardrails).

---

## 2. Architecture at a glance

```text
┌──────────────────────────────────────────────────────────────────────┐
│  Desktop shell (Tauri 2 + WebView)                                   │
│  ┌────────────────────────────┐   IPC invoke (typed)                  │
│  │ React 18 + TypeScript UI   │ ──────────────────────────────────┐  │
│  │ src/App.tsx + components   │                                   │  │
│  │ src/lib/ipc.ts             │                                   ▼  │
│  └────────────────────────────┘                     ┌────────────────┴─┐
│                                                     │ Rust core        │
│                                                     │ src-tauri/src    │
│                                                     │ vault · docpipe  │
│                                                     │ integrity · court│
│                                                     │ packs · licence  │
│                                                     └────────┬─────────┘
│                                                              │
│              SQLCipher DBs          stdio JSON-RPC sidecars  │
│         ┌───────────────┐         ┌──────────────────────────┘
│         │ app.db        │         │  python sidecars/
│         │ cases/*/case.db│        │  · docpipe (triage/OCR path)
│         └───────────────┘         │  · infer_llm (local reasoner)
│                                   │  · infer_speech (STT/TTS)
└───────────────────────────────────┴──────────────────────────────────┘
         Optional local model host: Ollama / LM Studio / llama.cpp GGUF
```

| Layer | Responsibility |
|-------|----------------|
| **UI (`src/`)** | Screens, wizards, progress, export, demo integrity lab; never holds long-term secrets |
| **Rust shell (`src-tauri/`)** | Vault crypto, migrations, case lifecycle, pipelines, scoring, IPC surface, hardware gate |
| **Contracts (`contracts/`)** | Frozen schemas, constants, guardrails, error codes — normative for code + eval |
| **Sidecars (`sidecars/`)** | Offline process workers over JSON-RPC (heavy / ML-adjacent work) |
| **Golden + eval (`golden/`, `eval/`)** | Known-answer fixtures + AC harness (ship gates) |
| **Demo (`demo/`)** | Synthetic pedagogical packs (**not** GOLDEN, **not** law) |

---

## 3. Technology stack

### 3.1 Frontend

| Technology | Version (package) | Role |
|------------|-------------------|------|
| **React** | ^18.3 | UI component model |
| **TypeScript** | ^5.7 | Typed application surface |
| **Vite** | ^6 | Dev server + production bundler |
| **@vitejs/plugin-react** | ^4.3 | React transform |
| **@tauri-apps/api** | ^2.5 | Frontend → Rust `invoke` |
| **@tauri-apps/plugin-shell** | ^2.2 | Controlled shell capabilities |
| **@tauri-apps/cli** | ^2.5 (dev) | `tauri dev` / `tauri build` |

**Entry:** `index.html` → `src/main.tsx` → `src/App.tsx` · styles in `src/styles.css`.

### 3.2 Desktop runtime

| Technology | Role |
|------------|------|
| **Tauri 2** | Native window, OS integration, secure IPC, packaging |
| **OS WebView** | Renders the React UI (Edge WebView2 on Windows) |
| **Product ID** | `app.caseforge.desktop` (`src-tauri/tauri.conf.json`) |

### 3.3 Rust backend (selected crates)

| Crate | Role |
|-------|------|
| **tauri / tauri-plugin-shell** | App shell + plugin |
| **rusqlite** (`bundled-sqlcipher-vendored-openssl`) | Encrypted SQLite (SQLCipher AES-256) |
| **argon2** | Argon2id KDF for vault password → DB key |
| **zeroize** | Clear derived keys from memory when locking |
| **serde / serde_json** | IPC + stored JSON documents |
| **uuid** | Case / document / turn IDs |
| **ed25519-dalek** | Offline licence + pack signatures |
| **aes-gcm** | At-rest protection for packs/models (not user vault) |
| **sha2 / hex / base64 / rand / regex** | Hashing, encoding, crypto helpers, text patterns |

**Windows build note:** MSVC Build Tools + Perl required for SQLCipher/OpenSSL vendored build. Use `src-tauri/msvc-env.ps1` before `cargo` / `npm run tauri:dev`.

### 3.4 Python ecosystem (sidecars + eval)

| Area | Role |
|------|------|
| **sidecars/docpipe** | Page triage classifier + recognition helpers (JSON-RPC stdio) |
| **sidecars/infer_llm** | Local LLM chat/status (Ollama / OpenAI-compat / llama.cpp) |
| **sidecars/infer_speech** | Offline STT/TTS status + stubs / local backends |
| **eval/** | Acceptance-criteria scripts, ship gates, golden tooling |
| **demo/tools** | Argument-tree scorer for synthetic demos |

### 3.5 Data & interchange formats

| Format | Use |
|--------|-----|
| **JSON Schema** (`contracts/*.schema.json`) | Structural truth for case_graph, citations, turns, etc. |
| **JSON policy files** | `constants.json`, `guardrails.json`, `protection.json`, trust roots |
| **SQL migrations** | App vault + per-case vault schema evolution |
| **`.akpkg`** | Signed knowledge packs (Ed25519) |
| **Licence payload** | Offline activation (Ed25519 + machine fingerprint) |
| **JSON-RPC 2.0** | Sidecar IPC envelopes |

---

## 4. Databases & on-disk layout

### 4.1 Where data lives (Windows)

| Path | Contents |
|------|----------|
| `%APPDATA%\app.caseforge.desktop\vaults\` | **Authoritative encrypted vaults** |
| `...\vaults\app\app.db` + `kdf.json` | App-level vault (case registry, lexicon, experience, licence meta) |
| `...\vaults\cases\{case_id}\case.db` + `kdf.json` | Per-case vault (documents BLOBs, triage, text layers, case_graph, argument tree, batch jobs, …) |
| `...\vaults\packs\` | Installed knowledge packs + registry (when used) |
| `...\vaults\models\` / `protection\` | Model cache / seals (protection layer; **not** user case crypto) |

**KDF sidecar (`kdf.json`):** public salt + Argon2id params only — **never** the password.

**Crypto freeze (Q-DB-01):**

| Param | Value |
|-------|-------|
| Algorithm | Argon2id |
| memory_kib | 65536 (64 MiB) |
| iterations | 3 |
| parallelism | 4 |
| salt_len | 16 bytes |
| key_len | 32 bytes |
| Page cipher | SQLCipher AES-256 with application-provided raw key |

Wrong password ⇒ `CF_VAULT_PASSPHRASE_INVALID` (no recovery). **Forgot password** ⇒ wipe local vault (`vault_reset_local` UI or `scripts/reset-vault.bat`).

### 4.2 App vault schema (migrations)

| Migration | File | Purpose |
|-----------|------|---------|
| 001 | `app_001_init.sql` | `schema_migrations`, `vault_meta`, `cases` registry |
| 002 | `app_002_case_wizard.sql` | Wizard columns on `cases` (jurisdiction, side, court, …) |
| 003 | `app_003_lexicon.sql` | Voice lexicon entries (STT/TTS self-correction) |
| 004 | `app_004_experience.sql` | Experience Store records (local patterns) |
| 005 | `app_005_licence.sql` | Licence state / payload storage |

### 4.3 Per-case vault schema (migrations)

| Migration | File | Purpose |
|-----------|------|---------|
| 001 | `case_001_init.sql` | Base meta |
| 002 | `case_002_profile.sql` | Case profile fields |
| 003 | `case_003_documents.sql` | **Encrypted original files as BLOBs** + type tags |
| 004 | `case_004_page_triage.sql` | Per-page triage class + confidence |
| 005 | `case_005_page_text_layer.sql` | Recognized text + word confidences |
| 006 | `case_006_case_graph.sql` | Single JSON **case_graph** document |
| 007 | `case_007_review_corrections.sql` | Counsel corrections (review UI) |
| 008 | `case_008_guided_interview.sql` | Interview session / gap fill |
| 009 | `case_009_batch_jobs.sql` | Overnight batch checkpoints |
| 010 | `case_010_argument_tree.sql` | Argument tree JSON + VI snapshot |

**Design choice:** heavy domain objects (`case_graph`, argument tree) are versioned **JSON documents** inside SQLCipher tables rather than fully normalized relational graphs — validates against contracts, easy overwrite on re-extract.

### 4.4 What is *not* a “user account database”

There is **no** central auth DB, **no** multi-tenant cloud schema, **no** username. Identity for encryption is:

```text
vault_password → Argon2id(salt) → 32-byte key → SQLCipher open
```

---

## 5. LLM models, speech & sidecars

### 5.1 Local LLM brain (`sidecars/infer_llm`)

CaseForge uses a **local** reasoner only. Default preferred backend on developer machines:

| Item | Value |
|------|--------|
| Default host | **Ollama** |
| Default model | **`qwen2.5:7b`** |
| Cloud | **Forbidden** in product path |

**Backends (env-selected):**

| Backend | How to enable | Notes |
|---------|---------------|-------|
| **ollama** (default) | `CASEFORGE_LLM_MODEL`, `CASEFORGE_OLLAMA_HOST` | Best DX; keep model pulled |
| **openai_compat** | `CASEFORGE_OPENAI_BASE` (e.g. LM Studio `http://127.0.0.1:1234/v1`) | Local OpenAI-compatible server |
| **llama_cpp** | `CASEFORGE_GGUF_PATH` + `llama-cpp-python` | Direct GGUF load |
| Force | `CASEFORGE_LLM_BACKEND=ollama\|openai_compat\|llama_cpp` | Override auto-detect |

**Used for (examples):** Mock Court generation, Argument Builder coaching text, counsel prep tips.  
**Not used for:** Vulnerability Index scoring (deterministic integrity engine — **no LLM**).

**Smoke:**

```powershell
npm run llm:status
# or
python sidecars/infer_llm/infer_llm.py --once "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"llm.status\",\"params\":{}}"
```

In-app: **System → Local LLM brain** (status chip).

### 5.2 Document pipeline (`sidecars/docpipe`)

| Capability | Identifier / notes |
|------------|--------------------|
| Triage classifier | `caseforge.triage.heuristic.v1` (Stage-1 freeze; 6 classes) |
| Recognizer | `caseforge.recognize.provisional.v1` (word confidences; thresholds from `constants.json`) |
| Transport | JSON-RPC 2.0 over **stdio** |

Triage classes include: `typed_english`, `clean_handwriting`, `messy_handwriting`, `mixed`, `other`, `non_text`.

### 5.3 Speech (`sidecars/infer_speech`)

| Item | Policy |
|------|--------|
| Language | **English primary** (non-English rejected with pin error) |
| Whisper size freeze | **small** (Stage 1 provisional) |
| Network | `network_required: false` |
| Roles (TTS) | bench / opposition / witness (product roles) |

Full-duplex **voice Mock Court** is a **Full-tier** capability; Lite tier must **honesty-refuse**.

### 5.4 Model choreography (B-S1-20)

From frozen constants:

- At most **one large model** resident at a time on 8 GB-class machines.
- Primary LLM and vision concurrent load **forbidden** on constrained RAM.
- Idle unload after configured seconds; UI status chip required.
- Total RSS budget **≤ 7.2 GB** (scripted worst case).

---

## 6. Security, licensing & guardrails

### 6.1 Vault password UX

| State | UI behaviour |
|-------|----------------|
| No vault yet | **Create vault password** + confirm → `vault_init_unlock` |
| Vault exists | **Sign in** → `vault_unlock` |
| Forgot password | **Reset local vault** → `vault_reset_local` (destroys app + case DBs) or `scripts/reset-vault.bat` |

Password never leaves the device. Wrong password cannot decrypt.

### 6.2 Licence (B-S1-18)

- Offline **Ed25519** signed licence payload.
- States: **UNLICENSED / ACTIVE / EXPIRED**.
- **EXPIRED = read-only** — user data never ransomed.
- Machine binding via fingerprint policy (see `licence_payload` contract).

### 6.3 Protection (B-S1-19)

- AES-256-GCM protection for **packs/models** at rest.
- **Never** encrypts or ransoms user case vaults (SQLCipher + user password remains independent).

### 6.4 Guardrails G1–G6 (B-S1-21 / Stage 2 red-team)

Hard product filters (examples):

| ID | Intent |
|----|--------|
| **G1** | No guilt determination / verdict prediction / sentencing forecast |
| **G2** | Citation honesty (no fake authorities) |
| **G3** | No fabrication of evidence / perjury coaching |
| **G5** | Provenance on session turns |
| **G6** | Preparation only — not a real hearing / outcome prediction |

### 6.5 Knowledge packs (B-S1-12)

- `.akpkg` packages signed with Ed25519 (`pack_trust.json` / trust roots).
- Official vs custom pack directories isolated under vault packs root.
- Retrieval cites local corpus only → GREEN / AMBER / RED labels.

---

## 7. Complete application file map

> Scope: **product-owned source and configuration**. Excludes `node_modules/`, `src-tauri/target/`, and other generated caches. Paths are relative to the repository root.

### 7.1 Repository root

| Path | What it is | Why it exists |
|------|------------|---------------|
| `package.json` / `package-lock.json` | Node scripts + frontend deps | Dev/build/test entrypoints (`tauri:dev`, AC scripts) |
| `index.html` | Vite HTML shell | Loads React app |
| `vite.config.ts` | Vite config | Dev URL for Tauri (`localhost:1420`), React plugin |
| `tsconfig.json` / `tsconfig.node.json` | TS project config | Typecheck frontend |
| `README.md` | Engineer onboarding | Run shell, ship gates, LLM setup |
| `CHANGELOG.md` | Release notes | Version history |
| `CaseForge_PRD_v1.0.md` | **Layer 1** product requirements | Normative AC-x.y, non-goals, construction rules |
| `CaseForge_Dev_Batches_v1.0.md` | Ordered batch plan | B-S0 / B-S1 / B-S2 work sequence |
| `.gitignore` | Ignore rules | Keep secrets/build artifacts out of VCS |
| `dist/` | Production frontend build | Tauri `frontendDist` packaging |
| `scripts/reset-vault.bat` | Local vault wipe helper | Recover from forgotten password during testing |

### 7.2 Frontend — `src/`

| Path | What it is | Why it is used |
|------|------------|----------------|
| `src/main.tsx` | React bootstrap | Mounts `<App />` into the WebView |
| `src/App.tsx` | **Main application shell** | Navigation, vault unlock, case wizard, document pipeline UI, wiring of all feature screens |
| `src/styles.css` | Global visual system | Offline-product chrome (unlock screen, cards, stamps) |
| `src/vite-env.d.ts` | Vite type shims | `?raw` imports, import.meta |

#### `src/components/` — feature screens

| File | Feature | Role in product |
|------|---------|-----------------|
| `ArgumentBuilderView.tsx` | Argument Builder | Local LLM-assisted argument map from case file context |
| `ArgumentTreeView.tsx` | Argument Tree | Deterministic VI + Missing Links presentation |
| `BatchView.tsx` | Overnight batch | Start/monitor/kill/resume long doc jobs |
| `CompanionView.tsx` | Phone capture | LAN-only QR/URL companion for phone scans |
| `DemoArgumentPanel.tsx` | Integrity lab | Bundled demo trees without vault |
| `DigestView.tsx` | Digest | CASEBOOK / CHAMBER / DIGEST + prep completeness |
| `DraftingView.tsx` | Drafting | Offline drafting surfaces from digest graph |
| `ExperienceView.tsx` | Experience Store | Local pattern store + Stage-2 categories |
| `GuardrailsView.tsx` | Guardrails | Policy status / red-team related UI |
| `InterviewView.tsx` | Guided interview | Gaps → counsel-sourced fills |
| `LicenceView.tsx` | Licence | Activate / status / air-gap messaging |
| `MockCourtView.tsx` | Mock Court | Text formats + session UX |
| `PacksView.tsx` | Knowledge packs | Install / list signed packs |
| `ProtectionView.tsx` | Protection | Packs/models seal status (never ransoms vault) |
| `RetrievalView.tsx` | Retrieval | Query local law corpus; GREEN/AMBER/RED |
| `ReviewView.tsx` | Review | Side-by-side counsel corrections |
| `VoiceView.tsx` | Voice & lexicon | STT staging, TTS roles, lexicon CRUD/export |

#### `src/lib/` — pure client helpers + IPC

| File | Role |
|------|------|
| `ipc.ts` | **Typed Tauri invoke API** — vault, cases, docs, docpipe, court, packs, licence, etc. |
| `caseWizard.ts` | Wizard field helpers / validation helpers |
| `completeness.ts` | Digest / prep completeness presentation helpers |
| `demoArgumentIntegrity.ts` | Types + loaders for demo argument trees |
| `demoSeeds.ts` | One-click demo case seeds (wizard + packet text) |
| `exportDraft.ts` | Export drafted text from UI |
| `mockCourtCoach.ts` | Client-side coaching helpers for court UX |
| `review.ts` | Review correction helpers |

### 7.3 Rust backend — `src-tauri/`

| Path | Role |
|------|------|
| `src-tauri/tauri.conf.json` | App identity, window, dev URL, icons, bundle |
| `src-tauri/Cargo.toml` / `Cargo.lock` | Rust deps + SQLCipher feature set |
| `src-tauri/build.rs` | Tauri build hooks |
| `src-tauri/capabilities/default.json` | Tauri capability allow-list |
| `src-tauri/icons/*` | Desktop icons |
| `src-tauri/msvc-env.ps1` | Windows MSVC/Perl PATH for SQLCipher builds |
| `src-tauri/migrations/*.sql` | App + case DB migrations (see §4) |
| `src-tauri/src/main.rs` | Binary entry |
| `src-tauri/src/lib.rs` | **Command registry** — all `#[tauri::command]` handlers, app setup |

#### Rust modules (`src-tauri/src/`)

| Module | Batch / PRD | Responsibility |
|--------|-------------|----------------|
| `vault/` | B-S1-02+ | Paths, KDF, SQLCipher open/create, cases, docs, graph, jobs, lexicon, experience |
| `vault/kdf.rs` | Q-DB-01 | Argon2id params + sidecar load/save |
| `vault/sql.rs` | — | Encrypted open, migrations apply |
| `vault/wizard.rs` | B-S1-03 | Jurisdiction-first case wizard rules |
| `vault/docs.rs` | B-S1-04 | Document intake / BLOB storage |
| `vault/error.rs` | — | `CF_VAULT_*` error codes |
| `vault/paths.rs` | — | Canonical vault directory layout |
| `docpipe/` | B-S1-05–07 | Orchestrate triage, recognize, skeleton extract |
| `docpipe/triage.rs` | B-S1-05 | Page classification pipeline |
| `docpipe/recognize.rs` | B-S1-06 | Text layer + confidence |
| `docpipe/skeleton.rs` | B-S1-07 | case_graph extraction |
| `retrieval/` | B-S1-13 | Lexical retrieval + citation labels |
| `integrity/` | B-S2-01 | **Deterministic** VI + Missing Links (no LLM) |
| `mock_court/` | B-S2-02 | Five sequential text formats; case_graph + retrieval only |
| `adaptive_bench/` | B-S2-03 | Six generic archetypes + sliders (no named judges) |
| `voice_court/` | B-S2-04 | Voice duplex Mock Court + debrief (Full tier) |
| `bench_prep/` | B-S2-05 | AI plays both counsel — never decision-maker |
| `infer_llm/` | — | Spawn/status bridge to LLM sidecar |
| `infer_speech/` | B-S1-16 | Speech sidecar bridge |
| `companion/` | B-S1-11 | LAN phone capture server + HTML |
| `packs/` | B-S1-12 | `.akpkg` verify/install/registry |
| `licence/` | B-S1-18 | Offline licence verify/bind |
| `protection/` | B-S1-19 | Packs/models seals |
| `choreography/` | B-S1-20 | Model load/unload policy, RSS awareness |
| `guardrails/` | B-S1-21 / S2-06 | G1–G6 filters + red-team runners |
| `installer/` | B-S1-22 | Hardware gate (64-bit, ≥4 GB; Lite vs Full) |

### 7.4 Sidecars — `sidecars/`

| Path | Role |
|------|------|
| `sidecars/docpipe/docpipe.py` | JSON-RPC triage/recognize worker |
| `sidecars/docpipe/*.py` | Classifier / helpers |
| `sidecars/docpipe/README.md` | Operator docs |
| `sidecars/infer_llm/infer_llm.py` | Local LLM JSON-RPC worker |
| `sidecars/infer_llm/README.md` | Ollama / backends |
| `sidecars/infer_speech/infer_speech.py` | STT/TTS JSON-RPC worker |
| `sidecars/infer_speech/README.md` | Language pin / whisper policy |

### 7.5 Contracts — `contracts/` (Layer 2 truth)

| Path | Purpose |
|------|---------|
| `MANIFEST.json` | Inventory of contracts + freeze status |
| `case_graph.schema.json` | Canonical matter graph |
| `argument_node.schema.json` | Argument tree nodes + VI semantics |
| `citation_record.schema.json` | GREEN/AMBER/RED citations |
| `session_turn.schema.json` | Mock Court turn provenance |
| `case_wizard.schema.json` | New-case wizard fields |
| `document_intake.schema.json` | Intake metadata |
| `page_triage.schema.json` | Per-page triage record |
| `page_text_layer.schema.json` | Recognition output |
| `lexicon_entry.schema.json` | Voice lexicon entry |
| `experience_record.schema.json` | Experience Store record |
| `licence_payload.schema.json` | Offline licence blob |
| `akpkg.schema.json` | Knowledge pack package shape |
| `constants.json` | Hardware tiers, budgets, thresholds |
| `errors.json` | Product error code catalogue |
| `guardrails.json` | G1–G6 policy |
| `protection.json` | Protection layer policy |
| `adaptive_bench.json` | Bench archetypes contract |
| `licence_trust.json` / `pack_trust.json` | Trust roots / keys policy |
| `voice_preseed_lexicon.json` | Default lexicon seed |
| `vault_sql.schema.md` | Human SQL layout notes |
| `ipc/*.schema.json` | Shell / docpipe / infer IPC envelopes |
| `QUESTIONS.md` | Open product questions (HALT list) |
| `README.md` | Contracts layer guide |

### 7.6 Golden datasets — `golden/` (Layer 4)

| Path | Purpose |
|------|---------|
| `POLICY.md` / `README.md` / `MANIFEST.json` | GOLDEN authorship rules |
| `G-DOCS/` | Page-level triage/recognition fixtures (ship AC-6.1) |
| `G-CASE-IN/` / `G-CASE-US/` | India / US case skeletons |
| `G-RETRIEVAL-IN/` / `G-RETRIEVAL-US/` | Retrieval known-answers |
| `G-SCRIPTS/` | Mock Court script fixtures |
| `G-AUDIO/` | Voice evaluation fixtures |
| `G-INTEGRITY/` | Integrity scorer fixtures |
| `G-REDTEAM/` | Guardrail red-team prompts |

**Rule:** Demo packs are **not** GOLDEN and must not close ship gates.

### 7.7 Evaluation harness — `eval/`

| Path | Purpose |
|------|---------|
| `run_eval.py` / `run_eval.ps1` | Master eval runner |
| `checks/*.py` | One script family per AC / batch gate |
| `tools/*` | Scaffolders, licence maker, golden generators |
| `bakeoff/` | Vision/triage candidate bake-off harness |
| `reports/` | Last / ship-gate JSON reports |
| `waivers/` | Explicit ship waivers (if any) |
| `README.md` | How to run AC checks |

### 7.8 Demo packs — `demo/`

| Path | Purpose |
|------|---------|
| `cases/demo_01_easy_dishonour/` | Easy India NI / cheque teaching matter |
| `cases/demo_02_medium_wrongful_term/` | Medium US employment matter |
| `cases/demo_03_hard_partition/` | Hard India partition matter |
| Each case: `meta.json`, `case_packet.txt`, `known_answer.json`, `argument_tree.json`, `narrative.md` | Intake text + expected teaching graph + VI pedagogy |
| `tools/score_argument_tree.py` | Offline VI scorer (`npm run demo:score`) |
| `README.md` | Demo rules (not law, not GOLDEN) |

### 7.9 Documentation — `docs/`

| Path | Purpose |
|------|---------|
| `B-S1-*.md` / `B-S2-*.md` | Per-batch engineering notes + exit criteria |
| `RELEASE_CHECKLIST_v1.0.md` / `v2.0.md` | Ship checklists |
| `OPTION_A_runbook.md` | Strict “no invent” development mode |
| `MVP80_LABELING_RUNBOOK.md` | Human labeling ops for G-DOCS |
| `worksheets/*` | Product-owner freeze worksheets |
| **`CASEFORGE_CTO_TECHNICAL_MANUAL.md`** | **This document** |

### 7.10 Packs fixtures — `packs/fixtures/`

Test keys / sample pack material for pack verification gates (not production corpus).

---

## 8. End-to-end product workflows

### 8.1 First launch (vault)

```text
Start app (npm run tauri:dev)
        │
        ▼
 Hardware gate (≥4 GB RAM, 64-bit)? ──no──► Refuse screen
        │ yes
        ▼
 Vault exists?
   │              │
  no             yes
   │              │
   ▼              ▼
 Create password  Sign in with password
 + confirm        │
   │              ├─ wrong → CF_VAULT_PASSPHRASE_INVALID
   │              └─ forgot → Reset local vault (wipe) → Create password
   ▼
 Vault unlocked → Home
```

### 8.2 Create a case and build the file

```text
Cases → New case
  1. Jurisdiction (India | United States | …)  [immutable after create]
  2. Cause title, court, matter type, side, code era, language
  3. Create encrypted case.db
        │
        ▼
 Case file workspace tabs:
  overview → documents → triage → text → skeleton
         → review → interview → batch → capture
```

| Tab | What you do | What you should see |
|-----|-------------|---------------------|
| **Documents** | Upload PDF/DOCX/images/TXT | List of ingested docs; originals stored encrypted |
| **Triage** | Run page triage | Per-page class + confidence |
| **Text** | Run recognition | Full text + word confidences / low-confidence flags |
| **Skeleton** | Build case_graph | Structured parties, facts, issues, timeline skeleton |
| **Review** | Correct OCR/graph | Side-by-side counsel corrections persisted |
| **Interview** | Answer gaps | Gaps closed as counsel-sourced |
| **Batch** | Overnight job | Progress, checkpoint, kill/resume |
| **Capture** | Phone companion | LAN URL/QR; images flow into intake |

### 8.3 Digest → retrieval → drafting

```text
case_graph ready
      │
      ├─► Digest  → CASEBOOK / CHAMBER / DIGEST packs + prep score
      ├─► Knowledge packs installed?
      │         │
      │         ▼
      │    Retrieval → query → GREEN / AMBER / RED citations
      └─► Drafting → offline draft artifacts from graph + citations
```

### 8.4 Argument integrity (Stage 2 core)

```text
case_graph (only — AC-2.1)
      │
      ▼
 Argument Tree rebuild / score
      │
      ├─ Limb scores: Fact, Evidence, Law, Exposure (0–100)
      ├─ VI = 100 − min(Fact, Evidence, Law, Exposure)
      └─ Missing Links: edges below threshold (default 60)
```

**Higher VI = more vulnerable.** Scoring is **deterministic** — not an LLM opinion.

### 8.5 Mock Court practice

```text
Prerequisite: case_graph + retrieval context; Full tier for live voice
      │
      ▼
 Select format (sequential product order):
   1. Bail_Interim_Motion
   2. Final_Arguments
   3. Witness_Cross_Examination
   4. Appellate
   5. Motion_Generic
      │
      ▼
 Pipeline stages: DECOMPOSE → RETRIEVE → GENERATE → VERIFY → SCORE → RENDER
      │
      ▼
 Session turns with provenance (session_turn schema)
 + disclaimer: preparation only / no outcome prediction
```

Related modes:

| Mode | Behaviour |
|------|-----------|
| **Adaptive Bench** | Six generic archetypes + sliders — **no named judges** |
| **Bench Prep** | AI plays **both counsel** only — never the decision-maker |
| **Voice Court** | Full duplex English; Lite **refuses with honesty notice** |

### 8.6 Demo-seeded path (fastest manual dogfood)

1. Unlock vault.  
2. From Home / Cases, **load a demo pack** (easy / medium / hard) — creates case + ingests synthetic packet.  
3. Run triage → text → skeleton as available.  
4. Open **Argument Tree** / **Integrity lab** and compare with `demo/cases/*/argument_tree.json` pedagogy.  
5. Optionally open **Mock Court** if LLM is ready (Full tier).

---

## 9. Manual testing playbook (QA / CTO)

### 9.0 Preconditions

| Check | Command / action |
|-------|------------------|
| Node ≥ 18 | `node -v` |
| Rust + MSVC (Windows) | `. .\src-tauri\msvc-env.ps1` then `cargo -V` |
| App start | `npm run tauri:dev` |
| Optional LLM | `ollama pull qwen2.5:7b` then `npm run llm:status` |
| Vault | Create password **or** unlock; use Reset if stuck |

### 9.1 Test matrix — smoke (30–45 min)

| # | Scenario | Steps | Pass criteria |
|---|----------|-------|---------------|
| S1 | Vault create | Fresh vault password + confirm | Home loads; System shows vault unlocked |
| S2 | Vault wrong password | Relock / restart; enter wrong password | Clear rejection; still locked |
| S3 | Vault reset | Forgot password → reset (or bat) | Create-password flow returns |
| S4 | New case India | Wizard with jurisdiction India | Case appears; jurisdiction immutable |
| S5 | New case US | Second matter United States | Isolated case DB; both list |
| S6 | Document upload | Upload TXT/PDF to case | Document row; byte size; encrypted store |
| S7 | Triage | Run triage | Classes + confidences present |
| S8 | Recognize | Run text layer | Text preview; word confidences |
| S9 | Skeleton | Build graph | case_graph JSON non-empty; completeness score |
| S10 | Review | Correct one item | Correction persisted after re-open |
| S11 | Digest | Open Digest | CASEBOOK/CHAMBER/DIGEST sections + score |
| S12 | Integrity | Build/score Argument Tree | VI computed; Missing Links list |
| S13 | Retrieval | Query with pack/corpus available | Results labeled GREEN/AMBER/RED |
| S14 | Mock Court text | Run one turn (Full + LLM) | Turn ≤ ~15s budget intent; disclaimer present |
| S15 | Guardrails | Prompt “predict the verdict” | Refuse with G1/G6 style message |
| S16 | Lite honesty | On <8 GB if measurable | Voice court refused with honesty notice |
| S17 | Demo pack | Load easy demo | Packet ingested; integrity panel usable |
| S18 | Lock vault | Lock from UI | Unlock screen returns |

### 9.2 Deep manual path — “one real matter”

1. **Create case** with realistic caption (or demo_02 medium).  
2. **Ingest** 3–10 pages (mixed TXT + one image if available).  
3. Run **triage → recognize → skeleton**.  
4. In **Review**, fix 2 OCR mistakes; confirm they stick.  
5. Complete **Interview** for top 3 gaps.  
6. Open **Digest**; screenshot prep score before/after interview.  
7. Install / confirm **knowledge pack** if testing citations.  
8. Run **Retrieval** for a known issue phrase; note citation colours.  
9. Open **Argument Tree**; record weakest limb VI and Missing Links.  
10. Run **Argument Builder** with LLM on; compare narrative to tree (LLM is assistive; tree is source of truth for integrity).  
11. Run **Mock Court** format 1; save/export turns if UI offers; verify no verdict language.  
12. **Lock vault**, restart app, unlock, re-open case — data intact.

### 9.3 Where to look for “results”

| Result type | In-app location | On disk / report |
|-------------|-----------------|------------------|
| Vault status | Unlock screen / System | `%APPDATA%\app.caseforge.desktop\vaults\` |
| Documents | Case file → Documents | `case.db` documents table (encrypted) |
| Triage | Case file → Triage | case DB triage tables |
| Text layer | Case file → Text | case DB text layer |
| case_graph | Skeleton / Digest | `case_graph.graph_json` |
| VI / Missing Links | Argument Tree | `argument_tree` table + UI |
| Citations | Retrieval / Digest | citation records with labels |
| Court turns | Mock Court | session turns (provenance fields) |
| Automated AC | N/A | `eval/reports/*.json` |
| Demo scores | Integrity lab / CLI | `demo/reports/last_demo_score.json` |

### 9.4 Failure modes you should *expect* (honesty)

| Symptom | Likely meaning | Action |
|---------|----------------|--------|
| `CF_VAULT_PASSPHRASE_INVALID` | Wrong password for existing vault | Correct password or Reset local vault |
| LLM not ready | Ollama down / model not pulled | `ollama serve` + `ollama pull qwen2.5:7b` |
| Voice court refused | Lite tier or speech backend | Expect honesty notice; use text court |
| Empty retrieval | No packs / empty corpus | Install pack; use demo law stubs carefully |
| Guardrail refuse | Prompt hits G1–G3/G6 | Pass if refuse is clear and non-leaky |
| Slow first start | SQLCipher + first model load | Cold start budget 6s shell; models extra |

---

## 10. Automated gates & demos

### 10.1 Stage ship gates

```powershell
npm run ship:stage1   # → eval/reports/stage1_ship_gate.json
npm run ship:stage2   # → eval/reports/stage2_ship_gate.json
```

### 10.2 Feature AC scripts (selected)

| npm script | Area |
|------------|------|
| `test:shell` | IPC ping + AC wiring |
| `test:vault` | AC-16.2 vault crypto structural |
| `test:wizard` | Case wizard |
| `test:docs` | Document intake |
| `test:triage` / `test:recognize` / `test:skeleton` | Docpipe stages |
| `test:review` / `test:interview` / `test:batch` | Counsel loops + batch |
| `test:companion` | Phone capture |
| `test:packs` / `test:retrieval` | Packs + citations |
| `test:digest` / `test:drafting` | Digest + drafts |
| `test:voice` / `test:experience` | Lexicon + experience |
| `test:licence` / `test:protection` / `test:choreography` / `test:guardrails` | Platform |
| `test:integrity` / `test:mockcourt` / `test:bench` / `test:voicecourt` / `test:benchprep` | Stage 2 |
| `test:s2redteam` | Stage 2 experience red-team |
| `demo:score` | Synthetic argument trees |

### 10.3 Demo CLI scoring

```powershell
npm run demo:score
# Formula: VI = 100 − min(Fact, Evidence, Law, Exposure)
```

| Demo | Difficulty | Teaching VI (weakest) |
|------|------------|------------------------|
| `demo_01_easy_dishonour` | Easy (IN) | ~15 (strong) |
| `demo_02_medium_wrongful_term` | Medium (US) | ~62 |
| `demo_03_hard_partition` | Hard (IN) | ~78 |

---

## 11. Hardware tiers & performance budgets

| Tier | RAM | Experience |
|------|-----|------------|
| **Not supported** | < 4 GB or 32-bit | Installer/run **refuses** |
| **Lite** | 4–6 GB (band) | Digest, drafting, citations, lexicon dictation; **no live voice simulation** |
| **Full** | ≥ 8 GB | Full Digest + Mock Court (text + Adaptive Bench + Bench Prep + voice duplex) |

**Key budgets (from `contracts/constants.json`):**

| Budget | Value |
|--------|-------|
| Cold start | ≤ 6 s (models unloaded) |
| Idle RAM | ≤ 400 MB (models unloaded) |
| UI freeze | ≤ 200 ms on UI thread |
| Overnight batch | ≤ 300 pages / ≤ 8 h |
| Mock Court turn | ≤ 15 s (~300 token adversary) |
| Foreground digest 40 pages | ≤ 5 min |
| VI recompute | ≤ 5 s |
| Total RSS (worst scripted) | ≤ 7.2 GB |

---

## 12. Known limitations & non-goals

**Non-goals (PRD A20 / product law):**

- No cloud case management  
- No CLM / enterprise DMS  
- No generalist chatbot  
- No verdict / outcome prediction  
- No marketplace  
- No named-judge profiling  
- No automatic judicial decision-making  

**Operational limitations for testers:**

- Vault password is **unrecoverable** without wipe.  
- Stage-1 triage classifier is **heuristic freeze** (`caseforge.triage.heuristic.v1`) until vision bake-off + real-scan GOLDEN re-open.  
- Speech STT may use provisional/stub paths depending on local model install.  
- Demo packs are **synthetic teaching material**, not legal advice and not ship-gate GOLDEN.  
- Browser-only `npm run dev` is **UI preview** — not the full encrypted desktop product.

---

## 13. Quick command reference

### Start the full desktop app (Windows)

```powershell
cd "c:\Users\puroh\Downloads\Case Forge- Legal AI app"
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path
. .\src-tauri\msvc-env.ps1
npm run tauri:dev
```

### Reset local vault (forgot password)

```powershell
# UI: Forgot password? Reset local vault…
# or:
scripts\reset-vault.bat
# or:
rmdir /s /q "%APPDATA%\app.caseforge.desktop\vaults"
```

### Local LLM

```powershell
ollama pull qwen2.5:7b
npm run llm:status
```

### Score demos

```powershell
npm run demo:score
```

### Ship gates

```powershell
npm run ship:stage1
npm run ship:stage2
```

---

## Appendix A — Navigation map (in-app)

| Nav item | Section | Primary capability |
|----------|---------|-------------------|
| Home | Workspace | Status, demos, entry |
| Cases / New case | Workspace | List + jurisdiction-first wizard |
| Case file | Workspace | Documents → capture pipeline |
| Argument Builder | Workspace | LLM-assisted map |
| Argument Tree | Workspace | Deterministic integrity |
| Digest | Workspace | Completeness + packs views |
| Voice | Workspace | Lexicon / STT / TTS |
| Experience | Workspace | Local pattern store |
| Mock Court | Practice | Text/voice court formats |
| Integrity lab | Practice | Bundled demo VI lab |
| Knowledge packs | Library | `.akpkg` install |
| Licence | App | Offline activation |
| Protection | App | Model/pack seals |
| Guardrails | App | G1–G6 policy surface |
| System | App | Sidecars, LLM brain, vault stamp |

---

## Appendix B — Layered engineering model

| Layer | Location | Rule |
|-------|----------|------|
| L1 Product | `CaseForge_PRD_v1.0.md` | What to build; AC definitions |
| L2 Contracts | `contracts/` | Frozen schemas & constants |
| L3 Implementation | `src/`, `src-tauri/`, `sidecars/` | Code only after contracts allow |
| L4 Proof | `golden/`, `eval/` | Known answers + automated gates |
| Batches | `CaseForge_Dev_Batches_v1.0.md` + `docs/B-*.md` | Ordered delivery & exit criteria |

**Prime rule:** If truth is missing from PRD/contracts/golden/batch — **HALT** and raise a `QUESTION`. Never invent law or ship metrics.

---

## Appendix C — Related deep dives

| Topic | Doc |
|-------|-----|
| Vault crypto | `docs/B-S1-02_vault.md` |
| Docpipe triage | `docs/B-S1-05_docpipe_triage.md` |
| Integrity engine | `docs/B-S2-01_integrity.md` |
| Mock Court | `docs/B-S2-02_mock_court.md` |
| Stage 2 ship | `docs/B-S2-07_ship_gate.md` / `docs/RELEASE_CHECKLIST_v2.0.md` |
| Demo packs | `demo/README.md` |
| Option A process | `docs/OPTION_A_runbook.md` |

---

*CaseForge — Offline Legal Intelligence for Litigators. Preparation only. Nothing leaves the machine.*
