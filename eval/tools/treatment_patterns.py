#!/usr/bin/env python3
"""B-S4-13: treatment wording rules shared by the pack-build tool (sc_citation_graph.py) and the golden gate.

The host (src-tauri/src/treatment.rs) carries the same patterns and re-verifies every stored edge against them when it is
looked up, so a pack that stores a wrong edge cannot make the app show it. Keep the two files in step; the golden
(golden/G-TREATMENT) is run against both.

A cue is wording by which a Court treats an earlier decision (overruled, reversed, distinguished, followed, doubted, per
incuriam, referred to a larger Bench). A cue counts only if it survives two guards:
  - negation:  not/never/no/neither/nor/cannot/hardly/without in the 40 characters before the cue, or inside it;
  - argument:  wording of a submission or a question (submitted, contended, urged, argued, counsel, if, whether, should,
               ought, may, would, seeks, proposed...) anywhere before the cue in the same sentence.
Precision matters more than recall here: an edge that is missed shows as "no treatment found in the installed corpus"
(never as "good law"), while a wrong one would put an overruling on a case that was not overruled.
"""
from __future__ import annotations

import re

# (treatment, pattern_id, regex) in priority order; the first cue that survives the guards names the treatment.
CUES = [
    ("PER_INCURIAM", "per-incuriam.v2", re.compile(r"(?i)\bper\s+incuriam\b")),
    ("OVERRULED", "overruled.v2", re.compile(r"(?i)\b(?:is|are|stands?|be|been|was|were)\s+(?:(?:hereby|accordingly|expressly|thus)\s+)?overruled\b|\bwe\s+(?:hereby\s+)?overrule\b")),
    ("REFERRED_TO_LARGER_BENCH", "larger-bench.v2", re.compile(r"(?i)\b(?:refer|referred|referring|placed?)\b[^.]{0,80}\blarger\s+bench\b")),
    ("REVERSED", "reversed.v2", re.compile(r"(?i)\b(?:decision|judgment|order|view)\b.{0,120}\b(?:was|were|stands|has\s+been|have\s+been)\s+reversed\s+(?:in\s+appeal|on\s+appeal|by\s+(?:this\s+court|the\s+supreme\s+court|a\s+larger\s+bench|a\s+constitution\s+bench))")),
    ("DOUBTED", "doubted.v2", re.compile(r"(?i)\bdoubt(?:ed)?\s+the\s+correctness\b|\bcorrectness\s+of\b.{0,60}\bdoubted\b|\bwe\s+have\s+our\s+doubts\b|\b(?:has|have|had|is|are|was|were|been)\s+(?:(?:since|often|also|now|been)\s+)*doubted\b")),
    ("DISTINGUISHED", "distinguished.v2", re.compile(r"(?i)\bdistinguishable\b|\b(?:is|are|was|were|stands?|be|been)\s+(?:(?:clearly|therefore|thus|easily)\s+)?distinguished\b|\bdistinguish(?:ed|ing)\s+(?:the\s+)?(?:decision|judgment|case|ruling)\b|\bdistinguish(?:ed|ing)\s+on\s+(?:the\s+)?facts\b")),
    ("FOLLOWED", "followed.v2", re.compile(r"(?i)\b(?:follow|followed|following)\s+(?:the\s+)?(?:decision|judgment|view|ratio|law\s+laid\s+down|dictum|ruling)\b|\b(?:respectfully\s+)?agree\s+with\s+(?:the\s+)?(?:view|decision|judgment)\b")),
]
NEGATION = re.compile(r"(?i)\b(?:not|never|no|neither|nor|cannot|hardly|without)\b")
ARGUMENT = re.compile(r"(?i)\b(?:submit(?:s|ted)?|contend(?:s|ed)?|urge[ds]?|argu(?:e|es|ed)|plea|prayer|contention|counsel|if|whether|should|ought|may|would|seek(?:s|ing)?|propos(?:e|es|ed|al))\b")
NEGATION_WINDOW = 40


def guarded(sentence: str, m: re.Match) -> bool:
    """True if the cue is negated, or sits in a submission or a question."""
    return bool(NEGATION.search(sentence[max(0, m.start() - NEGATION_WINDOW): m.end()]) or ARGUMENT.search(sentence[: m.start()]))


def detect(sentence: str):
    """(treatment, pattern_id) of the first cue that survives the guards, else None."""
    for name, pid, rx in CUES:
        for m in rx.finditer(sentence):
            if not guarded(sentence, m):
                return name, pid
    return None


def verify(treatment: str, sentence: str) -> str | None:
    """None if `sentence` supports `treatment`; else CUE_NOT_FOUND or NEGATED_OR_ARGUED."""
    seen = False
    for name, _pid, rx in CUES:
        if name != treatment:
            continue
        for m in rx.finditer(sentence):
            seen = True
            if not guarded(sentence, m):
                return None
    return "NEGATED_OR_ARGUED" if seen else "CUE_NOT_FOUND"
