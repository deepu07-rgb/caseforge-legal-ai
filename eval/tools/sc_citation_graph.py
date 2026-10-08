#!/usr/bin/env python3
"""B-S4-25: Supreme Court citation graph — resolver, treatment detector and authority score (deterministic).

Input: the per-year corpus from sc_judgments.py (year=YYYY.jsonl.gz). For every citation inside every judgment:

1. RESOLVE it to a judgment in the corpus, most certain method first:
   a. S.C.R.  "[2023] 15 S.C.R. 812" -> the judgment whose report spans volume 15, pages 808-820 (exact, pinpoint-safe);
   b. neutral "2023 INSC 1047"       -> exact;
   c. concordance: citations printed together ("2023:INSC:766 = 2023 (11) Scale 530", "(2006) 5 SCC 446 : AIR 2006 SC
      2129") are the same judgment; equivalence classes are merged across the whole corpus (union-find), so an SCC
      citation resolves if ANY occurrence anywhere was printed beside an S.C.R./INSC citation;
   d. case name + year: party tokens of the cited name against judgments decided that year or the year before
      (SCC/AIR are dated by report year), accepted only above a similarity floor AND a clear margin over the runner-up.
2. DETECT treatment in the citing sentence (overruled, distinguished, followed/relied on, doubted, per incuriam,
   referred to a larger bench) — attached only when the cue is nearest to that citation in the sentence.
3. SCORE authority: PageRank over the resolved citation graph (plus in-degree), used to pick the Landmark set.

Evaluation hooks: the reporter's own labels in digital-era reports ("– relied on", "– distinguished") are compared
with the detector, and resolution methods are counted, so precision can be measured rather than assumed.

Usage: python eval/tools/sc_citation_graph.py <corpus_dir> <out_dir>
"""
from __future__ import annotations

import gzip
import json
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

STOP = {"and", "others", "other", "ors", "anr", "another", "the", "of", "etc", "ltd", "limited", "m", "s", "pvt", "private", "co",
        "company", "corporation", "versus", "vs", "v", "through", "thr", "its", "by", "in", "re", "state", "union", "india"}
TREATMENT = [
    ("PER_INCURIAM", "per-incuriam.v1", re.compile(r"\bper\s+incuriam\b", re.I)),
    ("OVERRULED", "overruled.v1", re.compile(r"\b(?:is|are|stands?|be|been|was|were)\s+(?:hereby\s+|accordingly\s+|expressly\s+)?overruled\b|\bwe\s+(?:hereby\s+)?overrule\b", re.I)),
    ("REFERRED_TO_LARGER_BENCH", "larger-bench.v1", re.compile(r"\b(?:refer|referred|referring|placed?)\b[^.]{0,80}\blarger\s+bench\b", re.I)),
    ("DOUBTED", "doubted.v1", re.compile(r"\b(?:doubt(?:ed)?\s+the\s+correctness|correctness\s+of\b[^.]{0,60}\bdoubted|we\s+have\s+our\s+doubts)\b", re.I)),
    ("DISTINGUISHED", "distinguished.v1", re.compile(r"\bdistinguishable\b|\b(?:is|are|was|were|stands?|be)\s+(?:clearly\s+)?distinguished\b|\bdistinguish(?:ed|ing)\s+on\s+(?:the\s+)?facts\b", re.I)),
    ("FOLLOWED", "followed.v1", re.compile(r"\b(?:we\s+)?(?:follow|followed|following)\s+(?:the\s+)?(?:decision|judgment|view|ratio|law\s+laid\s+down|dictum)\b|\b(?:relied|placed\s+reliance)\s+(?:up)?on\b|\breliance\s+(?:is|was)\s+placed\b|\bapproved\b", re.I)),
]


def norm_scr(c: str):
    m = re.match(r"\[(\d{4})\]\s*(?:(\d{1,3})\s*)?(?:Supp\.?\s*)?S\.?\s?C\.?\s?R\.?\s*(\d{1,5})", c)
    return (int(m.group(1)), int(m.group(2) or 1), int(m.group(3))) if m else None


def norm_insc(c: str):
    m = re.match(r"(\d{4})\s*:?\s*INSC\s*:?\s*(\d{1,5})", c)
    return f"{m.group(1)} INSC {int(m.group(2))}" if m else None


def cite_key(c: dict) -> str:
    s = re.sub(r"[\s.:]+", "", c["cite"]).upper()
    return f"{c['kind']}:{s}"


def cite_year(c: dict):
    m = re.search(r"(\d{4})", c["cite"])
    return int(m.group(1)) if m else None


def party_tokens(s: str | None) -> set:
    return {w for w in re.findall(r"[a-z]{2,}", (s or "").lower()) if w not in STOP}


def split_name(name: str | None):
    if not name:
        return set(), set()
    parts = re.split(r"\s+v\.?\s+|\s+vs\.?\s+|\s+versus\s+", name, maxsplit=1, flags=re.I)
    return (party_tokens(parts[0]), party_tokens(parts[1])) if len(parts) == 2 else (party_tokens(name), set())


def jacc(a: set, b: set) -> float:
    return len(a & b) / len(a | b) if a and b else 0.0


class UF:
    def __init__(self):
        self.p = {}

    def find(self, x):
        self.p.setdefault(x, x)
        while self.p[x] != x:
            self.p[x] = self.p[self.p[x]]
            x = self.p[x]
        return x

    def union(self, a, b):
        ra, rb = self.find(a), self.find(b)
        if ra != rb:
            self.p[rb] = ra


def equivalent_runs(cites: list[dict]):
    """Citations printed together (gap of a few characters: ' = ', ' : ', ', ') are one judgment."""
    s = sorted((c for c in cites if "end" in c), key=lambda c: c["offset"])
    for a, b in zip(s, s[1:]):
        if 0 <= b["offset"] - a["end"] <= 4 and a["kind"] != b["kind"]:
            yield a, b


def sentence_window(text: str, cite: str):
    i = text.find(cite)
    if i < 0:
        flat = re.sub(r"\s+", " ", text)
        i = flat.find(cite)
        if i < 0:
            return None
        text = flat
    start = max(text.rfind(". ", 0, i) + 2, i - 400, 0)
    end_dot = text.find(". ", i + len(cite))
    end = min(end_dot + 1 if end_dot >= 0 else len(text), i + len(cite) + 400)
    return text, start, end, i


def main() -> int:
    corpus, out = Path(sys.argv[1]), Path(sys.argv[2])
    out.mkdir(parents=True, exist_ok=True)
    J = {}                      # path -> compact record
    by_scr = defaultdict(list)  # (year, vol) -> [(start, end, path)]
    by_insc, by_year = {}, defaultdict(list)
    occurrences = []            # (citing_path, citation dict, para_id, window)
    uf = UF()
    for f in sorted(corpus.glob("year*.jsonl.gz")):
        with gzip.open(f, "rt", encoding="utf-8") as fh:
            for line in fh:
                r = json.loads(line)
                p = r["path"]
                parts = p.split("_")
                if len(parts) == 4 and all(x.isdigit() for x in parts):
                    by_scr[(int(parts[0]), int(parts[1]))].append((int(parts[2]), int(parts[3]), p))
                if r.get("neutral"):
                    k = norm_insc(r["neutral"].replace("INSC", " INSC "))
                    if k:
                        by_insc[k] = p
                year = int(r["decision_date"][-4:]) if r.get("decision_date") and r["decision_date"][-4:].isdigit() else int(parts[0])
                J[p] = {"path": p, "title": r.get("title"), "scr": r.get("scr"), "neutral": r.get("neutral"), "year": year, "decision_date": r.get("decision_date"),
                        "bench": r.get("bench"), "disposal": r.get("disposal"), "ocr_quality": r.get("ocr_quality"), "split": r.get("split"),
                        "subject": r.get("subject"), "pet": party_tokens(r.get("petitioner")), "res": party_tokens(r.get("respondent")), "paragraphs": len(r.get("paragraphs", []))}
                by_year[year].append(p)
                for a, b in equivalent_runs(r.get("citations", [])):
                    uf.union(cite_key(a), cite_key(b))
                for a, b in equivalent_runs(r.get("listed_citations", [])):
                    uf.union(cite_key(a), cite_key(b))
                # Reporter labels apply to the whole run of citations that ends with the label.
                labels, pending = {}, []
                for c in sorted(r.get("listed_citations", []), key=lambda c: c["offset"]):
                    pending.append(c)
                    if c.get("reporter_label"):
                        for x in pending:
                            labels[cite_key(x)] = c["reporter_label"]
                        pending = []
                for c in r.get("citations", []):
                    para = next((q for q in r.get("paragraphs", []) if c["cite"] in re.sub(r"\s+", " ", q["text"])), None)
                    win = sentence_window(re.sub(r"\s+", " ", para["text"]), c["cite"]) if para else None
                    occurrences.append((p, c, para["id"] if para else None, (win[0][win[1]:win[2]], win[3] - win[1]) if win else None, labels.get(cite_key(c))))

    def direct(c: dict):
        if c["kind"] == "SCR":
            n = norm_scr(c["cite"])
            if n:
                for s, e, path in by_scr.get((n[0], n[1]), []):
                    if s <= n[2] <= e:
                        return path, "scr-range"
        if c["kind"] == "INSC":
            k = norm_insc(c["cite"])
            if k in by_insc:
                return by_insc[k], "neutral"
        return None, None

    # Seed every concordance class with any directly resolvable member.
    class_target = {}
    for _, c, *_ in occurrences:
        path, _m = direct(c)
        if path:
            class_target.setdefault(uf.find(cite_key(c)), path)

    token_index = defaultdict(set)  # (year, petitioner token) -> judgments
    for path, j in J.items():
        for t in j["pet"]:
            token_index[(j["year"], t)].add(path)

    def by_name(c: dict, citing_year: int):
        pet, res = split_name(c.get("case_name"))
        y = cite_year(c)
        if not pet or not res or not y:
            return None, None
        scored = []
        for yy in (y, y - 1):
            candidates = set().union(*(token_index.get((yy, t), set()) for t in pet))
            for path in candidates:
                j = J[path]
                if j["year"] > citing_year:
                    continue
                s = min(jacc(pet, j["pet"]), jacc(res, j["res"]))
                if s > 0:
                    scored.append((s, path))
        scored.sort(reverse=True)
        if scored and scored[0][0] >= 0.5 and (len(scored) == 1 or scored[0][0] - scored[1][0] >= 0.15):
            return scored[0][1], "name+year"
        return None, None

    edges = defaultdict(lambda: {"count": 0, "occurrences": []})
    methods = Counter()
    treatments = []
    label_eval = Counter()
    for citing, c, para, win, rep_label in occurrences:
        target, method = direct(c)
        if not target:
            t = class_target.get(uf.find(cite_key(c)))
            if t:
                target, method = t, "concordance"
        if not target:
            target, method = by_name(c, J[citing]["year"])
        methods[method or "unresolved"] += 1
        if not target or target == citing:
            continue
        e = edges[(citing, target)]
        e["count"] += 1
        found = None
        if win:
            sentence, pos = win
            for name, pid, rx in TREATMENT:
                for m in rx.finditer(sentence):
                    # the cue must be nearer to this citation than to any other citation in the sentence
                    others = [mm.start() for mm in re.finditer(r"\(\d{4}\)\s*\d+\s*SCC|\[\d{4}\]|AIR\s*\d{4}|\d{4}\s*:?\s*INSC", sentence) if abs(mm.start() - pos) > 3]
                    d = abs(m.start() - pos)
                    if all(d <= abs(m.start() - o) for o in others):
                        found = (name, pid)
                        break
                if found:
                    break
            if found and para:
                treatments.append({"citing": citing, "cited": target, "paragraph": para, "treatment": found[0], "pattern_id": found[1],
                                   "excerpt": sentence, "cite": c["cite"], "resolution": method})
        e["occurrences"].append({"para": para, "cite": c["cite"], "method": method, "treatment": found[0] if found else None})
        if rep_label:
            label_eval[(rep_label, found[0] if found else "none")] += 1

    # PageRank (damping 0.85) over unique citing->cited edges.
    nodes = list(J)
    out_links = defaultdict(set)
    for (a, b) in edges:
        out_links[a].add(b)
    pr = {n: 1 / len(nodes) for n in nodes} if nodes else {}
    for _ in range(40):
        nxt = {n: (1 - 0.85) / len(nodes) for n in nodes}
        dangling = sum(pr[n] for n in nodes if not out_links.get(n))
        for n in nodes:
            nxt[n] += 0.85 * dangling / len(nodes)
        for a, outs in out_links.items():
            if not outs:
                continue
            share = 0.85 * pr[a] / len(outs)
            for b in outs:
                nxt[b] += share
        pr = nxt
    indeg = Counter(b for (_, b) in edges)
    for p in J:
        J[p]["pagerank"] = round(pr.get(p, 0) * len(nodes), 4)
        J[p]["cited_by"] = indeg.get(p, 0)
        J[p]["pet"], J[p]["res"] = sorted(J[p]["pet"]), sorted(J[p]["res"])
    tcount = Counter(t["treatment"] for t in treatments)
    metrics = {"judgments": len(J), "citation_occurrences": len(occurrences), "resolution_methods": dict(methods),
               "resolved_share": round(1 - methods["unresolved"] / max(1, sum(methods.values())), 4), "unique_edges": len(edges),
               "treatment_edges": dict(tcount),
               "reporter_label_vs_detector": {f"{a} -> {b}": n for (a, b), n in sorted(label_eval.items(), key=lambda x: -x[1])}}
    with gzip.open(out / "judgments_index.json.gz", "wt", encoding="utf-8") as fh:
        json.dump(J, fh)
    with gzip.open(out / "citation_edges.jsonl.gz", "wt", encoding="utf-8") as fh:
        for (a, b), e in edges.items():
            fh.write(json.dumps({"citing": a, "cited": b, **e}) + "\n")
    with gzip.open(out / "treatments.jsonl.gz", "wt", encoding="utf-8") as fh:
        for t in treatments:
            fh.write(json.dumps(t, ensure_ascii=False) + "\n")
    (out / "metrics.json").write_text(json.dumps(metrics, indent=1), encoding="utf-8")
    print(json.dumps(metrics, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
