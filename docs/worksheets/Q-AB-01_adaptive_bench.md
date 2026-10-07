# Q-AB-01 — Per-Case Adaptive Bench (AC-10.1 & AC-10.2)

**Status:** DECIDED (2026-08-08)  
**Unblocks:** B-S2-03  
**Related:** PRD §10 · §20 non-goals · AC-2.1 · Experience `bench_setting`

---

## 1. Owner decision

| Field | Answer |
|-------|--------|
| Owner | CaseForge Product Owner |
| Date | 2026-08-08 |
| Decision | **DECIDED** |
| Pipeline id | `caseforge.adaptive_bench.v1` |

---

## 2. Six generic archetypes (never named judges)

| id | Label | Default posture |
|----|--------|-----------------|
| `procedure_first` | Procedure-first | Maintainability, notice, jurisdiction before merits |
| `evidence_heavy` | Evidence-heavy | Exhibit chain, contemporaneous record, burden of proof |
| `textualist` | Textualist / statutory | Elements checklist, plain meaning, authority on graph |
| `relief_focused` | Relief-focused | Precise prayer, adequacy of damages, proportionality |
| `skeptical_probe` | Skeptical probe | Stress gaps, concessions, alternative theories |
| `settlement_lens` | Settlement lens | Narrow issues, interim compromise signals, costs risk |

**Forbidden:** Real-person judge names, court-assignment databases, “profile this judge”.

---

## 3. User dial-in sliders (0–100 integers)

| Slider | Effect |
|--------|--------|
| `tempo` | Question / turn pace (higher = more questions per turn) |
| `formality` | Register of address and structure |
| `opposition_aggression` | How hard adversary/bench presses (PRD “opposition aggression control”) |
| `technicality` | Depth of statute/element demand |
| `interruption` | How often bench cuts in / redirects |

Defaults: all **50** unless archetype overrides baseline offsets.

---

## 4. Case-order signals (this case only)

Extracted **only** from `case_graph` (+ optional argument_tree):

| Signal | Source |
|--------|--------|
| `jurisdiction` | case_graph.jurisdiction |
| `relief_count` | len(prayer_relief) |
| `open_gap_count` | open gaps |
| `evidence_count` | len(evidence) |
| `statute_count` | len(statutes) |
| `chronology_density` | len(chronology) |
| `has_procedural_posture` | procedural_posture non-null |
| `urgency_hint` | true if relief/posture text matches interim/bail/injunction keywords |
| `suggested_archetype` | deterministic map from signals |

No external order banks. No other cases’ orders.

---

## 5. AC procedures (freeze Q-EVAL-01 for AC-10)

### AC-10.1

1. Exactly **six** archetypes as above; ids stable.  
2. `named_judge_profiles === false` in policy.  
3. Any API field `judge_name` / `named_judge_id` → hard error `CF_BENCH_NO_NAMED_JUDGE`.  
4. Rendered bench brief must not contain personal judicial honorific + proper name patterns supplied as “profile”.  
5. Behaviour controller is pure code; no judge DB tables.

### AC-10.2

1. Case-order signals computed solely from this case’s `case_graph`.  
2. Changing `opposition_aggression` (or other sliders) changes the **behaviour brief** / turn prompt material deterministically.  
3. Same graph + same sliders + same archetype → same signal fingerprint (hash).  
4. Different graphs can yield different `suggested_archetype` / signal vectors.  
5. Bench settings may be exported as Experience `bench_setting` later; this batch persists session profile only.

---

## 6. Integration

- Mock Court `role=bench` injects Adaptive Bench brief into GENERATE.  
- UI: archetype cards + five sliders on Mock Court.  
- Stage 2 still consumes no raw documents.
