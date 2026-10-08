#!/usr/bin/env python3
"""Sentinel check for the version token recorded by the version decision.

The version decision (nodes/nlomlqum.json) records deccy's version as one
machine-readable token, `content.content`. The token the decision records is a
*development sentinel* carried in the tree between releases, not a release
number: the graph records no release number at all, and the release tag is the
only record of a release.

This check reads that token from the recorded node and requires it to be the
sentinel the decision names. Two properties of the one token are observed:

  * sentinel shape — the token parses as a semantic version carrying a `dev`
    pre-release marker, so it is marked as a development sentinel rather than
    stamped at release time. A bare `MAJOR.MINOR.PATCH` release value fails this
    property whatever its digits: the decision records no release number.
  * sentinel identity — the token is `0.0.0-dev`, the sentinel the decision
    records, so a dev-marked release core (`0.2.2-dev`) does not stand in for it.

It reads no stamping surface. A surface carrying the token is a separate use of
the version decision, on its own edge, and the equality of that surface to the
recorded token is checked by the evaluation covering that use (E-I1a,
nodes/uzwwmprz.json, for the manifest; nodes/pnoptpkk.json, for the lockfile).
How a release replaces the sentinel with the tagged version is the release
step's business; this check's subject is the token the graph records.

Stdlib only: no cargo, no build, no network. Exit 0 when the recorded token is
the sentinel, exit 1 with a finding when it is not.

Run from the repository root:

    python3 tests/version_token/version_token_test.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

# The sentinel the version decision records before any release substitutes a tag.
SENTINEL = "0.0.0-dev"

# The pre-release identifier that marks a token as a development sentinel.
DEV_MARKER = "dev"

NODE = "nodes/nlomlqum.json"

ROOT = Path(__file__).resolve().parents[2]

# Semantic Versioning 2.0.0, the grammar the token has to be written in.
SEMVER = re.compile(
    r"^(?P<major>0|[1-9]\d*)\.(?P<minor>0|[1-9]\d*)\.(?P<patch>0|[1-9]\d*)"
    r"(?:-(?P<prerelease>"
    r"(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*"
    r"))?"
    r"(?:\+(?P<build>[0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)

failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)
    print(f"FAIL: {message}")


def recorded_token() -> str | None:
    location = ROOT / NODE
    if not location.is_file():
        fail(f"{NODE}: missing; the version token this check reads is not recorded")
        return None
    try:
        document = json.loads(location.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"{NODE}: not valid JSON ({error}); the recorded version token cannot be read")
        return None
    content = document.get("content")
    token = content.get("content") if isinstance(content, dict) else None
    if not isinstance(token, str) or not token:
        fail(f"{NODE}: content.content does not hold a version token")
        return None
    return token


def check_sentinel_shape(token: str) -> None:
    """The token is a development sentinel, not a release number."""
    match = SEMVER.match(token)
    if match is None:
        fail(
            f"{NODE}: version token {token!r} is not a semantic version; the decision "
            f"records the development sentinel {SENTINEL!r}"
        )
        return
    prerelease = match.group("prerelease")
    if prerelease is None:
        fail(
            f"{NODE}: version token {token!r} is a release version with no pre-release "
            f"marker — the decision records the development sentinel {SENTINEL!r} and no "
            "release number at all"
        )
        return
    if DEV_MARKER not in prerelease.split("."):
        fail(
            f"{NODE}: version token {token!r} carries pre-release {prerelease!r} but no "
            f"{DEV_MARKER!r} identifier — it is not the development sentinel {SENTINEL!r}"
        )


def check_sentinel_identity(token: str) -> None:
    """The token is the sentinel the decision records."""
    if token != SENTINEL:
        fail(
            f"{NODE}: version token is {token!r}, but the development sentinel the "
            f"decision records is {SENTINEL!r}"
        )


def main() -> int:
    token = recorded_token()

    if token is not None:
        check_sentinel_shape(token)
        check_sentinel_identity(token)

    if failures:
        print(f"\n{len(failures)} version-token check(s) failed")
        return 1

    print(f"version token {SENTINEL!r} is the development sentinel the version decision records")
    return 0


if __name__ == "__main__":
    sys.exit(main())
