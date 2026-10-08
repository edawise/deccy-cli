#!/usr/bin/env python3
"""Token check for the version token recorded by the version decision.

The version decision (nodes/nlomlqum.json) records deccy's version as one
machine-readable token, `content.content`. This check reads that token from the
recorded node and requires it to equal the release value the decision is pinned
to.

It reads no stamping surface. A surface carrying the token is a separate use of
the version decision, made by another edge, and the equality of that surface to
the recorded token is checked by the evaluation covering that use (E-I1a,
nodes/uzwwmprz.json, for the manifest). Pinning the recorded token here keeps
this gate's subject to the token the version decision itself carries.

Stdlib only: no cargo, no build, no network. Exit 0 when the recorded token is
the release value, exit 1 with a finding when it is not.

Run from the repository root:

    python3 tests/version_token/version_token_test.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# The release value this decision is pinned to; the node is its single source.
EXPECTED_VERSION = "0.2.1"

NODE = "nodes/nlomlqum.json"

ROOT = Path(__file__).resolve().parents[2]

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


def main() -> int:
    token = recorded_token()

    if token is not None and token != EXPECTED_VERSION:
        fail(
            f"{NODE}: version token is {token!r}, expected {EXPECTED_VERSION!r} — "
            "the single source does not record the release value this decision is pinned to"
        )

    if failures:
        print(f"\n{len(failures)} version-token check(s) failed")
        return 1

    print(f"version token {EXPECTED_VERSION!r} is the token the version decision records")
    return 0


if __name__ == "__main__":
    sys.exit(main())
