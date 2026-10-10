#!/usr/bin/env python3
"""B-S4-12 Timed Oral Rehearsal gate.

AC-S4-07.1  Timer accuracy within the PROPOSED 1 s, and a graceful end at the limit: the host keeps a monotonic clock, stamps the
            expiry at the limit itself, refuses later responses without losing the unsent text, and the slot presets are the proposed
            ones. Measured on a real clock as well as a scripted one.
AC-S4-07.2  The debrief timeline lists every limb of the argument tree with reached / not reached, when, and for how long, and
            says which points were never reached; the record validates against the frozen contracts/rehearsal_timeline.schema.json.
AC-S4-07.3  Every interruption comes from a Missing Link (or weakest limb) of the matter's own argument tree, worded by the host from the
            scorer's record and citing its inputs; nothing in the module can call a model. (This is the host-built counterpart of the
            B-S3-16 rule; model-worded interruptions through the B-S3-16 prompt are NOT built - see docs/B-S4-12_timed_rehearsal.md.)
AC-3.3      The untimed Mock Court turn is untouched and a timed response adds no model call, so no added latency.

Golden: golden/G-SCRIPTS/fixtures/timed_rehearsal_core.json, pinned before the engine (949a7c6), with contracts/rehearsal_policy.json.

Usage: python eval/checks/timed_rehearsal_b_s4_12.py [--cargo-log PATH] [--evidence-dir DIR]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src-tauri" / "src"
GOLD = ROOT / "golden" / "G-SCRIPTS" / "fixtures" / "timed_rehearsal_core.json"
GOLD_SHA256 = "2579e4b7786abe06dcebdb76f2c9d4a6c05157b8ad301f1954de6d2b7cd5ec25"
POLICY = ROOT / "contracts" / "rehearsal_policy.json"
POLICY_SHA256 = "47218694b4de9cba2dedd375d29b9385f64297a16f7d04efa473ee907b4102b7"
SCHEMA = ROOT / "contracts" / "rehearsal_timeline.schema.json"
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted", "stronger case", "weaker case", "the court should"]
PREDICTION = ["will win", "likely to win", "chance of", "probability", "the court will", "you will lose", "should be allowed"]
T = "timed_rehearsal::tests::"
TESTS = {
    "targets": T + "golden_the_missing_link_order_and_the_interruption_wording",
    "reach": T + "golden_a_limb_is_reached_by_its_own_words",
    "session": T + "golden_a_five_minute_session_gives_the_expected_timeline_and_findings",
    "grounded": T + "golden_every_interruption_is_grounded_in_a_missing_link_of_the_tree",
    "timer": T + "golden_the_timer_warns_expires_exactly_at_the_limit_and_refuses_late_responses",
    "real_clock": T + "the_timer_is_accurate_on_a_real_monotonic_clock",
    "rates": T + "golden_interruption_rates_and_the_order_in_which_targets_are_used",
    "slots": T + "golden_slots_and_starts_are_validated",
    "early_end": T + "ending_early_closes_the_timeline_and_a_session_without_an_end_has_none",
    "latency": T + "a_response_costs_the_screen_no_perceptible_time",
    "schema": T + "the_timeline_validates_against_the_frozen_schema_and_nothing_here_can_reach_a_model",
}
COMMANDS = ["rehearsal_start", "rehearsal_state", "rehearsal_response", "rehearsal_end", "rehearsal_get", "rehearsal_list"]


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

    if hashlib.sha256(GOLD.read_bytes()).hexdigest() != GOLD_SHA256:
        errors.append("the timed-rehearsal golden changed since it was pinned")
    if hashlib.sha256(POLICY.read_bytes()).hexdigest() != POLICY_SHA256:
        errors.append("the rehearsal policy changed since it was pinned")
    policy = json.loads(POLICY.read_text(encoding="utf-8"))
    thr = {t["id"]: t for t in json.loads((ROOT / "contracts" / "stage4_thresholds.json").read_text(encoding="utf-8"))["thresholds"]}
    presets = [m * 60 for m in thr["S4.rehearsal.slot_presets_min"]["proposed"]]
    accuracy_s = thr["S4.rehearsal.timer_accuracy_max_s"]["proposed"]

    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    eng = (SRC / "timed_rehearsal.rs").read_text(encoding="utf-8")
    code = eng.split("#[cfg(test)]")[0]
    cmds = {f: lib[s:e] for f, s, e in functions(lib) if lib[:s].rstrip().endswith("#[tauri::command]")}
    emap = json.loads((ROOT / "contracts" / "edition_entitlements.json").read_text(encoding="utf-8"))
    panel = (ROOT / "src" / "components" / "TimedRehearsalPanel.tsx").read_text(encoding="utf-8")
    court = (ROOT / "src" / "components" / "MockCourtView.tsx").read_text(encoding="utf-8")
    send_timed = court[court.index("const sendTimed"): court.index("const sendVoice")]
    non_comment = "\n".join(l for l in code.splitlines() if not l.lstrip().startswith("//"))

    static = {
        "policy_presets_are_the_proposed_ones": policy["slot"]["presets_seconds"] == presets,
        "policy_accuracy_is_the_proposed_one": policy["timer_accuracy_max_ms"] == int(accuracy_s * 1000),
        "policy_is_marked_proposed": policy["status"].startswith("PROPOSED"),
        "no_model_or_network_in_the_engine": not re.search(r"infer_llm|chat_with_timeout|llm_chat|grounded_mock_court|reqwest|TcpStream|ollama", non_comment),
        "engine_reads_a_monotonic_clock": "Instant" in code and "SystemTime" not in code,
        "commands_exist_and_are_mapped_to_the_edition": all(c in cmds and emap["commands"].get(c) == "timed_rehearsal" for c in COMMANDS),
        "commands_check_the_feature_first": all(re.search(r"\{\s*edition::require_feature\(\"timed_rehearsal\"\)\?;", cmds[c]) for c in COMMANDS if c in cmds),
        "policy_command_registered": "rehearsal_policy," in lib and all(f"{c}," in lib for c in COMMANDS),
        "timed_response_does_not_call_the_model_turn": "mockCourtTurnForCase" not in send_timed and "voiceCourtTurnForCase" not in send_timed and "rehearsalResponse(" in send_timed,
        "untimed_grounded_turn_untouched": court.count("mockCourtTurnForCase({") == 1 and court.count("voiceCourtTurnForCase({") == 1,
        "text_and_voice_both_use_the_timer": court.count("sendTimed(") >= 2 and 'sendTimed(text, "voice")' in court and 'sendTimed(draft.trim(), "text")' in court,
        "unsent_text_kept_when_time_is_up": send_timed.index("rehearsalResponse(") < send_timed.index('setDraft("")') and "still in the box" in send_timed,
        "bench_text_comes_from_the_host_only": "r.interruption!.text" in send_timed and not re.search(r"Counsel, on|What is your answer", court + panel),
        "panel_says_where_interruptions_come_from": "not from a model" in panel and "Missing Links" in panel,
        "panel_says_how_reached_is_decided": "enough of its own words" in panel and "estimated" in panel,
        "panel_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", panel) for w in PROHIBITED_UI),
        "timeline_view_lists_never_reached": "never reached" in panel and "limb-not-reached" in panel,
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    ev_ok = None
    findings_ok = None
    ev_detail: dict = {}
    if args.evidence_dir and (args.evidence_dir / "rehearsal_timeline.json").is_file():
        try:
            import jsonschema

            tl = json.loads((args.evidence_dir / "rehearsal_timeline.json").read_text(encoding="utf-8"))
            jsonschema.Draft202012Validator(json.loads(SCHEMA.read_text(encoding="utf-8")), format_checker=jsonschema.FormatChecker()).validate(tl)
            tree = json.loads(GOLD.read_text(encoding="utf-8"))["tree"]
            ids = [l["node_id"] for l in tree["limbs"]]
            assert [l["limb_node_id"] for l in tl["limbs"]] == ids, "every limb, in tree order"
            assert all(l["reached"] == (l["first_reached_ms"] is not None) for l in tl["limbs"])
            assert tl["events"][0]["kind"] == "SESSION_START" and tl["events"][-1]["kind"] == "SESSION_END"
            assert [e["t_ms"] for e in tl["events"]] == sorted(e["t_ms"] for e in tl["events"]), "events in time order"
            total = sum(l["time_spent_ms"] for l in tl["limbs"])
            assert total <= tl["slot_seconds"] * 1000, "time per limb never exceeds the slot"
            for e in tl["events"]:
                if e["kind"] == "INTERRUPTION":
                    assert e["grounding_ref"] and (e["grounding_ref"].startswith("ml:") or e["grounding_ref"].startswith("limb:")), "every interruption names its source"
            ev_ok = True
            ev_detail = {"limbs": len(tl["limbs"]), "never_reached": [l["limb_node_id"] for l in tl["limbs"] if not l["reached"]], "interruptions": sum(1 for e in tl["events"] if e["kind"] == "INTERRUPTION"), "ended_by": tl["ended_by"]}
        except Exception as e:  # noqa: BLE001
            ev_ok = False
            errors.append(f"timeline evidence invalid: {e}")
    if args.evidence_dir and (args.evidence_dir / "rehearsal_findings.json").is_file():
        fs = json.loads((args.evidence_dir / "rehearsal_findings.json").read_text(encoding="utf-8"))
        findings_ok = bool(fs) and all(f["source_kind"] in ("TIMELINE_LIMB", "TIMELINE_EVENT") and (f["limb_node_id"] or f["event_index"] is not None) and not any(w in f["text"].lower() for w in PREDICTION) for f in fs)
        if not findings_ok:
            errors.append("a finding has no timeline source or uses prediction language")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        s1 = ok("timer", "real_clock", "slots", "early_end") and static["policy_presets_are_the_proposed_ones"] and static["policy_accuracy_is_the_proposed_one"]
        acs["AC-S4-07.1"] = {"status": "PASS" if s1 and not errors else "FAIL",
                             "evidence": {"slot_presets_seconds": policy["slot"]["presets_seconds"], "threshold": f"PROPOSED {accuracy_s} s and presets {thr['S4.rehearsal.slot_presets_min']['proposed']} min (Q-S4-01)", "warnings_before_end_s": policy["warnings_remaining_seconds"],
                                          "how": "the expiry is stamped at the limit itself on a monotonic clock; how soon the screen learns of it is how often it asks (every 0.5 s)"},
                             "detail": "Scripted and real-clock tests: warnings at their marks, the expiry exactly at the limit, later responses refused with the unsent text kept, and counsel can end early."}
        s2 = ok("session", "reach", "schema") and ev_ok is not False and static["timeline_view_lists_never_reached"]
        acs["AC-S4-07.2"] = {"status": "PASS" if s2 and ev_ok and not errors else "FAIL",
                             "evidence": {"timeline": ev_detail, "schema_validated": ev_ok, "findings_sourced": findings_ok, "reached_by": policy["reach"]["vocabulary"]},
                             "detail": "Every limb of the tree is listed with reached / not reached, when it was first reached and the time spent; limbs never reached are named; each debrief statement points at the part of the timeline it comes from. 'Reached' is detected from wording and can miss a point made in other words."}
        s3 = ok("targets", "grounded", "rates") and static["no_model_or_network_in_the_engine"] and static["bench_text_comes_from_the_host_only"]
        acs["AC-S4-07.3"] = {"status": "PASS" if s3 and not errors else "FAIL",
                             "evidence": {"interruption_source": "Missing Links of the matter's argument tree in the scorer's rank order, then its weakest limbs", "wording": "built by the host from the scorer's record (label, link, score, threshold, cited inputs)", "model_involved": False},
                             "detail": "Every interruption carries the id of the Missing Link it came from and the inputs it cites; its wording is the host's. Model-worded interruptions through the B-S3-16 prompt path are not built."}
        acs["AC-3.3"] = {"status": "PASS" if ok("latency") and static["untimed_grounded_turn_untouched"] and static["timed_response_does_not_call_the_model_turn"] and not errors else "FAIL",
                         "detail": "The untimed grounded Mock Court turn is unchanged; a timed response makes no model call, so it adds microseconds, not seconds."}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
    else:
        for ac in ("AC-S4-07.1", "AC-S4-07.2", "AC-S4-07.3", "AC-3.3"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-12", "golden_sha256": GOLD_SHA256, "policy_sha256": POLICY_SHA256, "acs": acs, "static": static, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_12_timed_rehearsal.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
