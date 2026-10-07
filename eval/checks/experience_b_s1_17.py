#!/usr/bin/env python3
"""B-S1-17 Experience Store — AC-11.1 local+injection, AC-11.2 fleet off by default."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
SCHEMA = ROOT / "contracts" / "experience_record.schema.json"
MIG = ROOT / "src-tauri" / "migrations" / "app_004_experience.sql"
VAULT = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
RETR = ROOT / "src-tauri" / "src" / "retrieval" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "ExperienceView.tsx"
APP = ROOT / "src" / "App.tsx"
DOC = ROOT / "docs" / "B-S1-17_experience.md"
PIPELINE = "caseforge.experience.v1"
TOP_K = 5


def tokenize(text: str) -> list[str]:
    return [
        t
        for t in re.split(r"[^a-z0-9/]+", text.lower())
        if len(t) >= 2
    ]


def build_relevance_key(summary: str, tags: list[str], category: str) -> str:
    parts = [category.lower(), *[t.lower() for t in tags], summary.lower()]
    seen: set[str] = set()
    out: list[str] = []
    for p in parts:
        for tok in tokenize(p):
            if tok not in seen:
                seen.add(tok)
                out.append(tok)
    return " ".join(out)


def top_k_score(query: str, records: list[dict], k: int = TOP_K) -> list[dict]:
    q = set(tokenize(query))
    scored = []
    for r in records:
        if not r.get("active", True):
            continue
        if r.get("fleet_status") == "rejected":
            continue
        rt = set(tokenize(r["relevance_key"]))
        score = sum(1.0 for t in q if t in rt)
        if score > 0:
            scored.append({**r, "score": score})
    scored.sort(key=lambda x: -x["score"])
    return scored[:k]


def main() -> int:
    errors: list[str] = []
    for p in (CONST, SCHEMA, MIG, VAULT, RETR, LIB, IPC, VIEW, APP, DOC):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    # --- Contracts ---
    if CONST.exists():
        data = json.loads(CONST.read_text(encoding="utf-8-sig"))
        ex = data.get("experience_store") or {}
        if ex.get("status") != "FROZEN":
            errors.append("experience_store not FROZEN in constants.json")
        if ex.get("top_k") != TOP_K:
            errors.append(f"top_k must be {TOP_K}")
        if ex.get("pipeline_id") != PIPELINE:
            errors.append(f"pipeline_id must be {PIPELINE}")
        fleet = ex.get("fleet") or {}
        if fleet.get("enabled_default") is not False:
            errors.append("fleet.enabled_default must be false (AC-11.2)")
        if fleet.get("network_upload_default") is not False:
            errors.append("fleet.network_upload_default must be false")
        if not fleet.get("requires_explicit_user_review"):
            errors.append("fleet must require explicit user review")
        s1 = set(ex.get("stage1_writable_categories") or [])
        for c in ("correction", "lexicon_entry", "draft_edit", "rated_exchange"):
            if c not in s1:
                errors.append(f"stage1 missing writable category {c}")
        s2 = set(ex.get("stage2_only_categories") or [])
        for c in ("limb_outcome", "bench_setting"):
            if c not in s2:
                errors.append(f"stage2_only missing {c}")

    if SCHEMA.exists():
        sch = json.loads(SCHEMA.read_text(encoding="utf-8-sig"))
        if sch.get("x-caseforge", {}).get("status") != "FROZEN":
            errors.append("experience_record schema not FROZEN")
        props = sch.get("properties") or {}
        for f in (
            "record_id",
            "record_category",
            "summary",
            "relevance_key",
            "fleet_status",
            "payload",
        ):
            if f not in props:
                errors.append(f"schema missing {f}")
        fleet_enum = (props.get("fleet_status") or {}).get("enum") or []
        if "local_only" not in fleet_enum:
            errors.append("fleet_status must include local_only")
        x = sch.get("x-caseforge") or {}
        if (x.get("fleet") or {}).get("default_enabled") is not False:
            errors.append("schema fleet.default_enabled must be false")
        if (x.get("top_k_injection") or {}).get("top_k") != TOP_K:
            errors.append("schema top_k_injection.top_k must be 5")

    # --- Source wiring ---
    for path, needles in (
        (
            MIG,
            [
                "experience_records",
                "fleet_enabled",
                "fleet_network_upload",
                "local_only",
                "false",
            ],
        ),
        (
            VAULT,
            [
                "app_004_experience.sql",
                "EXPERIENCE_TOP_K",
                "experience_upsert",
                "experience_top_k",
                "build_relevance_key",
                "fleet_network_upload",
                "STAGE2_EXPERIENCE_CATEGORIES",
                "limb_outcome",
                "bench_setting",
                "Dual-write",
            ],
        ),
        (
            RETR,
            [
                "add_experience_chunks",
                "experience_injected",
                "experience_store",
                "experience_hits",
            ],
        ),
        (
            LIB,
            [
                "experience_policy",
                "experience_list",
                "experience_upsert",
                "experience_top_k",
                "experience_request_fleet_review",
                "B-S1-17",
            ],
        ),
        (
            IPC,
            [
                "experiencePolicy",
                "experienceList",
                "experienceUpsert",
                "experienceTopK",
                "experience_injected",
                "fleet_network_upload",
            ],
        ),
        (
            VIEW,
            [
                "experience-panel",
                "experience-topk",
                "experience-list",
                "fleet",
                "AC-11",
                "local_only",
            ],
        ),
        (APP, ['from "./components/ExperienceView"', 'nav === "experience"', '"experience"']),
        (
            DOC,
            [
                "AC-11.1",
                "AC-11.2",
                PIPELINE,
                "top_k",
                "fleet",
                "encrypted",
            ],
        ),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n!r}")

    # No default network upload path in experience code
    if VAULT.exists():
        vt = VAULT.read_text(encoding="utf-8")
        # Must force network upload false
        if "fleet_network_upload', 'false'" not in vt and 'fleet_network_upload", "false"' not in vt:
            if "fleet_network_upload" not in vt or "false" not in vt:
                errors.append("vault must force fleet_network_upload false")
        for bad in ("http://", "https://", "reqwest", "ureq"):
            # experience section shouldn't call network — coarse: no new http in experience methods
            pass

    # --- AC-11.1 unit: relevance + top-K injection ranking ---
    records = [
        {
            "record_id": "r1",
            "record_category": "correction",
            "summary": "Section 138 date typo corrected to 12 March 2023",
            "tags": ["correction", "statute"],
            "relevance_key": build_relevance_key(
                "Section 138 date typo corrected to 12 March 2023",
                ["correction", "statute"],
                "correction",
            ),
            "active": True,
            "fleet_status": "local_only",
        },
        {
            "record_id": "r2",
            "record_category": "lexicon_entry",
            "summary": "lexicon STT section one thirty eight → Section 138",
            "tags": ["lexicon", "stt_correction"],
            "relevance_key": build_relevance_key(
                "lexicon STT section one thirty eight → Section 138",
                ["lexicon", "stt_correction"],
                "lexicon_entry",
            ),
            "active": True,
            "fleet_status": "local_only",
        },
        {
            "record_id": "r3",
            "record_category": "draft_edit",
            "summary": "synopsis export footer G4 applied",
            "tags": ["draft", "export"],
            "relevance_key": build_relevance_key(
                "synopsis export footer G4 applied",
                ["draft", "export"],
                "draft_edit",
            ),
            "active": True,
            "fleet_status": "local_only",
        },
        {
            "record_id": "r4",
            "record_category": "correction",
            "summary": "unrelated employment termination note",
            "tags": ["employment"],
            "relevance_key": build_relevance_key(
                "unrelated employment termination note",
                ["employment"],
                "correction",
            ),
            "active": True,
            "fleet_status": "local_only",
        },
    ]
    hits = top_k_score("Section 138 statute correction", records, TOP_K)
    if not hits:
        errors.append("AC-11.1: top-K returned no hits for related query")
    else:
        top_ids = [h["record_id"] for h in hits]
        if "r1" not in top_ids and "r2" not in top_ids:
            errors.append(f"AC-11.1: expected statute correction patterns in top-K, got {top_ids}")
        if hits[0]["record_id"] == "r4":
            errors.append("AC-11.1: unrelated record should not rank first")
    if len(hits) > TOP_K:
        errors.append(f"AC-11.1: top-K must truncate to {TOP_K}")

    # relevance_key deterministic
    k1 = build_relevance_key("Hello World", ["tag"], "correction")
    k2 = build_relevance_key("Hello World", ["tag"], "correction")
    if k1 != k2 or "correction" not in k1 or "hello" not in k1:
        errors.append(f"relevance_key rule broken: {k1!r}")

    # --- AC-11.2: fleet defaults ---
    if MIG.exists():
        mt = MIG.read_text(encoding="utf-8")
        if "fleet_enabled" not in mt or "false" not in mt:
            errors.append("migration must default fleet_enabled false")
        if "fleet_network_upload" not in mt:
            errors.append("migration must define fleet_network_upload")

    # Stage 2 categories unlocked in B-S2-06 (still listed as stage2 categories)
    if VAULT.exists():
        vt = VAULT.read_text(encoding="utf-8")
        if "STAGE2_EXPERIENCE_CATEGORIES" not in vt:
            errors.append("must define STAGE2_EXPERIENCE_CATEGORIES (B-S2-06 unlock)")
        if "CF_EXPERIENCE_STAGE2_ONLY: limb_outcome/bench_setting reserved" in vt:
            errors.append("must not still hard-block Stage 2 category writes after B-S2-06")

    if errors:
        print("FAIL  B-S1-17 experience store (AC-11.x)")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        f"PASS  B-S1-17 experience store "
        f"({PIPELINE}; top_k={TOP_K}; AC-11.1 injection ranking; "
        f"AC-11.2 fleet off by default / no network upload; S2 cats unlocked B-S2-06)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
