#!/usr/bin/env python3
"""B-S4-16 Encrypted Bundle Exchange gate.

AC-S4-10.1  A bundle is undecryptable without the recipient's key, and any change to it is detected: tests with the right key, a
            stranger's key, the sender's own key and an impostor holding a recipient's key id; every single byte of a small bundle
            flipped, every truncation, and swapped or grafted items. Published test vectors check HKDF, HMAC and X25519. The manifest
            written by the host validates against the frozen contracts/bundle_manifest.schema.json.
AC-S4-10.2  Redacted text is absent from every byte of the bundle: redaction runs on the plaintext before encryption, an independent check
            refuses the export if any term survives, and the tests look for each term, in many shapes, in the file and in everything
            decrypted from it. The redaction log holds counts only.
AC-S4-10.3  LAN only. The transfer binds and connects only to private, link-local or loopback addresses (the Phone Companion's rules), never
            resolves a name, serves one download, and is refused under air-gap. Enforced and tested in code, with the addresses used
            recorded as evidence. A packet capture on a real network has NOT been made, so this criterion is reported BLOCKED, not PASS.
AC-S4-10.4  Import never overwrites a recipient correction: nothing in the import path can write a correction (static), and across two
            real vaults the recipient's corrections, events, graph and documents are unchanged after an import in which the sender's
            correction on the same event differs; the difference is listed against the recipient's, never applied.

Golden: golden/G-BUNDLE/fixtures/bundle_core.json, pinned before the engine (52749d5).

Usage: python eval/checks/bundle_b_s4_16.py [--cargo-log PATH] [--evidence-dir DIR]
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
GOLD = ROOT / "golden" / "G-BUNDLE" / "fixtures" / "bundle_core.json"
GOLD_SHA256 = "59b8cb0a66e24f11b3b477b8fc4027c61252cdaa531304cab25be542e73ca90c"
SCHEMA = ROOT / "contracts" / "bundle_manifest.schema.json"
PROHIBITED_UI = ["fake", "fabricated", "court-approved", "guaranteed", "good law", "valid law", "predicts", "decides", "inadmissible", "will be admitted", "stronger case", "weaker case", "the court should"]
COMMANDS = ["bundle_identity", "bundle_peer_add", "bundle_peer_verify", "bundle_peer_list", "bundle_peer_remove", "bundle_export", "bundle_inspect", "bundle_import", "bundle_received_list", "bundle_received_get", "bundle_lan_share", "bundle_lan_stop", "bundle_lan_fetch"]
WRITES_CORRECTIONS = ["case_graph_correct_item", "correction_revert", "authority_conflict_resolve", "case_graph_put", "doc_ingest", "argument_tree_put", "page_text_layer_replace", "case_graph_put_generated", "working_set_"]
B = "bundle::tests::"
L = "bundle_lan::tests::"
V = "bundle_vault_tests::"
TESTS = {
    "primitives": B + "primitives_match_their_published_test_vectors",
    "identity": B + "an_identity_has_separate_keys_a_stable_id_and_a_card_that_checks",
    "redaction_rules": B + "golden_redaction_rules",
    "redacted": B + "golden_a_bundle_is_redacted_counted_and_the_redacted_text_is_in_no_byte_of_it",
    "refuses_leak": B + "an_export_that_would_leave_a_redacted_term_behind_is_refused_and_nothing_is_written",
    "keys": B + "golden_the_right_key_opens_it_and_no_other_key_does",
    "tamper": B + "golden_changing_any_byte_of_a_bundle_is_detected",
    "swap": B + "items_cannot_be_swapped_or_moved_between_bundles",
    "signer": B + "a_manifest_signed_by_someone_else_still_names_its_real_signer",
    "merge": B + "golden_a_received_correction_never_replaces_the_recipients_own",
    "attribution": B + "golden_attribution_names_the_sender_and_the_date",
    "schema": B + "the_manifest_satisfies_the_frozen_schema_and_the_module_holds_no_network_or_model",
    "lan_addresses": L + "golden_only_local_network_addresses_are_allowed",
    "lan_transfer": L + "a_bundle_crosses_the_local_network_once_and_only_with_the_secret_path",
    "lan_stop": L + "an_offer_ends_when_stopped_and_empty_or_oversize_offers_are_refused",
    "lan_module": L + "the_transfer_module_never_names_a_remote_service",
    "vault_flow": V + "a_junior_hands_a_senior_an_encrypted_attributable_bundle_and_nothing_of_the_seniors_changes",
    "vault_trust": V + "a_bundle_is_not_made_for_someone_whose_code_has_not_been_checked_and_an_unknown_sender_must_be_accepted_by_hand",
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

    if hashlib.sha256(GOLD.read_bytes()).hexdigest() != GOLD_SHA256:
        errors.append("the bundle golden changed since it was pinned")
    schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
    lib = (SRC / "lib.rs").read_text(encoding="utf-8")
    eng = (SRC / "bundle.rs").read_text(encoding="utf-8")
    lan = (SRC / "bundle_lan.rs").read_text(encoding="utf-8")
    vault = (SRC / "vault" / "mod.rs").read_text(encoding="utf-8")
    code = eng.split("#[cfg(test)]")[0]
    lan_code = lan.split("#[cfg(test)]")[0]
    cmds = {f: lib[s:e] for f, s, e in functions(lib) if lib[:s].rstrip().endswith("#[tauri::command]")}
    allfns = {f: lib[s:e] for f, s, e in functions(lib)}
    emap = json.loads((ROOT / "contracts" / "edition_entitlements.json").read_text(encoding="utf-8"))
    panel = (ROOT / "src" / "components" / "BundlePanel.tsx").read_text(encoding="utf-8")
    app = (ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
    ipc = (ROOT / "src" / "lib" / "ipc.ts").read_text(encoding="utf-8")
    build_fn = next(code[s:e] for f, s, e in functions(code) if f == "build")
    import_core = allfns["bundle_import_core"]
    overlay_fn = next(vault[s:e] for f, s, e in functions(vault) if f == "correction_overlay_values")
    plan_fn = next(code[s:e] for f, s, e in functions(code) if f == "plan_corrections")
    non_comment = lambda t: "\n".join(l for l in t.splitlines() if not l.lstrip().startswith("//"))  # noqa: E731

    static = {
        "algorithms_are_the_frozen_ones": schema["properties"]["crypto"]["properties"] == {"key_agreement": {"const": "X25519"}, "kdf": {"const": "HKDF-SHA256"}, "content_cipher": {"const": "AES-256-GCM"}, "signature": {"const": "Ed25519"}} and all(w in code for w in ("Aes256Gcm", "hkdf_sha256", "MontgomeryPoint", "Signature")),
        "no_network_or_model_in_the_format_module": not re.search(r"infer_llm|reqwest|TcpStream|TcpListener|UdpSocket|std::net|ollama", non_comment(code)),
        "redaction_runs_before_encryption": build_fn.index("apply_value") < build_fn.index("gcm_seal") and build_fn.index("leaks(") < build_fn.index("gcm_seal"),
        "export_refuses_when_a_term_survives": "E_REDACTION_INCOMPLETE" in build_fn and "nothing was written" in build_fn,
        "redaction_log_is_counts_only": '"item_id": id, "redaction_count": n' in build_fn.replace("json!({\"item_id\": id, \"redaction_count\": n})", '"item_id": id, "redaction_count": n'),
        "key_agreement_is_ephemeral_per_recipient": "let eph: [u8; 32] = random(rng);" in build_fn,
        "item_bound_to_bundle_item_and_kind": "fn aad(" in code and "bundle_id" in code[code.index("fn aad("): code.index("fn aad(") + 200],
        "signature_covers_the_manifest": "sender.signing.sign(&canonical_v2(&manifest)" in build_fn,
        "hex_fields_must_be_lowercase": "fn lower_hex(" in code,
        "bundle_signing_key_is_not_the_certificate_key": 'b"signing"' in code and 'b"agreement"' in code and "hkdf_sha256(seed" in code,
        "lan_uses_the_companion_rules": all(w in lan_code for w in ("is_lan_or_loopback", "assert_bind_allowed", "detect_lan_ipv4")),
        "lan_never_resolves_a_name": not re.search(r"ToSocketAddrs|lookup_host|gethostbyname", non_comment(lan_code)) and "hostport.parse()" in lan_code,
        "lan_serves_one_download": "one download, then done" in lan_code,
        "lan_peers_filtered_before_reading": lan_code.index("is_lan_or_loopback(peer.ip())") < lan_code.index("s.read(&mut head)"),
        "lan_commands_refuse_under_air_gap": all("air_gap_enabled" in cmds[c] for c in ("bundle_lan_share", "bundle_lan_fetch")),
        "lan_has_no_relay_or_cloud_word": not re.search(r"relay|cloud|upload\.|s3\.|amazonaws|googleapis", non_comment(lan_code), re.I),
        "import_cannot_write_a_correction": not any(w in import_core for w in WRITES_CORRECTIONS),
        "overlay_reader_is_read_only": "SELECT" in overlay_fn.upper() or "authority_overlays_list" in overlay_fn and not re.search(r"execute|INSERT|UPDATE|DELETE", overlay_fn),
        "plan_function_is_pure": not re.search(r"mgr|vault|execute|write|fs::", plan_fn),
        "unknown_sender_needs_hand_acceptance": "CF_BUNDLE_UNKNOWN_SENDER" in import_core and "!verified && !accept_unknown_sender" in import_core,
        "export_needs_checked_contacts": "CF_BUNDLE_PEER_UNVERIFIED" in allfns["bundle_export_core"],
        "commands_exist_and_are_mapped_to_the_edition": all(c in cmds and emap["commands"].get(c) == "bundle_exchange" for c in COMMANDS),
        "commands_check_the_feature_first": all(re.search(r"\{\s*edition::require_feature\(\"bundle_exchange\"\)\?;", cmds[c]) for c in COMMANDS if c in cmds),
        "commands_registered": all(f"{c}," in lib for c in COMMANDS),
        "curve25519_is_already_in_the_lock_file": "curve25519-dalek" in (ROOT / "src-tauri" / "Cargo.lock").read_text(encoding="utf-8"),
        "panel_lists_only_checked_contacts_to_send_to": "verifiedPeers.map" in panel and "code not checked" in panel,
        "panel_says_it_never_leaves_the_machines": "never goes to a server or the cloud" in panel,
        "panel_explains_redaction_honestly": "cannot be undone in the bundle" in panel and "Your own copy of the matter is not changed" in panel and "the check finds the phrases you give it" in panel,
        "panel_requires_acceptance_of_unchecked_senders": "acceptUnknown" in panel and "Anyone can make a bundle for your key" in panel,
        "panel_says_received_corrections_never_replace_yours": "yours stays" in panel and "Nothing here has changed your own" in panel,
        "tab_is_in_the_case_file": '"share"' in app and "<BundlePanel" in app,
        "ipc_has_the_commands": all(re.search(rf'"{c}"', ipc) for c in COMMANDS),
        "panel_vocabulary_honest": not any(re.search(rf"(?i)['\"`>][^'\"`<]*\b{re.escape(w)}\b", panel) for w in PROHIBITED_UI),
    }
    for k, v in static.items():
        if not v:
            errors.append(f"static check failed: {k}")

    manifest_ok = None
    lan_ev: dict = {}
    if args.evidence_dir and (args.evidence_dir / "bundle_manifest.json").is_file():
        try:
            import jsonschema

            m = json.loads((args.evidence_dir / "bundle_manifest.json").read_text(encoding="utf-8"))
            jsonschema.Draft202012Validator(schema, format_checker=jsonschema.FormatChecker()).validate(m)
            assert len(m["recipients"]) == 2 and all(re.fullmatch(r"[a-f0-9]{64}", r["agreement_public_key_hex"]) for r in m["recipients"])
            assert all(set(e) == {"item_id", "redaction_count"} for e in m["redaction_log"]), "counts only"
            assert not re.search(r"Ramesh|9876|रमेश", json.dumps(m, ensure_ascii=False)), "redacted text is not in the manifest"
            manifest_ok = True
        except Exception as e:  # noqa: BLE001
            manifest_ok = False
            errors.append(f"manifest evidence invalid: {e}")
    if args.evidence_dir and (args.evidence_dir / "bundle_lan.json").is_file():
        lan_ev = json.loads((args.evidence_dir / "bundle_lan.json").read_text(encoding="utf-8"))
        if not (lan_ev.get("bound") and all(b.startswith("127.") or b.startswith("10.") or b.startswith("192.168.") or re.match(r"172\.(1[6-9]|2\d|3[01])\.", b) for b in lan_ev["bound"])):
            errors.append("the transfer bound a non-local address")
        if not all(ok for _, ok in lan_ev.get("peers_seen", [])):
            errors.append("a non-local peer was served")

    if args.cargo_log and args.cargo_log.is_file():
        log = args.cargo_log.read_text(encoding="utf-8", errors="replace")
        res = {k: ("ok" if re.search(rf"test {re.escape(t)} \.\.\. ok", log) else "FAILED" if re.search(rf"test {re.escape(t)} \.\.\. FAILED", log) else "missing") for k, t in TESTS.items()}
        ok = lambda *ks: all(res[k] == "ok" for k in ks)  # noqa: E731
        s1 = ok("primitives", "identity", "keys", "tamper", "swap", "signer", "schema", "vault_flow", "vault_trust") and manifest_ok is not False and static["algorithms_are_the_frozen_ones"] and static["signature_covers_the_manifest"] and static["hex_fields_must_be_lowercase"]
        acs["AC-S4-10.1"] = {"status": "PASS" if s1 and manifest_ok and not errors else "FAIL",
                             "evidence": {"cipher": "X25519 + HKDF-SHA256 + AES-256-GCM, signed Ed25519", "manifest_schema_validated": manifest_ok, "every_byte_flipped": "all bytes of a small bundle and a sample of a large one", "wrong_key": "CF_BUNDLE_NOT_FOR_YOU", "changed_file": "CF_BUNDLE_TAMPERED / CF_BUNDLE_BAD_FILE"},
                             "detail": "A bundle opens only with the recipient's key; a stranger's key, the sender's, and an impostor with a recipient's key id all fail; every flipped byte, truncation, swapped or grafted item is rejected. The sender is whoever's key signed it: the app shows that key's code and asks you to accept an unchecked sender by hand."}
        s2 = ok("redaction_rules", "redacted", "refuses_leak", "vault_flow") and static["redaction_runs_before_encryption"] and static["export_refuses_when_a_term_survives"] and static["redaction_log_is_counts_only"]
        acs["AC-S4-10.2"] = {"status": "PASS" if s2 and not errors else "FAIL",
                             "evidence": {"how": "redaction on the plaintext before encryption; an independent normalising check refuses the export if a term survives; terms searched for in the file and in all decrypted items in raw, JSON-escaped, upper, lower, spaceless and split-by-newline shapes", "limits": "finds the phrases it is given; a different spelling, a nickname or a number not listed is not found"},
                             "detail": "Redacted wording is in no byte of the bundle or of anything decrypted from it, the log holds counts only, and the sender's own copy is untouched."}
        s3 = ok("lan_addresses", "lan_transfer", "lan_stop", "lan_module") and all(static[k] for k in ("lan_uses_the_companion_rules", "lan_never_resolves_a_name", "lan_serves_one_download", "lan_peers_filtered_before_reading", "lan_commands_refuse_under_air_gap", "lan_has_no_relay_or_cloud_word"))
        acs["AC-S4-10.3"] = {"status": "BLOCKED" if s3 and not errors else "FAIL",
                             "reason": "needs a packet capture on a real local network; the code-level LAN-only rules pass (below) but the criterion asks for a capture, which has not been made",
                             "evidence": {"addresses_bound": lan_ev.get("bound"), "peers_seen": lan_ev.get("peers_seen"), "connected_to": lan_ev.get("connected_to"), "rules": "private, link-local and loopback only; numeric addresses only; one download; refused under air-gap"},
                             "detail": "Enforced and tested in code, over loopback; the USB route needs no network."}
        s4 = ok("merge", "vault_flow") and all(static[k] for k in ("import_cannot_write_a_correction", "overlay_reader_is_read_only", "plan_function_is_pure"))
        acs["AC-S4-10.4"] = {"status": "PASS" if s4 and not errors else "FAIL",
                             "evidence": {"two_vaults": "the recipient's active corrections, correction events, graph and documents are identical before and after importing a bundle whose correction on the same event differs", "listed_as": "CONFLICTS_WITH_YOUR_CORRECTION (yours stays), SAME_AS_YOURS, or SUGGESTED, each attributed to the sender as COUNSEL_INPUT"},
                             "detail": "The import path cannot write a correction (static scan) and the received ones are kept beside the matter as attributed suggestions."}
        for k, v in res.items():
            if v != "ok":
                errors.append(f"test not passing: {TESTS[k]} ({v})")
    else:
        for ac in ("AC-S4-10.1", "AC-S4-10.2", "AC-S4-10.3", "AC-S4-10.4"):
            acs[ac] = {"status": "BLOCKED", "reason": "pass --cargo-log with `cargo test --lib` output"}
    report = {"batch": "B-S4-16", "golden_sha256": GOLD_SHA256, "acs": acs, "static": static, "errors": errors}
    print(json.dumps(report, indent=2))
    out = ROOT / "eval" / "reports" / "b_s4_16_bundle.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
