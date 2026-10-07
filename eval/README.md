# CaseForge `/eval` — acceptance criteria harness

**Batch:** B-S0-05  
**PRD:** §17 — every AC maps to an executable test; DONE only when listed tests pass.

---

## Quick start

```powershell
# From repo root
python eval/run_eval.py

# Or
.\eval\run_eval.ps1
```

Options:

```powershell
python eval/run_eval.py --ac AC-16.1
python eval/run_eval.py --json eval/reports/custom.json
python eval/run_eval.py --fail-on-skip    # ship-strict: SKIP/BLOCKED also fail CI
```

Default exit code: **1 if any FAIL**, else 0 (SKIP/BLOCKED do not fail the process unless `--fail-on-skip`).

---

## Result meanings

| Result | Meaning |
|--------|---------|
| **PASS** | Check ran and met criteria. |
| **FAIL** | Check ran and missed criteria, or required artefacts broken. |
| **SKIP** | Cannot score yet (no product, golden not authored, check not wired). **Not a pass.** |
| **BLOCKED** | Open Layer-2 QUESTION prevents an honest implementation/test. |

**Silent PASS is forbidden.** Placeholder golden fixtures never produce PASS for content ACs.

Policy detail: [`../golden/POLICY.md`](../golden/POLICY.md).

---

## Files

| Path | Role |
|------|------|
| [`ac_inventory.json`](./ac_inventory.json) | Every `AC-x.y` from the PRD (expanded ranges) |
| [`run_eval.py`](./run_eval.py) | Harness runner (Python 3 stdlib only) |
| [`run_eval.ps1`](./run_eval.ps1) | Windows launcher |
| `reports/last_report.json` | Last run output (generated; gitignored pattern in root) |

---

## Inventory expansion notes

PRD compact ranges are expanded explicitly:

| PRD text | Inventory ids |
|----------|----------------|
| AC-9.1–9.4 | AC-9.1 WER, AC-9.2 correction persistence, AC-9.3 voice-only rehearsal, AC-9.4 language pinning |
| AC-16.1–16.3 | AC-16.1 schema validation, AC-16.2 unreadable without passphrase, AC-16.3 `.akcase` round-trip |
| AC-10.1 & 10.2 | AC-10.1, AC-10.2 (procedure partial in Layer 1) |
| AC-11.1 & 11.2 | AC-11.1, AC-11.2 (procedure partial in Layer 1) |
| AC-14.1 & 14.2 | AC-14.1, AC-14.2 (procedure partial in Layer 1) |

See `questions` in `ac_inventory.json` (**Q-EVAL-01**).

---

## What runs today (S0)

| Check | Typical result after B-S0-05 |
|-------|------------------------------|
| HARNESS-INVENTORY | PASS if all ACs listed |
| HARNESS-GOLDEN-DISCOVERY | PASS if 8 sets + micro-fixtures exist |
| HARNESS-CONSTANTS-BUDGETS | PASS if PRD numbers match `constants.json` |
| **AC-16.1** | PASS if contract JSON files load |
| Most product ACs | SKIP (`product_not_implemented`) |
| AC-4.1 / AC-7.1 / AC-16.2 / AC-16.3 | BLOCKED when blocked by open QUESTIONS and no product |

As Stage 1 code lands, replace SKIP reasons with real check modules — still never invent PASS.

---

## S0 exit gate

This batch completes Track S0 when:

1. `python eval/run_eval.py` runs successfully.
2. Inventory includes every PRD AC.
3. No silent PASS.
4. AC-16.1 structural load is green (or FAIL for real contract breakage).
