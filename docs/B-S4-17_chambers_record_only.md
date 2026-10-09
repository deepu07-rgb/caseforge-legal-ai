# B-S4-17 — Chambers Edition shell and Record-only mode (G7)

The Chambers Edition is the same application with a `JUDICIARY` licence. It shows the judge's screens, hides the lawyers'
advocacy tools, keeps the app offline, and confines everything a model says to the record and installed law. It assists a
judge's reading of the record; it never writes analysis, findings, holdings or orders.

## Who gets it (`src-tauri/src/edition.rs`)

- **Licence.** A version 2 licence (`contracts/licence_payload_v2.schema.json`) is checked offline: Ed25519 over the canonical
  payload with the keys in `contracts/licence_trust.json`, 3-of-4 machine fingerprint. The canonical form is the version 1
  rule applied to the whole object (every key but the signature, sorted, compact, non-ASCII escaped); a test pins it to
  Python's `json.dumps`, and the gate checks that the pinned strings are what Python produces.
- **Entitlement is the signed `features[]`.** The edition-to-feature map and prices are owner decisions (Q-S4-06, authored
  in B-S4-22); this batch only recognises the `chambers` feature and the contract's invariants for `JUDICIARY`
  (institution-issued, air-gap forced, no telemetry). A licence that does not verify, is for another machine, is version 1,
  or breaks those invariants grants nothing. An expired licence keeps its edition (the vault is read-only; work on the record
  stays readable and exportable).
- **Activation** accepts version 2 licences (`licence_activate`); a forced air-gap is switched on at activation and the switch
  cannot be turned off afterwards. `eval/tools/make_licence.py --edition JUDICIARY` issues a development licence.
- **Preview without a licence** (debug builds only): `CASEFORGE_DEV_EDITION=JUDICIARY`. Release builds never read it.

## What it changes

- **Screens.** Chambers shows a new **Chambers** screen and the Issue Matrix (which now belong to the Chambers Edition only)
  and hides the advocacy-only screens: Argument Builder, Argument Analysis, Voice, Experience, Mock Court, Opponent's
  Filing, Witnesses and the Integrity lab. Every other edition sees neither Chambers screen. The menu is tidied from the
  host's list; the host is what refuses.
- **The model has one road.** `check_generation` sits in front of every call to the local model: in Chambers, free
  generation (chat, Mock Court, voice, drafting help) is refused with `CF_EDITION_G7_FREE_GENERATION`. The only way through
  is `chambers_reason`, which holds a per-thread permit. The state is learned when the vault opens and when a licence is
  activated.
- **Commands.** The Issue Matrix and Chambers commands need the Chambers entitlement; opposition scan, the impeachment model
  scan and counsel's reasoning refuse in Chambers.
- **Pilot court constraints (Q-S4-10) are configuration**, not code: `chambers_pilot.json` in the vault folder, e.g.
  `{"court": "Pilot Court", "disabled_capabilities": ["memo_export", "citation_audit"]}`. Capabilities: `reasoning`,
  `issue_matrix`, `memo_export`, `citation_audit`. A disabled capability is refused in Chambers; a file that cannot be read
  switches all of them off (fail closed); no file switches none off. If a court's policy forbids another capability, the
  rule is to disable it, not to work around it (QUESTION).

## Record-only mode (G7) (`src-tauri/src/record_only.rs`)

The model answers with numbered statements. The app's own proposition verifier checks each against the passage it cites,
word for word. The G7 classifier then gives each statement a class and decides whether it renders:

| Class | What it is | Renders |
|---|---|---|
| `RECORD` | verified word for word against a passage of a document on the record (filings, evidence, orders) | yes, with document, page, paragraph and the exact excerpt |
| `LAW` | verified word for word against an installed official, applicable, current authority | yes, with the authority, its chunk and the exact excerpt, and the pack's currency stamp |
| `INFERENCE_FROM_RECORD` | rests, by listed bases, only on statements that themselves render | yes, with its bases' locators |
| `EXTRA_RECORD` | everything else: model memory, an invented or altered quotation, a custom pack, something said in the question, an inference resting on any of those, no support | **no** — counted by reason, logged by hash (never the text) |

- The model is offered **passages of the record** (paragraphs of every document in the matter that share a word with the
  question, best first, within what the model can take), never the question as a source. The answer says how many of the
  record's passages were offered.
- Custom packs never supply law or record here. (The PRD allows law from a custom pack the user marked "law, not facts"; this is
  stricter, and the marking arrives with the first feature that needs it.)
- The unit of classification is the statement the model structured, not each sentence inside it.
- **On by default** in Chambers. It can be switched off for a matter until a judgment scaffold exists; then unsupported
  statements show labelled **EXTRA_RECORD — NOT FROM THE RECORD**, never as record or law. Once a scaffold exists (written by
  B-S4-21 under `scaffold:`) the switch is refused (`CF_G7_LOCKED`) and record-only is on whatever was stored.
- Statements pass the existing safety policy afterwards. The decision-free zone (G8) for scaffolds arrives with B-S4-21.

## Evidence

- Golden **G-CHAMBERS record-only** (pinned before the engine, `d725ba0`): five legitimate statements (two record, one law, two
  inferences) and nine planted extra-record ones: model memory, an invented quotation, a custom-pack law, a fact in the
  prompt, an inference resting on the prompt, an inference with no bases, a statute passage offered as a fact, an inference
  resting on model memory, an explicit gap. The statements go through the real verifier first.
- Rust tests: the golden (including that the log holds hashes only, the withheld count line, determinism); the switch-off
  behaviour; the lock; inference carries its bases' locators; the edition tests (signed Judiciary licence, every other
  edition, tampered/unknown-signer/other-machine/one-component-differs/v1, Judiciary invariants, expired, the model gate and
  its per-thread permit, the air-gap switch, pilot configuration).
- `eval/checks/chambers_record_only_b_s4_17.py`: pins; the golden contains every planted kind; the Rust surface list equals the
  app's navigation ids; the licence canonical form matches Python; static properties (model gate in both entry points, permit
  only on the record-only path, every Chambers-only command and every advocacy command guards itself, the air-gap guard, the
  question is never a source, the log keeps hashes, the vault refreshes the edition on unlock, v2 activation, the screen shows
  class and source and never calls raw generation, the menu follows the edition).
- **AC-S4-11.1 PASS** (planted extra-record content never renders; Q-S4-10 pilot constraints are configuration).
  **AC-S4-11.2 PASS** (every rendered statement has a class and a locator).

## Not done here

- Licence payload v2's activation screen, seat transfer, pack-subscription expiry notices, and the edition-to-feature map and
  prices are B-S4-22. This batch recognises only the `chambers` feature.
- The judgment scaffold and its decision-free zone (B-S4-21), both-sides citation check and record lookup (B-S4-19), and the
  daily cause-list brief (B-S4-20) build on this shell.
- Reasoning quality has not been measured with the real 7B model on a real record (needs your machine); the host's checks do not depend on it.
