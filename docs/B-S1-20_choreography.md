# B-S1-20 — Performance & model choreography hardening

**Status:** DONE (2026-08-08)

## Goal

Enforce **Full/Lite** performance budgets and **8 GB model choreography**: at most one large model resident, sequential load, idle unload, visible **status chip**.

## Freezes

| Item | Value |
|------|--------|
| Pipeline | `caseforge.choreography.v1` |
| Max large resident | **1** |
| Primary + vision concurrent | **Forbidden** (hard fail / sequential eviction) |
| Idle unload | **120 s** |
| RSS budget (AC-4.2) | **7.2 GB** |
| Cold start (AC-3.1) | **≤ 6 s** |
| Idle RAM unloaded (AC-3.1) | **≤ 400 MB** |
| UI freeze (AC-3.5) | **≤ 200 ms** |
| Digest 40-page (AC-3.4) | **≤ 5 min** foreground |

## Roles

| Role | Class | Notes |
|------|-------|-------|
| `primary_llm` | Large | infer-llm chat / reasoner |
| `vision` | Large | docpipe recognize path |
| `speech_stt` / `speech_tts` | Small | tracked for chip |
| `embeddings` | Small | reserved |

Acquiring `primary_llm` **evicts** `vision` and vice versa.

## Status chip

Top bar chip `data-testid="model-status-chip"`:

- `MODEL · unloaded`
- `MODEL · primary_llm` / `vision` / small roles
- `MODEL CONFLICT · primary+vision` (must not persist)

## Tier detection

| RAM | Tier | Live simulation |
|-----|------|-----------------|
| &lt; 4 GB | not_supported | no |
| 4–&lt;8 GB | lite | no |
| ≥ 8 GB | full | yes |
| unknown | unknown | yes (do not block) |

## AC evaluation notes

| AC | How measured |
|----|----------------|
| **AC-3.1** | Process uptime diagnostic near launch; idle RSS when large models unloaded |
| **AC-3.4** | Budget enforced in contracts; measure digest path on Full-tier hardware (may SKIP with note) |
| **AC-3.5** | Existing heavy-job probe + rAF frame gap chip |
| **AC-4.2** | Working-set RSS vs 7.2 GB (`choreography_metrics`) |

Hardware-dependent PASS may be **SKIP** with measurement note when RSS API unavailable.

## Architecture

| Layer | Path |
|-------|------|
| Supervisor | `src-tauri/src/choreography/mod.rs` |
| Hooks | `llm_chat` → primary; `docpipe_recognize_*` → vision; `speech_stt` → speech |
| UI | Top-bar chip + System → choreography panel |

## Eval

```powershell
npm run test:choreography
```

## Out of scope

- Stage 2 Mock Court turn latency (AC-3.3)
- Exact primary GGUF selection (already provisional)
