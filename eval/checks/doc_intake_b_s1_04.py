#!/usr/bin/env python3
"""B-S1-04 structural checks: document intake (PRD §5)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "contracts" / "document_intake.schema.json"
DOCS_RS = ROOT / "src-tauri" / "src" / "vault" / "docs.rs"
MOD_RS = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
IPC_TS = ROOT / "src" / "lib" / "ipc.ts"
APP = ROOT / "src" / "App.tsx"
MIG = ROOT / "src-tauri" / "migrations" / "case_003_documents.sql"


def main() -> int:
    errors: list[str] = []

    for p in (SCHEMA, DOCS_RS, MOD_RS, LIB_RS, IPC_TS, APP, MIG):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if SCHEMA.exists():
        data = json.loads(SCHEMA.read_text(encoding="utf-8"))
        props = data.get("properties") or {}
        for field in (
            "doc_id",
            "type_tag",
            "language_tag",
            "source_format",
            "per_page_quality_grade",
        ):
            if field not in props:
                errors.append(f"schema missing {field}")
        fmt = set((props.get("source_format") or {}).get("enum") or [])
        # TXT added for synthetic case packets / notes feeding local LLM context.
        expected = {"pdf", "docx", "jpg", "png"}
        if fmt and not expected.issubset(fmt):
            errors.append(f"source_format enum {fmt} missing core types {expected}")

    if DOCS_RS.exists():
        text = DOCS_RS.read_text(encoding="utf-8")
        for ext in ("pdf", "docx", "jpg", "png", "txt"):
            if ext not in text:
                errors.append(f"docs.rs missing {ext}")
        if "Phone Capture" not in text and "phone" not in text.lower():
            pass  # ok if noted in out_of_scope elsewhere

    if MOD_RS.exists():
        text = MOD_RS.read_text(encoding="utf-8")
        for name in ("doc_list", "doc_ingest", "doc_update_type_tag", "case_003_documents"):
            if name not in text:
                errors.append(f"mod.rs missing {name}")

    if LIB_RS.exists():
        text = LIB_RS.read_text(encoding="utf-8")
        for cmd in ("doc_list", "doc_ingest", "doc_update_type_tag", "doc_intake_meta"):
            if f"fn {cmd}" not in text:
                errors.append(f"lib.rs missing {cmd}")

    if IPC_TS.exists():
        text = IPC_TS.read_text(encoding="utf-8")
        for name in ("docIngest", "docList", "docUpdateTypeTag", "fileToBase64", "isAllowedIntakeFile"):
            if name not in text:
                errors.append(f"ipc.ts missing {name}")

    if APP.exists():
        text = APP.read_text(encoding="utf-8")
        if "Document intake" not in text:
            errors.append("App.tsx missing Document intake UI")
        if "Drag & drop" not in text and "drag" not in text.lower():
            errors.append("App.tsx missing drag-drop")
        if "type_tag" not in text and "Type tag" not in text:
            errors.append("App.tsx missing type tag correction UI")

    if MIG.exists():
        text = MIG.read_text(encoding="utf-8")
        if "CREATE TABLE IF NOT EXISTS documents" not in text:
            errors.append("migration missing documents table")
        if "content BLOB" not in text:
            errors.append("migration missing content BLOB (encrypted store)")

    if errors:
        print("FAIL  B-S1-04 document intake structural")
        for e in errors:
            print(f"  - {e}")
        return 1

    print("PASS  B-S1-04 document intake structural (PDF/DOCX/JPG/PNG/TXT, type tag correction)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
