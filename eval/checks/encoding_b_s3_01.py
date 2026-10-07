#!/usr/bin/env python3
"""B-S3-01 aggregate gate."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


def run(command: list[str]) -> None:
    result = subprocess.run(command, cwd=ROOT, text=True)
    if result.returncode:
        raise SystemExit(result.returncode)


run([sys.executable, "scripts/check_text_encoding.py"])
run(["node", "scripts/test_export_unicode.mjs"])
run([sys.executable, "scripts/check_size_budget.py"])
print("PASS B-S3-01 encoding, Unicode export, and size-budget gate")
