#!/usr/bin/env python3
"""Fail on invalid UTF-8, replacement characters, or common mojibake in production text."""

from __future__ import annotations

import argparse
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SCAN_ROOTS = [
    "src", "src-tauri/src", "src-tauri/migrations", "sidecars", "contracts", "docs",
    "README.md", "CHANGELOG.md", "CaseForge_PRD_v1.0.md", "CaseForge_Dev_Batches_v1.0.md",
]
TEXT_SUFFIXES = {
    ".css", ".html", ".js", ".json", ".md", ".mjs", ".py", ".rs", ".sql",
    ".toml", ".ts", ".tsx", ".txt", ".xml", ".yml", ".yaml",
}
EXCLUDED_PARTS = {"node_modules", "dist", "target", "__pycache__", ".git"}
MOJIBAKE = ("â€", "â†", "â‰", "Â§", "Â·", "Â©", "Â®", "ï»¿", "Ã")


def iter_files() -> list[Path]:
    files: list[Path] = []
    for item in SCAN_ROOTS:
        path = ROOT / item
        if path.is_file():
            files.append(path)
        elif path.is_dir():
            files.extend(
                p for p in path.rglob("*")
                if p.is_file()
                and p.suffix.lower() in TEXT_SUFFIXES
                and not any(part in EXCLUDED_PARTS for part in p.parts)
            )
    return sorted(set(files))


def load_allowlist() -> dict[str, dict[str, object]]:
    path = ROOT / "contracts" / "encoding_allowlist.json"
    data = json.loads(path.read_text(encoding="utf-8"))
    out: dict[str, dict[str, object]] = {}
    for entry in data.get("entries", []):
        reason = str(entry.get("reason", "")).strip()
        if not reason:
            raise ValueError(f"Encoding allowlist entry lacks reason: {entry}")
        out[str(entry["path"]).replace("\\", "/")] = entry
    return out


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    allowlist = load_allowlist()
    failures: list[dict[str, object]] = []
    scanned = 0
    for path in iter_files():
        scanned += 1
        rel = path.relative_to(ROOT).as_posix()
        # The policy file necessarily contains the exact forbidden signatures it permits.
        if rel == "contracts/encoding_allowlist.json":
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="strict")
        except UnicodeDecodeError as exc:
            failures.append({"path": rel, "line": None, "token": "invalid UTF-8", "detail": str(exc)})
            continue
        allowed = set(allowlist.get(rel, {}).get("substrings", []))
        for line_no, line in enumerate(text.splitlines(), 1):
            tokens = [token for token in MOJIBAKE if token in line]
            if "\ufffd" in line:
                tokens.append("replacement character U+FFFD")
            for token in tokens:
                literal = "\ufffd" if token.startswith("replacement") else token
                if literal in allowed or any(literal in item and item in line for item in allowed):
                    continue
                failures.append({"path": rel, "line": line_no, "token": token})
    result = {"batch": "B-S3-01", "scanned_files": scanned, "failures": failures, "pass": not failures}
    if args.json:
        print(json.dumps(result, indent=2, ensure_ascii=False))
    elif failures:
        for failure in failures:
            print(f"FAIL {failure['path']}:{failure.get('line') or '-'} {failure['token']}")
        print(f"FAIL B-S3-01 encoding scan ({len(failures)} issue(s), {scanned} files)")
    else:
        print(f"PASS B-S3-01 UTF-8/mojibake scan ({scanned} files; explicit allowlist enforced)")
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
