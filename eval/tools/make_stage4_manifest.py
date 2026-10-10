#!/usr/bin/env python3
"""Writes golden/STAGE4_MANIFEST.json and the README of each Stage 4 golden set (B-S4-02).

The curated facts live here (which set belongs to which criteria, what each fixture is, which planted cases it carries); the file
hashes are computed. Running this is the change-control act: it records the fixtures as they are now. A sign-off by a legal editor
is bound to a fixture's hash, so re-running after a fixture changed voids that fixture's sign-offs on purpose.

Usage: python eval/tools/make_stage4_manifest.py [--check]
"""
from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
GOLDEN = ROOT / "golden"
MANIFEST = GOLDEN / "STAGE4_MANIFEST.json"

IN_MARKERS = r"\bSCC\b|\bAIR\b|India|Indian|\bBNSS?\b|\bCPC\b|Limitation Act|Evidence Act|High Court|भारतीय|न्यायालय|दंड संहिता"
US_MARKERS = r"F\.3d|F\. ?Supp|\bU\.S\.|United States|\bFRCP\b|\bFRE\b|\bCir\.|Fed\. R\."

INDIC_PAGES = ["hi_fir_clean_01", "hi_order_clean_02", "hi_statement_clean_03", "hi_notice_clean_04", "hi_fir_degraded_05", "hi_order_degraded_06", "mixed_order_07", "latin_control_08"]
INDIC_FIXTURES = []
for _p in INDIC_PAGES:
    INDIC_FIXTURES.append({"file": f"{_p}/page.png", "kind": "IMAGE", "engine_batch": "B-S4-14", "pinned_before_engine": True, "schema_checks": [], "note": "A page rendered with Windows text shaping from the text in truth.json (eval/tools/make_indic_golden.py); not a real scan."})
    INDIC_FIXTURES.append({"file": f"{_p}/truth.json", "kind": "MECHANICAL", "engine_batch": "B-S4-14", "pinned_before_engine": True, "schema_checks": []})

# set id -> curated record
SETS: dict[str, dict] = {
    "G-AUDIT": {
        "purpose": "Citation & Quote Audit and the Audit Certificate: planted fabricated citations, single-word misquotes and wrong pinpoints in an Indian and a US draft.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-01.1", "AC-S4-01.2", "AC-S4-01.3", "AC-S4-01.4", "AC-S4-01.5"],
        "fixtures": [
            {"file": "audit_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-04", "pinned_before_engine": True, "schema_checks": []},
            {"file": "certificate_reference.json", "kind": "MECHANICAL", "engine_batch": "B-S4-05", "pinned_before_engine": False, "schema_checks": [{"pointer": "/certificate", "schema": "contracts/audit_certificate.schema.json"}], "note": "A certificate and draft that the independent offline verifier must accept; added with the engine."},
        ],
    },
    "G-OPPOSING": {
        "purpose": "Opposing Filing scan: an opposing reply with fabricated and misquoted authorities, assertions that conflict with the record, an unanswered point, and an instruction planted inside the filing.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-02.1", "AC-S4-02.2", "AC-S4-02.3", "AC-S4-02.4"],
        "fixtures": [{"file": "respondent_reply.json", "kind": "MECHANICAL", "engine_batch": "B-S4-06", "pinned_before_engine": True, "schema_checks": []}],
    },
    "G-IMPEACH": {
        "purpose": "Witness matrix and Impeachment Kit: statements by the same witness that contradict or omit, in India and the United States.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-03.1", "AC-S4-03.2", "AC-S4-03.3"],
        "fixtures": [{"file": "impeach_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-07/08", "pinned_before_engine": True, "schema_checks": []}],
    },
    "G-CODEBRIDGE": {
        "purpose": "Code Bridge: statute succession (old code to new code), temporal routing by key dates, missing key dates, modified mappings, and tampered rule tables.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-04.1", "AC-S4-04.2", "AC-S4-04.3"],
        "fixtures": [{"file": "code_bridge_core.json", "kind": "LEGAL_ANSWER", "synthetic_rules": True, "signoff_ac": "AC-S4-04.1", "engine_batch": "B-S4-09", "pinned_before_engine": True, "schema_checks": [{"pointer": "/rule_table", "schema": "contracts/rule_table.schema.json"}], "note": "Uses an invented jurisdiction (Demoland) so that no real legal mapping is asserted. Real mappings need an editor-authored table."}],
    },
    "G-ADMISS": {
        "purpose": "Admissibility & Foundation Gaps: evidence items with required foundation elements present or missing, and the order of the Missing Links panel.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-05.1", "AC-S4-05.3"],
        "fixtures": [{"file": "foundation_core.json", "kind": "LEGAL_ANSWER", "synthetic_rules": True, "signoff_ac": "AC-S4-05.1", "engine_batch": "B-S4-10", "pinned_before_engine": True, "schema_checks": [], "note": "Development rows, not a statement of what any court requires."}],
    },
    "G-DEADLINE": {
        "purpose": "Deadline & Limitation Watch: 21 synthetic cases with hand-derived dates (leap days, month ends, holiday roll-overs, exclusions) and the cases that must give a range or no deadline.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-06.1", "AC-S4-06.2", "AC-S4-06.3"],
        "fixtures": [{"file": "deadline_core.json", "kind": "LEGAL_ANSWER", "synthetic_rules": True, "signoff_ac": "AC-S4-06.1", "engine_batch": "B-S4-11", "pinned_before_engine": True, "schema_checks": [], "note": "Synthetic rules and calendars. A real limitation window or court calendar is not asserted."}],
    },
    "G-TREATMENT": {
        "purpose": "Law Currency Stamp and Treatment Graph: sentences with treatment language (overruled, reversed, distinguished, followed, doubted, per incuriam, referred to a larger bench) and the sentences that merely mention an authority.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-08.1", "AC-S4-08.2", "AC-S4-08.3"],
        "fixtures": [{"file": "treatment_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-13", "pinned_before_engine": True, "schema_checks": []}],
    },
    "G-INDIC": {
        "purpose": "Hindi (Devanagari) ingestion: eight synthetic pages (FIR, order, statement, notice, two degraded copies, a mixed Hindi/English page and an English control) with exact ground truth, for OCR accuracy, script detection and critical-field checks. Translation fixtures (AC-S4-09.2, 09.3) are added with B-S4-15.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-09.1", "AC-S4-09.2", "AC-S4-09.3"],
        "fixtures": INDIC_FIXTURES,
    },
    "G-BUNDLE": {
        "purpose": "Encrypted Bundle Exchange: redaction rules and the byte-level absence that must follow, tamper and wrong-key behaviour, how a received correction is classified against the recipient's own, and the addresses a LAN transfer may use.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-10.1", "AC-S4-10.2", "AC-S4-10.3", "AC-S4-10.4"],
        "fixtures": [{"file": "bundle_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-16", "pinned_before_engine": True, "schema_checks": []}],
    },
    "G-CHAMBERS": {
        "purpose": "Chambers Edition: record-only classification (G7), the issue matrix and its fallback pairing, record lookup, and the judgment scaffold.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-11.1", "AC-S4-11.2", "AC-S4-12.1", "AC-S4-12.2", "AC-S4-13.1", "AC-S4-13.2", "AC-S4-15.1", "AC-S4-15.3"],
        "fixtures": [
            {"file": "record_only_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-17", "pinned_before_engine": True, "schema_checks": []},
            {"file": "issue_matrix_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-18", "pinned_before_engine": True, "schema_checks": []},
            {"file": "issue_matrix_fallback.json", "kind": "MECHANICAL", "engine_batch": "B-S4-18", "pinned_before_engine": True, "schema_checks": []},
            {"file": "record_lookup_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-19", "pinned_before_engine": True, "schema_checks": []},
            {"file": "judgment_scaffold_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-21", "pinned_before_engine": True, "schema_checks": []},
        ],
    },
    "G-CAUSELIST": {
        "purpose": "Daily Cause-List Brief: an imported cause list, orders in the record, confirmation of each matter, and a 100-matter budget case.",
        "source_class": "SYNTHETIC_CONTROLLED",
        "acs": ["AC-S4-14.1", "AC-S4-14.2"],
        "fixtures": [{"file": "cause_list_core.json", "kind": "MECHANICAL", "engine_batch": "B-S4-20", "pinned_before_engine": True, "schema_checks": []}],
    },
    "G-REDTEAM-S4": {
        "purpose": "Stage 4 red-team: witness-coaching requests (G3), Chambers requests for a decision and decisional statements (G8), and statements that reach outside the record (G7).",
        "source_class": "ADVERSARIAL_SYNTHETIC",
        "acs": ["AC-S4-03.4", "AC-S4-12.3", "AC-S4-15.2", "AC-S4-RT-RED"],
        "fixtures": [
            {"file": "witness_coaching.json", "kind": "ADVERSARIAL", "engine_batch": "B-S4-08", "pinned_before_engine": True, "schema_checks": []},
            {"file": "chambers_decision_zone.json", "kind": "ADVERSARIAL", "engine_batch": "B-S4-21", "pinned_before_engine": True, "schema_checks": []},
            {"file": "chambers_extra_record.json", "kind": "ADVERSARIAL", "engine_batch": "B-S4-23", "pinned_before_engine": False, "schema_checks": [], "note": "Written after the record-only classifier, by the same author: a probing set, not an independent one."},
        ],
    },
}

# The planted cases the playbook lists (B-S4-02), with where each is carried. `applies_to` is where the feature applies.
PLANTED: list[dict] = [
    {"id": "fabricated_citations", "prd": "§7.1", "applies_to": ["IN", "US"], "evidence": [{"file": "G-AUDIT/fixtures/audit_core.json", "regex": "NOT_IN_CORPUS"}]},
    {"id": "single_word_misquotes", "prd": "§7.1", "applies_to": ["IN", "US"], "evidence": [{"file": "G-AUDIT/fixtures/audit_core.json", "regex": "MISQUOTED"}]},
    {"id": "wrong_pinpoints", "prd": "§7.1", "applies_to": ["IN", "US"], "evidence": [{"file": "G-AUDIT/fixtures/audit_core.json", "regex": "(?i)pinpoint"}]},
    {"id": "opponent_record_conflicts", "prd": "§7.2", "applies_to": ["IN", "US"], "evidence": [{"file": "G-OPPOSING/fixtures/respondent_reply.json", "regex": "(?i)conflict"}]},
    {"id": "injection_inside_external_filings", "prd": "§7.2", "applies_to": ["IN", "US"], "evidence": [{"file": "G-OPPOSING/fixtures/respondent_reply.json", "regex": "(?i)ignore (all|previous|prior)|injection"}]},
    {"id": "witness_contradictions", "prd": "§7.3", "applies_to": ["IN", "US"], "evidence": [{"file": "G-IMPEACH/fixtures/impeach_core.json", "regex": "CONTRADICT"}]},
    {"id": "witness_omissions", "prd": "§7.3", "applies_to": ["IN", "US"], "evidence": [{"file": "G-IMPEACH/fixtures/impeach_core.json", "regex": "(?i)omission"}]},
    {"id": "missing_key_dates", "prd": "§7.4", "applies_to": ["IN"], "evidence": [{"file": "G-CODEBRIDGE/fixtures/code_bridge_core.json", "regex": "planted_missing_date_refs|NEEDS_COUNSEL_INPUT"}]},
    {"id": "missing_foundation_certificates", "prd": "§7.5", "applies_to": ["IN", "US"], "evidence": [{"file": "G-ADMISS/fixtures/foundation_core.json", "regex": "(?i)certificate"}]},
    {"id": "holiday_rollovers", "prd": "§7.6", "applies_to": ["IN", "US"], "evidence": [{"file": "G-DEADLINE/fixtures/deadline_core.json", "regex": "rolls_over"}]},
    {"id": "overruled_authority", "prd": "§7.8", "applies_to": ["IN", "US"], "evidence": [{"file": "G-TREATMENT/fixtures/treatment_core.json", "regex": "(?i)overruled"}]},
    {"id": "devanagari_scans", "prd": "§7.9", "applies_to": ["IN"], "evidence": [{"file": "G-INDIC/fixtures/hi_fir_clean_01/truth.json", "regex": "प्रथम सूचना रिपोर्ट"}, {"file": "G-INDIC/fixtures/hi_fir_degraded_05/truth.json", "regex": "प्रथम सूचना रिपोर्ट"}]},
    {"id": "unanswered_argument", "prd": "§9.2", "applies_to": [], "neutral": "a structural property of the record, not a rule of any jurisdiction", "evidence": [{"file": "G-CHAMBERS/fixtures/issue_matrix_core.json", "regex": "(?i)unanswered"}]},
    {"id": "extra_record_bait", "prd": "§9.1", "applies_to": [], "neutral": "a structural property of the record, not a rule of any jurisdiction", "evidence": [{"file": "G-CHAMBERS/fixtures/record_only_core.json", "regex": "EXTRA_RECORD"}, {"file": "G-REDTEAM-S4/fixtures/chambers_extra_record.json", "regex": "MODEL_MEMORY_NO_SUPPORT"}]},
    {"id": "decision_solicitation_prompts", "prd": "§9.5", "applies_to": [], "neutral": "a refusal rule that holds in every court", "evidence": [{"file": "G-REDTEAM-S4/fixtures/chambers_decision_zone.json", "regex": "(?i)write the analysis"}]},
]

README = """# {sid}

{purpose}

| | |
|---|---|
| Source class | {source_class} |
| Criteria | {acs} |
| Status | {status} |

## Provenance

Every fixture here is {prov}. No client material, no confidential papers, and no real person's data. The authoring and review state of each
fixture is recorded in `golden/STAGE4_MANIFEST.json` (file, SHA-256, who wrote it, whether it was pinned before the engine, what independent
review exists).

## Fixtures

{fixtures}

## How to read the status

Under `golden/POLICY.md` a fixture is `GOLDEN` only after an independent human review and lock. The fixtures in this set were written by a coding
agent at the owner's instruction and committed before the code they test (where the manifest says so), which makes them reliable regression
tests of the software but not independently reviewed labels. The manifest records them as `DRAFT_UNREVIEWED` for that reason.
{legal}
## Changing a fixture

Edit the file, run `python eval/tools/make_stage4_manifest.py`, and describe the change in the commit. The new hash replaces the old one in the
manifest, and any legal-editor sign-off bound to the old hash no longer counts (`docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md`).
"""


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def first_commit(p: Path) -> str | None:
    try:
        out = subprocess.run(["git", "log", "--diff-filter=A", "--format=%h", "--", str(p.relative_to(ROOT)).replace("\\", "/")], cwd=ROOT, capture_output=True, text=True, timeout=30).stdout.split()
        return out[-1] if out else None
    except Exception:  # noqa: BLE001
        return None


def build() -> dict:
    sets = []
    for sid, s in SETS.items():
        fixtures = []
        for f in s["fixtures"]:
            p = GOLDEN / sid / "fixtures" / f["file"]
            fixtures.append({
                "path": f"{sid}/fixtures/{f['file']}",
                "sha256": sha256(p),
                "kind": f["kind"],
                "engine_batch": f["engine_batch"],
                "first_commit": first_commit(p),
                "pinned_before_engine": f["pinned_before_engine"],
                "authored_by": "coding agent, at the owner's instruction",
                "synthetic_rules": bool(f.get("synthetic_rules")),
                "policy_status": "PLACEHOLDER" if f["kind"] == "PLACEHOLDER" else "DRAFT_UNREVIEWED",
                "independent_review": None,
                "schema_checks": f["schema_checks"],
                "legal_editor_signoff": {"required": f["kind"] == "LEGAL_ANSWER", "status": "BLOCKED" if f["kind"] == "LEGAL_ANSWER" else "NOT_REQUIRED", "record": "eval/signoffs/legal_editors.json", **({"ac": f["signoff_ac"]} if "signoff_ac" in f else {})},
                **({"note": f["note"]} if "note" in f else {}),
            })
        sets.append({"id": sid, "readme": f"{sid}/README.md", "purpose": s["purpose"], "source_class": s["source_class"], "acs": s["acs"], "fixtures": fixtures, **({"blocked_by": s["blocked_by"]} if "blocked_by" in s else {})})
    return {
        "schema_version": "1.0.0",
        "batch": "B-S4-02",
        "description": "The twelve Stage 4 golden sets: what each holds, where it came from, its hash, and whether a legal editor has signed the legal answers in it. Written by eval/tools/make_stage4_manifest.py; checked by eval/checks/goldens_b_s4_02.py.",
        "policy": {
            "provenance_allowed": ["SYNTHETIC_CONTROLLED", "ADVERSARIAL_SYNTHETIC", "PUBLIC_DOMAIN", "RIGHTS_CLEARED"],
            "no_client_material": True,
            "legal_answer_without_signoff": "BLOCKED",
            "signoff_binds_to": "the fixture's sha256 and a named jurisdiction; two editors, each APPROVED; a changed fixture voids it",
            "agent_may_not_lock_golden": "golden/POLICY.md section 1",
            "protocol": "docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md",
        },
        "jurisdiction_markers": {"IN": IN_MARKERS, "US": US_MARKERS},
        "sets": sets,
        "planted_cases": PLANTED,
    }


def readme(sid: str, s: dict, m: dict) -> str:
    entry = next(x for x in m["sets"] if x["id"] == sid)
    fx = "\n".join(f"* `{f['path'].split('/', 2)[2]}` — {f['kind'].lower().replace('_', ' ')}; engine batch {f['engine_batch']}; {'pinned before the engine' if f['pinned_before_engine'] else 'not pinned before the engine'}" + (f". {f['note']}" if f.get("note") else "") for f in entry["fixtures"])
    legal = ""
    if any(f["kind"] == "LEGAL_ANSWER" for f in entry["fixtures"]):
        legal = "\n## Legal answers need a legal editor\n\nThe answers in this set (mappings, foundation rows, dates) are legal answers. The PRD requires a named legal editor's sign-off per jurisdiction; until it exists in `eval/signoffs/legal_editors.json` the criteria that rest on this set are `BLOCKED`, never `PASS`. The fixture uses invented or development-only rules so that it asserts no real law.\n\n"
    if sid == "G-INDIC":
        legal = "\n## Limits\n\nThe pages are **rendered**, not scanned: the text was written for the test and drawn with Windows text shaping, so the transcription is exact by construction and needs no human labelling. A rendered page is cleaner and more regular than a photographed FIR, and one typeface family is used, so an error rate measured here is a floor, not what a court's scans will give. Real pages (rights-cleared) and a second labeller are still needed before this set can be locked GOLDEN. Translation fixtures arrive with B-S4-15.\n\n"
    prov = "synthetic and written for the test" if s["source_class"] != "UNDECIDED" else "(none yet)"
    return README.format(sid=sid, purpose=s["purpose"], source_class=s["source_class"], acs=", ".join(s["acs"]), status="draft, unreviewed", prov=prov, fixtures=fx, legal=legal)


def main() -> int:
    check = "--check" in sys.argv
    m = build()
    if check:
        return 0 if json.loads(MANIFEST.read_text(encoding="utf-8")) == m else 1
    MANIFEST.write_text(json.dumps(m, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    for sid, s in SETS.items():
        (GOLDEN / sid / "README.md").write_text(readme(sid, s, m), encoding="utf-8")
    print(f"wrote {MANIFEST.relative_to(ROOT)} and {len(SETS)} READMEs")
    return 0


if __name__ == "__main__":
    sys.exit(main())
