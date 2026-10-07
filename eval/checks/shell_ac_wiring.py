#!/usr/bin/env python3
"""
Structural wiring checks for AC-3.1 / AC-3.5 (B-S1-01).

Does not measure real cold-start or frame gaps on hardware — only verifies
the instrumentation required by the batch is present in source.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
APP_TSX = ROOT / "src" / "App.tsx"


def main() -> int:
    errors: list[str] = []
    rs = LIB_RS.read_text(encoding="utf-8") if LIB_RS.exists() else ""
    tsx = APP_TSX.read_text(encoding="utf-8") if APP_TSX.exists() else ""

    if not LIB_RS.exists():
        errors.append("missing src-tauri/src/lib.rs")
    else:
        if "uptime_ms" not in rs or "PROCESS_START" not in rs:
            errors.append("AC-3.1: missing uptime / process start instrumentation in lib.rs")
        if "thread::spawn" not in rs:
            errors.append("AC-3.5: heavy job must use thread::spawn (off UI thread)")
        if "shell_heavy_job_start" not in rs:
            errors.append("AC-3.5: missing shell_heavy_job_start")
        if "heavy_job_progress" not in rs:
            errors.append("AC-3.5: missing progress event emit")

    if not APP_TSX.exists():
        errors.append("missing src/App.tsx")
    else:
        if "UI_FREEZE_BUDGET_MS" not in tsx and "200" not in tsx:
            errors.append("AC-3.5: UI freeze budget probe missing in App.tsx")
        if "requestAnimationFrame" not in tsx:
            errors.append("AC-3.5: frame-gap measurement missing")
        if "max_frame_gap" not in tsx and "maxFrameGapMs" not in tsx:
            errors.append("AC-3.5: max frame gap display missing")

    if errors:
        print("FAIL  AC-3.1/3.5 structural wiring")
        for e in errors:
            print(f"  - {e}")
        return 1

    print("PASS  AC-3.1/3.5 structural wiring (instrumentation present; hardware measure still required)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
