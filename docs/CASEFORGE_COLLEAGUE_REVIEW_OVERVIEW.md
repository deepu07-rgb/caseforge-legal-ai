# CaseForge — Colleague Review Overview

**Read this first.**  
**Then use:** [`CASEFORGE_CTO_TECHNICAL_MANUAL.md`](./CASEFORGE_CTO_TECHNICAL_MANUAL.md) for technical depth, commands, and test steps.

| Item | Detail |
|------|--------|
| **Product** | CaseForge (formerly Aakhada — name retired) |
| **Version covered** | `2.0.0-rc1` (Digest Engine + Mock Court release candidate) |
| **Who this is for** | Colleagues reviewing the app for the first time — product, design, legal ops, engineering, QA |
| **Goal of this note** | Give a clear, plain-language picture of **what CaseForge is**, **why it exists**, and **how it is structured** |
| **Companion doc** | CTO Technical Manual — architecture, file map, workflows, manual test playbook |

---

## How to use these two documents together

| If you want… | Open… |
|--------------|--------|
| Big picture, purpose, product story | **This overview** |
| Exact tech stack, databases, security numbers, IPC, module list | **CTO Technical Manual** §§1–7 |
| Step-by-step user journey and where results appear | **This overview** §7 + Manual §8–9 |
| How to run the app, reset vault, score demos, ship gates | **Manual** §§9–13 |
| Per-feature engineering notes | `docs/B-S1-*.md` / `docs/B-S2-*.md` |

You do **not** need to understand every technical section to give useful review feedback. Focus on: *Does this help a litigator prepare better? Is it honest about limits? Is the flow clear?*

---

## 1. What is CaseForge? (one-minute pitch)

**CaseForge is an offline desktop app for litigators.**

It helps a lawyer prepare a case by:

1. **Reading and remembering** the case file (documents, facts, parties, dates).
2. **Organising** that material into a structured case picture.
3. **Showing weak points** in the argument chain (facts → evidence → law).
4. **Sparring against the lawyer** in mock courtroom formats — using only the case’s own material and local law packs.

**One-line pitch (product truth):**

> CaseForge reads your case file on your machine, remembers it, shows where the case is weakest, and argues against you until your prep is stronger — client papers never need to leave your computer.

### What it is *not*

| CaseForge is **not**… | Why this matters for review |
|------------------------|-----------------------------|
| A substitute for a lawyer | AI assists preparation; counsel remains responsible |
| A verdict / win predictor | Product law forbids outcome prediction |
| A cloud case-management system | Everything is local by default |
| A general chatbot for any legal question | It is built around **this case’s file** |
| A tool that profiles named judges | Adaptive Bench uses generic archetypes only |

---

## 2. Why this product exists

In many firms, especially small practices and junior teams, building a clean case digest still takes **days of manual work**:

- Reconstruct chronology from messy PDFs and scans  
- Map parties, issues, evidence, and prayers  
- Check whether every claim is backed by a fact and a document  
- Rehearse arguments before a hearing  

CaseForge aims to shorten that preparation loop **without sending client papers to a cloud service**.

**Honest value proposition:**

| Mechanism | Plain meaning |
|-----------|----------------|
| **Perfect memory of this case** | Digest: facts, dates, parties, contradictions, each linked back to sources |
| **Deterministic weak-link detection** | Graph scoring over claim → fact → evidence → law — not “AI vibes” |
| **Verified citations** | Authorities are labeled GREEN / AMBER / RED against local corpus |
| **Relentless sparring partner** | Mock Court attacks using case graph + retrieved law only |

Reviewers should judge the product on **preparation completeness**, never on “will this win the case?”

---

## 3. Two stages of the product

CaseForge ships as a **two-stage product**. Stage 2 depends on Stage 1’s structured output.

```text
  STAGE 1 — The Digest Engine                    STAGE 2 — The Mock Court
  ─────────────────────────────────              ──────────────────────────
  Messy / clean case file                        Structured case_graph only
         │                                                │
         ▼                                                ▼
  Vault → Documents → Triage → Text              Argument Tree (weak links)
         → Skeleton (case graph)                 Mock Court practice formats
         → Review / Interview                    Adaptive Bench, Bench Prep
         → Digest / Draft / Citations            Voice court (Full hardware)
```

### Stage 1 — The Digest Engine

**Purpose:** Turn a case file into a verified, source-linked case picture.

Typical capabilities:

- Encrypted local vault (password on this machine only)  
- Case wizard (jurisdiction first: India, United States, etc.)  
- Document upload (PDF, DOCX, images, text)  
- Page triage (what kind of page is this?)  
- Text recognition (OCR-style text layer with confidence)  
- Case skeleton / **case graph** (parties, facts, issues, timeline)  
- Counsel review and guided interview (fill gaps honestly)  
- Overnight batch for large files  
- Phone capture companion (scan pages on the same local network)  
- Knowledge packs + retrieval with citation labels  
- Digest views and offline drafting  
- Voice lexicon (self-correcting legal terms for dictation)  
- Licence, protection, and safety guardrails  

**Commercial note:** Stage 1 alone is useful. Many firms still rebuild this material by hand.

### Stage 2 — The Mock Court

**Purpose:** Practice and stress-test the argument **after** the case graph exists.

Typical capabilities:

- **Argument Integrity** — Vulnerability Index + Missing Links (deterministic scoring, not LLM guessing)  
- **Text Mock Court** — five sequential rehearsal formats  
- **Adaptive Bench** — generic bench “styles,” not named judges  
- **Bench Prep** — AI plays both counsel sides; never acts as the decision-maker  
- **Voice Court** — spoken practice on Full-tier machines  

**Hard rule:** Stage 2 must **not** re-read raw documents. It only uses the structured case graph (and retrieval context). That keeps the pipeline clean and reviewable.

---

## 4. Core design laws (non-negotiable)

These are product laws, not marketing slogans. Reviewers should flag any UI or behaviour that violates them.

| Law | Plain meaning |
|-----|----------------|
| **Nothing leaves the machine by default** | Offline-first; air-gap ready. No cloud account for case data. |
| **User data is password-encrypted locally** | Vault password → encryption key. Wrong password = no access. Forgot password = wipe and start over (no cloud recovery). |
| **No verdict prediction** | Never promise outcomes, guilt findings, or sentencing forecasts. |
| **No named-judge profiling** | Bench practice uses generic archetypes + sliders only. |
| **No fabrication assistance** | No fake authorities, fake evidence, or perjury coaching. |
| **Preparation only** | Mock Court is rehearsal, not a real hearing. |
| **Truth before code** | Product requirements → contracts → tests → implementation. If something is undefined, the team **halts** rather than invents. |

---

## 5. Who is it for?

| Audience | How they use CaseForge |
|----------|------------------------|
| **Junior lawyers / associates** | Build digests faster; see missing links before senior review |
| **Small firms** | Overnight processing of paper-heavy files without cloud DMS |
| **Senior counsel** | Stress-test arguments; rehearse opposition attacks |
| **Review colleagues (you)** | Validate clarity, honesty, workflow, and product completeness |

**Hardware reality:**

| Tier | Rough machine | What you get |
|------|---------------|--------------|
| **Not supported** | Under 4 GB RAM or 32-bit OS | App refuses to run |
| **Lite** | About 4–6 GB RAM | Digest, drafting, citations, basic voice dictation; **no live voice Mock Court** |
| **Full** | 8 GB+ RAM | Full Digest + Mock Court including voice practice |

On Lite machines, the product should **refuse politely and clearly** (honesty notice) rather than silently fail.

---

## 6. Core structure of the application

Think of CaseForge as **four cooperating layers** inside one desktop installer.

```text
┌─────────────────────────────────────────────────────────────┐
│  WHAT YOU SEE                                               │
│  React screens (cases, documents, digest, mock court, …)    │
└───────────────────────────┬─────────────────────────────────┘
                            │ typed requests (IPC)
┌───────────────────────────▼─────────────────────────────────┐
│  BRAIN / CONTROL ROOM (Rust)                                │
│  Vault crypto, case lifecycle, pipelines, scoring, rules    │
└───────────────┬─────────────────────────────┬───────────────┘
                │                             │
     Encrypted databases              Helper processes (Python)
     (SQLCipher vaults)               docpipe · local LLM · speech
                │                             │
                └────────── local machine only ───────────────┘
```

### 6.1 Layer A — The screens (frontend)

| Location | Role in simple terms |
|----------|----------------------|
| `src/` | Everything the user clicks and reads |
| `src/App.tsx` | Main shell: unlock vault, navigate, open case workspaces |
| `src/components/` | One screen per major feature (Digest, Mock Court, Packs, …) |
| `src/lib/ipc.ts` | Safe “remote control” from UI → Rust (typed commands) |

The UI **does not** permanently store secrets. Long-term secrets and case BLOBs live in the vault.

### 6.2 Layer B — The control room (Rust desktop core)

| Location | Role in simple terms |
|----------|----------------------|
| `src-tauri/` | Desktop shell (Tauri 2) + all serious business logic |
| `vault/` | Create/unlock vaults, store cases and documents encrypted |
| `docpipe/` | Orchestrate triage → recognition → skeleton extraction |
| `integrity/` | Score argument strength **without** calling an LLM |
| `mock_court/` / related | Court practice modes |
| `packs/`, `licence/`, `guardrails/` | Knowledge packs, offline licence, safety filters |

### 6.3 Layer C — Helper processes (sidecars)

Heavy or ML-related work runs in separate local processes:

| Sidecar | Job |
|---------|-----|
| **docpipe** | Page triage and recognition helpers |
| **infer_llm** | Local language model (e.g. via Ollama) for coaching / Mock Court text |
| **infer_speech** | Speech-to-text and text-to-speech (English primary) |

These talk to the Rust core over **JSON-RPC on standard I/O** — still on the same machine, not the internet.

### 6.4 Layer D — Contracts, tests, and demos (truth system)

CaseForge is built so that “what the product must do” is written down **before** code freezes behaviour.

| Folder | Plain meaning |
|--------|----------------|
| `contracts/` | Frozen schemas and rules (shapes of data, error codes, guardrails) |
| `golden/` | Known-answer fixtures for automated checks |
| `eval/` | Scripts that prove acceptance criteria pass |
| `demo/` | Synthetic teaching cases (easy / medium / hard) — **not real law** |
| `CaseForge_PRD_v1.0.md` | Product requirements (Layer 1) |
| `CaseForge_Dev_Batches_v1.0.md` | Ordered build plan |

**For reviewers:** demos are for learning the product. They are pedagogical, not legal advice and not the official “golden” ship dataset.

### 6.5 Where data lives on a Windows PC

| Path idea | What is stored |
|-----------|----------------|
| `%APPDATA%\app.caseforge.desktop\vaults\app\` | App-level vault: case list, lexicon, experience, licence meta |
| `...\vaults\cases\{case_id}\` | Per-case vault: documents, triage, text, case graph, argument tree |
| `...\vaults\packs\` | Installed knowledge packs (when used) |

There is **no user account database in the cloud**. Identity for encryption is simply:

```text
your vault password  →  key derivation  →  encrypted databases open
```

---

## 7. The journey of a case (how the app “works” for a user)

This is the mental model to keep while reviewing.

### Step 1 — Unlock the vault

- First time: create a vault password (and confirm).  
- Later: sign in with that password.  
- Wrong password: access denied.  
- Forgot password: **reset wipes local vaults** — there is no email recovery by design.

### Step 2 — Create a case

Wizard captures:

- **Jurisdiction** (e.g. India or United States) — chosen first; treated as fixed after create  
- Cause title, court, matter type, side, language, and related profile fields  

Each case gets its **own encrypted database**.

### Step 3 — Build the case file

Inside a case workspace, typical tabs / steps:

| Step | User action | Result |
|------|-------------|--------|
| **Documents** | Upload files | Originals stored encrypted |
| **Triage** | Run classification | Each page labeled (typed, handwriting, mixed, non-text, …) |
| **Text** | Run recognition | Searchable text + confidence flags |
| **Skeleton** | Build case graph | Structured parties, facts, issues, timeline |
| **Review** | Fix mistakes | Counsel corrections saved |
| **Interview** | Answer gap questions | Missing pieces filled as counsel-sourced |
| **Batch** | Overnight job | Large files processed with progress / resume |
| **Capture** | Phone companion on LAN | Photos of pages flow into intake |

### Step 4 — Digest, retrieve law, draft

Once a case graph exists:

- **Digest** — casebook / chamber / digest style views + preparation completeness  
- **Retrieval** — query local knowledge packs; citations labeled GREEN / AMBER / RED  
- **Drafting** — offline drafting surfaces grounded in the graph  

### Step 5 — Argument integrity (Stage 2 core)

From the **case graph only**, the app builds an Argument Tree and scores:

| Limb | Question it asks |
|------|------------------|
| **Fact** | Are the factual claims clear and supported? |
| **Evidence** | Is there evidence for those facts? |
| **Law** | Is the legal basis connected? |
| **Exposure** | Where is the other side most likely to attack? |

**Vulnerability Index (VI)** is derived deterministically:

```text
VI = 100 − weakest limb score
```

- **Higher VI = more vulnerable** (more work needed).  
- **Missing Links** = connections that fall below a threshold (default 60).  
- This scoring does **not** use the LLM — so the number is stable and inspectable.

### Step 6 — Mock Court practice

Lawyer chooses a format (product order):

1. Bail / Interim Motion  
2. Final Arguments  
3. Witness Cross-Examination  
4. Appellate  
5. Generic Motion  

The system runs a controlled pipeline (decompose → retrieve → generate → verify → score → render), keeps provenance on turns, and always frames output as **preparation only**.

Related practice modes:

| Mode | What it does |
|------|----------------|
| **Adaptive Bench** | Six generic bench styles + sliders |
| **Bench Prep** | AI plays both counsel; never the judge deciding the case |
| **Voice Court** | Spoken duplex practice (Full tier only) |

### Fast path for reviewers

If you only have 15–30 minutes:

1. Unlock / create vault.  
2. Load a **demo pack** (easy / medium / hard).  
3. Walk documents → skeleton (as available).  
4. Open **Argument Tree** / Integrity lab.  
5. Optionally open **Mock Court** if a local LLM is running and the machine is Full tier.

Details and pass criteria: Technical Manual §8–9.

---

## 8. Major features map (what each area is for)

### Workspace

| Area | Purpose in plain language |
|------|---------------------------|
| **Home** | Entry, status, demo entry points |
| **Cases / New case** | List matters; create with jurisdiction-first wizard |
| **Case file** | Document pipeline and case workspace |
| **Argument Builder** | LLM-assisted argument map (assistive; tree remains integrity source of truth) |
| **Argument Tree** | Deterministic VI + Missing Links |
| **Digest** | Completeness views of the structured case |
| **Voice** | Lexicon and speech-related tools |
| **Experience** | Local pattern store from practice (stays on device) |

### Practice

| Area | Purpose |
|------|---------|
| **Mock Court** | Text / voice courtroom rehearsal |
| **Integrity lab** | Demo argument trees without needing a full real matter |

### Library & platform

| Area | Purpose |
|------|---------|
| **Knowledge packs** | Install signed local law / knowledge packages (`.akpkg`) |
| **Licence** | Offline activation; expired licence becomes **read-only** (data is never ransomed) |
| **Protection** | Seals for packs/models — separate from user vault encryption |
| **Guardrails** | Safety policy surface (no verdicts, no fabrication, etc.) |
| **System** | Sidecar status, local LLM brain chip, vault state |

---

## 9. AI in CaseForge — where it is used, where it is not

Reviewers often ask: “Is everything AI?” **No.**

| Work | Engine | Why it matters |
|------|--------|----------------|
| Vault encryption | Cryptography (Argon2id + SQLCipher) | Security is not “LLM magic” |
| Argument Vulnerability Index | Deterministic graph scoring | Stable, explainable, no hallucination in the score |
| Page triage / recognition | Local docpipe pipeline | Document understanding path |
| Mock Court narrative / coaching text | **Local** LLM only | Never a cloud legal bot by product path |
| Citation labels | Local retrieval against packs | Honesty labels GREEN / AMBER / RED |

**Local LLM notes (for technical reviewers):**

- Default developer setup often uses **Ollama** with a model such as `qwen2.5:7b`.  
- Cloud APIs are **forbidden** on the product path.  
- On smaller machines, models load one at a time (choreography) so RAM stays within budget.

---

## 10. Safety, privacy, and trust (what to check)

### Privacy

- Case files stay in encrypted local vaults.  
- No cloud login for user case data.  
- Phone capture is **LAN companion**, not a public upload portal.

### Licence honesty

| State | Expected behaviour |
|-------|--------------------|
| Unlicensed | Limited / activation path |
| Active | Normal use |
| Expired | **Read-only** — user can still access their data |

### Guardrails (examples)

| ID | Intent |
|----|--------|
| G1 | No guilt determination / verdict prediction / sentencing forecast |
| G2 | Citation honesty — no fake authorities |
| G3 | No fabrication of evidence / perjury coaching |
| G5 | Provenance on session turns |
| G6 | Preparation only — not a real hearing or outcome predictor |

**Review prompt you can try:** ask the product (where applicable) to “predict the verdict.” A **clear refusal** is a pass, not a failure.

---

## 11. Dual-market design (India + United States)

CaseForge is built as a **dual-market** product with global expansion in mind.

| What stays the same | What changes by market |
|---------------------|------------------------|
| Features and workflows | Knowledge packs / local corpus |
| App structure | Jurisdiction selected at case creation |
| Guardrails and offline design | Language of matter materials as configured |

Jurisdiction is **not** a separate product fork. It is a case setting that selects which knowledge base applies.

---

## 12. How the engineering team keeps quality high

You do not need to run every test to review the product, but it helps to know the discipline:

```text
PRD (what to build)
   → Contracts (exact shapes and rules)
      → Golden fixtures + eval scripts (proof)
         → Code in ordered batches
```

| Idea | Meaning |
|------|---------|
| **Acceptance criteria (AC-x.y)** | Each important rule is meant to be executable as a test |
| **Ship gates** | Stage 1 and Stage 2 automated gate scripts (`npm run ship:stage1` / `ship:stage2`) |
| **Option A / HALT** | If truth is missing, stop and ask — do not invent law or metrics |
| **Batch docs** | `docs/B-S1-*` and `docs/B-S2-*` explain each feature slice |

This is why the CTO Technical Manual is long: the product is **contract-driven**, not improvised.

---

## 13. What good review feedback looks like

### Product / UX feedback (high value)

- Is the vault unlock story clear?  
- Is jurisdiction-first case creation understandable?  
- Can a new user find Documents → Triage → Text → Skeleton without a tour?  
- Are Digest and Argument Tree results readable for a non-engineer lawyer?  
- Are refusals (Lite tier, guardrails, missing LLM) **honest and calm**, or confusing?  
- Does Mock Court always feel like **practice**, never like a court outcome?

### Content / legal-ops feedback

- Are citation colours (GREEN / AMBER / RED) explained well enough?  
- Does the Argument Tree language match how litigators talk about weak limbs?  
- Are demo packs labeled clearly as **synthetic / teaching only**?

### Engineering feedback

- Use the CTO Technical Manual for stack, module ownership, and test matrix.  
- Prefer comments tied to a screen or a workflow step over abstract architecture opinions.

### Please do **not** score the product on

- “Did it predict who would win?” (forbidden by design)  
- “Did it profile Judge X?” (forbidden by design)  
- “Did browser-only preview match full desktop crypto?” (browser `npm run dev` is UI preview only)

---

## 14. Known limits (set expectations for this RC)

This is a **release candidate** style build (`2.0.0-rc1`). Honest limits include:

| Topic | Expectation |
|-------|-------------|
| Vault password | Unrecoverable without wipe |
| Triage classifier | Heuristic freeze; not the final vision model forever |
| Speech | May depend on local model install; stubs possible in some paths |
| Demo packs | Teaching material only — not legal advice |
| Lite machines | No live voice Mock Court |
| Browser-only mode | Not the full encrypted product |

Full technical list: Manual §12.

---

## 15. Suggested review agenda (60–90 minutes)

| Time | Activity |
|------|----------|
| 0–10 min | Read **this overview** §§1–5 |
| 10–20 min | Skim Manual §1–2 (executive + architecture diagram) |
| 20–50 min | Run the app: vault → demo case → skeleton → Argument Tree |
| 50–70 min | Digest + Retrieval (if packs available) + one Mock Court text turn (if LLM ready) |
| 70–90 min | Capture notes using §13 feedback prompts; flag anything that feels like outcome prediction or cloud leakage |

**Start commands and deeper QA steps:** Technical Manual §9 and §13.

---

## 16. Glossary (simple definitions)

| Term | Meaning |
|------|---------|
| **Vault** | Encrypted local store for app and case data |
| **Case graph** | Structured JSON picture of the case (parties, facts, issues, links) |
| **Triage** | Classifying each page type before full text work |
| **Recognition** | Turning page images/PDF into text with confidence |
| **Digest** | Human-facing organised case materials and prep completeness |
| **Knowledge pack (`.akpkg`)** | Signed offline package of knowledge / law material |
| **GREEN / AMBER / RED** | Citation trust labels against local corpus |
| **Vulnerability Index (VI)** | How weak the weakest limb of the argument is (higher = weaker) |
| **Missing Links** | Weak or missing connections in the argument graph |
| **Mock Court** | Structured adversarial rehearsal, not a real hearing |
| **Sidecar** | Helper process (docpipe / LLM / speech) on the same machine |
| **IPC** | Message path between UI and Rust core |
| **Lite / Full tier** | Hardware capability bands for which features are allowed |
| **Guardrails** | Hard safety rules the product must enforce |
| **Ship gate** | Automated pack of checks that must pass before release |

---

## 17. Document map (what else lives in `docs/`)

| Document | When to open it |
|----------|-----------------|
| **`CASEFORGE_COLLEAGUE_REVIEW_OVERVIEW.md`** (this file) | First pass for any colleague |
| **`CASEFORGE_CTO_TECHNICAL_MANUAL.md`** | Architecture, file map, manual testing, commands |
| `B-S1-*.md` / `B-S2-*.md` | Deep dive on one feature batch |
| `RELEASE_CHECKLIST_v1.0.md` / `v2.0.md` | Release readiness checklists |
| `OPTION_A_runbook.md` | Strict “do not invent” development mode |
| Root `README.md` | Engineer onboarding and npm scripts |
| Root `CaseForge_PRD_v1.0.md` | Full product requirements |

---

## 18. Closing summary for reviewers

**CaseForge** is an **offline-first legal preparation product** for litigators.

- **Stage 1** builds an encrypted, source-linked case digest and structured case graph.  
- **Stage 2** uses that graph to show argument weaknesses and run mock-court practice.  
- **AI is local and bounded**; integrity scoring is deterministic; client data stays on the machine.  
- The product optimises for **better preparation**, never for **predicted outcomes**.

Share feedback against that purpose. Use this overview for the story and structure; use the **CTO Technical Manual** for precise technical and QA detail.

---

*CaseForge — Offline Legal Intelligence for Litigators.*  
*Preparation only. Nothing leaves the machine.*  
*Companion to: `docs/CASEFORGE_CTO_TECHNICAL_MANUAL.md`*
