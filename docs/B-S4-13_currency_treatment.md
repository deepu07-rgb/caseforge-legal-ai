# B-S4-13 — Law Currency Stamp, Treatment Graph and delta packs

Every citation shows which pack it was checked against and how current that pack is; a judgment shows how later
judgments in the installed library treated it, with their exact words. None of this changes a citation's standing, and no
screen says a case is good law.

## Currency stamp

`verified against <pack> as of <currency date>` appears on every screen that shows a citation: the Citation Audit and the
Opponent's Filing citations, Mock Court authorities, retrieval citations, the working library, the Issue Matrix authorities,
foundation-gap and deadline authorities, and the Code Bridge text. The Reasoning panel lists the installed packs with their
dates. The date is the pack's own `currency_date` (the date its legal content was checked); a pack that states none says
*"this pack does not state a currency date"* — it is never guessed from another date. The audit's pack snapshot (and so
the audit certificate) now carries each pack's currency date. A test scans the screens: any component that renders a pack
citation without the stamp fails the gate.

## Treatment graph

A pack may carry `metadata/treatment_edges.json` (bound by its signed manifest): each edge says one installed judgment
treats another — overruled, reversed, distinguished, followed, doubted, per incuriam, referred to a larger Bench — and
stores the exact citing sentence. The pack-build tool finds them (`eval/tools/sc_citation_graph.py`, using the shared rules
in `eval/tools/treatment_patterns.py`); **the app never trusts the builder**. Whenever an edge is looked up
(`src-tauri/src/treatment.rs`) it is re-verified against the installed text:

1. it satisfies the frozen contract (derived content, `affects_supersession` false) — also checked when the pack is installed;
2. the citing passage is installed and is the passage the edge names; the cited and citing judgments differ and the cited one is installed;
3. the excerpt is the exact text at the stated place in the citing passage;
4. the excerpt really contains the treatment wording for that treatment, **not negated** (not, never, no, cannot...
   within 40 characters before or inside the cue) and **not inside a submission or a question** (submitted, urged, counsel,
   if, whether, should, may...). "Relying on" a case, "followed by", "no doubt" are not treatment.

An edge that fails any step is not shown (the lookup counts them). The wording rules exist twice, in Python for the
builder and in Rust for the host, and the gate compares them character for character.

How it shows: the audit adds a `TREATMENT` note for each adverse treatment (anything but "followed") and turns a VERIFIED
citation into VERIFIED_WITH_NOTE; other results keep their result and gain the notes. Judgments in the official packs are
recorded with unknown supersession status, which the audit already reports as "Not applicable / status unknown", so for them
the notes appear under that result. Each judgment citation also has *How later judgments treated this one* (opened on
demand): the verified passages with their wording, or **"No treatment found in installed corpus"** with a line that this
speaks only for the library; or, if no installed pack carries treatment data, a different line saying it could not be checked.

## Delta packs

A delta carries only what changed since a base (`delta: {base_version, base_content_sha256}` in the manifest).

1. Build the new full pack and sign it as usual.
2. `python eval/tools/build_delta_pack.py build --base <base> --new <new, signed> --out <delta>` — the delta has the new
   manifest naming its base, the new pack's own signature (it signs `content_sha256`, unchanged by sending fewer files) and
   only the files that differ. `... check --base <base> --delta <delta>` re-applies it as the app does.
3. The user installs the delta folder like any pack. The app **fails closed** unless the installed pack is exactly the base
   (version and content hash) and the delta is newer; files the delta does not carry are read from the installed base
   (decrypted if protected) and must match the hashes the manifest lists; the result then goes through the **ordinary
   install** (signature, lineage, treatment checks, protection, transactional activate). A delta can therefore install
   nothing a full pack could not, and any failure leaves the base installed and untouched (tested).

Cadence (proposed monthly for official packs, Q-S4-05) is policy, not code. Distribution is a file the user installs; the
channel is undecided (QUESTION). Only the failed-install rollback exists; a successful update does not keep the previous
version.

## Limits

- Treatment is found by wording in the citing sentence. A treatment expressed any other way is missed ("no treatment found"
  means none found *by these rules in the installed library*), and a later judgment outside the library is not covered. The
  corpus-wide precision of the detector is measured against the reporters' own labels by `sc_citation_graph.py`, not by this batch.
- "Reversed" is recognised only as "the decision/judgment/order ... was reversed in appeal / on appeal / by this Court /
  by the Supreme Court / by a Constitution or larger Bench"; a bare "reversed" is the history of a case, not treatment of a citation.
- Packs built before this batch carry treatment edges found with looser rules. The host re-verifies each edge against the new
  rules, so a loose edge simply stops being shown until the pack is rebuilt.

## Evidence

- Golden **G-TREATMENT** (pinned before the engine, `45eba02`): 9 planted treatments covering all seven kinds and 9 traps
  (a submission, a negation, counsel urging, "followed by", "no doubt", "not distinguishable", a question, case history,
  relying on a case); 14 stored edges with hand-authored verdicts — good ones, negated, argued, altered offsets, a different
  treatment than the excerpt supports, self-reference, chunk not installed, cited judgment not installed, wrong shape.
- Real installer, store and audit (custom test packs built on the fly): the planted overruling surfaces on the overruled
  citation with the exact citing paragraph; malformed edges and a missing currency date are refused at install; an altered
  edge is not shown; the delta suite.
- `eval/checks/treatment_b_s4_13.py`: pins, the golden against the Python rules, Rust/Python rule parity, the delta tool
  run on two packs, a scan that every citation screen carries the stamp, a scan that no screen says "good law", "valid
  law" or the like, static properties (no model or network; edges never change supersession; every audit decorated).
- **AC-S4-08.1 PASS. AC-S4-08.2 PASS. AC-S4-08.3 PASS.** Delta packs: PASS for the file-based pipeline; the distribution channel is a QUESTION.
