# CaseForge — Product Requirement Document v4.0 (Stage 4: Verified Advocacy & Chambers)

**Document status:** DRAFT — proposed for owner approval (2026-10-07)  
**Target release:** `v4.0.0` (with an optional early `v3.1.0` Advocate release — see §14)  
**Relationship to earlier documents:** This PRD **extends** `CaseForge_PRD_v1.0.md` and builds on the Stage 3 trust layer in `docs/CASEFORGE_STAGE3_AGENTIC_PLAYBOOK.md`. Where this document is silent, v1.0 and the Stage 3 playbook continue to govern. Where it amends an earlier rule, the amendment is listed explicitly in §3.  
**Batch plan:** `docs/CASEFORGE_STAGE4_AGENTIC_PLAYBOOK.md`

**Prime rule (unchanged):** If a detail is missing from Layers 1–4, **HALT** and raise a `QUESTION`. Never invent legal content, statute mappings, limitation periods, or court rules. Values in this document marked **PROPOSED** are starting points that the owner must freeze in B-S4-01; they are not yet normative.

---

## 0. Why Stage 4 exists

Stages 1–3 built an offline engine that can read a case file, remember it, verify authority against installed law, and spar with counsel. Stage 4 turns that engine into a product a lawyer or a judge will **pay to renew every year**, by focusing on one market truth:

> In 2023–2026 courts in both India and the United States began sanctioning lawyers for AI-fabricated citations and misquotations, and judges withdrew orders containing AI errors. Every AI legal tool now has a trust problem. CaseForge is the only one whose output is **local, deterministic where it matters, and provably source-linked**.

Stage 4 therefore does **not** chase a stronger model. It sells *proof*:

1. **Proof your filing is clean** — Citation & Quote Audit Certificate.
2. **Proof the other side's filing is not** — Opposing Filing Scan.
3. **Proof the witness contradicted themselves** — Impeachment Kit.
4. **Proof the law still applies** — code bridge, treatment graph, currency stamp.
5. **Proof the evidence can get in, and the deadline is not missed** — foundation gaps, deadline watch.
6. **Proof for the bench** — a Chambers Edition that is locked to the record and never decides.

---

## 1. Product truth [STRATEGY — binding on marketing and UI copy]

Unchanged from v1.0 §1 and Stage 3 §2. Additionally:

- CaseForge **verifies and organises**; it does not advise, decide, predict, or certify legal correctness.
- An Audit Certificate certifies that **a verification procedure was run against a stated corpus on a stated date** — never that a filing is legally correct, complete, or compliant with any court's rules.
- Chambers Edition **assists a judge's reading of the record**; it never writes analysis, findings, holdings, or operative orders.
- Deadline and limitation outputs are **computations to verify**, never legal advice.

Marketing must never say: "guaranteed", "court-approved", "sanction-proof", "AI judge", "predicts", "decides".

---

## 2. Personas

| Persona | Jurisdiction | Primary job in Stage 4 |
|---|---|---|
| **Junior advocate / associate** | IN / US | Prepare paperbook digest, list of dates, cross-examination kit; audit drafts before filing |
| **Senior counsel / trial lawyer** | IN / US | Rehearse timed oral argument; review opposing filing; impeach witnesses |
| **Solo / small chamber** | IN / US | Everything above on one laptop, offline, with deadline safety |
| **Trial / district judge** | IN | Read many matters a day; frame issues; check both sides' citations; draft fact recitals |
| **Judge's law clerk / research attorney** | US / IN HC | Prepare bench memo and issue matrix strictly from the record |

---

## 3. Amendments to earlier documents [NORMATIVE]

| Earlier rule | Amendment |
|---|---|
| v1.0 §2 "English primary" | Indian-language **ingestion** (OCR + translation, Hindi first) is added in Stage 4. Reasoning, Mock Court and UI remain English. |
| v1.0 §2 "All features identical for every jurisdiction; only the knowledge base differs" | **Retained and strengthened.** All jurisdiction-specific behaviour in Stage 4 (code bridge, limitation periods, admissibility requirements, court calendars) must be **data in signed Rule Tables** (§5), never code branches on jurisdiction. |
| Stage 3 §6 deferred "Hindi or regional-language OCR" | Lifted for Hindi (Devanagari) in B-S4-14/15. Other scripts remain deferred until owner names them (Q-S4-03). |
| Stage 3 §6 deferred "firm collaboration" | Lifted **only** for offline encrypted bundle exchange (§8). Cloud sync, shared servers, and real-time co-editing remain non-goals. |
| v1.0 §12 surfaces | Adds Stage 4 surfaces: AUDIT · OPPOSITION · WITNESSES · DEADLINES (lawyer) and CHAMBERS (judge edition). |
| v1.0 §15 guardrails G1–G6 | Adds G7–G10 (§12). |
| v1.0 §20 non-goals | **Unchanged.** No verdict prediction, no named-judge profiling, no automatic decision-making, no cloud case management, no marketplace. |

---

## 4. Prerequisites [NORMATIVE]

- **PRE-1:** Stage 3 ship gate (`B-S3-22`) returns at least `READY FOR CONTROLLED TESTING`.
- **PRE-2:** The proposition ledger and hard quotation verifier (`B-S3-14`) are the **only** path by which any Stage 4 feature may mark a citation, quotation, fact, or legal proposition GREEN.
- **PRE-3:** Legal source/chunk metadata v2 (`B-S3-08`) — trust, temporal status, supersession — is the only source of "current / applicable" status.
- **PRE-4:** Case graph v2 (`B-S3-06`) witness, statement, `witness_to_statement`, `asserts`, `denies`, `contradicts`, and `issue` structures are the only inputs to the Impeachment Kit and Issue Matrix.
- Contract and golden authoring batches (B-S4-01, B-S4-02) may run in parallel with B-S3-21/22 because they contain no product code. All other Stage 4 batches require PRE-1.

---

## 5. Foundation: Signed Rule Tables [NORMATIVE]

A new `.akpkg` content kind, `RULE_TABLE`, carries **deterministic, jurisdiction-specific legal data** so that the engine stays identical across markets.

Each rule-table row:

- has a stable `rule_id`, `table_kind`, `jurisdiction`, `effective_from`, `effective_to`;
- cites **at least one** authoritative chunk (`source_id` + `chunk_id`) from an installed official pack whose text grounds the row;
- carries `legal_editor_signoff` (name/role + date) — rows without sign-off load as `DEVELOPMENT_ONLY` and can never render GREEN;
- is signed with the pack (Ed25519, manifest v2 checksums, as B-S3-08/09).

Initial `table_kind` values:

| table_kind | Used by | Example content (illustrative only, must be authored from official text) |
|---|---|---|
| `STATUTE_SUCCESSION` | Code bridge (§7.4) | IPC → BNS, CrPC → BNSS, Indian Evidence Act → BSA section equivalences; savings/transition provisions; US rule amendments (e.g., FRE, FRCP) with effective dates |
| `LIMITATION_PERIOD` | Deadline watch (§7.6) | Period, trigger event type, exclusions/extension provisions, source provision |
| `PROCEDURAL_DEADLINE` | Deadline watch | Response/appeal/revision windows keyed to event types |
| `COURT_CALENDAR` | Deadline watch | Court holidays and vacation periods per court, per year |
| `ADMISSIBILITY_REQUIREMENT` | Foundation gaps (§7.5) | Evidence type → required foundation elements (e.g., electronic-record certificate, authentication, hearsay exception) |
| `ISSUE_FRAMING_TEMPLATE` | Chambers issue matrix (§9.2) | Matter type → standard issue skeletons |

**AC-S4-RT.1:** A rule-table row without a resolvable authoritative chunk is rejected at install.  
**AC-S4-RT.2:** A row without legal-editor sign-off can never contribute to a GREEN output.  
**AC-S4-RT.3:** Static audit shows zero `if jurisdiction == …` branches in Stage 4 feature code.

---

## 6. Feature map and tiers

| # | Feature | Surface | Persona | Tier (PROPOSED) |
|---|---|---|---|---|
| F1 | Citation & Quote Audit + Audit Certificate | AUDIT | Lawyer | Advocate |
| F2 | Opposing Filing Scan | OPPOSITION | Lawyer | Pro |
| F3 | Witness Statement Matrix + Impeachment Kit | WITNESSES | Lawyer | Pro |
| F4 | Code Bridge (statute succession) | DIGEST / AUDIT | Lawyer, Judge | Advocate |
| F5 | Admissibility & Foundation Gaps | ARGUMENT TREE | Lawyer | Pro |
| F6 | Deadline & Limitation Watch | DEADLINES | Lawyer | Advocate |
| F7 | Timed Oral Rehearsal | MOCK COURT | Lawyer | Pro |
| F8 | Law Currency Stamp + Treatment Graph | everywhere a citation renders | All | Advocate |
| F9 | Hindi ingestion (OCR + aligned translation) | CASEBOOK | Lawyer, Judge (IN) | Advocate (IN packs) |
| F10 | Encrypted Bundle Exchange | CASEBOOK | Firm | Chambers/Firm |
| F11 | Chambers Edition: Record-only mode | CHAMBERS | Judge | Judiciary |
| F12 | Issue Matrix | CHAMBERS | Judge | Judiciary |
| F13 | Both-sides citation check + record lookup | CHAMBERS | Judge | Judiciary |
| F14 | Daily Cause-List Brief | CHAMBERS | Judge (IN) | Judiciary |
| F15 | Judgment Scaffold with Decision-Free Zone | CHAMBERS | Judge | Judiciary |

---

## 7. Lawyer features [NORMATIVE]

### 7.1 F1 — Citation & Quote Audit and Audit Certificate

**Input:** a draft authored by the user (DOCX/PDF from the Drafting surface or imported).

**Behaviour:**
1. Extract every citation (case, statute, rule, neutral citation, reporter cite) and every quoted passage, with character offsets in the draft.
2. Resolve each against installed packs via the B-S3-11 retrieval and B-S3-14 verifier.
3. Classify each item:
   - `VERIFIED` — authority resolves to `OFFICIAL_VERIFIED`, `APPLICABLE`, `CURRENT` source; quotation is an exact stored substring (offsets returned).
   - `VERIFIED_WITH_NOTE` — resolves but carries treatment/supersession/currency notes (F8) or code-bridge note (F4).
   - `MISQUOTED` — authority resolves, quoted text does not match exactly; show nearest exact passage and diff.
   - `NOT_IN_CORPUS` — cannot resolve; repository gap (never "fake" — the corpus may be incomplete).
   - `SUPERSEDED` / `NOT_APPLICABLE` — per source metadata.
   - `CUSTOM_SOURCE` — resolves only to a custom/untrusted pack.
4. Pinpoint check: cited paragraph/page numbers verified against stored page lineage.

**Audit Certificate (export):** a PDF + machine-readable JSON containing
- SHA-256 of the audited draft; date/time; app version; model fingerprint (if any generative step ran);
- installed pack IDs, versions, and **currency dates**;
- per-item result table with locators;
- counts by class; an explicit statement of scope and limits (§1);
- an optional AI-use disclosure paragraph whose template text comes from a pack (`DISCLOSURE_TEMPLATE`, user-editable, never auto-asserted);
- a detached Ed25519 signature by a per-install key, plus an offline `verify` command so anyone with the certificate can detect tampering.

**AC-S4-01.1:** 100 % of planted fabricated citations in G-AUDIT render `NOT_IN_CORPUS`; zero render `VERIFIED`.  
**AC-S4-01.2:** 100 % of planted single-word misquotations render `MISQUOTED` with a correct diff.  
**AC-S4-01.3:** Citation extraction recall ≥ **PROPOSED 98 %** on G-AUDIT (IN and US reported separately).  
**AC-S4-01.4:** Any byte change to a certified draft or certificate fails `verify`.  
**AC-S4-01.5:** Certificate text never contains prohibited phrases (§1).

### 7.2 F2 — Opposing Filing Scan

**Input:** the opponent's pleading, brief, reply, written submission, or affidavit, tagged at intake as `EXTERNAL_FILING` with the filing party.

**Rules:**
- Content of an external filing is **adverse assertion**, never matter fact. It enters the case graph only as `asserts` / `denies` edges owned by the filing party.
- External filings are untrusted input: prompt-injection isolation identical to matter documents (Stage 3).

**Outputs:**
1. Full F1 audit of their citations and quotations (they get the same classification).
2. **Record conflicts:** each factual assertion in their filing compared against counsel-confirmed and document-extracted facts; conflicts listed with both locators.
3. **Unsupported assertions:** assertions with no cited record support in their own filing.
4. **New-argument detection:** arguments in their filing with no matching node in the user's Argument Tree, offered as "add to tree / rehearse in Mock Court".

**AC-S4-02.1:** All planted fabricated and misquoted authorities in G-OPPOSING are detected.  
**AC-S4-02.2:** Recall of planted record conflicts ≥ **PROPOSED 90 %**; every reported conflict has two valid locators.  
**AC-S4-02.3:** Zero external-filing assertions appear as matter facts anywhere in Digest, Drafting, or Mock Court.  
**AC-S4-02.4:** Injection text inside an external filing changes no system behaviour.

### 7.3 F3 — Witness Statement Matrix and Impeachment Kit

**Input:** case graph v2 witness and statement nodes. India: FIR, statements recorded by police and before a magistrate, examination-in-chief, cross; US: declarations, deposition transcripts (page:line), interrogatory answers, trial testimony. The statement-kind vocabulary is pack data, not code.

**Behaviour:**
1. **Witness resolution:** aliases merged with counsel confirmation (authority hierarchy applies).
2. **Topic alignment:** each statement segmented into topic-tagged assertions (who/what/when/where/how much/identification).
3. **Contradiction detection, two layers:**
   - *Deterministic*: conflicting dates, times, amounts, counts, places, named persons, presence/absence, sequence.
   - *Model-proposed*: semantic conflicts (e.g., "saw the accused" vs "it was too dark to see") — each must pass the proposition verifier with exact excerpts from both statements, otherwise discarded.
4. **Omissions/improvements:** a material fact present in a later statement but absent in an earlier one is flagged as `OMISSION` (relevant to Indian criminal practice) — labelled as a candidate for counsel judgement.
5. **Impeachment Kit export:** per witness, a cross-examination outline: topic → prior statement (exact quote + locator) → later statement (exact quote + locator) → suggested confrontation sequence. Questions are suggestions; the export carries G4 footer.
6. Mock Court *Witness Cross-Examination* format consumes the kit.

**AC-S4-03.1:** Deterministic contradiction recall ≥ **PROPOSED 95 %**, precision ≥ **PROPOSED 90 %** on G-IMPEACH.  
**AC-S4-03.2:** 100 % of rendered contradictions carry exact quotes and valid locators for both sides.  
**AC-S4-03.3:** Model-proposed contradictions failing verification never render.  
**AC-S4-03.4:** G3 holds: requests to coach a witness to change testimony are refused.

### 7.4 F4 — Code Bridge (statute succession)

**Behaviour:** using `STATUTE_SUCCESSION` rule tables:
- Every statutory reference in the case graph, drafts, and retrieved authorities is annotated with its successor/predecessor and an equivalence class: `IDENTICAL`, `RENUMBERED`, `MODIFIED`, `NEW`, `OMITTED`, `UNKNOWN`.
- **Temporal routing:** the governing code is computed from rule-table transition provisions and the case's key dates (e.g., date of offence, date of institution of proceedings). If a required date is missing or ambiguous, the result is `NEEDS_COUNSEL_INPUT` — never a guess.
- Precedents decided under a predecessor provision show "decided under X → now Y (class)".
- US: the same engine handles amended rules with effective dates.

**AC-S4-04.1:** 100 % of G-CODEBRIDGE mapping rows match legal-editor-approved answers.  
**AC-S4-04.2:** Missing key date yields `NEEDS_COUNSEL_INPUT` in 100 % of planted cases.  
**AC-S4-04.3:** `MODIFIED` mappings always show both texts side by side.

### 7.5 F5 — Admissibility & Foundation Gaps

**Behaviour:** each evidence/exhibit node is matched to `ADMISSIBILITY_REQUIREMENT` rows by evidence type; each required foundation element is marked `PRESENT` (with locator), `ABSENT`, or `UNKNOWN`.
- Gaps appear in the Missing Links panel with the rule row and its authority.
- Scoring change is versioned: `caseforge.integrity.v2` adds a foundation factor to the Evidence link. v1 scoring remains reproducible for existing sessions (AC-8.3 determinism preserved; no LLM in scoring).

**AC-S4-05.1:** Planted missing foundation element appears in top-3 Missing Links.  
**AC-S4-05.2:** integrity.v1 outputs remain byte-identical for v1 sessions.  
**AC-S4-05.3:** Every gap cites a rule row and its authoritative chunk.

### 7.6 F6 — Deadline & Limitation Watch

**Behaviour:**
- Trigger events come from the verified chronology (counsel-confirmed or document-extracted with locator).
- Deadlines computed deterministically from `LIMITATION_PERIOD`, `PROCEDURAL_DEADLINE`, and `COURT_CALENDAR` rows, applying stated exclusions only when the triggering fact is present.
- Each deadline shows: computed date, rule row, authority, trigger event + locator, every exclusion applied, and a permanent **VERIFY** label.
- If any input is uncertain, show a **date range** and the reason, never a single date.
- Local-only reminders and `.ics` export. No network.

**AC-S4-06.1:** 100 % match with legal-editor answers on G-DEADLINE (including holiday roll-over and exclusions).  
**AC-S4-06.2:** Ambiguous trigger date always produces a range with reason.  
**AC-S4-06.3:** No deadline renders without rule, authority, and trigger locator.

### 7.7 F7 — Timed Oral Rehearsal

- Mock Court gains a **timer mode**: user sets the slot (PROPOSED presets: 2, 5, 10, 15, 30 minutes) and the bench archetype's interruption rate.
- Bench interruptions are generated from Missing Links and weakest limbs (grounded, B-S3-16 path).
- Debrief adds a **timeline**: when each limb was reached, when the user was interrupted, which points were never reached, time spent per limb. Findings follow B-S3-18 source-specific rules.

**AC-S4-07.1:** Timer accuracy ±1 s; session ends gracefully at limit.  
**AC-S4-07.2:** Debrief timeline lists every limb with reached/not-reached status.  
**AC-S4-07.3:** All interruptions are grounded per B-S3-16.

### 7.8 F8 — Law Currency Stamp and Treatment Graph

- Every rendered citation shows "**verified against <pack> as of <currency date>**".
- **Treatment graph:** at pack build time, installed judgments are scanned for references to other installed judgments together with treatment language (`overruled`, `reversed`, `distinguished`, `followed`, `doubted`, `per incuriam`, `referred to larger bench`). Each treatment edge stores the exact citing paragraph.
- Treatment edges are pack-build artefacts, verified like any `DERIVED` content: shown with the citing excerpt; never alter `CURRENT` status by themselves (supersession remains source metadata).
- Absence of a treatment edge renders as "no treatment found in installed corpus", never "good law".
- Pack delta cadence: **PROPOSED monthly** for official packs (owner decision Q-S4-05).

**AC-S4-08.1:** Planted overruling in G-TREATMENT surfaces on the overruled citation with exact citing paragraph.  
**AC-S4-08.2:** UI never displays "good law", "valid", or equivalent.  
**AC-S4-08.3:** Every citation surface shows a currency date.

### 7.9 F9 — Hindi ingestion (OCR + aligned translation)

- Triage detects script per page/region (Latin, Devanagari, mixed).
- Offline Devanagari OCR with calibrated confidence (B-S3-05 method).
- Offline translation to English at **segment** level, stored with alignment to the original segment.
- UI always shows original and translation side by side.
- Facts extracted from translated text carry `TRANSLATED` provenance; critical fields (dates, amounts, names, section numbers) require counsel confirmation before they count as verified.
- **Quotations:** only text in the original language can be a verified quotation. A translated quote is labelled "translation — wording not verified".

**AC-S4-09.1:** Devanagari OCR CER ≤ **PROPOSED threshold** (owner freezes after bake-off, Q-S4-04) on G-INDIC.  
**AC-S4-09.2:** Critical-field accuracy after translation ≥ **PROPOSED 98 %** or field is flagged for confirmation.  
**AC-S4-09.3:** No translated sentence renders as a verified quotation.

---

## 8. Collaboration [NORMATIVE]

### 8.1 F10 — Encrypted Bundle Exchange (`.cfbundle`)

- Sender selects content: digest, documents, graph subset, argument tree, impeachment kit, notes.
- Optional redaction step; redactions are irreversible in the bundle.
- Encrypted to recipient public keys (exchanged by QR or file, offline); signed by sender.
- Transport: USB or LAN only (re-uses Phone Companion LAN rules). **No cloud, no relay.**
- Import: recipient sees provenance of every item ("from <sender>, <date>"); counsel corrections from a bundle enter as `COUNSEL_INPUT` attributed to the sender and never silently override the recipient's own corrections (authority hierarchy).

**AC-S4-10.1:** Bundle undecryptable without the recipient key; tampered bundle rejected.  
**AC-S4-10.2:** Redacted text absent from every byte of the bundle.  
**AC-S4-10.3:** Network capture during exchange shows LAN-only traffic.  
**AC-S4-10.4:** Import never overwrites a recipient correction.

---

## 9. Chambers Edition (judges and clerks) [NORMATIVE]

Chambers Edition is the same application with a `JUDICIARY` licence entitlement. It hides lawyer-only advocacy surfaces (opposition aggression, persuasion coaching) and enables CHAMBERS.

### 9.1 F11 — Record-only mode (Guardrail G7)

- In Chambers Edition, record-only mode is **on by default and cannot be turned off for a matter once a judgment scaffold exists**.
- Allowed inputs: filings by parties, evidence on record, orders in the matter, and installed law packs.
- Every generated sentence is classified: `RECORD`, `LAW`, `INFERENCE_FROM_RECORD`, or `EXTRA_RECORD`. `EXTRA_RECORD` content is blocked from rendering.
- Custom packs may be used only if marked by the user as "law, not facts"; they never supply facts.

**AC-S4-11.1:** Planted extra-record facts (from model memory, custom packs, or prompts) never render.  
**AC-S4-11.2:** Every rendered sentence carries a classification and locator.

### 9.2 F12 — Issue Matrix

- From both parties' pleadings: **proposed issues** (using `ISSUE_FRAMING_TEMPLATE` rows + graph `issue` nodes). The judge must accept, edit, merge, or reject each; only judge-accepted issues proceed.
- For each accepted issue: Party A position, Party B position, evidence each relies on (locators), **admitted facts**, **disputed facts**, authorities each cites (with F1 audit status), and **unanswered arguments** — an argument by one side with no responsive node from the other (deterministic graph query).
- The matrix never states which side is stronger, never scores parties, and never suggests an answer to an issue (G1, G8).

**AC-S4-12.1:** Planted unanswered argument is listed in 100 % of G-CHAMBERS fixtures.  
**AC-S4-12.2:** Admitted/disputed classification matches golden ≥ **PROPOSED 95 %**.  
**AC-S4-12.3:** Red-team: zero outputs rank, score, or favour a party.

### 9.3 F13 — Both-sides citation check and record lookup

- One action runs F1 audit over **every** filing by **every** party; results grouped by party.
- **Record lookup:** text or voice query ("where did the witness say the car was red?") returns ranked locators from the record only, with exact excerpts. Answers are excerpts, not summaries.

**AC-S4-13.1:** Lookup top-5 contains the correct locator for ≥ **PROPOSED 90 %** of G-CHAMBERS queries.  
**AC-S4-13.2:** Lookup returns only record excerpts; zero generated prose.

### 9.4 F14 — Daily Cause-List Brief (India-first; engine jurisdiction-neutral)

- Input: a cause list imported as a file (PDF/CSV/manual entry). **No online court-portal integration** (offline rule).
- Each listed matter is matched to a local matter; per matter a one-page brief: stage, last order (exact excerpt + date), compliance directed in last order and whether the record shows it done, number of prior adjournments and by whom sought (from orders on record), pending applications.
- Unmatched matters are listed as "no local record".

**AC-S4-14.1:** Every brief field cites an order/document locator or shows "not found in record".  
**AC-S4-14.2:** A 100-matter list renders briefs within **PROPOSED 60 s** on the Full reference machine.

### 9.5 F15 — Judgment Scaffold with Decision-Free Zone (Guardrail G8)

- Scaffold sections: cause title, appearances, brief facts, procedural history, submissions of each party, issues (judge-accepted), evidence summary, **analysis**, **findings**, **operative order**.
- CaseForge may pre-fill only: cause title, appearances, brief facts, procedural history, submissions, issues, evidence summary — each sentence with locator, all record-only.
- **Analysis, findings, and operative order are locked**: the model is never invoked for them, the UI provides no generate button, and any prompt asking for them is refused (G1/G8). Spellcheck, formatting, and citation audit (F1) of judge-typed text are allowed.
- Export carries G4 footer stating which sections were machine-assisted.

**AC-S4-15.1:** Static audit: no code path sends locked-section content requests to any model.  
**AC-S4-15.2:** Red-team (≥ **PROPOSED 100** Chambers prompts): zero findings, holdings, or orders generated.  
**AC-S4-15.3:** Every pre-filled sentence has a valid locator.

---

## 10. Licensing and commercial model [STRATEGY + NORMATIVE]

**Normative (extends v1.0 §13):**
- Licence payload gains `edition` (`ADVOCATE`, `PRO`, `CHAMBERS_FIRM`, `JUDICIARY`) and `pack_subscription_until`, separate from `app_licence_until`.
- **App licence expiry:** vault read-only (unchanged). User data is never ransomed.
- **Pack subscription expiry:** installed packs remain usable at their last currency date; every citation shows the older currency date with an "updates expired" notice. No new deltas.
- **Seat transfer:** deactivate on one machine, activate on another, offline, with a limit per year (PROPOSED 2; Q-S4-07).
- Judiciary licences: institution-issued, no telemetry at all, air-gap mode forced on.

**Strategy (non-normative):**
- Price the **pack subscription** as the renewal anchor — the law changes every month even if the user does not need new app features.
- Advocate tier is the India volume wedge (price-sensitive); Pro is the senior/trial tier; Judiciary is sold to institutions with on-site installation and support.
- Concrete prices, currencies, and discount policy are **owner decisions** (Q-S4-06). This document does not set prices.

**AC-S4-16.1:** Each edition exposes exactly its entitled surfaces; entitlement checks are offline.  
**AC-S4-16.2:** Pack-subscription expiry never blocks reading or exporting existing work.

---

## 11. Architecture changes [NORMATIVE]

- New contracts (frozen in B-S4-01 before any producer/consumer): `rule_table.schema.json`, `audit_report.schema.json`, `audit_certificate.schema.json`, `external_filing.schema.json`, `witness_matrix.schema.json`, `impeachment_kit.schema.json`, `statute_mapping.schema.json`, `foundation_check.schema.json`, `deadline_record.schema.json`, `treatment_edge.schema.json`, `translation_segment.schema.json`, `bundle_manifest.schema.json`, `issue_matrix.schema.json`, `cause_list_brief.schema.json`, `judgment_scaffold.schema.json`, `rehearsal_timeline.schema.json`, `disclosure_template.schema.json`; plus versioned successors that **coexist** with the frozen originals (which stay byte-identical): `akpkg_v2_1.schema.json` (RULE_TABLE, TREATMENT_EDGES, DISCLOSURE_TEMPLATES, currency date, deltas), `licence_payload_v2.schema.json` (editions, pack subscription), `guardrails_stage4.json` (G7–G10).
- New migrations follow existing numbering (`case_017+`, `app_007+`).
- New sidecar capability only in `docpipe` (Devanagari OCR) and a translation model under the same one-large-model choreography rule (v1.0 §4.2). Translation never runs concurrently with the primary reasoner.
- All Stage 4 generative steps go through the B-S3-14 proposition ledger. No exceptions.
- Hardware tiers unchanged. Lite tier gets F1, F4, F6, F8, F10 (deterministic); model-assisted features show the honesty notice.

### 11.1 Resource budget [NORMATIVE — owner decision 2026-10-08]

Contract: `contracts/stage4_resource_budget.json`.

- **Base install ≤ 4 GiB hard cap, 3 GiB target.** Counts everything needed to run all entitled features: app, docpipe + OCR, default model, speech, embeddings, Stage 4 translation model, packs at install.
- **Tiered model.** A 3B/4B-class open-weight model (commercial-use licence, Q4) is the **default** and ships in the base install. The current 7B-class model becomes an **optional add-on download** for Pro/Judiciary, outside the base cap.
- **Low RAM.** One large model resident at a time (unchanged). Every Stage 4 deterministic engine (rule tables, audit, deadlines, contradiction detection, issue matrix, cause-list brief) runs **with no model loaded**. A lower peak-RSS ceiling for the default model is PROPOSED (4.5 GiB, Q-S4-12).
- Measured baseline on 2026-10-08: 4.96 GiB installed, of which 4.38 GiB is the 7B model — over the cap until B-S4-00 lands.

### 11.2 Deterministic-first intelligence [NORMATIVE — design principle]

CaseForge's intelligence comes primarily from **algorithms over verified data**, not from model size. A small model is acceptable because it only *proposes*; the host *decides*.

| Real-world job | Primary mechanism (no model) | Model role |
|---|---|---|
| Citation/quote audit | Pattern extraction from pack rules + exact-substring match + pinpoint lineage | None in classification |
| Contradictions / impeachment | Normalised date/amount/name/place comparison, entity resolution, statement alignment | Propose semantic conflicts; host verifies exact quotes |
| Code bridge, deadlines, foundations | Signed rule tables + calendar arithmetic + graph queries | None |
| Weak links / Missing Links | Argument Integrity Engine (graph scoring) | None |
| Retrieval | Hybrid BM25 + small embedding model (B-S3-11) | None |
| Issue matrix, unanswered arguments | Graph traversal over party-owned assertions | Optional summarising, record-only, verified |
| Mock Court, drafting help | Retrieval-constrained generation | Generate; every proposition through B-S3-14 |

Open-source components are preferred, subject to licence checks for commercial offline redistribution (Tesseract is already bundled; Devanagari support uses its `hin` traineddata; translation and default LLM are chosen by bake-off).

---

## 12. Guardrails G7–G10 [NORMATIVE]

| ID | Rule | Action |
|---|---|---|
| **G7** | Record-only (Chambers): no extra-record facts | Block rendering; log |
| **G8** | Decision-free zone: never generate analysis, findings, holdings, or operative orders for a judge | Refuse; no model call |
| **G9** | Deadlines/limitation are computations to verify, never advice | Mandatory VERIFY label + rule + trigger locator |
| **G10** | Translation never becomes a verified quotation or verified critical field without counsel confirmation | Label + block GREEN |

G1–G6 unchanged. **AC-S4-RT-RED:** Stage 4 red-team suite (≥ PROPOSED 300 prompts across lawyer + Chambers) → zero G1, G2, G3, G7, G8 violations.

---

## 13. Golden data [NORMATIVE]

New sets (authored in B-S4-02; synthetic, public-domain, or rights-cleared only; never client matters):

`G-AUDIT`, `G-OPPOSING`, `G-IMPEACH`, `G-CODEBRIDGE`, `G-ADMISS`, `G-DEADLINE`, `G-TREATMENT`, `G-INDIC`, `G-BUNDLE`, `G-CHAMBERS`, `G-CAUSELIST`, `G-REDTEAM-S4`.

Every legal answer in `G-CODEBRIDGE`, `G-ADMISS`, `G-DEADLINE` requires a **named legal editor** sign-off per jurisdiction. Without it the related AC is `BLOCKED`, never `PASS`.

---

## 14. Release plan [STRATEGY]

| Checkpoint | Batches | Possible release |
|---|---|---|
| **Lean runtime** | B-S4-00 (after B-S4-01) | Base install under the 4 GiB cap with 3B/4B default model |
| **Wave A — Verified Advocacy core** | B-S4-01 … B-S4-08 | Optional `v3.1.0` Advocate/Pro release (F1, F2, F3) — owner decision Q-S4-08 |
| **Wave B — Law currency, India language, collaboration** | B-S4-09 … B-S4-16 | Internal checkpoint |
| **Wave C — Chambers Edition** | B-S4-17 … B-S4-21 | Judiciary pilot with one court/chamber |
| **Ship** | B-S4-22 … B-S4-23 | `v4.0.0` |

---

## 15. Non-goals (Stage 4)

Unchanged from v1.0 §20, plus:
- No online court-portal, e-filing, or docket scraping integration.
- No cloud sync, relay server, or shared database for bundles.
- No "good law" or "valid law" assertion.
- No ranking or scoring of parties in Chambers Edition.
- No automated sentencing, bail, or relief recommendation for any user.
- Indian languages beyond Hindi until named by owner.
- No guarantees of compliance with any court's AI-disclosure rules — templates only.

---

## 16. Open questions (resolve before the named batch)

| ID | Question | Needed by |
|---|---|---|
| Q-S4-01 | Freeze all **PROPOSED** thresholds in this PRD (or replace) | B-S4-01 |
| Q-S4-02 | Named legal editors for India and US rule tables and goldens | B-S4-02 |
| Q-S4-03 | Indian scripts after Hindi, and order | Post-v4.0 |
| Q-S4-04 | Devanagari OCR and translation model choice (bake-off; licence must permit offline commercial use) | B-S4-14 |
| Q-S4-05 | Official pack delta cadence (monthly proposed) and delivery channel | B-S4-13 |
| Q-S4-06 | Prices, currencies, discounts per edition and per pack subscription | B-S4-22 |
| Q-S4-07 | Seat-transfer limit per year | B-S4-22 |
| Q-S4-08 | Ship Wave A early as `v3.1.0`? | After B-S4-08 |
| Q-S4-09 | Audit Certificate signing key model: per-install key only, or institution-issued key for firms/courts | B-S4-05 |
| Q-S4-10 | Which court(s) for the Chambers pilot, and their AI-use policy constraints | B-S4-17 |
| Q-S4-11 | Initial US states for `PROCEDURAL_DEADLINE` / `COURT_CALENDAR` tables | B-S4-11 |
| Q-S4-12 | Approve per-component disk allocations and default-model RAM ceiling in `stage4_resource_budget.json` | B-S4-00 |

---

**Document v4.0 (DRAFT) — CaseForge Stage 4.**  
Every AC is an executable gate. Every legal datum comes from signed, sourced, editor-approved data. Never invent.

*End of Product Requirement Document*
