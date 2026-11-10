#!/usr/bin/env python3
"""Fidelity probe for E-I1a-set (rsnnqmpw): the packaged file set answers to xrvrslzq.

Three layers, each grounded in something recorded:

  A. the shipped set    prose (xrvrslzq) <-> archive (`cargo package --list`)
  B. the mechanism      subject (trolstmt, via its ref Cargo.toml) <-> what the
                        subject itself records about how it selects files
  C. the expectation    this probe's expected set <-> the direction xrvrslzq's
                        recorded text asserts for each of its paths

A. The packaged-file-set decision records a *closed* set: the crate ships exactly
the paths it names and no other path ships. Whatever mechanism the manifest uses,
the set cargo actually packages must be that set, in both directions:

  necessity / not looser   a path the prose names must appear in the listing: a
                           named file, and *every* repository file under a named
                           directory, because the prose names every file under
                           `src/` and `schemas/`. Shipping one file of a named
                           directory is not shipping the group;
  fidelity  / not stricter every listed path must be one the prose names or sit
                           under a directory the prose names, and nothing under a
                           group the prose excludes may appear.

The observed set comes from `cargo package --list --allow-dirty` over this crate:
the archive cargo would build, never a restatement of the manifest. The record
fixes the groups; the crate's own tree under those groups and the listing are the
observation, so a named group is compared file by file rather than sampled.

B. The dependent's own recorded rationale commits to a mechanism, not only to an
observable: it implements the packaged-file-set decision "with an explicit cargo
`include` allowlist ... An allowlist is what makes the shipped set closed: new
development material cannot join the package by default, and widening the set
takes an edit to this manifest". A fail-open rule that happens to ship the same
paths today satisfies A while abandoning that commitment, and no archive listing
can tell the two apart. So the manifest is read for the mechanism, and only one
question is asked of it: is there an allowlist at all? A literal `[package]
include` array of path strings, or `[package] include.workspace = true` that
resolves to one in `[workspace.package]`, is that allowlist. If one is present,
cargo selects with it and any `[package] exclude` alongside it is inert, so the
shape reports nothing about it beyond a note. If no allowlist is present, the set
is decided by exclusion and by cargo's defaults, which is a different mechanism,
and that is the finding. The contents of the allowlist are never read: expected
membership comes from the prerequisite's prose (layer A), never from the manifest,
so A cannot be made green by construction.

C. Expected-set provenance: the constants below are a transcription of
nodes/xrvrslzq.json's recorded inline text and nothing else. Re-reading the node
must confirm that the text still asserts the same direction for every path this
probe expects. The guard is clause-scoped and negation-aware, because a path token
alone says nothing about what the prose claims for it:

  * a sentence is split into clauses at `:`, `;`, and at a comma that opens a new
    clause; a membership predicate in one clause is never attributed to a path
    named only in another (a sentence saying `nodes/` is not part of the crate and
    going on to say what a CLI ships claims nothing about `nodes/`);
  * in a clause naming a path, an inclusion predicate (ship, travel, carried,
    included, part of) is inclusion, the same predicate behind a negator (`not
    carried`, `never ships`, `is not part of`) is exclusion, an exclusion
    predicate (`excluded from`, `stays out`, `out of the archive`) is exclusion,
    and the same predicate behind a negator (`is not excluded from`, `never
    dropped from`) is inclusion — a denial of an exclusion states inclusion, just
    as a denial of an inclusion states exclusion;
  * the negator is looked for anywhere between the clause's start and the
    predicate, not within a fixed distance of it, so `is not, under any
    circumstances whatsoever, carried in the crate` reads as a denial and so does
    `no file under `src/` ships`;
  * a clause in an enumeration with no predicate of its own inherits the nearest
    governing predicate before it in the same sentence, so `every file under
    `src/`` reads as the `it ships exactly` list it belongs to — but a member
    that carries its own predicate uses that predicate and lends nothing to its
    neighbours, so one denied member (`no file under `src/` ships`) does not turn
    the rest of the list into denials;
  * a relative clause is its own clause: in `the models, which ship`, the
    predicate qualifies the models, so the path heading the sentence is not read
    as shipping and stays undecided in that sentence;
  * a clause that yields no predicate for a named path, or two conflicting ones,
    is reported as the recorded text not deciding that path. It is never reported
    as the text asserting a direction it does not state.

A path this probe expects to ship must be asserted in that direction by at least
one clause and contradicted by none; a path it expects to exclude must be asserted
as excluded and contradicted by none. Rewording inside a direction (a different
shipping phrase, a reordered or merged sentence) leaves the clause polarity
unchanged, while a reversal — `specs/` rewritten to ship, or a required inclusion
denied — flips it even though every path token is still present. The vocabulary
is finite and the guard fails closed: membership expressed in words outside it is
reported as not decidable rather than silently passed. The guard judges the
direction asserted for the paths the recorded set names; it does not audit prose
about anything else, and it does not judge whether the stated reasons are sound.

Cargo's own pack step adds bookkeeping files to the archive that the manifest's
file selection neither adds nor removes. They are outside the set the manifest
decides, so they are not judged as shipped members; the probe names them in its
output as a note instead of hiding them.

Stdlib + cargo only. No build, no network preferred (`--offline`, with one retry
without it if cargo itself cannot run). Exit 0 when the shipped set is exactly the
recorded set, the selection mechanism is an allowlist, and the recorded text
asserts the recorded direction; exit 1 with a `FAIL:` finding per offending path,
clause or mechanism.

Usage: python3 tests/package_set/package_set.py
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

SET_NODE = Path("nodes") / "xrvrslzq.json"
MANIFEST = Path("Cargo.toml")

# Transcribed from the recorded text of xrvrslzq: "it ships exactly `Cargo.toml`;
# `Cargo.lock`, cargo-generated and carried with the build; every file under
# `src/`; every file under `schemas/`; `README.md`; `LICENSE-MIT`; and
# `LICENSE-APACHE`."
SHIPPED_FILES = (
    "Cargo.toml",
    "Cargo.lock",
    "README.md",
    "LICENSE-MIT",
    "LICENSE-APACHE",
)
SHIPPED_DIRS = ("src/", "schemas/")

# Transcribed from the recorded text of xrvrslzq: "`nodes/` does not ship [...]
# `specs/` does not ship [...] `.github/` does not ship [...] `dist-workspace.toml`
# does not ship [...] `tests/` does not ship".
EXCLUDED_DIRS = ("nodes/", "specs/", ".github/", "tests/")
EXCLUDED_FILES = ("dist-workspace.toml",)

# Produced by cargo's pack step for every crate, whether or not the manifest
# names them; the file selection can neither add nor drop them. Reported, not
# judged.
CARGO_PACK_TIME = (".cargo_vcs_info.json", "Cargo.toml.orig")

# Membership predicates, i.e. the words a clause uses to say whether a path is
# part of the package. Negating one of the inclusion predicates states exclusion.
INCLUSION_PREDICATES = (
    "ship",
    "ships",
    "shipped",
    "shipping",
    "travel",
    "travels",
    "travelled",
    "traveled",
    "travelling",
    "traveling",
    "carry",
    "carries",
    "carried",
    "include",
    "includes",
    "included",
    "part of",
    "belongs to",
    "is in the package",
    "are in the package",
)
EXCLUSION_PREDICATES = (
    "excluded from",
    "is excluded",
    "are excluded",
    "excluded",
    "dropped from",
    "dropped",
    "omitted from",
    "omitted",
    "stays out",
    "stay out",
    "stays behind",
    "stay behind",
    "left out",
    "left behind",
    "kept out",
    "kept in the repository",
    "stays in the repository",
    "out of the archive",
    "out of the package",
)
NEGATORS = ("not", "never", "no", "neither", "nor", "without", "cannot")

# Every path token this probe's expectation names. Used only to tell a clause
# that describes its own path (predicate after the path) from the clause that
# opens an enumeration (predicate before the path).
KNOWN_PATHS = (
    SHIPPED_FILES + SHIPPED_DIRS + EXCLUDED_FILES + EXCLUDED_DIRS
)

# Words that open a new clause after a comma. Splitting there keeps a predicate
# that governs a later noun from being attributed to a path named earlier.
CLAUSE_OPENERS = (
    "and",
    "but",
    "while",
    "whereas",
    "yet",
    "so",
    "since",
    "because",
    "although",
    "though",
    "it",
    "they",
    # relative clauses: a predicate inside one belongs to the relative
    # clause's own subject, not to a path named before the comma.
    "which",
    "whose",
    "who",
    "that",
    "where",
)
COMMA_CLAUSE = re.compile(
    r",\s+(?=(?:" + "|".join(CLAUSE_OPENERS) + r")\b)", re.IGNORECASE
)


def _alternation(words: tuple[str, ...]) -> re.Pattern[str]:
    ordered = sorted(words, key=len, reverse=True)
    return re.compile(
        r"\b(?:" + "|".join(re.escape(w) for w in ordered) + r")\b", re.IGNORECASE
    )


INCLUSION = _alternation(INCLUSION_PREDICATES)
EXCLUSION = _alternation(EXCLUSION_PREDICATES)


def fail(message: str) -> None:
    print(f"FAIL: {message}")


def under(path: str, directory: str) -> bool:
    return path.startswith(directory) and path != directory.rstrip("/")


# --- A. the shipped set -------------------------------------------------------


def evaluate(listing: list[str], tree: list[str]) -> list[str]:
    """Judge one observed listing against the recorded closed set.

    Pure: takes the observed paths and the crate's own repository files under the
    directories the record names, returns findings. Empty findings mean the shipped
    set is exactly the recorded set. The record fixes the groups (`every file under
    `src/``); the tree and the listing are the observation.
    """
    findings: list[str] = []
    observed = sorted({line.strip() for line in listing if line.strip()})
    members = [p for p in observed if p not in CARGO_PACK_TIME]
    present = set(tree)

    named: set[str] = set()
    for path in SHIPPED_FILES:
        if path not in members:
            findings.append(
                f"named path {path!r} does not ship "
                "(it is absent from `cargo package --list`)"
            )
        else:
            named.add(path)
    for directory in SHIPPED_DIRS:
        expected = sorted(p for p in present if under(p, directory))
        if not expected:
            if not any(under(p, directory) for p in members):
                findings.append(
                    f"no file under {directory!r} ships, though the recorded set "
                    "names every file under it"
                )
            named.update(p for p in members if under(p, directory))
            continue
        for path in expected:
            if path in members:
                named.add(path)
            else:
                findings.append(
                    f"file {path!r} does not ship, though the recorded set names "
                    f"every file under {directory!r}"
                )

    for path in members:
        if path in named:
            continue
        excluded_by = next((d for d in EXCLUDED_DIRS if under(path, d)), None)
        if excluded_by is None:
            excluded_by = next((f for f in EXCLUDED_FILES if path == f), None)
        if excluded_by is not None:
            findings.append(
                f"excluded path {path!r} ships, though the recorded set excludes "
                f"{excluded_by!r}"
            )
        else:
            findings.append(
                f"unnamed path {path!r} ships; the recorded set is closed and names "
                "no such path"
            )
    return findings


def pack_time_note(listing: list[str]) -> str | None:
    """Name the files cargo's pack step adds, so they are visible, not hidden."""
    observed = {line.strip() for line in listing if line.strip()}
    pack_time = [p for p in sorted(observed) if p in CARGO_PACK_TIME]
    if not pack_time:
        return None
    return (
        "cargo's pack step added "
        + ", ".join(repr(p) for p in pack_time)
        + " (generated by cargo regardless of the manifest's file selection)"
    )


# --- B. the mechanism ---------------------------------------------------------


def resolve_allowlist(
    include: object, workspace: object
) -> tuple[list[str] | None, str | None, str | None]:
    """Resolve `[package] include`, following `workspace = true`.

    Returns (allowlist, finding, note). `allowlist` is the literal list of path
    patterns the dependency selected by, or None when there is none. Pure.
    """
    workspace_include: object = None
    workspace_package = (
        workspace.get("package") if isinstance(workspace, dict) else None
    )
    if isinstance(workspace_package, dict):
        workspace_include = workspace_package.get("include")

    if isinstance(include, dict):
        if include.get("workspace") is not True:
            return (
                None,
                "[package] `include` is not a literal array of path strings and does "
                "not inherit one from the workspace; the manifest declares no "
                "allowlist this probe can read",
                None,
            )
        if isinstance(workspace_include, list) and workspace_include:
            return list(workspace_include), None, (
                "[package] `include.workspace = true` resolves to "
                "`[workspace.package] include`; that inherited list is the allowlist"
            )
        return (
            None,
            "[package] `include` inherits from the workspace with "
            "`include.workspace = true`, but `[workspace.package]` declares no "
            "non-empty literal `include`, so the package selects with no allowlist",
            None,
        )

    if include is None:
        return (
            None,
            "the manifest selects packaged files with no `include` allowlist, so the "
            "shipped set is decided by exclusion and by cargo's defaults; the "
            "mechanism the dependent records is a closed allowlist, which fails shut",
            None,
        )

    if not isinstance(include, list):
        return (
            None,
            "[package] `include` is not a literal array of path strings, so the "
            "allowlist is not a closed list carried by this manifest",
            None,
        )
    if not include:
        return (
            None,
            "[package] `include` is an empty array, so it names no path and the "
            "allowlist is not a closed list of the named set",
            None,
        )
    if not all(isinstance(entry, str) and entry.strip() for entry in include):
        return (
            None,
            "[package] `include` carries a non-string or empty entry; the allowlist "
            "is not a literal list of path strings",
            None,
        )
    return list(include), None, None


def check_mechanism(manifest: dict) -> tuple[list[str], list[str]]:
    """Whether the manifest selects packaged files with an allowlist at all.

    Pure: takes the parsed manifest, returns (findings, notes). Only presence and
    resolvability of the allowlist is judged; its contents are deliberately not
    read, because expected membership comes from the prerequisite's prose (layer
    A), never from the manifest. A `[package] exclude` alongside a present
    allowlist is inert in cargo, so it is reported as a note, not a finding.
    """
    package = manifest.get("package")
    if not isinstance(package, dict):
        return ["Cargo.toml has no [package] table; no file selection is declared"], []

    allowlist, finding, note = resolve_allowlist(
        package.get("include"), manifest.get("workspace")
    )
    findings: list[str] = []
    notes: list[str] = [note] if note else []
    if finding is not None:
        findings.append(finding)
    elif "exclude" in package:
        notes.append(
            "the manifest also carries [package] `exclude`, which cargo ignores while "
            "an `include` allowlist is set; the allowlist decides the shipped set"
        )
    return findings, notes


# --- C. the expectation -------------------------------------------------------


def sentences(text: str) -> list[str]:
    """Split prose into sentences without inventing structure it does not have."""
    out: list[str] = []
    for paragraph in re.split(r"\n\s*\n", text):
        for sentence in re.split(r"(?<=[.!?])\s+", paragraph):
            if sentence.strip():
                out.append(sentence.strip())
    return out


def clauses(sentence: str) -> list[str]:
    """Split one sentence into clauses: `:`, `;`, and clause-opening commas."""
    out: list[str] = []
    for part in re.split(r"[:;]", sentence):
        out.extend(COMMA_CLAUSE.split(part))
    return [c.strip() for c in out if c.strip()]


def negated(clause: str, at: int) -> bool:
    """Whether a negator sits between the clause's start and the predicate at `at`.

    The window is the whole clause prefix, not a fixed number of words: a denial
    can carry any amount of intervening material (`is not, under any
    circumstances whatsoever, carried`) and a negation can precede the path it
    denies (`no file under `src/` ships`).
    """
    words = [w.lower() for w in re.findall(r"[A-Za-z']+", clause[:at])]
    if any("n't" in word for word in words):
        return True
    return any(word in NEGATORS for word in words)


def predicates(clause: str) -> list[tuple[int, str]]:
    """Every membership predicate in a clause, with the direction it carries.

    A negated inclusion is exclusion and a negated exclusion is inclusion: a
    denial states the opposite side of the predicate it denies.
    """
    out: list[tuple[int, str]] = []
    for match in INCLUSION.finditer(clause):
        neg = negated(clause, match.start())
        out.append((match.start(), "exclusion" if neg else "inclusion"))
    for match in EXCLUSION.finditer(clause):
        neg = negated(clause, match.start())
        out.append((match.start(), "inclusion" if neg else "exclusion"))
    return sorted(out)


def polarity(clause: str) -> tuple[str | None, bool]:
    """What a clause's own membership predicates say.

    Returns (side, conflict): side is "inclusion", "exclusion" or None, and
    conflict is True when the clause yields two different directions.
    """
    sides = {side for _, side in predicates(clause)}
    if not sides:
        return None, False
    if len(sides) > 1:
        return None, True
    return sides.pop(), False


def first_path(clause: str) -> int | None:
    """Where a known path token first appears in a clause, if it appears at all."""
    positions = [clause.find(p) for p in KNOWN_PATHS]
    positions = [p for p in positions if p >= 0]
    return min(positions) if positions else None


def governing(clause_list: list[str], index: int) -> str | None:
    """The direction the nearest governing predicate earlier in the sentence gives.

    An enumerated member of a list (`every file under `src/``) carries no predicate
    of its own; the clause that opens the list (`it ships exactly `Cargo.toml``)
    does, its predicate standing before the path it governs. A clause whose
    predicate follows the path it names describes that path alone (`no file under
    `src/` ships`) and lends its direction to nothing, so a single denied member
    cannot turn its neighbours into denials.
    """
    for earlier in reversed(clause_list[:index]):
        side, conflict = polarity(earlier)
        if conflict:
            return None
        if side is None:
            continue
        own_path = first_path(earlier)
        opening_predicate = predicates(earlier)[0][0]
        if own_path is None or opening_predicate < own_path:
            return side
    return None


def check_prose(text: str) -> list[str]:
    """The expectation must be the direction the recorded text asserts.

    Pure: takes the recorded text, returns findings. Empty findings mean every
    path this probe expects is asserted in that direction and contradicted in
    none.
    """
    findings: list[str] = []
    units = sentences(text)

    expectations: list[tuple[str, str]] = []
    expectations += [(p, "inclusion") for p in SHIPPED_FILES + SHIPPED_DIRS]
    expectations += [(p, "exclusion") for p in EXCLUDED_FILES + EXCLUDED_DIRS]

    for path, expected in expectations:
        naming = [u for u in units if path in u]
        if not naming:
            findings.append(
                f"this probe expects {path!r} but the recorded text does not name it; "
                "the expected set has drifted from the recorded text"
            )
            continue

        asserted: list[str] = []
        contrary: list[tuple[str, str]] = []
        undecided: list[str] = []
        conflicting: list[str] = []
        for sentence in naming:
            clause_list = clauses(sentence)
            for index, clause in enumerate(clause_list):
                if path not in clause:
                    continue
                side, conflict = polarity(clause)
                if conflict:
                    conflicting.append(clause)
                    continue
                if side is None:
                    side = governing(clause_list, index)
                if side is None:
                    undecided.append(clause)
                elif side == expected:
                    asserted.append(clause)
                else:
                    contrary.append((clause, side))

        want = "to ship" if expected == "inclusion" else "not to ship"
        for clause in conflicting:
            findings.append(
                f"the recorded text does not decide {path!r} (this probe expects it "
                f"{want}): the clause {clause!r} carries both directions"
            )
        for clause, side in contrary:
            findings.append(
                f"contradiction for {path!r} (this probe expects it {want}): the "
                f"recorded clause {clause!r} carries the {side} direction"
            )
        if conflicting or contrary or asserted:
            continue
        detail = undecided[0] if undecided else naming[0]
        findings.append(
            f"the recorded text does not decide {path!r} (this probe expects it "
            f"{want}): {detail!r}"
        )
    return findings


def read_prose(root: Path) -> tuple[str | None, list[str]]:
    node_path = root / SET_NODE
    if not node_path.is_file():
        return None, [
            f"{SET_NODE} is missing; the packaged-file-set decision is not recorded"
        ]
    try:
        node = json.loads(node_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        return None, [f"{SET_NODE} is not valid JSON ({exc})"]
    if node.get("kind") != "decision":
        return None, [f"{SET_NODE} is not a decision node (kind is {node.get('kind')!r})"]
    content = node.get("content") or {}
    if content.get("ref") != "inline":
        return None, [
            f"{SET_NODE} carries no inline text to check the shipped set against"
        ]
    text = content.get("content")
    if not isinstance(text, str) or not text.strip():
        return None, [f"{SET_NODE} records no inline text naming a shipped set"]
    return text, []


# --- observation -------------------------------------------------------------


def read_manifest(root: Path) -> tuple[dict | None, list[str]]:
    if sys.version_info < (3, 11):
        return None, ["python3 >= 3.11 is required to read TOML (tomllib)"]
    import tomllib

    manifest_path = root / MANIFEST
    if not manifest_path.is_file():
        return None, ["Cargo.toml is missing; there is no manifest to read"]
    try:
        return tomllib.loads(manifest_path.read_text(encoding="utf-8")), []
    except tomllib.TOMLDecodeError as exc:
        return None, [f"Cargo.toml is not valid TOML ({exc})"]


def observe(root: Path) -> tuple[list[str] | None, str | None]:
    cargo = shutil.which("cargo")
    if cargo is None:
        return None, "cargo is not on PATH; the shipped set cannot be listed"
    first_failure = None
    for extra in (["--offline"], []):
        proc = subprocess.run(
            [cargo, "package", "--list", "--allow-dirty", *extra],
            cwd=root,
            capture_output=True,
            text=True,
        )
        if proc.returncode == 0:
            return proc.stdout.splitlines(), None
        if first_failure is None:
            detail = (proc.stderr or proc.stdout).strip().splitlines()
            first_failure = (
                f"`cargo package --list` failed (exit {proc.returncode}): "
                f"{detail[0] if detail else 'no cargo output'}"
            )
    return None, first_failure


def shipped_tree(root: Path) -> list[str]:
    """The crate's own repository files under every directory the record names.

    The record fixes the group (`every file under `src/``); these paths are the
    observation a listing is judged against, so a manifest that ships one file of
    a named directory does not ship the group.
    """
    tree: list[str] = []
    for directory in SHIPPED_DIRS:
        base = root / directory
        if not base.is_dir():
            continue
        tree.extend(
            path.relative_to(root).as_posix()
            for path in sorted(base.rglob("*"))
            if path.is_file()
        )
    return tree


def main() -> int:
    root = Path(__file__).resolve().parents[2]
    findings: list[str] = []
    notes: list[str] = []

    text, errors = read_prose(root)
    findings += errors
    if text is not None:
        findings += check_prose(text)

    manifest, errors = read_manifest(root)
    findings += errors
    if manifest is not None:
        mechanism_findings, mechanism_notes = check_mechanism(manifest)
        findings += mechanism_findings
        notes += mechanism_notes

    tree = shipped_tree(root)
    listing: list[str] = []
    observed, error = observe(root)
    if error is not None:
        findings.append(error)
    else:
        listing = observed or []
        findings += evaluate(listing, tree)
        note = pack_time_note(listing)
        if note is not None:
            notes.insert(0, note)

    for note in notes:
        print(f"note: {note}")
    if findings:
        for finding in findings:
            fail(finding)
        return 1

    members = sorted({p.strip() for p in listing if p.strip() and p not in CARGO_PACK_TIME})
    print(
        f"ok: the {len(members)} shipped paths are exactly the set {SET_NODE.stem} "
        "records and are selected by the allowlist its dependent records"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
