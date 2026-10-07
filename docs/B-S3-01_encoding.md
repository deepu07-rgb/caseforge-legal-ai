# B-S3-01 — UTF-8 and content-integrity baseline

**Status:** PASS (2026-08-13)  
**Scope:** Stage 3 batch 01 only

## Policy

- Production text files must decode as strict UTF-8.
- Common mojibake signatures and U+FFFD fail the gate.
- Intentional adversarial/documentation examples require an exact substring allowlist entry and reason.
- DOCX retains exact UTF-8 in OOXML.
- The minimal PDF writer keeps streams ASCII-safe and stores exact Unicode in PDF `/ActualText`; WinAnsi-visible fallbacks are used only where Helvetica lacks a glyph.

## Size budgets

| Surface | Preferred/warning | Hard failure |
|---|---:|---:|
| Development workspace | warning above 8 GiB | above 9 GiB |
| Client bundle/payload | target below 4 GiB | above 9 GiB |

`src-tauri/target`, `node_modules`, and `dist` are reported as generated cache/build usage. They are never part of the intended client package unless files are explicitly bundled. When no final installer exists, the client number is labelled as a pre-bundle estimate; a future real installer replaces that estimate automatically.

Large local Ollama models are external runtime dependencies in the current architecture, but the size gate includes the configured installed model in both the development total and estimated installed-client total. A later packaging batch must replace the pre-bundle app estimate with the real signed installer measurement.

The current `qwen2.5:7b` model is approximately 4.7 decimal GB (4.361 GiB), so the complete installed footprint is under the 9 GiB hard ceiling but cannot meet the preferred sub-4 GiB target with this model. B-S3-13 must evaluate a smaller/strongly quantized supported model if the sub-4 GiB delivery target remains mandatory; the size gate will prevent any configuration above 9 GiB from passing.

## Commands

```powershell
npm run test:s3-encoding
npm run test:size
```

## Verification evidence

- UTF-8/mojibake scan: PASS, 160 active production/contract/documentation files.
- Unicode plain-text/DOCX/PDF round trip: PASS.
- Production frontend build: PASS.
- Existing Stage 2 ship gate: PASS (7 checks, zero skip/fail).
- Repository workspace after safe Rust-cache cleanup: 0.088 GiB.
- Generated cache/build footprint after cleanup: 0.077 GiB.
- Configured external `qwen2.5:7b` plus detected Ollama runtime included by the gate.
- Development total: 4.499 GiB, below the 9 GiB hard limit.
- Estimated installed-client total: 4.412 GiB, below the 9 GiB hard limit but above the preferred 4 GiB target.
- Final signed installer: not built in this batch; the size gate clearly labels the client app number as a pre-bundle estimate.

## Cleanup

Rust `target/` is regenerable and was the dominant development footprint at the start of this batch. It may be removed after verification to remain safely below the development ceiling; source and user data must never be removed by the cleanup.
