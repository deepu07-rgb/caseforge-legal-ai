# B-S1-17 — Experience Store (Stage 1 subset)

**Status:** DONE (2026-08-08)

## Goal

Local **encrypted** Experience Store that accumulates Stage 1 patterns and injects **top-K** relevant past patterns into retrieval (PRD §11). Fleet contribution is **off by default** and never network-uploads without explicit review.

## Freezes (Q-EX-01)

| Item | Value |
|------|--------|
| Schema | `experience_record.schema.json` v1.0.0 |
| Pipeline | `caseforge.experience.v1` |
| top_k | **5** |
| relevance_key | lowercase unique tokens from summary + tags + category |
| Storage | Encrypted app vault (SQLCipher) |
| Fleet default | `enabled=false`, `network_upload=false`, records `local_only` |

### Stage 1 writable categories

- `correction` (dual-write from review)
- `lexicon_entry` (dual-write from lexicon upsert)
- `draft_edit`
- `rated_exchange`

### Stage 2 only (write rejected)

- `limb_outcome`
- `bench_setting`

## AC gates

| AC | Implementation |
|----|----------------|
| **AC-11.1** | Records in encrypted app DB; `experience_top_k` ranks by token overlap; retrieval injects hits as `experience_store/*` corpus chunks |
| **AC-11.2** | `fleet_enabled=false` by default; `fleet_network_upload` forced false; review request fails until fleet explicitly enabled — still no network path in Stage 1 |

## Architecture

```
Review correction  ─┐
Lexicon upsert     ─┼─► experience_records (app vault, encrypted)
Manual draft_edit  ─┘
                         │
                         ▼
              experience_top_k(query)  ─► retrieval corpus injection
```

| Layer | Path |
|-------|------|
| Migration | `app_004_experience.sql` |
| Vault API | `VaultManager::experience_*` |
| Retrieval | `add_experience_chunks` in `retrieval/mod.rs` |
| UI | **Experience** nav → `ExperienceView` |

## Eval

```powershell
npm run test:experience
```

## Out of scope

- Fleet contribution backend / network upload
- Limb outcomes & bench settings (Stage 2 / B-S2-06)
- Default-on any upload path (hard fail / AC-11.2)
