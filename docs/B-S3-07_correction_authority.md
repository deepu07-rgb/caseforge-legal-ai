# B-S3-07 — Counsel authority and rerun merge

Counsel corrections are immutable authority events and active overlays in the encrypted per-case
SQLCipher vault. They are no longer only mutations of generated snapshots. Generated graph writes
and text-layer replacements apply authority before persistence, so all downstream consumers and
exports receive the same merged state.

Each overlay uses a `cfat:v1:` target key and an anchor matched by exact identity, source content
hash/bounding box, then unique prior base value. Multiple candidates create `AMBIGUOUS_MATCH`; an
absent candidate creates `TARGET_DELETED`. Neither silently deletes counsel information.

Precedence is explicit counsel deletion, active counsel overlay, then new generation. Graph merge
inputs are prior generation, active overlay and new generation. Missing counsel graph items are
restored from the prior graph and flagged. Word overlays are reapplied before OCR rows replace the
previous snapshot.

New values differing from both base and counsel create `MATERIAL_DISAGREEMENT`. Review shows both
values and requires Keep counsel or Accept generated. SQL triggers reject correction-event updates
and deletes. Revert and resolution append new hashed events; reason, timestamp, actor, anchor and
supersession remain auditable.

The same policy covers direct recognition, recognize-all, skeleton rebuild, document-addition
rebuild, overnight batch/resume and pipeline/model upgrades. Controlled evidence is in
`eval/reports/b_s3_07_authority.json`; the frozen `correction_survival >= 1.0` metric is in
`eval/results/stage3/correction_survival.json`.

This batch adds no model, runtime, index or network dependency.

Verification on 2026-08-14: the B-S3-07 acceptance gate passed with
`correction_survival = 1.0`; all 51 Rust library tests passed; the TypeScript/Vite production
build passed. The development workspace measured 3.257 GiB. Including the shared local Ollama
store (4.361 GiB), the measured development footprint was 7.618 GiB, below the 8-9 GiB ceiling.
