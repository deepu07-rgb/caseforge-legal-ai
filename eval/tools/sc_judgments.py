#!/usr/bin/env python3
"""Supreme Court judgment corpus (B-S4-24): official Supreme Court Reports PDFs -> clean judgment text by paragraph.

Source: the Indian Supreme Court Judgments open dataset (AWS Open Data, maintained by Dattam Labs, CC-BY-4.0),
which mirrors the Supreme Court's eCourts judgments: one PDF per reported judgment, named by its S.C.R. citation
("2023_15_808_820" = [2023] 15 S.C.R. 808-820), with structured metadata (parties, bench, neutral citation, date).

For every judgment this keeps FACTS from the reporter's matter (parties, date, bench, citations, subject catchword)
and the COURT's text (everything from the judgment marker on), split into paragraphs. The reporter's headnote prose
is not kept. Scanned (pre-digital) volumes carry OCR text: each judgment gets an OCR quality score so poor scans can
be kept out of verified packs.

Usage:
  python eval/tools/sc_judgments.py year 2023 <out_dir>          # download + process one year (CI)
  python eval/tools/sc_judgments.py pdf <file.pdf> [--meta json]  # process one PDF (local check)
"""
from __future__ import annotations

import argparse
import gzip
import io
import json
import re
import sys
import tarfile
import time
import urllib.request
from pathlib import Path

BUCKET = "https://indian-supreme-court-judgments.s3.amazonaws.com"
DIGITAL_MARK = re.compile(r"JUDGMENT\s*/\s*ORDER OF THE SUPREME COURT", re.I)
JUDGMENT_START = [
    re.compile(r"(?im)^\s*the\s+judgment\s+of\s+the\s+court\s+was\s+(?:delivered|pronounced)\s+by\b.*$"),
    re.compile(r"(?im)^\s*(?:the\s+following\s+)?(?:judgment|order)s?\s+of\s+the\s+court\s+(?:was|were)\s+delivered\b.*$"),
    re.compile(r"(?m)^\s*JUDGMENT\s*:?\s*$"),
    re.compile(r"(?m)^\s*O\s*R\s*D\s*E\s*R\s*$"),
    re.compile(r"(?m)^[A-Z][A-Z.'\s]{2,70}?,?\s+(?:C\.\s?J\.(?:I\.)?|J\.)\s*[—–\-:.]"),
]
RUNNING_HEAD = [
    re.compile(r"^\s*\[?\(?\d{4}[\])]?\s*\d{1,3}\s*S\.?\s*C\.?\s*R\.?.*$"),
    re.compile(r"^\s*\d{1,4}\s+SUPREME COURT REPORTS.*$", re.I),
    re.compile(r"^\s*S\.?C\.?R\.?\s+SUPREME COURT REPORTS.*$", re.I),
    re.compile(r"^.{3,140}\s+v\.\s+.{3,140}\s\d{1,4}\s*$"),
    re.compile(r"^\s*\d{1,4}\s*[A-H]?\s*$"),
]
MARGIN_LINE = re.compile(r"^\s*[A-H]\s*$")
MARGIN_TAIL = re.compile(r"\s+[A-H]$")
LIGATURE = re.compile(r"(?<=[a-z])(ffi|ffl|fi|ff|fl) (?=[a-z])|\b(ffi|ffl|fi|ff|fl) (?=[a-z]{2,})")
NUMBERED_PARA = re.compile(r"(?m)^\s*(\d{1,3})\.\s+(?=[A-Z\"“'(\[])")
WORD = re.compile(r"[A-Za-z]{2,}")


def fix_ligatures(s: str) -> str:
    return LIGATURE.sub(lambda m: m.group(1) or m.group(2), s)


def page_texts(pdf_bytes: bytes) -> list[str]:
    import fitz  # PyMuPDF

    with fitz.open(stream=pdf_bytes, filetype="pdf") as doc:
        return [p.get_text("text") for p in doc]


def clean_page(text: str) -> str:
    lines = text.replace("\r", "").split("\n")
    # Running heads sit in the first two lines; page numbers and margin letters anywhere.
    for i in range(min(2, len(lines))):
        if any(r.match(lines[i]) for r in RUNNING_HEAD):
            lines[i] = ""
    out = []
    for ln in lines:
        if MARGIN_LINE.match(ln) or RUNNING_HEAD[-1].match(ln):
            continue
        out.append(MARGIN_TAIL.sub("", ln.rstrip()))
    return fix_ligatures("\n".join(out)).strip()


def judgment_start(full: str) -> tuple[int, str]:
    m = DIGITAL_MARK.search(full)
    if m:
        return m.end(), "digital-marker"
    held = re.search(r"(?m)\bHELD\b|\bHeld\b", full)
    floor = held.end() if held else min(300, len(full) // 10)
    best = None
    for r in JUDGMENT_START:
        for mm in r.finditer(full, floor):
            if best is None or mm.start() < best[0]:
                best = (mm.start(), r.pattern[:30])
            break
    return (best[0], "scanned-marker") if best else (-1, "unsplit")


def ocr_quality(text: str) -> float | None:
    try:
        from wordfreq import zipf_frequency
    except ImportError:
        return None
    words = [w.lower() for w in WORD.findall(text)]
    if not words:
        return 0.0
    sample = words[:: max(1, len(words) // 3000)]
    return round(sum(1 for w in sample if zipf_frequency(w, "en") > 0) / len(sample), 4)


def paragraphs(text: str) -> list[dict]:
    marks = [(int(m.group(1)), m.start()) for m in NUMBERED_PARA.finditer(text)]
    seq, last = [], 0
    for n, pos in marks:  # keep a strictly increasing run of paragraph numbers (para 1 may share the byline's line)
        if n == last + 1 or (not seq and n == 2) or (seq and last < n <= last + 3):
            seq.append((n, pos))
            last = n
    paras = []
    if len(seq) >= 3:
        if seq[0][1] > 0 and text[: seq[0][1]].strip():
            paras.append({"id": "intro", "text": text[: seq[0][1]].strip()})
        for i, (n, pos) in enumerate(seq):
            end = seq[i + 1][1] if i + 1 < len(seq) else len(text)
            paras.append({"id": str(n), "text": text[pos:end].strip()})
        return paras
    # Unnumbered judgment (older reports): ~1500-character blocks on sentence boundaries.
    buf, k = "", 0
    for sent in re.split(r"(?<=[.!?])\s+(?=[A-Z\"“(])", text):
        buf = f"{buf} {sent}".strip() if buf else sent
        if len(buf) >= 1500:
            k += 1
            paras.append({"id": f"b{k}", "text": buf})
            buf = ""
    if buf.strip():
        paras.append({"id": f"b{k + 1}", "text": buf.strip()})
    return paras


CITATION_PATTERNS = {
    "SCC": re.compile(r"\(\d{4}\)\s*\d{1,3}\s*SCC\s*(?:\(\w+\)\s*)?\d{1,5}"),
    "SCR": re.compile(r"\[\d{4}\]\s*(?:\d{1,3}\s*)?(?:Supp\.?\s*)?S\.?\s?C\.?\s?R\.?\s*\d{1,5}|\(\d{4}\)\s*\d{1,3}\s*SCR\s*\d{1,5}"),
    "AIR": re.compile(r"A\.?\s?I\.?\s?R\.?\s*\(?\d{4}\)?\s*S\.?\s?C\.?\s*\d{1,5}"),
    "INSC": re.compile(r"\d{4}\s*:?\s*INSC\s*:?\s*\d{1,5}"),
    "SCALE": re.compile(r"\d{4}\s*\(\d{1,2}\)\s*SCALE\s*\d{1,5}", re.I),
}


# "G. M. Tank vs. State of Gujarat & Others (2006) 5 SCC 446": the party names just before a citation.
CASE_NAME = re.compile(r"([A-Z][A-Za-z0-9.&'()\- ]{1,80}?)\s+(?:v\.|vs\.?|versus|V\.)\s+([A-Z][A-Za-z0-9.&,'()\- ]{1,110}?)[\s,:=(]*$")


def citations_in(text: str) -> list[dict]:
    out = []
    for kind, r in CITATION_PATTERNS.items():
        for m in r.finditer(text):
            before = re.sub(r"\s+", " ", text[max(0, m.start() - 220): m.start()])
            # In a chain ("A v. B, (2006) 5 SCC 446; C v. D (2008) ...") keep only what follows the previous citation.
            cuts = [c.end() for r2 in CITATION_PATTERNS.values() for c in r2.finditer(before)] + [i + 1 for i, ch in enumerate(before) if ch in ";"]
            before = before[max(cuts):] if cuts else before
            before = re.sub(r"^(?:\s*(?:see|also|in|cf\.?|vide|and|,)\b)+", "", before.strip(" ,"), flags=re.I)
            cm = CASE_NAME.search(before)
            left = re.sub(r"^(?:(?:See|see|Also|also|In|in|Cf\.?|cf\.?|Vide|vide|and|And)\s+)+", "", cm.group(1).strip()) if cm else ""
            name = f"{left} v. {cm.group(2).strip()}" if cm and left else None
            out.append({"kind": kind, "cite": re.sub(r"\s+", " ", m.group(0)), "case_name": name, "offset": m.start()})
    return out


def process_pdf(pdf_bytes: bytes, meta: dict) -> dict:
    pages = [clean_page(t) for t in page_texts(pdf_bytes)]
    full = "\n".join(pages)
    start, how = judgment_start(full)
    reporter = full[:start] if start > 0 else ""
    court = full[start:].strip() if start >= 0 else ""
    subject = None
    m = re.search(r"(?m)^(?:HEADNOTES\s*\n)?(?:Issue for consideration:.*?\n)?([A-Z][A-Za-z ,.'()&-]{3,60}?)\s+[–—-]\s", reporter)
    if m:
        subject = m.group(1).strip()
    listed = re.search(r"LIST OF CITATIONS AND OTHER REFERENCES(.*?)(?:OTHER CASE DETAILS|$)", reporter, re.S)
    return {
        "path": meta.get("path"),
        "scr": meta.get("citation"),
        "neutral": meta.get("case_id") or meta.get("nc_display"),
        "title": meta.get("title"),
        "petitioner": meta.get("petitioner"),
        "respondent": meta.get("respondent"),
        "bench": meta.get("judge"),
        "author": meta.get("author_judge"),
        "decision_date": meta.get("decision_date"),
        "disposal": meta.get("disposal_nature"),
        "cnr": meta.get("cnr"),
        "pages": len(pages),
        "split": how,
        "subject": subject,
        "listed_citations": citations_in(listed.group(1)) if listed else [],
        "ocr_quality": ocr_quality(court or full),
        "paragraphs": paragraphs(court) if court else [],
        "citations": citations_in(court),
    }


def get(url: str, tries: int = 5) -> bytes:
    for t in range(tries):
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "CaseForge-pack-builder/1.0"}), timeout=600) as r:
                return r.read()
        except Exception as e:  # noqa: BLE001
            if t == tries - 1:
                raise
            print(f"retry {url}: {e}", flush=True)
            time.sleep(10 * (t + 1))
    raise RuntimeError("unreachable")


def process_year(year: int, out: Path) -> int:
    import pyarrow.parquet as pq

    out.mkdir(parents=True, exist_ok=True)
    meta_rows = pq.read_table(io.BytesIO(get(f"{BUCKET}/metadata/parquet/year={year}/metadata.parquet"))).to_pylist()
    meta = {r["path"]: r for r in meta_rows}
    tar_bytes = get(f"{BUCKET}/data/tar/year={year}/english/english.tar")
    stats = {"year": year, "metadata_rows": len(meta_rows), "pdfs": 0, "split": {}, "failed": 0}
    with tarfile.open(fileobj=io.BytesIO(tar_bytes)) as tar, gzip.open(out / f"year={year}.jsonl.gz", "wt", encoding="utf-8") as w:
        for member in tar:
            if not member.isfile() or not member.name.lower().endswith(".pdf"):
                continue
            path = Path(member.name).name.rsplit("_", 1)[0]  # 2023_7_322_346_EN.pdf -> 2023_7_322_346
            try:
                rec = process_pdf(tar.extractfile(member).read(), meta.get(path, {"path": path}))
                rec["source_pdf"] = f"{BUCKET}/data/pdf/year={year}/english/{Path(member.name).name}"
                w.write(json.dumps(rec, ensure_ascii=False) + "\n")
                stats["pdfs"] += 1
                stats["split"][rec["split"]] = stats["split"].get(rec["split"], 0) + 1
            except Exception as e:  # noqa: BLE001 - counted; the year continues
                stats["failed"] += 1
                print(f"FAILED {member.name}: {e}", flush=True)
    (out / f"year={year}.stats.json").write_text(json.dumps(stats, indent=1), encoding="utf-8")
    print(json.dumps(stats), flush=True)
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    y = sub.add_parser("year")
    y.add_argument("year", type=int)
    y.add_argument("out", type=Path)
    p = sub.add_parser("pdf")
    p.add_argument("file", type=Path)
    p.add_argument("--meta", default="{}")
    a = ap.parse_args()
    if a.cmd == "year":
        return process_year(a.year, a.out)
    rec = process_pdf(a.file.read_bytes(), json.loads(a.meta))
    print(json.dumps(rec, ensure_ascii=False, indent=1)[:6000])
    return 0


if __name__ == "__main__":
    sys.exit(main())
