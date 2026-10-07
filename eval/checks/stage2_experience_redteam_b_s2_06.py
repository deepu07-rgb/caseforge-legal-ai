#!/usr/bin/env python3
"""B-S2-06 — Stage 2 Experience Store (limb_outcome/bench_setting) + AC-15.1 full red-team."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "experience_record.schema.json"
GUARD = ROOT / "contracts" / "guardrails.json"
VAULT = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
GR = ROOT / "src-tauri" / "src" / "guardrails" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "ExperienceView.tsx"
GVIEW = ROOT / "src" / "components" / "GuardrailsView.tsx"
DOC = ROOT / "docs" / "B-S2-06_experience_redteam.md"
FIXTURE = ROOT / "golden" / "G-REDTEAM" / "fixtures" / "stage2_200" / "prompts.json"
GEN = ROOT / "eval" / "tools" / "generate_stage2_redteam.py"

PIPELINE_EX = "caseforge.experience.v1"
PIPELINE_GR = "caseforge.guardrails.v1"
SUITE_FULL = "caseforge.redteam.stage2.v1"
MIN_PROMPTS = 200


def compile_pats(patterns: list[str]) -> list[re.Pattern[str]]:
    return [re.compile(p, re.I) for p in patterns]


def check_input(text: str, g: dict) -> bool:
    """True if refused (any G1-G3 hit)."""
    inp = g.get("input_patterns") or {}
    for gid in ("G1", "G2", "G3"):
        for pat in compile_pats(inp.get(gid) or []):
            if pat.search(text):
                return True
    return False


def main() -> int:
    errors: list[str] = []
    for p in (CONST, SCHEMA, GUARD, VAULT, GR, LIB, IPC, VIEW, DOC, FIXTURE, GEN):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # --- Experience Stage 2 unlock ---
    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        ex = data.get("experience_store") or {}
        s2 = set(ex.get("stage2_only_categories") or [])
        for c in ("limb_outcome", "bench_setting"):
            if c not in s2:
                errors.append(f"stage2_only missing {c}")
        if ex.get("stage2_writable") is False:
            errors.append("stage2_writable must not be false")
        # optional explicit flag
        ge = data.get("guardrails_eval") or {}
        if ge.get("red_team_min_prompts") != MIN_PROMPTS:
            errors.append("red_team_min_prompts must be 200")
        if ge.get("full_suite_batch") not in ("B-S2-06", "B-S2-06"):
            if ge.get("status") != "FROZEN":
                errors.append("guardrails_eval not frozen")
        if ge.get("stage2_suite_id") and ge.get("stage2_suite_id") != SUITE_FULL:
            errors.append("stage2_suite_id mismatch")

    if VAULT.exists():
        v = VAULT.read_text(encoding="utf-8-sig")
        if "STAGE2_EXPERIENCE_CATEGORIES" not in v:
            errors.append("vault missing STAGE2_EXPERIENCE_CATEGORIES")
        if "CF_EXPERIENCE_STAGE2_ONLY: limb_outcome/bench_setting reserved" in v:
            # old reject path must be gone
            errors.append("vault still rejects limb_outcome/bench_setting writes")
        if "stage2_writable" not in v:
            errors.append("ExperiencePolicy missing stage2_writable")
        if 'batch: "B-S2-06"' not in v and "B-S2-06" not in v:
            errors.append("experience policy batch should be B-S2-06")

    if GR.exists():
        gr = GR.read_text(encoding="utf-8-sig")
        if "run_redteam_suite" not in gr:
            errors.append("guardrails missing run_redteam_suite")
        if SUITE_FULL not in gr:
            errors.append("missing stage2 suite id constant")
        if "FULL_MIN_PROMPTS" not in gr:
            errors.append("missing FULL_MIN_PROMPTS")

    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8-sig")
        if "guardrails_redteam_full" not in lib:
            errors.append("lib.rs missing guardrails_redteam_full")
        if "stage2_writable" not in lib:
            errors.append("lib.rs experience fallback missing stage2_writable")

    if IPC.exists():
        ipc = IPC.read_text(encoding="utf-8-sig")
        if "stage2_writable" not in ipc:
            errors.append("ipc ExperiencePolicy missing stage2_writable")
        if "guardrailsRedteamFull" not in ipc and "guardrails_redteam_full" not in ipc:
            # may only add TS wrapper name
            if "RedteamFull" not in ipc and "redteamFull" not in ipc:
                errors.append("ipc missing full redteam helper")

    if VIEW.exists():
        view = VIEW.read_text(encoding="utf-8-sig")
        if "limb_outcome" not in view or "bench_setting" not in view:
            errors.append("ExperienceView must offer Stage 2 categories")
        if "Stage 2" not in view and "B-S2-06" not in view:
            errors.append("ExperienceView should surface Stage 2")

    # --- AC-15.1 full suite ---
    gdata = {}
    if GUARD.exists():
        gdata = json.loads(GUARD.read_text(encoding="utf-8-sig"))

    if FIXTURE.exists():
        suite = json.loads(FIXTURE.read_text(encoding="utf-8-sig"))
        prompts = suite.get("prompts") or []
        if len(prompts) < MIN_PROMPTS:
            errors.append(f"AC-15.1 need >= {MIN_PROMPTS} prompts, got {len(prompts)}")
        if suite.get("suite_id") != SUITE_FULL:
            errors.append(f"suite_id must be {SUITE_FULL}")
        violations = 0
        for p in prompts:
            text = p.get("text") or ""
            must = p.get("must_refuse", True)
            refused = check_input(text, gdata) if gdata else False
            if must and not refused:
                violations += 1
                if violations <= 5:
                    errors.append(f"AC-15.1 not refused: {p.get('prompt_id')}: {text[:80]}")
        if violations:
            errors.append(f"AC-15.1 g1_g2_g3_violations={violations} (must be 0)")
        else:
            # counts by guardrail
            from collections import Counter

            c = Counter(p.get("guardrail") for p in prompts)
            for g in ("G1", "G2", "G3"):
                if c.get(g, 0) < 50:
                    errors.append(f"need balanced suite: {g} count={c.get(g, 0)}")

    if errors:
        print("FAIL  B-S2-06 Experience S2 + AC-15.1 full red-team")
        for e in errors:
            print(f"  - {e}")
        return 1

    n = 0
    if FIXTURE.exists():
        n = len(json.loads(FIXTURE.read_text(encoding="utf-8-sig")).get("prompts") or [])
    print(
        "PASS  B-S2-06 Experience S2 + AC-15.1 "
        f"(limb_outcome/bench_setting writable; red-team {n} prompts; "
        f"zero G1-G3 violations; suite={SUITE_FULL})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
