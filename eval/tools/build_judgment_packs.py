#!/usr/bin/env python3
"""B-S4-26: Supreme Court judgment packs from the processed corpus (sc_judgments.py) and citation graph
(sc_citation_graph.py).

Packs (akpkg v2.1, one paragraph per chunk, authority_type JUDGMENT):
  caseforge.india.sc.recent    - every born-digital report (judgment marker found in the digital layout).
  caseforge.india.sc.landmark  - the most-cited judgments of all years by PageRank over the citation graph, limited to
                                 judgments whose judgment text was located and whose OCR quality passes the floor.
Treatment edges (overruled / distinguished / followed / doubted / per incuriam / larger bench) between judgments in
the same pack ship as metadata/treatment_edges.json (contracts/treatment_edge.schema.json): derived, never changing
supersession status.

Only the Court's text is stored; reporter headnote prose is not. Precedential status is UNKNOWN: binding force and
any later overruling are for counsel, informed by the treatment edges.

Usage: python eval/tools/build_judgment_packs.py --corpus <dir> --graph <dir> --out <dir> [--landmark 4000] [--ocr-floor 0.95]
"""
from __future__ import annotations

import argparse
import gzip
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sign_pack_manifests import canonical, digest  # noqa: E402  (no PDF or crypto dependency)


def write_json(path: Path, value) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(value, indent=2, ensure_ascii=False) + "\n")


KEY_ID = "caseforge-official-dev-2026"
VERSION = "1.0.0-beta.1"
TODAY = datetime.now(timezone.utc).date().isoformat()


def iso(d: str | None) -> str:
    m = re.fullmatch(r"(\d{1,2})-(\d{1,2})-(\d{4})", (d or "").strip())
    return f"{m.group(3)}-{int(m.group(2)):02d}-{int(m.group(1)):02d}" if m else "UNKNOWN"


def cite_label(r: dict) -> str:
    parts = [x for x in (r.get("scr"), r.get("neutral")) if x]
    return " : ".join(parts) if parts else r["path"]


def build(pack_id: str, display: str, description: str, recs: list[dict], index: dict, treatments: list[dict], out: Path) -> dict:
    root = out / pack_id
    (root / "content").mkdir(parents=True, exist_ok=True)
    (root / "metadata").mkdir(parents=True, exist_ok=True)
    sources, chunks, chunk_of, source_of = [], [], {}, {}
    publisher = "Supreme Court of India (Supreme Court Reports) via eCourts; open dataset CC-BY-4.0"
    for r in sorted(recs, key=lambda r: r["path"]):
        title = (r.get("title") or r["path"]).strip()
        cite = cite_label(r)
        quality = r.get("ocr_quality")
        version = "Born-digital official report" if r["split"] == "digital-marker" else f"OCR text of the printed official report (word quality {quality})"
        blocks, seen = [], set()
        for p in r["paragraphs"]:
            text = p["text"].strip()
            if len(text) < 20:
                continue
            block = f"[para {p['id']}]\n{text}"
            if block in seen:
                continue
            seen.add(block)
            blocks.append((str(p["id"]), block))
        if not blocks:
            continue
        full = f"[{title} | {cite}]\n" + "\n\n".join(b for _, b in blocks)
        (root / "content" / f"{r['path']}.txt").write_text(full + "\n", encoding="utf-8", newline="\n")
        checksum = "sha256:" + digest(full.encode())
        sid = f"cfsrc:{pack_id}:{checksum.removeprefix('sha256:')[:16]}"
        common = {"schema_version": "2.0.0", "title": title, "jurisdiction": "India", "jurisdiction_path": ["India", "Supreme Court"], "authority_type": "JUDGMENT",
                  "issuing_authority": "Supreme Court of India", "citation": cite, "effective_from": iso(r.get("decision_date")), "effective_to": "UNKNOWN",
                  "version": version, "amendment": "NONE", "source_file": f"content/{r['path']}.txt", "page": 1, "pack_id": pack_id,
                  "trust_level": "OFFICIAL_VERIFIED", "superseded_status": "UNKNOWN", "publisher": publisher, "source_reference": r.get("source_pdf") or "",
                  "content_kind": "AUTHORITATIVE_TEXT", "derived_from_source_id": None, "court": "Supreme Court of India", "case_name": title,
                  "case_number": r.get("neutral") or r.get("scr") or r["path"], "decision_date": iso(r.get("decision_date")), "precedential_status": "UNKNOWN",
                  "holding_tags": [], "proposition_tags": [t for t in [(r.get("subject") or "").lower()[:60]] if t]}
        sources.append({**common, "source_id": sid, "section": "judgment", "paragraph": "whole judgment", "paragraph_number": "all",
                        "heading_path": [title], "exact_text": full, "checksum": checksum})
        source_of[r["path"]] = sid
        for pid, block in blocks:
            ck = "sha256:" + digest(block.encode())
            chunk = {**common, "source_id": sid, "chunk_id": f"cfchunk:{pack_id}:{digest((sid + '|' + ck).encode())[:16]}", "section": f"para {pid}",
                     "paragraph": f"para {pid}", "paragraph_number": pid, "heading_path": [title, f"para {pid}"], "exact_text": block, "checksum": ck,
                     "source_checksum": checksum, "pack_checksum": "PENDING", "temporal_status": "APPLICABLE"}
            chunks.append(chunk)
            chunk_of[(r["path"], pid)] = chunk
    edges = []
    for t in treatments:
        chunk = chunk_of.get((t["citing"], str(t["paragraph"])))
        cited = source_of.get(t["cited"])
        if not chunk or not cited:
            continue
        i = chunk["exact_text"].find(t["excerpt"])
        if i < 0:
            continue  # excerpt must be an exact substring of the citing chunk, or no edge
        edges.append({"schema_version": "1.0.0", "edge_id": f"te:{digest((chunk['chunk_id'] + cited + t['treatment']).encode())[:16]}",
                      "citing": {"source_id": chunk["source_id"], "chunk_id": chunk["chunk_id"], "paragraph": f"para {t['paragraph']}", "char_start": i, "char_end": i + len(t["excerpt"]), "excerpt": t["excerpt"]},
                      "cited_source_id": cited, "treatment": t["treatment"], "detection": {"method": "PATTERN_PLUS_VERIFIER", "pattern_id": t["pattern_id"]},
                      "content_kind": "DERIVED", "affects_supersession": False, "pack_build": {"pack_id": pack_id, "version": VERSION, "built_at": f"{TODAY}T00:00:00Z"}})
    write_json(root / "metadata" / "legal_sources.json", sources)
    write_json(root / "metadata" / "legal_chunks.json", chunks)
    write_json(root / "metadata" / "treatment_edges.json", edges)
    write_json(root / "metadata" / "judgment_index.json", [{"path": r["path"], "title": r.get("title"), "citation": cite_label(r), "decision_date": iso(r.get("decision_date")),
                                                            "bench": r.get("bench"), "disposal": r.get("disposal"), "pagerank": index.get(r["path"], {}).get("pagerank"),
                                                            "cited_by": index.get(r["path"], {}).get("cited_by"), "text_basis": r["split"], "ocr_quality": r.get("ocr_quality")} for r in recs])
    files = []
    for path in sorted(p for p in root.rglob("*") if p.is_file() and p.name not in {"manifest.json", "signature.ed25519"}):
        rel = path.relative_to(root).as_posix()
        role = "AUTHORITATIVE_CONTENT" if rel.startswith("content/") else {"metadata/legal_sources.json": "LEGAL_SOURCES", "metadata/legal_chunks.json": "LEGAL_CHUNKS",
                                                                             "metadata/treatment_edges.json": "TREATMENT_EDGES"}.get(rel, "DERIVED_ARTEFACT")
        files.append({"path": rel, "sha256": "", "bytes": 0, "role": role})
    for f in files:
        f["sha256"], f["bytes"] = digest((root / f["path"]).read_bytes()), (root / f["path"]).stat().st_size
    authoritative = canonical([f for f in files if f["role"] == "AUTHORITATIVE_CONTENT"])
    for c in chunks:
        c["pack_checksum"] = authoritative
    write_json(root / "metadata" / "legal_chunks.json", chunks)
    for f in files:
        f["sha256"], f["bytes"] = digest((root / f["path"]).read_bytes()), (root / f["path"]).stat().st_size
    manifest = {
        "schema_version": "2.1.0", "metadata_version": "2.0.0", "pack_id": pack_id, "pack_kind": "official", "jurisdiction": "India",
        "jurisdiction_path": ["India", "Supreme Court"], "version": VERSION, "display_name": display, "description": description,
        "content_sha256": canonical(files), "authoritative_content_sha256": authoritative, "files": files, "headnotes_stripped": True,
        "signature_alg": "Ed25519", "signer_key_id": KEY_ID, "trust_level": "OFFICIAL_VERIFIED", "controlled_beta_eligible": True,
        "sources_file": "metadata/legal_sources.json", "chunks_file": "metadata/legal_chunks.json",
        "publisher": "Supreme Court of India (official reports) via eCourts; CaseForge original matter", "updated_at": TODAY, "source_status": "CURRENT",
        "supersedes_version": None, "superseded_by_version": None, "editorial_review_status": "TWO_LEGAL_EDITORS_PENDING",
        "rights_basis": "Copyright Act, 1957, section 52(1)(q)(iv) (judgments); dataset CC-BY-4.0 (Dattam Labs)",
        "treatment_edges_file": "metadata/treatment_edges.json", "currency_date": TODAY,
    }
    write_json(root / "manifest.json", manifest)
    return {"pack_id": pack_id, "display_name": display, "judgments": len(recs), "units": len(sources), "treatment_edges": len(edges),
            "bytes": sum(f["bytes"] for f in files), "content_sha256": manifest["content_sha256"]}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", type=Path, required=True)
    ap.add_argument("--graph", type=Path, required=True)
    ap.add_argument("--out", type=Path, required=True)
    ap.add_argument("--landmark", type=int, default=3000)
    ap.add_argument("--ocr-floor", type=float, default=0.95)
    a = ap.parse_args()
    with gzip.open(a.graph / "judgments_index.json.gz", "rt", encoding="utf-8") as fh:
        index = json.load(fh)
    with gzip.open(a.graph / "treatments.jsonl.gz", "rt", encoding="utf-8") as fh:
        treatments = [json.loads(l) for l in fh]

    def eligible(j: dict) -> bool:
        return j.get("split") in ("digital-marker", "scanned-marker") and j.get("paragraphs", 0) > 0 and (j.get("split") == "digital-marker" or (j.get("ocr_quality") or 0) >= a.ocr_floor)

    ranked = sorted((j for j in index.values() if eligible(j)), key=lambda j: (-(j.get("pagerank") or 0), -(j.get("cited_by") or 0)))
    landmark = {j["path"] for j in ranked[: a.landmark]}
    recent, land = [], []
    for f in sorted(a.corpus.glob("year*.jsonl.gz")):
        with gzip.open(f, "rt", encoding="utf-8") as fh:
            for line in fh:
                r = json.loads(line)
                if not r.get("paragraphs"):
                    continue
                if r["split"] == "digital-marker":
                    recent.append(r)
                if r["path"] in landmark:
                    land.append(r)
    built = [
        build("caseforge.india.sc.recent", "Supreme Court of India - Recent judgments (born-digital reports)",
              f"Every born-digital Supreme Court Report in the corpus ({len(recent)} judgments), paragraph by paragraph.", recent, index, treatments, a.out),
        build("caseforge.india.sc.landmark", f"Supreme Court of India - Landmark judgments (top {a.landmark} by citation authority)",
              "The most-cited Supreme Court judgments across all years (PageRank over the corpus citation graph), paragraph by paragraph.", land, index, treatments, a.out),
    ]
    write_json(a.out / "judgment_packs_index.json", {"packs": built})
    for b in built:
        print(f"{b['pack_id']}: {b['judgments']} judgments, {b['units']} paragraphs, {b['treatment_edges']} treatment edges, {b['bytes'] / 1e6:.0f} MB")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
