# B-S4-22 — Editions, pack subscription, seat transfer

PRD v4.0 §10. Acceptance: AC-S4-16.1, AC-S4-16.2; Stage 3 AC-S3-22.8 still holds. Gate: `eval/checks/editions_b_s4_22.py`
(report `eval/reports/b_s4_22_editions.json`). Golden pinned before the engine: `golden/G-EDITIONS/fixtures/editions_matrix.json`
(commit `bd3c1d4`, SHA-256 in the gate).

**What is decided and what is not.** The mechanism is built and tested. The *contents* are not decided: which features each edition
includes and what they cost is owner decision **Q-S4-06**, and the yearly seat-transfer limit is **Q-S4-07** (PROPOSED 2). The
entitlement map is therefore marked `DEVELOPMENT_ONLY` and the app says so on the Licence screen. Changing an edition's contents is a
change to one JSON file, not to code, and the pinned golden does not move because it runs on its own synthetic map.

## 1. The entitlement map — one place

`contracts/edition_entitlements.json` (read by `src-tauri/src/entitlements.rs`, compiled in):

| Part | Says |
|---|---|
| `editions.<EDITION>.features` | the features that edition includes (placeholder lists) |
| `surfaces.<screen>` | the feature a screen needs (`null` = every install) |
| `commands.<command>` | the feature a host command needs — 34 commands today |
| `stage4_features`, `unlicensed_stage4_policy` | what an install with no licence may use (below) |
| `feature_labels` | plain names shown to the user |

**A licence can only narrow its edition.** What a licence entitles is its signed `features[]` intersected with its edition's list: a
feature the licence names but the edition does not include is ignored (and listed on the Licence screen as ignored); a feature the
edition includes but the licence omits is not granted. A JUDICIARY licence hides the advocacy screens whatever else it carries; only
JUDICIARY has `chambers`; the Judiciary invariants (institution-issued, air-gap forced on, no telemetry) are unchanged.

**Enforced in the host, not the screen.** The first statement of each mapped command is `edition::require_feature("<feature>")?`. The
gate scans `lib.rs` and fails if a mapped command lacks it, if its feature differs from the map, or if any unmapped command gates.
The screens hide what the host would refuse; hiding alone is never the control.

**No licence.** `unlicensed_stage4_policy` is `OPEN_DEVELOPMENT`: an install with no verified v2 licence (none, a v1 licence, or one
that does not verify) keeps every feature except Chambers, so development builds and licences issued before Stage 4 keep working.
`LOCKED` refuses every Stage 4 feature until a v2 licence for an edition that includes it is activated. Which applies to a shipped
build is part of Q-S4-06. A feature the map does not list is refused under either policy. Debug builds can preview an edition without
a licence with `CASEFORGE_DEV_EDITION=<ADVOCATE|PRO|CHAMBERS_FIRM|JUDICIARY>` (release builds never read it).

## 2. Pack subscription — separate from the app licence

`pack_subscription_until_unix_ms` is its own date (the licence tool takes `--pack-days`). No value, or zero, means no end date, like
the app licence.

* **While subscribed:** nothing changes.
* **After it ends** (`UPDATES_EXPIRED`): installed packs stay exactly as they are — usable, searchable, cited, exported. Every
  citation's currency stamp keeps its older date and adds *"updates expired: the pack subscription ended on <date>, so newer law may
  not be reflected"*; the Licence screen shows the notice *"Updates expired on <date>. Your installed packs stay available…; Your work
  and your documents are not affected."*
* **What stops:** installing newer **official** content — any official delta, or a full official pack whose currency date is after the
  subscription's last day. An older official pack can be put back (a restore); an official pack that states no date cannot claim to be
  newer and is allowed; the lawyer's own custom packs are never blocked. The refusal says the user keeps their packs and work
  (`CF_PACK_UPDATES_EXPIRED`).
* **Where it is checked:** exactly one place, `pack_install`, before anything is installed. The gate scans the source and fails if the
  check appears anywhere else, in the vault, or in any read, search, audit, export or status command. The two dates are independent:
  an expired app licence makes the vault read-only (v1 rule); an expired subscription does not touch the vault.
  Since 2026-10-11 "read-only" is enforced at one choke point: an expired licence puts every open matter database in SQLite's query-only mode
  (`VaultManager::apply_write_policy`), so every write - document upload, graph, corrections, witness data, translations, bundle import - is refused
  with `CF_VAULT_READ_ONLY`, while reading, listing and exporting keep working; reopening a matter does not lift it and a valid licence does.
  (Before, only case creation, the lexicon and the experience store were guarded; found by running the real app.)

## 3. Seat transfer — offline, with a yearly limit

A licence is bound to one machine by its 3-of-4 fingerprint, and only the issuer's key can bind it to another. So a transfer is a short
paper trail that needs no network and no server:

1. **New computer:** *Save this computer's fingerprint* (Licence screen) → a small JSON file.
2. **Old computer:** *Give up this computer's licence* (confirmation first). The app writes a **release receipt**, signed by the
   install's own key, forgets the licence and remembers that it gave the seat away. Cases, documents and exports are untouched and stay
   readable; this is not an expiry and nothing is ransomed.
3. **Issuer:** `python eval/tools/make_licence.py --transfer-receipt receipt.json --prior-licence old.json --new-fingerprint new.json`
   re-issues the licence for the new computer, with the transfer recorded in the licence's signed `seat_transfers`. The tool refuses
   a receipt that does not verify, belongs to another licence, was not made on the licence's machine, is not the next transfer, was
   already used, or would exceed the yearly limit.
4. **New computer:** activate the re-issued licence.

The limit is `seat_transfers_per_year` in the signed licence (`null` = transfer disabled). Transfers are counted over a rolling 365
days (a transfer exactly a year old no longer counts) by the issuer's tool *and* by the app at activation, so a licence recording more
transfers than its limit in the window — or any, when transfer is disabled — is refused even if an issuer slips. The app refuses to
give the seat away when none are left (before it deactivates, so a user is never stranded), and an old licence file that this machine
gave away is refused (`CF_SEAT_RELEASED`).

`seat_transfers` is a new optional property of `licence_payload_v2.schema.json` — additive: every licence valid before stays valid and
means the same.

**Limits of any offline scheme, stated plainly.** Someone who deletes the vault can re-use an old licence file on the old machine, and
a copied licence file differs from the original only by its fingerprint. The receipt and the record make a transfer *honest*; they do
not make cheating *impossible*, and nothing here pretends they do. Issuer-assisted re-issue is the part that makes a yearly limit
enforceable without a server; whether the owner wants that design or a different one is part of Q-S4-07.

## 4. The licence tool, all editions

`eval/tools/make_licence.py --edition <ADVOCATE|PRO|CHAMBERS_FIRM|JUDICIARY>`: features default to the edition's list in the map and
can only be narrowed (`--features`); `--days` is the app licence, `--pack-days` the pack subscription; `--seat-transfers-per-year`;
`--list-editions` prints the placeholder map; `--transfer-receipt/--prior-licence/--new-fingerprint` is the re-issue above. The
licence signing key stays out of the repository. The key in use is a **development key**; a production signing ceremony (an offline
key, a documented rotation, `contracts/licence_trust.json` carrying only public keys) has not happened and must precede any real issue.

## 5. How it was checked

* Rust (`cargo test --lib`): the golden on the synthetic map (surfaces per edition, narrowing, ignored features, command refusals,
  unlicensed policy, subscription states and dates, install refusals, seat counting, licence validity, released-seat refusal), the
  shipped map's invariants, a real vault giving a seat away (work kept, old file refused, limit held), and an expired licence plus an
  expired subscription together (readable, entitled, writes refused, no transfer).
* Cross-language: the Python tool's licences and receipt are verified by the Rust host from a committed file (and the gate fails if
  the file differs from what the tool makes now), and a receipt written by the Rust host is verified and re-issued by the tool.
* Static scans listed above.

## 6. What the owner needs to decide

* **Q-S4-06** — what each edition includes (the lists in the map), prices, currencies, discounts, and whether an unlicensed install is
  `OPEN_DEVELOPMENT` or `LOCKED` in a shipped build.
* **Q-S4-07** — the yearly transfer limit (PROPOSED 2) and whether issuer-assisted re-issue is the design wanted.
* A production licence-signing key and ceremony.
* Whether a pack that states no currency date may be installed after the subscription ends (today: yes, since it cannot claim to be
  newer, and the rule is "never lock a user out of their own material").

Nothing here is a claim that any edition is approved or priced.
