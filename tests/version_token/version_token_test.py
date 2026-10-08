#!/usr/bin/env python3
"""Single-source check for deccy's version token.

The version decision (nodes/nlomlqum.json) records deccy's version as one
token and claims every stamping surface takes that token from it. This check
reads the recorded token, requires it to be the release value, and requires
each stamping surface the tree already carries (the package manifest, and the
lockfile when it exists) to carry the same token instead of drifting on its
own. A surface the tree does not yet carry is not a finding: the recorded
token is the unconditional check at this decision.

Run from the repository root:

    python3 tests/version_token/version_token_test.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

# The release value this layer sets; the decision node is its single source.
EXPECTED_VERSION = "0.2.0"

NODE = "nodes/nlomlqum.json"
MANIFEST = "Cargo.toml"
LOCKFILE = "Cargo.lock"
PACKAGE = "deccy-cli"

ROOT = Path(__file__).resolve().parents[2]

failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)
    print(f"FAIL: {message}")


def read_text(path: str) -> str | None:
    location = ROOT / path
    if not location.is_file():
        fail(f"{path}: missing; the version decision is the single source and this surface reads it")
        return None
    return location.read_text(encoding="utf-8")


def read_text_if_present(path: str) -> str | None:
    location = ROOT / path
    return location.read_text(encoding="utf-8") if location.is_file() else None


def recorded_token() -> str | None:
    text = read_text(NODE)
    if text is None:
        return None
    try:
        document = json.loads(text)
    except json.JSONDecodeError as error:
        fail(f"{NODE}: not valid JSON ({error}); the recorded version token cannot be read")
        return None
    token = document.get("content", {}).get("content")
    if not isinstance(token, str) or not token:
        fail(f"{NODE}: content.content does not hold a version token")
        return None
    return token


def toml_section_version(text: str, section: str) -> str | None:
    """The `version` value declared inside `[section]` of a TOML document."""
    inside = False
    for raw in text.splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]"):
            inside = line == f"[{section}]"
            continue
        if inside and "=" in line:
            key, _, value = line.partition("=")
            if key.strip() == "version":
                return value.strip().strip('"').strip("'")
    return None


def lock_package_version(text: str, name: str) -> str | None:
    """The `version` of the package named `name` in a TOML lockfile."""
    for chunk in text.split("[[package]]")[1:]:
        fields: dict[str, str] = {}
        for raw in chunk.splitlines():
            line = raw.split("#", 1)[0].strip()
            if not line or line.startswith("["):
                continue
            if "=" in line:
                key, _, value = line.partition("=")
                fields[key.strip()] = value.strip().strip('"').strip("'")
        if fields.get("name") == name:
            return fields.get("version")
    return None


def main() -> int:
    token = recorded_token()

    if token is not None and token != EXPECTED_VERSION:
        fail(
            f"{NODE}: version token is {token!r}, expected {EXPECTED_VERSION!r} — "
            "the single source records a version other than the release value"
        )

    if token is not None:
        # A stamping surface that the tree does not carry yet is not a finding: only the
        # recorded token is unconditional at this decision. Surfaces are compared where they exist.
        manifest = read_text_if_present(MANIFEST)
        if manifest is not None:
            manifest_version = toml_section_version(manifest, "package")
            if manifest_version is None:
                fail(f"{MANIFEST}: [package] declares no version; the manifest cannot take the token from {NODE}")
            elif manifest_version != token:
                fail(
                    f"{MANIFEST}: [package] version is {manifest_version!r} but {NODE} records "
                    f"{token!r} — the stamping surface has drifted from the single source"
                )

        lockfile = read_text_if_present(LOCKFILE)
        if lockfile is not None:
            locked = lock_package_version(lockfile, PACKAGE)
            if locked is not None and locked != token:
                fail(
                    f"{LOCKFILE}: {PACKAGE} is locked at {locked!r} but {NODE} records "
                    f"{token!r} — the stamping surface has drifted from the single source"
                )

    if failures:
        print(f"\n{len(failures)} version-token check(s) failed")
        return 1

    print(f"version token {EXPECTED_VERSION!r} is single-sourced across the recorded surfaces")
    return 0


if __name__ == "__main__":
    sys.exit(main())
