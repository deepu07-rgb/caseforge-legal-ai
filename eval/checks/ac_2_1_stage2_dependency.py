#!/usr/bin/env python3
"""AC-2.1 — Stage 2 modules must not import into the document pipeline.

Stage 2 consumes only case_graph.json. No Stage 2 module may parse raw documents.
Static audit: zero Stage 2 imports into docpipe / document intake path.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

# Document pipeline roots (must stay free of Stage 2 product modules).
DOC_PIPELINE_GLOBS = [
    "src-tauri/src/docpipe/**/*.rs",
    "sidecars/docpipe/**/*.py",
]

# Forbidden Stage 2 identifiers inside document pipeline sources.
FORBIDDEN_IN_DOCPIPE = [
    r"\bmod\s+mock_court\b",
    r"\buse\s+crate::mock_court\b",
    r"\bcrate::mock_court\b",
    r"\bcrate::integrity\b",
    r"\bcrate::adaptive_bench\b",
    r"\bcrate::voice_court\b",
    r"\bcrate::bench_prep\b",
    r"\bMockCourt\b",
    r"\bguardrails_redteam_full\b",
    r"\bBenchPrep\b",
    r"\bintegrity_engine\b",
    r"\bArgumentTree\b",
    r"\badaptive_bench\b",
    r"\bparse_raw_pdf_for_stage2\b",
    r"from\s+mock_court\s+import",
    r"import\s+mock_court\b",
]

# Allowed mentions (out_of_scope lists, comments that forbid Stage 2).
ALLOWED_CONTEXT = [
    "out_of_scope",
    "argument_tree",  # listed as out_of_scope string in docpipe meta
    "Stage 2",
    "forbidden",
    "never",
]


def iter_files() -> list[Path]:
    files: list[Path] = []
    for g in DOC_PIPELINE_GLOBS:
        files.extend(ROOT.glob(g))
    return [p for p in files if p.is_file()]


def main() -> int:
    errors: list[str] = []
    files = iter_files()
    if not files:
        errors.append("no docpipe sources found")

    for path in files:
        text = path.read_text(encoding="utf-8", errors="replace")
        for pat in FORBIDDEN_IN_DOCPIPE:
            for m in re.finditer(pat, text):
                # line context
                line_start = text.rfind("\n", 0, m.start()) + 1
                line_end = text.find("\n", m.end())
                if line_end < 0:
                    line_end = len(text)
                line = text[line_start:line_end]
                # Allow if clearly in out_of_scope / forbid comment
                lower = line.lower()
                if any(a.lower() in lower for a in ("out_of_scope", "forbid", "never parse", "not imported")):
                    continue
                if "argument_tree" in pat and "out_of_scope" in text:
                    # string listed as out of scope is OK
                    if "out_of_scope" in text[max(0, m.start() - 200) : m.end() + 50]:
                        continue
                rel = path.relative_to(ROOT)
                errors.append(f"AC-2.1: {rel} matches forbidden Stage-2 pattern {pat!r}: {line.strip()[:120]}")

    # Positive: docpipe must declare argument_tree out of scope or similar
    docpipe_mod = ROOT / "src-tauri" / "src" / "docpipe" / "mod.rs"
    if docpipe_mod.exists():
        t = docpipe_mod.read_text(encoding="utf-8")
        if "argument_tree" not in t and "out_of_scope" not in t:
            errors.append("docpipe should document Stage-2 argument_tree as out_of_scope")

    # UI may have Mock Court for preview, but doc pipeline must not depend on it.
    # Ensure no import from docpipe into frontend MockCourt (inverse is fine).
    for p in (ROOT / "src-tauri" / "src" / "docpipe").rglob("*.rs"):
        t = p.read_text(encoding="utf-8", errors="replace")
        if "MockCourtView" in t or "components/MockCourt" in t:
            errors.append(f"AC-2.1: {p.relative_to(ROOT)} must not reference MockCourt UI")

    if errors:
        print("FAIL  AC-2.1 Stage-2 dependency audit")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        f"PASS  AC-2.1 Stage-2 dependency audit "
        f"({len(files)} docpipe files; zero Stage-2 imports into document pipeline)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
