#!/usr/bin/env python3
"""B-S1-07 structural: case_graph schema frozen + skeleton extractor + source pointers."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "contracts" / "case_graph.schema.json"
SKELETON_PY = ROOT / "sidecars" / "docpipe" / "skeleton.py"
DOCPIPE_PY = ROOT / "sidecars" / "docpipe" / "docpipe.py"
SKELETON_RS = ROOT / "src-tauri" / "src" / "docpipe" / "skeleton.rs"
MIG = ROOT / "src-tauri" / "migrations" / "case_006_case_graph.sql"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
IPC_TS = ROOT / "src" / "lib" / "ipc.ts"
APP = ROOT / "src" / "App.tsx"


def main() -> int:
    errors: list[str] = []
    for p in (SCHEMA, SKELETON_PY, DOCPIPE_PY, SKELETON_RS, MIG, LIB_RS, IPC_TS, APP):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if SCHEMA.exists():
        data = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        if data.get("x-caseforge", {}).get("status") != "FROZEN":
            errors.append("case_graph schema not FROZEN")
        if (data.get("properties") or {}).get("schema_version", {}).get("const") != "1.0.0":
            errors.append("case_graph schema_version const must be 1.0.0")
        for field in (
            "parties",
            "chronology",
            "statutes",
            "evidence",
            "prayer_relief",
            "contradictions",
            "gaps",
            "counsel_facts",
        ):
            if field not in (data.get("properties") or {}):
                errors.append(f"schema missing {field}")
        defs = data.get("$defs") or {}
        if "SourcePointer" not in defs or "Provenance" not in defs:
            errors.append("schema missing SourcePointer/Provenance defs")

    if MIG.exists() and "case_graph" not in MIG.read_text(encoding="utf-8"):
        errors.append("migration missing case_graph table")

    for path, needle in (
        (LIB_RS, "docpipe_skeleton_start"),
        (LIB_RS, "docpipe_case_graph_get"),
        (SKELETON_RS, "EXTRACTOR_ID"),
        (IPC_TS, "docpipeSkeletonStart"),
        (IPC_TS, "CaseGraph"),
        (APP, "Skeleton"),
    ):
        if path.exists() and needle not in path.read_text(encoding="utf-8"):
            errors.append(f"{path.relative_to(ROOT)} missing {needle}")

    # Live extract + source invariant
    if SKELETON_PY.exists():
        sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
        from skeleton import assert_every_extracted_has_source, extract_from_pages  # type: ignore

        sample = """
IN THE COURT OF Sample District
Plaintiff: Acme Calibration Pvt. Ltd.
Defendant: Northern Sample Traders
On 12 March 2023, goods were delivered under purchase order.
Section 138 of the Negotiable Instruments Act is invoked.
The plaintiff prays for recovery of INR 1,25,000.
Exhibit A-1 is the delivery challan dated 15 September 2024.
"""
        graph = extract_from_pages(
            case_id="test",
            jurisdiction="India",
            pages=[{"doc_id": "d1", "page_index": 0, "text": sample, "mean_confidence": 0.9}],
        )
        if graph.get("schema_version") != "1.0.0":
            errors.append("graph schema_version not 1.0.0")
        if not graph.get("chronology"):
            errors.append("expected chronology events from sample")
        if not graph.get("parties"):
            errors.append("expected parties from sample")
        defects = assert_every_extracted_has_source(graph)
        if defects:
            errors.append(f"source defects: {defects}")

    # RPC smoke
    if DOCPIPE_PY.exists():
        req = json.dumps(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "docpipe.extract_skeleton",
                "params": {
                    "case_id": "rpc_test",
                    "jurisdiction": "India",
                    "text": "Plaintiff: A Co. On 01 April 2022 notice issued. Prayer: injunction.",
                },
            }
        )
        proc = subprocess.run(
            [sys.executable, str(DOCPIPE_PY)],
            input=req + "\n",
            text=True,
            capture_output=True,
            cwd=str(ROOT),
            timeout=30,
        )
        if proc.returncode != 0:
            errors.append(f"extract_skeleton RPC exit {proc.returncode}")
        else:
            line = (proc.stdout or "").strip().splitlines()
            if not line:
                errors.append("extract_skeleton empty stdout")
            else:
                resp = json.loads(line[0])
                if "error" in resp:
                    errors.append(f"extract_skeleton error: {resp['error']}")
                elif not (resp.get("result") or {}).get("case_graph"):
                    errors.append("extract_skeleton missing case_graph")

    if errors:
        print("FAIL  B-S1-07 skeleton structural")
        for e in errors:
            print(f"  - {e}")
        return 1
    print(
        "PASS  B-S1-07 skeleton structural "
        "(case_graph FROZEN, extractor, source pointers, IPC, sidecar RPC)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
