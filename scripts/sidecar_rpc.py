#!/usr/bin/env python3
"""Send one JSON-RPC request to a CaseForge sidecar the way the Rust host does, and print the JSON response.

Usage (run with the BUNDLED interpreter so the check covers the shipped runtime):
    runtime-python/python.exe scripts/sidecar_rpc.py <sidecar.py> <method>

Standard library only. Exit code 0 only when the sidecar returns a JSON-RPC result (no error object).
"""
from __future__ import annotations

import json
import subprocess
import sys


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: sidecar_rpc.py <sidecar.py> <method>", file=sys.stderr)
        return 2
    script, method = sys.argv[1], sys.argv[2]
    request = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": {}}) + "\n"
    proc = subprocess.run([sys.executable, script], input=request.encode("utf-8"), capture_output=True, timeout=60)
    out = proc.stdout.decode("utf-8", errors="replace")
    if proc.returncode != 0:
        print(f"sidecar exited {proc.returncode}: {proc.stderr.decode('utf-8', errors='replace')[:400]} {out[:200]}", file=sys.stderr)
        return 1
    line = next((l for l in out.splitlines() if l.strip().startswith("{")), None)
    if line is None:
        print(f"no JSON response: {out[:200]}", file=sys.stderr)
        return 1
    response = json.loads(line)
    if "error" in response:
        print(json.dumps(response["error"]), file=sys.stderr)
        return 1
    print(json.dumps(response["result"], ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
