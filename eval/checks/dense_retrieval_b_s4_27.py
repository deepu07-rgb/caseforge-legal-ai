#!/usr/bin/env python3
"""B-S4-27 meaning-based retrieval gate (offline multilingual-e5-small, int8).

Runs golden/G-DENSE (plain-language and Hindi questions -> criminal-code sections) over the official criminal-codes pack
three ways, mirroring the app (src-tauri/src/retrieval/mod.rs):
  KEYWORD  the app's lexical-only ranking: same tokenizer, BM25 (k1 1.2, b 0.75) over title/section/citation/headings/
           text, + metadata boost x0.3 + 0.08 for OFFICIAL_VERIFIED;
  MEANING  cosine of the query vector with the pack's int8 passage vectors (as shipped);
  FUSED    what the app ranks when vectors exist: reciprocal-rank fusion, 1/(61+r), of KEYWORD (passages with real
           evidence) and the top 100 by MEANING with similarity > 0.
Hard gates: golden pinned; FUSED is not worse than KEYWORD on Recall@10 and MRR@10; int8 passage vectors agree with
float vectors (min cosine >= 0.98). The absolute Recall@10 target is a proposal awaiting the owner (Q-S4-01).

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

GOLDEN = ROOT / "golden" / "G-DENSE" / "fixtures" / "dense_retrieval_queries.json"
GOLDEN_SHA256 = "d118b6f7408f7a2fed1efd280e1718356c7b4676d33201d58b804876e5a55d0e"
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


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--evidence", type=Path)
    args = ap.parse_args()
    errors: list[str] = []
    if hashlib.sha256(GOLDEN.read_bytes()).hexdigest() != GOLDEN_SHA256:
        errors.append("G-DENSE golden changed since it was pinned")
    golden = json.loads(GOLDEN.read_text(encoding="utf-8"))["queries"]
    chunks = json.loads((PACK / "metadata" / "legal_chunks.json").read_text(encoding="utf-8"))
    labels = [LABEL.match(c["exact_text"]).group(1) for c in chunks]

    t0 = time.time()
    passages = [LABEL.sub("", c["exact_text"], count=1)[:2000] for c in chunks]
    pv = np.asarray(embed(passages, kind="passage"), dtype=np.float32)
    shipped = np.frombuffer(quantise(pv.tolist()), dtype=np.int8).reshape(pv.shape).astype(np.float32) / 127.0  # as the app reads the pack
    int8_fidelity = float(np.min((pv * shipped).sum(1) / np.linalg.norm(shipped, axis=1)))
    qv = np.asarray(embed([q["query"] for q in golden], kind="query"), dtype=np.float32)
    all_sims = qv @ shipped.T
    embed_s = time.time() - t0

    docs = [tokenize(search_text(c)) for c in chunks]
    df = Counter(t for d in docs for t in set(d))
    avg = max(1.0, sum(map(len, docs)) / len(docs))
    modes: dict[str, list[list[str]]] = {"KEYWORD": [], "MEANING": [], "FUSED": []}
    expected_sim, top_sim = [], []
    for q, sims in zip(golden, all_sims.tolist()):
        kw = keyword_rank(chunks, docs, df, avg, tokenize(q["query"]))
        dense = sorted(range(len(chunks)), key=lambda i: (-sims[i], i))
        modes["KEYWORD"].append([labels[i] for i, _, _ in kw[:10]])
        modes["MEANING"].append([labels[i] for i in dense[:10]])
        lex_rank = {i: r for r, i in enumerate(i for i, _, ev in kw if ev)}
        den_rank = {i: r for r, i in enumerate(i for i in dense[:100] if sims[i] > 0)}
        fused = {i: (1 / (61 + lex_rank[i]) if i in lex_rank else 0) + (1 / (61 + den_rank[i]) if i in den_rank else 0) for i in set(lex_rank) | set(den_rank)}
        modes["FUSED"].append([labels[i] for i in sorted(fused, key=lambda i: (-fused[i], chunks[i]["chunk_id"]))[:10]])
        exp = [sims[i] for i in range(len(chunks)) if labels[i] in set(q["expected"])]
        expected_sim.append(max(exp) if exp else 0.0)
        top_sim.append(sims[dense[0]])

    results = {m: evaluate(r, golden) for m, r in modes.items()}
    by_lang = {lang: {m: evaluate([r for r, q in zip(modes[m], golden) if q["lang"] == lang], [q for q in golden if q["lang"] == lang])["recall_at_10"] for m in modes}
               for lang in sorted({q["lang"] for q in golden})}
    if results["FUSED"]["recall_at_10"] < results["KEYWORD"]["recall_at_10"] or results["FUSED"]["mrr_at_10"] < results["KEYWORD"]["mrr_at_10"]:
        errors.append("meaning-based fusion ranks worse than keyword search on G-DENSE")
    if int8_fidelity < 0.98:
        errors.append(f"int8 passage vectors drift from float vectors (min cosine {int8_fidelity:.4f})")

    report = {
        "batch": "B-S4-27", "model": MODEL_ID, "golden": "G-DENSE", "queries": len(golden), "passages": len(chunks),
        "embedding_seconds": round(embed_s, 1), "int8_min_cosine": round(int8_fidelity, 4), "results": results, "recall_at_10_by_language": by_lang,
        "similarity": {"expected_best_mean": round(sum(expected_sim) / len(golden), 4), "top1_mean": round(sum(top_sim) / len(golden), 4)},
        "acs": {
            "AC-S4-27.1": {"status": "PASS" if not errors else "FAIL", "detail": "fusion not worse than keyword search; int8 vectors faithful; golden pinned"},
            "AC-S4-27.2": {"status": "PROPOSED", "target": f"FUSED Recall@10 >= {PROPOSED_RECALL_AT_10}", "measured": results["FUSED"]["recall_at_10"],
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
