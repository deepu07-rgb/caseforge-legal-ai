#!/usr/bin/env python3
"""B-S1-14 Digest surface + deterministic preparation completeness (G6)."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONSTANTS = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "case_graph.schema.json"
SKELETON_PY = ROOT / "sidecars" / "docpipe" / "skeleton.py"
COMP_TS = ROOT / "src" / "lib" / "completeness.ts"
VIEW = ROOT / "src" / "components" / "DigestView.tsx"
APP = ROOT / "src" / "App.tsx"
DOC = ROOT / "docs" / "B-S1-14_digest.md"
SCORE_ID = "caseforge.prep_completeness.v1"
FORMULA = (
    "20 points for each non-empty array among: parties, chronology, "
    "statutes, evidence, prayer_relief (max 100)."
)


def compute_prep(graph: dict) -> dict:
    components = [
        ("parties", 20),
        ("chronology", 20),
        ("statutes", 20),
        ("evidence", 20),
        ("prayer_relief", 20),
    ]
    earned = []
    score = 0
    for field, pts in components:
        n = len(graph.get(field) or [])
        filled = n > 0
        got = pts if filled else 0
        score += got
        earned.append({"id": field, "count": n, "earned": got, "points": pts})
    open_gaps = len([g for g in (graph.get("gaps") or []) if g.get("status") == "open"])
    return {
        "score_id": SCORE_ID,
        "score_0_100": score,
        "open_gaps": open_gaps,
        "components": earned,
        "llm_used": False,
    }


def main() -> int:
    errors: list[str] = []
    for p in (CONSTANTS, SCHEMA, SKELETON_PY, COMP_TS, VIEW, APP, DOC):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # Formula frozen in contracts
    if CONSTANTS.exists():
        data = json.loads(CONSTANTS.read_text(encoding="utf-8-sig"))
        prep = data.get("preparation_completeness") or {}
        if prep.get("status") != "FROZEN":
            errors.append("preparation_completeness not FROZEN in constants.json")
        if prep.get("score_id") != SCORE_ID:
            errors.append(f"score_id must be {SCORE_ID}")
        if "llm_judgment" not in (prep.get("forbidden") or []):
            errors.append("forbidden must include llm_judgment")
        formula = prep.get("formula") or ""
        if "20 points" not in formula and "20pts" not in formula:
            errors.append("formula must document 20-point buckets")
        comps = prep.get("components") or []
        fields = {c.get("graph_field") for c in comps}
        for f in ("parties", "chronology", "statutes", "evidence", "prayer_relief"):
            if f not in fields:
                errors.append(f"preparation_completeness missing component {f}")
        if "G6" not in str(prep.get("prd_refs")) and "preparation" not in (
            prep.get("language_g6") or ""
        ).lower():
            errors.append("G6 preparation language missing in completeness contract")

    for path, needles in (
        (
            COMP_TS,
            [
                "computePrepCompleteness",
                SCORE_ID,
                "llm_used: false",
                "parties",
                "prayer_relief",
                "PREP_G6",
            ],
        ),
        (
            VIEW,
            [
                "CASEBOOK",
                "CHAMBER",
                "DIGEST",
                "prep-completeness",
                "computePrepCompleteness",
                "surface-casebook",
                "G6",
                "Preparation completeness",
                "win prediction",
            ],
        ),
        (
            APP,
            [
                'from "./components/DigestView"',
                "caseGraph={caseGraph}",
                "onRunSkeleton",
            ],
        ),
        (DOC, ["caseforge.prep_completeness.v1", "G6", "CASEBOOK", "deterministic"]),
        (SKELETON_PY, [SCORE_ID, "completeness"]),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n}")

    # No LLM in scoring path (source scan)
    if COMP_TS.exists():
        ct = COMP_TS.read_text(encoding="utf-8")
        if re.search(r"\b(openai|ollama|chat\.completions|llm_score)\b", ct, re.I):
            errors.append("completeness.ts must not call LLM APIs")

    # Unit: empty graph = 0
    empty = compute_prep(
        {
            "parties": [],
            "chronology": [],
            "statutes": [],
            "evidence": [],
            "prayer_relief": [],
            "gaps": [],
        }
    )
    if empty["score_0_100"] != 0:
        errors.append(f"empty graph score expected 0 got {empty['score_0_100']}")

    # Unit: all five buckets filled = 100
    full = compute_prep(
        {
            "parties": [{"x": 1}],
            "chronology": [{"x": 1}],
            "statutes": [{"x": 1}],
            "evidence": [{"x": 1}],
            "prayer_relief": [{"x": 1}],
            "gaps": [{"status": "open"}, {"status": "closed"}],
        }
    )
    if full["score_0_100"] != 100:
        errors.append(f"full graph score expected 100 got {full['score_0_100']}")
    if full["open_gaps"] != 1:
        errors.append(f"open_gaps expected 1 got {full['open_gaps']}")
    if full["llm_used"] is not False:
        errors.append("llm_used must be False")

    # Partial
    partial = compute_prep(
        {
            "parties": [{"x": 1}],
            "chronology": [{"x": 1}],
            "statutes": [],
            "evidence": [],
            "prayer_relief": [],
            "gaps": [],
        }
    )
    if partial["score_0_100"] != 40:
        errors.append(f"partial score expected 40 got {partial['score_0_100']}")

    # Skeleton extractor must match formula (live if possible)
    if SKELETON_PY.exists():
        sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
        try:
            from skeleton import extract_from_pages  # type: ignore

            sample = """
IN THE COURT OF Sample District
Plaintiff: Acme Calibration Pvt. Ltd.
Defendant: Northern Sample Traders
On 12 March 2023, goods were delivered under purchase order.
Section 138 of the Negotiable Instruments Act is invoked.
The plaintiff prays for recovery of INR 1,25,000.
Exhibit A-1 is the delivery challan dated 15 September 2024.
"""
            graph = extract_from_pages(
                case_id="digest-test",
                jurisdiction="India",
                pages=[{"doc_id": "d1", "page_index": 0, "text": sample}],
                recognizer_id="test",
                classifier_id="test",
            )
            live = compute_prep(graph)
            stored = (graph.get("completeness") or {}).get("score_0_100")
            if stored is not None and stored != live["score_0_100"]:
                errors.append(
                    f"skeleton completeness {stored} != recompute {live['score_0_100']}"
                )
            if (graph.get("completeness") or {}).get("score_id") not in (None, SCORE_ID):
                # prefer score_id present
                pass
            if live["score_0_100"] < 60:
                errors.append(
                    f"sample graph prep score unexpectedly low: {live['score_0_100']}"
                )
        except Exception as e:  # noqa: BLE001
            errors.append(f"skeleton live extract failed: {e}")

    # G6: forbidden win-prediction copy in DigestView
    if VIEW.exists():
        vt = VIEW.read_text(encoding="utf-8").lower()
        for bad in ("win probability", "likely to win", "verdict prediction", "chance of success"):
            # "never win prediction" is OK; ban affirmative claims
            if bad in vt and "never" not in vt[max(0, vt.find(bad) - 40) : vt.find(bad) + len(bad) + 10]:
                # allow "never win prediction"
                if "never win" not in vt and "not a prediction" not in vt:
                    errors.append(f"possible G6 violation phrasing: {bad}")

    if "function DigestView" in APP.read_text(encoding="utf-8") if APP.exists() else "":
        # inline duplicate should be gone
        app_t = APP.read_text(encoding="utf-8")
        if app_t.count("function DigestView") > 0 and 'from "./components/DigestView"' in app_t:
            # only import, no local function
            if re.search(r"^function DigestView", app_t, re.M):
                errors.append("App.tsx still has inline DigestView; use components/DigestView.tsx")

    if errors:
        print("FAIL  B-S1-14 digest / preparation completeness")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        f"PASS  B-S1-14 digest surfaces + {SCORE_ID} "
        f"(empty=0 full=100 partial=40; formula frozen; no LLM)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
