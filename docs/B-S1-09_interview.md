# B-S1-09 — Guided Intake Interview

**Status:** DONE (2026-08-07)

## Goal

Targeted questions from **skeleton gaps only**. Answers become `SOURCED_BY_COUNSEL` facts with counsel provenance. No legal strategy prompts.

## AC gates

| AC | Meaning | Implementation |
|----|---------|----------------|
| **AC-6.5** | ≤ 12 questions complete skeleton to ≥ 90% | `guided_interview_max_questions` / `guided_interview_min_skeleton_completion_percent` in constants; dry-run in eval |
| **AC-6.6** | Resumable, skippable, re-openable | `pause` / `resume` / `skip` / re-open after completed |

## Vault

- Migration `case_008_guided_interview.sql`
- Tables: `interview_session`, `interview_question`
- Host commands: `interview_open`, `interview_get`, `interview_pause`, `interview_resume`, `interview_answer`, `interview_skip`

## UI

Case file → **Interview** tab (`src/components/InterviewView.tsx`)

## Eval

```powershell
npm run test:interview
python eval/run_eval.py --ac AC-6.5
python eval/run_eval.py --ac AC-6.6
```

## Out of scope

- Voice dictation (B-S1-16)
- Open-ended strategy chat
