# B-S3-12 — Matter legal working set

Status: **IMPLEMENTED for controlled development testing** (2026-08-16).

## Outcome

Every open case now has an encrypted, versioned legal working set answering: **Which law is CaseForge relying on for this matter?**

The Library screen can generate authority proposals from the case jurisdiction, court, matter type, code era, case-graph statutes/issues/relief, and installed verified packs. Counsel can also add an individual hybrid-retrieval result.

Proposals never become controlling automatically. Counsel must:

1. Confirm one of the surfaced applicable-law regimes.
2. Include the authority.
3. Explicitly confirm it as controlling.

Only an `INCLUDED`, counsel-confirmed authority whose metadata is `APPLICABLE` and `CURRENT` enters the usable downstream authority list.

If counsel confirms a surfaced regime ending in `_REQUIRED` (for example, personal/succession or state land law), downstream selection remains blocked until an authoritative pack for that regime is installed. Confirming that a missing regime applies never permits the app to substitute model memory.

## Counsel controls

Each item supports include, exclude, irrelevant, remove, pin/unpin, and confirm/unconfirm controlling. Removed items remain as auditable tombstones instead of being physically erased. A refresh never overwrites those decisions.

If a pack update marks an authority `SUPERSEDED`, the working set forces the decision to `SUPERSEDED` and clears controlling status. It cannot silently return through a later suggestion.

Stable `cfauth:` identities derive from jurisdiction, title, citation, and section rather than mutable content checksums. Updated source/chunk checksums refresh the same authority record while its counsel decision survives.

## Inspection and traceability

The lawyer-facing item inspector shows exact source text plus:

- authority, source, chunk, and pack IDs;
- title, citation, section, jurisdiction, and authority type;
- trust, temporal, and supersession status;
- source and chunk checksums;
- pack path and official source reference.

Exact text is stored only inside the encrypted per-case SQLCipher vault and remains bound to its lineage. It is not copied into an untraceable summary store.

## Versioning and downstream selection

Every suggestion refresh, counsel decision, and regime confirmation creates a monotonically increasing working-set revision with a full traceable snapshot. The downstream selector accepts `DIGEST`, `DRAFTING`, `ARGUMENT_INTEGRITY`, or `MOCK_COURT` and records:

- output ID and kind;
- working-set version;
- exact usable authority IDs;
- selection timestamp.

Consumer-specific integrations continue in their scheduled batches, particularly B-S3-14 through B-S3-16. They must call this selector rather than constructing unversioned authority arrays.

## Files

- Contract: [legal_working_set.schema.json](../contracts/legal_working_set.schema.json)
- Encrypted migration: [case_015_working_set.sql](../src-tauri/migrations/case_015_working_set.sql)
- Policy and proposal logic: [working_set.rs](../src-tauri/src/working_set.rs)
- Vault persistence: [vault/mod.rs](../src-tauri/src/vault/mod.rs)
- UI: [WorkingLibraryView.tsx](../src/components/WorkingLibraryView.tsx)
- Evidence: [b_s3_12_working_library.json](../eval/reports/b_s3_12_working_library.json)

## Acceptance gates

| Gate | Result |
|---|---|
| AC-S3-12.1 exact text and metadata inspection | PASS |
| AC-S3-12.2 decisions survive restart/update/rerun | PASS |
| AC-S3-12.3 ambiguous regimes require confirmation | PASS |
| AC-S3-12.4 versioned downstream selector/provenance | PASS |
| AC-S3-12.5 removed/superseded cannot silently re-enter | PASS |

## Verification

```powershell
npm run test:s3:12
cargo test --manifest-path src-tauri/Cargo.toml --lib b_s3_12
npm run build
```
