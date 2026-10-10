# B-S4-16 — Encrypted Bundle Exchange (`.cfbundle`)

PRD v4.0 §8. Acceptance: AC-S4-10.1 to 10.4. Gate: `eval/checks/bundle_b_s4_16.py` (report `eval/reports/b_s4_16_bundle.json`).
Golden, pinned before the engine (commit `52749d5`): `golden/G-BUNDLE/fixtures/bundle_core.json`. Contract (frozen in B-S4-01, unchanged):
`contracts/bundle_manifest.schema.json`.

**Done when:** a junior can hand a senior a complete, encrypted, attributable case bundle on a USB stick. That works, between two real
vaults, in a test. Read "What it does not do" before relying on it.

## What a lawyer does

In a matter, **Share with colleagues**:

1. **People and keys.** Make your card (your name; a small public file) and a six-group code. Give your card to people who will send you
   bundles, and read them your code in person or by phone. Add the cards people give you and type the code they read to you. A contact
   whose code you have not confirmed cannot be sent anything.
2. **Send a bundle.** Pick the verified people, what to include (the case understanding, argument tree, documents as the text CaseForge read
   from them, your corrections, a note), and any names, numbers or phrases to hide. *Build the bundle*, then save the `.cfbundle` file to a USB
   stick or folder, or *Offer on this network*.
3. **Receive a bundle.** Open the file (or type the numeric address shown on the other computer). You see who sent it, whether you have
   checked their code, what is in it and what they hid; then *Open and keep in this matter*. Everything is kept beside the matter, each item
   labelled *from <sender>, <date>*.

## The format

`CFBUNDLE1\n`, a 4-byte length, the manifest (JSON, signed), then each item as `nonce(12) ‖ AES-256-GCM ciphertext+tag`, in manifest order, and
nothing after. Every byte is covered: the container's length checks, the manifest's Ed25519 signature, and per item a hash and GCM
authentication.

| Piece | Choice |
|---|---|
| Identity | two keys derived by HKDF from the install's existing per-install secret, each with its own label: an Ed25519 signing key and an X25519 agreement key. The bundle signing key is **not** the key that signs Audit Certificates. `key_id` = hash of both public keys |
| Key exchange | a public **card** (name, both public keys, signed by its own key), by file; a verification code of six groups of five digits from both public keys, read aloud |
| Encryption | a random content key per bundle; each item AES-256-GCM with the bundle id, item id and kind as associated data (an item cannot be moved or swapped); the key wrapped per recipient with a fresh X25519 key and HKDF |
| Signature | Ed25519 over the manifest's canonical bytes |
| Frozen-schema note | the schema's `recipients[].agreement_public_key_hex` carries the **ephemeral** key made for that recipient (the recipient needs it; the schema has no other field). The recipient's own key is named by `key_id` |

Primitives are checked against published vectors (RFC 4231 HMAC, RFC 5869 HKDF, RFC 7748 X25519). X25519 uses `curve25519-dalek`, which was already
in the lock file as `ed25519-dalek`'s dependency; HKDF/HMAC are written out over `sha2` (nothing new was downloaded). 

## Redaction

Before anything is encrypted, each phrase you give is replaced by `[REDACTED]` wherever it appears: matching is by wording, with optional
whitespace between any two characters (so it is found split across a line break or with the spaces left out), longest phrases first, in one
pass (a replacement is never searched again), exact or ignoring capitals. It covers every text value, filenames included. Only a **count** per item is
kept in the manifest. An independent check then looks for each phrase (by normalising, not by pattern) in what is about to be encrypted, and the export is
**refused** if any survives. The tests then look for each phrase, in raw, JSON-escaped, upper, lower, spaceless and line-split shapes, in the
file's bytes and in everything decrypted from it. Your own copy of the matter is not touched.

## Receiving, and corrections

The bundle's manifest is readable without a key (see below), so *Inspect* shows who sent it and what it holds before anything is decrypted. Import
needs the recipient's key; the sender must be a contact whose code you checked, or you accept an unchecked sender by hand (and even then they do not
become trusted). Received items are stored in the matter's encrypted store as received; a bundle can be imported once.

**Received corrections never replace yours.** The import path writes no correction, graph, tree or document of yours (a source scan checks this),
and across two real vaults all of those are identical before and after. Each received correction is listed against your own for the same
target: *suggested by them* (you have none), *same as yours*, or *differs from your correction — yours stays*. They are attributed to the sender as
counsel input.

## LAN transfer

Optional. The sender's computer offers the (already encrypted) file once on the local network; the other types its numeric address. It follows the
Phone Companion's rules: bind only a private, link-local or loopback address; connect only to such a numeric address (a name is never looked up); drop
a peer that is not local before reading a byte; one download at a secret path, then stop (or after 10 minutes). It is refused under air-gap (use USB).

## What it does not do

* **No packet capture.** AC-S4-10.3 asks for a network capture showing LAN-only traffic. The LAN-only rules are enforced and tested in code, over
  loopback, and the addresses used are recorded; but no capture has been made on a real network, so the gate reports that criterion **BLOCKED**, not PASS.
* **A card is trusted only as far as the two of you checked the code.** Anyone can make a bundle for your public key; what protects you is the
  signer's key and the code you confirmed. If you accept an unchecked sender, that is on you.
* **The manifest is not secret:** sender name, item kinds and sizes, recipient key ids, redaction counts and the date are readable by anyone holding
  the file. Item contents are not.
* **Redaction finds the phrases you give it.** A nickname, another spelling or a number you did not list is not found. Redaction applies to text
  values; the original document files are never put in a bundle (only the text CaseForge read), partly for this reason.
* **What is carried:** the case understanding, argument tree, documents (as text, up to 40,000 characters for plain files), your corrections, and a note. A
  digest, impeachment kits and audit reports have a place in the format but are not offered from their screens yet.
* **Received items are kept and shown, not merged into your matter.** There is no button that turns a received graph or tree into yours; that, and
  accepting a received correction through the normal correction screen, are follow-ups.
* **No QR code.** Cards are files and codes are read aloud; drawing a QR code needs a library that is not in the app and was not added.
* Not checked by a person on the screen: the Share tab type-checks and its wording is scanned, but no one has used it in the app, and nothing was
  tried with a real USB stick or two real computers.
* A lost computer loses its keys with its vault; contacts will have to re-add the new card.

## Licence

The commands need the `bundle_exchange` feature (Pro, Chambers (firm) and Judiciary in the placeholder edition map). Under a Judiciary licence the
air-gap is forced on, so only the USB route is available.
