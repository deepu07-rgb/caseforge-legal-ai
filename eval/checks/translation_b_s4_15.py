#!/usr/bin/env python3
"""B-S4-15 aligned offline translation and guardrail G10 gate.

AC-S4-09.2  Critical-field accuracy after translation is at least the PROPOSED 98%, or the field is flagged for confirmation. The fields of a
            segment are set aside before the model sees it and written back by fixed rules, so the model cannot change a number; when it
            fails to give a marker back intact, that field and its segment are flagged. Checked three ways: (1) the engine against the
            pinned golden (segmentation, every field's English form, the rendering-rule table, the model-protocol failure cases), with a
            model that returns the markers intact, one that ignores them, and one that corrupts them; (2) the real model's run over the
            golden, read from the evidence file `eval/evidence/translation_live_b_s4_15.json` (refused if it was made with a different
            prompt, model or golden than the ones now in force); (3) every record validates against the frozen
            contracts/translation_segment.schema.json. The 98% is PROPOSED, not frozen (S4.indic.critical_field_accuracy_min), so the criterion
            is measured here and formally blocked until the owner approves it.
AC-S4-09.3  No translated sentence renders as a verified quotation (guardrail G10): the verifier never turns anything resting on translated text
            GREEN, never calls a quotation of it verified, and the label follows an inference built on it; translated pages reach the
            verifier marked translated; the screen shows the original beside the English with the label and offers no way to quote the English.
            Proven by the Rust tests (pass --cargo-log) and by static checks.

Also checked: segments keep their original and span; provenance TRANSLATED; quotation_verifiable false; the job is resumable and stoppable; the
translation model takes the primary model's slot (never beside another large model); facts read from a translation are TRANSLATED and not merged
into the matter graph; no network primitive in the translation code; commands are mapped to the Hindi ingestion feature.

Golden: golden/G-INDIC/fixtures/translation_core.json, pinned before the engine (e9c1098).

Usage: python eval/checks/translation_b_s4_15.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "sidecars" / "docpipe"))

import translate as T  # noqa: E402

GOLD = ROOT / "golden" / "G-INDIC" / "fixtures" / "translation_core.json"
GOLD_SHA256 = "08f569955d7466f451ea29c8c49933fcfbdf1a679cc399603769244203a3fe48"
EVIDENCE = ROOT / "eval" / "evidence" / "translation_live_b_s4_15.json"
SCHEMA = ROOT / "contracts" / "translation_segment.schema.json"
CONFIG = ROOT / "contracts" / "translation_config.json"
SRC = ROOT / "src-tauri" / "src"
FIXTURES = ROOT / "golden" / "G-INDIC" / "fixtures"
FIELD_MIN = 0.98  # PROPOSED (S4.indic.critical_field_accuracy_min), not frozen
LABEL = "machine translation - wording not verified"
COMMANDS = ["translation_start", "translation_status", "translation_cancel", "translation_list", "translation_events", "translation_confirm_field", "translation_unconfirm_field", "translation_graph", "translation_graph_get"]
P = "proposition::tests::"
X = "translation::tests::"
TESTS = {
    "g10_translated_fact": P + "g10_translated_fact_is_never_green",
    "g10_original_fact": P + "g10_original_language_fact_is_green",
    "g10_translated_quote": P + "g10_translated_quotation_is_not_verified",
    "g10_original_quote": P + "g10_original_quotation_is_verified",
    "g10_launder": P + "g10_translated_source_cannot_launder_a_fact_through_a_second_source",
    "g10_inference": P + "g10_inference_on_a_translated_basis_stays_labelled",
    "g10_legal": P + "g10_legal_authority_is_unaffected",
    "flow": X + "a_document_is_translated_segment_by_segment_stored_aligned_and_resumed",
    "stop": X + "stopping_keeps_progress_and_a_later_run_finishes",
    "stale": X + "changed_source_text_replaces_the_segment_and_a_vanished_segment_is_removed",
    "confirm": X + "counsel_confirms_a_field_by_an_event_and_can_withdraw_it_and_the_machine_text_never_changes",
    "verifier": X + "a_translated_page_reaches_the_verifier_marked_translated",
    "marking": X + "facts_read_from_a_translation_carry_translated_provenance_and_are_not_trusted",
    "extractor": X + "facts_found_in_a_translation_come_out_of_the_real_extractor_as_translated",
    "no_network": X + "the_module_opens_no_network_and_reads_no_file",
}


def functions(src: str) -> list[tuple[str, int, int]]:
    heads = [(m.group(1), m.start()) for m in re.finditer(r"(?m)^(?:    )?(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+(\w+)", src)]
    return [(n, s, heads[i + 1][1] if i + 1 < len(heads) else len(src)) for i, (n, s) in enumerate(heads)]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--evidence-dir", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    # ---------------------------------------------------------------- pinned inputs
    if hashlib.sha256(GOLD.read_bytes()).hexdigest() != GOLD_SHA256:
        errors.append("the translation golden changed since it was pinned")
    gold = json.loads(GOLD.read_text(encoding="utf-8"))
    cfg = json.loads(CONFIG.read_text(encoding="utf-8"))
    prompt = (ROOT / cfg["prompt"]["file"]).read_bytes()
    if hashlib.sha256(prompt).hexdigest() != cfg["prompt"]["sha256"]:
        errors.append("the translation prompt does not match the hash in contracts/translation_config.json")
    models = json.loads((ROOT / "contracts" / "supported_models.json").read_text(encoding="utf-8"))
    if cfg["model"]["digest"] != models["model"]["digest"]:
        errors.append("the translation model is not the default local model (a second large model would break the one-model rule)")
    if cfg["label"] != LABEL or cfg["guardrail"]["translated_quote_label"] != "translation - wording not verified" or T.LABEL != LABEL:
        errors.append("the translation label differs between the config, the engine and this gate")
    g10 = json.loads((ROOT / "contracts" / "guardrails_stage4.json").read_text(encoding="utf-8"))["rules"]["G10"]
    if g10["translated_quote_label"] != cfg["guardrail"]["translated_quote_label"] or g10["error_code"] != cfg["guardrail"]["error_code"]:
        errors.append("the translation config does not match guardrail G10")

    # ---------------------------------------------------------------- segmentation and fields against the golden
    seg_ok = seg_total = 0
    fields_total = fields_exact = names = names_flagged = 0
    for page, rows in gold["pages"].items():
        truth = json.loads((FIXTURES / page / "truth.json").read_text(encoding="utf-8"))
        segs = T.segment(truth["text"])
        got, want = [s["source_text"] for s in segs], [r["source"] for r in rows]
        seg_total += len(want)
        seg_ok += sum(1 for a, b in zip(got, want) if a == b) if len(got) == len(want) else 0
        if got != want:
            errors.append(f"{page}: segmentation differs from the golden")
            continue
        for s, r in zip(segs, rows):
            if s["translate"] != r["translate"]:
                errors.append(f"{page}: a segment's translate flag differs from the golden: {r['source'][:30]}")
                continue
            if not r["translate"]:
                continue
            found = T.find_fields(s["source_text"])
            expect = [(f["kind"], f["source_value"], f["english"]) for f in r["fields"]]
            if [(f["field_kind"], f["source_value"]) for f in found] != [(k, v) for k, v, _ in expect]:
                errors.append(f"{page}: fields found differ from the golden in '{s['source_text'][:30]}'")
                continue
            for f, (kind, value, english) in zip(found, expect):
                rendered, reason = T.RENDERERS[kind](value)
                if kind == "NAME":
                    names += 1
                    ok = bool(rendered) and re.fullmatch(r"[A-Za-z .]+", rendered) is not None and reason is not None
                    names_flagged += int(ok)
                    if not ok:
                        errors.append(f"name '{value}' was not given a plain suggestion that counsel must confirm: {rendered!r}")
                    continue
                fields_total += 1
                fields_exact += int(rendered == english)
                if rendered != english:
                    errors.append(f"field '{value}' rendered {rendered!r}, golden says {english!r}")
    for case in gold["rendering_rule_cases"]:
        rendered, reason = T.RENDERERS[case["kind"]](case["source"])
        if rendered != case["english"] or (reason is None) != (case["needs_confirmation_reason"] is None):
            errors.append(f"rendering rule: {case['source']!r} -> {rendered!r} / {reason!r}, golden {case['english']!r} / {case['needs_confirmation_reason']!r}")

    # ---------------------------------------------------------------- model-protocol failures
    for case in gold["protocol_cases"]:
        fields = [{"field_index": i + 1, "field_kind": "AMOUNT", "source_value": "₹ १,२५,०००"} for i in range(case["fields"])]
        out = T.fill(case["model_output"], fields)
        states = {f"F{k}": v for k, v in out["field_states"].items()}
        if states != case["expect"] or out["segment_flagged"] != case["segment_flagged"] or sorted(f"F{k}" for k in out["invented"]) != sorted(case.get("invented", [])):
            errors.append(f"protocol case {case['id']}: {states} flagged={out['segment_flagged']} invented={out['invented']}")
    # a model that rewrites the markers' neighbours cannot change a number: whatever it writes, the host's form is the only one used
    hostile = {"fields": [{"field_index": 1, "field_kind": "AMOUNT", "source_value": "₹ १,२५,०००"}]}
    filled = T.fill("He demanded Rs. 9,99,999 [F1].", hostile["fields"])
    if "₹1,25,000" not in filled["text"] or filled["field_states"][1] != "OK":
        errors.append("the host's form of a field was not used")
    # the tolerant marker forms
    for form in ("[F1]", "F1", "(F1)"):
        if T.fill(f"On {form} he left.", [{"field_index": 1, "field_kind": "DATE", "source_value": "१० मार्च २०२४"}])["field_states"][1] != "OK":
            errors.append(f"marker written as {form} was not read")
    if T.fill("In the F1 visa case.", [{"field_index": 2, "field_kind": "DATE", "source_value": "१० मार्च २०२४"}])["invented"]:
        errors.append("ordinary text 'F1' was treated as an invented marker")

    # ---------------------------------------------------------------- accuracy with three kinds of model, through the whole pipeline
    def run_all(translator, count_unflagged_wrong: bool) -> dict:
        total = exact = flagged_wrong = silent_wrong = 0
        for page, rows in gold["pages"].items():
            for r in rows:
                if not r["translate"]:
                    continue
                res = T.translate_segment(r["source"], translator)
                for f in res["fields"]:
                    kind = f["field_kind"]
                    if kind == "NAME":
                        continue
                    want = next(x["english"] for x in r["fields"] if x["source_value"] == f["source_value"])
                    total += 1
                    english, reason = res["rendered"][f["field_index"]]
                    state = res["field_states"][f["field_index"]]
                    if english == want and state == "OK":
                        exact += 1
                    elif state != "OK" or reason:
                        flagged_wrong += 1
                    else:
                        silent_wrong += 1
        return {"fields": total, "exact": exact, "flagged": flagged_wrong, "silent_wrong": silent_wrong, "accuracy_exact_or_flagged": round((exact + flagged_wrong) / max(1, total), 4)}

    rng = random.Random(15)

    def intact(masked: str) -> str:
        return masked

    def ignores(masked: str) -> str:
        return "The accused demanded Rs. 9,99,999 on 1 January 2000 under Section 999 in Case No. 1/1999."

    def corrupts(masked: str) -> str:
        out = masked
        for m in sorted(set(re.findall(r"\[F\d+\]", masked))):
            roll = rng.random()
            out = out.replace(m, "" if roll < 0.34 else (m + " " + m if roll < 0.67 else "[" + m[1:-1] + "7]"), 1)
        return out

    measured = {"marker_intact": run_all(intact, True), "marker_ignored": run_all(ignores, True), "marker_corrupted": run_all(corrupts, True)}
    for name, m in measured.items():
        if m["silent_wrong"] or m["accuracy_exact_or_flagged"] < FIELD_MIN:
            errors.append(f"{name}: {m['silent_wrong']} wrong field(s) were not flagged; accuracy {m['accuracy_exact_or_flagged']}")
    if measured["marker_intact"]["exact"] != measured["marker_intact"]["fields"]:
        errors.append("with the markers intact, not every field was rendered as the golden says")

    # ---------------------------------------------------------------- records satisfy the frozen schema
    import jsonschema

    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    validator = jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker())
    records = 0
    for page, rows in gold["pages"].items():
        truth = json.loads((FIXTURES / page / "truth.json").read_text(encoding="utf-8"))
        prepared = T.prepare(doc_id="doc-" + page, page_index=0, text=truth["text"], words=[])
        for prep in prepared:
            if not prep["translate"]:
                continue
            rec = T.finish(doc_id="doc-" + page, page_index=0, prepared=prep, model_output=prep["masked"], calibration_id=None)
            seg = rec["segment"]
            try:
                validator.validate(seg)
            except jsonschema.ValidationError as e:
                errors.append(f"{page}: a record violates the frozen schema: {e.message[:120]}")
                break
            records += 1
            if seg["provenance_origin"] != "TRANSLATED" or seg["quotation_verifiable"] is not False or any(f["counsel_confirmed"] for f in seg["critical_fields"]):
                errors.append(f"{page}: a record is not TRANSLATED / unverifiable / unconfirmed")
            if seg["source_text"] != prep["source_text"] or seg["source_char_end"] <= seg["source_char_start"]:
                errors.append(f"{page}: a record lost its original or its place in the page")
            if rec["review"]["label"] != LABEL:
                errors.append(f"{page}: a record lacks the label")
    if records < 30:
        errors.append(f"only {records} records were built")

    # ---------------------------------------------------------------- a graph read from a translation satisfies the v2.1 successor schema
    graph_status = "NOT_CHECKED"
    if args.evidence_dir:
        gp = args.evidence_dir / "translated_graph.json"
        if not gp.is_file():
            errors.append("the Rust test did not write the translated graph evidence")
        else:
            g_schema = json.loads((ROOT / "contracts" / "case_graph_v2_1.schema.json").read_text(encoding="utf-8"))
            try:
                jsonschema.Draft202012Validator(g_schema, format_checker=jsonschema.FormatChecker()).validate(json.loads(gp.read_text(encoding="utf-8")))
                graph_status = "VALID"
            except jsonschema.ValidationError as e:
                graph_status = "INVALID"
                errors.append(f"the graph read from a translation violates case_graph_v2_1: {e.message[:160]} at {list(e.absolute_path)[:4]}")
    old_schema = (ROOT / "contracts" / "case_graph.schema.json").read_bytes()
    if hashlib.sha256(old_schema).hexdigest() != "5783da2511e44b8ab9e2c8aea2d31fe7978c826402d2d870a43892ef4f09b79c":
        errors.append("the pinned case_graph 2.0.0 schema was changed; the v2.1 successor exists so that it never has to be")

    # ---------------------------------------------------------------- the real model's run (evidence file)
    live: dict = {"status": "MISSING"}
    if EVIDENCE.is_file():
        ev = json.loads(EVIDENCE.read_text(encoding="utf-8"))
        stale = []
        if ev["prompt_sha256"] != cfg["prompt"]["sha256"]:
            stale.append("prompt")
        if ev["model"]["digest"] != cfg["model"]["digest"]:
            stale.append("model")
        if ev["golden_sha256"] != GOLD_SHA256:
            stale.append("golden")
        if stale:
            errors.append(f"the live evidence is out of date ({', '.join(stale)} changed); run eval/tools/translate_golden_live.py")
        s = ev["summary"]
        live = {"status": "STALE" if stale else "CURRENT", "model": ev["model"], "made_at": ev["made_at"], **{k: s[k] for k in ("segments", "segments_flagged", "fields", "markers_carried_intact", "marker_survival", "non_name_fields", "non_name_fields_equal_to_expected", "non_name_fields_wrong_and_not_flagged", "names", "chrf_mean")}}
        if s["non_name_fields_wrong_and_not_flagged"]:
            errors.append("the live run produced a wrong field that was not flagged")
        silent = [f for r in ev["segments"] for f in r["fields"] if f["kind"] != "NAME" and f["matches_expected"] is False and f["marker"] == "OK" and not f["reason"]]
        if silent:
            errors.append(f"live evidence: {len(silent)} wrong non-name field(s) with an intact marker")
        safe = sum(1 for r in ev["segments"] for f in r["fields"] if f["kind"] != "NAME" and (f["matches_expected"] or f["marker"] != "OK" or f["reason"]))
        if safe / max(1, s["non_name_fields"]) < FIELD_MIN:
            errors.append("live evidence: critical-field accuracy (exact or flagged) is under the proposed limit")
        if any(f["kind"] == "NAME" and f["reason"] is None for r in ev["segments"] for f in r["fields"]):
            errors.append("live evidence: a name was not left for counsel to confirm")
    else:
        errors.append("no live evidence (eval/evidence/translation_live_b_s4_15.json)")

    # ---------------------------------------------------------------- static checks
    tr_rs = (SRC / "translation.rs").read_text(encoding="utf-8")
    tr_code = tr_rs.split("#[cfg(test)]")[0]
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    prop = (SRC / "proposition.rs").read_text(encoding="utf-8")
    llm = (SRC / "infer_llm" / "mod.rs").read_text(encoding="utf-8")
    py = (ROOT / "sidecars" / "docpipe" / "translate.py").read_text(encoding="utf-8")
    panel = (ROOT / "src" / "components" / "TranslationPanel.tsx").read_text(encoding="utf-8")
    app = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    ipc = (ROOT / "src" / "lib" / "ipc.ts").read_text(encoding="utf-8")
    emap = json.loads((ROOT / "contracts" / "edition_entitlements.json").read_text(encoding="utf-8"))
    cmds = {f: lib[s:e] for f, s, e in functions(lib) if lib[:s].rstrip().endswith("#[tauri::command]")}
    start_fn = cmds.get("translation_start", "")
    non_comment = lambda t: "\n".join(l for l in t.splitlines() if not l.lstrip().startswith("//"))  # noqa: E731
    verify_fn = next((prop[s:e] for f, s, e in functions(prop) if f == "verify"), "")
    llm_translate = llm[llm.index("pub fn translate"):] if "pub fn translate" in llm else ""
    static = {
        "no_network_in_the_python_module": not re.search(r"requests\.|urllib|http://|https://|socket", non_comment(py)),
        "no_network_in_the_host_module": not re.search(r"TcpStream|UdpSocket|reqwest|std::net|std::fs|ollama", non_comment(tr_code)),
        "commands_exist_registered_and_mapped": all(c in cmds and f"{c}," in lib and emap["commands"].get(c) == "indic_ingestion" for c in COMMANDS),
        "commands_check_the_feature_first": all(re.search(r"\{\s*edition::require_feature\(\"indic_ingestion\"\)\?;", cmds[c]) for c in COMMANDS if c in cmds),
        "translation_takes_the_primary_models_slot": 'choreography::acquire("primary_llm"' in start_fn and 'choreography::release("primary_llm")' in start_fn and "!acq.ok" in start_fn,
        "the_model_call_respects_the_chambers_rule": "edition::check_generation()?" in llm_translate and '"llm.translate"' in llm_translate,
        "verifier_marks_translated_sources": "pub translated: bool" in prop and "rests_on_translation" in verify_fn and '"AMBER"' in verify_fn and "quote_verified" in verify_fn,
        "verifier_never_greens_a_translation": "if rests_on_translation && status == \"GREEN\"" in verify_fn,
        "translated_pages_reach_the_verifier": "translated: true" in tr_code and "translation::support_sources" in lib,
        "no_other_source_is_marked_translated": lib.count("translated: true") == 0 and tr_code.count("translated: true") == 1,
        "segments_are_translated_provenance_and_unquotable": '"TRANSLATED"' in py and '"quotation_verifiable": False' in py and "PROVENANCE_ORIGIN" in tr_code,
        "critical_fields_start_unconfirmed": '"counsel_confirmed": False, "confirmed_value": None' in py,
        "names_are_always_left_to_counsel": "a name is always confirmed by counsel" in py,
        "the_job_is_resumable": "summary.resumed" in tr_code and 'v["segment"]["source_text"] == source_text' in tr_code,
        "the_job_can_be_stopped": "AtomicBool" in tr_code and "job_cancel" in tr_code,
        "derived_graph_is_not_merged": "case_graph_put" not in tr_code and "case_graph_put_generated" not in tr_code,
        "panel_shows_original_beside_english": 'data-testid="translation-original"' in panel and 'data-testid="translation-english"' in panel and "gridTemplateColumns: \"1fr 1fr\"" in panel,
        "panel_labels_every_segment": f'const LABEL = "{LABEL}"' in panel and "{LABEL}" in panel,
        "panel_offers_no_way_to_quote_the_english": not re.search(r"(?i)\b(copy|cite|insert|use) (as|in) (a )?(quote|quotation|draft)\b|>\s*Quote\b", panel) and "never a" in panel,
        "panel_says_confirmation_is_against_the_original": "as the original gives it" in panel and "not confirmed" in panel,
        "tab_in_the_case_workspace": '"translate"' in app and "<TranslationPanel" in app,
        "ipc_has_the_commands": all(re.search(rf'"{c}"', ipc) for c in COMMANDS),
    }
    # the panel must not use 'verified' except in the label and in the sentence that denies it
    stripped = re.sub(r"wording not verified|its wording is not verified|is not verified", "", panel, flags=re.I)
    static["panel_never_calls_the_english_verified"] = not re.search(r"verified", stripped, re.I)
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    # ---------------------------------------------------------------- the Rust tests
    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
        g10_ok = all(res[k] == "ok" for k in res if k.startswith("g10_")) and res["verifier"] == "ok" and static["verifier_never_greens_a_translation"] and static["translated_pages_reach_the_verifier"]
        flow_ok = all(res[k] == "ok" for k in ("flow", "stop", "stale", "confirm", "marking", "extractor", "no_network"))
    else:
        res = {}
        g10_ok = flow_ok = False

    ratio = measured["marker_intact"]["accuracy_exact_or_flagged"]
    if args.cargo_log and args.cargo_log.is_file():
        acs["AC-S4-09.2"] = {
            "status": "FAIL" if errors else "PASS",
            "measured": {"critical_field_accuracy_exact_or_flagged": min(m["accuracy_exact_or_flagged"] for m in measured.values()), "proposed_min": FIELD_MIN, "by_model": measured,
                         "live_model_run": live, "records_checked_against_the_frozen_schema": records, "names": {"total": names, "left_for_counsel": names_flagged}},
            "note": "S4.indic.critical_field_accuracy_min (0.98) is PROPOSED, not frozen. The guarantee is structural: the model never writes a field, and a marker it fails to return flags the field.",
            "limits": "Reference translations and the 22 labelled fields are the coding agent's, on synthetic pages; the live run is one 7B local model. Prose quality is not scored against a limit.",
        }
        acs["AC-S4-09.3"] = {
            "status": "FAIL" if errors or not g10_ok else "PASS",
            "measured": {"g10_tests": {k: v for k, v in res.items() if k.startswith("g10_") or k == "verifier"}, "rules": "never GREEN; a quotation of translated text is never verified; the label follows an inference; legal authority unaffected"},
            "note": "Enforced in the host verifier (proposition::verify); the screen shows the original beside the English, labelled, with no way to quote the English.",
        }
    else:
        for ac in ("AC-S4-09.2", "AC-S4-09.3"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-15", "implementation_status": "FAIL" if errors else "PASS", "golden_sha256": GOLD_SHA256, "acs": acs,
              "segmentation": {"segments": seg_total, "matching_the_golden": seg_ok}, "fields": {"non_name_total": fields_total, "rendered_as_golden": fields_exact, "names": names},
              "translated_graph_schema": graph_status, "model_protocol_cases": len(gold["protocol_cases"]), "rendering_rule_cases": len(gold["rendering_rule_cases"]), "live": live, "static": static, "rust_tests": res, "errors": errors}
    REPORT = ROOT / "eval" / "reports" / "b_s4_15_translation.json"
    REPORT.parent.mkdir(parents=True, exist_ok=True)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"{'PASS' if not errors else 'FAIL'} B-S4-15; AC-S4-09.2 {acs['AC-S4-09.2']['status']}, AC-S4-09.3 {acs['AC-S4-09.3']['status']}; segments {seg_ok}/{seg_total} as the golden; "
          f"non-name fields {fields_exact}/{fields_total}; live marker survival {live.get('marker_survival')}; records {records}")
    for e in errors:
        print("  ERROR:", e)
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
