# IPC contracts — Typed JSON-RPC over stdio

**PRD:** §4.1  
**Transport:** JSON-RPC over stdio  
**Sidecars (spawned on demand, killed when idle):**

| Sidecar | Contract file | Role |
|---------|---------------|------|
| `infer-llm` | [`infer-llm.schema.json`](./infer-llm.schema.json) | llama.cpp (or equivalent GGUF); exactly one GGUF at a time |
| `infer-speech` | [`infer-speech.schema.json`](./infer-speech.schema.json) | whisper.cpp + local English TTS |
| `docpipe` | [`docpipe.schema.json`](./docpipe.schema.json) | OCR / vision / HTR + overnight batch queue |

**Envelope:** [`envelope.schema.json`](./envelope.schema.json) — JSON-RPC 2.0 request/response/error shape (protocol named by PRD; not product invention).

**Shell host (Tauri invoke, not stdio):** [`shell.schema.json`](./shell.schema.json) — **PARTIAL** freeze for B-S1-01 (`shell.ping`, `shell.version`, heavy-job probe, sidecar status stubs).

**AC-4.1:** Full contract round-trip test suite must pass (after methods are frozen). Partial shell wiring passes via `eval/checks/shell_ipc_ping.py`.

**Status:** Sidecar method catalogues remain **BLOCKED** (see `../QUESTIONS.md` Q-IPC-01). Do not invent sidecar method names or payloads.
