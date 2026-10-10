#!/usr/bin/env python3
"""Builds the G-INDIC golden pages (B-S4-14): synthetic Hindi (Devanagari) pages with exact ground truth.

The text is written here, rendered to a page image with Windows' own text shaping (render_pages.ps1; Pillow in this environment has no complex
text layout and would draw Devanagari wrongly), and the transcription is the text that was rendered, so the truth is exact by construction and
needs no human labelling. Some pages are then degraded (rotation, blur, noise, low contrast, JPEG) from a fixed seed. Everything is synthetic:
invented names, places and numbers; no client material.

What this is not: real scans. A rendered page is far cleaner and more regular than a photographed FIR; one typeface family (Nirmala UI) is used.
An error rate measured here is a floor, not what a court's scans will give.

Usage (Windows): python eval/tools/make_indic_golden.py [--out golden/G-INDIC/fixtures]
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image, ImageEnhance, ImageFilter

ROOT = Path(__file__).resolve().parents[2]

FIR = [
    "प्रथम सूचना रिपोर्ट",
    "थाना: काल्पनिक नगर, जिला: काल्पनिकपुर",
    "प्रथम सूचना रिपोर्ट सं. ०१२३/२०२४, दिनांक १२/०३/२०२४",
    "धारा ३०२/३४ भारतीय दंड संहिता के अंतर्गत अपराध",
    "",
    "शिकायतकर्ता श्री रमेश कुमार पुत्र श्री मोहन लाल, निवासी ग्राम रामपुर ने",
    "बताया कि दिनांक १० मार्च २०२४ को रात लगभग ९ बजे आरोपी श्री सुरेश वर्मा",
    "ने उनके घर में घुसकर विवाद किया और ₹ १,२५,००० की मांग की।",
    "मौके पर श्रीमती सीता देवी और श्री अजय सिंह गवाह के रूप में उपस्थित थे।",
    "मामला दर्ज कर जांच प्रारंभ की गई।",
]
ORDER = [
    "अपर जिला एवं सत्र न्यायाधीश का न्यायालय, काल्पनिकपुर",
    "वाद संख्या १४५/२०२३",
    "श्रीमती गीता शर्मा बनाम श्री विजय प्रसाद",
    "आदेश दिनांक २८ फरवरी २०२४",
    "",
    "प्रार्थिनी ने धारा ८० सिविल प्रक्रिया संहिता के अंतर्गत नोटिस की सेवा का",
    "उल्लेख किया है। विपक्षी ने रुपये ७५,००० की राशि जमा करने का आश्वासन दिया था।",
    "अगली सुनवाई की तिथि १५ अप्रैल २०२४ नियत की जाती है।",
    "पक्षकारों को सूचित किया जाए।",
    "आदेश खुले न्यायालय में सुनाया गया।",
]
STATEMENT = [
    "गवाह का बयान",
    "नाम: श्री अजय सिंह, आयु 42 वर्ष",
    "मैंने दिनांक 10/03/2024 को लगभग 9 बजे रात शोर सुना।",
    "मैंने देखा कि श्री सुरेश वर्मा के हाथ में लाठी थी।",
    "उसने Rs. 50,000 की मांग की थी, ऐसा मैंने सुना।",
    "थाने में मेरा बयान दिनांक 11 मार्च 2024 को दर्ज हुआ।",
    "मैं अपना बयान पढ़कर हस्ताक्षर करता हूं।",
]
NOTICE = [
    "विधिक नोटिस",
    "प्रेषक: श्रीमती गीता शर्मा, काल्पनिकपुर",
    "प्रति: श्री विजय प्रसाद, नया बाजार",
    "आपने दिनांक ५ अप्रैल २०२३ को ₹ २ लाख ५० हजार लेकर भूमि बेचने का",
    "अनुबंध किया था। धारा ४२० भारतीय दंड संहिता तथा धारा ३४ के अंतर्गत",
    "कार्यवाही की जा सकती है। यह नोटिस प्राप्ति के १५ दिन के भीतर उत्तर दें।",
    "अपराध क्रमांक ०४५६/२०२३ का संदर्भ लें।",
]
MIXED = [
    "जिला न्यायालय, काल्पनिकपुर",
    "Criminal Case No. 456/2024",
    "राज्य बनाम श्री सुरेश वर्मा",
    "आरोपी के विरुद्ध Section 420 IPC तथा धारा ३४ के अंतर्गत आरोप तय किए गए।",
    "The next hearing is fixed on 20 April 2024.",
    "अभियोजन ने दिनांक १८/०४/२०२४ तक समय मांगा।",
    "जमानत राशि ₹ 50,000 निर्धारित की गई।",
]
LATIN = [
    "Order of the District Court",
    "Case No. 789/2024",
    "The plaintiff filed this suit on 12 March 2024 under Section 302 of the Code",
    "and claims Rs. 1,25,000 from the defendant. The defendant denies the claim.",
    "The next hearing is fixed on 15 April 2024.",
]

D, L, M = "DEVANAGARI", "LATIN", "MIXED"
# id, base lines, font, bold, size, degradation, expected page script, expected script of each non-empty line, critical fields (substrings of the text)
PAGES = [
    {"id": "hi_fir_clean_01", "lines": FIR, "font": "Nirmala UI", "bold": False, "size": 38, "degrade": None, "script": D,
     "line_scripts": [D] * 9, "fields": [("date", "१२/०३/२०२४"), ("date", "१० मार्च २०२४"), ("money", "₹ १,२५,०००"), ("provision", "धारा ३०२/३४"), ("case_number", "०१२३/२०२४"), ("name", "श्री रमेश कुमार"), ("name", "श्री सुरेश वर्मा"), ("name", "श्रीमती सीता देवी"), ("name", "श्री अजय सिंह")]},
    {"id": "hi_order_clean_02", "lines": ORDER, "font": "Nirmala UI", "bold": False, "size": 36, "degrade": None, "script": D,
     "line_scripts": [D] * 9, "fields": [("case_number", "१४५/२०२३"), ("date", "२८ फरवरी २०२४"), ("provision", "धारा ८०"), ("money", "रुपये ७५,०००"), ("date", "१५ अप्रैल २०२४"), ("name", "श्रीमती गीता शर्मा"), ("name", "श्री विजय प्रसाद")]},
    {"id": "hi_statement_clean_03", "lines": STATEMENT, "font": "Nirmala UI Semilight", "bold": False, "size": 40, "degrade": None, "script": D,
     "line_scripts": [D] * 7, "fields": [("date", "10/03/2024"), ("money", "Rs. 50,000"), ("date", "11 मार्च 2024"), ("name", "श्री अजय सिंह"), ("name", "श्री सुरेश वर्मा")]},
    {"id": "hi_notice_clean_04", "lines": NOTICE, "font": "Nirmala UI", "bold": False, "size": 34, "degrade": None, "script": D,
     "line_scripts": [D] * 7, "fields": [("date", "५ अप्रैल २०२३"), ("money", "₹ २ लाख ५० हजार"), ("provision", "धारा ४२०"), ("provision", "धारा ३४"), ("case_number", "०४५६/२०२३"), ("name", "श्रीमती गीता शर्मा"), ("name", "श्री विजय प्रसाद")]},
    {"id": "hi_fir_degraded_05", "lines": FIR, "font": "Nirmala UI", "bold": False, "size": 38, "degrade": {"rotate": 1.2, "blur": 1.0, "noise": 18, "contrast": 0.75, "jpeg": None, "seed": 5}, "script": D,
     "line_scripts": [D] * 9, "fields": [("date", "१२/०३/२०२४"), ("date", "१० मार्च २०२४"), ("money", "₹ १,२५,०००"), ("provision", "धारा ३०२/३४"), ("case_number", "०१२३/२०२४"), ("name", "श्री रमेश कुमार"), ("name", "श्री सुरेश वर्मा"), ("name", "श्रीमती सीता देवी"), ("name", "श्री अजय सिंह")]},
    {"id": "hi_order_degraded_06", "lines": ORDER, "font": "Nirmala UI", "bold": False, "size": 36, "degrade": {"rotate": -1.6, "blur": 1.3, "noise": 22, "contrast": 0.8, "jpeg": 45, "seed": 6}, "script": D,
     "line_scripts": [D] * 9, "fields": [("case_number", "१४५/२०२३"), ("date", "२८ फरवरी २०२४"), ("provision", "धारा ८०"), ("money", "रुपये ७५,०००"), ("date", "१५ अप्रैल २०२४"), ("name", "श्रीमती गीता शर्मा"), ("name", "श्री विजय प्रसाद")]},
    {"id": "mixed_order_07", "lines": MIXED, "font": "Nirmala UI", "bold": False, "size": 36, "degrade": None, "script": M,
     "line_scripts": [D, L, D, M, L, D, D], "fields": [("case_number", "Criminal Case No. 456/2024"), ("provision", "Section 420"), ("provision", "धारा ३४"), ("date", "20 April 2024"), ("date", "१८/०४/२०२४"), ("money", "₹ 50,000"), ("name", "श्री सुरेश वर्मा")]},
    {"id": "latin_control_08", "lines": LATIN, "font": "Nirmala UI", "bold": False, "size": 36, "degrade": None, "script": L,
     "line_scripts": [L] * 5, "fields": [("case_number", "Case No. 789/2024"), ("date", "12 March 2024"), ("provision", "Section 302"), ("money", "Rs. 1,25,000"), ("date", "15 April 2024")]},
]


def degrade(img: Image.Image, p: dict) -> Image.Image:
    img = img.convert("L")
    rng = random.Random(p["seed"])
    if p.get("rotate"):
        img = img.rotate(p["rotate"], resample=Image.Resampling.BICUBIC, expand=False, fillcolor=255)
    if p.get("blur"):
        img = img.filter(ImageFilter.GaussianBlur(p["blur"]))
    if p.get("contrast"):
        img = ImageEnhance.Contrast(img).enhance(p["contrast"])
    if p.get("noise"):
        try:
            import numpy as np

            arr = np.asarray(img, dtype="float32")
            arr += np.random.default_rng(p["seed"]).normal(0, p["noise"], arr.shape)
            img = Image.fromarray(arr.clip(0, 255).astype("uint8"), "L")
        except ImportError:  # slow, same idea
            px = img.load()
            for y in range(img.height):
                for x in range(img.width):
                    px[x, y] = max(0, min(255, int(px[x, y] + rng.gauss(0, p["noise"]))))
    if p.get("jpeg"):
        buf = io.BytesIO()
        img.save(buf, format="JPEG", quality=int(p["jpeg"]))
        img = Image.open(io.BytesIO(buf.getvalue())).convert("L")
    return img


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=ROOT / "golden" / "G-INDIC" / "fixtures")
    args = ap.parse_args()
    if sys.platform != "win32":
        print("this generator renders with Windows text shaping; run it on Windows", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        spec = [{"id": p["id"], "font": p["font"], "bold": p["bold"], "size_px": p["size"], "lines": p["lines"]} for p in PAGES]
        (tmp / "spec.json").write_text(json.dumps(spec, ensure_ascii=False), encoding="utf-8")
        subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(ROOT / "eval" / "tools" / "render_pages.ps1"), "-Spec", str(tmp / "spec.json"), "-OutDir", str(tmp / "out")], check=True)
        for p in PAGES:
            lines = p["lines"]
            nonempty = [l for l in lines if l]
            assert len(nonempty) == len(p["line_scripts"]), p["id"]
            text = "\n".join(nonempty)
            fields = []
            for i, (kind, sub) in enumerate(p["fields"]):
                at = text.index(sub)
                assert text.count(sub) >= 1
                fields.append({"field_id": f"f{i + 1}", "kind": kind, "text": sub, "char_start": at, "char_end": at + len(sub)})
            img = Image.open(tmp / "out" / f"{p['id']}.png").convert("L")
            if p["degrade"]:
                img = degrade(img, p["degrade"])
            d = args.out / p["id"]
            d.mkdir(parents=True, exist_ok=True)
            img.save(d / "page.png", optimize=True)
            (d / "truth.json").write_text(json.dumps({"fixture_id": f"G-INDIC/{p['id']}", "lines": nonempty, "text": text, "script": p["script"], "line_scripts": p["line_scripts"], "critical_fields": fields}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            (d / "meta.json").write_text(json.dumps({
                "fixture_id": f"G-INDIC/{p['id']}",
                "status": "DRAFT",
                "source_class": "SYNTHETIC_CONTROLLED",
                "authored_by": "coding agent, at the owner's instruction",
                "independent_review": None,
                "description": "A synthetic Hindi page: text written for the test and rendered with Windows text shaping; the transcription is the rendered text.",
                "font": f"{p['font']}{' Bold' if p['bold'] else ''} {p['size']}px at 200 dpi",
                "degradation": p["degrade"],
                "primary_acs": ["AC-S4-09.1"],
                "limits": "Rendered text is cleaner than a photographed scan; one typeface family. An error rate here is a floor, not what real scans will give.",
                "page_sha256": hashlib.sha256((d / "page.png").read_bytes()).hexdigest(),
            }, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {len(PAGES)} pages to {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
