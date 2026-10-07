#!/usr/bin/env python3
"""Thin wrapper: scaffold DRAFT G-CASE-IN (India) fixture.

Delegates to scaffold_gcase_fixture.py with --jurisdiction India.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
CORE = HERE / "scaffold_gcase_fixture.py"


def main() -> int:
    # Inject jurisdiction if not already provided
    argv = list(sys.argv[1:])
    if "--jurisdiction" not in argv:
        argv = ["--jurisdiction", "India", *argv]
    return subprocess.call([sys.executable, str(CORE), *argv])


if __name__ == "__main__":
    raise SystemExit(main())
