#!/usr/bin/env python3
"""Cut a deccy release from a local checkout.

The release is a local step, not a generated CI workflow. This one entry point

  1. takes the named version from the caller,
  2. puts it in the manifest's `[package] version` in place of the token the
     version decision (nodes/nlomlqum.json) records — and only there: a
     manifest that carries any other version has already been substituted or
     was never the token, so the script refuses instead of overwriting it,
  3. drives the release from `dist-workspace.toml`: `dist build` makes the
     prebuilt binaries and installers for the targets that config names,
     `dist host --steps=create,upload,release` puts them on the GitHub release,
     and `cargo publish` uploads the crate to crates.io.

`dist-workspace.toml` is read, never rewritten: this script does not run
`dist init` and does not regenerate workflows.

The version can be given in any of these forms; the first one found wins:

    scripts/release.py 1.2.3
    scripts/release.py 1.2.3 --dry-run
    scripts/release.py --version 1.2.3
    scripts/release.py --tag v1.2.3
    scripts/release.py v1.2.3
    VERSION=1.2.3 scripts/release.py
    echo 1.2.3 | scripts/release.py

`--dry-run` stops after the substitution and prints the commands it would run.

The substitution is done before anything is built, because dist derives the tag
it releases from the manifest version and refuses a manifest that disagrees with
the tag. The caller commits the substituted manifest (and the refreshed
lockfile) before the tag is pushed.

The recorded token is the only value the substitution replaces. If the manifest
carries something else the script stops before building, with a non-zero exit
and nothing written: either the tree has already been substituted for another
release (rerunning with the version it already carries is a no-op that
proceeds), or it is in a state this script did not produce and a human has to
decide. If nodes/nlomlqum.json is absent there is no token to substitute, so the
release is refused rather than given a guessed version.
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from pathlib import Path

# The file whose `[package] version` carries the release version.
MANIFEST = "Cargo.toml"
# The node that records the token the substitution replaces.
VERSION_NODE = Path("nodes") / "nlomlqum.json"
# The dist config the release reads. It is never written.
DIST_CONFIG = "dist-workspace.toml"

# A named release version, optionally with pre-release/build metadata.
VERSION_RE = re.compile(r"^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$")

PACKAGE_HEADER = re.compile(r"^\[package\]\s*$", re.MULTILINE)
SECTION_HEADER = re.compile(r"^\[", re.MULTILINE)
VERSION_LINE = re.compile(r'^(?P<lead>[ \t]*version[ \t]*=[ \t]*")(?P<value>[^"\n]*)(?P<tail>")[ \t]*$', re.MULTILINE)


def repo_root() -> Path:
    """The checkout this script lives in: scripts/ sits at its top level."""
    return Path(__file__).resolve().parent.parent


def die(message: str):
    print(f"release: {message}", file=sys.stderr)
    raise SystemExit(1)


def recorded_token(root: Path) -> str:
    """The token the version decision records, as the graph holds it."""
    location = root / VERSION_NODE
    if not location.is_file():
        die(f"{VERSION_NODE}: missing; the version the release substitutes is not recorded")
    document = json.loads(location.read_text(encoding="utf-8"))
    content = document.get("content")
    token = content.get("content") if isinstance(content, dict) else None
    if not isinstance(token, str) or not token:
        die(f"{VERSION_NODE}: content.content does not hold a version token")
    return token


def strip_tag_prefix(text: str) -> str:
    """Accept the tag form `v1.2.3` as the version `1.2.3`."""
    text = text.strip()
    return text[1:] if text[:1] in {"v", "V"} and text[1:2].isdigit() else text


def parse_version(argv: list[str]) -> str:
    """The named version from the arguments, the environment, or standard input."""
    args = list(argv)
    if "--help" in args or "-h" in args:
        print(__doc__)
        raise SystemExit(0)
    args = [arg for arg in args if arg != "--dry-run"]

    for flag in ("--version", "--tag"):
        if flag in args:
            index = args.index(flag)
            if index + 1 >= len(args):
                die(f"{flag} needs a version after it")
            return strip_tag_prefix(args[index + 1])

    positional = [arg for arg in args if not arg.startswith("-")]
    if positional:
        return strip_tag_prefix(positional[0])

    from_environment = os.environ.get("VERSION")
    if from_environment and from_environment.strip():
        return strip_tag_prefix(from_environment)

    if not sys.stdin.isatty():
        from_stdin = sys.stdin.read()
        if from_stdin.strip():
            return strip_tag_prefix(from_stdin)

    die(
        "no version given; pass it as an argument, as --version <v>, as --tag v<v>, "
        "as VERSION=<v> in the environment, or on standard input"
    )


def package_version(text: str) -> re.Match:
    """The `[package] version` line, matched in place so the rest of the file is untouched."""
    header = PACKAGE_HEADER.search(text)
    if header is None:
        die(f"{MANIFEST}: has no [package] table to carry the version")
    section_end = SECTION_HEADER.search(text, header.end())
    line = VERSION_LINE.search(text, header.end(), section_end.start() if section_end else len(text))
    if line is None:
        die(
            f"{MANIFEST}: [package] declares no literal `version = \"...\"`; the release "
            "substitutes the version there and does not follow an inherited one"
        )
    return line


def substitute(root: Path, version: str) -> bool:
    """Put `version` in the manifest's [package] version; return whether it changed."""
    location = root / MANIFEST
    if not location.is_file():
        die(f"{MANIFEST}: missing; the release substitutes the version there")
    text = location.read_text(encoding="utf-8")
    line = package_version(text)
    current = line.group("value")

    # The token is read from the record and nowhere else: no literal fallback,
    # so a tree without the record cannot be substituted at all.
    token = recorded_token(root)
    if current == version:
        print(f"{MANIFEST}: already at {version}; nothing to substitute")
        return False
    if current != token:
        die(
            f"refusing to substitute: {MANIFEST} carries [package] version {current!r} but "
            f"{VERSION_NODE} records the token {token!r}; the release replaces the recorded "
            "token and does not overwrite a version the record does not carry (rerun with the "
            "version the manifest already carries, or restore the token the version decision "
            "records)"
        )

    updated = (
        text[: line.start()]
        + f'{line.group("lead")}{version}{line.group("tail")}'
        + text[line.end() :]
    )

    written = package_version(updated).group("value")
    if written != version:
        die(f"{MANIFEST}: substitution did not take; [package] version is {written!r}")
    if token and token in updated:
        die(f"{MANIFEST}: the token {token!r} survives the substitution; the tree still carries it")

    location.write_text(updated, encoding="utf-8")
    print(
        f"{MANIFEST}: [package] version {current} -> {version} "
        f"(the recorded token is {token})"
    )
    return True


def run(root: Path, command: list[str]) -> None:
    print(f"+ {' '.join(command)}")
    try:
        subprocess.run(command, cwd=str(root), check=True)
    except FileNotFoundError:
        die(f"{command[0]}: not found on PATH; the release drives {command[0]}")
    except subprocess.CalledProcessError as error:
        die(f"{' '.join(command)} failed with exit status {error.returncode}")


def main(argv: list[str]) -> int:
    dry_run = "--dry-run" in argv
    version = parse_version(argv)
    if not VERSION_RE.match(version):
        die(f"{version!r} is not a named MAJOR.MINOR.PATCH version")

    root = repo_root()
    if not (root / DIST_CONFIG).is_file():
        die(f"{DIST_CONFIG}: missing; the release reads the dist machinery there")

    substitute(root, version)

    steps = [
        # Build the prebuilt binaries, archives and installers the dist config names.
        ["dist", "build"],
        # Create the GitHub release, upload the artifacts, publish it.
        ["dist", "host", "--steps=create", "--steps=upload", "--steps=release"],
        # Upload the crate to crates.io.
        ["cargo", "publish"],
    ]
    if dry_run:
        print("dry run; the release would run:")
        for step in steps:
            print(f"  + {' '.join(step)}")
        return 0

    for step in steps:
        run(root, step)

    print(f"released {version}; commit the substituted {MANIFEST} and the lockfile, then tag")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
