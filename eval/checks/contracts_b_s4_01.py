#!/usr/bin/env python3
"""B-S4-01 Stage 4 contracts, constants, guardrails freeze.

Structural + adversarial gate:
  1. Every Stage 4 contract exists, parses, and is a valid JSON Schema 2020-12.
  2. Frozen Stage 1-3 contracts that Stage 4 promised not to touch are byte-identical.
  3. Each schema accepts a minimal valid instance and rejects the specific
     violation it exists to prevent (adversarial fixtures).
  4. Guardrails G7-G10 extend (never redefine) G1-G6; error codes exist; patterns
     catch decision-solicitation prompts and leave benign prompts alone.
  5. Every AC-S4-* named in the PRD/playbook is inventoried, and vice versa.
  6. PROPOSED thresholds are not leaked into constants.json; dependent ACs are BLOCKED.
  7. Owner-approved resource budget is internally consistent.

Implementation status can PASS; formal acceptance stays BLOCKED until the owner
approves Q-S4-01 thresholds. No AC-S4-* can PASS from this script.
"""

from __future__ import annotations

import copy
import hashlib
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

import jsonschema
from jsonschema import Draft202012Validator

ROOT = Path(__file__).resolve().parents[2]
C = ROOT / "contracts"
PRD = ROOT / "CaseForge_PRD_v4.0.md"
PLAYBOOK = ROOT / "docs" / "CASEFORGE_STAGE4_AGENTIC_PLAYBOOK.md"
INVENTORY = ROOT / "eval" / "stage4_ac_inventory.json"
REPORT = ROOT / "eval" / "reports" / "b_s4_01_contracts.json"

STAGE4_SCHEMAS = [
    "rule_table", "treatment_edge", "disclosure_template", "akpkg_v2_1", "licence_payload_v2",
    "audit_report", "audit_certificate", "external_filing", "witness_matrix", "impeachment_kit",
    "statute_mapping", "foundation_check", "deadline_record", "rehearsal_timeline",
    "translation_segment", "bundle_manifest", "issue_matrix", "cause_list_brief", "judgment_scaffold",
]
STAGE4_POLICY_FILES = ["guardrails_stage4.json", "stage4_thresholds.json", "stage4_resource_budget.json"]

# Contracts Stage 4 must leave byte-identical. case_graph is re-pinned by B-S4-06
# (planned v2.1.0 amendment) and supported_models by B-S4-00 (default model switch).
PINNED = {
    "akpkg.schema.json": "a7dadeb730a46423952d65c9dbbe6ebe70b4d935a339a068107678fb369af88f",
    "licence_payload.schema.json": "ef9a815b6cc45568f10e7d138a5c3f2c0ead26799162d6679a15aeba873093c0",
    "guardrails.json": "5bc5678f12bbf7bc323540a2899b8981b09a88d76a5c2f2b7d25538da15ded31",
    "document_intake.schema.json": "5932241be473229280ab48960815316babef97b59694cef3d57efe849d74d478",
    "mock_court_debrief_v2.schema.json": "ec98d625e559690be0226d8ae2d4e56742281ab02fa51dad54510c900c646bd8",
    "proposition_ledger.schema.json": "1a8ed0429288649d0e10ba7ebc74b361a487238f7eab61a0d5a831319c375520",
    "case_graph.schema.json": "5783da2511e44b8ab9e2c8aea2d31fe7978c826402d2d870a43892ef4f09b79c",
    "supported_models.json": "3fc8c6a13e4427f06ad403f33aee0e2c003d7786987df29a24b91c32854bb16a",
}
REPIN_BATCH = {"case_graph.schema.json": "B-S4-06", "supported_models.json": "B-S4-00 (re-pinned 2026-10-08 after OD-S4-MODEL-03: 7B default)"}

H64 = "a" * 64
SIG = "b" * 128
LOC = {"doc_id": "d1", "page_index": 0, "char_start": 0, "char_end": 5, "excerpt": "hello"}
LEGAL = {"pack_id": "p", "source_id": "s", "chunk_id": "c", "char_start": 0, "char_end": 5, "excerpt": "hello"}

errors: list[str] = []


def need(cond: bool, msg: str) -> None:
    if not cond:
        errors.append(msg)


def load(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def mutate(obj, path: str, value):
    """Return a deep copy with dotted path set (list indexes allowed); value DELETE removes key."""
    out = copy.deepcopy(obj)
    cur = out
    parts = path.split(".")
    for p in parts[:-1]:
        cur = cur[int(p)] if isinstance(cur, list) else cur[p]
    last = parts[-1]
    if value == "__DELETE__":
        del cur[last]
    elif isinstance(cur, list):
        cur[int(last)] = value
    else:
        cur[last] = value
    return out


# ---------- minimal valid instances ----------
def section(kind, policy, origin="JUDGE_AUTHORED"):
    sent = {"text": "x", "origin": origin, "classification": "JUDGE_TEXT" if origin == "JUDGE_AUTHORED" else "RECORD", "locators": [] if origin == "JUDGE_AUTHORED" else [LOC]}
    return {"section_kind": kind, "assist_policy": policy, "sentences": [sent]}


KINDS = ["CAUSE_TITLE", "APPEARANCES", "BRIEF_FACTS", "PROCEDURAL_HISTORY", "SUBMISSIONS", "ISSUES", "EVIDENCE_SUMMARY", "ANALYSIS", "FINDINGS", "OPERATIVE_ORDER"]
LOCKED = {"ANALYSIS", "FINDINGS", "OPERATIVE_ORDER"}

VALID = {
    "rule_table": {"schema_version": "1.0.0", "table_id": "in.limitation", "table_kind": "LIMITATION_PERIOD", "jurisdiction": "IN", "pack_id": "pack.in", "rows": [{
        "rule_id": "in.lim.example", "table_kind": "LIMITATION_PERIOD", "jurisdiction": "IN", "effective_from": "2000-01-01", "effective_to": None,
        "authorities": [{"pack_id": "pack.in", "source_id": "s", "chunk_id": "c"}],
        "legal_editor_signoff": {"editor_name": "Synthetic Editor", "editor_role": "fixture", "signed_on": "2026-10-08"}, "trust_state": "SIGNED_OFF",
        "payload": {"deadline_label": "synthetic", "trigger_event_type": "ORDER", "period": {"value": 30, "unit": "DAYS"}, "counting": {"exclude_trigger_day": True, "roll_forward_if_court_closed": True}, "exclusions": []}}]},
    "treatment_edge": {"schema_version": "1.0.0", "edge_id": "e1", "citing": {"source_id": "s1", "chunk_id": "c1", "char_start": 0, "char_end": 5, "excerpt": "hello"}, "cited_source_id": "s2", "treatment": "OVERRULED", "detection": {"method": "PATTERN_PLUS_VERIFIER", "pattern_id": "p1"}, "content_kind": "DERIVED", "affects_supersession": False, "pack_build": {"pack_id": "p", "version": "1", "built_at": "2026-10-08T00:00:00Z"}},
    "disclosure_template": {"schema_version": "1.0.0", "template_id": "t1", "jurisdiction": "US", "court_scope": "synthetic", "text": "x", "authority": None, "editable": True, "auto_assert": False},
    "akpkg_v2_1": {"schema_version": "2.1.0", "metadata_version": "2.0.0", "pack_id": "pack.custom", "pack_kind": "custom", "version": "1", "display_name": "x", "content_sha256": H64, "authoritative_content_sha256": H64, "files": [{"path": "metadata/legal_sources.json", "sha256": H64, "bytes": 1, "role": "LEGAL_SOURCES"}], "headnotes_stripped": True, "trust_level": "CUSTOM", "controlled_beta_eligible": False, "sources_file": "metadata/legal_sources.json", "chunks_file": "metadata/legal_chunks.json", "currency_date": "2026-10-01", "rule_tables_files": []},
    "licence_payload_v2": {"schema_version": "2.0.0", "licence_id": "l1", "product": "CaseForge", "edition": "JUDICIARY", "issued_to_kind": "INSTITUTION", "issued_at_unix_ms": 0, "expires_at_unix_ms": None, "pack_subscription_until_unix_ms": None, "fingerprint_components": {"hostname": H64, "username": H64, "machine_guid": H64, "disk_root": H64}, "features": ["digest", "chambers"], "air_gap_forced": True, "telemetry_allowed": False, "seat_transfers_per_year": None, "signer_key_id": "k", "signature_alg": "Ed25519", "signature_hex": SIG},
    "audit_report": {"schema_version": "1.0.0", "audit_id": "a1", "pipeline_id": "caseforge.audit.v1", "generated_at": "2026-10-08T00:00:00Z", "subject": {"subject_kind": "OWN_DRAFT", "sha256": H64, "filename": "d.docx", "format": "docx"}, "pack_snapshot": [], "items": [{
        "item_id": "i1", "kind": "CITATION", "citation_kind": "CASE", "draft_char_start": 0, "draft_char_end": 4, "draft_text": "x v y", "result": "NOT_IN_CORPUS", "resolved": None, "pinpoint": {"claimed": None, "status": "NO_PINPOINT"}, "notes": [], "misquote": None, "proposition_ids": []}],
        "counts": {k: 0 for k in ["VERIFIED", "VERIFIED_WITH_NOTE", "MISQUOTED", "NOT_IN_CORPUS", "SUPERSEDED", "NOT_APPLICABLE", "CUSTOM_SOURCE"]}},
    "external_filing": {"schema_version": "1.0.0", "doc_id": "d1", "type_tag": "EXTERNAL_FILING", "filing_party_id": "p2", "filing_kind": "BRIEF", "trust": "UNTRUSTED_ADVERSE", "assertions": [], "scan": {"audit_report_id": None, "record_conflicts": [], "unsupported_assertion_ids": [], "new_arguments": []}},
    "witness_matrix": {"schema_version": "1.0.0", "matrix_id": "m1", "case_id": "c1", "built_at": "2026-10-08T00:00:00Z", "graph_version": "2.0.0", "statement_kind_vocabulary": {"pack_id": None, "table_id": None}, "witnesses": [{"witness_id": "w1", "display_name": "W", "alias_node_ids": [], "resolution": "PROPOSED", "statements": [{"statement_id": "s1", "statement_kind": "OTHER", "recorded_on": None, "doc_id": "d1", "locator_kind": "PAGE_SPAN", "sequence_index": 0}], "topics": [{"topic_id": "t1", "topic_kind": "WHEN", "label": "time"}], "cells": [{"topic_id": "t1", "statement_id": "s1", "assertions": [{"assertion_id": "a1", "text": "at noon", "locator": LOC}]}]}]},
    "impeachment_kit": {"schema_version": "1.0.0", "kit_id": "k1", "case_id": "c1", "witness_id": "w1", "matrix_id": "m1", "generated_at": "2026-10-08T00:00:00Z", "export_footer_id": "caseforge.export_footer.v1", "findings": [{
        "finding_id": "f1", "finding_kind": "CONTRADICTION", "detection": "DETERMINISTIC", "dimension": "TIME", "topic_id": "t1",
        "earlier": {"statement_id": "s1", "quote": "at noon", "locator": LOC}, "later": {"statement_id": "s2", "quote": "at night", "locator": LOC},
        "proposition_ids": [], "counsel_judgement_required": False, "confrontation_order": 1, "status": "PROPOSED"}]},
    "statute_mapping": {"schema_version": "1.0.0", "reference_id": "r1", "context": "DRAFT", "ref_text": "s. 1", "code": "CODE-A", "provision": "1", "equivalence": "RENUMBERED", "counterpart": {"direction": "SUCCESSOR", "code": "CODE-B", "provision": "2"}, "rule_ids": ["r"], "governing": {"status": "NEEDS_COUNSEL_INPUT", "governing_code": None, "basis_date_kind": None, "basis_date": None, "basis_locator": None, "transition_rule_id": None}, "side_by_side": None},
    "foundation_check": {"schema_version": "1.0.0", "check_id": "f1", "evidence_node_id": "e1", "evidence_type": "ELECTRONIC_RECORD", "evidence_type_status": "PROPOSED", "rule_id": "r1", "elements": [{"element_id": "el1", "label": "certificate", "status": "ABSENT", "locators": [], "authority": {"pack_id": "p", "source_id": "s", "chunk_id": "c"}}], "scorer_version": "caseforge.integrity.v2"},
    "deadline_record": {"schema_version": "1.0.0", "deadline_id": "dl1", "case_id": "c1", "label": "x", "rule_id": "r1", "authority": {"pack_id": "p", "source_id": "s", "chunk_id": "c"}, "trigger": {"event_node_id": "n1", "event_type": "ORDER", "date": "2026-01-01", "date_precision": "DAY", "locator": LOC, "origin": "EXTRACTED"}, "computation": {"period": {"value": 30, "unit": "DAYS"}, "exclude_trigger_day": True, "exclusions_applied": [], "calendar_rule_ids": [], "rolled_forward_from": None}, "result": {"kind": "DATE", "date": "2026-01-31", "range_from": None, "range_to": None, "uncertainty_reasons": []}, "verify_label": "VERIFY", "guardrail": "G9", "computed_at": "2026-10-08T00:00:00Z", "engine_version": "caseforge.deadlines.v1"},
    "rehearsal_timeline": {"schema_version": "1.0.0", "timeline_id": "t1", "session_id": "s1", "debrief_id": None, "slot_seconds": 300, "interruption_rate": "LOW", "ended_by": "TIME_EXPIRED", "events": [{"t_ms": 0, "kind": "SESSION_START"}], "limbs": [{"limb_node_id": "l1", "reached": False, "first_reached_ms": None, "time_spent_ms": 0, "interruptions": 0}]},
    "translation_segment": {"schema_version": "1.0.0", "segment_id": "g1", "doc_id": "d1", "page_index": 0, "source_script": "DEVANAGARI", "source_language": "hi", "source_text": "क", "source_char_start": 0, "source_char_end": 1, "ocr_confidence": {"raw": 0.9, "calibrated": None, "calibration_id": None}, "target_language": "en", "target_text": "k", "model": {"model_id": "m", "digest": "d"}, "provenance_origin": "TRANSLATED", "quotation_verifiable": False, "critical_fields": [{"field_id": "cf1", "field_kind": "DATE", "source_char_start": 0, "source_char_end": 1, "source_value": "x", "translated_value": "x", "counsel_confirmed": False, "confirmed_value": None}]},
    "bundle_manifest": {"schema_version": "1.0.0", "bundle_id": "b1", "created_at": "2026-10-08T00:00:00Z", "app_version": "4.0.0", "source_case_id": "c1", "sender": {"key_id": "k", "signing_public_key_hex": H64, "display_name": "S"}, "recipients": [{"key_id": "r", "agreement_public_key_hex": H64, "wrapped_content_key_b64": "AA=="}], "crypto": {"key_agreement": "X25519", "kdf": "HKDF-SHA256", "content_cipher": "AES-256-GCM", "signature": "Ed25519"}, "contents": [{"item_id": "i1", "item_kind": "DIGEST", "path": "items/i1.bin", "ciphertext_sha256": H64, "plaintext_bytes": 1, "redacted": True}], "redaction_log": [{"item_id": "i1", "redaction_count": 1}], "signature_hex": SIG},
    "issue_matrix": {"schema_version": "1.0.0", "matrix_id": "im1", "case_id": "c1", "built_at": "2026-10-08T00:00:00Z", "parties": [{"party_id": "p1", "display_name": "A", "role": "plaintiff"}, {"party_id": "p2", "display_name": "B", "role": "defendant"}], "issues": [{"issue_id": "is1", "text": "Whether x", "origin": "GRAPH_ISSUE", "template_rule_id": None, "judge_decision": "ACCEPTED", "merged_into": None, "positions": [{"party_id": "p1", "sentences": [{"text": "A says x", "classification": "RECORD", "locators": [LOC]}], "evidence": [LOC], "authorities": []}], "admitted_facts": [], "disputed_facts": [], "unanswered_arguments": [{"argument_node_id": "a1", "by_party_id": "p1", "unanswered_by_party_id": "p2", "locator": LOC}]}], "neutrality": {"party_scoring": False, "outcome_suggestion": False, "record_only": True}},
    "cause_list_brief": {"schema_version": "1.0.0", "list_id": "cl1", "court_label": "Synthetic Court", "list_date": "2026-10-08", "import": {"format": "CSV", "sha256": H64}, "generated_at": "2026-10-08T00:00:00Z", "entries": [
        {"serial": "1", "listed_title": "A v B", "match": {"status": "MATCHED", "case_id": "c1"}, "brief": {"stage": {"state": "NOT_FOUND_IN_RECORD", "value": None, "locator": None}, "last_order": {"state": "FOUND", "value": "Adjourned.", "locator": LOC}, "last_order_date": {"state": "FOUND", "value": "2026-09-01", "locator": LOC}, "compliance": [], "adjournments": {"count": 0, "entries": []}, "pending_applications": []}},
        {"serial": "2", "listed_title": "C v D", "match": {"status": "NO_LOCAL_RECORD", "case_id": None}, "brief": None}]},
    "judgment_scaffold": {"schema_version": "1.0.0", "scaffold_id": "js1", "case_id": "c1", "created_at": "2026-10-08T00:00:00Z", "record_only_locked": True, "issue_matrix_id": None, "export_footer_id": "caseforge.export_footer.v1",
        "sections": [section(k, "LOCKED_JUDGE_ONLY" if k in LOCKED else "PREFILL_ALLOWED", "JUDGE_AUTHORED" if k in LOCKED else "MACHINE_PREFILL") for k in KINDS]},
}

_cert_report = copy.deepcopy(VALID["audit_report"])
VALID["audit_certificate"] = {"schema_version": "1.0.0", "certificate_id": "cert1", "issued_at": "2026-10-08T00:00:00Z", "app_version": "4.0.0", "draft_sha256": H64, "audit_report_sha256": H64, "audit_report": _cert_report, "pack_snapshot": [{"pack_id": "p", "version": "1", "currency_date": "2026-10-01"}], "model_fingerprint": None, "summary": {"total_items": 1, "counts": {"NOT_IN_CORPUS": 1}}, "scope_statement": None, "disclosure": {"included": False, "template_id": None, "text": None, "user_confirmed": False}, "export_footer_id": "caseforge.export_footer.v1", "signing": {"alg": "Ed25519", "key_kind": "PER_INSTALL", "key_id": "k", "public_key_hex": H64, "canonicalization": "JSON object without signing.signature_hex, keys sorted, compact separators, UTF-8", "signature_hex": SIG}}

# Each adversarial case: (schema, dotted path, bad value, what it proves)
OMISSION = {"finding_id": "f2", "finding_kind": "OMISSION", "detection": "DETERMINISTIC", "dimension": "MATERIAL_FACT", "topic_id": "t1", "earlier": {"statement_id": "s1", "quote": None, "locator": None, "absence_note": "not mentioned"}, "later": {"statement_id": "s2", "quote": "saw a knife", "locator": LOC}, "proposition_ids": [], "counsel_judgement_required": True, "confrontation_order": 2, "status": "PROPOSED"}
EXTRA_VALID = [
    ("impeachment_kit", "findings", [OMISSION], "OMISSION with no earlier quote is valid"),
    ("audit_report", "items.0", {**VALID["audit_report"]["items"][0], "result": "MISQUOTED", "resolved": LEGAL, "misquote": {"nearest": LEGAL, "diff_ops": [{"op": "EQUAL", "text": "x"}]}}, "MISQUOTED with diff is valid"),
]
ADVERSARIAL = [
    ("rule_table", "rows.0.legal_editor_signoff", None, "unsigned row cannot claim SIGNED_OFF (AC-S4-RT.2)"),
    ("rule_table", "rows.0.authorities", [], "row without authority rejected (AC-S4-RT.1)"),
    ("rule_table", "rows.0.payload", {"entry_type": "MAPPING"}, "payload must match its table_kind"),
    ("treatment_edge", "affects_supersession", True, "treatment never changes supersession"),
    ("disclosure_template", "auto_assert", True, "disclosure never auto-asserted"),
    ("akpkg_v2_1", "rule_tables_files", ["metadata/rule_tables/x.json"], "custom pack cannot carry rule tables"),
    ("akpkg_v2_1", "schema_version", "2.0.0", "v2.1 schema does not silently accept v2.0 manifests"),
    ("licence_payload_v2", "telemetry_allowed", True, "Judiciary edition never allows telemetry"),
    ("licence_payload_v2", "air_gap_forced", False, "Judiciary edition forces air-gap"),
    ("licence_payload_v2", "edition", "ADVOCATE", "chambers feature only in Judiciary edition"),
    ("audit_report", "items.0.resolved", LEGAL, "NOT_IN_CORPUS cannot carry a resolved source"),
    ("audit_report", "items.0.result", "MISQUOTED", "MISQUOTED requires a diff"),
    ("audit_report", "items.0.result", "FAKE", "no 'fake' classification exists"),
    ("audit_certificate", "scope_statement", "This filing is verified and court-approved.", "scope statement is fixed text"),
    ("audit_certificate", "disclosure", {"included": True, "template_id": "t", "text": "x", "user_confirmed": False}, "disclosure requires user confirmation"),
    ("external_filing", "trust", "TRUSTED", "external filings are always untrusted"),
    ("impeachment_kit", "findings.0.detection", "MODEL_VERIFIED", "model findings require proposition ledger ids"),
    ("impeachment_kit", "findings.0.earlier.quote", None, "contradiction needs both quotes"),
    ("statute_mapping", "governing.governing_code", "CODE-B", "NEEDS_COUNSEL_INPUT never picks a code"),
    ("statute_mapping", "equivalence", "MODIFIED", "MODIFIED requires side-by-side texts"),
    ("foundation_check", "elements.0.status", "PRESENT", "PRESENT requires a locator"),
    ("deadline_record", "verify_label", "__DELETE__", "VERIFY label mandatory (G9)"),
    ("deadline_record", "result", {"kind": "RANGE", "date": None, "range_from": "2026-01-01", "range_to": "2026-02-01", "uncertainty_reasons": []}, "range requires reasons"),
    ("rehearsal_timeline", "limbs.0.time_spent_ms", 500, "unreached limb cannot have time spent"),
    ("translation_segment", "quotation_verifiable", True, "translation is never a verifiable quotation (G10)"),
    ("translation_segment", "critical_fields.0.counsel_confirmed", True, "confirmation requires a confirmed value"),
    ("bundle_manifest", "redaction_log.0.redacted_text", "secret", "redaction log cannot carry redacted text"),
    ("bundle_manifest", "crypto.content_cipher", "NONE", "bundle must be encrypted"),
    ("issue_matrix", "neutrality.party_scoring", True, "issue matrix never scores parties"),
    ("issue_matrix", "issues.0.judge_decision", "PROPOSED", "only judge-accepted issues carry positions"),
    ("issue_matrix", "issues.0.positions.0.sentences.0.classification", "EXTRA_RECORD", "extra-record sentences not renderable (G7)"),
    ("cause_list_brief", "entries.0.brief.last_order.locator", None, "FOUND field needs a locator"),
    ("cause_list_brief", "entries.1.brief", VALID["cause_list_brief"]["entries"][0]["brief"], "no brief without a local record"),
    ("judgment_scaffold", f"sections.{KINDS.index('ANALYSIS')}.sentences.0", {"text": "x", "origin": "MACHINE_PREFILL", "classification": "RECORD", "locators": [LOC]}, "machine text never enters ANALYSIS (G8)"),
    ("judgment_scaffold", f"sections.{KINDS.index('OPERATIVE_ORDER')}.assist_policy", "PREFILL_ALLOWED", "operative order always locked (G8)"),
    ("judgment_scaffold", "record_only_locked", False, "scaffold implies record-only lock (G7)"),
]


def validator(name: str) -> Draft202012Validator:
    return Draft202012Validator(load(C / f"{name}.schema.json"), format_checker=jsonschema.FormatChecker())


def main() -> int:
    results: dict[str, object] = {}

    # 1. contracts exist and are valid schemas
    for name in STAGE4_SCHEMAS:
        path = C / f"{name}.schema.json"
        if not path.exists():
            errors.append(f"missing contract {path.name}")
            continue
        try:
            data = load(path)
            Draft202012Validator.check_schema(data)
        except (json.JSONDecodeError, jsonschema.SchemaError) as exc:
            errors.append(f"{path.name}: {exc}")
            continue
        xc = data.get("x-caseforge", {})
        need(xc.get("batch") == "B-S4-01", f"{path.name}: x-caseforge.batch must be B-S4-01")
        need(xc.get("status") == "FROZEN_FOR_IMPLEMENTATION", f"{path.name}: status must be FROZEN_FOR_IMPLEMENTATION")
        need(bool(xc.get("implemented_by")), f"{path.name}: implemented_by missing")
    for name in STAGE4_POLICY_FILES:
        need((C / name).exists(), f"missing {name}")

    # 2. pinned frozen contracts unchanged
    for name, digest in PINNED.items():
        actual = hashlib.sha256((C / name).read_bytes()).hexdigest()
        hint = f" (expected change belongs to {REPIN_BATCH[name]}; re-pin there)" if name in REPIN_BATCH else ""
        need(actual == digest, f"{name} changed since B-S4-01 freeze{hint}")

    # 3. valid instances accepted, adversarial instances rejected
    if not errors:
        cert = VALID["audit_certificate"]
        cert["scope_statement"] = load(C / "audit_certificate.schema.json")["properties"]["scope_statement"]["const"]
        for name in STAGE4_SCHEMAS:
            errs = sorted(validator(name).iter_errors(VALID[name]), key=str)
            need(not errs, f"{name}: valid fixture rejected: {errs[0].message if errs else ''}")
        for name, path, value, proves in EXTRA_VALID:
            errs = list(validator(name).iter_errors(mutate(VALID[name], path, value)))
            need(not errs, f"{name}: expected valid ({proves}) but rejected: {errs[0].message if errs else ''}")
        caught = 0
        for name, path, value, proves in ADVERSARIAL:
            bad = mutate(VALID[name], path, value)
            if validator(name).is_valid(bad):
                errors.append(f"{name}: adversarial fixture accepted - {proves}")
            else:
                caught += 1
        results["adversarial_cases"] = {"total": len(ADVERSARIAL), "rejected": caught}

    # 4. guardrails + errors
    g1 = load(C / "guardrails.json")
    g4 = load(C / "guardrails_stage4.json")
    codes = {c["code"] for c in load(C / "errors.json")["codes"]}
    need(set(g4["rules"]) == {"G7", "G8", "G9", "G10"}, "guardrails_stage4 must define exactly G7-G10")
    need(not set(g4["rules"]) & set(g1["rules"]), "guardrails_stage4 redefines a Stage 1 rule")
    for rid, rule in g4["rules"].items():
        need(rule.get("error_code") in codes, f"{rid}: error code {rule.get('error_code')} not in errors.json")
    all_codes = [c["code"] for c in load(C / "errors.json")["codes"]]
    need(len(all_codes) == len(set(all_codes)), "duplicate codes in errors.json")
    g8 = [re.compile(p, re.I) for p in g4["input_patterns"]["G8"]]
    must_catch = ["Write the findings for this suit", "Decide the issue of limitation", "Which party should win?", "Should the court grant bail?", "recommend a sentence"]
    must_pass = ["Show where the witness mentions the car", "List admitted facts on issue 2", "Summarise the plaintiff's submissions with page references"]
    for p in must_catch:
        need(any(r.search(p) for r in g8), f"G8 pattern misses decision prompt: {p!r}")
    for p in must_pass:
        need(not any(r.search(p) for r in g8), f"G8 pattern blocks benign prompt: {p!r}")

    # 5. inventory completeness vs PRD + playbook
    inv = load(INVENTORY)
    ids = [a["id"] for a in inv["acceptance_criteria"]]
    need(len(ids) == len(set(ids)), "duplicate AC ids in stage4 inventory")
    doc_ids = set(re.findall(r"AC-S4-(?:RT-RED|RT\.\d+|\d{2}\.\d+)", PRD.read_text(encoding="utf-8") + PLAYBOOK.read_text(encoding="utf-8")))
    need(not (doc_ids - set(ids)), f"ACs in PRD/playbook missing from inventory: {sorted(doc_ids - set(ids))}")
    need(not (set(ids) - doc_ids), f"inventory ACs not in PRD/playbook: {sorted(set(ids) - doc_ids)}")
    batches = set(re.findall(r"^## (B-S4-\d{2})", PLAYBOOK.read_text(encoding="utf-8"), re.M))
    for a in inv["acceptance_criteria"]:
        need(a["batch"] in batches, f"{a['id']}: batch {a['batch']} not in playbook")

    # 6. thresholds: proposed values never leak into constants; dependent ACs BLOCKED
    th = load(C / "stage4_thresholds.json")
    th_map = {t["id"]: t for t in th["thresholds"]}
    constants_text = (C / "constants.json").read_text(encoding="utf-8")
    for tid, t in th_map.items():
        if t.get("approval") is None:
            need(tid not in constants_text, f"unapproved threshold {tid} found in constants.json")
    status_counts = {"PASS": 0, "FAIL": 0, "SKIP": 0, "BLOCKED": 0}
    ac_status = {}
    for a in inv["acceptance_criteria"]:
        for tid in a["threshold_ids"]:
            need(tid in th_map, f"{a['id']}: unknown threshold {tid}")
        reasons = [f"threshold {t} unapproved (Q-S4-01)" for t in a["threshold_ids"] if th_map.get(t, {}).get("approval") is None]
        if a["legal_editor_required"]:
            reasons.append("legal-editor sign-off absent (Q-S4-02)")
        reasons += [f"open question {q}" for q in a["questions"] if q in inv["open_questions"]]
        st = "BLOCKED" if reasons else "SKIP"
        ac_status[a["id"]] = {"status": st, "batch": a["batch"], "reason": "; ".join(reasons) or f"not implemented (wired by {a['batch']})"}
        status_counts[st] += 1
    need(status_counts["PASS"] == 0, "silent PASS in stage4 inventory")

    # 7. resource budget consistency
    rb = load(C / "stage4_resource_budget.json")
    need(rb["base_install"]["hard_cap_gib"] == 4.0, "base install hard cap must be the owner-approved 4.0 GiB")
    need({d["id"] for d in rb["owner_decisions"]} >= {"OD-S4-SIZE-01", "OD-S4-MODEL-01"}, "owner decisions missing")
    alloc = rb["proposed_allocations_gib"]
    parts = sum(v for k, v in alloc.items() if k.endswith("_max"))
    need(abs(parts - alloc["sum"]) < 1e-9, "allocation parts do not add up to sum")
    need(alloc["sum"] <= rb["base_install"]["hard_cap_gib"], "allocations exceed hard cap")

    # 8. manifest lists every Stage 4 file
    manifest_paths = {c["path"] for c in load(C / "MANIFEST.json")["contracts"]}
    for name in STAGE4_SCHEMAS:
        need(f"{name}.schema.json" in manifest_paths, f"MANIFEST missing {name}.schema.json")
    for name in STAGE4_POLICY_FILES:
        need(name in manifest_paths, f"MANIFEST missing {name}")

    implementation = "PASS" if not errors else "FAIL"
    formal = "BLOCKED" if implementation == "PASS" else "FAIL"
    report = {
        "batch": "B-S4-01",
        "generated_at": datetime.now(timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "implementation_status": implementation,
        "formal_acceptance_status": formal,
        "formal_blockers": ["Q-S4-01 thresholds awaiting owner approval", "Q-S4-02 legal editors not named", "Q-S4-12 resource allocations/RAM ceiling awaiting approval"],
        "contracts": len(STAGE4_SCHEMAS) + len(STAGE4_POLICY_FILES),
        "pinned_unchanged": sorted(PINNED),
        **results,
        "stage4_ac_counts": status_counts,
        "stage4_ac_status": ac_status,
        "errors": errors,
    }
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")

    if errors:
        print("FAIL  B-S4-01 Stage 4 contracts")
        for e in errors:
            print(f"  - {e}")
        return 1
    print(f"PASS  B-S4-01 Stage 4 contracts ({report['contracts']} contracts; "
          f"{results.get('adversarial_cases', {}).get('rejected')}/{len(ADVERSARIAL)} adversarial fixtures rejected; "
          f"{len(ids)} ACs inventoried: BLOCKED={status_counts['BLOCKED']} SKIP={status_counts['SKIP']} PASS=0)")
    print("      formal acceptance BLOCKED: owner approval pending for Q-S4-01 / Q-S4-02 / Q-S4-12")
    return 0


if __name__ == "__main__":
    sys.exit(main())
