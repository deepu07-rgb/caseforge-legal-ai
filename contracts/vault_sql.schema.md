# Vault SQL surface (B-S1-02)

**Not** the Case Graph schema. Minimal tables only so create/open/close work without inventing Stage 1 extraction fields.

## App vault (`app.db`)

| Table | Purpose |
|-------|---------|
| `schema_migrations` | Migration version tracking |
| `vault_meta` | Key/value (`schema_version`, `created_at_unix_ms`) |
| `cases` | Registry of case ids (no document bodies) |

### `cases` columns

| Column | Type | Notes |
|--------|------|-------|
| `case_id` | TEXT PK | UUID |
| `created_at_unix_ms` | INTEGER | |
| `label` | TEXT | Empty until B-S1-03 wizard; not document content |

## Case vault (`case.db`)

| Table | Purpose |
|-------|---------|
| `schema_migrations` | Migration version tracking |
| `vault_meta` | Key/value for this case vault |

Document blobs, case_graph, etc. arrive in later batches.

## On-disk layout

```
{app_data}/CaseForge/vaults/
  app/
    app.db          # SQLCipher AES-256
    kdf.json        # public salt + params_version only
  cases/{case_id}/
    case.db
    kdf.json
```

**AC-16.2:** Opening either `.db` without the correct passphrase-derived key must fail; no case document plaintext beside these files.
