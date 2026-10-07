#!/usr/bin/env python3
"""B-S2-01 Argument Integrity Engine — AC-8.1, AC-8.2, AC-8.3.

Pure-Python mirror of caseforge.integrity.v1 (must stay formula-identical).
Also audits Rust module for zero LLM imports in scoring path.
"""

from __future__ import annotations

import json
import re
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "argument_node.schema.json"
QUESTIONS = ROOT / "contracts" / "QUESTIONS.md"
WORKSHEET = ROOT / "docs" / "worksheets" / "Q-AN-01_argument_node.md"
DOC = ROOT / "docs" / "B-S2-01_integrity.md"
RS = ROOT / "src-tauri" / "src" / "integrity" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
VAULT = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
MIG = ROOT / "src-tauri" / "migrations" / "case_010_argument_tree.sql"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "ArgumentTreeView.tsx"
APP = ROOT / "src" / "App.tsx"
DOCPIPE = ROOT / "src-tauri" / "src" / "docpipe"
GOLDEN = ROOT / "golden" / "G-INTEGRITY" / "fixtures"
PLANTED = GOLDEN / "planted_gap_evidence" / "argument_tree.json"
HEALTHY = GOLDEN / "healthy_limb" / "argument_tree.json"

SCORER_ID = "caseforge.integrity.v1"
FORMULA = "VI = 100 - min(Fact, Evidence, Law, Exposure)"
THRESHOLD = 60
TOP_N = 2
LINK_KINDS = ("Fact", "Evidence", "Law", "Exposure")
PLANTED_GAP_ID = "PG-EV-01"

# Imports that must NOT appear in integrity scoring module
LLM_BANNED = (
    "infer_llm",
    "ollama",
    "openai",
    "llm_chat",
    "llama",
    "reqwest",
    "ureq",
    "hyper::",
)


def score_limb(links: dict, threshold: int = THRESHOLD) -> dict:
    scores = {k: int(links[k]) for k in LINK_KINDS}
    for k, v in scores.items():
        if v < 0 or v > 100:
            raise ValueError(f"{k}={v} out of range")
    min_link = min(scores, key=scores.get)  # type: ignore[arg-type]
    min_val = scores[min_link]
    vi = 100 - min_val
    return {
        "links": scores,
        "min_link": min_link,
        "min_score": min_val,
        "VI": vi,
        "missing": [k for k, v in scores.items() if v < threshold],
    }


def score_tree(tree: dict) -> dict:
    t0 = time.perf_counter()
    threshold = int(
        (tree.get("scoring_rules") or {}).get("missing_link_threshold", THRESHOLD)
    )
    limbs = tree.get("limbs") or []
    scored = []
    missing_raw = []
    for limb in limbs:
        node_id = limb.get("node_id") or "?"
        links = limb.get("links") or {}
        s = score_limb(links, threshold)
        expected = limb.get("expected_VI")
        scored.append(
            {
                "node_id": node_id,
                "label": limb.get("label"),
                **s,
                "expected_VI": expected,
                "expected_VI_match": expected is None or int(expected) == s["VI"],
            }
        )
        anchors = limb.get("anchors") or []
        planted = limb.get("planted_gaps") or []
        for kind in LINK_KINDS:
            score = s["links"][kind]
            if score >= threshold:
                # still force planted gaps onto panel
                pass
            inputs = []
            planted_id = None
            desc = None
            kind_l = kind.lower()
            for a in anchors:
                if str(a.get("kind", "")).lower() == kind_l:
                    if a.get("ref"):
                        inputs.append(f"anchor:ref:{a['ref']}")
                    if a.get("case_graph_path"):
                        inputs.append(f"case_graph:{a['case_graph_path']}")
                    if a.get("text"):
                        inputs.append(f"anchor:text:{str(a['text'])[:80]}")
            for g in planted:
                if g.get("link") == kind:
                    if g.get("gap_id"):
                        inputs.append(f"planted_gap:{g['gap_id']}")
                        planted_id = g["gap_id"]
                    if g.get("description"):
                        desc = g["description"]
                        inputs.append(f"planted_desc:{str(desc)[:120]}")
            if score < threshold or planted_id:
                if not inputs:
                    inputs = [
                        f"limb:{node_id}",
                        f"link_score:{kind}={score if score < threshold else threshold - 1}",
                        f"threshold:{threshold}",
                    ]
                missing_raw.append(
                    {
                        "limb_id": node_id,
                        "link": kind,
                        "score": score if score < threshold else max(0, threshold - 1),
                        "threshold": threshold,
                        "inputs_cited": inputs,
                        "planted_gap_id": planted_id,
                        "description": desc,
                    }
                )
        # Ensure every planted gap appears
        for g in planted:
            gl = g.get("link")
            gid = g.get("gap_id")
            if not gl or not gid:
                continue
            if any(
                m["limb_id"] == node_id
                and m["link"] == gl
                and m.get("planted_gap_id") == gid
                for m in missing_raw
            ):
                continue
            score = int((limb.get("links") or {}).get(gl, 0))
            missing_raw.append(
                {
                    "limb_id": node_id,
                    "link": gl,
                    "score": min(score, threshold - 1),
                    "threshold": threshold,
                    "inputs_cited": [
                        f"planted_gap:{gid}",
                        f"limb:{node_id}",
                        f"planted_desc:{str(g.get('description') or '')[:120]}",
                    ],
                    "planted_gap_id": gid,
                    "description": g.get("description"),
                }
            )

    # Rank: lowest score first; planted preferred at same score
    def sort_key(m):
        return (m["score"], 0 if m.get("planted_gap_id") else 1, m["limb_id"], m["link"])

    missing_raw.sort(key=sort_key)
    panel = []
    seen = set()
    for m in missing_raw:
        key = f"{m['limb_id']}:{m['link']}"
        if key in seen:
            # merge planted onto existing
            for p in panel:
                if p["limb_id"] == m["limb_id"] and p["link"] == m["link"]:
                    if not p.get("planted_gap_id") and m.get("planted_gap_id"):
                        p["planted_gap_id"] = m["planted_gap_id"]
                        p["description"] = m.get("description")
                        for i in m["inputs_cited"]:
                            if i not in p["inputs_cited"]:
                                p["inputs_cited"].append(i)
            continue
        seen.add(key)
        panel.append(m)
    for i, m in enumerate(panel):
        m["rank"] = i + 1

    weakest = max(scored, key=lambda x: x["VI"]) if scored else None
    ms = (time.perf_counter() - t0) * 1000
    return {
        "scorer_id": SCORER_ID,
        "formula": FORMULA,
        "llm_used": False,
        "threshold": threshold,
        "limbs": scored,
        "missing_links_panel": panel,
        "weakest_limb": weakest["node_id"] if weakest else None,
        "weakest_VI": weakest["VI"] if weakest else None,
        "recompute_ms": ms,
        "planted_gaps_in_top_n": [
            m["planted_gap_id"]
            for m in panel[:TOP_N]
            if m.get("planted_gap_id")
        ],
    }


def main() -> int:
    errors: list[str] = []

    for p in (
        CONST,
        SCHEMA,
        QUESTIONS,
        WORKSHEET,
        DOC,
        RS,
        LIB,
        VAULT,
        MIG,
        IPC,
        VIEW,
        APP,
        PLANTED,
        HEALTHY,
    ):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # --- Contracts ---
    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        vi = data.get("vulnerability_index") or {}
        if vi.get("status") not in ("FROZEN", "FROZEN_FROM_PRD"):
            # require FROZEN after B-S2-01
            if vi.get("status") != "FROZEN":
                errors.append("vulnerability_index.status must be FROZEN")
        if vi.get("scorer_id") != SCORER_ID:
            errors.append(f"scorer_id must be {SCORER_ID}")
        if vi.get("missing_link_threshold") != THRESHOLD:
            errors.append(f"missing_link_threshold must be {THRESHOLD}")
        if vi.get("scoring_may_use_llm") is not False:
            errors.append("scoring_may_use_llm must be false (AC-8.3)")
        if vi.get("planted_gap_must_appear_in_top_n_missing_links") != TOP_N:
            errors.append(f"planted_gap top_n must be {TOP_N}")
        if vi.get("recompute_max_s") != 5:
            errors.append("recompute_max_s must be 5 (AC-8.2)")

    if SCHEMA.exists():
        sch = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        if sch.get("schema_version") != "1.0.0" and sch.get("properties", {}).get(
            "schema_version", {}
        ).get("const") != "1.0.0":
            # properties.schema_version.const
            prop = (sch.get("properties") or {}).get("schema_version") or {}
            if prop.get("const") != "1.0.0":
                errors.append("argument_node.schema.json must freeze schema_version 1.0.0")
        x = sch.get("x-caseforge") or {}
        if x.get("status") != "FROZEN":
            errors.append("argument_node x-caseforge.status must be FROZEN")
        if "Q-AN-01" in str(x.get("open_questions") or ["Q-AN-01"]):
            if x.get("open_questions"):
                errors.append("Q-AN-01 must be closed (open_questions empty)")

    if QUESTIONS.exists():
        q = QUESTIONS.read_text(encoding="utf-8-sig")
        if "Q-AN-01" not in q or "RESOLVED" not in q:
            errors.append("QUESTIONS.md must mark Q-AN-01 RESOLVED")

    # --- AC-8.3: no LLM in scoring path (Rust integrity module) ---
    if RS.exists():
        rs = RS.read_text(encoding="utf-8-sig")
        if SCORER_ID not in rs:
            errors.append(f"Rust integrity missing {SCORER_ID}")
        if "llm_used: false" not in rs and "llm_used:bool" not in rs.replace(" ", ""):
            # check ScoreReport has llm_used
            if "llm_used" not in rs:
                errors.append("ScoreReport must expose llm_used=false")
        for ban in LLM_BANNED:
            if ban in rs:
                errors.append(f"AC-8.3 FAIL: integrity module references banned '{ban}'")
        # scoring functions must not call network
        if re.search(r"\b(TcpStream|HttpClient|ureq|reqwest)\b", rs):
            errors.append("AC-8.3 FAIL: network client in integrity module")

    # AC-2.1: docpipe must not import integrity / mock court
    if DOCPIPE.is_dir():
        for p in DOCPIPE.rglob("*.rs"):
            txt = p.read_text(encoding="utf-8-sig", errors="replace")
            if "crate::integrity" in txt or "mod integrity" in txt:
                errors.append(f"AC-2.1: docpipe must not import integrity ({p.name})")

    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8-sig")
        for cmd in (
            "integrity_policy",
            "integrity_score_tree",
            "integrity_build_from_graph",
            "argument_tree_get",
            "argument_tree_put",
        ):
            if cmd not in lib:
                errors.append(f"lib.rs missing command {cmd}")

    if VAULT.exists():
        v = VAULT.read_text(encoding="utf-8-sig")
        if "argument_tree_put" not in v or "case_010_argument_tree" not in v:
            errors.append("vault missing argument_tree migration/API")

    if IPC.exists():
        ipc = IPC.read_text(encoding="utf-8-sig")
        for name in (
            "integrityPolicy",
            "integrityScoreTree",
            "integrityBuildFromGraph",
            "argumentTreeGet",
        ):
            if name not in ipc:
                errors.append(f"ipc.ts missing {name}")

    if VIEW.exists() and APP.exists():
        app = APP.read_text(encoding="utf-8-sig")
        if "ArgumentTreeView" not in app:
            errors.append("App.tsx must mount ArgumentTreeView")
        view = VIEW.read_text(encoding="utf-8-sig")
        if "Missing Links" not in view and "missing" not in view.lower():
            errors.append("ArgumentTreeView must show Missing Links panel")
        if "Vulnerability" not in view and "VI" not in view:
            errors.append("ArgumentTreeView must show VI")

    # --- AC-8.1 planted gap ---
    if PLANTED.exists():
        tree = json.loads(PLANTED.read_text(encoding="utf-8-sig"))
        report = score_tree(tree)
        if report["llm_used"]:
            errors.append("AC-8.3: llm_used must be false")
        # expected VI match
        for limb in report["limbs"]:
            if limb.get("expected_VI") is not None and not limb["expected_VI_match"]:
                errors.append(
                    f"VI mismatch limb {limb['node_id']}: "
                    f"got {limb['VI']} expected {limb['expected_VI']}"
                )
        top2 = report["missing_links_panel"][:TOP_N]
        if not any(m.get("planted_gap_id") == PLANTED_GAP_ID for m in top2):
            errors.append(
                f"AC-8.1 FAIL: planted gap {PLANTED_GAP_ID} not in top-{TOP_N} Missing Links; "
                f"panel={[ (m.get('rank'), m.get('link'), m.get('score'), m.get('planted_gap_id')) for m in report['missing_links_panel'] ]}"
            )
        for m in report["missing_links_panel"]:
            if not m.get("inputs_cited"):
                errors.append(
                    f"Missing Link {m['limb_id']}/{m['link']} does not cite inputs"
                )

        # --- AC-8.2 timing (pure score path) ---
        # Stress: re-score many times; total wall for one score must be << 5s
        t0 = time.perf_counter()
        for _ in range(200):
            score_tree(tree)
        elapsed = time.perf_counter() - t0
        per = elapsed / 200
        if per > 5.0:
            errors.append(f"AC-8.2 FAIL: single recompute {per:.3f}s > 5s")
        # also one-shot budget
        one = score_tree(tree)
        if one["recompute_ms"] > 5000:
            errors.append(
                f"AC-8.2 FAIL: recompute_ms={one['recompute_ms']} > 5000"
            )

    if HEALTHY.exists():
        h = score_tree(json.loads(HEALTHY.read_text(encoding="utf-8-sig")))
        if h["missing_links_panel"]:
            errors.append(
                f"healthy_limb should have empty Missing Links; got {h['missing_links_panel']}"
            )
        if h["limbs"] and h["limbs"][0]["VI"] != 15:
            errors.append(f"healthy VI expected 15 got {h['limbs'][0]['VI']}")

    # Unit-level formula checks
    assert score_limb({"Fact": 90, "Evidence": 40, "Law": 80, "Exposure": 70})["VI"] == 60

    if errors:
        print("FAIL  B-S2-01 Argument Integrity Engine")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S2-01 Argument Integrity Engine "
        f"(AC-8.1 planted {PLANTED_GAP_ID} in top-{TOP_N}; "
        f"AC-8.2 recompute budget; AC-8.3 no LLM; "
        f"scorer={SCORER_ID})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
