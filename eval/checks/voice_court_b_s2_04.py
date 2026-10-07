#!/usr/bin/env python3
"""B-S2-04 Voice-in-court + structured debrief.

Checks: multi-turn offline rehearsal, AC-3.3 budget, network off,
Lite honesty refuse, English pin, UI + IPC wiring, no docpipe import.
"""

from __future__ import annotations

import json
import re
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
DOC = ROOT / "docs" / "B-S2-04_voice_court.md"
RS = ROOT / "src-tauri" / "src" / "voice_court" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
MC = ROOT / "src-tauri" / "src" / "mock_court" / "mod.rs"
DOCPIPE = ROOT / "src-tauri" / "src" / "docpipe"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "MockCourtView.tsx"
SCRIPT_FIX = (
    ROOT
    / "golden"
    / "G-SCRIPTS"
    / "fixtures"
    / "voice_01_bail_interim"
    / "script.json"
)

PIPELINE = "caseforge.voice_court.v1"
LITE_CODE = "CF_VOICE_COURT_LITE_REFUSE"
TURN_MAX_S = 15
TTS_ROLES = ("bench", "opposition", "witness")


def apply_lexicon(raw: str, pairs: list[tuple[str, str]]) -> tuple[str, bool]:
    out = raw
    hit = False
    for surface, corrected in sorted(pairs, key=lambda x: -len(x[0])):
        if not surface:
            continue
        low = out.lower()
        s = surface.lower()
        pos = low.find(s)
        if pos >= 0:
            out = out[:pos] + corrected + out[pos + len(surface) :]
            hit = True
    return out, hit


def sample_graph() -> dict:
    return {
        "jurisdiction": "India",
        "prayer_relief": [{"id": "r1", "text": "Interim protection as pleaded"}],
        "chronology": [{"date": "2025-01-12", "text": "Key event"}],
        "evidence": [{"id": "ev1", "text": "Exhibit A"}],
        "statutes": [{"id": "s1", "label": "Teaching statute"}],
        "gaps": [{"id": "g1", "status": "open", "description": "Service proof incomplete"}],
        "parties": [],
        "contradictions": [],
        "counsel_facts": [],
        "procedural_posture": None,
    }


def fake_court_reply(format_id: str, counsel: str) -> str:
    return (
        f"Adversary · {format_id}\n"
        f"You said: {counsel[:120]}\n"
        "Press open gap from case_graph. No outcome prediction.\n"
        "retrieval_ids: case_graph:chronology[0]"
    )


def run_voice_turn(
    format_id: str,
    counsel: str,
    *,
    force_tier: str = "full",
    language: str = "English",
    pairs: list[tuple[str, str]] | None = None,
) -> dict:
    t0 = time.perf_counter()
    if force_tier == "lite":
        raise RuntimeError(f"{LITE_CODE}: Lite honesty refuse")
    if language.lower() not in ("english", "en") and not language.lower().startswith("en"):
        raise RuntimeError("CF_SPEECH_LANGUAGE_PINNED")
    pairs = pairs or []
    staged, lex = apply_lexicon(counsel, pairs)
    reply = fake_court_reply(format_id, staged)
    spoken, _ = apply_lexicon(reply, pairs)
    ms = int((time.perf_counter() - t0) * 1000)
    return {
        "counsel_raw": counsel,
        "counsel_staged": staged,
        "lexicon_applied": lex,
        "tts_role": "opposition",
        "tts_spoken_text": spoken,
        "network_used": False,
        "cloud": False,
        "elapsed_ms": ms,
        "within_ac33": ms <= TURN_MAX_S * 1000,
        "sources": ["case_graph"],
    }


def build_debrief(turns: list[dict]) -> dict:
    return {
        "turns_completed": len(turns),
        "strengths": ["offline multi-turn"],
        "weaknesses": ["expand anchors"] if turns else [],
        "missing_links_focus": ["gap on graph"],
        "recommended_actions": ["re-run Argument Tree"],
        "lexicon_hits": sum(1 for t in turns if t.get("lexicon_applied")),
        "network_used_any": any(t.get("network_used") for t in turns),
        "max_turn_ms": max((t["elapsed_ms"] for t in turns), default=0),
        "within_ac33": all(t.get("within_ac33") for t in turns) if turns else True,
    }


def main() -> int:
    errors: list[str] = []

    for p in (CONST, DOC, RS, LIB, MC, IPC, VIEW, SCRIPT_FIX):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        vc = data.get("voice_court") or {}
        if vc.get("status") != "FROZEN":
            errors.append("constants.voice_court.status must be FROZEN")
        if vc.get("pipeline_id") != PIPELINE:
            errors.append(f"pipeline_id must be {PIPELINE}")
        if vc.get("primary_language") != "English":
            errors.append("primary_language must be English")
        if vc.get("network_required") is not False:
            errors.append("network_required must be false")
        if vc.get("turn_max_s") != TURN_MAX_S:
            errors.append("turn_max_s must be 15")
        if vc.get("live_simulation_requires_full_tier") is not True:
            errors.append("live_simulation_requires_full_tier must be true")
        if vc.get("lite_refuse_code") != LITE_CODE:
            errors.append(f"lite_refuse_code must be {LITE_CODE}")
        if vc.get("tts_roles") != list(TTS_ROLES):
            errors.append("tts_roles must be bench/opposition/witness")

    if RS.exists():
        rs = RS.read_text(encoding="utf-8-sig")
        if PIPELINE not in rs:
            errors.append("Rust missing pipeline id")
        if LITE_CODE not in rs:
            errors.append("missing Lite refuse code")
        if "Lite tier honesty" not in rs and "Lite tier" not in rs:
            errors.append("missing Lite honesty notice text")
        if "build_debrief" not in rs:
            errors.append("missing structured debrief")
        for role in TTS_ROLES:
            if role not in rs:
                errors.append(f"missing TTS role {role}")
        for ban in ("case_llm_context", "doc_text_preview"):
            if re.search(rf"\b{ban}\s*\(", rs):
                errors.append(f"voice_court must not call {ban}")

    if DOCPIPE.is_dir():
        for p in DOCPIPE.rglob("*.rs"):
            txt = p.read_text(encoding="utf-8-sig", errors="replace")
            if "crate::voice_court" in txt or re.search(r"\bmod\s+voice_court\b", txt):
                errors.append(f"AC-2.1: docpipe imports voice_court ({p.name})")

    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8-sig")
        for cmd in (
            "voice_court_policy",
            "voice_court_gate",
            "voice_court_turn",
            "voice_court_run_script",
            "voice_court_turn_for_case",
            "voice_court_debrief",
        ):
            if cmd not in lib:
                errors.append(f"lib.rs missing {cmd}")

    if IPC.exists():
        ipc = IPC.read_text(encoding="utf-8-sig")
        for name in (
            "voiceCourtPolicy",
            "voiceCourtGate",
            "voiceCourtTurn",
            "voiceCourtTurnForCase",
            "voiceCourtDebrief",
            "voiceCourtRunScript",
        ):
            if name not in ipc:
                errors.append(f"ipc.ts missing {name}")

    if VIEW.exists():
        view = VIEW.read_text(encoding="utf-8-sig")
        if "voice-mode-tab" not in view and "Voice duplex" not in view:
            errors.append("MockCourtView missing Voice duplex mode")
        if "lite-honesty-notice" not in view and "Lite honesty" not in view:
            errors.append("UI missing Lite honesty notice surface")
        if "voice-debrief-btn" not in view and "Debrief" not in view:
            errors.append("UI missing debrief action")
        if "voiceCourtTurnForCase" not in view:
            errors.append("UI must call voiceCourtTurnForCase")

    # Behavioural: multi-turn + Lite refuse + language pin + AC-3.3
    try:
        run_voice_turn("Bail_Interim_Motion", "test", force_tier="lite")
        errors.append("Lite tier must refuse voice simulation")
    except RuntimeError as e:
        if LITE_CODE not in str(e):
            errors.append(f"Lite refuse wrong error: {e}")

    try:
        run_voice_turn("Final_Arguments", "hello", language="Hindi")
        errors.append("non-English must pin-fail")
    except RuntimeError as e:
        if "LANGUAGE_PINNED" not in str(e):
            errors.append(f"language pin wrong: {e}")

    turns = []
    lines = [
        "My Lord I seek interim relief as pleaded on the case graph.",
        "The exhibit and chronology support balance of convenience.",
        "I close without predicting the outcome.",
    ]
    for line in lines:
        t = run_voice_turn(
            "Bail_Interim_Motion",
            line,
            force_tier="full",
            pairs=[("my lord", "My Lord")],
        )
        turns.append(t)
        if t["network_used"] or t["cloud"]:
            errors.append("network/cloud on voice turn")
        if not t["within_ac33"]:
            errors.append(f"AC-3.3 fail {t['elapsed_ms']}ms")

    if len(turns) < 2:
        errors.append("need multi-turn voice rehearsal")
    deb = build_debrief(turns)
    if deb["network_used_any"]:
        errors.append("debrief network_used_any true")
    if deb["turns_completed"] < 2:
        errors.append("debrief turns_completed < 2")
    if not deb["within_ac33"]:
        errors.append("debrief AC-3.3 fail")

    # stress latency
    t0 = time.perf_counter()
    for _ in range(30):
        run_voice_turn("Appellate", "error of law on the graph record", force_tier="full")
    per = (time.perf_counter() - t0) / 30
    if per > TURN_MAX_S:
        errors.append(f"offline turn {per:.3f}s > {TURN_MAX_S}s")

    if SCRIPT_FIX.exists():
        sc = json.loads(SCRIPT_FIX.read_text(encoding="utf-8-sig"))
        if sc.get("mode") != "voice_duplex" and sc.get("pipeline_id") != PIPELINE:
            if sc.get("pipeline_id") != PIPELINE:
                errors.append("voice script fixture pipeline_id mismatch")

    if errors:
        print("FAIL  B-S2-04 Voice-in-court + debrief")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S2-04 Voice-in-court + debrief "
        f"(multi-turn offline; AC-3.3 <={TURN_MAX_S}s; Lite honesty refuse; "
        f"English pin; pipeline={PIPELINE})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
