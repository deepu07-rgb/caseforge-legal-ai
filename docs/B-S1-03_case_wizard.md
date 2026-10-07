# B-S1-03 — New Case wizard (jurisdiction-first)

## PRD §5 fields (only)

| Field | Rule |
|-------|------|
| cause title | Required |
| court | Required |
| **jurisdiction** | Required: `India` \| `United States` \| `Other` — **immutable** after create |
| matter type | Required free text (no enum in PRD) |
| user’s side | Required free text |
| code/era | Auto-suggest **stub** by jurisdiction; user may edit |
| language | `English` only (Stage 1 primary) |

## Outputs

- Encrypted empty case vault (`case.db` + profile row)
- App registry row with denormalized wizard fields for listing
- `case_profile` + `vault_meta.jurisdiction` in case vault

## Commands

- `case_create({ passphrase, wizard })`
- `case_wizard_meta`
- `case_suggest_code_era`

## Out of scope

Document upload (B-S1-04), packs, AI suggestions beyond stub.

## Verify

```powershell
python eval/checks/case_wizard_b_s1_03.py
npm run build
```
