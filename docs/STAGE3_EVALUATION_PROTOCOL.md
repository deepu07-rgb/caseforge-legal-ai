# CaseForge Stage 3 evaluation protocol

**Contract version:** 1.0.0  
**Frozen:** 2026-08-13 by B-S3-02

## Decision rule

Stage 3 has two independent states:

1. **Batch contract state** — the fixture schema, metric definitions, provenance,
   integrity validation, evidence separation, and fail-closed rules are valid.
2. **Controlled-beta evidence readiness** — every required suite has at least one
   integrity-verified, human-locked `GOLDEN` fixture and its required measurements.

A contract PASS never implies product readiness. Missing assets, labels,
reviewers, legal packs, measurement files, or hardware runs produce `BLOCKED` or
`NOT_TESTED`, never PASS.

## Source classes

| Source class | Meaning | May satisfy real-world evidence? |
|---|---|---|
| `REAL_WORLD_RIGHTS_CLEARED` | Acquired real document with documented internal-use or redistribution rights | Yes, after GOLDEN lock |
| `AUTHORITATIVE_LEGAL` | Versioned legal repository with source and licence provenance | Yes, for legal grounding only |
| `HUMAN_AUDIO_RIGHTS_CLEARED` | Consented or licensed human recording | Yes, for voice only |
| `SYNTHETIC_CONTROLLED` | CaseForge-created deterministic case or performance input | No |
| `ADVERSARIAL_SYNTHETIC` | CaseForge-created injection/red-team input | No |

Results are emitted under separate `source_class` keys. The runner never emits
a pooled real-plus-synthetic aggregate.

## Fixture lifecycle

`DRAFT` → `SOURCE_IDENTIFIED` → `READY_FOR_REVIEW` → `GOLDEN`

`BLOCKED` may be used at any point when licensing, confidentiality, source
availability, labels, or adjudication prevent evaluation. Only a human or
product owner may lock `GOLDEN`. To lock a fixture:

- confirm its source and rights basis;
- verify every asset checksum and byte count;
- inspect the expected answer;
- record two reviewers, or record adjudication and the adjudicator;
- set expected-answer status to `DUAL_REVIEWED` or `ADJUDICATED`;
- set fixture status to `GOLDEN` and rerun validation.

The schema is `contracts/eval_fixture_manifest.schema.json`. Fixture paths must
be workspace-relative and cannot escape the workspace.

## Metrics and results

`eval/stage3_metrics.json` freezes every metric's formula, threshold, permitted
source classes, human-review rule, and command. Threshold changes require a
registry version bump and explicit product-owner approval.

Measured result files live at:

```text
eval/results/stage3/<metric_id>.json
```

At minimum each result contains:

```json
{
  "metric_id": "page_count_accuracy",
  "source_class": "REAL_WORLD_RIGHTS_CLEARED",
  "fixture_ids": ["USCOURTS-PRO-SE-1-2016"],
  "samples": 1,
  "value": 1.0,
  "reviewed_by": []
}
```

The runner rejects results that do not name exclusively checksum-valid GOLDEN
fixtures, mix source classes, contain no samples, or omit required human review.
Metric implementations added by later batches must write measured result files;
source-code string inspection cannot create metric PASS evidence.

## Reproducible commands

```powershell
npm run stage3:fixtures
npm run test:s3-evidence
npm run report:s3-evidence
python eval/run_stage3.py --metric page_count_accuracy
python eval/run_stage3.py --strict-evidence
```

`stage3:fixtures` downloads the official U.S. Courts Pro Se 1 PDF and DOCX and
derives scan, mixed, PNG, JPG, degraded/rotated, and TXT variants. The manifest
does not authorize client redistribution. Network is not required once the
assets exist unless reacquisition is requested.

## Human scoring

Human-scored metrics require two independent reviewers. If they disagree, the
retained result must name an adjudicator and preserve the rationale alongside
the result. Legal judgment metrics require legally competent reviewers. A
single agent-generated label is always DRAFT.

## Environment and provenance

Every report records OS, CPU, physical RAM, Python, model backend, model name,
quantization, model digest, available pack manifest versions/digests, commit if
the workspace is a Git repository, and a SHA-256 source fingerprint otherwise.
This distinguishes contract validation on one machine from measurement on the
reference client hardware.

## Current blockers at B-S3-02 close

- The real official-form fixture is acquired and `READY_FOR_REVIEW`, not GOLDEN.
- Stamped, signed, table-heavy, and footnote-heavy rights-cleared documents are
  still blocked pending a documented source.
- Controlled India/US cases and injection prompts exist but lack locked labels.
- Authoritative India/US legal repository fixtures wait for legal-pack governance.
- 40/100/300-page performance corpora and reference-hardware measurements are absent.
- Consented rights-cleared human legal speech is absent.

These blockers are expected, visible outputs of the protocol—not waived passes.
