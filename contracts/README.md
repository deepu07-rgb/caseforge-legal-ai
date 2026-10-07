# CaseForge Layer 2 — Contracts

**Status:** Reserved / partially blocked (B-S0-02)  
**Source:** `CaseForge_PRD_v1.0.md` §16, §4.1, §6.2, §8, §9, §11, §13  
**Rule:** No implementer may invent types or fields. If a property is not frozen here, emit `QUESTION` and halt.

---

## What this folder is

Layer 2 freezes:

| Artefact | Path | Role |
|----------|------|------|
| Case Graph | [`case_graph.schema.json`](./case_graph.schema.json) | Stage 1 → Stage 2 hand-off (`case_graph.json` instances) |
| Argument node | [`argument_node.schema.json`](./argument_node.schema.json) | Argument Tree limbs / anchors / links |
| Citation record | [`citation_record.schema.json`](./citation_record.schema.json) | GREEN / AMBER / RED verification |
| Session turn | [`session_turn.schema.json`](./session_turn.schema.json) | Generative / Mock Court turns + provenance |
| Lexicon entry | [`lexicon_entry.schema.json`](./lexicon_entry.schema.json) | STT corrections + TTS overrides |
| Experience record | [`experience_record.schema.json`](./experience_record.schema.json) | Local Experience Store items |
| Licence payload | [`licence_payload.schema.json`](./licence_payload.schema.json) | Ed25519-signed offline licence |
| Constants | [`constants.json`](./constants.json) | Budgets, VI, AC numbers, hardware tiers (**B-S0-03**; confidence thresholds still BLOCKED) |
| Error codes | [`errors.json`](./errors.json) | Stable `CF_*` codes for IPC / vault / licence / … (**B-S0-03**) |
| IPC | [`ipc/`](./ipc/) | Typed JSON-RPC over stdio for sidecars |

**Inventory file:** [`MANIFEST.json`](./MANIFEST.json)  
**Open gaps:** [`QUESTIONS.md`](./QUESTIONS.md)

---

## Schema status legend

| Status | Meaning |
|--------|---------|
| `BLOCKED` | Path reserved; **field inventory incomplete** in PRD Layer 1. Do not implement production serializers against invented fields. |
| `PARTIAL` | Some normative enums / invariants from the PRD are frozen; full property set still open. |
| `FROZEN` | Ready for implementers (none yet as of B-S0-02). |

All schemas currently: **`BLOCKED`** or **`PARTIAL`** until QUESTIONS are resolved by product owner / Layer 2 authoring.

---

## Normative storage notes (PRD §16)

- **DB:** SQLCipher (AES-256 + Argon2id). One app DB + one DB per case.
- **Argon2id parameters:** Not specified in Layer 1 → see QUESTIONS.
- **`.akcase` round-trip:** Named in AC-16.3; format not specified in Layer 1 → see QUESTIONS.

---

## How to use (implementers)

1. Read `MANIFEST.json` for the list of contract files and status.
2. Treat any schema with `x-caseforge.status` of `BLOCKED` as non-implementable for full validation.
3. You **may** rely only on `x-caseforge.normative_invariants` and frozen `enum` values present in the schema.
4. You **must not** add properties “to make progress.” Open a QUESTION instead.
5. Runtime code that *validates* these files waits for B-S0-05 (eval harness) and later S1 batches.

---

## Naming convention

| Kind | Convention |
|------|------------|
| JSON Schema files | `*.schema.json` |
| Runtime instance of Case Graph | `case_graph.json` (conforms to `case_graph.schema.json`) |
| Constants document | `constants.json` (not a JSON Schema; values document) |
| IPC method contracts | `ipc/<sidecar>.schema.json` |

---

*B-S0-02 — contracts path freeze. Field-level freeze incomplete until QUESTIONS resolved.*
