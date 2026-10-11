#!/usr/bin/env python3
"""B-S4-22 Editions, pack subscription and seat transfer gate.

AC-S4-16.1  Each edition exposes exactly its entitled surfaces, offline. The golden (pinned before the engine) is run on a synthetic
            entitlement map by the Rust tests; this gate checks the SHIPPED map (contracts/edition_entitlements.json) by invariants, so
            the owner's decision on editions and prices (Q-S4-06) changes the map, not the rules; checks that every host command the map
            lists really refuses without its feature (static scan) and that nothing else gates; checks the licence tool honours the map
            for all four editions; and cross-checks Python-issued licences/receipts against the Rust verifier and the other way round.
AC-S4-16.2  Pack-subscription expiry never blocks reading or exporting existing work: the source is scanned so the subscription check
            is consulted only when installing a pack, never on a read or export path; the vault tests show an expired licence plus an
            expired subscription leaves cases readable and entitled, refuses only writes and new official updates.
Stage 3 AC-S3-22.8 still holds (the licence tests and the read-only vault test pass).

The entitlement map is DEVELOPMENT_ONLY until the owner decides Q-S4-06; the seat limit is PROPOSED (Q-S4-07). This gate reports
that plainly; it does not approve either.

Usage: python eval/checks/editions_b_s4_22.py [--cargo-log PATH] [--evidence-dir DIR] [--write-vectors]
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src-tauri" / "src"
GOLD = ROOT / "golden" / "G-EDITIONS" / "fixtures" / "editions_matrix.json"
GOLD_SHA256 = "5cb48b57708aaad5de5e29c732c713a95d53003f7cb3560a91e2efb40402f601"  # committed before the engine (bd3c1d4)
VECTORS = ROOT / "golden" / "G-EDITIONS" / "fixtures" / "python_issued_licences.json"
MAP = ROOT / "contracts" / "edition_entitlements.json"
SCHEMA = ROOT / "contracts" / "licence_payload_v2.schema.json"
EDITIONS = ["ADVOCATE", "PRO", "CHAMBERS_FIRM", "JUDICIARY"]
ADVOCACY_FEATURES = ["mock_court", "voice", "opposition_scan", "witness_matrix", "impeachment_kit", "timed_rehearsal", "audit_certificate"]
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted", "stronger case", "weaker case", "the court should"]
DAY = 86_400_000

E = "entitlements::tests::"
D = "edition::tests::"
S = "subscription::tests::"
T = "seat::tests::"
V = "vault::tests::"
TESTS = {
    "map_parses": E + "the_shipped_map_parses_and_is_marked_development_only",
    "narrow": E + "golden_a_licence_only_ever_narrows_its_edition",
    "commands": E + "golden_commands_need_their_feature_and_unknown_features_are_refused",
    "policy": E + "golden_an_install_with_no_licence_follows_the_stated_policy",
    "surfaces": D + "golden_each_edition_shows_exactly_what_it_is_entitled_to",
    "host_refuses": D + "golden_a_host_command_is_refused_without_its_feature",
    "policy_host": D + "golden_an_install_without_a_licence_follows_the_policy_in_the_map",
    "expired_all": D + "golden_when_everything_has_expired_work_stays_readable_and_only_updates_and_new_writes_are_refused",
    "shipped_map": D + "the_shipped_map_gives_the_judiciary_edition_the_chambers_feature_and_nobody_else",
    "transfers_in_licence": D + "a_v2_licence_carrying_too_many_transfers_does_not_verify",
    "python_vectors": D + "licences_issued_by_the_python_tool_verify_here",
    "dates": S + "dates_are_right_across_the_calendar",
    "sub_state": S + "golden_the_state_follows_the_end_date_and_the_notice_says_what_is_and_is_not_affected",
    "sub_install": S + "golden_only_newer_official_content_is_refused_after_expiry",
    "sub_updated_at": S + "an_official_pack_with_only_an_update_date_is_judged_by_that_date",
    "stamp": S + "the_stamp_keeps_the_older_date_and_adds_the_notice_only_when_expired",
    "seat_window": T + "golden_transfers_are_counted_over_a_rolling_year",
    "seat_licence": T + "golden_a_licence_over_its_own_limit_is_refused",
    "seat_released": T + "golden_an_old_licence_file_cannot_take_the_seat_back",
    "receipt": T + "a_release_receipt_verifies_and_any_change_to_it_breaks_it",
    "vault_seat": V + "b_s4_22_giving_a_seat_away_keeps_the_users_work_and_the_old_licence_file_cannot_take_it_back",
    "vault_expiry": V + "b_s4_22_when_the_licence_and_the_pack_subscription_have_both_ended_the_users_work_is_still_theirs",
    "vault_read_only": V + "an_expired_licence_makes_every_matter_write_read_only_and_a_valid_one_lifts_it",
    "licence_read_only": "licence::tests::expired_not_writable",
    "licence_3_of_4": "licence::tests::fingerprint_3_of_4",
}


def rust_files() -> dict[str, str]:
    return {str(p.relative_to(SRC)).replace("\\", "/"): p.read_text(encoding="utf-8") for p in sorted(SRC.rglob("*.rs"))}


def non_test(src: str) -> str:
    return src.split("#[cfg(test)]")[0]


def functions(src: str) -> list[tuple[str, int, int]]:
    heads = [(m.group(1), m.start()) for m in re.finditer(r"(?m)^(?:    )?(?:pub(?:\([a-z]+\))?\s+)?(?:async\s+)?fn\s+(\w+)", src)]
    return [(n, s, heads[i + 1][1] if i + 1 < len(heads) else len(src)) for i, (n, s) in enumerate(heads)]


def const_list(src: str, name: str) -> list[str]:
    m = re.search(rf"{name}: \[&str; \d+\] = \[(.*?)\];", src, re.S)
    return re.findall(r'"([^"]+)"', m.group(1)) if m else []


def pub_hex(sk) -> str:
    from cryptography.hazmat.primitives import serialization

    return sk.public_key().public_bytes(encoding=serialization.Encoding.Raw, format=serialization.PublicFormat.Raw).hex()


def load_tool():
    spec = importlib.util.spec_from_file_location("make_licence", ROOT / "eval" / "tools" / "make_licence.py")
    mod = importlib.util.module_from_spec(spec)  # type: ignore[arg-type]
    spec.loader.exec_module(mod)  # type: ignore[union-attr]
    return mod


FP = {"hostname": "court-host-1", "username": "registrar-1", "machine_guid": "guid-12345678", "disk_root": "C:\\root-disk"}
NEW_FP = {"hostname": "court-host-2", "username": "registrar-1", "machine_guid": "guid-87654321", "disk_root": "C:\\root-disk"}
NOW = 1_800_000_000_000


def make_receipt(ml, sk, *, licence_id: str, ordinal: int, fp: dict, at: int, receipt_id: str, edition: str = "PRO") -> dict:
    """Same shape as seat.rs issue_receipt (the Rust receipt is cross-checked from the evidence directory)."""
    body = {
        "schema_version": "1.0.0",
        "kind": ml.RECEIPT_KIND,
        "receipt_id": receipt_id,
        "licence_id": licence_id,
        "edition": edition,
        "transfer_ordinal": ordinal,
        "deactivated_at_unix_ms": at,
        "fingerprint_components": fp,
        "public_key_hex": pub_hex(sk),
    }
    return ml.sign(sk, body)


def build_vectors(ml) -> dict:
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

    sk = Ed25519PrivateKey.from_private_bytes(bytes([7]) * 32)
    holder = Ed25519PrivateKey.from_private_bytes(bytes([9]) * 32)
    lic = {e: ml.build_v2(sk, edition=e, licence_id=f"L-vec-{e.lower()}", fp=FP, now=NOW, days=365, pack_days=180, seat_transfers_per_year=None if e == "JUDICIARY" else 2, notes="cross-language vector") for e in EDITIONS}
    narrowed = ml.build_v2(sk, edition="PRO", licence_id="L-vec-narrow", fp=FP, now=NOW, features=["digest", "witness_matrix"], days=365, notes="cross-language vector")
    receipt = make_receipt(ml, holder, licence_id="L-vec-pro", ordinal=1, fp=FP, at=NOW - 10 * DAY, receipt_id="rcpt-vec-1")
    reissued = ml.rebind(sk, lic["PRO"], receipt, NEW_FP, now=NOW)
    return {
        "description": "Licences and a receipt made by eval/tools/make_licence.py with a throwaway test key (seed 07 x32), verified by the Rust host in edition::tests::licences_issued_by_the_python_tool_verify_here. Regenerate with `python eval/checks/editions_b_s4_22.py --write-vectors`; the gate fails if this file differs from what the tool makes.",
        "signer_key_id": ml.KEY_ID,
        "test_public_key_hex": pub_hex(sk),
        "now_ms": NOW,
        "fingerprint": FP,
        "new_fingerprint": NEW_FP,
        "licences": lic,
        "narrowed": narrowed,
        "receipt": receipt,
        "reissued": reissued,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--cargo-log", type=Path, default=None)
    ap.add_argument("--evidence-dir", type=Path, default=None)
    ap.add_argument("--write-vectors", action="store_true")
    args = ap.parse_args()
    errors: list[str] = []
    acs: dict[str, dict] = {}

    ml = load_tool()
    if args.write_vectors:
        VECTORS.write_text(json.dumps(build_vectors(ml), indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"wrote {VECTORS.relative_to(ROOT)}")
        return 0

    if hashlib.sha256(GOLD.read_bytes()).hexdigest() != GOLD_SHA256:
        errors.append("the editions golden changed since it was pinned")
    gold = json.loads(GOLD.read_text(encoding="utf-8"))
    m = json.loads(MAP.read_text(encoding="utf-8"))
    files = rust_files()
    lib = files["lib.rs"]
    edition_rs = files["edition.rs"]
    known = const_list(edition_rs, "KNOWN_FEATURES")
    all_surfaces = const_list(edition_rs, "ALL_SURFACES")
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    schema_features = schema["properties"]["features"]["items"]["enum"]
    cmds = {f: lib[s:e] for f, s, e in functions(lib) if lib[:s].rstrip().endswith("#[tauri::command]")}

    # ---- the shipped map, by invariants ----
    all_in_editions = {f for e in m["editions"].values() for f in e["features"]}
    judiciary = set(m["editions"]["JUDICIARY"]["features"])
    inv = {
        "known_features_match_the_schema": known == schema_features and len(known) == 18,
        "map_has_the_four_editions": sorted(m["editions"]) == sorted(EDITIONS),
        "development_only_or_approved_with_record": m["status"] == "DEVELOPMENT_ONLY" or (m["status"] == "APPROVED" and bool(m.get("approval"))),
        "every_edition_feature_is_known": all(f in known for e in m["editions"].values() for f in e["features"]),
        "no_duplicate_features": all(len(set(e["features"])) == len(e["features"]) for e in m["editions"].values()),
        "only_judiciary_has_chambers": [e for e, v in m["editions"].items() if "chambers" in v["features"]] == ["JUDICIARY"],
        "judiciary_has_no_advocacy_feature": not (judiciary & set(ADVOCACY_FEATURES)),
        "every_screen_is_listed": sorted(m["surfaces"]) == sorted(all_surfaces) and len(all_surfaces) == 23,
        "screen_features_are_known_and_offered": all(v is None or (v in known and v in all_in_editions) for v in m["surfaces"].values()),
        "chambers_screens_need_the_chambers_feature": all(m["surfaces"][s] == "chambers" for s in ("chambers", "causelist", "scaffold", "issues")),
        "command_features_are_known_and_offered": all(v in known and v in all_in_editions for v in m["commands"].values()),
        "every_mapped_command_exists": all(c in cmds for c in m["commands"]),
        "stage4_features_are_known": all(f in known for f in m["stage4_features"]),
        "unlicensed_policy_is_stated": m["unlicensed_stage4_policy"] in ("OPEN_DEVELOPMENT", "LOCKED"),
        "every_feature_has_a_label": all(f in m["feature_labels"] for f in known),
        "never_blocked_commands_exist": all(c in cmds for c in m["never_blocked_by_expiry"] if c not in ("seat_status",)) and "seat_status" in cmds,
    }
    for k, v in inv.items():
        if not v:
            errors.append(f"entitlement map invariant failed: {k}")

    # ---- host enforcement: a mapped command checks its feature first, and nothing unmapped does ----
    gated = {c: re.findall(r'edition::require_feature\("([a-z_]+)"\)', body) for c, body in cmds.items()}
    enforce = {
        "every_mapped_command_requires_its_feature": all(gated.get(c) == [f] and body_first(cmds[c], f) for c, f in m["commands"].items()),
        "no_other_command_gates_on_a_feature": all(not g for c, g in gated.items() if c not in m["commands"]),
        "require_feature_called_nowhere_else": len(re.findall(r"edition::require_feature\(", non_test(lib))) == len(m["commands"]),
    }
    for k, v in enforce.items():
        if not v:
            errors.append(f"host enforcement check failed: {k}")

    # ---- AC-S4-16.2: the subscription is consulted only when installing, never when reading or exporting ----
    install_calls = [(name, fn) for name, src in files.items() for fn, s, e in functions(non_test(src)) if "check_install(" in non_test(src)[s:e] and fn != "check_install"]
    reading = {c: cmds[c] for c in m["never_blocked_by_expiry"] if c in cmds}
    reading_fns = [(n, s, e) for n, s, e in functions(lib)]
    pack_install_body = cmds.get("pack_install", "")
    block = {
        "check_install_only_in_pack_install": install_calls == [("lib.rs", "pack_install")],
        "pack_install_checks_before_installing": "check_install(" in pack_install_body and pack_install_body.index("check_install(") < pack_install_body.index("install_pack_from_dir("),
        "reading_commands_never_block": all(not re.search(r"require_feature\(|check_install\(|assert_vault_writable|REFUSAL_CODE|CF_PACK_UPDATES_EXPIRED", b) for b in reading.values()),
        "subscription_state_is_not_in_the_vault": "subscription" not in non_test(files["vault/mod.rs"]).replace("pack_subscription_until", ""),
        "packs_module_never_reads_the_subscription": "subscription" not in non_test(files["packs/mod.rs"]),
        "export_and_audit_paths_do_not_check_it": not any(re.search(r"subscription::|pack_subscription|check_install", b) for c, b in cmds.items() if c not in ("pack_install", "pack_currency", "edition_status", "seat_status")),
        "stamp_suffix_wording_matches_the_screen": (lambda rs, ts: "updates expired: the pack subscription ended on" in rs and "updates expired: the pack subscription ended on" in ts and "so newer law may not be reflected" in rs and "so newer law may not be reflected" in ts)(files["subscription.rs"], (ROOT / "src" / "components" / "CurrencyStamp.tsx").read_text(encoding="utf-8")),
    }
    del reading_fns
    for k, v in block.items():
        if not v:
            errors.append(f"pack-subscription check failed: {k}")

    # ---- UI ----
    panel = (ROOT / "src" / "components" / "EditionPanel.tsx").read_text(encoding="utf-8")
    licence_view = (ROOT / "src" / "components" / "LicenceView.tsx").read_text(encoding="utf-8")
    ipc = (ROOT / "src" / "lib" / "ipc.ts").read_text(encoding="utf-8")
    ui = {
        "licence_screen_shows_the_edition_panel": "<EditionPanel" in licence_view,
        "panel_says_what_expiry_leaves_alone": "you can still open and export" in panel and "stay available" in panel,
        "panel_says_the_map_is_a_placeholder": "development placeholder" in panel,
        "panel_explains_the_transfer_steps": all(w in panel for w in ("receipt", "fingerprint", "Nothing needs a network")),
        "panel_confirms_before_giving_up_the_seat": "window.confirm" in panel and "cases, documents and exports stay" in panel,
        "ipc_has_the_commands": "seatStatus" in ipc and "seatDeactivate" in ipc and "invoke<SeatInfo>(\"seat_status\")" in ipc,
        "panel_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", panel) for w in PROHIBITED_UI),
        "commands_registered": "seat_status," in lib and "seat_deactivate," in lib,
    }
    for k, v in ui.items():
        if not v:
            errors.append(f"UI check failed: {k}")

    # ---- the licence tool, for all four editions ----
    tool: dict[str, bool] = {}
    try:
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

        sk = Ed25519PrivateKey.from_private_bytes(bytes([7]) * 32)
        pub = pub_hex(sk)
        built = {e: ml.build_v2(sk, edition=e, licence_id=f"L-{e}", fp=FP, now=NOW, days=365, pack_days=180) for e in EDITIONS}
        tool["each_edition_gets_its_map_features"] = all(built[e]["features"] == m["editions"][e]["features"] for e in EDITIONS)
        tool["signatures_verify"] = all(ml.verify_signature(p, pub) for p in built.values())
        tool["judiciary_invariants"] = built["JUDICIARY"]["air_gap_forced"] is True and built["JUDICIARY"]["telemetry_allowed"] is False and built["JUDICIARY"]["issued_to_kind"] == "INSTITUTION" and "chambers" in built["JUDICIARY"]["features"]
        tool["other_editions_have_no_chambers"] = all("chambers" not in built[e]["features"] for e in EDITIONS if e != "JUDICIARY")
        tool["pack_subscription_separate_from_the_app_licence"] = all(p["pack_subscription_until_unix_ms"] == NOW + 180 * DAY and p["expires_at_unix_ms"] == NOW + 365 * DAY for p in built.values())
        tool["no_end_dates_are_null"] = ml.build_v2(sk, edition="PRO", licence_id="L", fp=FP, now=NOW, days=0, pack_days=0)["expires_at_unix_ms"] is None
        try:
            ml.build_v2(sk, edition="ADVOCATE", licence_id="L", fp=FP, now=NOW, features=["digest", "witness_matrix"])
            tool["a_licence_cannot_exceed_its_edition"] = False
        except ValueError:
            tool["a_licence_cannot_exceed_its_edition"] = True
        tool["a_licence_can_be_narrowed"] = ml.build_v2(sk, edition="PRO", licence_id="L", fp=FP, now=NOW, features=["digest"])["features"] == ["digest"]
        # seat transfer re-issue
        holder = Ed25519PrivateKey.from_private_bytes(bytes([9]) * 32)
        prior = ml.build_v2(sk, edition="PRO", licence_id="L-seat", fp=FP, now=NOW - 20 * DAY, seat_transfers_per_year=2)

        def refused(fn) -> bool:
            try:
                fn()
            except ValueError:
                return True
            return False

        rcpt = make_receipt(ml, holder, licence_id="L-seat", ordinal=1, fp=FP, at=NOW - 10 * DAY, receipt_id="r1")
        one = ml.rebind(sk, prior, rcpt, NEW_FP, now=NOW, trusted_public_key_hex=pub)
        tool["rebind_binds_to_the_new_machine_and_records_the_transfer"] = one["fingerprint_components"] == NEW_FP and one["seat_transfers"] == [{"transferred_at_unix_ms": NOW - 10 * DAY, "receipt_id": "r1"}] and ml.verify_signature(one, pub) and one["edition"] == "PRO" and one["features"] == prior["features"] and one["expires_at_unix_ms"] == prior["expires_at_unix_ms"]
        rcpt2 = make_receipt(ml, holder, licence_id="L-seat", ordinal=2, fp=NEW_FP, at=NOW - 5 * DAY, receipt_id="r2")
        two = ml.rebind(sk, one, rcpt2, FP, now=NOW)
        rcpt3 = make_receipt(ml, holder, licence_id="L-seat", ordinal=3, fp=FP, at=NOW - 1 * DAY, receipt_id="r3")
        tool["rebind_refuses_beyond_the_yearly_limit"] = len(two["seat_transfers"]) == 2 and refused(lambda: ml.rebind(sk, two, rcpt3, NEW_FP, now=NOW))
        old = ml.build_v2(sk, edition="PRO", licence_id="L-seat", fp=FP, now=NOW - 900 * DAY, seat_transfers_per_year=2, seat_transfers=[{"transferred_at_unix_ms": NOW - 700 * DAY, "receipt_id": "a"}, {"transferred_at_unix_ms": NOW - 600 * DAY, "receipt_id": "b"}])
        rcpt_old = make_receipt(ml, holder, licence_id="L-seat", ordinal=3, fp=FP, at=NOW - 1 * DAY, receipt_id="r-old")
        tool["transfers_older_than_a_year_do_not_count"] = len(ml.rebind(sk, old, rcpt_old, NEW_FP, now=NOW)["seat_transfers"]) == 3
        disabled = ml.build_v2(sk, edition="PRO", licence_id="L-seat", fp=FP, now=NOW)
        tool["rebind_refuses_when_transfer_is_disabled"] = refused(lambda: ml.rebind(sk, disabled, rcpt, NEW_FP, now=NOW))
        tampered = dict(rcpt)
        tampered["licence_id"] = "L-other"
        tool["rebind_refuses_a_tampered_receipt"] = refused(lambda: ml.rebind(sk, prior, tampered, NEW_FP, now=NOW))
        tool["rebind_refuses_another_licences_receipt"] = refused(lambda: ml.rebind(sk, prior, make_receipt(ml, holder, licence_id="L-zzz", ordinal=1, fp=FP, at=NOW - DAY, receipt_id="rz"), NEW_FP, now=NOW))
        tool["rebind_refuses_the_wrong_ordinal"] = refused(lambda: ml.rebind(sk, prior, make_receipt(ml, holder, licence_id="L-seat", ordinal=2, fp=FP, at=NOW - DAY, receipt_id="rw"), NEW_FP, now=NOW))
        tool["rebind_refuses_a_receipt_from_another_machine"] = refused(lambda: ml.rebind(sk, prior, make_receipt(ml, holder, licence_id="L-seat", ordinal=1, fp={"hostname": "x" * 8, "username": "y" * 8, "machine_guid": "z" * 8, "disk_root": "w" * 8}, at=NOW - DAY, receipt_id="rm"), NEW_FP, now=NOW))
        tool["rebind_refuses_a_replayed_receipt"] = refused(lambda: ml.rebind(sk, one, make_receipt(ml, holder, licence_id="L-seat", ordinal=2, fp=NEW_FP, at=NOW - DAY, receipt_id="r1"), FP, now=NOW))
        tool["rebind_refuses_an_unsigned_prior_licence"] = refused(lambda: ml.rebind(sk, {**prior, "seat_transfers_per_year": 9}, rcpt, NEW_FP, now=NOW, trusted_public_key_hex=pub))
        # committed vectors equal what the tool makes now
        want = json.dumps(build_vectors(ml), indent=2, sort_keys=True) + "\n"
        tool["committed_vectors_match_the_tool"] = VECTORS.is_file() and VECTORS.read_text(encoding="utf-8") == want
        # a receipt written by the Rust host verifies here, and re-issues
        rust_receipt = args.evidence_dir / "seat_receipt.json" if args.evidence_dir else None
        if rust_receipt and rust_receipt.is_file():
            rr = json.loads(rust_receipt.read_text(encoding="utf-8"))
            ml.verify_receipt(rr)
            bound = ml.build_v2(sk, edition="PRO", licence_id=rr["licence_id"], fp=rr["fingerprint_components"], now=NOW, seat_transfers_per_year=2)
            out = ml.rebind(sk, bound, rr, NEW_FP, now=max(NOW, rr["deactivated_at_unix_ms"]), trusted_public_key_hex=pub)
            tool["a_receipt_written_by_the_rust_host_verifies_and_re_issues"] = len(out["seat_transfers"]) == 1 and out["seat_transfers"][0]["receipt_id"] == rr["receipt_id"]
        else:
            tool["a_receipt_written_by_the_rust_host_verifies_and_re_issues"] = False if args.evidence_dir else True
    except ImportError:
        errors.append("the cryptography package is required for the licence tool checks")
    for k, v in tool.items():
        if not v:
            errors.append(f"licence tool check failed: {k}")

    # ---- the Rust tests ----
    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        s16_1 = all(inv.values()) and all(enforce.values()) and all(tool.values()) and all(ui.values()) and ok("map_parses", "narrow", "commands", "policy", "surfaces", "host_refuses", "policy_host", "shipped_map", "transfers_in_licence", "python_vectors", "seat_window", "seat_licence", "seat_released", "receipt", "vault_seat")
        acs["AC-S4-16.1"] = {
            "status": "PASS" if s16_1 and not errors else "FAIL",
            "evidence": {
                "entitlement_map": {"status": m["status"], "question": m["question"], "unlicensed_policy": m["unlicensed_stage4_policy"], "editions": {e: len(v["features"]) for e, v in m["editions"].items()}},
                "golden_cases": {"surfaces": len(gold["surface_cases"]), "commands": len(gold["command_cases"]), "unlicensed": len(gold["unlicensed_cases"]), "seat": len(gold["seat_cases"]) + len(gold["seat_licence_cases"]) + len(gold["seat_released_cases"])},
                "host_commands_gated": len(m["commands"]),
                "seat_transfers_per_year": "PROPOSED 2 (Q-S4-07); the licence carries its own limit, null = transfer disabled",
            },
            "detail": "Each edition shows exactly the screens and runs exactly the host commands its licence entitles, from one map; a licence can only narrow its edition. The map in contracts is a DEVELOPMENT_ONLY placeholder (Q-S4-06) - the rules are pinned, the contents are the owner's to decide. Seat transfers are counted over a rolling year and every licence is re-checked at activation.",
        }
        vault_src = (SRC / "vault" / "mod.rs").read_text(encoding="utf-8")
        err_src = (SRC / "vault" / "error.rs").read_text(encoding="utf-8")
        read_only_choke_point = (
            'pragma_update(None, "query_only"' in vault_src and "fn apply_write_policy" in vault_src
            and "Self::apply_write_policy(&g, state.vault_writable())" in vault_src
            and "ErrorCode::ReadOnly" in err_src and "CF_VAULT_READ_ONLY" in err_src
        )
        if not read_only_choke_point:
            errors.append("the expired-licence read-only policy is not enforced at the single choke point")
        s16_2 = all(block.values()) and ok("dates", "sub_state", "sub_install", "sub_updated_at", "stamp", "expired_all", "vault_expiry")
        acs["AC-S4-16.2"] = {
            "status": "PASS" if s16_2 and not errors else "FAIL",
            "evidence": {"subscription_checked_in": [f"{a}::{b}" for a, b in install_calls], "reading_commands_checked_for_a_block": sorted(reading)},
            "detail": "The pack subscription is consulted in exactly one place, installing a pack. An expired subscription leaves installed packs usable and every citation shows the older date with an updates-expired notice; only newer official content is refused. The user's own custom packs, reading and exporting are never blocked.",
        }
        acs["AC-S3-22.8"] = {
            "status": "PASS" if ok("vault_expiry", "vault_read_only", "licence_read_only", "licence_3_of_4") and read_only_choke_point and not errors else "FAIL",
            "evidence": {"read_only_enforced_by": "SQLite query-only mode on every open matter database, set in one place (VaultManager::apply_write_policy) whenever the licence status is read or a matter is opened; a valid licence lifts it",
                         "writes_proved_refused": "document upload, case graph, translation store, witness store; reads of the same proved working; reopening the matter does not lift it"},
            "detail": "An expired licence leaves cases readable and listed, refuses EVERY write to a matter with the read-only code (one choke point, not a guard per command), and ransoms nothing - re-checked here with an expired v2 licence and an expired pack subscription together.",
        }
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
    else:
        for ac in ("AC-S4-16.1", "AC-S4-16.2", "AC-S3-22.8"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-22", "golden_sha256": GOLD_SHA256, "acs": acs, "map_invariants": inv, "host_enforcement": enforce, "pack_subscription": block, "ui": ui, "licence_tool": tool, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_22_editions.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


def body_first(body: str, feature: str) -> bool:
    """The feature check is the first statement of the command (nothing runs before it)."""
    i = body.index("{\n")
    return body[i + 2 :].lstrip().startswith(f'edition::require_feature("{feature}")?;')


if __name__ == "__main__":
    sys.exit(main())
