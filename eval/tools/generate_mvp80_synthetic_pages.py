#!/usr/bin/env python3
"""Generate synthetic G-DOCS staging page texts (MVP-80 or full 500-page ship pack).

Pages are SYNTHETIC pedagogy — not client files, not real authorities.
Does NOT set GOLDEN. Locking requires product owner / dual review (see POLICY.md).

Intended class lives only in calibration_manifest.csv — not as a class_hint
footer that classifiers could cheat on.
"""

from __future__ import annotations

import argparse
import csv
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
STAGING = ROOT / "golden" / "G-DOCS" / "mvp80" / "staging"
MVP80 = ROOT / "golden" / "G-DOCS" / "mvp80"

# Quotas from golden/G-DOCS/MVP_80_PLAN.md (sum = 80)
BASE_QUOTAS = {
    "typed_english": 32,
    "clean_handwriting": 12,
    "messy_handwriting": 10,
    "mixed": 10,
    "other": 10,
    "non_text": 6,
}
BASE_TOTAL = sum(BASE_QUOTAS.values())


def quotas_for(count: int) -> dict[str, int]:
    """Scale base MVP-80 class ratios to exactly `count` pages."""
    if count <= 0:
        raise ValueError("count must be positive")
    if count == BASE_TOTAL:
        return dict(BASE_QUOTAS)
    if count < BASE_TOTAL:
        # Truncate in class order (same as historical --count < 80 behaviour)
        plan: dict[str, int] = {k: 0 for k in BASE_QUOTAS}
        n = 0
        for cls, quota in BASE_QUOTAS.items():
            for _ in range(quota):
                if n >= count:
                    return plan
                plan[cls] += 1
                n += 1
        return plan

    # Largest-remainder proportional scale
    raw = {cls: count * q / BASE_TOTAL for cls, q in BASE_QUOTAS.items()}
    floors = {cls: int(v) for cls, v in raw.items()}
    rem = count - sum(floors.values())
    order = sorted(BASE_QUOTAS.keys(), key=lambda c: (raw[c] - floors[c]), reverse=True)
    for cls in order[:rem]:
        floors[cls] += 1
    return floors


def typed_english(i: int) -> str:
    return f"""IN THE COURT OF [SYNTHETIC CALIBRATION]
Civil Miscellaneous Application No. {1000 + i} of 2024

Applicant: Acme Calibration Pvt. Ltd.
Respondent: Northern Sample Traders

ORDER

1. Heard learned counsel for the parties on the limited prayer for interim protection.
2. The dispute concerns supply of industrial fasteners under a written purchase order dated 12 March 2023.
3. Prima facie, the documents on record show delivery challans and a running account statement.
4. Without expressing any opinion on merits, the respondent is directed to maintain status quo
   regarding the disputed goods lying in Warehouse B until the next date of hearing.
5. List on 15 September 2024 for further consideration. Variant serial {i}.

Sd/-
[Synthetic Judge Name]
Judge (Calibration Only)

[CASEFORGE_SYNTHETIC_PAGE]
page_index: {i}
"""


def clean_handwriting(i: int) -> str:
    return f"""Counsel's neat handwritten conference notes (synthetic — clean handwriting)

Matter: Estate of Sample / partition notes
Date: 04.02.2024  · note #{i}

- Client states father purchased plot in 1998 from savings (self-acquired claim).
- Need: title deed copy, mutation entries, tax receipts 2010-2023.
- Sister may rely on 2008 gift deed — verify attesting witnesses.
- Interim: do not file partition suit until genealogy chart is complete.
- Follow-up call scheduled Tuesday 10:30.

Written carefully in clear lines for labeling practice. Clean handwriting throughout.

[CASEFORGE_SYNTHETIC_PAGE]
page_index: {i}
"""


def messy_handwriting(i: int) -> str:
    return f"""wits notes ??? rushed  (messy handwriting synthetic #{i})

mtg w/ client — said he PAID cash?? no rcpt
chk bounced? or bank err?? call branch
dates: mar??? 22 or april — unclear
opp says "no debt" — see sms screenshots later
URGENT: file before limtn??? check calendar

scrawl / crossouts / hard to read intentionally — messy handwriting sample

[CASEFORGE_SYNTHETIC_PAGE]
page_index: {i}
"""


def mixed(i: int) -> str:
    return f"""TYPED NOTICE (synthetic mixed page #{i})

To,
The Manager,
Sample Cooperative Bank

Subject: Dishonour of Cheque No. {2000 + i}

Please take notice that Cheque No. {2000 + i} drawn on your bank for INR 1,25,000/-
was returned unpaid on 18.01.2024. You are called upon to make payment within 15 days.

--- HANDWRITTEN MARGIN NOTES (synthetic) ---
OK to send? — yes, after confirm address
Add ACK request  [initials]
Call client re quantum — maybe 1,50,000?

[CASEFORGE_SYNTHETIC_PAGE]
page_index: {i}
"""


def other(i: int) -> str:
    # Spanish-dominant synthetic "other" (not typed English primary)
    return f"""AVISO SINTÉTICO DE CALIBRACIÓN — página {i}

Expediente ficticio N.º {3000 + i}
Demandante: Empresa Ejemplo S.A.
Demandado: Comercio del Norte

Se presenta la presente demanda de reclamación de cantidad por impago de facturas
de suministro correspondientes al ejercicio 2023. El texto principal está en español
y no es predominantemente inglés mecanografiado.

Nota: material sintético CaseForge — no es un caso real.

[CASEFORGE_SYNTHETIC_PAGE]
page_index: {i}
"""


def non_text(i: int) -> str:
    # Near-empty capture: few printable chars so heuristic can mark non_text
    return f"""


. {i}


"""


GENERATORS = {
    "typed_english": typed_english,
    "clean_handwriting": clean_handwriting,
    "messy_handwriting": messy_handwriting,
    "mixed": mixed,
    "other": other,
    "non_text": non_text,
}


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate synthetic G-DOCS staging pages")
    ap.add_argument(
        "--count",
        type=int,
        default=80,
        help="Total pages (default 80 = MVP quotas; use 500 for AC-6.1 pack)",
    )
    ap.add_argument(
        "--clean-staging",
        action="store_true",
        help="Delete staging/* before write (recommended when changing count)",
    )
    args = ap.parse_args()

    quotas = quotas_for(args.count)
    if args.clean_staging and STAGING.exists():
        shutil.rmtree(STAGING)
    STAGING.mkdir(parents=True, exist_ok=True)

    plan: list[tuple[str, int]] = []
    for cls, quota in quotas.items():
        for j in range(1, quota + 1):
            plan.append((cls, j))

    manifest_rows: list[dict[str, str]] = []
    for page_num, (cls, local_i) in enumerate(plan, start=1):
        slug = f"page_{page_num:04d}"
        folder = STAGING / cls
        folder.mkdir(parents=True, exist_ok=True)
        fname = f"{slug}.txt"
        path = folder / fname
        body = GENERATORS[cls](local_i)
        path.write_text(body, encoding="utf-8")
        source_id = "S-SYNTH-MVP80" if args.count <= 80 else "S-SYNTH-GDOCS-500"
        manifest_rows.append(
            {
                "fixture_id": f"G-DOCS/{slug}",
                "slug": slug,
                "intended_class": cls,
                "staging_path": str(path.relative_to(ROOT)).replace("\\", "/"),
                "source_id": source_id,
                "rights": "synthetic_pedagogy",
                "status_after_scaffold": "DRAFT",
                "may_lock_golden_after": "dual_human_review_per_POLICY",
            }
        )

    MVP80.mkdir(parents=True, exist_ok=True)
    man_csv = MVP80 / "calibration_manifest.csv"
    with man_csv.open("w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=list(manifest_rows[0].keys()))
        w.writeheader()
        w.writerows(manifest_rows)

    man_json = MVP80 / "calibration_manifest.json"
    man_json.write_text(
        json.dumps(
            {
                "description": (
                    "Synthetic G-DOCS page plan. Intended class is adjudicator key only — "
                    "not GOLDEN until product-owner lock."
                ),
                "count": len(manifest_rows),
                "quotas": quotas,
                "pages": manifest_rows,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )

    print(f"PASS  generated {len(manifest_rows)} synthetic staging pages")
    print(f"  quotas: {quotas}")
    print(f"  staging: {STAGING}")
    print(f"  manifest: {man_csv}")
    print("  next: python eval/tools/scaffold_gdocs_batch.py --from-manifest --force --blind")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
