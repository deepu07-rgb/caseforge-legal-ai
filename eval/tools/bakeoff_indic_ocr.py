#!/usr/bin/env python3
"""Hindi OCR bake-off (B-S4-14): which model and mode reads the G-INDIC pages best.

Runs each candidate over the eight G-INDIC pages through the same preprocessing the product uses, scores it against the exact transcription
(character and word error rate after Unicode NFC and whitespace normalisation), and applies the selection rule written in
contracts/ocr_config_indic.json: among the Hindi+English candidates, the lowest mean character error on the degraded pages, ties broken by
the clean pages. The report is evidence for the owner's decision Q-S4-04, not the decision.

The two Hindi models are official Tesseract models (Apache-2.0) and are NOT kept in the repository except the chosen one, so this tool takes
the other one by path:  python eval/tools/bakeoff_indic_ocr.py --fast <tessdata_fast hin.traineddata> --best <tessdata_best hin.traineddata>

Honest limits are written into the report: rendered pages are cleaner than photographed scans, one typeface family.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import shutil
import sys
import tempfile
import time
import unicodedata
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))

import recognize as R  # noqa: E402

REPORT = ROOT / "eval" / "bakeoff" / "indic_ocr_b_s4_14.json"
FIXTURES = ROOT / "golden" / "G-INDIC" / "fixtures"


def levenshtein(a, b) -> int:
    prev = list(range(len(b) + 1))
    for i, x in enumerate(a, 1):
        cur = [i]
        for j, y in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (x != y)))
        prev = cur
    return prev[-1]


def norm(s: str) -> str:
    return " ".join(unicodedata.normalize("NFC", s).split())


def make_tessdata(root: Path, name: str, hin: Path) -> Path:
    """A tessdata folder with English, OSD, the configs, and one Hindi model."""
    src = R.find_tesseract().parent / "tessdata"
    out = root / name
    out.mkdir(parents=True)
    for f in ("eng.traineddata", "osd.traineddata"):
        shutil.copy(src / f, out / f)
    for d in ("configs", "tessconfigs"):
        if (src / d).is_dir():
            shutil.copytree(src / d, out / d)
    shutil.copy(hin, out / "hin.traineddata")
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fast", type=Path, required=True)
    ap.add_argument("--best", type=Path, required=True)
    ap.add_argument("--out", type=Path, default=REPORT)
    args = ap.parse_args()
    exe = R.find_tesseract()
    thresholds = R.load_thresholds()
    pages = sorted(d for d in FIXTURES.iterdir() if d.is_dir())
    candidates = [
        ("hin-fast", "fast", "hin", 3), ("hin-best", "best", "hin", 3),
        ("hin+eng-fast", "fast", "hin+eng", 3), ("hin+eng-best", "best", "hin+eng", 3), ("hin+eng-best-psm6", "best", "hin+eng", 6),
        ("eng-only (current path)", "fast", "eng", 3),
    ]
    results: dict[str, list[dict]] = {}
    with tempfile.TemporaryDirectory() as tmp_name:
        tmp = Path(tmp_name)
        tessdata = {"fast": make_tessdata(tmp, "fast", args.fast), "best": make_tessdata(tmp, "best", args.best)}
        processed_cache: dict[str, tuple[Image.Image, bytes]] = {}
        for d in pages:
            img = Image.open(d / "page.png").convert("RGB")
            processed, _ = R.preprocess_image(img, exe)
            processed_cache[d.name] = (processed, R._png_bytes(processed))
        for name, family, lang, psm in candidates:
            rows = []
            for d in pages:
                truth = json.loads((d / "truth.json").read_text(encoding="utf-8"))
                processed, png = processed_cache[d.name]
                command = [str(exe), "stdin", "stdout", "--tessdata-dir", str(tessdata[family]), "-l", lang, "--oem", "1", "--psm", str(psm), "tsv"]
                started = time.perf_counter()
                proc, rss = R._run_engine(command, png, 180)
                seconds = time.perf_counter() - started
                text, _words = R._parse_tsv(proc.stdout.decode("utf-8", "replace"), processed.width, processed.height, thresholds)
                ref, hyp = norm(truth["text"]), norm(text)
                rows.append({
                    "page": d.name, "cer": round(levenshtein(ref, hyp) / max(1, len(ref)), 4),
                    "wer": round(levenshtein(ref.split(), hyp.split()) / max(1, len(ref.split())), 4),
                    "seconds": round(seconds, 2), "peak_rss_mb": round(rss / 1e6),
                })
            results[name] = rows

    def mean(rows: list[dict], key: str, pick) -> float:
        chosen = [r[key] for r in rows if pick(r["page"])]
        return round(sum(chosen) / len(chosen), 4)

    is_clean = lambda p: p.startswith(("hi_", "mixed")) and "degraded" not in p  # noqa: E731
    is_degraded = lambda p: "degraded" in p  # noqa: E731
    is_latin = lambda p: p.startswith("latin")  # noqa: E731
    summary = {}
    for name, rows in results.items():
        summary[name] = {
            "clean_cer": mean(rows, "cer", is_clean), "degraded_cer": mean(rows, "cer", is_degraded), "latin_control_cer": mean(rows, "cer", is_latin),
            "clean_wer": mean(rows, "wer", is_clean), "seconds_per_page": round(sum(r["seconds"] for r in rows) / len(rows), 2),
            "max_peak_rss_mb": max(r["peak_rss_mb"] for r in rows),
        }
    eligible = [n for n in summary if n.startswith("hin+eng") and not n.endswith("psm6")]
    chosen = min(eligible, key=lambda n: (summary[n]["degraded_cer"], summary[n]["clean_cer"]))
    report = {
        "batch": "B-S4-14", "tool": "eval/tools/bakeoff_indic_ocr.py",
        "engine": R.runtime_manifest()["engine_version"],
        "models": {"tessdata_fast hin": hashlib.sha256(args.fast.read_bytes()).hexdigest(), "tessdata_best hin": hashlib.sha256(args.best.read_bytes()).hexdigest()},
        "model_sizes_bytes": {"tessdata_fast hin": args.fast.stat().st_size, "tessdata_best hin": args.best.stat().st_size},
        "selection_rule": "among hin+eng candidates at psm 3: lowest mean degraded-page CER, ties by clean-page CER",
        "selected": chosen,
        "summary": summary,
        "pages": results,
        "limits": "Rendered pages are cleaner than photographed scans and use one typeface family (Nirmala UI); these error rates are a floor. The latin_control column is the English page read by a Hindi-capable model, shown only to confirm routing matters - the product reads Latin pages with the frozen English path.",
        "decision_owner": "Q-S4-04 (OCR and translation model choice) - this report informs it",
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    width = max(len(n) for n in summary)
    for name, s in summary.items():
        print(f"{name:{width}}  clean CER {s['clean_cer']:.4f}  degraded CER {s['degraded_cer']:.4f}  latin {s['latin_control_cer']:.4f}  clean WER {s['clean_wer']:.3f}  {s['seconds_per_page']}s  {s['max_peak_rss_mb']} MB")
    print("selected:", chosen)
    return 0


if __name__ == "__main__":
    sys.exit(main())
