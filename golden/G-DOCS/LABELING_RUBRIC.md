# G-DOCS labeling rubric (AC-6.1)

**Purpose:** One shared definition of the six PRD §6.1 classes so two independent labelers agree.  
**Authority:** Product / legal-content owner. Coding agents do not lock GOLDEN.  
**Related:** [`AUTHORING.md`](./AUTHORING.md) · [`../POLICY.md`](../POLICY.md)

---

## 1. Unit of labeling

| Rule | Detail |
|------|--------|
| Unit | **One page** = one label |
| Multi-page PDF | Label **each page** separately (`page_index` 0..n-1) |
| Crop / photo of one page | Still one page |
| Double-page spread photo | Prefer split into two fixtures; if not split, label dominant class and note in description |

Do **not** label “the whole file” as one class when pages differ.

---

## 2. Class definitions (normative for G-DOCS)

### `typed_english`

**Primary signal:** Machine-printed (or clean digital) **Latin-script English** text is the main content.

Include:

- Court orders, pleadings, contracts, emails printed to PDF, clean OCR-ready scans of typed pages
- Tables and letterhead if body is typed English
- Light stamps / signatures that do **not** dominate the page

Exclude → use another class if:

- Handwriting is a substantial share of meaning → `mixed` or handwriting classes
- Non-English script dominates → `other`
- Almost no readable text → `non_text`

### `other`

**Primary signal:** Content is **not** primarily typed English (language, script, or layout).

Include:

- Typed non-English (Hindi, Tamil, Spanish, etc.) as main body
- Mixed scripts where English is not dominant
- Forms in another language

Exclude:

- English handwriting → handwriting classes  
- Blank / pure image → `non_text`

### `mixed`

**Primary signal:** **Both** machine-typed **and** handwriting (or heavy manual markup) carry meaning on the same page.

Include:

- Typed order with handwritten annotations, interlineations, “OK” in margin
- Form with typed boilerplate + filled handwriting fields that matter
- Typed page with substantial strikeouts / counsel notes

Exclude:

- Typed page with only a signature block and date → still `typed_english` (signature alone is not enough for `mixed`)
- Pure handwriting → handwriting classes

### `clean_handwriting`

**Primary signal:** Page is **mostly handwritten**, and a careful human reader can follow lines without special effort.

Include:

- Neat diary notes, clean witness statements in pen, tidy counsel notes
- Printed form that is blank except for neat handwriting filling most of the page (handwriting is primary)

Exclude:

- Heavy scribble / unreadable → `messy_handwriting`
- Typed + hand → `mixed`

### `messy_handwriting`

**Primary signal:** Mostly handwritten, but **hard to read** (rushed, overlapping, faint, cramped, multi-writer chaos).

Include:

- Marginalia-dense pages, rushed courtroom notes, low-contrast pencil
- Pages where a human needs multiple passes to recover meaning

Exclude:

- Neat hand → `clean_handwriting`
- Mostly typed → `typed_english` or `mixed`

### `non_text`

**Primary signal:** Page is **not primarily linguistic content**.

Include:

- Blank pages, pure logos, photos of people/places without readable text
- Dense stamps/seals with no body text
- Pure diagrams / maps without captions that form sentences
- Severe noise / black page / capture failure with no recoverable text

Exclude:

- Photo of a document that is still readable typed text → treat as that text class

---

## 3. Decision tree (use in order)

```text
1. Is recoverable linguistic content essentially absent?
   → non_text
2. Is the page mostly handwriting?
   → clean_handwriting if legible with normal effort
   → messy_handwriting if not
3. Is there both typed body AND meaningful handwriting/markup?
   → mixed
4. Is primary body typed Latin English?
   → typed_english
5. Else
   → other
```

When torn between two classes, prefer the class that best predicts **downstream pipeline path** (OCR vs HTR vs skip).

---

## 4. Dual-review protocol

| Step | Who | Action |
|------|-----|--------|
| A | Labeler 1 | Assign class; write one-line justification in `description` or review log |
| B | Labeler 2 | Labels **blind** (does not see A’s class) |
| C | Compare | Agreement → candidate for GOLDEN |
| D | Conflict | Discuss using this rubric; if still split, escalate to product owner |
| E | Lock | Only after agreement: set `status: GOLDEN`, `authored_by`, `reviewed_by`, `locked_at`, `content_hash` |

### Agreement metrics (track in review log)

| Metric | Target for MVP (80 pages) | Target for ship (500) |
|--------|---------------------------|------------------------|
| Dual-label agreement | ≥ 90% before lock | ≥ 92% |
| Adjudicated residual | ≤ 10% | ≤ 8% |

Disagreements are **data quality bugs**, not model bugs—fix the rubric first.

---

## 5. Rights and privacy

Allowed sources:

- Public domain / openly licensed court PDFs
- Firm-owned synthetic packs created for training
- Redacted client material **only** with written policy sign-off

Forbidden in git:

- Unredacted confidential client papers
- GOLDEN status without dual review

---

## 6. Edge-case bank (train labelers on these)

| Edge case | Default class | Rationale |
|-----------|---------------|-----------|
| Typed English + single signature | `typed_english` | Signature not “mixed” alone |
| Typed English + 3 lines of counsel note | `mixed` if notes carry meaning; else typed | Meaning threshold |
| Hindi typed + English header | `other` if body non-English | Body dominates |
| Blank with faint stamp | `non_text` | No linguistic body |
| Neat hand on lined paper | `clean_handwriting` | |
| Coffee-stained rushed hand | `messy_handwriting` | |
| Photo of typed order, skewed | `typed_english` | Content is typed English |
| Half typed / half hand columns | `mixed` | |

---

## 7. What coding agents may do

| Allowed | Forbidden |
|---------|-----------|
| Scaffold DRAFT / SYNTHETIC fixtures | Set `GOLDEN` without human review |
| Score GOLDEN via eval harness | Invent case facts as gold truth |
| Propose rubric clarifications | Change class enum outside PRD |

---

*Rubric v1.0 — B-S1-05 path to AC-6.1*
