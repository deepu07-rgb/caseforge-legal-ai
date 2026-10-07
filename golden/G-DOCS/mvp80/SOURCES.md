# Rights-cleared / synthetic page sources (MVP-80)

## Active sources

| source_id | description | origin | rights status | redaction | target classes | page count |
|-----------|-------------|---------|---------------|-----------|----------------|------------|
| S-SYNTH-MVP80 | Synthetic pedagogy pages for dual-label practice and optional GOLDEN after human review | `eval/tools/generate_mvp80_synthetic_pages.py` | synthetic_pedagogy (synthetic-but-reviewed allowed by POLICY after dual lock) | n/a | all 6 | 80 |

## Redaction policy

- [x] No live client confidential material in S-SYNTH-MVP80  
- [ ] Content owner reviewed synthetic pack before any GOLDEN lock  
- [ ] Labeler A / B / adjudicator named in `ROSTER.json`  

## Notes

- Track B day-1 pack is **synthetic text pages** so the dual-label pipeline can run immediately.  
- Prefer replacing or supplementing with rights-cleared real scans for vision bake-off strength.  
- Agents must **not** set `status: GOLDEN`. Humans lock after dual review.
