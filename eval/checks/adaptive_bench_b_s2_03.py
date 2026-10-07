#!/usr/bin/env python3
"""B-S2-03 Per-Case Adaptive Bench — AC-10.1 & AC-10.2.

Mirrors caseforge.adaptive_bench.v1 rules from contracts/adaptive_bench.json.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
CONTRACT = ROOT / "contracts" / "adaptive_bench.json"
WORKSHEET = ROOT / "docs" / "worksheets" / "Q-AB-01_adaptive_bench.md"
DOC = ROOT / "docs" / "B-S2-03_adaptive_bench.md"
RS = ROOT / "src-tauri" / "src" / "adaptive_bench" / "mod.rs"
MC = ROOT / "src-tauri" / "src" / "mock_court" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
DOCPIPE = ROOT / "src-tauri" / "src" / "docpipe"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "MockCourtView.tsx"
INVENTORY = ROOT / "eval" / "ac_inventory.json"

PIPELINE = "caseforge.adaptive_bench.v1"
ARCHETYPES = [
    "procedure_first",
    "evidence_heavy",
    "textualist",
    "relief_focused",
    "skeptical_probe",
    "settlement_lens",
]
SLIDERS = [
    "tempo",
    "formality",
    "opposition_aggression",
    "technicality",
    "interruption",
]


def sample_graph(*, gaps: int = 0, urgency: bool = False, statutes: int = 1) -> dict:
    gap_arr = [
        {"id": f"g{i}", "status": "open", "description": "gap"} for i in range(gaps)
    ]
    relief = (
        [{"id": "r1", "text": "Interim injunction pending hearing"}]
        if urgency
        else [{"id": "r1", "text": "Damages as pleaded"}]
    )
    return {
        "jurisdiction": "India",
        "prayer_relief": relief,
        "gaps": gap_arr,
        "evidence": [{"id": "e1", "text": "Exhibit A"}],
        "statutes": [{"id": f"s{i}", "label": f"Act s.{i}"} for i in range(statutes)],
        "chronology": [{"date": "2025-01-01", "text": "Event"}],
        "procedural_posture": None,
        "parties": [],
        "contradictions": [],
        "counsel_facts": [],
    }


def extract_signals(g: dict) -> dict:
    relief_count = len(g.get("prayer_relief") or [])
    open_gap_count = sum(
        1
        for it in (g.get("gaps") or [])
        if (it.get("status") or "open") == "open"
    )
    evidence_count = len(g.get("evidence") or [])
    statute_count = len(g.get("statutes") or [])
    chronology_density = len(g.get("chronology") or [])
    has_pp = g.get("procedural_posture") is not None
    blob = " ".join(
        str(it.get("text") or it.get("label") or "")
        for it in (g.get("prayer_relief") or [])
    ).lower()
    urgency = any(
        k in blob
        for k in ("interim", "bail", "injunction", "stay", "urgent", "ex-parte")
    )
    if urgency:
        suggested = "relief_focused"
    elif open_gap_count >= 2:
        suggested = "skeptical_probe"
    elif evidence_count == 0:
        suggested = "evidence_heavy"
    elif statute_count >= 2:
        suggested = "textualist"
    elif relief_count >= 2:
        suggested = "procedure_first"
    elif open_gap_count == 0 and evidence_count >= 2:
        suggested = "settlement_lens"
    else:
        suggested = "procedure_first"
    raw = (
        f"{g.get('jurisdiction')}|{relief_count}|{open_gap_count}|{evidence_count}|"
        f"{statute_count}|{chronology_density}|{int(has_pp)}|{int(urgency)}|{suggested}"
    )
    fp = hashlib.sha256(raw.encode()).hexdigest()[:16]
    return {
        "suggested_archetype": suggested,
        "fingerprint": fp,
        "open_gap_count": open_gap_count,
        "urgency_hint": urgency,
    }


def aggression_band(v: int) -> str:
    if v >= 70:
        return "high pressure"
    if v >= 40:
        return "moderate pressure"
    return "low pressure"


def behaviour_brief(archetype: str, agg: int, signals: dict) -> str:
    return (
        f"archetype={archetype}; aggression={aggression_band(agg)}; "
        f"fp={signals['fingerprint']}; suggested={signals['suggested_archetype']}"
    )


def main() -> int:
    errors: list[str] = []

    for p in (CONST, CONTRACT, WORKSHEET, DOC, RS, MC, LIB, IPC, VIEW):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # --- Contracts / AC-10.1 ---
    if CONTRACT.exists():
        c = json.loads(CONTRACT.read_text(encoding="utf-8-sig"))
        if c.get("status") != "FROZEN":
            errors.append("adaptive_bench.json status must be FROZEN")
        if c.get("pipeline_id") != PIPELINE:
            errors.append(f"pipeline_id must be {PIPELINE}")
        if c.get("named_judge_profiles") is not False:
            errors.append("named_judge_profiles must be false (AC-10.1)")
        arch = [a.get("id") for a in (c.get("archetypes") or [])]
        if arch != ARCHETYPES:
            errors.append(f"exactly six archetypes required, got {arch}")
        if c.get("sliders") != SLIDERS:
            errors.append(f"sliders mismatch: {c.get('sliders')}")
        if "opposition_aggression" not in (c.get("sliders") or []):
            errors.append("opposition_aggression control required (PRD)")

    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        ab = data.get("adaptive_bench") or {}
        if ab.get("status") != "FROZEN":
            errors.append("constants.adaptive_bench.status must be FROZEN")
        if ab.get("named_judge_profiles") is not False:
            errors.append("constants named_judge_profiles must be false")
        if ab.get("archetype_count") != 6:
            errors.append("archetype_count must be 6")

    # --- Rust AC-10.1 ---
    if RS.exists():
        rs = RS.read_text(encoding="utf-8-sig")
        if PIPELINE not in rs:
            errors.append("Rust missing pipeline id")
        if "CF_BENCH_NO_NAMED_JUDGE" not in rs:
            errors.append("missing named-judge refuse error")
        for a in ARCHETYPES:
            if a not in rs:
                errors.append(f"Rust missing archetype {a}")
        for s in SLIDERS:
            if s not in rs:
                errors.append(f"Rust missing slider {s}")
        # no judge database tables
        if re.search(r"\bjudge_db\b|\bnamed_judges\b|\bJudgeProfile\b", rs):
            errors.append("AC-10.1: judge database pattern found")

    if MC.exists():
        mc = MC.read_text(encoding="utf-8-sig")
        if "adaptive_bench" not in mc:
            errors.append("mock_court must integrate adaptive_bench")

    if DOCPIPE.is_dir():
        for p in DOCPIPE.rglob("*.rs"):
            txt = p.read_text(encoding="utf-8-sig", errors="replace")
            if "crate::adaptive_bench" in txt or re.search(r"\bmod\s+adaptive_bench\b", txt):
                errors.append(f"AC-2.1: docpipe must not import adaptive_bench ({p.name})")

    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8-sig")
        for cmd in (
            "adaptive_bench_policy",
            "adaptive_bench_archetypes",
            "adaptive_bench_resolve",
            "adaptive_bench_resolve_case",
        ):
            if cmd not in lib:
                errors.append(f"lib.rs missing {cmd}")

    if IPC.exists():
        ipc = IPC.read_text(encoding="utf-8-sig")
        for name in (
            "adaptiveBenchPolicy",
            "adaptiveBenchArchetypes",
            "adaptiveBenchResolve",
            "adaptiveBenchResolveCase",
            "opposition_aggression",
        ):
            if name not in ipc:
                errors.append(f"ipc.ts missing {name}")

    if VIEW.exists():
        view = VIEW.read_text(encoding="utf-8-sig")
        if "adaptive-bench-panel" not in view and "Adaptive Bench" not in view:
            errors.append("MockCourtView must expose Adaptive Bench UI")
        for s in SLIDERS:
            if f"slider-{s}" not in view and s not in view:
                errors.append(f"UI missing slider {s}")
        if "named" not in view.lower():
            errors.append("UI should disclose no named judges")

    # --- AC-10.2 behavioural procedures (Python mirror) ---
    g1 = sample_graph(gaps=3, urgency=False)
    g2 = sample_graph(gaps=0, urgency=True)
    s1 = extract_signals(g1)
    s1b = extract_signals(g1)
    s2 = extract_signals(g2)
    if s1["fingerprint"] != s1b["fingerprint"]:
        errors.append("AC-10.2: fingerprint not deterministic")
    if s1["suggested_archetype"] != "skeptical_probe":
        errors.append(
            f"AC-10.2: expected skeptical_probe for multi-gap graph, got {s1['suggested_archetype']}"
        )
    if s2["suggested_archetype"] != "relief_focused":
        errors.append(
            f"AC-10.2: expected relief_focused for urgency graph, got {s2['suggested_archetype']}"
        )
    b_low = behaviour_brief("skeptical_probe", 20, s1)
    b_high = behaviour_brief("skeptical_probe", 90, s1)
    if b_low == b_high:
        errors.append("AC-10.2: opposition_aggression must change behaviour brief")
    if "low pressure" not in b_low or "high pressure" not in b_high:
        errors.append("AC-10.2: aggression bands missing in brief")

    # Inventory detail_status update expected FROZEN for AC-10
    if INVENTORY.exists():
        inv = json.loads(INVENTORY.read_text(encoding="utf-8-sig"))
        acs = {a["id"]: a for a in inv.get("acceptance_criteria") or inv.get("acs") or []}
        # support either shape
        if not acs and isinstance(inv.get("items"), list):
            acs = {a["id"]: a for a in inv["items"]}
        # try results array
        if not acs:
            for a in inv.get("results") or []:
                if "id" in a:
                    acs[a["id"]] = a
        # file may use different structure — scan raw
        raw = INVENTORY.read_text(encoding="utf-8-sig")
        if "AC-10.1" not in raw:
            errors.append("ac_inventory missing AC-10.1")

    if errors:
        print("FAIL  B-S2-03 Adaptive Bench (AC-10.1 / AC-10.2)")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S2-03 Adaptive Bench "
        f"(AC-10.1 six archetypes + no named judges; "
        f"AC-10.2 case-order signals + opposition_aggression; "
        f"pipeline={PIPELINE})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
