# B-S4-09 — Code Bridge (statute succession), with B-S4-03 rule-table core

**Status (2026-10-08): IMPLEMENTED — engine; real mappings BLOCKED.**
- **Checks that pass:**
  - AC-S4-04.2 PASS: 7 of 7 planted missing-date references give NEEDS_COUNSEL_INPUT.
  - AC-S4-04.3 PASS: MODIFIED mappings show both texts.
  - AC-S4-RT.1, RT.2 and RT.3 PASS. These rule-table checks are in B-S4-03's scope; only its core was built here.
  - The annotations validate against the full frozen `contracts/statute_mapping.schema.json`.
- **Check that is blocked:** AC-S4-04.1 is **BLOCKED**. It requires legal-editor-signed succession rows for real codes (for example IPC → BNS, CrPC → BNSS, Evidence Act → BSA), shipped in an official pack together with the text of both codes. None exist yet (Q-S4-02).
- **What that means today:** in the app every reference currently shows **Not mapped**. The playbook's halt rule says exactly this: the feature never guesses.

## What a lawyer gets (once a signed succession pack is installed)

In **Drafting**, under the citation audit: **Check statutory references**. Each section cited in the draft shows:
- **Its class:**
  - same text;
  - renumbered;
  - changed — with the **old and new text side by side**;
  - new provision;
  - omitted;
  - not mapped.
- **Its counterpart:** "now BNS 103" or "earlier IPC 302".
- **The governing code for this case:** computed from the transition rules in the table and the case's key dates (date of offence, date of institution, …). If a needed date is missing, the panel asks for it ("When was the offence committed?") and shows no code until it is answered. If the reference cites the other code ("this reference cites BNS but the offence date selects IPC"), it is flagged in red.
- **Rows without legal-editor sign-off:** shown as "unsigned table (reference only)" and never treated as verified.

The citation audit also uses signed rows: a verified statutory citation whose provision was renumbered, changed or omitted becomes **Verified (see note)**, with a Code Bridge note naming the counterpart.

## How it works

- **`src-tauri/src/rule_tables.rs` (B-S4-03 core).** Official packs may carry rule tables (pack manifest v2.1, `metadata/rule_tables/*.json`, role `RULE_TABLE`). Because the manifest hashes are bound to the pack signature, a table cannot be edited after signing.
  - **At install**, and again on every load, a table is **rejected** (`CF_PACK_RULE_TABLE_INVALID` / `CF_PACK_RULE_TABLE_IN_CUSTOM_PACK`) when:
    - an authority does not resolve to an installed chunk;
    - an authority is not OFFICIAL_VERIFIED;
    - windows for one rule overlap, or a window ends before it starts;
    - a row is SIGNED_OFF without a legal-editor sign-off;
    - a MODIFIED mapping lacks both texts;
    - a row's kind differs from its table's;
    - it ships in a custom or v1 pack.
  - **Query API:** `query(kind, jurisdiction, as_of)`.
- **`src-tauri/src/code_bridge.rs` (B-S4-09).** References are found in generic forms: "section/s./u/s/rule/article N [of the] Code", plus "N CODE" for codes the installed rows name. Codes are matched lexically ("I.P.C." = "IPC"; "Indian Penal Code, 1860" = "IPC" by acronym).
  - **Class and counterpart:** taken from MAPPING rows only.
  - **Governing code:** from TRANSITION rows. A date on or after the switch date selects the new code. A missing date gives NEEDS_COUNSEL_INPUT with question `keydate:<kind>`. Transition rules that disagree also give NEEDS_COUNSEL_INPUT.
  - **No legal value lives in code:** every code, number and date comes from the signed tables.
- **Key dates** are counsel input records, stored in the case vault. They are the `basis_locator` of every routing.
- **AC-S4-RT.3:** Stage 4 feature code no longer branches on jurisdiction names. The witness date order (day-first vs month-first) moved into the statement-kind vocabulary data, and a static audit in the gate keeps it that way.

Discipline: `golden/G-CODEBRIDGE/fixtures/code_bridge_core.json` was committed (c3f8005) **before** the loader and engine, and is hash-pinned. It uses **synthetic** codes (Demo Penal Code → Demo Nyaya Sanhita), so no real succession answer is asserted without a legal editor.

## Gates

- `python eval/checks/rule_tables_b_s4_03.py --cargo-log <log>`
- `python eval/checks/code_bridge_b_s4_09.py --cargo-log <log> --annotations <evidence>/code_bridge_annotations.json`

Both run in CI. The new **Checks** workflow (`.github/workflows/checks.yml`) runs every gate and the Rust tests on each push, so nothing has to be compiled on a local machine. CI now fails on *any* failing gate; previously only the last command in a step could fail it.

## What is needed for real use (owner decisions)

1. **An official pack with the code texts:** for example the Bharatiya Nyaya Sanhita, BNSS and BSA texts plus the IPC, CrPC and Evidence Act texts, from official sources (India Code / Gazette), signed with the pack key.
2. **A named legal editor** to author and sign the succession rows (Q-S4-02).

## Deferred within B-S4-03 / B-S4-09

- **Pack tool:** pack-tool (`scripts/legal_pack_tool.py`) support for authoring and validating rule tables.
- **Exploit coverage:** extending the B-S3-09 malicious-pack fixtures to full install-time packs. Table-level exploits and the custom/v1 pack rejection are covered.
- **More annotation surfaces:** Digest, retrieval results and Opponent's Filing. The engine and command accept those contexts; only Drafting and the citation audit are wired.
- **Precedent annotations:** "decided under X → now Y" shown on precedents.
- **Interview integration:** the questions are asked inline in the panel, not yet through the guided-interview module.
