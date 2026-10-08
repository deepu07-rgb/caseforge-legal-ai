# B-S4-28 — Step-by-step reasoning with best-of-n verification

A 7B local model reasons less well than a large cloud model, and it can produce a confident step that is wrong or cites
text that is not in the source. What it cannot do is fool the host verifier: every factual and legal step must copy an
exact excerpt from a supplied source, and every inference must name the earlier steps it rests on
(`proposition::verify`, B-S3-14). B-S4-28 turns that mechanical check into quality: more attempts, keep the best-checked
one.

## How it works

1. **Ask for a ledger, not prose.** The model returns numbered propositions: facts (copied from the matter), rules
   (copied from an authority in the counsel-confirmed working set), inferences that name their bases, and a conclusion.
   A rule it cannot find becomes an explicit gap instead of a statement.
2. **Sample.** Up to 3 (Careful) or 5 (Thorough) candidates. Variant 0 uses the contract's settings exactly; variants 1-7
   use a different seed and a higher temperature (`VARIANT_TEMPERATURE`), so they differ yet stay reproducible and are
   recorded in the provenance.
3. **Verify every candidate with the host, not the model.** A candidate scores 3 per verified step, 1 per labelled
   inference, -6 per rejected step, -8 if the conclusion is rejected. Agreement between the candidates' conclusions adds
   up to 4 points as a tie-breaker; it never overrides verification.
4. **Stop early** (Careful): the first candidate with no rejected step ends the run. **Time budget:** a further sample is
   not started if it would probably not finish inside the allowance (300 s / 900 s / 1,800 s).
5. **One targeted repair** of the best candidate if steps were rejected: the model is shown which steps failed and why and
   must either copy a verbatim excerpt or turn the step into a gap. Kept only if it scores strictly higher.
6. **Say what it is.** `VERIFIED` only if the host verified every step (a fabricated excerpt can never reach it);
   `PARTIAL` shows gaps; `NO_VERIFIED_ANSWER` tells the lawyer not to rely on anything. Output also passes the G1-G6
   guardrails, and the working-set authorities used are recorded as a `REASONING` output.

## Where

- `src-tauri/src/reasoning.rs` — the engine (no model call; driven by a generator closure) and the prompts.
- `lib.rs` `reasoning_run(case_id, question, depth)` — retrieval over the working set (as Mock Court), model calls, guards.
- `sidecars/infer_llm/infer_llm.py` — `variant` parameter; `infer_llm::LlmChatRequest.variant`.
- `src/components/ReasoningPanel.tsx` (Drafting view) — question, depth, checked steps with their source excerpts, the
  attempts table.

## Evidence

- Golden **G-REASON** (pinned before the engine): 9 scripted scenarios — first candidate verified (early stop); a
  fabricated first candidate and a good second; thorough run with agreement; partial beats fabricated but is not called
  verified; all fabricated; repair fixes it; errors/empty candidates skipped; quick never repairs; time budget.
- `eval/checks/reasoning_b_s4_28.py`: fixture pin, the Rust tests from the CI log, sidecar variants (variant 0 equals the
  contract; 8 distinct, reproducible, bounded), static properties (engine never calls a model; scores come from the
  verifier; guards present), UI vocabulary.
- **AC-S4-28.1 PASS** (when CI log supplied): a verified answer means every step was verified; the best-verified of
  several is kept.
- **AC-S4-28.2 BLOCKED**: whether sampling raises the share of verified answers *for the real 7B model* has not been
  measured — CI has no model. It needs a run on the owner's machine over held-out questions, and a threshold the owner
  approves (Q-S4-01). The engine's guarantees are proven with scripted candidates, not with the model.

## Limits

- It checks that steps are *grounded*, not that the reasoning is *correct*: an inference between verified steps can
  still be a poor inference. Inferences are labelled as such for counsel to judge.
- Each attempt costs a full generation (tens of seconds on a CPU), so Careful can take several minutes.
- Authority and matter context must fit the model's limits (as for Mock Court); a larger working set is rejected with a
  clear error rather than silently cut.
