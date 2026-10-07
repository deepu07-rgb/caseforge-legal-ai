#!/usr/bin/env python3
"""B-S2-07 Stage 2 ship gate (v2.0) — Mock Court matrix + AC-2.1 + AC-15.1 + Full-tier messaging.

Runs B-S2-01 … B-S2-06 batch checks, dependency audit, G1/G6 surface audits,
and structural ship artefacts. FAIL always fails ship.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKS = ROOT / "eval" / "checks"
WAIVERS = ROOT / "eval" / "waivers" / "stage2_ship_waivers.json"
REPORTS = ROOT / "eval" / "reports"
DOC = ROOT / "docs" / "B-S2-07_ship_gate.md"
CHANGELOG = ROOT / "CHANGELOG.md"
CHECKLIST = ROOT / "docs" / "RELEASE_CHECKLIST_v2.0.md"
APP = ROOT / "src" / "App.tsx"
MOCK = ROOT / "src" / "components" / "MockCourtView.tsx"
VOICE_RS = ROOT / "src-tauri" / "src" / "voice_court" / "mod.rs"
INSTALLER = ROOT / "src-tauri" / "src" / "installer" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
PACKAGE = ROOT / "package.json"
TAURI_CONF = ROOT / "src-tauri" / "tauri.conf.json"
CARGO = ROOT / "src-tauri" / "Cargo.toml"
REDTEAM = ROOT / "golden" / "G-REDTEAM" / "fixtures" / "stage2_200" / "prompts.json"

# Stage 2 matrix (required green for v2.0)
STAGE2_CHECKS: list[tuple[str, str]] = [
    ("B-S2-01", "integrity_b_s2_01.py"),
    ("B-S2-02", "mock_court_b_s2_02.py"),
    ("B-S2-03", "adaptive_bench_b_s2_03.py"),
    ("B-S2-04", "voice_court_b_s2_04.py"),
    ("B-S2-05", "bench_prep_b_s2_05.py"),
    ("B-S2-06", "stage2_experience_redteam_b_s2_06.py"),
    ("AC-2.1", "ac_2_1_stage2_dependency.py"),
]


def run_check(script: Path) -> tuple[str, str]:
    if not script.exists():
        return "ERROR", f"missing {script.name}"
    cmd = [sys.executable, str(script)]
    try:
        proc = subprocess.run(
            cmd,
            cwd=str(ROOT),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=600,
        )
    except subprocess.TimeoutExpired:
        return "FAIL", f"{script.name} timed out"
    out = (proc.stdout or "") + (proc.stderr or "")
    lines = [ln.strip() for ln in out.splitlines() if ln.strip()]
    last = lines[-1] if lines else f"exit={proc.returncode}"
    if proc.returncode == 0:
        if "PASS" in out.upper() or last.upper().startswith("PASS"):
            return "PASS", last
        return "PASS", last
    if "SKIP" in last.upper():
        return "SKIP", last
    return "FAIL", last


def g1_g6_surface_audit() -> list[str]:
    """Stage 2 surfaces must not claim win prediction; must mark non-decision-maker where relevant."""
    errs: list[str] = []
    bad = [
        "win probability",
        "likely to win",
        "verdict prediction",
        "you will be convicted",
        "client is guilty",
    ]
    for path in (MOCK, APP):
        if not path.exists():
            errs.append(f"missing {path.relative_to(ROOT)}")
            continue
        text = path.read_text(encoding="utf-8", errors="replace").lower()
        for phrase in bad:
            if phrase not in text:
                continue
            idx = text.find(phrase)
            window = text[max(0, idx - 60) : idx + len(phrase) + 50]
            if any(
                n in window
                for n in (
                    "never",
                    "not ",
                    "forbid",
                    "refuse",
                    "g1",
                    "g6",
                    "preparation",
                    "no verdict",
                )
            ):
                continue
            errs.append(f"G1/G6 surface audit: {path.relative_to(ROOT)} contains {phrase!r}")
    if MOCK.exists():
        mt = MOCK.read_text(encoding="utf-8", errors="replace")
        if "non-decision-maker" not in mt and "decision-maker" not in mt.lower():
            errs.append("MockCourtView missing non-decision-maker messaging (B-S2-05)")
        if "case_graph" not in mt and "case_graph only" not in mt.lower():
            # soft: stage2 constraint
            if "AC-2.1" not in mt and "case_graph" not in mt:
                errs.append("MockCourtView should reference case_graph / AC-2.1 constraint")
    return errs


def full_tier_messaging_audit() -> list[str]:
    errs: list[str] = []
    if not INSTALLER.exists():
        errs.append("missing installer")
        return errs
    t = INSTALLER.read_text(encoding="utf-8")
    for n in ("Lite", "Full", "live", "Mock Court", "8"):
        if n not in t and n.lower() not in t.lower():
            # Mock Court might be "simulation"
            if n == "Mock Court" and "simulation" in t:
                continue
            if n not in t:
                errs.append(f"installer messaging missing {n!r}")
    if VOICE_RS.exists():
        v = VOICE_RS.read_text(encoding="utf-8")
        if "LITE_REFUSE" not in v and "CF_VOICE_COURT_LITE_REFUSE" not in v:
            errs.append("voice_court missing Lite refuse for live sim")
        if "Full" not in v and "full" not in v:
            errs.append("voice_court should reference Full tier")
    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8")
        if "ship_stage2_meta" not in lib:
            errs.append("lib.rs missing ship_stage2_meta")
        if "mock_court_requires_full_tier" not in lib:
            errs.append("ship_stage2_meta must declare mock_court_requires_full_tier")
    return errs


def structural_artefacts() -> list[str]:
    errs: list[str] = []
    for p in (DOC, CHANGELOG, CHECKLIST, WAIVERS, LIB, PACKAGE):
        if not p.exists():
            errs.append(f"missing ship artefact {p.relative_to(ROOT)}")
    if PACKAGE.exists():
        pkg = json.loads(PACKAGE.read_text(encoding="utf-8"))
        ver = str(pkg.get("version") or "")
        if not ver.startswith("2."):
            errs.append(f"package.json version should be 2.x for Stage 2 ship, got {ver!r}")
    if TAURI_CONF.exists():
        conf = json.loads(TAURI_CONF.read_text(encoding="utf-8"))
        ver = str(conf.get("version") or "")
        if not ver.startswith("2."):
            errs.append(f"tauri.conf.json version should be 2.x, got {ver!r}")
    if CARGO.exists():
        cargo = CARGO.read_text(encoding="utf-8")
        if 'version = "2.' not in cargo and "version = '2." not in cargo:
            # allow if workspace uses 2.0.0-rc1
            if "2.0.0" not in cargo:
                errs.append("Cargo.toml package version should be 2.x for Stage 2 RC")
    if REDTEAM.exists():
        suite = json.loads(REDTEAM.read_text(encoding="utf-8-sig"))
        n = len(suite.get("prompts") or [])
        if n < 200:
            errs.append(f"AC-15.1 fixture has {n} prompts; need >= 200")
    else:
        errs.append("missing stage2_200 redteam fixture")
    if LIB.exists():
        lib = LIB.read_text(encoding="utf-8")
        if "B-S2-07" not in lib:
            errs.append("lib.rs BATCH should be B-S2-07")
    return errs


def main() -> int:
    errors: list[str] = []
    results: list[dict] = []

    for e in structural_artefacts():
        errors.append(e)
    for e in g1_g6_surface_audit():
        errors.append(e)
    for e in full_tier_messaging_audit():
        errors.append(e)

    if not WAIVERS.exists():
        errors.append("missing stage2_ship_waivers.json")
    else:
        wdata = json.loads(WAIVERS.read_text(encoding="utf-8-sig"))
        # Stage 2 requires AC-15.1 full suite — must NOT waive zero-violation requirement
        for w in wdata.get("waivers") or []:
            if w.get("ac") == "AC-15.1" and w.get("result") == "SKIP":
                errors.append("AC-15.1 must not be SKIP-waived for Stage 2 ship")

    for batch, script_name in STAGE2_CHECKS:
        script = CHECKS / script_name
        status, last = run_check(script)
        results.append(
            {
                "batch": batch,
                "script": script_name,
                "status": status,
                "detail": last[:400],
            }
        )
        if status in ("FAIL", "ERROR"):
            errors.append(f"{batch}/{script_name}: {status} — {last[:200]}")
        elif status == "SKIP":
            errors.append(
                f"{batch}/{script_name}: SKIP not allowed for Stage 2 ship gate"
            )

    n_pass = sum(1 for r in results if r["status"] == "PASS")
    n_fail = sum(1 for r in results if r["status"] in ("FAIL", "ERROR"))
    n_skip = sum(1 for r in results if r["status"] == "SKIP")

    REPORTS.mkdir(parents=True, exist_ok=True)
    report = {
        "batch": "B-S2-07",
        "release": "v2.0.0-rc1",
        "product_surface": "Digest + Mock Court (Full tier for live voice simulation)",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "summary": {
            "pass": n_pass,
            "fail": n_fail,
            "skip": n_skip,
            "errors": len(errors),
        },
        "results": results,
        "errors": errors,
        "gates": {
            "AC-2.1": "docpipe free of Stage 2 imports",
            "AC-8.x": "via B-S2-01 integrity",
            "AC-10.x": "via B-S2-03 adaptive bench",
            "AC-3.3": "via B-S2-02/04 turn budgets",
            "AC-15.1": "via B-S2-06 full red-team",
            "full_tier_mock_court": "Lite refuse live sim; Full messaging present",
        },
        "ship_ok": len(errors) == 0 and n_fail == 0,
    }
    (REPORTS / "stage2_ship_gate.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )

    if errors or n_fail:
        print("FAIL  B-S2-07 Stage 2 ship gate")
        print(f"  summary: PASS={n_pass} FAIL={n_fail} SKIP={n_skip}")
        for e in errors:
            print(f"  - {e}")
        for r in results:
            if r["status"] in ("FAIL", "ERROR"):
                print(f"  - {r['batch']}/{r['script']}: {r['detail'][:160]}")
        return 1

    print(
        "PASS  B-S2-07 Stage 2 ship gate "
        f"(v2.0.0-rc1; checks PASS={n_pass} SKIP={n_skip}; "
        f"AC-2.1 clean; AC-15.1 full suite; Full-tier Mock Court messaging; "
        f"report=eval/reports/stage2_ship_gate.json)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
