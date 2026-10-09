#!/usr/bin/env python3
"""B-S4-13 Law Currency Stamp, Treatment Graph and delta packs gate.

Evidence:
  - golden/G-TREATMENT (pinned before the engine): eighteen sentences (every treatment planted, nine traps) checked against
    BOTH the Python build-time rules (eval/tools/treatment_patterns.py) and the Rust host verifier (cargo tests), and the
    two rule sets are compared pattern by pattern so they cannot drift; fourteen stored edges with hand-authored verdicts.
  - Rust tests (pass the `cargo test --lib` log with --cargo-log): edges are re-verified against the installed text; a
    planted overruling surfaces on the overruled citation with the exact citing paragraph through the real installer,
    store and audit; malformed edges are refused at install; the delta suite (installs on its base, fails closed on a
    wrong/missing base, a tampered or unavailable file, a version that is not newer; the base survives every failure).
  - Static checks of the screens: every citation surface carries the currency stamp (discovered by scan, not listed by hand),
    and no screen text says a case is good law or valid or the like.
  - The delta build tool is run on two small packs and its result re-applied.
AC-S4-08.1/.2/.3 are measured here. The corpus-wide precision of the treatment detector on real judgments is measured by
sc_citation_graph.py against the reporters' own labels (B-S4-25), not here.

Usage: python eval/checks/treatment_b_s4_13.py [--cargo-log PATH]
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "eval" / "tools"))
import treatment_patterns as tp  # noqa: E402

FIXTURE = ROOT / "golden" / "G-TREATMENT" / "fixtures" / "treatment_core.json"
FIXTURE_SHA256 = "e99813cbb2b3026af2aec8148d03fcc842e693e6bcbc7856cb0d620d42a54d19"  # committed before the engine (45eba02)
T = "treatment::tests::"
P = "packs::test_pack::tests::"
TESTS = {
    "wording": T + "golden_wording_finds_planted_treatments_and_ignores_traps",
    "edges": T + "golden_edges_are_verified_against_the_installed_text",
    "notes": T + "golden_audit_notes_and_lookup_states",
    "currency": T + "currency_is_stamped_from_the_pack_and_missing_dates_are_stated",
    "labels": T + "labels_never_say_a_case_is_good_law",
    "end_to_end": P + "a_planted_overruling_surfaces_on_the_overruled_citation_with_the_exact_citing_paragraph",
    "install_refusals": P + "install_refuses_malformed_treatment_edges_and_a_missing_currency_date",
    "altered_edge": P + "an_edge_whose_excerpt_was_altered_after_the_build_is_not_shown",
    "delta_installs": P + "delta_pack_installs_on_its_base_and_adds_treatment_without_resending_the_judgments",
    "delta_base": P + "delta_pack_fails_closed_when_its_base_differs_or_is_missing_and_leaves_the_installed_pack_alone",
    "delta_tamper": P + "delta_pack_with_a_tampered_file_or_an_unavailable_inherited_file_is_refused_and_the_base_survives",
}
# Words a screen must not use about a case's standing.
PROHIBITED_UI = ["good law", "bad law", "valid law", "still valid", "remains valid", "no longer valid", "invalid law", "safe to rely", "safe to cite", "still good", "reliable authority", "settled law"]
# Components that render a pack's citation or a legal authority's label must carry the currency stamp; these do not render
# one (they manage packs, print the pack snapshot themselves, or build model context rather than screens).
NOT_A_SURFACE = {
    "PacksView.tsx": "manages packs (shows versions and install state, no citation)",
    "CertificatePanel.tsx": "prints the pack snapshot with each pack's currency date itself",
    "ProvenanceInspector.tsx": "generic provenance inspector; its host surface carries the stamp",
    "ArgumentBuilderView.tsx": "builds model context from the working set; no citation is rendered",
    "DemoArgumentPanel.tsx": "synthetic demonstration data, no installed pack is cited",
    "CurrencyStamp.tsx": "the stamp itself",
}
SURFACE_MARKERS = re.compile(r"pack_id|provision_label|\.citation\b|authority_string|section_label")
LLM_TOKENS = re.compile(r"infer_llm|ollama|openai|llm_chat|chat_with_timeout|reqwest|TcpStream|http://|https://", re.I)


def rust_patterns(src: str) -> dict[str, str]:
    """Regex source of each cue and guard in treatment.rs, in order."""
    out = {}
    block = src[src.index("static CUES"): src.index("const NEGATION_WINDOW")]
    for m in re.finditer(r'\("([A-Z_]+)",\s*"([a-z.0-9-]+)",\s*r\(r"((?:[^"\\]|\\.)*)"\)\)', block):
        out[f"cue:{m.group(1)}:{m.group(2)}"] = m.group(3)
    for name in ("NEGATION", "ARGUMENT"):
        m = re.search(rf'static {name}: Lazy<Regex> = Lazy::new\(\|\|\s*\{{?\s*Regex::new\(r"((?:[^"\\]|\\.)*)"\)', src, re.S)
        if m:
            out[f"guard:{name}"] = m.group(1)
    return out


def strip_ts_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return "\n".join(re.sub(r"(^|\s)//.*$", "", l) for l in text.splitlines())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    if hashlib.sha256(FIXTURE.read_bytes()).hexdigest() != FIXTURE_SHA256:
        errors.append("G-TREATMENT fixture changed since it was pinned")
    fx = json.loads(FIXTURE.read_text(encoding="utf-8"))

    # --- Python build-time rules against the golden ---
    py_bad = []
    for s in fx["sentences"]:
        got, exp = tp.detect(s["text"]), s["expect"]
        if (got is None) != (exp is None) or (exp and got != (exp["treatment"], exp["pattern_id"])):
            py_bad.append(s["id"])
        if exp and tp.verify(exp["treatment"], s["text"]) is not None:
            py_bad.append(s["id"] + ":verify")
    if py_bad:
        errors.append(f"Python treatment rules disagree with the golden on {py_bad}")
    planted = {s["expect"]["treatment"] for s in fx["sentences"] if s["expect"]}
    if planted != set(tp_t for tp_t, _, _ in tp.CUES):
        errors.append(f"the golden must plant every treatment: {sorted(planted)}")
    if sum(1 for s in fx["sentences"] if not s["expect"]) < 9:
        errors.append("the golden needs at least nine traps")

    # --- Rust and Python rules are the same rules ---
    engine_src = (ROOT / "src-tauri" / "src" / "treatment.rs").read_text(encoding="utf-8")
    engine = engine_src.split("#[cfg(test)]")[0]
    rs = rust_patterns(engine)
    py = {f"cue:{n}:{pid}": rx.pattern for n, pid, rx in tp.CUES}
    py["guard:NEGATION"] = tp.NEGATION.pattern
    py["guard:ARGUMENT"] = tp.ARGUMENT.pattern
    if rs != py:
        diff = sorted(set(rs) ^ set(py)) or [k for k in py if rs.get(k) != py[k]]
        errors.append(f"treatment.rs and treatment_patterns.py differ: {diff}")
    if f"const NEGATION_WINDOW: usize = {tp.NEGATION_WINDOW};" not in engine:
        errors.append("the negation window differs between treatment.rs and treatment_patterns.py")

    # --- the delta build tool ---
    delta_ok = None
    try:
        with tempfile.TemporaryDirectory() as t:
            t = Path(t)

            def make(d: Path, version: str, files: dict[str, str], currency: str) -> None:
                entries = []
                for rel, text in files.items():
                    (d / rel).parent.mkdir(parents=True, exist_ok=True)
                    (d / rel).write_text(text, encoding="utf-8", newline="\n")
                    entries.append({"path": rel, "sha256": hashlib.sha256(text.encode()).hexdigest(), "bytes": len(text.encode()), "role": "LEGAL_SOURCES"})
                h = hashlib.sha256(("\n".join(sorted(f"{e['path']}:{e['sha256']}" for e in entries)) + "\n").encode()).hexdigest()
                (d / "manifest.json").write_text(json.dumps({"pack_id": "x.pack", "pack_kind": "custom", "version": version, "files": entries, "content_sha256": h, "currency_date": currency}), encoding="utf-8")

            make(t / "base", "1.0.0", {"content/a.txt": "alpha\n", "metadata/legal_sources.json": "[]\n"}, "2026-01-31")
            make(t / "new", "1.1.0", {"content/a.txt": "alpha\n", "metadata/legal_sources.json": "[]\n", "metadata/treatment_edges.json": "[]\n"}, "2026-02-28")
            tool = ROOT / "eval" / "tools" / "build_delta_pack.py"
            built = subprocess.run([sys.executable, "-I", str(tool), "build", "--base", str(t / "base"), "--new", str(t / "new"), "--out", str(t / "delta")], capture_output=True, text=True)
            checked = subprocess.run([sys.executable, "-I", str(tool), "check", "--base", str(t / "base"), "--delta", str(t / "delta")], capture_output=True, text=True)
            sent = sorted(p.relative_to(t / "delta").as_posix() for p in (t / "delta").rglob("*") if p.is_file())
            wrong = subprocess.run([sys.executable, "-I", str(tool), "check", "--base", str(t / "new"), "--delta", str(t / "delta")], capture_output=True, text=True)
            older = subprocess.run([sys.executable, "-I", str(tool), "build", "--base", str(t / "new"), "--new", str(t / "base"), "--out", str(t / "bad")], capture_output=True, text=True)
            delta_ok = built.returncode == 0 and checked.returncode == 0 and sent == ["manifest.json", "metadata/treatment_edges.json"] and wrong.returncode != 0 and older.returncode != 0
            if not delta_ok:
                errors.append(f"delta build tool: build={built.returncode} check={checked.returncode} sent={sent} wrong_base_refused={wrong.returncode != 0} older_refused={older.returncode != 0}")
    except Exception as e:  # noqa: BLE001
        delta_ok = False
        errors.append(f"delta build tool could not be exercised: {e}")

    # --- screens ---
    comps = ROOT / "src" / "components"
    surfaces, uncovered = [], []
    for f in sorted(comps.glob("*.tsx")):
        text = f.read_text(encoding="utf-8")
        if f.name in NOT_A_SURFACE or not SURFACE_MARKERS.search(strip_ts_comments(text)):
            continue
        uses = "<CurrencyStamp" in text or "<PackCurrencyList" in text
        (surfaces if uses else uncovered).append(f.name)
    if uncovered:
        errors.append(f"citation surfaces without a currency stamp: {uncovered}")
    for must in ("AuditPanel.tsx", "MockCourtView.tsx", "RetrievalView.tsx", "WorkingLibraryView.tsx", "IssueMatrixView.tsx", "ReasoningPanel.tsx", "FoundationPanel.tsx", "DeadlinesView.tsx", "CodeBridgePanel.tsx"):
        text = (comps / must).read_text(encoding="utf-8")
        if not ("<CurrencyStamp" in text or "<PackCurrencyList" in text):
            errors.append(f"{must} should carry the currency stamp")
    ui_text = {f.name: strip_ts_comments(f.read_text(encoding="utf-8")) for f in sorted((ROOT / "src").rglob("*.ts*"))}
    banned = sorted({(name, w) for name, text in ui_text.items() for w in PROHIBITED_UI if re.search(rf"(?i)['\"`>][^'\"`<\n]*\b{re.escape(w)}\b", text)})
    if banned:
        errors.append(f"screen text says something about a case's standing: {banned}")
    stamp = (comps / "CurrencyStamp.tsx").read_text(encoding="utf-8")
    ui_checks = {
        "none_found_wording": "No treatment found in installed corpus" in stamp,
        "no_data_is_not_none_found": "No installed pack carries treatment data" in stamp,
        "currency_missing_is_stated": "does not state a currency date" in stamp,
        "excerpt_quoted_exactly": "quoted exactly" in stamp,
    }
    for k, v in ui_checks.items():
        if not v:
            errors.append(f"UI check failed: {k}")
    lib = (ROOT / "src-tauri" / "src" / "lib.rs").read_text(encoding="utf-8")
    pk = (ROOT / "src-tauri" / "src" / "packs" / "mod.rs").read_text(encoding="utf-8")
    static = {
        "engine_has_no_model_or_network": not LLM_TOKENS.search(engine),
        "edges_never_change_supersession": "affects_supersession" in engine and "superseded_status =" not in engine and "affects_supersession: true" not in engine,
        "commands_registered": all(f"fn {n}" in lib for n in ("pack_currency", "treatment_lookup")),
        "audit_decorated_everywhere": lib.count("treatment::decorate_report") >= 3,
        "install_checks_treatment_edges": "treatment::precheck_pack" in pk,
        "delta_fails_closed": all(c in pk for c in ("CF_PACK_DELTA_BASE_MISSING", "CF_PACK_DELTA_BASE_MISMATCH", "CF_PACK_DELTA_NOT_NEWER", "CF_PACK_DELTA_FILE_MISMATCH")),
        "delta_uses_the_ordinary_install": "install_full(paths, &full)" in pk,
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        b309 = re.findall(r"test (packs::tests::b_s3_0[89]\S*|packs::tests::\S*install\S*) \.\.\. (\w+)", log)
        acs["AC-S4-08.1"] = {"status": "PASS" if ok("wording", "edges", "notes", "end_to_end", "altered_edge") and not py_bad and rs == py else "FAIL",
                             "evidence": {"golden_sentences": len(fx["sentences"]), "edges_checked": len(fx["scenario"]["edges"]), "end_to_end": res["end_to_end"], "python_and_rust_rules_identical": rs == py},
                             "detail": "A planted overruling surfaces on the overruled citation with the exact citing paragraph (through the real installer, store and audit); edges that are negated, argued, altered, self-referential or point at passages that are not installed are not shown."}
        acs["AC-S4-08.2"] = {"status": "PASS" if ok("labels", "notes") and not banned and all(ui_checks.values()) else "FAIL",
                             "evidence": {"prohibited_phrases_found": [list(b) for b in banned], "files_scanned": len(ui_text)},
                             "detail": "No screen says a case is good law, valid or the like; absence of treatment reads 'no treatment found in installed corpus' and says it speaks only for the library."}
        acs["AC-S4-08.3"] = {"status": "PASS" if ok("currency", "end_to_end") and not uncovered else "FAIL",
                             "evidence": {"citation_surfaces_with_stamp": surfaces, "uncovered": uncovered, "not_surfaces": sorted(NOT_A_SURFACE)},
                             "detail": "Every component that renders a pack citation carries 'verified against <pack> as of <currency date>', or says the pack states no date; the audit's pack snapshot carries the date."}
        acs["delta-packs"] = {"status": "PASS" if ok("delta_installs", "delta_base", "delta_tamper", "install_refusals") and delta_ok and static["delta_fails_closed"] else "FAIL",
                              "evidence": {"delta_tests": {k: res[k] for k in ("delta_installs", "delta_base", "delta_tamper")}, "build_tool": delta_ok, "existing_install_tests": {n: r for n, r in b309}},
                              "detail": "Build (tool) -> install (host, fails closed on a missing or different base, a version that is not newer, a tampered or unavailable file) -> a failed delta leaves the installed base untouched. Distribution channel is a file the user installs (undecided channel: QUESTION)."}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
        if any(r != "ok" for _, r in b309):
            errors.append(f"existing pack install tests not passing: {b309}")
    else:
        for ac in ("AC-S4-08.1", "AC-S4-08.2", "AC-S4-08.3", "delta-packs"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-13", "fixture_sha256": FIXTURE_SHA256, "acs": acs, "static": static, "ui": ui_checks, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_13_treatment.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
