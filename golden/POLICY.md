# Golden dataset policy

**Applies to:** all sets under `/golden`  
**Batch introduced:** B-S0-04  

---

## 1. Authorship process (full golden sets)

Production code must not invent known-answers. Full sets are authored as follows:

1. **Scope** — Pick the target AC(s) and set (e.g. G-DOCS → AC-6.1).
2. **Source material** — Use only rights-cleared or synthetic-but-reviewed material. Never paste confidential client papers into git without redaction policy sign-off.
3. **Known-answer file** — Write `known_answer.json` against frozen Layer 2 schemas when unblocked; until then, document expected fields in `meta.json` notes without claiming schema completeness.
4. **Independent review** — A second reviewer checks labels (triage class, citations, chronology dates, red-team expected refusals).
5. **Lock** — Set `meta.json` → `status: "GOLDEN"`, record `authored_by`, `reviewed_by`, `locked_at`, `content_hash`.
6. **Eval** — Only `GOLDEN` fixtures may cause **FAIL** on mismatch for ship gates.
7. **Change control** — Edits to `GOLDEN` fixtures require bumping `fixture_version` and a short change note in the set README.

### Who may lock GOLDEN

- Product owner or designated legal-content owner for jurisdiction packs.
- Not an automated coding agent acting alone on real legal labels.

### What coding agents may do

- Add `SYNTHETIC_PLACEHOLDER` micro-fixtures for harness plumbing.
- Propose DRAFT labels **only** when marked non-authoritative and not used for ship PASS.
- Never flip `SYNTHETIC_PLACEHOLDER` → `GOLDEN` without human review evidence in `meta.json`.

---

## 2. Harness SKIP vs FAIL vs BLOCKED (for B-S0-05+)

| Result | When to use |
|--------|-------------|
| **PASS** | Implementation exists; fixture `status` is `GOLDEN`; output matches known-answer within AC tolerance from `contracts/constants.json`. |
| **FAIL** | Implementation exists; fixture is `GOLDEN`; output mismatches; **or** a hard invariant is violated (e.g. fake citation not RED). |
| **SKIP** | Feature or fixture not ready: fixture is `SYNTHETIC_PLACEHOLDER` or `DRAFT`; or dependent product module not implemented yet. **SKIP is visible in report; never counted as PASS.** |
| **BLOCKED** | Cannot run because Layer 2 contract field, constant, or open QUESTION blocks the test (e.g. confidence thresholds Q-CONST-01). |

### Fail-open rules

- Empty set directory → harness error at discovery time (misconfiguration), not PASS.
- Set exists but only placeholders → all AC tests for that set **SKIP** with reason `golden_not_authored`.
- Missing `known_answer.json` on a `GOLDEN` fixture → **FAIL** (authoring defect).
- Planted fake citation tests may use synthetic authority strings explicitly labeled `PLANTED_FAKE` — that is not “inventing legal truth”; it is a negative test.

### Silent PASS is forbidden

Any path that cannot evaluate correctness must be SKIP or BLOCKED, never PASS.

---

## 3. Jurisdiction separation

- India chronology / retrieval: **G-CASE-IN**, **G-RETRIEVAL-IN** only.
- United States chronology / retrieval: **G-CASE-US**, **G-RETRIEVAL-US** only.
- Do not mix jurisdiction labels inside one fixture.
- G-DOCS / G-AUDIO / G-REDTEAM / G-SCRIPTS may be jurisdiction-neutral or tagged per fixture in `meta.json`.

---

## 4. Synthetic micro-fixture content rules (B-S0-04)

Allowed in repo now:

- Empty or one-line text stubs tagged `[SYNTHETIC_PLACEHOLDER_NOT_LAW]`.
- Explicitly fake citations for AC-4.3 negative tests (`PLANTED_FAKE`).
- Red-team prompts that probe refusals (not real advice).
- Structural known-answer shells with `"authoritative": false`.

Forbidden:

- Presenting fabricated case law, statutes, or party facts as real.
- Claiming AC percentages against placeholder sets.
- Checking in unredacted client documents.

---

## 5. Open content QUESTIONS

```text
QUESTION
- batch: B-S0-04
- missing: Rights-cleared full golden corpora for G-DOCS (500 pages), G-CASE-IN/US (40-page), G-RETRIEVAL queries, G-AUDIO clips, G-REDTEAM ≥200 prompts
- blocked_artefact: golden/*/fixtures (status GOLDEN)
- options_considered: none
- need_from_owner: Authored + reviewed GOLDEN fixtures per POLICY.md §1 before ship gates that depend on them
HALT (for AC PASS only — scaffold work may continue)
```

---

*End of golden policy*
