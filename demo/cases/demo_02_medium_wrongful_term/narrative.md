# Demo 02 — Medium: wrongful termination + bonus (US)

**Status:** `DEMO_SYNTHETIC` · **Not law** · **Not GOLDEN**

## Story

Avery Cole was fired in a “reorganization” after a positive review that mentioned a full bonus if Finance confirmed targets. HR says the bonus was discretionary. Cole also alleges age discrimination (54 vs retained peer 31) but has almost no animus evidence yet.

## Why this is “medium”

Two limbs:

| Limb | Bottleneck | VI (approx) |
|------|------------|------------:|
| L1 Bonus | Evidence 42, Law 55 | ~58 |
| L2 Age discrimination | Evidence 38 | ~62 |

**Weakest limb = L2 (VI 62).** Missing Links should flag **Evidence** (and Law on L1).

## Where the argument needs more strength

1. **Evidence (critical):** Finance target confirmation, bonus metrics, org chart / backfill, bias remarks.
2. **Law (material on L1):** Pure “discretionary” language fights wage theory — need authority map or alternative theory.
3. **Do not oversell L2** until discovery fills the file.

## How to test

1. Create **United States** case.
2. Ingest `case_packet.txt`.
3. `python demo/tools/score_argument_tree.py --case demo_02_medium_wrongful_term`
4. Confirm L2 is weakest and Evidence appears in Missing Links.
