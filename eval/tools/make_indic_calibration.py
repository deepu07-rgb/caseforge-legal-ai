#!/usr/bin/env python3
"""Builds the Hindi OCR confidence calibration (B-S4-14), by the same method as the English one (B-S3-05): word-level (raw engine confidence,
was the word right) pairs, a monotonic (isotonic) map fitted on the training pages, error measured on held-out pages.

Everything here is synthetic: Hindi sentences made from templates and invented names, rendered with Windows text shaping, degraded from fixed
seeds, recognized by the shipped Hindi route, and each recognized word is marked right or wrong by aligning it with the text that was rendered
(so the truth is exact). Training and held-out pages share no template, no name and no place, and neither shares a line with the G-INDIC golden
pages the accuracy gate is measured on.

What this is not: calibration on real scans. One typeface family, rendered text, synthetic degradation. The map says how far to trust a word
on pages like these; on a real photographed scan it can be over-confident, which is why the calibrated value is only ever used to HOLD words for
review (never to accept one automatically beyond the existing thresholds) and why the owner approves it (Q-S4-04) before it is final.

Usage (Windows): python eval/tools/make_indic_calibration.py
"""
from __future__ import annotations

import difflib
import hashlib
import io
import json
import random
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "eval" / "tools"))
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))

import make_indic_golden as G  # noqa: E402  (degrade())
import recognize as R  # noqa: E402
from confidence import INDIC_CALIBRATION_ID, INDIC_ENGINE_CLASS  # noqa: E402

DATASET = ROOT / "eval" / "calibration" / "indic_ocr_calibration_v1.json"
ARTIFACT = ROOT / "eval" / "calibration" / "confidence_calibration_indic_v1.json"
DEV = "०१२३४५६७८९"
MONTHS = ["जनवरी", "फरवरी", "मार्च", "अप्रैल", "मई", "जून", "जुलाई", "अगस्त", "सितंबर", "अक्टूबर", "नवंबर", "दिसंबर"]

TRAIN = {
    "templates": [
        "{n} ने दिनांक {d} को {p} थाने में शिकायत दर्ज कराई।",
        "वाद संख्या {c} में अगली सुनवाई {d} को होगी।",
        "प्रतिवादी {n} से रुपये {a} की वसूली का दावा किया गया है।",
        "धारा {s} के अंतर्गत {n} के विरुद्ध मामला पंजीकृत किया गया।",
        "गवाह {n} ने न्यायालय में बयान दिया कि घटना {p} में हुई थी।",
        "न्यायालय ने {n} को ₹ {a} जमा करने का निर्देश दिया।",
        "अभियोजन पक्ष ने दिनांक {d} तक का समय मांगा है।",
        "{p} के निवासी {n} ने भूमि विवाद की अर्जी प्रस्तुत की।",
    ],
    "names": ["श्री हरीश चंद्र", "श्रीमती कमला देवी", "श्री दिनेश यादव", "श्री प्रकाश मिश्रा", "श्रीमती रेखा गुप्ता", "श्री मनोज तिवारी", "श्री अनिल कपूर", "श्रीमती सुनीता पांडेय"],
    "places": ["रामगढ़", "सुंदरनगर", "हरिपुर", "लक्ष्मीपुर"],
}
HELD = {
    "templates": [
        "आवेदक {n} की ओर से जमानत प्रार्थना पत्र दिनांक {d} को प्रस्तुत हुआ।",
        "अपराध क्रमांक {c} की विवेचना {p} के उपनिरीक्षक द्वारा की जा रही है।",
        "पक्षकार {n} को धारा {s} के अंतर्गत नोटिस जारी किया जाता है।",
        "विपक्षी ने ₹ {a} की रसीद न्यायालय में दाखिल की।",
        "{n} द्वारा प्रस्तुत दस्तावेज़ों का परीक्षण {p} में किया गया।",
        "आदेश की प्रति दिनांक {d} को पक्षकारों को उपलब्ध कराई जाए।",
    ],
    "names": ["श्री राजेश खन्ना", "श्रीमती पूनम सक्सेना", "श्री संजय दुबे", "श्री कमल किशोर", "श्रीमती अनीता जोशी", "श्री भूपेंद्र सिंह"],
    "places": ["शिवगंज", "कृष्णानगर", "गोपालपुर"],
}
# (name, degradation) - from none to harsh, so the raw confidences spread out and there are errors to learn from
LEVELS = [
    ("clean", None),
    ("light", {"blur": 0.6, "noise": 8, "contrast": 0.9}),
    ("medium", {"rotate": 0.8, "blur": 1.0, "noise": 16, "contrast": 0.8}),
    ("heavy", {"rotate": -1.5, "blur": 1.4, "noise": 24, "contrast": 0.7, "jpeg": 40}),
    ("harsh", {"rotate": 1.0, "blur": 1.8, "noise": 32, "contrast": 0.6, "jpeg": 30}),
]
FONTS = [("Nirmala UI", False), ("Nirmala UI Semilight", False), ("Nirmala UI", True)]
SIZES = [30, 34, 38, 42]


def dev(n: int | str) -> str:
    return "".join(DEV[int(c)] if c.isdigit() else c for c in str(n))


def indian(n: int) -> str:
    s = str(n)
    if len(s) <= 3:
        return dev(s)
    head, tail = s[:-3], s[-3:]
    parts: list[str] = []
    while len(head) > 2:
        parts.insert(0, head[-2:])
        head = head[:-2]
    if head:
        parts.insert(0, head)
    return dev(",".join(parts + [tail]))


def sentence(rng: random.Random, pool: dict) -> str:
    t = rng.choice(pool["templates"])
    date = rng.choice([
        f"{dev(rng.randint(13, 28))}/{dev(rng.randint(1, 12))}/{dev(rng.randint(2019, 2024))}",
        f"{dev(rng.randint(1, 28))} {rng.choice(MONTHS)} {dev(rng.randint(2019, 2024))}",
    ])
    return t.format(n=rng.choice(pool["names"]), p=rng.choice(pool["places"]), d=date, c=f"{dev(rng.randint(10, 999))}/{dev(rng.randint(2019, 2024))}",
                    a=indian(rng.choice([4500, 12000, 75000, 125000, 250000, 1500000])), s=dev(rng.choice([302, 34, 420, 80, 354, 498])))


def make_pages(pool: dict, count: int, seed: int, prefix: str) -> list[dict]:
    rng = random.Random(seed)
    pages = []
    for i in range(count):
        level, degrade = LEVELS[i % len(LEVELS)]
        font, bold = FONTS[i % len(FONTS)]
        lines = [sentence(rng, pool) for _ in range(8)]
        pages.append({"id": f"{prefix}{i + 1:02d}", "level": level, "degrade": ({**degrade, "seed": seed * 100 + i} if degrade else None), "font": font, "bold": bold,
                      "size": SIZES[(i // 2) % len(SIZES)], "lines": lines})
    return pages


def norm(s: str) -> str:
    return unicodedata.normalize("NFC", s)


def label_words(truth_text: str, ocr_words: list[dict]) -> list[dict]:
    """Right/wrong for every recognized word: a word is right when it sits in an exactly matching run of the aligned token sequences."""
    truth = norm(truth_text).split()
    ocr = [norm(w["text"]) for w in ocr_words]
    right = [False] * len(ocr)
    for tag, _i1, _i2, j1, j2 in difflib.SequenceMatcher(None, truth, ocr, autojunk=False).get_opcodes():
        if tag == "equal":
            for j in range(j1, j2):
                right[j] = True
    return [{"raw": round(float(w["raw_engine_confidence"]), 4), "correct": int(r)} for w, r in zip(ocr_words, right)]


def render_and_recognize(pages: list[dict]) -> dict[str, dict]:
    out: dict[str, dict] = {}
    with tempfile.TemporaryDirectory() as tmp_name:
        tmp = Path(tmp_name)
        spec = [{"id": p["id"], "font": p["font"], "bold": p["bold"], "size_px": p["size"], "lines": p["lines"]} for p in pages]
        (tmp / "spec.json").write_text(json.dumps(spec, ensure_ascii=False), encoding="utf-8")
        subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(ROOT / "eval" / "tools" / "render_pages.ps1"), "-Spec", str(tmp / "spec.json"), "-OutDir", str(tmp / "out")],
                       check=True, capture_output=True)
        exe = R.find_tesseract()
        thresholds = R.load_thresholds()
        for p in pages:
            img = Image.open(tmp / "out" / f"{p['id']}.png").convert("L")
            if p["degrade"]:
                img = G.degrade(img, p["degrade"])
            png = R._png_bytes(img)
            processed, _ = R.preprocess_image(Image.open(io.BytesIO(png)).convert("RGB"), exe)
            attempt = R._recognize_pass(R.ROUTE_INDIC, processed, R._png_bytes(processed), exe, thresholds)
            out[p["id"]] = {"page_sha256": hashlib.sha256(png).hexdigest(), "words": attempt["words"]}
    return out


# ---------- fitting ----------

def isotonic_bins(rows: list[dict], max_bins: int = 8, min_count: int = 20) -> list[dict]:
    """Pool-adjacent-violators on the training pairs, then merge thin or near-equal neighbours into at most max_bins bins."""
    rows = sorted(rows, key=lambda r: r["raw"])
    blocks: list[dict] = []
    for r in rows:
        if blocks and blocks[-1]["hi"] == r["raw"]:
            b = blocks[-1]
            b["n"] += 1
            b["k"] += r["correct"]
            continue
        blocks.append({"lo": r["raw"], "hi": r["raw"], "n": 1, "k": r["correct"]})

    def mean(b: dict) -> float:
        return b["k"] / b["n"]

    def merge(i: int) -> None:
        a, b = blocks[i], blocks.pop(i + 1)
        a["hi"], a["n"], a["k"] = b["hi"], a["n"] + b["n"], a["k"] + b["k"]

    changed = True
    while changed:
        changed = False
        for i in range(len(blocks) - 1):
            if mean(blocks[i]) > mean(blocks[i + 1]):
                merge(i)
                changed = True
                break
    while len(blocks) > 1:
        thin = [i for i, b in enumerate(blocks) if b["n"] < min_count]
        if not thin and len(blocks) <= max_bins:
            break
        if thin:
            i = thin[0]
            if i == 0:
                j = 0
            elif i == len(blocks) - 1:
                j = i - 1
            else:
                j = i - 1 if abs(mean(blocks[i]) - mean(blocks[i - 1])) <= abs(mean(blocks[i]) - mean(blocks[i + 1])) else i
        else:
            j = min(range(len(blocks) - 1), key=lambda t: mean(blocks[t + 1]) - mean(blocks[t]))
        merge(j)
    bins = []
    for i, b in enumerate(blocks):
        lo = 0.0 if i == 0 else round((blocks[i - 1]["hi"] + b["lo"]) / 2, 4)
        hi = 1.000001 if i == len(blocks) - 1 else round((b["hi"] + blocks[i + 1]["lo"]) / 2, 4)
        value = (b["k"] + 0.5) / (b["n"] + 1.0)
        bins.append({"raw_min": lo, "raw_max": hi, "calibrated": round(min(0.995, max(0.01, value)), 4), "training_words": b["n"]})
    for i in range(1, len(bins)):
        bins[i]["calibrated"] = max(bins[i]["calibrated"], bins[i - 1]["calibrated"])
    return bins


def apply(bins: list[dict], raw: float) -> float:
    for b in bins:
        if b["raw_min"] <= raw < b["raw_max"]:
            return float(b["calibrated"])
    return float(bins[-1]["calibrated"])


def ece_brier(predicted: list[float], actual: list[int]) -> tuple[float, float]:
    """Same bucketing as the English gate (confidence_b_s3_05)."""
    brier = sum((p - y) ** 2 for p, y in zip(predicted, actual)) / len(actual)
    ece = 0.0
    for low, high in ((0, .25), (.25, .5), (.5, .7), (.7, .85), (.85, .95), (.95, 1.01)):
        idx = [i for i, p in enumerate(predicted) if low <= p < high]
        if idx:
            ece += len(idx) / len(actual) * abs(sum(predicted[i] for i in idx) / len(idx) - sum(actual[i] for i in idx) / len(idx))
    return ece, brier


def main() -> int:
    if sys.platform != "win32":
        print("this builder renders with Windows text shaping; run it on Windows", file=sys.stderr)
        return 2
    train_pages = make_pages(TRAIN, 15, 11, "tr")
    held_pages = make_pages(HELD, 10, 23, "ho")
    results = render_and_recognize(train_pages + held_pages)
    train_rows: list[dict] = []
    held_rows: list[dict] = []
    pages_meta: list[dict] = []
    for group, pages, rows in (("training", train_pages, train_rows), ("held_out", held_pages, held_rows)):
        for p in pages:
            labelled = label_words("\n".join(p["lines"]), results[p["id"]]["words"])
            for j, w in enumerate(labelled):
                rows.append({"id": f"{p['id']}w{j + 1:03d}", **w})
            pages_meta.append({"id": p["id"], "group": group, "level": p["level"], "font": f"{p['font']}{' Bold' if p['bold'] else ''} {p['size']}px", "degradation": p["degrade"],
                               "page_sha256": results[p["id"]]["page_sha256"], "lines": p["lines"], "word_count": len(labelled)})
    dataset = {
        "schema_version": "1.0.0",
        "dataset_id": "caseforge.synthetic-hindi-word-calibration.v1",
        "source_class": "SYNTHETIC_CONTROLLED",
        "purpose": "Calibration development only; never legal evidence and never part of the G-INDIC evaluation set.",
        "engine": R.runtime_manifest()["engine_version"],
        "hindi_model_sha256": R.load_indic_config()["model"]["sha256"],
        "pages": pages_meta,
        "training": train_rows,
        "held_out": held_rows,
    }
    DATASET.parent.mkdir(parents=True, exist_ok=True)
    DATASET.write_text(json.dumps(dataset, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")
    bins = isotonic_bins(train_rows)
    pred = [apply(bins, r["raw"]) for r in held_rows]
    act = [r["correct"] for r in held_rows]
    ece, brier = ece_brier(pred, act)
    raw_ece, raw_brier = ece_brier([r["raw"] for r in held_rows], act)
    artifact = {
        "schema_version": "1.0.0",
        "calibration_id": INDIC_CALIBRATION_ID,
        "status": "PROPOSED_AWAITING_OWNER_APPROVAL",
        "engine_class": INDIC_ENGINE_CLASS,
        "method": "monotonic_piecewise_constant",
        "dataset_path": "eval/calibration/indic_ocr_calibration_v1.json",
        "dataset_sha256": hashlib.sha256(DATASET.read_bytes()).hexdigest(),
        "evaluation_set_overlap": False,
        "bins": bins,
        "held_out_metrics": {"ece": round(ece, 6), "brier": round(brier, 6), "uncalibrated_ece": round(raw_ece, 6), "uncalibrated_brier": round(raw_brier, 6)},
        "word_accuracy": {"training": round(sum(r["correct"] for r in train_rows) / len(train_rows), 4), "held_out": round(sum(act) / len(act), 4)},
        "limits": "Synthetic rendered Hindi, one typeface family, synthetic degradation. Over-confident on real photographed scans is expected until a real-scan calibration set exists.",
        "approval": {"owner": None, "approved_at": None, "missing": ["ece_max", "brier_max", "critical_field_error_costs_by_type"]},
    }
    ARTIFACT.write_text(json.dumps(artifact, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"train words {len(train_rows)} (acc {artifact['word_accuracy']['training']}), held-out words {len(held_rows)} (acc {artifact['word_accuracy']['held_out']})")
    print("bins", [(b["raw_min"], b["raw_max"], b["calibrated"], b["training_words"]) for b in bins])
    print(f"held-out ECE {ece:.4f} (uncalibrated {raw_ece:.4f})  Brier {brier:.4f} (uncalibrated {raw_brier:.4f})")
    return 0


if __name__ == "__main__":
    sys.exit(main())
