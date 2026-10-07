# docpipe sidecar (B-S1-05 — triage only)

JSON-RPC 2.0 over stdio. Methods:

| Method | Purpose |
|--------|---------|
| `docpipe.ping` | Healthcheck |
| `docpipe.version` | Version + triage classes |
| `docpipe.triage_pages` | Per-page class + confidence |

## Run

```powershell
# Interactive: one JSON-RPC object per line on stdin
python sidecars/docpipe/docpipe.py
```

## Classifier

`classifier_id = caseforge.triage.heuristic.v1` — **Stage-1 frozen** (Q-VISION-01, 2026-08-06). Re-bake when offline VLM + real-scan GOLDEN available.

`recognizer_id = caseforge.recognize.provisional.v1` — **B-S1-06** word-level text layers + confidence. Thresholds only from `contracts/constants.json` (Q-CONST-01 frozen).

```powershell
# Recognize smoke (stdin JSON-RPC)
python -c "import json,sys; print(json.dumps({'jsonrpc':'2.0','id':1,'method':'docpipe.recognize_pages','params':{'text':'The plaintiff filed mar??? notes.','media_type':'text/plain'}}))" | npm run docpipe
# Structural gate
npm run test:recognize
```

Synthetic fixtures may embed:

```text
[CASEFORGE_TRIAGE_CLASS:typed_english]
```

## Out of scope (this batch)

- Full OCR word boxes / recognition (B-S1-06)
- Skeleton extraction
- Concurrent vision + primary LLM
