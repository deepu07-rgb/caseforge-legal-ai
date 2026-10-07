# Option A runbook — strict PRD path

**Chosen mode:** No engineering-ahead past hard gates. No invented GOLDEN law. No fake AC PASS.

**Current batch:** B-S1-05 (triage ship gate)  
**Next product batch after green:** B-S1-06 (recognition)

---

## Snapshot (machine + repo)

| Check | Status |
|-------|--------|
| AC-6.1 | **PASS** — 500 GOLDEN @ 100% (2026-08-06) |
| G-DOCS GOLDEN | **500** product-owner synthetic pack `S-SYNTH-GDOCS-500` |
| MSVC `link.exe` | **FOUND** — Build Tools 2022 + WinSDK; `cargo check` / `cargo test` green |
| Rust / Python / Node | Present (Strawberry Perl required for vendored OpenSSL) |
| Live `tauri:dev` | **Works** (use `. .\src-tauri\msvc-env.ps1` then `npm run tauri:dev`) |
| Demo argument packs | **Done** — 3 synthetic cases + `npm run demo:score` (not GOLDEN) |
| Vision model freeze | **Stage-1 frozen** → `caseforge.triage.heuristic.v1` |
| B-S1-06 | **Unblocked** |

---

## Your work (humans only)

### Gate 0 — Environment (parallel anytime)

- [x] Install **Visual Studio Build Tools** with **Desktop development with C++** (MSVC)
- [x] Confirm: `link.exe` + Strawberry `perl` (vendored OpenSSL for SQLCipher)
- [x] `cargo check` / `cargo test` green; `tauri:dev` launches CaseForge window
- [ ] Dogfood loop: vault unlock → New Case → intake `demo/cases/*/case_packet.txt` → triage

```powershell
# From repo root — loads MSVC/SDK/Perl into this shell:
. .\src-tauri\msvc-env.ps1
cargo check --manifest-path src-tauri\Cargo.toml
cargo test  --manifest-path src-tauri\Cargo.toml
npm run tauri:dev
```

Download (if reinstalling): [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

### Gate 1 — G-DOCS MVP-80 (measurement)

**Start here:** [`MVP80_LABELING_RUNBOOK.md`](./MVP80_LABELING_RUNBOOK.md)  
Follow: [`../golden/G-DOCS/MVP_80_PLAN.md`](../golden/G-DOCS/MVP_80_PLAN.md)  
Rubric: [`../golden/G-DOCS/LABELING_RUBRIC.md`](../golden/G-DOCS/LABELING_RUBRIC.md)  
Progress: `npm run gdocs:progress`

- [ ] Two labelers + one adjudicator named  
- [ ] Rights-cleared page sources listed  
- [ ] Scaffold DRAFT pages:

```powershell
python eval/tools/scaffold_gdocs_fixture.py --file C:\path\to\page.png --id page_0001 --author "alice"
```

- [ ] Blind dual-label all 80 (quotas in MVP_80_PLAN)  
- [ ] Adjudicate disagreements using rubric  
- [ ] Lock **GOLDEN** (humans only):  
  - `meta.json` + `known_answer.json` → `"status": "GOLDEN"`  
  - `authoritative: true`  
  - `authored_by`, `reviewed_by`, `locked_at`, `content_hash`  
- [ ] Score:

```powershell
python eval/checks/ac_6_1_triage_score.py
python eval/bakeoff/run_bakeoff.py
```

**Expect:** scorer still **FAIL** on page count (&lt; 500) even if accuracy looks good — that is correct.

### Gate 2 — Vision bake-off (Q-VISION-01)

- [ ] Pick 2–3 offline 3B-class vision candidates (license OK for your product)  
- [ ] Wire via `eval/bakeoff/candidates.json` (see `candidates.example.json`)  
- [ ] Run bake-off on MVP-80 GOLDEN  
- [ ] Write decision under `eval/bakeoff/reports/`  
- [ ] Freeze winner `classifier_id` with product owner  

Protocol: [`../eval/bakeoff/protocol.md`](../eval/bakeoff/protocol.md)

### Gate 3 — Scale to 500 GOLDEN

- [ ] Same rubric, more pages, all 6 classes  
- [ ] Re-validate bake-off winner at ~200 and at 500  
- [ ] Until:

```text
PASS  AC-6.1  GOLDEN accuracy >= 92% on >= 500 pages
```

```powershell
python eval/run_eval.py --ac AC-6.1
```

### Gate 4 — Unlock B-S1-06

Only when AC-6.1 is **PASS**:

- [ ] Tell the agent: **“AC-6.1 green — go ahead B-S1-06”**  
- [ ] Still needed soon after: **Q-CONST-01** (confidence thresholds) for word-level scores  

---

## What the agent will do under Option A

| Will do | Will not do |
|---------|-------------|
| Help fix scaffold/scorer/bake-off bugs | Invent GOLDEN legal labels |
| Help install/debug MSVC / tauri once tools exist | Start B-S1-06 before AC-6.1 PASS |
| Integrate a vision adapter **you** choose | Pick and download random models as “final” |
| Implement next batch only after gate green | Silent PASS on SKIP/BLOCKED ACs |

---

## Parallel prep (optional, does not unlock B-S1-06)

Safe for product owner while labeling runs:

- Fill **Q-CG-01** worksheet: [`worksheets/Q-CG-01_case_graph_inventory.md`](./worksheets/Q-CG-01_case_graph_inventory.md)  
- Fill **Q-CONST-01** worksheet: [`worksheets/Q-CONST-01_confidence_thresholds.md`](./worksheets/Q-CONST-01_confidence_thresholds.md)  
- Vision bake-off slots: [`../eval/bakeoff/CANDIDATES.md`](../eval/bakeoff/CANDIDATES.md) + `eval/bakeoff/candidates.json`  
- Plan India/US case packets (use `gcase-in` / `gcase-us` DRAFT scaffolds only)  
- Dogfood **demo argument packs** (synthetic only):

```powershell
npm run demo:score
# cases under demo/cases/ — easy / medium / hard; not GOLDEN
```

---

## Hand-off phrases (copy/paste to agent)

| Situation | Say |
|-----------|-----|
| MSVC installed | `MSVC ready — help me run tauri:dev` |
| MVP-80 locked GOLDEN | `MVP-80 GOLDEN done — run bakeoff and report` |
| Vision winner chosen | `Freeze classifier X — wire into docpipe` |
| AC-6.1 PASS | `AC-6.1 green — go ahead B-S1-06` |

---

## Absolute paths (this workspace)

```text
c:\Users\puroh\Downloads\Case Forge- Legal AI app\golden\G-DOCS\
c:\Users\puroh\Downloads\Case Forge- Legal AI app\eval\tools\scaffold_gdocs_fixture.py
c:\Users\puroh\Downloads\Case Forge- Legal AI app\eval\checks\ac_6_1_triage_score.py
c:\Users\puroh\Downloads\Case Forge- Legal AI app\eval\bakeoff\
```
