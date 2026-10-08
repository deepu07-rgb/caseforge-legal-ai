#!/usr/bin/env python3
"""Crawl the Central Acts from India Code's official DSpace REST API (https://indiacode.gov.in/server/api).

For every Act in the CENTRAL > Acts collection (plus the English Constitution and the three repealed criminal codes
needed by the Code Bridge) it records: official metadata (title, Act number, year, ministry, department, enforcement
date, repeal flag), the official English PDF (downloaded, SHA-256 in the ledger), and the official section records
(section number, order, heading, section text). Machine-written "summary" abstracts are deliberately NOT collected:
they are not legislative text.

Resumable: an Act whose JSON already exists is skipped. Polite: one request at a time, retries with backoff.

Usage: python -I eval/tools/indiacode_crawl.py <out_dir> [--limit N] [--handles 123456789/496548 ...] [--no-pdf]
Output: <out>/acts/<handle_id>.json, <out>/pdfs/<handle_id>.pdf, <out>/index.json
"""
from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import sys
import time
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

API = "https://indiacode.gov.in/server/api"
ACTS_COLLECTION = "69a0c1fb-7b22-4481-b16a-1dc59b5d02e6"      # CENTRAL > Acts (123456789/2)
SECTION_COLLECTION = "9a5b5f4a-4ccf-4cef-9820-0d33f8a6707f"   # CENTRAL > Section (123456789/148)
CONSTITUTION_COLLECTION = "d5475e8d-1998-4ac8-8694-82f941074bb7"
# Repealed central criminal codes (stub items keep the official central PDF; needed for the Code Bridge).
REPEALED_EXTRA = ["123456789/488475", "123456789/620185", "123456789/488783"]
UA = {"User-Agent": "CaseForge-pack-builder/1.0 (official-text acquisition; contact: repository owner)", "Accept": "application/json"}
META_KEYS = {
    "title": "dc.title", "long_title": "dc.title.long_title", "act_number": "dc.identifier.act_number", "act_year": "dc.date.act_year",
    "enact_date": "dc.date.enact_date", "enforcement_date": "dc.date.enforcement_date", "act_id": "dc.identifier.act_id",
    "ministry": "dc.identifier.ministry_name", "department": "dc.identifier.department_name", "repealed": "dc.identifier.repealed",
    "state": "dc.identifier.state_name", "no_of_section": "dc.identifier.no_of_section",
}


def get(url: str, binary: bool = False, tries: int = 6):
    delay = 5
    for attempt in range(tries):
        try:
            req = urllib.request.Request(url, headers=UA if not binary else {"User-Agent": UA["User-Agent"]})
            with urllib.request.urlopen(req, timeout=180) as r:
                data = r.read()
            time.sleep(0.15)
            return data if binary else json.loads(data)
        except Exception as e:  # noqa: BLE001 - retried, then raised
            if attempt == tries - 1:
                raise
            print(f"   retry {attempt + 1} {url[:110]}: {e}", flush=True)
            time.sleep(delay)
            delay = min(delay * 2, 120)


def search(scope: str, query: str = "", size: int = 100):
    page = 0
    while True:
        q = f"{API}/discover/search/objects?scope={scope}&dsoType=ITEM&size={size}&page={page}&sort=dc.title,ASC"
        if query:
            q += "&query=" + urllib.parse.quote(query)
        res = get(q)["_embedded"]["searchResult"]
        for o in res.get("_embedded", {}).get("objects", []):
            yield o["_embedded"]["indexableObject"]
        page += 1
        if page >= res["page"]["totalPages"]:
            break


def meta(item: dict, key: str):
    v = item.get("metadata", {}).get(key)
    return v[0]["value"] if v else None


def html_to_text(s: str) -> str:
    s = re.sub(r"<\s*(br|hr)[^>]*>", "\n", s, flags=re.I)
    s = re.sub(r"</\s*(p|div|center|li|tr)\s*>", "\n", s, flags=re.I)
    s = re.sub(r"<[^>]+>", "", s)
    s = html.unescape(s).replace("\xa0", " ")
    s = re.sub(r"[ \t]+", " ", s)
    s = re.sub(r" *\n *", "\n", s)
    return re.sub(r"\n{2,}", "\n", s).strip()


def english_pdf(item_uuid: str):
    bundles = get(f"{API}/core/items/{item_uuid}/bundles?embed=bitstreams")
    pdfs = []
    for b in bundles.get("_embedded", {}).get("bundles", []):
        if b["name"] != "ORIGINAL":
            continue
        for bs in b.get("_embedded", {}).get("bitstreams", {}).get("_embedded", {}).get("bitstreams", []):
            name = bs["name"]
            if not name.lower().endswith(".pdf"):
                continue
            hindi = name.lower().startswith(("h", "hh")) and not name.lower().startswith("hi") or "hindi" in name.lower()
            pdfs.append((hindi, bs.get("sizeBytes") or 0, name, bs["_links"]["content"]["href"]))
    if not pdfs:
        return None
    pdfs.sort(key=lambda p: (p[0], -1 if re.match(r"^[aA]\d", p[2]) else 0, p[1]))
    return {"name": pdfs[0][2], "url": pdfs[0][3], "bytes_reported": pdfs[0][1], "candidates": [p[2] for p in pdfs]}


def sections(act_id: str) -> list[dict]:
    out = []
    for it in search(SECTION_COLLECTION, f'dc.identifier.act_id:"{act_id}"'):
        if meta(it, "dc.identifier.act_id") != act_id:
            continue
        raw = meta(it, "dc.identifier.section_page_note") or ""
        out.append({
            "number": (meta(it, "dc.identifier.section_number") or "").strip(),
            "order": int(re.sub(r"\D", "", meta(it, "dc.identifier.order_number") or "0") or 0),
            "heading": (meta(it, "dc.title") or "").strip(),
            "page": meta(it, "dc.identifier.page_number"),
            "text": html_to_text(raw),
            "handle": it.get("handle"),
        })
    out.sort(key=lambda s: (s["order"], s["number"]))
    return out


def crawl_item(item: dict, out: Path, with_pdf: bool, kind: str) -> dict:
    hid = item["handle"].split("/")[-1]
    rec = {"handle": item["handle"], "uuid": item["uuid"], "kind": kind, **{k: meta(item, v) for k, v in META_KEYS.items()}}
    rec["title"] = rec["title"] or item.get("name")
    rec["record_url"] = f"https://indiacode.gov.in/handle/{item['handle']}"
    pdf = english_pdf(item["uuid"])
    if pdf and with_pdf:
        data = get(pdf["url"], binary=True)
        if data[:4] == b"%PDF":
            (out / "pdfs").mkdir(parents=True, exist_ok=True)
            (out / "pdfs" / f"{hid}.pdf").write_bytes(data)
            pdf.update(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data), fetched_at=datetime.now(timezone.utc).isoformat())
        else:
            pdf["error"] = "not a PDF"
    rec["pdf"] = pdf
    rec["sections"] = sections(rec["act_id"]) if rec.get("act_id") else []
    rec["text_basis"] = "SECTION_RECORDS" if rec["sections"] else ("PDF" if pdf else "NONE")
    rec["crawled_at"] = datetime.now(timezone.utc).isoformat()
    return rec


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out", type=Path)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--handles", nargs="*", default=None)
    ap.add_argument("--no-pdf", action="store_true")
    a = ap.parse_args()
    (a.out / "acts").mkdir(parents=True, exist_ok=True)

    items: list[tuple[str, dict]] = []
    if a.handles:
        for h in a.handles:
            ref = urllib.request.urlopen(urllib.request.Request(f"{API}/pid/find?id={h}", headers=UA), timeout=120)
            items.append(("ACT", json.loads(ref.read())))
    else:
        items += [("ACT", it) for it in search(ACTS_COLLECTION)]
        items += [("CONSTITUTION", it) for it in search(CONSTITUTION_COLLECTION) if re.fullmatch(r"(?i)the constitution of india( \(?english\)?)?( \d{4})?", (it.get("name") or "").strip())]
        for h in REPEALED_EXTRA:
            ref = urllib.request.urlopen(urllib.request.Request(f"{API}/pid/find?id={h}", headers=UA), timeout=120)
            items.append(("REPEALED_CODE", json.loads(ref.read())))
    if a.limit:
        items = items[: a.limit]
    print(f"{len(items)} items to crawl", flush=True)
    done = failed = 0
    for i, (kind, item) in enumerate(items, 1):
        hid = item["handle"].split("/")[-1]
        path = a.out / "acts" / f"{hid}.json"
        if path.exists():
            continue
        try:
            rec = crawl_item(item, a.out, not a.no_pdf, kind)
            path.write_text(json.dumps(rec, ensure_ascii=False, indent=1), encoding="utf-8")
            done += 1
            print(f"[{i}/{len(items)}] {rec['title'][:70]} | {rec['text_basis']} {len(rec['sections'])} sections | {rec['ministry']}", flush=True)
        except Exception as e:  # noqa: BLE001 - recorded, crawl continues
            failed += 1
            print(f"[{i}/{len(items)}] FAILED {item.get('name')}: {e}", flush=True)
    index = [json.loads(p.read_text(encoding="utf-8")) for p in sorted((a.out / "acts").glob("*.json"))]
    summary = [{k: r[k] for k in ("handle", "title", "kind", "ministry", "repealed", "text_basis")} | {"sections": len(r["sections"]), "pdf": (r.get("pdf") or {}).get("name")} for r in index]
    (a.out / "index.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1), encoding="utf-8")
    print(f"DONE crawled={done} failed={failed} total_on_disk={len(index)}", flush=True)
    return 0 if failed == 0 else 2


if __name__ == "__main__":
    sys.exit(main())
