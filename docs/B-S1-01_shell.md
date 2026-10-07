# B-S1-01 — Tauri v2 + React 18/TS shell

## Scope completed

- Single-window CaseForge shell (Ink/Parchment UI; no Stage 2 surfaces).
- Host IPC: `shell_ping`, `shell_version`, `shell_heavy_job_*`, `shell_sidecar_status`.
- Three sidecar **stubs** (`infer-llm`, `infer-speech`, `docpipe`) — idle only, no models.
- Dummy heavy job on a Rust background thread (AC-3.5 probe).
- Cold-start diagnostic via `uptime_ms` on `shell_version` (AC-3.1 measurement hook).

## Out of scope (do not expect)

- SQLCipher vault, OCR, LLM, speech, packs, Digest UI, Mock Court.

## Run

### Prerequisites

1. Node.js ≥ 18  
2. Rust stable (`rustup default stable`) — installed under `%USERPROFILE%\.cargo\bin`  
3. Windows: **MSVC linker** via [Build Tools for Visual Studio](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with **Desktop development with C++** / VCTools workload (**Q-ENV-01** — required for `tauri:dev`)  
4. WebView2 (preinstalled on modern Windows 10/11)

```powershell
# Ensure cargo on PATH for this session
$env:Path = "$env:USERPROFILE\.cargo\bin;" + $env:Path

npm install
npm run test:shell        # AC-4.1 partial + AC-3.1/3.5 structural
npm run build             # Vite production frontend
npm run tauri:dev         # full shell (needs link.exe / MSVC)
```

Frontend-only (no Tauri IPC):

```powershell
npm run dev
```

### Current machine status (B-S1-01 close-out)

| Check | Result |
|-------|--------|
| `npm run build` | PASS |
| `npm run test:shell` | PASS |
| `cargo check` / `tauri:dev` | **BLOCKED** — `link.exe` not found (Q-ENV-01) |

## Contracts

Frozen host methods: `contracts/ipc/shell.schema.json`  
Sidecar JSON-RPC catalogues remain **BLOCKED** (Q-IPC-01).

## AC status after this batch

| AC | Status |
|----|--------|
| AC-4.1 partial | Contract + command wiring test (`eval/checks/shell_ipc_ping.py`) |
| AC-3.1 | Measured in-app via `uptime_ms`; Full-tier cold start still hardware-gated |
| AC-3.5 | Dummy job off UI thread + frame-gap probe in UI |
