"""B-S3-06 deterministic graph-v2 sidecar extractor.

The Rust host is the primary persisted path. This module mirrors its RPC contract so direct
docpipe callers cannot fall back to graph v1 or the retired same-date contradiction heuristic.
"""
from __future__ import annotations

import hashlib
import re
from datetime import datetime, timezone
from typing import Any

EXTRACTOR_ID = "caseforge.graph-extractor.deterministic.v2"
ENTITY_RESOLVER_ID = "caseforge.entity-resolver.exact-alias.v2"
RELATIONSHIP_BUILDER_ID = "caseforge.relationship-builder.co-source.v2"
PIPELINE_VERSION = "B-S3-06.2.0.0"

NODE_PATTERNS = [
    ("claim", "pleaded", re.compile(r"^\s*claim\s*:\s*(.{4,300})$", re.I | re.M)),
    ("cause_or_ground", "pleaded", re.compile(r"^\s*(?:cause of action|ground)\s*:\s*(.{4,300})$", re.I | re.M)),
    ("defence", "pleaded", re.compile(r"^\s*(?:defen[cs]e|affirmative defen[cs]e)\s*:\s*(.{4,300})$", re.I | re.M)),
    ("counterclaim", "pleaded", re.compile(r"^\s*counterclaim\s*:\s*(.{4,300})$", re.I | re.M)),
    ("issue", "unknown", re.compile(r"^\s*(?:issue|question presented)\s*:\s*(.{4,300})$", re.I | re.M)),
    ("sub_issue", "unknown", re.compile(r"^\s*sub-?issue\s*:\s*(.{4,300})$", re.I | re.M)),
    ("fact", "pleaded", re.compile(r"^\s*(?:(?:admitted|disputed|material)\s+)?fact\s*:\s*(.{4,400})$", re.I | re.M)),
    ("argument", "pleaded", re.compile(r"^\s*argument\s*:\s*(.{4,400})$", re.I | re.M)),
    ("interpretation", "inferred", re.compile(r"^\s*alternative interpretation\s*:\s*(.{4,400})$", re.I | re.M)),
    ("witness", "unknown", re.compile(r"^\s*witness\s*:\s*(.{3,180})$", re.I | re.M)),
    ("statement", "pleaded", re.compile(r"^\s*(?:statement|testimony)\s*:\s*(.{4,400})$", re.I | re.M)),
    ("application", "pleaded", re.compile(r"^\s*application\s*:\s*(.{4,300})$", re.I | re.M)),
    ("hearing", "unknown", re.compile(r"^\s*hearing\s*:\s*(.{4,300})$", re.I | re.M)),
    ("order", "unknown", re.compile(r"^\s*order\s*:\s*(.{4,300})$", re.I | re.M)),
]
PARTY_RE = re.compile(r"^\s*(plaintiff|petitioner|applicant|defendant|respondent|appellant|appellee|accused|prosecution)\s*[:\-–]\s*([^\n;,]{2,120})", re.I | re.M)
LAW_RE = re.compile(r"\b(?:section|rule|article|§)\s*\d+[A-Za-z]?(?:\([0-9A-Za-z]+\))*[^\n.;]{0,90}|\b\d+\s+(?:U\.S\.|F\.\d+d|SCC|AIR)\s+\d+\b|\b(?:IPC|CrPC|CPC|U\.S\.C\.)\b", re.I)
RELIEF_RE = re.compile(r"^\s*(?:prayer|relief|wherefore)\s*:\s*(.{5,400})$", re.I | re.M)
EVIDENCE_RE = re.compile(r"^\s*(exhibit|annexure|document|evidence)\s*[:\-]\s*(.{3,300})$", re.I | re.M)
PROCEDURE_RE = re.compile(r"^\s*(?:procedural history|procedure|posture)\s*:\s*(.{4,400})$", re.I | re.M)
NAMED_DATE_RE = re.compile(r"\b(\d{1,2})\s+(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{4})\b", re.I)
US_DATE_RE = re.compile(r"\b(January|February|March|April|May|June|July|August|September|October|November|December)\s+(\d{1,2}),?\s+(\d{4})\b", re.I)
NUM_DATE_RE = re.compile(r"\b(\d{1,2})[./-](\d{1,2})[./-](\d{4})\b")
RELATIVE_RE = re.compile(r"\b(?:the\s+)?(?:same|next|following|previous)\s+(?:day|week|month|year)\b|\b\d+\s+(?:days?|weeks?|months?|years?)\s+(?:later|earlier|after|before)\b", re.I)
MONTHS = {name: i + 1 for i, name in enumerate("january february march april may june july august september october november december".split())}


def _now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _compact(value: str, limit: int = 420) -> str:
    return " ".join(value.split())[:limit]


def _pointer(doc_id: str, page: int, match: re.Match[str]) -> dict[str, Any]:
    quote = match.group(0)[:320]
    return {"doc_id": doc_id, "page_index": page, "kind": "pdf_span", "char_start": match.start(),
            "char_end": match.end(), "bbox": None, "quote": quote,
            "content_hash": f"sha256:{hashlib.sha256(quote.encode()).hexdigest()}"}


def _prov(source: dict[str, Any], confidence: float, at: str, origin: str = "EXTRACTED", notes: str | None = None) -> dict[str, Any]:
    return {"source": source, "sources": [source], "confidence": max(0.0, min(1.0, confidence)),
            "origin": origin, "pipeline_version": PIPELINE_VERSION, "recorded_at": at,
            "counsel_confirmed": False, "correction_lineage": [], "reviewed": False, "notes": notes}


def _attrs(**updates: Any) -> dict[str, Any]:
    value = {"aliases": [], "role": None, "side": None, "event_type": None, "date": None,
             "jurisdiction": None, "citation": None, "document_id": None,
             "statement_kind": None, "alternative_group": None}
    value.update(updates)
    return value


def _node(node_id: str, node_type: str, label: str, status: str, attributes: dict[str, Any], provenance: dict[str, Any]) -> dict[str, Any]:
    return {"node_id": node_id, "node_type": node_type, "label": label, "status": status,
            "attributes": attributes, "provenance": provenance}


def _date(raw: str, kind: str, precision: str, status: str, candidates: list[str], resolved: str | None) -> dict[str, Any]:
    return {"raw": raw, "kind": kind, "precision": precision, "status": status, "candidates": candidates,
            "resolved": resolved, "range_start": None, "range_end": None, "relative_to": None}


def _dates(text: str) -> list[tuple[dict[str, Any], re.Match[str]]]:
    out: list[tuple[dict[str, Any], re.Match[str]]] = []
    for pattern, order in ((NAMED_DATE_RE, "dmy"), (US_DATE_RE, "mdy")):
        for m in pattern.finditer(text):
            if order == "dmy": d, mon, y = int(m[1]), MONTHS[m[2].lower()], int(m[3])
            else: mon, d, y = MONTHS[m[1].lower()], int(m[2]), int(m[3])
            iso = f"{y:04}-{mon:02}-{d:02}"
            out.append((_date(m.group(), "exact", "day", "resolved", [iso], iso), m))
    for m in NUM_DATE_RE.finditer(text):
        a, b, y = int(m[1]), int(m[2]), int(m[3])
        if not a or not b or a > 31 or b > 31: continue
        if a <= 12 and b <= 12 and a != b:
            out.append((_date(m.group(), "exact", "day", "ambiguous", [f"{y:04}-{b:02}-{a:02}", f"{y:04}-{a:02}-{b:02}"], None), m))
        else:
            d, mon = (b, a) if b > 12 else (a, b)
            if d <= 31 and mon <= 12:
                iso = f"{y:04}-{mon:02}-{d:02}"
                out.append((_date(m.group(), "exact", "day", "resolved", [iso], iso), m))
    out.extend((_date(m.group(), "relative", "relative", "unresolved", [], None), m) for m in RELATIVE_RE.finditer(text))
    return sorted(out, key=lambda pair: pair[1].start())


def _fact_status(full: str) -> str:
    lower = full.lower()
    if "admitted" in lower or "undisputed" in lower: return "admitted"
    if "disputed" in lower or "denies" in lower: return "disputed"
    if "material fact" in lower: return "material"
    return "pleaded"


def _polarity(label: str) -> tuple[str, bool]:
    lower = label.lower()
    neg = bool(re.search(r"\b(?:not|never|no)\b", lower))
    return _compact(re.sub(r"\b(?:not|never|no)\b|[^a-z0-9 ]", " ", lower), 300), neg


def extract_from_pages(*, case_id: str, jurisdiction: str, pages: list[dict[str, Any]], classifier_id: str = "caseforge.triage.heuristic.v1", recognizer_id: str = "caseforge.recognize.provisional.v1") -> dict[str, Any]:
    at = _now(); jur = jurisdiction if jurisdiction in ("India", "United States", "Other") else "Other"
    nodes: list[dict[str, Any]] = []; parties: list[dict[str, Any]] = []; chronology: list[dict[str, Any]] = []
    statutes: list[dict[str, Any]] = []; evidence: list[dict[str, Any]] = []; relief: list[dict[str, Any]] = []
    procedural: dict[str, Any] | None = None; seen: set[str] = set()
    for page in pages:
        text = str(page.get("text") or ""); doc = str(page.get("doc_id") or "unknown")
        index = int(page.get("page_index") or 0); base = float(page.get("mean_confidence") or 0.7)
        for m in PARTY_RE.finditer(text):
            role, name = m[1].lower(), _compact(m[2], 140); key = f"party:{name.lower()}"
            if key in seen: continue
            seen.add(key); side = "initiating" if role in ("plaintiff", "petitioner", "applicant", "appellant", "prosecution") else "responding"
            alias_match = re.search(r"(?:also known as|a\.?k\.?a\.?)\s+([A-Z][A-Za-z0-9 .'-]{2,80})", name, re.I)
            aliases = [_compact(alias_match[1], 100)] if alias_match else []
            p = _prov(_pointer(doc, index, m), min(base, .94), at, notes="party caption pattern"); node_id = f"party_{len(parties)+1:04}"
            nodes.append(_node(node_id, "party", name, "pleaded", _attrs(role=role, side=side, aliases=aliases), p))
            parties.append({"party_id": node_id, "display_name": name, "role": role, "side": side, "aliases": aliases, "provenance": p})
        for kind, default_status, pattern in NODE_PATTERNS:
            for m in pattern.finditer(text):
                label = _compact(m[1]); key = f"{kind}:{label.lower()}"
                if key in seen: continue
                seen.add(key); status = _fact_status(m.group()) if kind == "fact" else default_status
                p = _prov(_pointer(doc, index, m), min(base, .88), at, notes=f"{kind} labelled extraction")
                nodes.append(_node(f"{kind}_{1+sum(n['node_type']==kind for n in nodes):04}", kind, label, status, _attrs(statement_kind="document_statement" if kind == "statement" else None), p))
        for expr, m in _dates(text):
            lo = max(text.rfind("\n", 0, m.start()) + 1, 0); hi0 = text.find("\n", m.end()); hi = len(text) if hi0 < 0 else hi0
            summary = _compact(text[lo:hi]); p = _prov(_pointer(doc, index, m), min(base, .9), at, notes="date expression plus source sentence")
            node_id = f"event_{len(chronology)+1:04}"; resolved = expr["resolved"]; raw = expr["raw"]
            event_type = "hearing" if "hearing" in summary.lower() else "order" if "order" in summary.lower() else "filing" if "filed" in summary.lower() else "event"
            nodes.append(_node(node_id, "event", summary, "unknown", _attrs(date=expr, event_type=event_type), p))
            chronology.append({"event_id": node_id, "date": resolved or raw, "date_precision": expr["precision"], "date_expression": expr, "summary": summary, "sort_key": resolved or f"~{raw}", "event_type": event_type, "provenance": p})
        for m in LAW_RE.finditer(text):
            label = _compact(m.group(), 220); key = f"law:{label.lower()}"
            if key in seen: continue
            seen.add(key); lower = label.lower()
            law_type = "rule" if lower.startswith("rule") else "authority" if re.search(r"\d+\s+(?:u\.s\.|f\.|scc|air)", lower) else "provision" if lower.startswith(("section", "article", "§")) else "statute"
            p = _prov(_pointer(doc, index, m), min(base, .9), at, notes="legal reference pattern"); node_id = f"law_{len(statutes)+1:04}"
            nodes.append(_node(node_id, law_type, label, "pleaded", _attrs(jurisdiction=jur, citation=label), p))
            statutes.append({"statute_id": node_id, "label": label, "law_type": law_type, "jurisdiction": jur, "citation_raw": label, "provenance": p})
        for m in RELIEF_RE.finditer(text):
            label = _compact(m[1]); p = _prov(_pointer(doc, index, m), min(base, .86), at, notes="relief labelled extraction"); node_id = f"relief_{len(relief)+1:04}"
            nodes.append(_node(node_id, "relief", label, "pleaded", _attrs(), p)); relief.append({"relief_id": node_id, "text": label, "quantum": None, "provenance": p})
        for m in EVIDENCE_RE.finditer(text):
            kind, label = m[1].lower(), _compact(m[2], 320); node_kind = "exhibit" if kind in ("exhibit", "annexure") else "document" if kind == "document" else "evidence"
            p = _prov(_pointer(doc, index, m), min(base, .88), at, notes="evidence/document label"); node_id = f"evidence_{len(evidence)+1:04}"
            nodes.append(_node(node_id, node_kind, label, "unknown", _attrs(document_id=doc), p)); evidence.append({"evidence_id": node_id, "label": label, "doc_id": doc, "kind": node_kind, "provenance": p})
        if procedural is None and (m := PROCEDURE_RE.search(text)):
            label = _compact(m[1]); p = _prov(_pointer(doc, index, m), min(base, .84), at, notes="procedural label")
            nodes.append(_node("procedure_0001", "procedure", label, "unknown", _attrs(), p)); procedural = {"court": None, "stage": None, "summary": label, "provenance": p}

    # Exact alias resolution is explicit and never merges merely similar names.
    for party in parties:
        for alias in party["aliases"]:
            source_node = next(n for n in nodes if n["node_id"] == party["party_id"]); p = _prov(source_node["provenance"]["source"], source_node["provenance"]["confidence"], at, "INFERRED", "alias candidate; not a document fact")
            nodes.append(_node(f"alias_{1+sum(n['node_type']=='interpretation' for n in nodes):04}", "interpretation", alias, "inferred", _attrs(alternative_group=party["party_id"]), p))

    edges: list[dict[str, Any]] = []
    specs = (("fact","document","fact_to_document"),("fact","evidence","fact_to_evidence"),("fact","exhibit","fact_to_evidence"),("issue","fact","issue_to_fact"),("issue","statute","issue_to_law"),("issue","rule","issue_to_law"),("issue","authority","issue_to_law"),("issue","provision","issue_to_law"),("argument","fact","argument_to_support"),("argument","evidence","argument_to_support"),("witness","statement","witness_to_statement"),("sub_issue","issue","sub_issue_of"),("procedure","order","procedure_to_order"))
    source_key = lambda n: (n["provenance"]["source"]["doc_id"], n["provenance"]["source"]["page_index"])
    for from_kind, to_kind, edge_kind in specs:
        for left in (n for n in nodes if n["node_type"] == from_kind):
            right = next((n for n in nodes if n["node_type"] == to_kind and source_key(n) == source_key(left)), None)
            if right:
                p = _prov(left["provenance"]["source"], min(left["provenance"]["confidence"], right["provenance"]["confidence"]) * .9, at, notes="co-source relationship candidate"); p["sources"] = [left["provenance"]["source"], right["provenance"]["source"]]
                edges.append({"edge_id": f"edge_{len(edges)+1:04}", "edge_type": edge_kind, "from_node_id": left["node_id"], "to_node_id": right["node_id"], "status": "candidate", "provenance": p})

    contradictions: list[dict[str, Any]] = []; facts = [n for n in nodes if n["node_type"] == "fact"]
    for i, left in enumerate(facts):
        for right in facts[i+1:]:
            left_key, left_neg = _polarity(left["label"]); right_key, right_neg = _polarity(right["label"])
            if left_key and left_key == right_key and left_neg != right_neg:
                ls, rs = left["provenance"]["source"], right["provenance"]["source"]
                p = _prov(ls, .78, at, "INFERRED", "explicit opposing polarity; counsel review required"); p["sources"] = [ls, rs]
                contradictions.append({"contradiction_id": f"contradiction_{len(contradictions)+1:04}", "summary": "Explicit affirmative and negative statements concern the same normalized proposition", "left_ref": left["node_id"], "right_ref": right["node_id"], "left_source": ls, "right_source": rs, "basis": "explicit_negation", "status": "candidate", "severity": "medium", "confidence": .78, "provenance": p})

    chronology.sort(key=lambda e: e["sort_key"]); gaps: list[dict[str, Any]] = []
    concepts = (("parties", "Who are the parties, aliases, roles and sides?", any(n["node_type"] == "party" for n in nodes)), ("claims", "What claims, grounds, defences or counterclaims are pleaded?", any(n["node_type"] in ("claim","cause_or_ground","defence","counterclaim") for n in nodes)), ("issues", "What issues and sub-issues require determination?", any(n["node_type"] in ("issue","sub_issue") for n in nodes)), ("relief", "What relief or prayers are sought?", bool(relief)), ("facts", "Which facts are material, admitted or disputed?", any(n["node_type"] == "fact" for n in nodes)), ("evidence", "What evidence, exhibits, documents, witnesses and statements support the matter?", bool(evidence)), ("chronology", "What are the key dates, ranges, relative dates and disputed events?", bool(chronology)), ("procedure", "What applications, hearings and orders form the procedural history?", any(n["node_type"] in ("procedure","application","hearing","order") for n in nodes)), ("law", "What statutes, rules, authorities and pleaded provisions apply?", bool(statutes)), ("support_links", "Which facts, evidence, issues, law and arguments are linked?", bool(edges)))
    for concept, prompt, found in concepts:
        if not found: gaps.append({"gap_id": f"gap_{concept}", "concept": concept, "topic": concept, "reason": "not_found", "question_prompt": prompt, "status": "open", "answered_fact_id": None})
    for event in chronology:
        if event["date_expression"]["status"] in ("ambiguous", "unresolved"):
            gaps.append({"gap_id": f"gap_date_{event['event_id']}", "concept": "chronology_date", "topic": "chronology", "reason": "ambiguous", "question_prompt": f"Resolve date expression '{event['date_expression']['raw']}' from its source.", "status": "open", "answered_fact_id": None})
    score = 20 * sum(bool(x) for x in (parties, chronology, statutes, evidence, relief))
    return {"schema_version": "2.0.0", "case_id": case_id, "jurisdiction": jur, "generated_at": at,
            "migration": {"strategy": "normalized_v2_with_v1_read_projections", "legacy_projections": True, "migrated_from": None},
            "pipeline": {"extractor_id": EXTRACTOR_ID, "recognizer_id": recognizer_id, "classifier_id": classifier_id, "entity_resolver_id": ENTITY_RESOLVER_ID, "relationship_builder_id": RELATIONSHIP_BUILDER_ID, "inference_policy": "inferences_are_never_document_facts", "notes": "four separated deterministic phases; normalized nodes/edges authoritative"},
            "completeness": {"score_id": "caseforge.prep_completeness.v1", "score_0_100": score, "open_gaps": len(gaps), "formula": "20 points for each non-empty legacy projection among parties, chronology, statutes, evidence, prayer_relief (max 100)."},
            "nodes": nodes, "edges": edges, "parties": parties, "chronology": chronology, "statutes": statutes, "evidence": evidence, "procedural_posture": procedural, "prayer_relief": relief, "contradictions": contradictions, "gaps": gaps, "counsel_facts": []}


def assert_every_extracted_has_source(graph: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for collection in ("nodes", "edges"):
        for index, item in enumerate(graph.get(collection) or []):
            prov = item.get("provenance") or {}
            if not prov.get("sources"): errors.append(f"{collection}[{index}]: missing sources")
            for field in ("confidence", "origin", "pipeline_version", "recorded_at", "counsel_confirmed", "correction_lineage"):
                if field not in prov or prov[field] is None: errors.append(f"{collection}[{index}]: missing {field}")
    for index, item in enumerate(graph.get("contradictions") or []):
        if not item.get("left_source") or not item.get("right_source"): errors.append(f"contradictions[{index}]: both source sides required")
    return errors
