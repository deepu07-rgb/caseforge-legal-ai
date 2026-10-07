#!/usr/bin/env python3
"""B-S1-06 structural checks: recognition + confidence thresholds from contracts."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "page_text_layer.schema.json"
DOCPIPE_SCHEMA = ROOT / "contracts" / "ipc" / "docpipe.schema.json"
RECOG_PY = ROOT / "sidecars" / "docpipe" / "recognize.py"
DOCPIPE_PY = ROOT / "sidecars" / "docpipe" / "docpipe.py"
RECOG_RS = ROOT / "src-tauri" / "src" / "docpipe" / "recognize.rs"
MIG = ROOT / "src-tauri" / "migrations" / "case_005_page_text_layer.sql"
LIB_RS = ROOT / "src-tauri" / "src" / "lib.rs"
VAULT_MOD = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
IPC_TS = ROOT / "src" / "lib" / "ipc.ts"
APP = ROOT / "src" / "App.tsx"

REQUIRED_THRESHOLDS = {
    "word_highlight_below",
    "word_force_review_below",
    "word_suppress_below",
    "page_mean_warn_below",
}


def main() -> int:
    errors: list[str] = []

    for p in (
        CONST,
        SCHEMA,
        DOCPIPE_SCHEMA,
        RECOG_PY,
        DOCPIPE_PY,
        RECOG_RS,
        MIG,
        LIB_RS,
        VAULT_MOD,
        IPC_TS,
        APP,
    ):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # Thresholds frozen
    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        block = data.get("confidence_thresholds") or {}
        if block.get("status") != "FROZEN":
            errors.append("confidence_thresholds.status must be FROZEN")
        values = block.get("values") or {}
        for k in REQUIRED_THRESHOLDS:
            if k not in values or values[k] is None:
                errors.append(f"missing threshold {k}")
            else:
                v = float(values[k])
                if not (0.0 <= v <= 1.0):
                    errors.append(f"threshold {k} out of [0,1]: {v}")

    if SCHEMA.exists():
        schema = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        for field in ("words", "mean_confidence", "full_text", "recognizer_id"):
            if field not in (schema.get("properties") or {}):
                errors.append(f"page_text_layer schema missing {field}")

    if DOCPIPE_SCHEMA.exists():
        text = DOCPIPE_SCHEMA.read_text(encoding="utf-8")
        if "docpipe.recognize_pages" not in text:
            errors.append("docpipe schema missing docpipe.recognize_pages")

    if MIG.exists():
        text = MIG.read_text(encoding="utf-8")
        if "page_text_layer" not in text or "page_words" not in text:
            errors.append("migration missing page_text_layer / page_words")

    for path, needle in (
        (LIB_RS, "docpipe_recognize_start"),
        (LIB_RS, "docpipe_text_layer_list"),
        (VAULT_MOD, "page_text_layer_replace"),
        (RECOG_RS, "RECOGNIZER_ID"),
        (IPC_TS, "docpipeRecognizeStart"),
        (IPC_TS, "PageTextLayerRow"),
        (APP, "Text layer"),
        (APP, "low_confidence"),
    ):
        if path.exists() and needle not in path.read_text(encoding="utf-8"):
            errors.append(f"{path.relative_to(ROOT)} missing {needle}")

    # Live Python recognizer + thresholds application
    if RECOG_PY.exists() and CONST.exists():
        sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
        from recognize import load_thresholds, recognize_text  # type: ignore

        thr = load_thresholds()
        for k in REQUIRED_THRESHOLDS:
            if k not in thr:
                errors.append(f"load_thresholds missing {k}")
        sample = "The plaintiff filed a petition on date mar??? unclear."
        layer = recognize_text(sample, thresholds=thr)
        if not layer["words"]:
            errors.append("recognize_text produced no words")
        low = [w for w in layer["words"] if w["low_confidence"]]
        force = [w for w in layer["words"] if w["force_review"]]
        # mar??? should force review
        if not any("???" in w["text"] for w in force):
            errors.append("expected ??? token to force_review under frozen thresholds")
        if not low:
            # still ok if all high; but ??? should be low
            if not any("???" in w["text"] for w in low):
                errors.append("expected ??? token to be low_confidence")
        # flags must match thresholds (strict below)
        hi = thr["word_highlight_below"]
        fr = thr["word_force_review_below"]
        for w in layer["words"]:
            expect_low = w["confidence"] < hi
            expect_fr = w["confidence"] < fr
            if w["low_confidence"] != expect_low:
                errors.append(f"low_confidence flag mismatch for {w['text']!r}")
            if w["force_review"] != expect_fr:
                errors.append(f"force_review flag mismatch for {w['text']!r}")

    # Sidecar RPC smoke
    if DOCPIPE_PY.exists():
        req = json.dumps(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "docpipe.recognize_pages",
                "params": {"text": "Counsel notes ??? unclear date", "media_type": "text/plain"},
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
            errors.append(f"docpipe recognize RPC exit {proc.returncode}: {proc.stderr}")
        else:
            line = (proc.stdout or "").strip().splitlines()
            if not line:
                errors.append("docpipe recognize RPC empty stdout")
            else:
                resp = json.loads(line[0])
                if "error" in resp:
                    errors.append(f"docpipe recognize RPC error: {resp['error']}")
                else:
                    pages = (resp.get("result") or {}).get("pages") or []
                    if not pages or not pages[0].get("words"):
                        errors.append("docpipe.recognize_pages returned no words")

    if errors:
        print("FAIL  B-S1-06 docpipe recognition structural")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S1-06 docpipe recognition structural "
        "(thresholds frozen, schema, vault, IPC, word flags, sidecar RPC)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
