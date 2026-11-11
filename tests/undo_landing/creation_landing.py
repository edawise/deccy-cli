#!/usr/bin/env python3
"""Gate for E2 (suxnppvq): the creation-undo landing answers to its recorded prerequisites.

The subject is the decision node `nodes/xwoxtupt.json`. That node records one
consequence of six settled decisions: undoing a creation leaves the created
change gone, its node document gone with it, and the working copy standing at
the change the birth was placed after, carrying that change's own content —
reading `changed` when that change carries content, `empty` only when that
change is itself empty.

Nothing here is a choice of the subject's. Each element below is forced by a
recorded prerequisite, and the probe names which one:

  created change gone          prsoxsuo  undoing a creation removes the created node entire
                               qxmzpupt  a creation is undone by removing the created change
  its document gone            prsoxsuo  (the node removed is the node document)
                               xkolyytz  one jj change owns one node
  lands at the birth's         ytvvzlsp  a creation is inserted after a named position change
  position change              olmoqxzo  the position is the dependency furthest up, the unique head
                               xkolyytz  what the working copy shows is always some change's tree
  carries that change's        xkolyytz  the content a checkout displays is that change's own
  own content                  zuqzrlrx  the undo records no tree for the working copy; it steps back
  changed / empty shade        xkolyytz  the working copy is the change's tree, so its emptiness is that
                                         change's emptiness, not the undo's

The check has two layers and one guard.

(1) Substance. For each element, the subject's recorded text must carry an
    assertion of it. Assertions are read through phrase families, not exact
    wording: a rewritten sentence that still says the working copy stands at the
    change the birth was placed after satisfies the element, and so does any of
    its natural equivalents (the position change the birth named, the created
    change's parent, the change immediately before the created change in jj
    history). A legal rewording outside the families is reported as the subject
    not asserting the element — an under-recording — and never as asserting its
    opposite.

(2) Non-contradiction. For each element, assertions of its opposite are
    findings: a working copy landed on a fresh, newly created empty change; the
    created change kept, emptied or left standing; its document surviving; the
    working copy always reading empty; the landing re-derived at undo time from
    the graph as it stands; a working copy composed, assembled or copied from
    the created change's content. Each is a statement the six prerequisites
    forbid, so the finding is a contradiction with the record, not a taste
    judgment about the subject's prose.

(3) The guard, on the subject's own record: it must record no verdict (a
    red/green, pass/fail, "currently" or verdict word — evidence belongs in the
    session, not in a node), and it must not claim as its own commitment a
    mechanism its prerequisites leave elsewhere — an undo that reverts, records
    an op id, steps the position or writes state names that mechanism's owner.
    Both are checked against the subject's recorded text only.

What this check does not do: it does not judge whether the subject's reasoning
in `content.rationale` is sound (that field is read only for contradictions and
verdicts), it does not decide whether the six prerequisites themselves are
right, and it does not read any implementation, marker, journal or working-copy
state — the landing's behavior is checked where the undo's answer is, not here.
It is a license check over a recorded decision, run because a wrong or drifted
subject would leave every later decision built on this landing resting on text
the prerequisites never licensed.

The subject is already recorded, so the probe is applied to a record rather than
to an absent artifact: a finding here is a contradiction in the recorded
decision, or a prerequisite whose recorded premise has moved out from under the
expectation below — the latter reported as gate drift, asking for a re-derived
expectation rather than for a change to the subject.

Stdlib only: no cargo, no build, no network. Exit 0 when every element is
asserted and nothing contradicts it; exit 1 with one finding per failure.

Run from the repository root:

    python3 tests/undo_landing/creation_landing.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

SUBJECT = "nodes/xwoxtupt.json"

ROOT = Path(__file__).resolve().parents[2]

# The six prerequisites the subject records as its ground. Each entry carries a
# loose premise phrase this probe's expectation is transcribed from; the phrase
# is re-read from the prerequisite's own recorded text, so an amended
# prerequisite is reported as gate drift instead of as a defect of the subject.
PREREQUISITES = {
    "zuqzrlrx": r"steps the position one further back",
    "qxmzpupt": r"creation is undone by removing the created change",
    "prsoxsuo": r"undoing a creation removes the created node",
    "ytvvzlsp": r"inserted after a named position change",
    "olmoqxzo": r"dependency furthest up",
    "xkolyytz": r"one jj change owns one node",
}

BESIDE_CHANGE = (
    r"(?:the\s+)?change (?:the birth was placed after|beside the birth|it stands at)"
    r"|beside-change"
    r"|the position change"
    r"|a (?:named )?position change"
    r"|the change named as the position"
    r"|the created change's (?:parent|position)"
    r"|immediately (?:before|after) the created change"
)

ELEMENTS = [
    {
        "name": "the created change is gone",
        "forced_by": ["prsoxsuo", "qxmzpupt"],
        "asserting": [
            r"created change\s+(?:is\s+|stays\s+|remains\s+)?(?:gone|removed|deleted|discarded|no longer (?:stands|exists))",
            r"remov(?:es|ing|ed|al of)\s+the created change",
            r"(?:deletes|delete|dropped|drops|discards)\s+(?:the\s+)?created change",
            r"the created change is not kept",
            r"neither the created change nor[^.;:]*surviv",
            r"nothing of the created change",
            r"created change[^.;:]{0,60}(?:does not|doesn't|is not|no longer)\s+(?:surviv|kept|stand|exist)",
            r"created change and its document (?:are\s+|is\s+)?gone",
        ],
        "denying": [
            r"the created change (?:is|stays|remains)\s+(?:kept|retained|preserved|left behind)",
            r"the created change (?:survives|remains|stands)(?!\s+gone)",
            r"the created change (?:is|stays) left as (?:an )?empty change",
            r"created change (?:is|stays) (?:an )?empty change",
        ],
    },
    {
        "name": "its node document is gone with it",
        "forced_by": ["prsoxsuo", "xkolyytz"],
        "asserting": [
            r"(?:its\s+)?node document\s+(?:is\s+|are\s+)?(?:gone|removed|deleted|no longer (?:stands|exists|recorded))",
            r"neither the created change nor[^.;:]*document[^.;:]*surviv",
            r"created change and its document (?:are\s+|is\s+)?gone",
            r"document it owns[^.;:]*\b(?:gone|no longer)",
            r"(?:deletes|delete|discards|discard|drops|drop|removes|remove)\s+the created change and\s+(?:the\s+|its\s+)(?:node\s+)?document",
            r"document (?:it owns|of the created change)[^.;:]{0,40}\bgoes with",
            r"node document[^.;:]{0,20}\bgoes with",
            r"document (?:it owns|of the created change|the created change owns)[^.;:]{0,40}(?:does not|doesn't|is not|no longer)\s+(?:surviv|kept|exist|recorded)",
        ],
        "denying": [
            r"node document (?:is|stays|remains)\s+(?:kept|retained|preserved|left behind)",
            r"document\b[^.]{0,50}\b(?:is|stays|remains) (?:kept|retained|preserved|left behind)",
            r"document (?:it owns|of the created change) (?:survives|remains|stays|is kept)",
            r"document (?:survives|is kept) the undo",
        ],
    },
    {
        "name": "the working copy stands at the change the birth was placed after",
        "forced_by": ["ytvvzlsp", "olmoqxzo", "xkolyytz"],
        "asserting": [
            r"working copy[^.]{0,160}?(?:" + BESIDE_CHANGE + r")",
            r"(?:" + BESIDE_CHANGE + r")[^.]{0,120}?working copy",
            r"working copy[^.]{0,120}?\b(?:stands|landing|lands|is checked out|checked out|left|sits)\b[^.]{0,40}?\b(?:at|on) the position\b",
            r"working copy is (?:that|the) same change",
            r"checked out as that change",
        ],
        "denying": [
            r"working copy[^.]{0,80}\b(?:fresh|new|newly created|freshly created|unrelated)\b[^.]{0,40}\bchange",
            r"\bfresh\b[^.]{0,40}\bempty change\b",
            r"a (?:new|fresh|newly created) empty change",
            r"working copy (?:lands|stands|returns|is left)[^.]{0,30}(?:at|on|to) the created change",
            r"\bpicked at undo time\b",
            r"\b(?:re-?derived|recomputed|recalculated) from the (?:graph|dependency set)\b",
            r"\bchosen (?:by|at) (?:the )?undo\b",
        ],
    },
    {
        "name": "it carries that change's own content",
        "forced_by": ["xkolyytz", "zuqzrlrx"],
        "asserting": [
            r"(?:carrying|carries|shows|displays|holds)\s+(?:its|that change's|the change's|the) own content",
            r"(?:that|the|its) change's own content",
            r"content (?:it|the working copy) (?:displays|shows|holds) is (?:its|that change's|the change's) own",
            r"checked out as that change",
        ],
        "denying": [
            r"(?:composed|assembled|synthesi[sz]ed|reconstructed|copied|merged) (?:in)?to the working copy",
            r"(?:working copy|a tree|the tree)[^.]{0,60}\b(?:composed|assembled|synthesi[sz]ed|reconstructed)\b",
            r"a copy of the created change",
            r"restor(?:es|ing|ed)\s+(?:the\s+)?created change's content",
        ],
    },
    {
        "name": "standing at a non-empty beside-change reads `changed`",
        "forced_by": ["xkolyytz", "zuqzrlrx"],
        "asserting": [
            r"reads?\s*`?changed`?,?\s*not\s+`?empty",
            r"reads?\s*`?changed`?\s*(?:unless|whenever|when|if)\b[^.]{0,80}empty",
            r"`?changed`?[^.]{0,80}(?:non-empty|carries content|is itself non-empty)",
            r"(?:non-empty|carries content of its own)[^.]{0,80}`?changed`?",
        ],
        "denying": [],
    },
    {
        "name": "the `empty` shape holds only where the beside-change is itself empty",
        "forced_by": ["xkolyytz", "zuqzrlrx"],
        "asserting": [
            r"`?empty`?\s+(?:shape|reading)\s+(?:holds|applies|is)\b[^.]{0,80}empty",
            r"`?empty`?\s+(?:shape\s+)?(?:only\s+)?(?:when|where|if)\b[^.]{0,80}(?:" + BESIDE_CHANGE + r")[^.]{0,40}empty",
            r"only where[^.]{0,80}empty",
        ],
        "denying": [
            r"working copy (?:always|invariably|necessarily)\s+(?:reads|is|stays|returns|is left)\s+`?empty",
            r"(?:undo|the undo)\s+(?:always\s+|inevitably\s+)?(?:leaves|empties|leaves behind)\s+the working copy\b[^.]{0,20}`?empty",
            r"the working copy is (?:left )?empty (?:after|following|once|whenever|in every case)",
            r"reads `?empty`? (?:regardless|always|every time)",
        ],
    },
]

VERDICT = re.compile(
    r"\b(?:red|green|verdict|currently)\b|\b(?:passes|pass|fails?|failing)\b",
    re.IGNORECASE,
)

OWN_VOICE_MECHANISM = re.compile(
    r"\b(?:undo|redo)\s+(?:itself\s+)?(?:reverts|records|applies|writes|composes|assembles|creates|empties|steps)\b",
    re.IGNORECASE,
)

CITATION = re.compile(r"`[0-9a-z]{8,}`|\b[SDM]\d+[a-z]?\b|\b[SDM]-[A-Za-z]")

CLAUSE_BREAK = re.compile(r"(?:[;:]|,\s*(?:and|but|so|then)\b)")
# An assertion is read on its own clause: a negator that governs an earlier, comma-
# separated item does not reach across the comma into a new claim. A denial is read
# more generously, because an enumeration inherits its governing negation
# (`never a copy, an ad-hoc tree`), so the loose break list is kept for denials.
ASSERTION_BREAK = re.compile(r"(?:[;:]|,|\band\b|\bbut\b|\bso\b|\bthen\b)")
NEGATORS = re.compile(r"\b(?:not|never|no|none|nothing|neither|nor|without|cannot|n't)\b", re.IGNORECASE)


failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)
    print(f"FAIL: {message}")


def sentences(text: str) -> list[str]:
    parts = re.split(r"(?<=[.!?])\s+|\n\s*\n", text)
    return [part for part in parts if part.strip()]


def negated_before(sentence: str, position: int, breaks: re.Pattern[str] = CLAUSE_BREAK) -> bool:
    """True when the clause the match opens in already carries a negator."""
    head = sentence[:position]
    last = None
    for match in breaks.finditer(head):
        last = match
    clause = head[last.end():] if last else head
    return bool(NEGATORS.search(clause))


def load_document() -> dict | None:
    location = ROOT / SUBJECT
    if not location.is_file():
        fail(f"{SUBJECT}: missing; the decision this gate evaluates is not recorded")
        return None
    try:
        document = json.loads(location.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"{SUBJECT}: not valid JSON ({error}); the recorded landing cannot be read")
        return None
    if not isinstance(document, dict):
        fail(f"{SUBJECT}: not a node document")
        return None
    return document


def check_shape(document: dict) -> str | None:
    if document.get("kind") != "decision":
        fail(f"{SUBJECT}: kind is {document.get('kind')!r}, not 'decision'")
    content = document.get("content")
    if not isinstance(content, dict):
        fail(f"{SUBJECT}: content is not an object")
        return None
    recorded = content.get("content")
    if not isinstance(recorded, str) or not recorded.strip():
        fail(f"{SUBJECT}: content.content is not recorded prose; the landing is not stated")
        return None
    dependencies = document.get("depends_on") or []
    for prerequisite in PREREQUISITES:
        if prerequisite not in dependencies:
            fail(
                f"{SUBJECT}: {prerequisite} is no longer a recorded prerequisite — "
                "the edge this gate covers is not drawn by the subject"
            )
    extra = [node for node in dependencies if node not in PREREQUISITES]
    if extra:
        print(f"note: {SUBJECT} also depends on {', '.join(extra)}; this gate reads the six it covers")
    return recorded


def check_premises() -> None:
    for prerequisite, premise in PREREQUISITES.items():
        location = ROOT / "nodes" / f"{prerequisite}.json"
        if not location.is_file():
            fail(f"nodes/{prerequisite}.json: missing; this gate's expectation is transcribed from it")
            continue
        try:
            document = json.loads(location.read_text(encoding="utf-8"))
        except json.JSONDecodeError as error:
            fail(f"nodes/{prerequisite}.json: not valid JSON ({error}); the premise cannot be re-read")
            continue
        content = document.get("content") or {}
        recorded = content.get("content") if isinstance(content, dict) else None
        if not isinstance(recorded, str) or not re.search(premise, recorded, re.IGNORECASE):
            fail(
                f"nodes/{prerequisite}.json: no longer states /{premise}/ — gate drift: "
                "the expectation below was transcribed from that premise and must be re-derived"
            )


def check_element(element: dict, recorded: str, evidence: str) -> None:
    asserted = False
    for pattern in element["asserting"]:
        compiled = re.compile(pattern, re.IGNORECASE)
        for sentence in sentences(recorded):
            for match in compiled.finditer(sentence):
                if not negated_before(sentence, match.start(), ASSERTION_BREAK):
                    asserted = True
                    break
            if asserted:
                break
        if asserted:
            break
    if not asserted:
        fail(
            f"{SUBJECT}: the recorded landing does not state that {element['name']} "
            f"(forced by {', '.join(element['forced_by'])})"
        )

    for pattern in element["denying"]:
        compiled = re.compile(pattern, re.IGNORECASE)
        for sentence in sentences(evidence):
            for match in compiled.finditer(sentence):
                if negated_before(sentence, match.start()):
                    continue
                fail(
                    f"{SUBJECT}: the recorded landing contradicts {', '.join(element['forced_by'])}: "
                    f"{element['name']} is denied by the sentence {sentence.strip()[:220]!r}"
                )


def check_guard(recorded: str, evidence: str) -> None:
    for sentence in sentences(evidence):
        for match in VERDICT.finditer(sentence):
            fail(
                f"{SUBJECT}: records a verdict ({match.group(0)!r}) — evidence belongs in the "
                f"session, not in the node: {sentence.strip()[:220]!r}"
            )
    for sentence in sentences(recorded):
        for match in OWN_VOICE_MECHANISM.finditer(sentence):
            if CITATION.search(sentence):
                continue
            fail(
                f"{SUBJECT}: claims an undo mechanism in its own voice ({match.group(0)!r}) "
                f"without naming the node that records it: {sentence.strip()[:220]!r}"
            )


def main() -> int:
    document = load_document()
    recorded = check_shape(document) if document else None

    if recorded is not None:
        content = document["content"]
        rationale = content.get("rationale") if isinstance(content.get("rationale"), str) else ""
        evidence = recorded + "\n\n" + rationale

        check_premises()
        for element in ELEMENTS:
            check_element(element, recorded, evidence)
        check_guard(recorded, evidence)

    if failures:
        print(f"\n{len(failures)} creation-landing check(s) failed")
        return 1

    print(
        f"{SUBJECT}: the recorded landing — created change and its document gone, the working copy "
        "at the change the birth was placed after, that change's own content, the changed/empty "
        "shade read off it — contradicts none of its six prerequisites and records no verdict"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
