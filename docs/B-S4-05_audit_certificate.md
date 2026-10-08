# B-S4-05 — Audit Certificate

**Status (2026-10-08): IMPLEMENTED.** AC-S4-01.4 and AC-S4-01.5 PASS. Rust tests 93/93; frontend build clean.

## What a lawyer gets

After an audit (Drafting → Audit citations & quotes): **Create certificate (JSON + PDF)**. The signed JSON is the certificate; the PDF is a readable summary of it. Optional AI-use disclosure: written in counsel's own words, and only placed on the certificate after counsel ticks "I have reviewed this disclosure". **Verify a certificate**: load a certificate received from anyone, optionally check it against the draft in the editor.

The certificate records the *procedure* — which exact document (SHA-256), which installed packs, when, and every result — and carries a fixed scope statement taken verbatim from the frozen contract: it **does not** certify that the document is legally correct or compliant with any court's rules.

## Trust model (honest)

- Signed with a **per-install Ed25519 key**. The private seed is created from the OS random generator on first use and stored **only** in the encrypted app vault (`vault_meta`, SQLCipher) — never in a plain file.
- A valid signature proves the certificate (and, if supplied, the document) is **unchanged since this install signed it**. It does not by itself prove *who* signed: recipients compare the printed `key_id` (fingerprint of the public key) with the one the advocate publishes. Institution-issued keys are open question Q-S4-09.
- Prohibited wording (guaranteed, court-approved, sanction-proof, AI judge, predicts, decides, good law, valid law) can never reach a certificate.

## Independent verification

`scripts/verify_certificate.py` (also shipped inside the installer) re-implements canonical JSON, SHA-256 checks and **Ed25519 from RFC 8032** with the Python standard library only — so a court registry, opposing counsel or client can verify without CaseForge or any install:

```
python verify_certificate.py certificate.json --draft document.txt
```

Its Ed25519 was cross-checked against the `cryptography` library: 200/200 valid signatures accepted, 600/600 tampered signatures/messages/keys rejected.

## Evidence

- `golden/G-AUDIT/fixtures/certificate_reference.json` — a certificate produced by the Rust signer with a fixed **test** seed (never a real key). The Rust test requires the signer to reproduce it byte for byte; the gate validates it against both frozen contracts and verifies it with the independent Python verifier.
- Tampering rejected (Rust and Python independently): one extra byte in the document, a changed audit result, a changed summary, a flipped signature byte, an altered scope statement.
- Gate: `python eval/checks/certificate_b_s4_05.py --cargo-log <cargo test --lib output>` (also in CI).

## Known limitations

- Certifies drafts audited on the Drafting surface (plain text). Certifying an uploaded DOCX/PDF file's bytes comes with file auditing.
- Pack `currency_date` is empty until B-S4-13 (law currency stamp).
- The PDF summary maps typographic characters to ASCII (built-in PDF font); the JSON keeps the exact text.
