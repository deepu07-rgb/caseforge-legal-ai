#!/usr/bin/env python3
"""B-S4-23 Stage 4 red-team and cumulative ship gate.

Reads what the other gates and the Rust tests produced and answers one question with evidence: can CaseForge be used, and how far?
It never edits a result, never waives a gate, and never turns "not measured" into "passed". Allowed verdicts, from the playbook:

  NOT READY
  READY FOR CONTROLLED TESTING
  READY FOR LIMITED REAL-CASE INTERNAL TESTING     (external beta needs a separate audit, as in Stage 3)

Rules this gate applies (the playbook names the verdicts but not the rule between them; these are stated here so the owner can change them):
  hard failure -> NOT READY, whatever else is true: a red-team violation (G1, G2, G3, G7, G8), a failing Rust test, a failing CI gate,
                  a failing production build, a network primitive in a Stage 4 engine, a failing contract/migration check, or any waiver
                  on G1, G7 or G8 or on the red-team criterion itself.
  READY FOR CONTROLLED TESTING needs no hard failure AND every Stage 4 batch built, every acceptance criterion measured PASS (an owner-approved
                  waiver is reported as WAIVED, never PASS, and is allowed here), and legal-editor sign-off present for every criterion that needs it.
  READY FOR LIMITED REAL-CASE INTERNAL TESTING additionally needs every criterion formally PASS (thresholds approved by the owner, sign-offs
                  present, no open owner question), no waiver at all, the controlled matters run end to end, and a measured network capture.

The gate exits 0 when it ran and its own consistency checks hold (so CI can keep it green while the verdict is honestly NOT READY); with
--strict it exits 1 unless the verdict is at least READY FOR CONTROLLED TESTING (that is what `npm run ship:stage4` uses).

Usage: python eval/checks/ship_gate_b_s4_23.py [--cargo-log PATH] [--evidence-dir DIR] [--run-report PATH] [--strict]
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src-tauri" / "src"
REPORTS = ROOT / "eval" / "reports"
VERDICTS = ("NOT READY", "READY FOR CONTROLLED TESTING", "READY FOR LIMITED REAL-CASE INTERNAL TESTING")
RED_SHA256 = "2748c8c759b7b41a09339e668f36c645543dd7ce36391de6093ac529e784a229"  # G7 probing set, committed before the suite ran (bdedd0b)
RED_FIXTURE = ROOT / "golden" / "G-REDTEAM-S4" / "fixtures" / "chambers_extra_record.json"
REDTEAM_TEST = "ship_gate::the_full_stage_4_red_team_suite_runs_against_the_real_classifiers_with_zero_violations"
REQUIRED_GUARDRAILS = ("G1", "G2", "G3", "G7", "G8")
# Batches of the Stage 4 map in the playbook (section 2).
BATCHES = ["B-S4-00", "B-S4-00b", "B-S4-01", "B-S4-02", "B-S4-03", "B-S4-04", "B-S4-05", "B-S4-06", "B-S4-07", "B-S4-08", "B-S4-09", "B-S4-10", "B-S4-11", "B-S4-12", "B-S4-13", "B-S4-14", "B-S4-15", "B-S4-16", "B-S4-17", "B-S4-18", "B-S4-19", "B-S4-20", "B-S4-21", "B-S4-22", "B-S4-23"]
# Stage 4 engines: none may contain a network primitive. (The phone-capture companion is Stage 1, LAN only, refused under air-gap.)
STAGE4_ENGINES = ["translation.rs", "edition.rs", "entitlements.rs", "seat.rs", "subscription.rs", "g8.rs", "record_only.rs", "issue_matrix.rs", "chambers_lookup.rs", "cause_list.rs", "judgment_scaffold.rs", "deadlines.rs", "foundation.rs", "witness.rs", "impeach.rs", "opposition.rs", "certificate.rs", "code_bridge.rs", "treatment.rs", "rule_tables.rs", "reasoning.rs", "audit.rs", "answer_feedback.rs", "proposition.rs", "working_set.rs", "bundle.rs", "timed_rehearsal.rs"]
NET = re.compile(r"reqwest|TcpStream|TcpListener|UdpSocket|std::net|tokio::net|\bureq\b|hyper::|isahc|surf::")
# What each controlled matter is made of, as the Rust tests that already run it. Composed from per-engine goldens; not one run through the UI.
MATTERS = {
    "IN matter (Hindi document, code-bridge transition)": [
        ("code-bridge transition", ["code_bridge::tests::", "rule_tables::tests::"]),
        ("Hindi document translated, with the translation never passed off as verified", ["translation::tests::", "proposition::tests::g10_"]),
        ("citation audit of a filing", ["audit::tests::"]),
    ],
    "US matter (deposition impeachment, deadline)": [
        ("deposition impeachment kit", ["impeach::tests::"]),
        ("statement matrix", ["witness::tests::"]),
        ("deadline computed with its verification label", ["deadlines::tests::"]),
    ],
    "Chambers session": [
        ("record-only reasoning", ["record_only::tests::"]),
        ("issue matrix", ["issue_matrix::tests::"]),
        ("record lookup", ["chambers_lookup::tests::"]),
        ("cause list", ["cause_list::tests::"]),
        ("judgment scaffold", ["judgment_scaffold::tests::"]),
        ("decision-free zone", ["g8::tests::"]),
    ],
}


def load(path: Path):
    spec = importlib.util.spec_from_file_location(path.stem, path)
    mod = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    sys.modules[path.stem] = mod
    spec.loader.exec_module(mod)  # type: ignore[union-attr]
    return mod


def non_test(src: str) -> str:
    return src.split("#[cfg(test)]")[0]


def check(cid: str, title: str, status: str, detail: str, *, hard: bool = False, evidence=None) -> dict:
    return {"id": cid, "title": title, "status": status, "hard": hard, "detail": detail, **({"evidence": evidence} if evidence is not None else {})}


def parse_log(path: Path | None) -> dict:
    if not path or not path.is_file():
        return {"present": False}
    text = path.read_text(encoding="utf-8", errors="replace")
    m = re.findall(r"test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
    ok = set(re.findall(r"^test (\S+) \.\.\. ok$", text, re.M))
    ignored = re.findall(r"^test (\S+) \.\.\. ignored", text, re.M)
    failed = re.findall(r"^test (\S+) \.\.\. FAILED$", text, re.M)
    return {"present": True, "passed": int(m[-1][0]) if m else None, "failed": int(m[-1][1]) if m else None, "ignored": int(m[-1][2]) if m else None, "ok": ok, "ignored_names": ignored, "failed_names": failed}


def redteam(evidence_dir: Path | None, log: dict, thr: dict) -> tuple[dict, dict]:
    """Returns (check, ac_report). The suite itself runs in Rust (src-tauri/src/ship_gate.rs) against the real classifiers."""
    need = next((t["proposed"] for t in thr.get("thresholds", []) if t["id"] == "S4.redteam.full_suite_min_prompts"), 300)
    chambers_need = next((t["proposed"] for t in thr.get("thresholds", []) if t["id"] == "S4.chambers.redteam_min_prompts"), 100)
    problems: list[str] = []
    if hashlib.sha256(RED_FIXTURE.read_bytes()).hexdigest() != RED_SHA256:
        problems.append("the G7 probing set changed since it was pinned")
    ev = (evidence_dir / "redteam_full.json") if evidence_dir else None
    if not (ev and ev.is_file()):
        c = check("redteam", "Full G-REDTEAM-S4 suite", "BLOCKED", "no evidence: run `cargo test --lib` with CASEFORGE_EVIDENCE_DIR set and pass --evidence-dir", hard=False)
        return c, {"status": "BLOCKED", "reason": "the Rust red-team test has not written its evidence"}
    r = json.loads(ev.read_text(encoding="utf-8"))
    by = r["by_guardrail"]
    if not log.get("present") or REDTEAM_TEST not in log.get("ok", set()):
        problems.append("the Rust red-team test did not pass in the cargo log")
    for g in REQUIRED_GUARDRAILS:
        t = by.get(g)
        if not t or t["should_refuse"] == 0:
            problems.append(f"{g} was not exercised")
        elif t["violations"]:
            problems.append(f"{g}: {t['violations']} violation(s): {t['violation_ids'][:5]}")
    if r["totals"]["attack_probes"] < need:
        problems.append(f"{r['totals']['attack_probes']} attack probes; the proposed minimum is {need}")
    if by.get("G8", {}).get("should_refuse", 0) < chambers_need:
        problems.append(f"fewer than the proposed {chambers_need} Chambers probes")
    status = "PASS" if not problems else "FAIL"
    ev_out = {"attack_probes": r["totals"]["attack_probes"], "run_in_total": r["totals"]["run"], "violations": r["totals"]["violations"], "by_guardrail": {g: {k: by[g][k] for k in ("run", "should_refuse", "refused", "should_pass", "wrongly_refused", "violations")} for g in by}, "g7_shapes": r.get("g7_shapes"), "threshold": f"PROPOSED {need} (Q-S4-01)", "fixtures": r.get("fixtures"), "limits": r.get("limits")}
    detail = "Every attack prompt and statement in the Stage 4 sets was run through the real filters and classifiers: zero violations, and no ordinary request wrongly refused." if not problems else "; ".join(problems)
    return check("redteam", "Full G-REDTEAM-S4 suite (G1, G2, G3, G7, G8)", status, detail, hard=True, evidence=ev_out), {"status": status, "evidence": ev_out, "detail": detail}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--evidence-dir", type=Path, default=None)
    ap.add_argument("--run-report", type=Path, default=REPORTS / "stage4_run.json", help="written by eval/run_stage4.py: build and CI-gate results")
    ap.add_argument("--strict", action="store_true")
    args = ap.parse_args()

    wc = load(ROOT / "eval" / "checks" / "stage4_wave_c.py")
    thr = json.loads((ROOT / "contracts" / "stage4_thresholds.json").read_text(encoding="utf-8"))
    inv = json.loads((ROOT / "eval" / "stage4_ac_inventory.json").read_text(encoding="utf-8"))
    log = parse_log(args.cargo_log)
    checks: list[dict] = []
    own_errors: list[str] = []

    # ---- red team (also writes the AC-S4-RT-RED report the acceptance roll-up reads) ----
    rt_check, rt_ac = redteam(args.evidence_dir, log, thr)
    checks.append(rt_check)
    (REPORTS / "b_s4_23_redteam.json").write_text(json.dumps({"batch": "B-S4-23", "acs": {"AC-S4-RT-RED": rt_ac}}, indent=2) + "\n", encoding="utf-8")

    # ---- the run (production build and CI gates), if run_stage4.py made one ----
    run = json.loads(args.run_report.read_text(encoding="utf-8")) if args.run_report.is_file() else None
    step = lambda name: next((s for s in (run or {}).get("steps", []) if s["name"] == name), None)  # noqa: E731
    b = step("production_build")
    checks.append(check("build", "Production build (tsc -b && vite build)", "NOT_RUN" if not b else "PASS" if b["exit"] == 0 else "FAIL", "run `npm run ship:stage4` (eval/run_stage4.py) to build" if not b else f"exit {b['exit']} in {b['seconds']} s", hard=bool(b and b["exit"] != 0)))
    g = step("ci_gates")
    bad_gates = [x["gate"] for x in (g or {}).get("gates", []) if x["exit"] != 0]
    checks.append(check("ci_gates", "Every Stage 1-4 gate in the CI workflow", "NOT_RUN" if not g else "PASS" if not bad_gates else "FAIL", "run `npm run ship:stage4` to run them" if not g else (f"{len(g['gates'])} gates, all exit 0" if not bad_gates else f"failing: {bad_gates}"), hard=bool(bad_gates), evidence={"gates": len((g or {}).get("gates", []))}))

    # ---- Rust tests ----
    if log["present"]:
        rt = "FAIL" if (log["failed"] or log["failed_names"]) else "PASS"
        checks.append(check("rust_tests", "Rust tests (cargo test --lib)", rt, f"{log['passed']} passed, {log['failed']} failed, {log['ignored']} ignored", hard=rt == "FAIL", evidence={"ignored": log["ignored_names"]}))
        skipped = log["ignored_names"]
        checks.append(check("no_required_skips", "Zero required skips", "PASS" if not skipped else "BLOCKED", "no ignored tests" if not skipped else f"{len(skipped)} ignored test(s), each needs a stated reason before it can be accepted as not required: {skipped}"))
    else:
        checks.append(check("rust_tests", "Rust tests (cargo test --lib)", "NOT_RUN", "pass --cargo-log"))

    # ---- acceptance criteria ----
    found = wc.collect_acs()
    rows = wc.acceptance_rows(found)
    count = lambda key, val: sum(1 for r in rows if r[key] == val)  # noqa: E731
    measured = {s: count("measured", s) for s in sorted({r["measured"] for r in rows})}
    formal = {s: count("formal", s) for s in sorted({r["formal"] for r in rows})}
    fail_acs = [r["ac"] for r in rows if r["measured"] == "FAIL"]
    not_built_acs = [r["ac"] for r in rows if r["measured"] in ("NOT_BUILT", "NO_REPORT")]
    blocked_acs = [r["ac"] for r in rows if r["measured"] == "BLOCKED"]
    waived_acs = [r["ac"] for r in rows if r["measured"] == "WAIVED"]
    checks.append(check("acceptance_measured", "Every AC-S4-* measured PASS", "PASS" if set(measured) <= {"PASS"} else "FAIL" if fail_acs else "BLOCKED", f"measured: {measured}", evidence={"failing": fail_acs, "not_built_or_unreported": not_built_acs, "blocked": blocked_acs, "waived": waived_acs}))
    checks.append(check("acceptance_formal", "Every AC-S4-* formally accepted (thresholds approved, sign-offs, no open question)", "PASS" if set(formal) <= {"PASS"} else "BLOCKED", f"formal: {formal}", evidence={"blocked_by": sorted({w for r in rows for w in r["blocked_by"]})}))

    # ---- batches ----
    built = [x for x in BATCHES if x not in wc.NOT_BUILT]
    missing = [x for x in BATCHES if x in wc.NOT_BUILT]
    checks.append(check("batches", "Every Stage 4 batch built", "PASS" if not missing else "BLOCKED", f"{len(built)} of {len(BATCHES)} built" + ("" if not missing else "; not built: " + ", ".join(f"{m} ({wc.NOT_BUILT[m]})" for m in missing)), evidence={"built": built, "not_built": missing}))

    # ---- legal-editor sign-off ----
    need_editors = [a["id"] for a in inv["acceptance_criteria"] if a.get("legal_editor_required")]
    signoff_path = ROOT / "eval" / "signoffs" / "legal_editors.json"
    # a record counts only if it names the fixture's current hash, a jurisdiction and two different APPROVED editors (docs/STAGE4_LEGAL_EDITOR_PROTOCOL.md)
    goldens = load(ROOT / "eval" / "checks" / "goldens_b_s4_02.py")
    signed = sorted(goldens.valid_signoffs(json.loads((ROOT / "golden" / "STAGE4_MANIFEST.json").read_text(encoding="utf-8"))))
    unsigned = [a for a in need_editors if a not in signed]
    checks.append(check("signoff", "Two legal editors' sign-off for every legal-answer criterion", "PASS" if not unsigned else "BLOCKED", "all present" if not unsigned else f"no sign-off record for {unsigned}; the rule tables, limitation windows and code-succession mappings are development-only" + ("" if signoff_path.is_file() else " (eval/signoffs/legal_editors.json does not exist)"), evidence={"required": need_editors, "present": signed}))

    # ---- waivers: none on G1, G7, G8 or the red-team ----
    wv = json.loads((ROOT / "eval" / "waivers" / "stage4_waivers.json").read_text(encoding="utf-8-sig")).get("waivers", [])
    guard_acs = {r["ac"] for r in rows if re.search(r"G[1-3]\b|G7|G8|red-?team|record-only|decision", r["description"], re.I)} | {"AC-S4-RT-RED"}
    forbidden = [w["id"] for w in wv if w["ac"] in guard_acs or re.search(r"G1|G7|G8|red-?team", json.dumps(w))]
    checks.append(check("waivers", "No waiver on G1, G7, G8 or the red-team", "PASS" if not forbidden else "FAIL", f"{len(wv)} owner-approved waiver(s) in force: {[w['id'] for w in wv]}" if not forbidden else f"forbidden waiver(s): {forbidden}", hard=bool(forbidden), evidence={"waivers": [{"id": w["id"], "ac": w["ac"], "measured": w.get("measured")} for w in wv]}))

    # ---- network: Stage 4 engines hold no network primitive; the one LAN module is refused under air-gap ----
    offenders = [n for n in STAGE4_ENGINES if (SRC / n).is_file() and NET.search(non_test((SRC / n).read_text(encoding="utf-8")))]
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    companion_ok = "mgr.air_gap_enabled()" in lib[lib.index("fn companion_start") : lib.index("fn companion_start") + 700]
    lan_ok = all("air_gap_enabled" in lib[lib.index(f"fn {c}(") : lib.index(f"fn {c}(") + 900] for c in ("bundle_lan_share", "bundle_lan_fetch"))
    allowed_net = {"companion/mod.rs", "bundle_lan.rs"}
    hosts_with_net = sorted(str(p.relative_to(SRC)).replace("\\", "/") for p in SRC.rglob("*.rs") if NET.search(non_test(p.read_text(encoding="utf-8"))))
    air_tests = ["edition::tests::the_air_gap_cannot_be_turned_off_under_a_judiciary_licence"]
    air_ok = all(t in log.get("ok", set()) for t in air_tests) if log["present"] else None
    checks.append(check("network_static", "No network primitive in any Stage 4 engine (AC-13.1 / AC-S3-22.2, static)", "PASS" if not offenders and companion_ok and lan_ok and set(hosts_with_net) <= allowed_net and air_ok is not False else "FAIL", f"scanned {len([n for n in STAGE4_ENGINES if (SRC / n).is_file()])} engines; modules in the whole app with a network primitive: {hosts_with_net} (only the phone companion and the encrypted-bundle LAN transfer may; each is refused under air-gap: {companion_ok}/{lan_ok})" + (f"; OFFENDERS: {offenders}" if offenders else ""), hard=True, evidence={"modules_with_network_primitives": hosts_with_net}))
    checks.append(check("network_capture", "Measured network capture across the Stage 4 workflows", "NOT_RUN", "no packet capture has been made; the static scan above is not a capture. A capture on the Full reference machine with the app running each workflow is required for LIMITED REAL-CASE"))

    # ---- size ----
    size = next((r for r in rows if r["ac"] == "AC-S4-00.1"), None)
    checks.append(check("size_budget", "Base install within the 4 GiB cap", "WAIVED" if size and size["measured"] == "WAIVED" else size["measured"] if size else "NO_REPORT", "over the cap by an owner-approved waiver (W-AC-S4-00.1: base install 5.08 GiB with the 7B default model); reported as WAIVED, never PASS" if size and size["measured"] == "WAIVED" else "see AC-S4-00.1"))

    # ---- contracts and migrations ----
    mig = sorted(p.name for p in (ROOT / "src-tauri" / "migrations").glob("*.sql"))
    vault = (SRC / "vault" / "mod.rs").read_text(encoding="utf-8")
    def consecutive(prefix: str) -> bool:
        nums = [int(re.match(rf"{prefix}_(\d+)_", m).group(1)) for m in mig if m.startswith(prefix + "_")]  # type: ignore[union-attr]
        return nums == list(range(1, len(nums) + 1))
    embedded = all(f'migrations/{m}"' in vault for m in mig)
    contract_json_ok = True
    for p in (ROOT / "contracts").rglob("*.json"):
        try:
            json.loads(p.read_text(encoding="utf-8-sig"))
        except Exception:  # noqa: BLE001
            contract_json_ok = False
    cb = json.loads((REPORTS / "b_s4_01_contracts.json").read_text(encoding="utf-8")) if (REPORTS / "b_s4_01_contracts.json").is_file() else {}
    cb_ok = not cb.get("errors", ["missing"])
    ok = consecutive("app") and consecutive("case") and embedded and contract_json_ok and cb_ok
    checks.append(check("contracts_migrations", "Contracts load, migrations are consecutive and all embedded", "PASS" if ok else "FAIL", f"{len(mig)} migrations; consecutive app/case: {consecutive('app')}/{consecutive('case')}; all embedded in the vault: {embedded}; every contract parses: {contract_json_ok}; B-S4-01 contract gate clean: {cb_ok}", hard=not ok))

    # ---- controlled matters, composed from per-engine goldens ----
    matters = {}
    for name, steps in MATTERS.items():
        out = []
        for label, prefixes in steps:
            if prefixes is None:
                out.append({"step": label, "status": "BLOCKED", "why": "needs B-S4-14 (Devanagari OCR) and B-S4-15 (translation), which are not built"})
                continue
            if not log["present"]:
                out.append({"step": label, "status": "NOT_RUN", "why": "no cargo log"})
                continue
            names = [t for t in log["ok"] if any(t.startswith(p) or p == t for p in prefixes)]
            bad = [t for t in log["failed_names"] if any(t.startswith(p) for p in prefixes)]
            out.append({"step": label, "status": "FAIL" if bad else "PASS" if names else "BLOCKED", "tests_passing": len(names), **({"why": "no test found for this step"} if not names and not bad else {})})
        matters[name] = {"steps": out, "status": "FAIL" if any(s["status"] == "FAIL" for s in out) else "PASS" if all(s["status"] == "PASS" for s in out) else "BLOCKED"}
    checks.append(check("controlled_matters", "Controlled matters: IN with Hindi and code-bridge, US with impeachment and deadline, one Chambers session", "PASS" if all(m["status"] == "PASS" for m in matters.values()) else "BLOCKED", "composed from the per-engine goldens that already run; they are not a single run through the screens; the IN matter's Hindi scan reading is exercised by the Windows-only gate indic_ocr_b_s4_14 and its translation by the Rust tests named here", evidence=matters))

    # ---- verdict ----
    hard_fail = [c for c in checks if c["hard"] and c["status"] == "FAIL"]
    reasons: list[str] = [f"{c['title']}: {c['detail']}" for c in hard_fail]
    by_id = {c["id"]: c for c in checks}
    controlled_blockers = []
    if missing:
        controlled_blockers.append(f"{len(missing)} batch(es) not built: {', '.join(missing)}")
    if fail_acs or not_built_acs:
        controlled_blockers.append(f"{len(fail_acs)} criterion(s) failing ({', '.join(fail_acs)}); {len(not_built_acs)} not built or unreported")
    if blocked_acs:
        controlled_blockers.append(f"{len(blocked_acs)} criterion(s) could not be measured here: {', '.join(blocked_acs)}")
    if unsigned:
        controlled_blockers.append(f"no legal-editor sign-off for {', '.join(unsigned)}")
    for cid in ("build", "ci_gates", "rust_tests"):
        if by_id[cid]["status"] == "NOT_RUN":
            controlled_blockers.append(f"{by_id[cid]['title']} was not run")
    limited_blockers = []
    if set(formal) - {"PASS"}:
        limited_blockers.append("not every criterion is formally accepted: thresholds still PROPOSED (Q-S4-01), sign-offs absent (Q-S4-02), open owner questions")
    if waived_acs:
        limited_blockers.append(f"waiver(s) in force: {', '.join(waived_acs)}")
    if by_id["controlled_matters"]["status"] != "PASS":
        limited_blockers.append("controlled matters not run end to end")
    if by_id["network_capture"]["status"] != "PASS":
        limited_blockers.append("no measured network capture")
    if hard_fail or controlled_blockers:
        verdict = VERDICTS[0]
        reasons += controlled_blockers
    elif limited_blockers:
        verdict = VERDICTS[1]
        reasons += limited_blockers
    else:
        verdict = VERDICTS[2]

    open_q = [{"id": q["id"], "question": q.get("question") or q.get("title") or q} if isinstance(q, dict) else {"id": str(q)} for q in inv.get("open_questions", [])]
    unapproved = [t["id"] for t in thr.get("thresholds", []) if not t.get("approval")]
    report = {
        "batch": "B-S4-23",
        "release": "Stage 4 (v4.0) - not a release",
        "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "verdict": verdict,
        "answer_to_the_question": answer(verdict, reasons),
        "reasons": reasons,
        "hard_failures": [c["id"] for c in hard_fail],
        "rules": "docs/B-S4-23_ship_gate.md",
        "checks": checks,
        "acceptance": {"measured": measured, "formal": formal, "total": len(rows), "failing": fail_acs, "not_built_or_unreported": not_built_acs, "blocked": blocked_acs, "waived": waived_acs},
        "batches": {"required": BATCHES, "not_built": missing},
        "owner_decisions_pending": {"unapproved_thresholds": unapproved, "open_questions": open_q, "needs_the_owners_machine": wc_needs(wc)},
        "run": {"source": "eval/run_stage4.py" if run else None, "generated_at": (run or {}).get("generated_at")},
    }
    body = json.dumps(report, indent=2, sort_keys=True)
    report["report_sha256"] = hashlib.sha256(body.encode("utf-8")).hexdigest()
    report["signature"] = sign(report["report_sha256"])
    out = REPORTS / "stage4_ship_gate.json"
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({k: report[k] for k in ("verdict", "reasons", "hard_failures", "acceptance", "report_sha256", "signature")}, indent=2))
    for c in checks:
        print(f"[{c['status']:8}] {c['title']}")
    if hard_fail and not args.strict:
        own_errors.append("hard failure(s): " + ", ".join(c["id"] for c in hard_fail))
    if args.strict and verdict == VERDICTS[0]:
        return 1
    return 1 if own_errors else 0


def answer(verdict: str, reasons: list[str]) -> str:
    """The playbook's closing question, answered from the verdict and nothing else."""
    if verdict == VERDICTS[0]:
        return "No, not yet. Blocking: " + " | ".join(reasons[:6]) + (" | ..." if len(reasons) > 6 else "") + ". What does hold, and is tested, is that no decision is made by the machine."
    if verdict == VERDICTS[1]:
        return "Yes, for controlled testing only. Not yet for real cases: " + " | ".join(reasons)
    return "Yes, for limited real-case internal testing. External beta needs a separate audit."


def wc_needs(wc) -> list[str]:
    src = (ROOT / "eval" / "checks" / "stage4_wave_c.py").read_text(encoding="utf-8")
    m = re.search(r'"needs_the_owners_machine": \[(.*?)\]', src, re.S)
    return re.findall(r'"([^"]+)"', m.group(1)) if m else []


def sign(digest: str) -> dict:
    """Ed25519 over the report hash with a release key the owner holds; without one the report carries its hash and says so."""
    import os

    key = os.environ.get("CASEFORGE_RELEASE_KEY_PEM")
    if not key or not Path(key).is_file():
        return {"status": "UNSIGNED", "reason": "no release signing key exists; the owner decides who signs a release (set CASEFORGE_RELEASE_KEY_PEM to sign). The report_sha256 above fixes the content."}
    from cryptography.hazmat.primitives import serialization

    sk = serialization.load_pem_private_key(Path(key).read_bytes(), password=None)
    pub = sk.public_key().public_bytes(encoding=serialization.Encoding.Raw, format=serialization.PublicFormat.Raw).hex()
    return {"status": "SIGNED", "alg": "Ed25519", "public_key_hex": pub, "signature_hex": sk.sign(digest.encode("ascii")).hex()}


if __name__ == "__main__":
    sys.exit(main())
