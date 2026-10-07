# B-S1-08 — Side-by-side verification review

**Status:** DONE (2026-08-06)

## AC gates

| AC | Meaning | Implementation |
|----|---------|----------------|
| **AC-6.3** | Click fact → source page + span | Review list → source pane with `<mark class="source-span">` |
| **AC-6.4** | ≤ 2 interactions to correct | Select (1) + Save (2); host rejects `interaction_count > 2` |

## Surfaces

- Case file → **Review** tab (`src/components/ReviewView.tsx`)
- Low-confidence filter (graph items &lt; `item_highlight_below`, words flagged low)
- Honesty copy from Q-CONST-01

## Host / vault

| Command | Role |
|---------|------|
| `review_correct_graph_item` | Patch case_graph field; `origin=USER_EDIT`, conf=1.0 |
| `review_correct_word` | Replace word on page text layer |
| `review_corrections_list` | Thin Experience Store hook |

Migration: `case_007_review_corrections.sql` → `experience_corrections`.

## Eval

```powershell
npm run test:review
python eval/run_eval.py --ac AC-6.3
python eval/run_eval.py --ac AC-6.4
```

## Out of scope (later)

- Full Experience Store retrieval (B-S1-17)
- Browser E2E automation beyond structural wiring
