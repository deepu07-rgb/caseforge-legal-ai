# B-S1-02 — SQLCipher vault & case DB

## Goal

Encrypted **app** database + **one encrypted DB per case**. No case document plaintext on disk.

## Crypto freeze (Q-DB-01 resolved)

Owner chose **OWASP 2023 interactive** during B-S1-02:

| Param | Value |
|-------|-------|
| Algorithm | Argon2id |
| memory_kib | 65536 (64 MiB) |
| iterations | 3 |
| parallelism | 4 |
| salt_len_bytes | 16 |
| key_len_bytes | 32 |
| params_version | 1 |

Frozen in `contracts/constants.json` → `crypto.argon2id` and mirrored in `src-tauri/src/vault/kdf.rs`.

**Page cipher:** SQLCipher AES-256 with application-provided raw key (Argon2id output).  
`kdf.json` sidecars store **public salt + params only** — never the passphrase.

## Layout

```
{app_data}/CaseForge/vaults/
  app/app.db + kdf.json
  cases/{case_id}/case.db + kdf.json
```

## Commands

| Command | Role |
|---------|------|
| `vault_status` | Paths + unlock state + crypto params |
| `vault_init_unlock` | Create app vault if missing, unlock |
| `vault_unlock` / `vault_lock` | Session lock |
| `case_create` | New case DB (passphrase re-confirmed) |
| `case_open` / `case_close` / `case_list` | Case handles |

## AC-16.2

- Wrong passphrase → `CF_VAULT_PASSPHRASE_INVALID`
- Opening `.db` without key must not yield schema
- Structural check: `npm run test:vault`
- Live unit test: `cargo test -p caseforge --lib vault::tests` (needs MSVC + SQLCipher build)

## Out of scope

Case wizard fields (B-S1-03), documents, case_graph, packs, licensing.
