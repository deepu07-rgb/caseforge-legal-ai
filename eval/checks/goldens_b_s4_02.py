#!/usr/bin/env python3
"""B-S4-02 Stage 4 golden sets and legal-editor protocol: the harness discovers every Stage 4 set and reports honestly.

Checks (golden/STAGE4_MANIFEST.json is the source; eval/tools/make_stage4_manifest.py writes it):
  * all twelve sets exist, each with a README that declares its provenance and at least one fixture;
  * every fixture in the manifest exists and its SHA-256 matches (change control: a changed fixture must be re-recorded);
  * every provenance is one the policy allows, no fixture claims more review than it has (none is GOLDEN here: nothing has had the independent
    review golden/POLICY.md requires), and there is no obvious client material (a heuristic, stated as one);
  * fixtures that embed a schema-shaped object satisfy that schema;
  * every planted case the playbook lists is carried by at least one fixture, and which of India / the United States each is carried for is
    detected from the fixture's text and reported (a feature that applies to both but is carried for neither real jurisdiction is reported
    SYNTHETIC_ONLY, not covered);
  * every legal answer (G-CODEBRIDGE, G-ADMISS, G-DEADLINE) is BLOCKED unless a legal-editor record that counts exists.

It fails only on a structural defect. It never turns a missing sign-off, a missing set of scans or a jurisdiction gap into a pass.

Usage: python eval/checks/goldens_b_s4_02.py
"""
from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOLDEN = ROOT / "golden"
MANIFEST = GOLDEN / "STAGE4_MANIFEST.json"
SIGNOFFS = ROOT / "eval" / "signoffs" / "legal_editors.json"
TWELVE = ["G-AUDIT", "G-OPPOSING", "G-IMPEACH", "G-CODEBRIDGE", "G-ADMISS", "G-DEADLINE", "G-TREATMENT", "G-INDIC", "G-BUNDLE", "G-CHAMBERS", "G-CAUSELIST", "G-REDTEAM-S4"]
ALLOWED_PROVENANCE = {"SYNTHETIC_CONTROLLED", "ADVERSARIAL_SYNTHETIC", "PUBLIC_DOMAIN", "RIGHTS_CLEARED"}
# Planted on purpose in the fixtures; anything else that looks like a real contact is flagged.
KNOWN_SYNTHETIC_IDENTIFIERS = {"98765 43210", "9876543210", "ramesh.k@example.org", "RAMESH.K@EXAMPLE.ORG"}
EMAIL = re.compile(r"[\w.+-]+@([\w-]+\.)+[a-z]{2,}", re.I)
SAFE_EMAIL_DOMAINS = ("example.org", "example.com", "example.net", "invalid", "test", "localhost")
MOBILE = re.compile(r"(?<!\d)(?:\+?91[ -]?)?[6-9]\d{4}[ -]?\d{5}(?!\d)")


def load(p: Path):
    return json.loads(p.read_text(encoding="utf-8-sig"))


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def valid_signoffs(manifest: dict, path: Path = SIGNOFFS) -> dict[str, list[dict]]:
    """AC id -> the sign-off records that count (see docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md). Used by the ship gate too."""
    need: dict[str, dict] = {}
    for s in manifest["sets"]:
        for f in s["fixtures"]:
            so = f.get("legal_editor_signoff", {})
            if so.get("required"):
                need[f["path"]] = {"ac": so.get("ac"), "sha256": f["sha256"]}
    out: dict[str, list[dict]] = {}
    if not path.is_file():
        return out
    for r in load(path).get("signoffs", []):
        n = need.get(r.get("fixture", ""))
        if not n or r.get("ac") != n["ac"] or r.get("fixture_sha256") != n["sha256"] or not str(r.get("jurisdiction", "")).strip():
            continue
        names = {e.get("name", "").strip().lower() for e in r.get("editors", []) if e.get("decision") == "APPROVED" and e.get("name", "").strip()}
        if len(names) >= 2:
            out.setdefault(n["ac"], []).append({"fixture": r["fixture"], "jurisdiction": r["jurisdiction"], "editors": sorted(names)})
    return out


def jurisdictions_in(text: str, markers: dict) -> list[str]:
    return [j for j in ("IN", "US") if re.search(markers[j], text)]


def main() -> int:
    errors: list[str] = []
    m = load(MANIFEST)
    markers = m["jurisdiction_markers"]
    by_id = {s["id"]: s for s in m["sets"]}
    if sorted(by_id) != sorted(TWELVE):
        errors.append(f"the manifest must list exactly the twelve sets; it lists {sorted(by_id)}")

    sets_report = []
    fixture_text: dict[str, str] = {}
    for sid in TWELVE:
        s = by_id.get(sid)
        row = {"set": sid, "readme": False, "fixtures": 0, "hash_ok": 0, "problems": []}
        d = GOLDEN / sid
        if not d.is_dir() or not s:
            row["problems"].append("set missing")
            errors.append(f"{sid}: set missing")
            sets_report.append(row)
            continue
        readme = d / "README.md"
        row["readme"] = readme.is_file()
        if not readme.is_file():
            errors.append(f"{sid}: README.md missing")
        else:
            t = readme.read_text(encoding="utf-8")
            if "Provenance" not in t or "No client material" not in t:
                errors.append(f"{sid}: README does not declare its provenance")
        if s["source_class"] not in ALLOWED_PROVENANCE and not s.get("blocked_by"):
            errors.append(f"{sid}: provenance {s['source_class']!r} is not one the policy allows")
        for f in s["fixtures"]:
            row["fixtures"] += 1
            p = GOLDEN / f["path"]
            if not p.is_file():
                errors.append(f"{f['path']}: missing")
                continue
            if sha256(p) != f["sha256"]:
                errors.append(f"{f['path']}: changed since it was recorded in the manifest (re-run eval/tools/make_stage4_manifest.py and say why in the commit; any legal-editor sign-off on it is void)")
            else:
                row["hash_ok"] += 1
            if f["policy_status"] == "GOLDEN" or f.get("independent_review"):
                errors.append(f"{f['path']}: claims a review or lock no one has done")
            if f["kind"] == "LEGAL_ANSWER" and not f["legal_editor_signoff"]["required"]:
                errors.append(f"{f['path']}: a legal answer without a sign-off requirement")
            txt = p.read_text(encoding="utf-8-sig")
            fixture_text[f["path"]] = txt
            if f["kind"] != "PLACEHOLDER":
                try:
                    obj = json.loads(txt)
                except Exception as e:  # noqa: BLE001
                    errors.append(f"{f['path']}: not valid JSON: {e}")
                    continue
                for chk in f.get("schema_checks", []):
                    try:
                        import jsonschema

                        node = obj
                        for part in [x for x in chk["pointer"].split("/") if x]:
                            node = node[part]
                        jsonschema.Draft202012Validator(load(ROOT / chk["schema"]), format_checker=jsonschema.FormatChecker()).validate(node)
                    except Exception as e:  # noqa: BLE001
                        errors.append(f"{f['path']}{chk['pointer']}: does not satisfy {chk['schema']}: {str(e)[:160]}")
                # client-material heuristic
                for em in EMAIL.finditer(txt):
                    if em.group(0) not in KNOWN_SYNTHETIC_IDENTIFIERS and not em.group(0).lower().endswith(SAFE_EMAIL_DOMAINS):
                        errors.append(f"{f['path']}: an email address that is not on a reserved example domain: {em.group(0)}")
                for mo in MOBILE.finditer(txt):
                    if mo.group(0).strip() not in KNOWN_SYNTHETIC_IDENTIFIERS:
                        errors.append(f"{f['path']}: something that looks like a real mobile number: {mo.group(0)}")
        sets_report.append(row)

    # ---- planted cases ----
    synthetic_rules = {f["path"] for s in m["sets"] for f in s["fixtures"] if f.get("synthetic_rules")}
    planted = []
    for c in m["planted_cases"]:
        entry = {"id": c["id"], "prd": c["prd"], "applies_to": c["applies_to"], "fixtures": [], "detected_jurisdictions": [], "status": ""}
        if c.get("blocked_by"):
            entry["status"] = "BLOCKED"
            entry["blocked_by"] = c["blocked_by"]
            planted.append(entry)
            continue
        found_in: set[str] = set()
        detected: set[str] = set()
        for ev in c["evidence"]:
            key = f"{ev['file']}"
            txt = fixture_text.get(key)
            if txt is None:
                p = GOLDEN / key
                txt = p.read_text(encoding="utf-8-sig") if p.is_file() else ""
            if re.search(ev["regex"], txt):
                found_in.add(key)
                if key not in synthetic_rules:  # rules invented for the test say nothing about a real jurisdiction
                    detected.update(jurisdictions_in(txt, markers))
        entry["fixtures"] = sorted(found_in)
        entry["detected_jurisdictions"] = sorted(detected)
        if not found_in:
            entry["status"] = "MISSING"
            errors.append(f"planted case {c['id']} ({c['prd']}) is carried by no fixture")
        else:
            want = set(c["applies_to"])
            entry["status"] = "COVERED" if want <= detected else "PARTIAL" if detected & want else "SYNTHETIC_ONLY"
            if c.get("neutral"):
                entry["note"] = c["neutral"]
            if entry["status"] != "COVERED":
                entry["missing_jurisdictions"] = sorted(want - detected)
        planted.append(entry)

    # ---- legal answers and sign-off ----
    counted = valid_signoffs(m)
    legal = []
    for s in m["sets"]:
        for f in s["fixtures"]:
            so = f["legal_editor_signoff"]
            if so["required"]:
                recs = [r for r in counted.get(so["ac"], []) if r["fixture"] == f["path"]]
                legal.append({"fixture": f["path"], "ac": so["ac"], "status": "SIGNED" if recs else "BLOCKED", "jurisdictions_signed": [r["jurisdiction"] for r in recs], "reason": None if recs else "no legal editor has signed (Q-S4-02)"})
    sign_file_problems = []
    if SIGNOFFS.is_file():
        for r in load(SIGNOFFS).get("signoffs", []):
            if not any(r.get("fixture") == c["fixture"] and r.get("jurisdiction") in c["jurisdictions_signed"] for c in legal):
                sign_file_problems.append(f"a sign-off record for {r.get('fixture')} ({r.get('jurisdiction')}) does not count: wrong fixture, changed fixture, fewer than two APPROVED editors, or no jurisdiction")
    if sign_file_problems:
        errors.extend(sign_file_problems)

    counts = {k: sum(1 for p in planted if p["status"] == k) for k in ("COVERED", "PARTIAL", "SYNTHETIC_ONLY", "BLOCKED", "MISSING")}
    report = {
        "batch": "B-S4-02",
        "acs": {},
        "sets": sets_report,
        "planted_cases": planted,
        "planted_counts": counts,
        "legal_answers": legal,
        "fixture_status": {"GOLDEN": 0, "DRAFT_UNREVIEWED": sum(1 for s in m["sets"] for f in s["fixtures"] if f["policy_status"] == "DRAFT_UNREVIEWED"), "PLACEHOLDER": sum(1 for s in m["sets"] for f in s["fixtures"] if f["policy_status"] == "PLACEHOLDER")},
        "reading": "Every fixture is a draft written by a coding agent; none has had the independent review that locks it GOLDEN. They are sound regression tests of the software. The legal answers are BLOCKED until two named legal editors sign them per jurisdiction. Several features that apply to both India and the United States are carried only by synthetic-jurisdiction fixtures (SYNTHETIC_ONLY / PARTIAL): real-jurisdiction coverage needs editor-supplied sources. The Devanagari set is a scaffold (BLOCKED).",
        "errors": errors,
    }
    print(json.dumps({k: report[k] for k in ("planted_counts", "legal_answers", "fixture_status", "errors")}, indent=2))
    for p in planted:
        extra = f" missing {p['missing_jurisdictions']}" if p.get("missing_jurisdictions") else ""
        print(f"[{p['status']:14}] {p['id']} ({p['prd']}) -> {', '.join(p['fixtures']) or p.get('blocked_by')}{extra}")
    out = ROOT / "eval" / "reports" / "b_s4_02_goldens.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
