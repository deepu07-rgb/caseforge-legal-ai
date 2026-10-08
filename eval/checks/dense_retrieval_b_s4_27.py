#!/usr/bin/env python3
"""B-S4-27 meaning-based retrieval gate (offline multilingual-e5-small, int8).

Runs golden/G-DENSE (plain-language and Hindi questions -> criminal-code sections) over the official criminal-codes pack
three ways, mirroring the app (src-tauri/src/retrieval/mod.rs):
  KEYWORD  the app's lexical-only ranking: same tokenizer, BM25 (k1 1.2, b 0.75) over title/section/citation/headings/
           text, + metadata boost x0.3 + 0.08 for OFFICIAL_VERIFIED;
  MEANING  cosine of the query vector with the pack's int8 passage vectors (as shipped);
  ENGINE   what the app ranks when vectors exist: weighted reciprocal-rank fusion, w/(61+r), of KEYWORD (passages with
           real evidence) and the top 100 by MEANING with similarity > 0. Weights follow the question: one naming a
           section, citation or year (a term with a digit) leans on keywords (1.0 / 0.5), any other on meaning (0.5 / 1.0).
  RRF_*    fixed-weight alternatives, reported for comparison only.
Two pinned sets: "plain" (plain-language and Hindi) and "identifier" (section numbers as lawyers cite them).
Hard gates: goldens pinned; ENGINE is not worse than KEYWORD on Recall@10 and MRR@10 on either set; int8 passage vectors
agree with float vectors (min cosine >= 0.98). The absolute Recall@10 target is a proposal awaiting the owner (Q-S4-01).

Usage: CASEFORGE_EMBED_MODEL=<dir> python eval/checks/dense_retrieval_b_s4_27.py [--evidence out.json]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
import time
from collections import Counter
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))
from embed import MODEL_ID, embed, quantise  # noqa: E402

FIX = ROOT / "golden" / "G-DENSE" / "fixtures"
GOLDENS = {  # pinned when committed, before the engine they measure
    "plain": (FIX / "dense_retrieval_queries.json", "d118b6f7408f7a2fed1efd280e1718356c7b4676d33201d58b804876e5a55d0e"),
    "identifier": (FIX / "identifier_queries.json", "2fbf037024b1e91c780d98bbb43a16f5e1c511dcb51d7307a96696d6cf3391bd"),
}
PACK = ROOT / "packs" / "official" / "caseforge.india.criminal-codes"
STOP = set("the a an and or of to in on for from by is are was were be been being what when where which who whom whose how may must "
           "does do did this that these those with under into any it its as at if not".split())
LABEL = re.compile(r"^\[([^\]\n]{0,300})\]\n")
PROPOSED_RECALL_AT_10 = 0.80


def tokenize(text: str) -> list[str]:
    return [t for t in re.split(r"[^a-z0-9/]", text.lower()) if len(t) >= 2 and t not in STOP]


def search_text(c: dict) -> str:
    # title, section, citation, headings, authority hint (= citation for pack chunks), text
    return " ".join([c["title"], c["section"], c["citation"], " ".join(c.get("heading_path", [])), c["citation"], c["exact_text"]])


def keyword_rank(chunks: list[dict], docs: list[list[str]], df: Counter, avg: float, terms: list[str]) -> list[tuple[int, float, bool]]:
    n = len(chunks)
    out = []
    for i, (c, toks) in enumerate(zip(chunks, docs)):
        if not terms:
            break
        counts = Counter(toks)
        lexical = 0.0
        for t in terms:
            tf = counts.get(t, 0)
            if tf:
                idf = math.log(1 + (n - df[t] + 0.5) / (df[t] + 0.5))
                lexical += idf * (tf * 2.2 / (tf + 1.2 * (1 - 0.75 + 0.75 * len(toks) / avg)))
        meta = f"{c['title']} {c['section']} {c['citation']}".lower()
        boost = min(1.0, sum(t in meta for t in terms) / max(1, len(terms)))
        authority = 0.08 if c.get("trust_level") == "OFFICIAL_VERIFIED" else 0.0
        score = lexical + boost * 0.3 + authority
        if score > 0:
            out.append((i, score, lexical > 0 or boost > 0))
    out.sort(key=lambda x: (-x[1], chunks[x[0]]["chunk_id"]))
    return out


def evaluate(ranked: list[list[str]], golden: list[dict]) -> dict:
    r5 = r10 = mrr = 0.0
    misses = []
    for labels, q in zip(ranked, golden):
        exp = set(q["expected"])
        first = next((k for k, lab in enumerate(labels[:10]) if lab in exp), None)
        r5 += first is not None and first < 5
        r10 += first is not None
        mrr += 1 / (first + 1) if first is not None else 0
        if first is None:
            misses.append(q["id"])
    n = len(golden)
    return {"recall_at_5": round(r5 / n, 4), "recall_at_10": round(r10 / n, 4), "mrr_at_10": round(mrr / n, 4), "missed": misses}


def fuse(lex_rank: dict[int, int], den_rank: dict[int, int], w_lex: float, w_den: float) -> dict[int, float]:
    return {i: (w_lex / (61 + lex_rank[i]) if i in lex_rank else 0) + (w_den / (61 + den_rank[i]) if i in den_rank else 0) for i in set(lex_rank) | set(den_rank)}


def engine_weights(terms: list[str]) -> tuple[float, float]:
    """Mirror of retrieval::fusion_weights: a question naming a section, citation or year (any term with a digit) leans
    on exact keyword matching; a plain-language or non-English question leans on meaning."""
    return (1.0, 0.5) if any(any(ch.isdigit() for ch in t) for t in terms) else (0.5, 1.0)


GRID = {"RRF_1_1": (1.0, 1.0), "RRF_LEX_HALF": (0.5, 1.0), "RRF_DENSE_HALF": (1.0, 0.5)}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence", type=Path)
    args = ap.parse_args()
    errors: list[str] = []
    sets = {}
    for name, (path, pin) in GOLDENS.items():
        if hashlib.sha256(path.read_bytes()).hexdigest() != pin:
            errors.append(f"G-DENSE {name} set changed since it was pinned")
        sets[name] = json.loads(path.read_text(encoding="utf-8"))["queries"]
    chunks = json.loads((PACK / "metadata" / "legal_chunks.json").read_text(encoding="utf-8"))
    labels = [LABEL.match(c["exact_text"]).group(1) for c in chunks]

    t0 = time.time()
    passages = [LABEL.sub("", c["exact_text"], count=1)[:2000] for c in chunks]
    pv = np.asarray(embed(passages, kind="passage"), dtype=np.float32)
    shipped = np.frombuffer(quantise(pv.tolist()), dtype=np.int8).reshape(pv.shape).astype(np.float32) / 127.0  # as the app reads the pack
    int8_fidelity = float(np.min((pv * shipped).sum(1) / np.linalg.norm(shipped, axis=1)))
    embed_s = time.time() - t0
    docs = [tokenize(search_text(c)) for c in chunks]
    df = Counter(t for d in docs for t in set(d))
    avg = max(1.0, sum(map(len, docs)) / len(docs))

    report_sets = {}
    for name, golden in sets.items():
        qv = np.asarray(embed([q["query"] for q in golden], kind="query"), dtype=np.float32)
        modes: dict[str, list[list[str]]] = {m: [] for m in ["KEYWORD", "MEANING", "ENGINE", *GRID]}
        for q, sims in zip(golden, (qv @ shipped.T).tolist()):
            terms = tokenize(q["query"])
            kw = keyword_rank(chunks, docs, df, avg, terms)
            dense = sorted(range(len(chunks)), key=lambda i: (-sims[i], i))
            modes["KEYWORD"].append([labels[i] for i, _, _ in kw[:10]])
            modes["MEANING"].append([labels[i] for i in dense[:10]])
            lex_rank = {i: r for r, i in enumerate(i for i, _, ev in kw if ev)}
            den_rank = {i: r for r, i in enumerate(i for i in dense[:100] if sims[i] > 0)}
            for m, (wl, wd) in [("ENGINE", engine_weights(terms)), *GRID.items()]:
                f = fuse(lex_rank, den_rank, wl, wd)
                modes[m].append([labels[i] for i in sorted(f, key=lambda i: (-f[i], chunks[i]["chunk_id"]))[:10]])
        results = {m: evaluate(r, golden) for m, r in modes.items()}
        report_sets[name] = {"queries": len(golden), "results": results,
                             "recall_at_10_by_language": {lang: {m: evaluate([r for r, q in zip(modes[m], golden) if q["lang"] == lang], [q for q in golden if q["lang"] == lang])["recall_at_10"] for m in modes}
                                                          for lang in sorted({q["lang"] for q in golden})}}
        e, k = results["ENGINE"], results["KEYWORD"]
        if e["recall_at_10"] < k["recall_at_10"] or e["mrr_at_10"] < k["mrr_at_10"]:
            errors.append(f"engine fusion ranks worse than keyword search on the G-DENSE {name} set")
    if int8_fidelity < 0.98:
        errors.append(f"int8 passage vectors drift from float vectors (min cosine {int8_fidelity:.4f})")

    plain = report_sets["plain"]["results"]["ENGINE"]["recall_at_10"]
    report = {
        "batch": "B-S4-27", "model": MODEL_ID, "golden": "G-DENSE", "passages": len(chunks), "embedding_seconds": round(embed_s, 1),
        "int8_min_cosine": round(int8_fidelity, 4), "engine_fusion": "weighted RRF 1/(61+r); digit in query -> keyword 1.0 / meaning 0.5, else 0.5 / 1.0",
        "sets": report_sets,
        "acs": {
            "AC-S4-27.1": {"status": "PASS" if not errors else "FAIL", "detail": "engine fusion not worse than keyword search on either set; int8 vectors faithful; goldens pinned"},
            "AC-S4-27.2": {"status": "PROPOSED", "target": f"ENGINE Recall@10 >= {PROPOSED_RECALL_AT_10} on the plain set", "measured": plain,
                           "reason": "absolute retrieval threshold awaits owner approval (Q-S4-01)"},
        },
        "errors": errors,
    }
    print(json.dumps(report, indent=2, ensure_ascii=False))
    if args.evidence:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
