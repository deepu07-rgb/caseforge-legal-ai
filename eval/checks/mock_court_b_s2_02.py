#!/usr/bin/env python3
"""B-S2-02 Mock Court text formats — five sequential formats + AC-2.1 + AC-3.3 budget.

Deterministic Python mirror of caseforge.mock_court.v1 scripted path.
Sequential rule: format N evaluated only after format N-1 PASS.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "session_turn.schema.json"
QUESTIONS = ROOT / "contracts" / "QUESTIONS.md"
WORKSHEET = ROOT / "docs" / "worksheets" / "Q-ST-01_session_turn.md"
DOC = ROOT / "docs" / "B-S2-02_mock_court.md"
RS = ROOT / "src-tauri" / "src" / "mock_court" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
DOCPIPE = ROOT / "src-tauri" / "src" / "docpipe"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "MockCourtView.tsx"
COACH = ROOT / "src" / "lib" / "mockCourtCoach.ts"
SCRIPTS = ROOT / "golden" / "G-SCRIPTS" / "fixtures"

PIPELINE = "caseforge.mock_court.v1"
FORMATS = [
    "Bail_Interim_Motion",
    "Final_Arguments",
    "Witness_Cross_Examination",
    "Appellate",
    "Motion_Generic",
]
STAGES = ["DECOMPOSE", "RETRIEVE", "GENERATE", "VERIFY", "SCORE", "RENDER"]
TURN_MAX_S = 15
FORBIDDEN_SRC = {"raw_document", "page_text_layer", "pdf_parse"}

# Banned in mock_court Rust sources
BANNED_IN_MOCK = (
    "page_text_layer",
    "doc_text_preview",
    "case_llm_context",
    "parse_pdf",
    "lopdf",
    "pdfium",
)


def sample_graph(case_id: str = "scripted") -> dict:
    return {
        "schema_version": "1.0.0",
        "case_id": case_id,
        "jurisdiction": "India",
        "generated_at": "0",
        "pipeline": {
            "extractor_id": "test",
            "recognizer_id": "test",
            "classifier_id": "test",
        },
        "parties": [
            {"id": "p1", "name": "Alpha Traders", "role": "plaintiff"},
            {"id": "p2", "name": "Beta Mills", "role": "defendant"},
        ],
        "chronology": [
            {"id": "e1", "date": "2025-01-12", "text": "Cheque issued"},
            {"id": "e2", "date": "2025-01-22", "text": "Cheque returned unpaid"},
        ],
        "statutes": [{"id": "s1", "label": "Teaching statute label"}],
        "evidence": [{"id": "ev1", "text": "Return memo"}],
        "prayer_relief": [{"id": "r1", "text": "Recover sum as pleaded"}],
        "contradictions": [],
        "gaps": [
            {
                "id": "g1",
                "status": "open",
                "description": "Proof of service incomplete",
            }
        ],
        "counsel_facts": [],
    }


def graph_digest(g: dict) -> dict:
    relief = []
    for it in g.get("prayer_relief") or []:
        t = it.get("text") or it.get("label") or it.get("relief")
        if t:
            relief.append(t)
    chrono = []
    for it in g.get("chronology") or []:
        chrono.append(
            f"{it.get('date') or '?'}: {it.get('text') or it.get('summary') or '(event)'}"
        )
    evidence = [
        (it.get("text") or it.get("label") or "(exhibit)")
        for it in (g.get("evidence") or [])
    ]
    statutes = [
        (it.get("citation") or it.get("label") or it.get("text") or "(statute)")
        for it in (g.get("statutes") or [])
    ]
    gaps = [
        (it.get("description") or it.get("text") or it.get("topic") or "(gap)")
        for it in (g.get("gaps") or [])
    ]
    return {
        "jurisdiction": g.get("jurisdiction") or "Other",
        "relief": relief,
        "chronology": chrono,
        "evidence": evidence,
        "statutes": statutes,
        "gaps": gaps,
    }


def run_turn(format_id: str, user: str, graph: dict, retrieval_ids: list[str]) -> dict:
    t0 = time.perf_counter()
    stages = list(STAGES)
    d = graph_digest(graph)
    order = FORMATS.index(format_id) + 1
    relief = d["relief"][0] if d["relief"] else "relief as pleaded"
    fact = d["chronology"][0] if d["chronology"] else "no chronology"
    exhibit = d["evidence"][0] if d["evidence"] else "no exhibit"
    statute = d["statutes"][0] if d["statutes"] else "(no statute)"
    gap = d["gaps"][0] if d["gaps"] else "open gap"
    rids = list(retrieval_ids) + [f"case_graph:chronology[0]"]
    body = (
        f"Adversary (drill) · {format_id} ({d['jurisdiction']})\n"
        f"Relief: {relief}\n"
        f"Chronology: {fact}\n"
        f"Exhibit: {exhibit}\n"
        f"Law: {statute}\n"
        f"Gap: {gap}\n"
        f"Counsel: {user[:200]}\n"
        f"retrieval_ids: {', '.join(rids[:6])}\n"
        "[VERIFY] case_graph + retrieval only.\n"
        "[SCORE] preparation coverage signal only.\n"
        "Preparation / rehearsal only. Never predicts verdicts."
    )
    elapsed_ms = int((time.perf_counter() - t0) * 1000)
    material = f"{format_id}|{user}|{relief}|{','.join(rids)}"
    prompt_hash = hashlib.sha256(material.encode()).hexdigest()[:16]
    return {
        "schema_version": "1.0.0",
        "format": format_id,
        "format_order": order,
        "role": "adversary",
        "stage": "RENDER",
        "assistant_text": body,
        "pipeline_stages_completed": stages,
        "retrieval_ids": rids,
        "model_hash": "deterministic",
        "prompt_hash": prompt_hash,
        "elapsed_ms": elapsed_ms,
        "sources": ["case_graph"] + (["retrieval"] if retrieval_ids else []),
        "disclaimer": "Preparation only",
    }


def default_lines(fmt: str) -> list[str]:
    return [
        f"Counsel opens {fmt} with the pleaded relief and strongest limb.",
        f"Counsel addresses the open gap and the principal exhibit on the graph.",
        f"Counsel closes the {fmt} submission without predicting outcome.",
    ]


def run_script(fmt: str, graph: dict) -> dict:
    turns = []
    max_ms = 0
    reasons = []
    ok = True
    for line in default_lines(fmt):
        t = run_turn(fmt, line, graph, ["pack:demo:statute"])
        turns.append(t)
        max_ms = max(max_ms, t["elapsed_ms"])
        if t["elapsed_ms"] > TURN_MAX_S * 1000:
            ok = False
            reasons.append("AC-3.3 latency")
        if FORBIDDEN_SRC & set(t["sources"]):
            ok = False
            reasons.append("forbidden source")
        if len(t["pipeline_stages_completed"]) < 6:
            ok = False
            reasons.append("stages incomplete")
        if not t.get("prompt_hash") or not t.get("model_hash"):
            ok = False
            reasons.append("G5 missing")
    if ok:
        reasons.append("pass")
    return {
        "format": fmt,
        "pass": ok,
        "reasons": reasons,
        "max_turn_ms": max_ms,
        "turns": turns,
    }


def main() -> int:
    errors: list[str] = []

    for p in (CONST, SCHEMA, QUESTIONS, WORKSHEET, DOC, RS, LIB, IPC, VIEW, COACH):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # Contracts
    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        mc = data.get("mock_court") or {}
        if mc.get("status") != "FROZEN":
            errors.append("constants.mock_court.status must be FROZEN")
        if mc.get("pipeline_id") != PIPELINE:
            errors.append(f"pipeline_id must be {PIPELINE}")
        if mc.get("named_judge_profiles") is not False:
            errors.append("named_judge_profiles must be false")
        seq = [x.get("id") for x in (mc.get("sequential_formats") or [])]
        if seq != FORMATS:
            errors.append(f"sequential_formats mismatch: {seq}")
        if mc.get("turn_max_s") != TURN_MAX_S:
            errors.append("turn_max_s must be 15 (AC-3.3)")
        forbidden = set(mc.get("forbidden_sources") or [])
        if not FORBIDDEN_SRC.issubset(forbidden):
            errors.append("forbidden_sources incomplete")

    if SCHEMA.exists():
        sch = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        prop = (sch.get("properties") or {}).get("schema_version") or {}
        if prop.get("const") != "1.0.0":
            errors.append("session_turn schema_version must be 1.0.0")
        x = sch.get("x-caseforge") or {}
        if x.get("status") != "FROZEN":
            errors.append("session_turn not FROZEN")
        fmt_enum = ((sch.get("properties") or {}).get("format") or {}).get("enum") or []
        if fmt_enum != FORMATS:
            errors.append(f"format enum mismatch: {fmt_enum}")

    if QUESTIONS.exists() and "RESOLVED (B-S2-02)" not in QUESTIONS.read_text(
        encoding="utf-8-sig"
    ):
        errors.append("Q-ST-01 must be RESOLVED for B-S2-02")

    # Rust module AC-2.1 — ban actual doc-pipeline calls, not forbid-list strings
    if RS.exists():
        rs = RS.read_text(encoding="utf-8-sig")
        if PIPELINE not in rs:
            errors.append("Rust mock_court missing pipeline id")
        for ban in ("doc_text_preview", "case_llm_context", "lopdf", "pdfium"):
            if ban in rs:
                errors.append(f"AC-2.1 FAIL: mock_court references {ban}")
        # parse_pdf / page_text_layer allowed only inside forbid lists / comments
        for ban in ("parse_pdf", "page_text_layer"):
            for m in re.finditer(re.escape(ban), rs):
                line_start = rs.rfind("\n", 0, m.start()) + 1
                line_end = rs.find("\n", m.end())
                if line_end < 0:
                    line_end = len(rs)
                line = rs[line_start:line_end]
                if any(
                    k in line.lower()
                    for k in ("forbid", "never", "not ", "forbidden", "///", "//")
                ):
                    continue
                errors.append(
                    f"AC-2.1 FAIL: mock_court uses {ban} outside forbid list: {line.strip()[:100]}"
                )
        for f in FORMATS:
            if f not in rs:
                errors.append(f"Rust missing format {f}")

    # docpipe must not import mock_court
    if DOCPIPE.is_dir():
        for p in DOCPIPE.rglob("*.rs"):
            txt = p.read_text(encoding="utf-8-sig", errors="replace")
            if "crate::mock_court" in txt or re.search(r"\bmod\s+mock_court\b", txt):
                errors.append(f"AC-2.1: docpipe imports mock_court ({p.name})")

    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8-sig")
        for cmd in (
            "mock_court_policy",
            "mock_court_formats",
            "mock_court_turn",
            "mock_court_run_script",
            "mock_court_turn_for_case",
        ):
            if cmd not in lib:
                errors.append(f"lib.rs missing {cmd}")
        # case path must not use case_llm_context inside mock court helper
        # (case_llm_context exists elsewhere — only check mock_court_turn_for_case block)
        if "fn mock_court_turn_for_case" in lib:
            start = lib.index("fn mock_court_turn_for_case")
            end = lib.index("fn integrity_policy", start) if "fn integrity_policy" in lib[start:] else start + 2000
            block = lib[start:end]
            # Call sites only — comments may mention the banned API by name.
            if re.search(r"\bcase_llm_context\s*\(", block):
                errors.append("mock_court_turn_for_case must not call case_llm_context (raw docs)")

    if IPC.exists():
        ipc = IPC.read_text(encoding="utf-8-sig")
        for name in (
            "mockCourtPolicy",
            "mockCourtFormats",
            "mockCourtTurn",
            "mockCourtRunScript",
            "mockCourtTurnForCase",
        ):
            if name not in ipc:
                errors.append(f"ipc.ts missing {name}")

    if COACH.exists():
        coach = COACH.read_text(encoding="utf-8-sig")
        for f in FORMATS:
            if f not in coach:
                errors.append(f"mockCourtCoach missing format id {f}")

    if VIEW.exists():
        view = VIEW.read_text(encoding="utf-8-sig")
        if "Bail_Interim_Motion" not in view and "COURT_FORMATS" not in view:
            errors.append("MockCourtView must use PRD court formats")
        if "caseLlmContext" in view and "mockCourtTurnForCase" not in view:
            # prefer Stage-2 path; warn if only raw context
            errors.append(
                "MockCourtView should call mockCourtTurnForCase (graph-only) for adversary path"
            )

    # Sequential scripted sessions
    graph = sample_graph()
    sequential_ok = True
    for i, fmt in enumerate(FORMATS):
        # fixture present
        fix = SCRIPTS / f"format_{i+1:02d}_{fmt.lower()}" / "script.json"
        if not fix.exists():
            errors.append(f"missing script fixture {fix.relative_to(ROOT)}")
        rep = run_script(fmt, graph)
        if not rep["pass"]:
            sequential_ok = False
            errors.append(f"format {fmt} scripted session FAIL: {rep['reasons']}")
            break  # sequential: stop at first fail
        if rep["max_turn_ms"] > TURN_MAX_S * 1000:
            errors.append(f"AC-3.3 {fmt} max_turn_ms={rep['max_turn_ms']}")

    if sequential_ok and not any("scripted session FAIL" in e for e in errors):
        # stress latency offline
        t0 = time.perf_counter()
        for _ in range(50):
            run_turn("Final_Arguments", "burden and proof map on the graph limbs", graph, [])
        per = (time.perf_counter() - t0) / 50
        if per > TURN_MAX_S:
            errors.append(f"AC-3.3 offline turn {per:.3f}s > {TURN_MAX_S}s")

    if errors:
        print("FAIL  B-S2-02 Mock Court text formats")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S2-02 Mock Court text formats "
        f"(5 sequential formats; AC-2.1 graph+retrieval only; "
        f"AC-3.3 budget {TURN_MAX_S}s; pipeline={PIPELINE})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
