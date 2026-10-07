# B-S1-12 — Knowledge pack system

**Status:** DONE (2026-08-07)

## Goal

Install **official Ed25519-signed** `.akpkg` packs and **user custom** packs. Jurisdiction switches the KB only; product features stay identical (PRD §7).

## Pack layout (directory `.akpkg`)

```text
manifest.json          # contracts/akpkg.schema.json
signature.ed25519      # base64 Ed25519 over content_sha256 (official only)
content/**             # raw judgment text + neutral citation (official)
```

## Rules

| Kind | Signature | Headnotes (AC-7.1) | Install root |
|------|-----------|--------------------|--------------|
| official | Required | **Must be zero matches** | `packs/official/` |
| custom | Not required | Allowed | `packs/custom/` (isolated) |

## Host IPC

- `pack_policy` · `pack_list` · `pack_install` · `pack_uninstall` · `pack_headnote_scan`

## Build placeholder packs

```powershell
python eval/tools/make_akpkg.py --out packs/fixtures/caseforge.core.india.placeholder `
  --pack-id caseforge.core.india.placeholder --kind official --name "India Core Placeholder" --placeholder-core india

python eval/tools/make_akpkg.py --out packs/fixtures/caseforge.core.us.placeholder `
  --pack-id caseforge.core.us.placeholder --kind official --name "US Core Placeholder" --placeholder-core us
```

Install in app: **Knowledge packs** → path to fixture folder → **Install pack** → **AC-7.1 scan**.

## Eval

```powershell
npm run test:packs
python eval/run_eval.py --ac AC-7.1
```

## Out of scope

- Full corpus authoring  
- Licence gating of pack install (B-S1-18)

## Follow-on

Retrieval + citations: **B-S1-13** (`docs/B-S1-13_retrieval.md`).
