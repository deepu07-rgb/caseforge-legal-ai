# G-DOCS MVP-80 plan (path to AC-6.1)

**Goal:** Get a **measurement signal** before commissioning all 500 ship pages.  
**Not a ship substitute:** AC-6.1 still requires **500 GOLDEN** pages at ≥ 92%.  
**Status:** Track B in progress — **80 DRAFT** synthetic pages scaffolded (`page_0001`–`page_0080`); **0 GOLDEN** until dual human lock.  
**Operational runbook:** [`../../docs/MVP80_LABELING_RUNBOOK.md`](../../docs/MVP80_LABELING_RUNBOOK.md)  
**Status board:** [`mvp80/STATUS.md`](./mvp80/STATUS.md)  
**Progress:** `npm run gdocs:progress`

---

## 1. Why 80 first

| Benefit | Detail |
|---------|--------|
| Rubric stress-test | Dual-review agreement on real pages |
| Bake-off set | Same 80 pages score all vision candidates fairly |
| Cost control | Fix labeling mistakes before 500 |
| Early FAIL | Discover heuristic / model weakness on typed vs hand |

**Rule:** MVP-80 pages may be locked as `GOLDEN` and **count toward** the 500. They are not a separate fake metric.

---

## 2. Class quotas (80 pages)

| Class | Count | Notes |
|-------|------:|-------|
| `typed_english` | 32 | Clean scans + some skew/compression |
| `clean_handwriting` | 12 | Neat hand |
| `messy_handwriting` | 10 | Hard hand |
| `mixed` | 10 | Typed + meaningful hand |
| `other` | 10 | Non-English or non-Latin-dominant |
| `non_text` | 6 | Blank / logo / capture fail |
| **Total** | **80** | |

Tolerance: ±2 per class if material is scarce; do not zero out a class.

---

## 3. Collection checklist

- [ ] Rights-cleared source list (URLs / internal pack IDs)
- [ ] Redaction policy signed if any client-adjacent material
- [ ] Two named labelers + one adjudicator
- [ ] Rubric walkthrough complete ([`LABELING_RUBRIC.md`](./LABELING_RUBRIC.md))
- [ ] Folder of page images or single-page PDFs ready
- [ ] Scaffold DRAFT fixtures: `python eval/tools/scaffold_gdocs_fixture.py ...`
- [ ] Blind dual labels recorded
- [ ] Conflicts adjudicated
- [ ] Lock GOLDEN + content hashes
- [ ] Run bake-off on MVP-80: `python eval/bakeoff/run_bakeoff.py`
- [ ] Run ship scorer (will FAIL until 500, but reports accuracy): `python eval/checks/ac_6_1_triage_score.py`

---

## 4. Suggested two-week schedule

| Day | Activity | Owner |
|-----|----------|--------|
| 1 | Rubric training (1–2 h) + 10 calibration pages | Labelers |
| 2–3 | Collect / redaction / file staging | Content owner |
| 4–6 | Labeler A: 40 pages/day | Labeler A |
| 4–6 | Labeler B: blind parallel | Labeler B |
| 7 | Adjudicate disagreements; refine rubric if needed | Adjudicator |
| 8 | Lock GOLDEN MVP-80; hash files | Content owner |
| 9–10 | Vision bake-off runs on MVP-80 | Engineering |
| 11 | Choose provisional winner or declare bake-off extend | Product + eng |
| 12–14 | Commission next 120 pages toward 500 | Content |

---

## 5. Success criteria for MVP-80 (process, not AC-6.1)

| Criterion | Pass |
|-----------|------|
| 80 pages locked GOLDEN | Yes |
| Dual-label raw agreement | ≥ 90% |
| All 6 classes present | Yes |
| Bake-off harness runs without error | Yes |
| Heuristic accuracy logged (informational) | Recorded, not used as ship PASS |

**AC-6.1 remains FAIL or SKIP until `pages >= 500` and accuracy ≥ 92%.**

---

## 6. After MVP-80

1. Scale labeling to 500 using same rubric version.  
2. Freeze vision model (Q-VISION-01) when bake-off winner is stable on ≥ 80 and re-checked on ≥ 200.  
3. Re-run full AC-6.1.  
4. Only on **PASS** → B-S1-06.
