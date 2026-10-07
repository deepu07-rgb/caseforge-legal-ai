# B-S1-18 — Licensing, air-gap, privacy audit

**Status:** DONE (2026-08-08)

## Goal

One-time **offline** activation with machine fingerprint **3-of-4**, Ed25519-signed licence, state machine, **air-gap** mode, and **AC-13.1** case-data network policy.

## Freezes (Q-LIC-01)

| Item | Value |
|------|--------|
| Schema | `licence_payload.schema.json` v1.0.0 |
| Pipeline | `caseforge.licence.v1` |
| States | `UNLICENSED` · `ACTIVE` · `EXPIRED` |
| Fingerprint | `hostname`, `username`, `machine_guid`, `disk_root` (SHA-256 hex); **3-of-4** |
| Signature | Ed25519 over canonical sorted JSON (no signature fields) |
| Trust | `contracts/licence_trust.json` |
| EXPIRED | **read-only vault** — data never ransomed/deleted |
| Air-gap | kills networking (companion refuse, fleet refuse) |
| Case data network | **forbidden** (AC-13.1) |

## AC-13.1

Network-capture / policy audit shows **zero user-document content** leaving the machine:

- No cloud case export path
- Experience fleet upload forced off
- Speech `cloud: false`
- Companion LAN-only (and blocked under air-gap)
- Licence validation is offline crypto only

Eval: structural policy + source wiring (`npm run test:licence`).

## Activation

```powershell
# Bind to this machine and write packs/fixtures/licence_dev.json
python eval/tools/make_licence.py --bind-live -o packs/fixtures/licence_dev.json
```

In app: unlock vault → **Licence** → paste JSON → **Activate offline**.

## EXPIRED behaviour

| Allowed | Forbidden |
|---------|-----------|
| Open vault / open case | Create case, lexicon write, experience write |
| Read graph / export existing | Mutations that require `assert_vault_writable` |
| Mark status EXPIRED | Delete or encrypt-ransom user data |

## Architecture

| Layer | Path |
|-------|------|
| Module | `src-tauri/src/licence/mod.rs` |
| Storage | `app_005_licence.sql` (`licence_state`) |
| UI | **Licence** nav → `LicenceView` |
| Tool | `eval/tools/make_licence.py` |

## Eval

```powershell
npm run test:licence
```

## Out of scope

- Heavy DRM / model encryption (B-S1-19)
- Online activation / kill-switch
- Inventing extra licence states beyond UNLICENSED / ACTIVE / EXPIRED
