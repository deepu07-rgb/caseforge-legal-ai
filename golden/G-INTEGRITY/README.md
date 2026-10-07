# G-INTEGRITY — Argument Integrity Engine fixtures (B-S2-01)

GOLDEN fixtures for **AC-8.1 / AC-8.2 / AC-8.3**.

| Fixture | Purpose |
|---------|---------|
| `planted_gap_evidence` | Planted Evidence gap must appear in **top-2** Missing Links (AC-8.1) |
| `healthy_limb` | Control: high links, empty Missing Links, low VI |

**Scoring:** pure deterministic `caseforge.integrity.v1` — no LLM.

```powershell
npm run test:integrity
```
