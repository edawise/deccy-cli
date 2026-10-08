#!/usr/bin/env python3
"""Conformance probe for E-I1a (uzwwmprz): the manifest stamps the version decision.

The version decision (nodes/nlomlqum.json) records one version token and is the
single source every stamping surface takes its version from. The manifest
decision (I1a) stamps the manifest's [package] version from that decision
instead of choosing one. This probe reads both artifacts and fails, naming both
sides, unless the manifest's [package] version is exactly the recorded token.

Stdlib only: no cargo, no build, no network. Exit 0 when the manifest conforms,
exit 1 with a finding when it does not.

Usage: python3 tests/version_stamp/version_stamp.py
"""

import json
import sys
import tomllib
from pathlib import Path

VERSION_NODE = Path("nodes") / "nlomlqum.json"
MANIFEST = Path("Cargo.toml")


def fail(message: str) -> int:
    print(f"FAIL: {message}")
    return 1


def main() -> int:
    root = Path(__file__).resolve().parents[2]

    if sys.version_info < (3, 11):
        return fail("python3 >= 3.11 is required to read TOML (tomllib)")

    node_path = root / VERSION_NODE
    if not node_path.is_file():
        return fail(f"{VERSION_NODE} is missing; the version decision is not recorded")
    node = json.loads(node_path.read_text(encoding="utf-8"))
    if node.get("kind") != "decision":
        return fail(
            f"{VERSION_NODE} is not a decision node (kind is {node.get('kind')!r})"
        )
    content = node.get("content") or {}
    if content.get("type") != "version":
        return fail(
            f"{VERSION_NODE} content type is {content.get('type')!r}, not 'version'"
        )
    token = content.get("content")
    if not isinstance(token, str) or not token.strip():
        return fail(f"{VERSION_NODE} records no version token")

    manifest_path = root / MANIFEST
    if not manifest_path.is_file():
        return fail("Cargo.toml is missing; there is no manifest to stamp")
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    package = manifest.get("package")
    if not isinstance(package, dict):
        return fail("Cargo.toml has no [package] table to carry the version")
    if "version" not in package:
        return fail(
            "[package] declares no version; the manifest does not stamp the version decision"
        )
    version = package["version"]
    if not isinstance(version, str):
        return fail(
            "[package] version is not a literal token "
            f"({type(version).__name__}); the manifest does not carry the decision's version"
        )

    if version != token:
        return fail(
            f'[package] version is "{version}" '
            f'but the version decision records "{token}"'
        )

    print(f'ok: [package] version "{version}" is the version decision token')
    return 0


if __name__ == "__main__":
    sys.exit(main())
