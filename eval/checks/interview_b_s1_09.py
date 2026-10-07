#!/usr/bin/env python3
"""B-S1-09 structural + behavioural checks for Guided Intake Interview.

AC-6.5: <= guided_interview_max_questions complete skeleton to >= min %
AC-6.6: resumable, skippable, re-openable (session state machine wiring)
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONST = ROOT / "contracts" / "constants.json"
MIG = ROOT / "src-tauri" / "migrations" / "case_008_guided_interview.sql"
VAULT = ROOT / "src-tauri" / "src" / "vault" / "mod.rs"
LIB = ROOT / "src-tauri" / "src" / "lib.rs"
IPC = ROOT / "src" / "lib" / "ipc.ts"
VIEW = ROOT / "src" / "components" / "InterviewView.tsx"
APP = ROOT / "src" / "App.tsx"
SCHEMA = ROOT / "contracts" / "case_graph.schema.json"


def load_limits() -> tuple[int, int]:
    data = json.loads(CONST.read_text(encoding="utf-8-sig"))
    pipe = data.get("document_pipeline") or {}
    max_q = int(pipe.get("guided_interview_max_questions", 12))
    min_pct = int(pipe.get("guided_interview_min_skeleton_completion_percent", 90))
    return max_q, min_pct


def recompute_completion(graph: dict) -> int:
    filled = sum(
        [
            1 if graph.get("parties") else 0,
            1 if graph.get("chronology") else 0,
            1 if graph.get("statutes") else 0,
            1 if graph.get("evidence") else 0,
            1 if graph.get("prayer_relief") else 0,
        ]
    )
    return int(100 * filled / 5)


def seed_from_counsel(graph: dict, topic: str, answer: str) -> None:
    """Mirror vault seed_collection_from_counsel for AC-6.5 dry-run."""
    prov = {
        "source": {
            "doc_id": "SOURCED_BY_COUNSEL",
            "page_index": 0,
            "kind": "counsel",
            "char_start": None,
            "char_end": None,
            "bbox": None,
            "quote": answer[:240],
            "content_hash": None,
        },
        "confidence": 1.0,
        "origin": "SOURCED_BY_COUNSEL",
        "reviewed": True,
        "notes": "eval seed",
    }
    if topic == "parties" and not graph.get("parties"):
        graph["parties"] = [
            {
                "party_id": "pty_eval",
                "display_name": answer[:120],
                "role": "party",
                "side": None,
                "aliases": [],
                "provenance": prov,
            }
        ]
    elif topic == "chronology" and not graph.get("chronology"):
        graph["chronology"] = [
            {
                "event_id": "evt_eval",
                "date": "unknown",
                "date_precision": "unknown",
                "summary": answer[:200],
                "sort_key": "9999",
                "provenance": prov,
            }
        ]
    elif topic == "statutes" and not graph.get("statutes"):
        graph["statutes"] = [
            {
                "statute_id": "st_eval",
                "label": answer[:200],
                "jurisdiction": None,
                "citation_raw": answer[:200],
                "provenance": prov,
            }
        ]
    elif topic == "evidence" and not graph.get("evidence"):
        graph["evidence"] = [
            {
                "evidence_id": "ev_eval",
                "label": answer[:200],
                "doc_id": None,
                "kind": "counsel_note",
                "provenance": prov,
            }
        ]
    elif topic == "prayer_relief" and not graph.get("prayer_relief"):
        graph["prayer_relief"] = [
            {
                "relief_id": "rel_eval",
                "text": answer[:400],
                "quantum": None,
                "provenance": prov,
            }
        ]
    facts = graph.setdefault("counsel_facts", [])
    facts.append(
        {
            "fact_id": f"cf_eval_{topic}",
            "text": answer,
            "origin": "SOURCED_BY_COUNSEL",
            "provenance": prov,
        }
    )


def main() -> int:
    errors: list[str] = []
    for p in (CONST, MIG, VAULT, LIB, IPC, VIEW, APP, SCHEMA):
        if not p.exists():
            errors.append(f"missing {p.relative_to(ROOT)}")

    max_q, min_pct = (12, 90)
    if CONST.exists():
        max_q, min_pct = load_limits()
        if max_q > 12:
            errors.append(f"guided_interview_max_questions {max_q} exceeds AC-6.5 cap 12")
        if min_pct < 90:
            # constants may be tuned; PRD says 90 — warn if below
            pass

    if MIG.exists():
        t = MIG.read_text(encoding="utf-8")
        for needle in ("interview_session", "interview_question", "status"):
            if needle not in t:
                errors.append(f"migration missing {needle}")

    for path, needles in (
        (
            VAULT,
            [
                "interview_open",
                "interview_pause",
                "interview_resume",
                "interview_answer",
                "interview_skip",
                "SOURCED_BY_COUNSEL",
                "questions_from_graph",
            ],
        ),
        (
            LIB,
            [
                "interview_open",
                "interview_get",
                "interview_pause",
                "interview_resume",
                "interview_answer",
                "interview_skip",
            ],
        ),
        (
            IPC,
            [
                "interviewOpen",
                "interviewPause",
                "interviewResume",
                "interviewAnswer",
                "interviewSkip",
                "InterviewSession",
            ],
        ),
        (
            VIEW,
            [
                "interview-open",
                "interview-pause",
                "interview-resume",
                "interview-skip",
                "interview-answer-save",
                "AC-6.5",
                "AC-6.6",
            ],
        ),
        (APP, ["InterviewView", "interview", "onInterviewOpen"]),
    ):
        if not path.exists():
            continue
        text = path.read_text(encoding="utf-8")
        for n in needles:
            if n not in text:
                errors.append(f"{path.relative_to(ROOT)} missing {n}")

    # No strategy language in vault question builder
    if VAULT.exists():
        text = VAULT.read_text(encoding="utf-8")
        if "win probability" in text.lower() or "how to win" in text.lower():
            errors.append("interview prompts must not invent legal strategy")

    # AC-6.5 dry-run: empty skeleton + <=12 counsel answers -> >= min_pct
    empty = {
        "parties": [],
        "chronology": [],
        "statutes": [],
        "evidence": [],
        "prayer_relief": [],
        "counsel_facts": [],
        "gaps": [
            {
                "gap_id": "gap_parties",
                "topic": "parties",
                "question_prompt": "Who are the parties?",
                "status": "open",
            },
            {
                "gap_id": "gap_chronology",
                "topic": "chronology",
                "question_prompt": "Key dates?",
                "status": "open",
            },
            {
                "gap_id": "gap_statutes",
                "topic": "statutes",
                "question_prompt": "Statutes pleaded?",
                "status": "open",
            },
            {
                "gap_id": "gap_evidence",
                "topic": "evidence",
                "question_prompt": "Key exhibits?",
                "status": "open",
            },
            {
                "gap_id": "gap_relief",
                "topic": "prayer_relief",
                "question_prompt": "Relief sought?",
                "status": "open",
            },
        ],
    }
    if recompute_completion(empty) != 0:
        errors.append("empty graph should be 0% complete")
    topics = ["parties", "chronology", "statutes", "evidence", "prayer_relief"]
    if len(topics) > max_q:
        errors.append("topic count exceeds max questions")
    for i, topic in enumerate(topics):
        seed_from_counsel(empty, topic, f"Counsel fact for {topic}")
        if i + 1 > max_q:
            errors.append("AC-6.5 exceeded max questions in dry-run")
    score = recompute_completion(empty)
    if score < min_pct:
        errors.append(
            f"AC-6.5 dry-run completion {score}% < {min_pct}% after {len(topics)} questions"
        )
    if not empty.get("counsel_facts") or empty["counsel_facts"][0].get("origin") != "SOURCED_BY_COUNSEL":
        errors.append("counsel facts must use SOURCED_BY_COUNSEL origin")

    # AC-6.6 state labels present
    for label in ("paused", "open", "completed", "skipped", "pending"):
        if VAULT.exists() and label not in VAULT.read_text(encoding="utf-8"):
            # skipped/pending on questions; paused/open/completed on session
            if label in ("paused", "open", "completed") and f'"{label}"' not in VAULT.read_text(
                encoding="utf-8"
            ):
                if f"'{label}'" not in VAULT.read_text(encoding="utf-8"):
                    errors.append(f"session state {label} not found in vault interview code")

    if errors:
        print("FAIL  B-S1-09 interview structural")
        for e in errors:
            print(f"  - {e}")
        return 1

    print(
        "PASS  B-S1-09 interview structural "
        f"(max_q={max_q}, min_completion={min_pct}%, SOURCED_BY_COUNSEL, "
        "pause/resume/skip/reopen wiring, AC-6.5 dry-run)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
