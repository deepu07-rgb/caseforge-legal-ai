# B-S1-19 — Model/pack encryption at rest + integrity

**Status:** DONE (2026-08-08)

## Goal

Economic protection for **knowledge packs** and **model weights** at rest: AES-256-GCM, machine-bound keys, runtime integrity, graceful degrade on tamper. **User case data is never ransomed.** Effort budget ≤ **8%**.

## Freezes

| Item | Value |
|------|--------|
| Contract | `contracts/protection.json` |
| Pipeline | `caseforge.protection.v1` |
| Cipher | AES-256-GCM |
| Key | SHA-256(fingerprint components + salt) — machine-bound |
| Envelope | `*.cfenc` under `content.cfenc/` |
| Manifest | `protection_manifest.json` |
| Tamper | Graceful degrade — pack unusable; vault untouched |

## What is protected

- Official pack `content/` (encrypted on install)
- Models directory (on demand / protect models)

## What is NOT protected by this layer

- User case DBs / documents (SQLCipher + passphrase)
- App vault user records

## AC gates

| AC | Meaning |
|----|---------|
| **AC-14.1** | Copied-folder: packs/models fail on foreign machine; user vault still opens with passphrase |
| **AC-14.2** | No ransom: protection never locks/deletes user files; no online kill-switch |

Foreign machine sim: set `CASEFORGE_PROTECTION_FOREIGN=1`.

## Architecture

```
official pack install
  → copy to packs/official/{id}
  → protect_pack_content (AES-GCM, remove plaintext)
  → protection_manifest.json

retrieval load
  → verify integrity
  → decrypt to memory OR degrade (skip pack)
```

| Layer | Path |
|-------|------|
| Module | `src-tauri/src/protection/mod.rs` |
| Hook | pack install + retrieval load |
| UI | **Protection** nav |

## Eval

```powershell
npm run test:protection
```

## Out of scope

- Unbreakable DRM claims
- Online kill-switch for user files
- Encrypting user case vaults with protection key (forbidden)
