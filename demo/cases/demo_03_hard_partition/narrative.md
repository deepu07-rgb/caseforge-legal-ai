# Demo 03 — Hard: partition + oral gift + limitation (India)

**Status:** `DEMO_SYNTHETIC` · **Not law** · **Not GOLDEN**

## Story

Kavita and Ankit seek partition of a house standing in late Om Prakash’s name. Devendra holds a registered 2008 gift and long mutation. Plaintiffs lean on a **1998 oral family settlement** with no writing. Defendants counterclaim for exclusive title. Dates and a phantom 2019 instrument conflict.

## Why this is “hard”

| Limb | Bottleneck links | VI (approx) |
|------|------------------|------------:|
| L1 HUF / partition | Law 40, Exposure 35 | ~65 |
| L2 Oral settlement | Evidence 22, Law 28 | **~78** (weakest) |
| L3 Void the gift | Evidence 30 | ~70 |

**Integrity lesson:** The story people *want* to tell (oral settlement) is the one the scorer should punish hardest.

## Where the argument needs more strength (priority order)

1. **Abandon or fortify L2** — without contemporaneous proof, leading with oral settlement is a VI disaster.
2. **Forensics on 2008 deed (L3)** — registration beats suspicion.
3. **Limitation & counterclaim war-game (Exposure)** — 1998–2023 silence is the defendant’s best friend.
4. **Prove HUF nucleus (L1)** — 1978 deed alone does not make HUF.

## How to test

```powershell
python demo/tools/score_argument_tree.py --case demo_03_hard_partition --verbose
```

Expect:
- Weakest limb **L2**
- VI **78**
- Many Missing Links below threshold 60
- Pedagogy text pointing to Evidence + Law collapse
