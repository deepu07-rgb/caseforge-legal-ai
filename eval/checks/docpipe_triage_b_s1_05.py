#!/usr/bin/env python3
"""B-S1-05 structural + sidecar checks: docpipe triage (PRD §6.1).

AC-6.1 ship gate (≥92% on 500 GOLDEN pages) is evaluated separately by run_eval /
this script's golden path. Scaffold fixtures must NOT produce a silent PASS.
"""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "contracts" / "page_triage.schema.json"
DOCPIPE_SCHEMA = ROOT / "contracts" / "ipc" / "docpipe.schema.json"
TRIAGE_RS = ROOT / "src-tauri" / "src" / "docpipe" / "triage.rs"
DOCPIPE_PY = ROOT / "sidecars" / "docpipe" / "docpipe.py"
TRIAGE_PY = ROOT / "sidecars" / "docpipe" / "triage.py"
MIG = ROOT / "src-tauri" / "migrations" / "case_004_page_triage.sql"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
MOD_RS = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
IPC_TS = ROOT / "src" / "lib" / "ipc.ts"
APP = ROOT / "src" / "App.tsx"
GDOCS = ROOT / "golden" / "G-DOCS" / "fixtures"

CLASSES = {
    "typed_english",
    "other",
    "mixed",
    "clean_handwriting",
    "messy_handwriting",
    "non_text",
}


def main() -> int:
    errors: list[str] = []

    for p in (
        SCHEMA,
        DOCPIPE_SCHEMA,
        TRIAGE_RS,
        DOCPIPE_PY,
        TRIAGE_PY,
        MIG,
        LIB_RS,
        MOD_RS,
        IPC_TS,
        APP,
    ):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if SCHEMA.exists():
        data = json.loads(SCHEMA.read_text(encoding="utf-8"))
        props = data.get("properties") or {}
        for field in (
            "doc_id",
            "page_index",
            "triage_class",
            "confidence",
            "classifier_id",
        ):
            if field not in props:
                errors.append(f"page_triage schema missing {field}")
        enum = set((props.get("triage_class") or {}).get("enum") or [])
        if enum != CLASSES:
            errors.append(f"triage_class enum mismatch: {enum}")

    if DOCPIPE_SCHEMA.exists():
        text = DOCPIPE_SCHEMA.read_text(encoding="utf-8")
        for m in ("docpipe.ping", "docpipe.version", "docpipe.triage_pages"):
            if m not in text:
                errors.append(f"docpipe schema missing method {m}")

    if MIG.exists():
        text = MIG.read_text(encoding="utf-8")
        if "CREATE TABLE IF NOT EXISTS page_triage" not in text:
            errors.append("migration missing page_triage table")

    if LIB_RS.exists():
        text = LIB_RS.read_text(encoding="utf-8")
        for cmd in (
            "docpipe_triage_start",
            "docpipe_triage_case",
            "docpipe_triage_status",
            "docpipe_page_list",
            "docpipe_meta",
        ):
            if f"fn {cmd}" not in text:
                errors.append(f"lib.rs missing {cmd}")

    if MOD_RS.exists():
        text = MOD_RS.read_text(encoding="utf-8")
        for name in (
            "page_triage_replace",
            "page_triage_list",
            "case_004_page_triage",
            "doc_content_bytes",
        ):
            if name not in text:
                errors.append(f"vault/mod.rs missing {name}")

    if IPC_TS.exists():
        text = IPC_TS.read_text(encoding="utf-8")
        for name in (
            "docpipeTriageStart",
            "docpipeTriageCase",
            "docpipeTriageStatus",
            "docpipePageList",
            "onDocpipeTriageProgress",
        ):
            if name not in text:
                errors.append(f"ipc.ts missing {name}")

    if APP.exists():
        text = APP.read_text(encoding="utf-8")
        if "Document triage" not in text:
            errors.append("App.tsx missing Document triage UI")
        if "runTriage" not in text and "docpipeTriageStart" not in text:
            errors.append("App.tsx missing triage action")

    # Sidecar JSON-RPC round-trip (no models)
    if DOCPIPE_PY.exists() and TRIAGE_PY.exists():
        try:
            proc = subprocess.run(
                [sys.executable, str(DOCPIPE_PY)],
                input=json.dumps(
                    {"jsonrpc": "2.0", "id": 1, "method": "docpipe.ping", "params": {}}
                )
                + "\n"
                + json.dumps(
                    {
                        "jsonrpc": "2.0",
                        "id": 2,
                        "method": "docpipe.triage_pages",
                        "params": {
                            "text": "[CASEFORGE_TRIAGE_CLASS:messy_handwriting]\nmessy sample"
                        },
                    }
                )
                + "\n",
                capture_output=True,
                text=True,
                cwd=str(DOCPIPE_PY.parent),
                timeout=15,
            )
            lines = [ln for ln in (proc.stdout or "").splitlines() if ln.strip()]
            if proc.returncode != 0 or len(lines) < 2:
                errors.append(
                    f"docpipe sidecar failed rc={proc.returncode} out={proc.stdout!r} err={proc.stderr!r}"
                )
            else:
                ping = json.loads(lines[0])
                tri = json.loads(lines[1])
                if not ping.get("result", {}).get("ok"):
                    errors.append(f"docpipe.ping failed: {ping}")
                pages = (tri.get("result") or {}).get("pages") or []
                if not pages or pages[0].get("triage_class") != "messy_handwriting":
                    errors.append(f"docpipe.triage_pages unexpected: {tri}")
        except Exception as e:
            errors.append(f"docpipe sidecar exception: {e}")

    # Synthetic marker fixtures: heuristic must match labels (plumbing only)
    if GDOCS.exists() and TRIAGE_PY.exists():
        sys.path.insert(0, str(TRIAGE_PY.parent))
        from triage import pages_from_document, triage_pages  # type: ignore

        matched = 0
        total = 0
        for fixture_dir in sorted(GDOCS.iterdir()):
            if not fixture_dir.is_dir():
                continue
            meta_path = fixture_dir / "meta.json"
            ans_path = fixture_dir / "known_answer.json"
            if not meta_path.exists() or not ans_path.exists():
                continue
            meta = json.loads(meta_path.read_text(encoding="utf-8-sig"))
            ans = json.loads(ans_path.read_text(encoding="utf-8-sig"))
            status = meta.get("status") or ans.get("status")
            # Only score marker-driven synthetic fixtures for plumbing accuracy
            if status not in {"SYNTHETIC_PLACEHOLDER", "SYNTHETIC_DEVELOPMENT"}:
                continue
            expected = (ans.get("triage") or {}).get("class")
            if not expected:
                continue
            page_file = fixture_dir / "page_stub.txt"
            text = page_file.read_text(encoding="utf-8") if page_file.exists() else ""
            pages = pages_from_document(text=text, media_type="text/plain")
            results = triage_pages(pages)
            total += 1
            if results and results[0]["triage_class"] == expected:
                matched += 1
            else:
                errors.append(
                    f"fixture {fixture_dir.name}: expected {expected}, got {results}"
                )
        if total == 0:
            errors.append("no synthetic G-DOCS fixtures found to score")
        # Plumbing fixtures must all match markers
        elif matched != total:
            errors.append(f"synthetic fixture accuracy {matched}/{total}")

    if errors:
        print("FAIL  B-S1-05 docpipe triage structural")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S1-05 docpipe triage structural "
        "(classes, IPC, sidecar ping, synthetic markers)"
    )
    print(
        "NOTE  Ship accuracy is scored separately: "
        "python eval/checks/ac_6_1_triage_score.py (GOLDEN only)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
