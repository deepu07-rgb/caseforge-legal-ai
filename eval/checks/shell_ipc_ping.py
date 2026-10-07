#!/usr/bin/env python3
"""
Partial AC-4.1 check for B-S1-01: shell.ping / shell.version contract freeze.

Does not require a running GUI. Validates:
  1. contracts/ipc/shell.schema.json exists and declares required methods
  2. Rust command names exist in src-tauri/src/lib.rs
  3. Frontend invoke wrappers exist in src/lib/ipc.ts

Full IPC round-trip against a live window remains SKIP until CI has a headless/runtime harness.
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SHELL_SCHEMA = ROOT / "contracts" / "ipc" / "shell.schema.json"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
IPC_TS = ROOT / "src" / "lib" / "ipc.ts"

REQUIRED_COMMANDS = [
    "shell_ping",
    "shell_version",
    "shell_heavy_job_start",
    "shell_heavy_job_status",
    "shell_sidecar_status",
]


def main() -> int:
    errors: list[str] = []

    if not SHELL_SCHEMA.exists():
        errors.append(f"missing {SHELL_SCHEMA}")
    else:
        data = json.loads(SHELL_SCHEMA.read_text(encoding="utf-8"))
        example = (data.get("examples") or [{}])[0]
        methods = example.get("methods") or []
        names = {m.get("tauri_command") for m in methods}
        for cmd in REQUIRED_COMMANDS:
            if cmd not in names:
                errors.append(f"shell.schema.json example missing tauri_command {cmd}")

    if not LIB_RS.exists():
        errors.append(f"missing {LIB_RS}")
    else:
        text = LIB_RS.read_text(encoding="utf-8")
        for cmd in REQUIRED_COMMANDS:
            if not re.search(rf"fn\s+{cmd}\s*\(", text):
                errors.append(f"lib.rs missing command fn {cmd}")
        if "generate_handler!" not in text:
            errors.append("lib.rs missing generate_handler!")

    if not IPC_TS.exists():
        errors.append(f"missing {IPC_TS}")
    else:
        text = IPC_TS.read_text(encoding="utf-8")
        for cmd in REQUIRED_COMMANDS:
            if f'"{cmd}"' not in text and f"'{cmd}'" not in text:
                errors.append(f"ipc.ts missing invoke target {cmd}")

    if errors:
        print("FAIL  AC-4.1 partial (shell IPC)")
        for e in errors:
            print(f"  - {e}")
        return 1

    print("PASS  AC-4.1 partial (shell.ping/version contract + command wiring)")
    print(f"  commands: {', '.join(REQUIRED_COMMANDS)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
