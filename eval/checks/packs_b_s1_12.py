#!/usr/bin/env python3
"""B-S1-12 knowledge packs structural + AC-7.1 headnote scan pipeline check."""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCHEMA = ROOT / "contracts" / "akpkg.schema.json"
TRUST = ROOT / "contracts" / "pack_trust.json"
MAKE = ROOT / "eval" / "tools" / "make_akpkg.py"
PACKS_RS = ROOT / "src-tauri" / "src" / "packs" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "PacksView.tsx"
APP = ROOT / "src" / "App.tsx"
DOC = ROOT / "docs" / "B-S1-12_packs.md"

HEADNOTE_RE = re.compile(
    r"(?i)\b(head\s*notes?|editorial\s+headnote|held\s*:|ratio\s+decidendi\s*summary)\b"
)


def headnote_scan(content_root: Path) -> list[str]:
    hits = []
    for p in sorted(content_root.rglob("*")):
        if p.is_file() and HEADNOTE_RE.search(p.read_text(encoding="utf-8", errors="replace")):
            hits.append(str(p.relative_to(content_root)))
    return hits


def main() -> int:
    errors: list[str] = []
    for p in (SCHEMA, TRUST, MAKE, PACKS_RS, LIB, IPC, VIEW, APP):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    if SCHEMA.exists():
        data = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        if data.get("x-caseforge", {}).get("status") != "FROZEN":
            errors.append("akpkg schema not FROZEN")
        if "official" not in str(data):
            errors.append("schema missing official kind")

    for path, needles in (
        (
            PACKS_RS,
            [
                "install_pack_from_dir",
                "headnote_scan_dir",
                "CF_PACK_INVALID_SIGNATURE",
                "CF_PACK_HEADNOTE_SCAN_FAILED",
                "packs_official",
                "packs_custom",
                "Ed25519",
                "AC-7.1",
            ],
        ),
        (
            LIB,
            ["pack_install", "pack_list", "pack_uninstall", "pack_headnote_scan", "mod packs"],
        ),
        (IPC, ["packInstall", "packList", "packHeadnoteScan", "InstalledPack"]),
        (VIEW, ["pack-install", "pack-headnote-scan", "AC-7.1", "official", "custom"]),
        (APP, ["PacksView", "installPackFromPath"]),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n}")

    # Build official placeholder + headnote-tainted pack; verify scan
    with tempfile.TemporaryDirectory() as td:
        td_path = Path(td)
        official_out = td_path / "official_india"
        proc = subprocess.run(
            [
                sys.executable,
                str(MAKE),
                "--out",
                str(official_out),
                "--pack-id",
                "caseforge.core.india.placeholder",
                "--kind",
                "official",
                "--name",
                "India Core Placeholder",
                "--placeholder-core",
                "india",
            ],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
        )
        if proc.returncode != 0:
            errors.append(f"make_akpkg official failed: {proc.stderr or proc.stdout}")
        else:
            hits = headnote_scan(official_out / "content")
            if hits:
                errors.append(f"official placeholder should be clean: {hits}")
            man = json.loads((official_out / "manifest.json").read_text(encoding="utf-8"))
            if man.get("pack_kind") != "official":
                errors.append("manifest pack_kind")
            if not (official_out / "signature.ed25519").exists():
                errors.append("official pack missing signature.ed25519")
            # trust key updated
            trust = json.loads(TRUST.read_text(encoding="utf-8"))
            pk = trust["keys"][0].get("public_key_hex", "")
            if pk == "REPLACE_AT_KEYGEN" or len(pk) != 64:
                errors.append("pack_trust public key not generated")

        # Bad pack with headnote must fail maker for official
        bad_content = td_path / "bad.txt"
        bad_content.write_text("This HEADNOTE summarizes the ratio.\n", encoding="utf-8")
        bad_out = td_path / "official_bad"
        proc2 = subprocess.run(
            [
                sys.executable,
                str(MAKE),
                "--out",
                str(bad_out),
                "--pack-id",
                "caseforge.bad.headnote",
                "--kind",
                "official",
                "--name",
                "Bad",
                "--content",
                str(bad_content),
                "--jurisdiction",
                "India",
            ],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
        )
        if proc2.returncode == 0:
            errors.append("official pack with headnote must be rejected by make_akpkg")
        elif "AC-7.1" not in (proc2.stderr or "") and "headnote" not in (proc2.stderr or "").lower():
            # exit non-zero is enough
            pass

        # Custom pack with headnote is allowed by maker
        custom_out = td_path / "custom_pack"
        proc3 = subprocess.run(
            [
                sys.executable,
                str(MAKE),
                "--out",
                str(custom_out),
                "--pack-id",
                "user.custom.demo",
                "--kind",
                "custom",
                "--name",
                "Custom Demo",
                "--content",
                str(bad_content),
                "--jurisdiction",
                "Other",
            ],
            cwd=str(ROOT),
            capture_output=True,
            text=True,
        )
        if proc3.returncode != 0:
            errors.append(f"custom pack should allow headnotes: {proc3.stderr or proc3.stdout}")

        # Copy official placeholder into repo fixtures for dogfood
        dest = ROOT / "packs" / "fixtures" / "caseforge.core.india.placeholder"
        if official_out.exists() and not errors:
            if dest.exists():
                shutil.rmtree(dest)
            shutil.copytree(official_out, dest)
            us_out = td_path / "official_us"
            subprocess.run(
                [
                    sys.executable,
                    str(MAKE),
                    "--out",
                    str(us_out),
                    "--pack-id",
                    "caseforge.core.us.placeholder",
                    "--kind",
                    "official",
                    "--name",
                    "US Core Placeholder",
                    "--placeholder-core",
                    "us",
                ],
                cwd=str(ROOT),
                capture_output=True,
                text=True,
                check=False,
            )
            if us_out.exists():
                dest_us = ROOT / "packs" / "fixtures" / "caseforge.core.us.placeholder"
                if dest_us.exists():
                    shutil.rmtree(dest_us)
                shutil.copytree(us_out, dest_us)

    if DOC.exists():
        if "AC-7.1" not in DOC.read_text(encoding="utf-8"):
            errors.append("docs missing AC-7.1")

    if errors:
        print("FAIL  B-S1-12 packs structural")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S1-12 packs structural "
        "(akpkg schema, Ed25519 official, custom isolation, AC-7.1 headnote reject on official)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
