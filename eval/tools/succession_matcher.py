#!/usr/bin/env python3
"""Deterministic succession-candidate matcher (B-S4-09 data side).

Given the sections of an old code and a new code (official texts), proposes STATUTE_SUCCESSION MAPPING candidates:
IDENTICAL / RENUMBERED / MODIFIED / NEW / OMITTED, plus the subsection of the new section that carries the old text.
It compares word 4-gram "shingles" of the operative text after neutralising section numbers and code names, so a
renumbered cross-reference does not count as a change. Containment (how much of the old text survives inside the new
section) handles consolidation (several old sections -> one new section) and splits.

Every candidate is DEVELOPMENT_ONLY: it is a proposal for a named legal editor, never a legal answer. Pairs in the
grey zone are not emitted as rows at all; they go to the editor worksheet only.
"""
from __future__ import annotations

import re
from dataclasses import dataclass, field

# Thresholds (tunable, reported in the worksheet).
SAME_TEXT = 0.95      # both-way containment at or above this = same text
MODIFIED_MIN = 0.60   # old text surviving in the new section at or above this = MODIFIED candidate
ABSENT_MAX = 0.15     # below this in both directions (and no heading match) = OMITTED / NEW candidate
SHINGLE = 4

_NUM = re.compile(r"\b\d+[A-Z]{0,3}\b|\(\d+[a-z]?\)|\([a-z]{1,3}\)|\([ivxl]+\)")
_WORD = re.compile(r"[a-z]+")
_CODE_NAMES = re.compile(
    r"indian penal code|bharatiya nyaya sanhita|code of criminal procedure|bharatiya nagarik suraksha sanhita|"
    r"indian evidence act|bharatiya sakshya adhiniyam|this code|this sanhita|this adhiniyam|this act",
    re.I,
)
_SUBSECTION = re.compile(r"(?m)(?:^|(?<=—)|(?<=[.:;]\s))\((\d+)\)\s")


@dataclass
class Section:
    code: str            # e.g. "IPC"
    number: str          # e.g. "302", "124A"
    heading: str         # marginal heading
    text: str            # exact section text (as stored in the pack chunk)
    chunk_id: str = ""
    source_id: str = ""
    _shingles: set = field(default=None, repr=False)

    def shingles(self) -> set:
        if self._shingles is None:
            self._shingles = shingles(self.text)
        return self._shingles


def normalise(text: str) -> list[str]:
    t = _CODE_NAMES.sub(" codeword ", text)
    t = _NUM.sub(" num ", t)
    return _WORD.findall(t.lower())


def shingles(text: str) -> set:
    w = normalise(text)
    if len(w) < SHINGLE:
        return {" ".join(w)} if w else set()
    return {" ".join(w[i:i + SHINGLE]) for i in range(len(w) - SHINGLE + 1)}


def containment(a: set, b: set) -> float:
    """Share of a's shingles found in b."""
    return len(a & b) / len(a) if a else 0.0


def heading_sim(a: str, b: str) -> float:
    wa, wb = set(normalise(a)), set(normalise(b))
    return len(wa & wb) / len(wa | wb) if wa and wb else 0.0


def subsections(text: str) -> list[tuple[str, int, int]]:
    """(label, char_start, char_end) of numbered subsections "(1)", "(2)" … inside a section text."""
    marks = [(m.group(1), m.start()) for m in _SUBSECTION.finditer(text)]
    out = []
    for i, (label, start) in enumerate(marks):
        end = marks[i + 1][1] if i + 1 < len(marks) else len(text)
        out.append((label, start, end))
    return out


@dataclass
class Candidate:
    equivalence: str               # IDENTICAL | RENUMBERED | MODIFIED | NEW | OMITTED | REVIEW
    old: Section | None
    new: Section | None
    new_provision: str | None      # e.g. "318(4)" when one subsection carries the old text
    sub_range: tuple[int, int] | None
    score_old_in_new: float
    score_new_in_old: float
    heading: float
    note: str = ""


def match(old: list[Section], new: list[Section]) -> list[Candidate]:
    out: list[Candidate] = []
    matched_new: set[str] = set()
    for o in old:
        so = o.shingles()
        if not so:
            continue
        best = max(new, key=lambda n: (containment(so, n.shingles()), heading_sim(o.heading, n.heading)))
        c_on = containment(so, best.shingles())
        c_no = containment(best.shingles(), so)
        h = heading_sim(o.heading, best.heading)
        if c_on >= SAME_TEXT and c_no >= SAME_TEXT:
            eq = "IDENTICAL" if o.number == best.number else "RENUMBERED"
            out.append(Candidate(eq, o, best, best.number, None, c_on, c_no, h))
            matched_new.add(best.number)
        elif c_on >= MODIFIED_MIN:
            prov, rng, eq = best.number, None, "MODIFIED"
            subs = subsections(best.text)
            if len(subs) > 1:
                # Body of the old section without its number/heading, compared with each subsection body.
                body = shingles(re.sub(r"^\s*\d+[A-Z]{0,3}\.\s*[^—]{0,200}—", "", o.text, count=1))
                sub_body = lambda s, e: shingles(re.sub(r"^\(\d+\)\s*", "", best.text[s:e]))  # noqa: E731
                scored = [(containment(body, sub_body(s, e)), lab, s, e) for lab, s, e in subs]
                top = max(scored)
                if top[0] >= MODIFIED_MIN:
                    prov, rng = f"{best.number}({top[1]})", (top[2], top[3])
                    if top[0] >= SAME_TEXT and containment(sub_body(top[2], top[3]), body) >= SAME_TEXT:
                        eq = "RENUMBERED"
            out.append(Candidate(eq, o, best, prov, rng, c_on, c_no, h))
            matched_new.add(best.number)
        elif c_on < ABSENT_MAX and h < 0.5:
            out.append(Candidate("OMITTED", o, None, None, None, c_on, c_no, h, note=f"closest new section {best.code} {best.number} ({c_on:.2f})"))
        else:
            out.append(Candidate("REVIEW", o, best, best.number, None, c_on, c_no, h, note="grey zone: editor decides"))
    for n in new:
        if n.number in matched_new:
            continue
        sn = n.shingles()
        if not sn:
            continue
        best = max(old, key=lambda o: containment(sn, o.shingles()))
        c = containment(sn, best.shingles())
        if c < ABSENT_MAX and heading_sim(n.heading, best.heading) < 0.5:
            out.append(Candidate("NEW", None, n, n.number, None, 0.0, c, 0.0, note=f"closest old section {best.code} {best.number} ({c:.2f})"))
    return out


if __name__ == "__main__":
    # Self-check on synthetic text (no legal content).
    a = [Section("OLD", "10", "Theft", "10. Theft.—Whoever dishonestly takes movable property out of the possession of any person without consent commits theft."),
         Section("OLD", "11", "Old rule", "11. Old rule.—A rule that the new code drops entirely about the keeping of certain archaic registers.")]
    b = [Section("NEW", "20", "Theft", "20. Theft.—(1) Whoever dishonestly takes movable property out of the possession of any person without consent commits theft. (2) A further new limb about repeat offenders and enhanced punishment."),
         Section("NEW", "21", "Snatching", "21. Snatching.—Whoever suddenly seizes property from a person commits snatching, a new offence.")]
    for c in match(a, b):
        print(c.equivalence, c.old and c.old.number, "->", c.new_provision, round(c.score_old_in_new, 2), c.note)
