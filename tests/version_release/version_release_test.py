#!/usr/bin/env python3
"""Release probe for E-M-release (kzoqworr): the substitution, then `cargo publish`.

The release decision (M-release, nodes/wtpwmkqr.json) is that a release is the
version substitution followed by `cargo publish` — two moves, one local entry
point, nothing else:

  * one executable entry point under `scripts/` takes the named version from its
    caller and puts it in the manifest's `[package] version`, in place of *the
    token the version decision (nodes/nlomlqum.json) records*;
  * then it publishes the crate: `cargo publish`.

The rest of the release machinery is gone. No prebuilt binaries, no installers,
no build platform plan, no hosting step, and no CI calling the entry point: the
release answers to the manifest and the version record alone. So every sandbox
below is a copy of this tree as it stands, with nothing planted and nothing
removed, and what a release drives over it is the substitution and then a
publish and nothing else — no build platform machinery, no hosting step. A
release that still needs a release-layout file to substitute and publish does
not conform.

The probe runs the entry point in throwaway copies of the tree with `cargo`,
`dist`, `gh`, `git`, `jj`, `curl` and `wget` replaced by recording stubs: no
build, no network, no repository is touched. The copies differ in what the
version record and the manifest carry, so what the entry point does depends on
the recorded token and on nothing else:

  A. recorded token T in `nodes/nlomlqum.json` and T in the manifest — the state
     the version decision describes for a tree between releases. The entry point
     must substitute the requested version, exit 0, and drive `cargo publish`
     after the substitution. Run once per documented interface form, each with
     its own distinct requested version, so an entry point that accepts only one
     of the forms the release documents is named as such.
  B. a token T' the record holds in place of T, with the manifest carrying T'.
     The substitution must still happen: it is driven by the recorded token, not
     by a literal in the entry point.
  C. the record still holds T but the manifest carries a named version the
     record does not carry. The entry point must refuse — non-zero exit, the
     manifest left as it was, no step driven — rather than blindly overwriting
     the version it finds.
  D. the version record absent. The entry point must refuse for the same reason:
     there is no recorded token to substitute.
  E. the substitution asked for a second, different named version over the tree
     scenario A releases. What lands is the version the caller names, so a
     version built into the entry point is not the version that is released.
  F. the manifest already carries the requested version — the one path the
     decision allows the entry point to be a no-op on, and the only path that
     can release the tree as it stands. The entry point must exit 0, leave that
     version in place, and still publish. Run for each version A and E ask for,
     so a no-op keyed to one literal version is not mistaken for the
     already-substituted path.
  G. the `cargo publish` stub exits non-zero. The entry point must exit non-zero
     and stop. "Stopped" is observable, not inferred: after the failing
     invocation the recording stubs see no later step.

What a release must not do is observed the same way: an invocation of `dist`, or
of `gh release`, during a release is named. Those are the prebuilt binaries, the
installers and the GitHub hosting the decision took out of the release, and a
release that still drives them does not conform.

The order inside a release is checked rather than assumed. The `cargo` stub
records the `[package] version` the manifest carries at the moment the crate is
published, so a publish issued before the substitution is named as such: it
would publish the token, not the release.

The release decision documents six ways a caller may name the version: as the
first argument, as `--version <v>`, as `--tag v<v>`, as a bare `v<v>`, as
`VERSION=<v>` in the environment, and on standard input. The probe holds the
entry point to that documented interface. Scenario A runs the substitution once
per form, each with its own distinct named version, and names every form the
entry point does not accept: accepting one form is not enough, because a caller
who names the version another way is not served. The form that conforms is
reused to offer a *different* version (scenario E), so the probe observes the
version travelling from the caller to the manifest rather than a literal the
entry point happens to carry.

Stdlib only: no cargo, no build, no network. Run from the repository root:

    python3 tests/version_release/version_release_test.py
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import tomllib
from collections import namedtuple
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

VERSION_NODE = Path("nodes") / "nlomlqum.json"
MANIFEST = Path("Cargo.toml")
SCRIPTS_DIR = Path("scripts")

# The named version the probe asks the entry point to release for the first
# documented interface form. Any concrete MAJOR.MINOR.PATCH dist would accept
# works; it only has to be a named release version, different from the token the
# version decision records.
REQUESTED = "9.9.9"

# One distinct named version per remaining documented interface form, so the
# run for a form cannot be satisfied by a version the entry point happens to
# carry, or by another form's run.
SECOND_FORM_REQUEST = "10.4.1"
THIRD_FORM_REQUEST = "11.5.2"
FOURTH_FORM_REQUEST = "12.6.3"
FIFTH_FORM_REQUEST = "13.7.4"
SIXTH_FORM_REQUEST = "14.8.5"

# A second, different named version (scenario E). The same tree, the same
# record, the same interface form, a different version asked for: whatever lands
# in the manifest came from the caller.
REQUESTED_ALTERNATE = "15.9.6"

# A token the version decision does *not* record. Scenario B substitutes the
# record with it: an entry point that hardcodes the sentinel it knows fails.
SENTINEL_ALTERNATE = "0.0.0-probe-sentinel"

# Neither the record nor the probe names this version. Scenario C hands the
# entry point a manifest carrying it: a blind overwrite replaces it, a
# substitution driven by the recorded token refuses.
UNRECORDED = "1.2.3"

# Directories that are not part of the tree the entry point reads, and that are
# far too large to copy into a throwaway sandbox.
SKIP = shutil.ignore_patterns(".jj", ".git", ".deccy", "target", "node_modules")

# Tools the release step drives, replaced by recording stubs in the sandbox.
# `dist` and `gh` are stubbed as well, so that a release which still builds,
# hosts or plans through the dist machinery records the invocation instead of
# reaching a real tool.
STUBBED_TOOLS = ("cargo", "dist", "gh", "git", "jj", "curl", "wget")

# The log line the stub writes when the crate is published: the manifest version
# at that moment, so the probe sees whether the substitution came first.
OBSERVED_MARKER = "PROBE_MANIFEST_VERSION"

STUB = """#!/bin/sh
# Recording stub for the release probe: touches no network, no build, no repo.
# PROBE_VERSION is the version this run asked for, so a tool that answers with a
# version answers with the one the release was given. PROBE_FAIL_TOOL names the
# one tool that must fail (scenario G) and PROBE_FAIL_ARG the argument that makes
# it fail; every other invocation exits 0.
tool=$(basename "$0")
{
  printf '%s' "$tool"
  for arg in "$@"; do printf '\\t%s' "$arg"; done
  printf '\\n'
} >> "$PROBE_LOG"
if [ "$tool" = cargo ]; then
  case " $* " in
    *" publish "*)
      # What the manifest carries at the moment of the publish: the substitution
      # has to have happened before it.
      observed=$(awk -F'"' '/^\\[package\\]/{p=1;next} /^\\[/{p=0} \\
        p && $0 ~ /^[[:space:]]*version[[:space:]]*=/ {print $2; exit}' Cargo.toml 2>/dev/null)
      printf '%s\\t%s\\n' "PROBE_MANIFEST_VERSION" "$observed" >> "$PROBE_LOG"
      ;;
  esac
fi
target="$tool"
case "$tool" in
  cargo) [ "${1:-}" = dist ] && target=dist ;;
esac
if [ -n "${PROBE_FAIL_TOOL:-}" ] && [ "$target" = "$PROBE_FAIL_TOOL" ]; then
  case " $* " in
    *" ${PROBE_FAIL_ARG:-} "*) exit 3 ;;
  esac
fi
case "$tool" in
  dist)
    case " $* " in *--output-format*) printf '{}' ;; esac
    ;;
  git)
    case " $* " in
      *describe*) printf 'v%s\\n' "${PROBE_VERSION:-0.0.0}" ;;
      *rev-parse*) printf '0000000000000000000000000000000000000000\\n' ;;
    esac
    ;;
  jj)
    case " $* " in *template*) printf '%s\\n' "${PROBE_VERSION:-0.0.0}" ;; esac
    ;;
esac
exit 0
"""

# How the probe offers a named version to the entry point, and the distinct
# version it offers through each form. The release documents all six ways of
# naming the version, so the probe requires a substitution for every one of them:
# accepting a single form leaves a caller who names the version another way
# unserved. Each is kept as a label plus a builder so the discriminating
# scenarios can offer a *different* version through an interface the entry point
# accepted.
FORMS: list[tuple[str, object, str]] = [
    ("<version> as the first argument", lambda version: ([version], {}, None), REQUESTED),
    (
        "--version <version>",
        lambda version: (["--version", version], {}, None),
        SECOND_FORM_REQUEST,
    ),
    (
        "--tag v<version>",
        lambda version: (["--tag", f"v{version}"], {}, None),
        THIRD_FORM_REQUEST,
    ),
    (
        "v<version> as the first argument",
        lambda version: ([f"v{version}"], {}, None),
        FOURTH_FORM_REQUEST,
    ),
    (
        "VERSION=<version> in the environment",
        lambda version: ([], {"VERSION": version}, None),
        FIFTH_FORM_REQUEST,
    ),
    (
        "<version> on standard input",
        lambda version: ([], {}, f"{version}\n"),
        SIXTH_FORM_REQUEST,
    ),
]
FORMS_BY_LABEL = {label: builder for label, builder, _ in FORMS}

# One conforming (entry point, interface form) pair, discovered in scenario A and
# reused for the discriminating scenarios.
Invocation = namedtuple("Invocation", "entry label")
# What one sandboxed run left behind: its exit status, the manifest version after
# it, the recorded invocations, and the manifest version observed at publish time.
Probe = namedtuple("Probe", "log rc version note calls publishes")


def invocation_for(
    invocation: Invocation, version: str
) -> tuple[list[str], dict[str, str], str | None]:
    """Offer `version` through the interface form that scenario A found conforming."""
    return FORMS_BY_LABEL[invocation.label](version)


failures: list[str] = []


def fail(message: str) -> None:
    failures.append(message)
    print(f"FAIL: {message}")


def recorded_token() -> str | None:
    """The version token the entry point has to replace, as the graph records it."""
    location = ROOT / VERSION_NODE
    if not location.is_file():
        fail(f"{VERSION_NODE}: missing; the version the release substitutes is not recorded")
        return None
    try:
        document = json.loads(location.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        fail(f"{VERSION_NODE}: not valid JSON ({error})")
        return None
    content = document.get("content")
    token = content.get("content") if isinstance(content, dict) else None
    if not isinstance(token, str) or not token:
        fail(f"{VERSION_NODE}: content.content does not hold a version token")
        return None
    return token


def entry_points() -> list[Path]:
    """Executable files under scripts/ — the release step's entry point(s)."""
    directory = ROOT / SCRIPTS_DIR
    if not directory.is_dir():
        fail(
            f"{SCRIPTS_DIR}/: missing; the release decision puts the release step in one "
            "executable entry point there"
        )
        return []
    executables: list[Path] = []
    others: list[str] = []
    for path in sorted(directory.rglob("*")):
        if not path.is_file():
            continue
        relative = path.relative_to(ROOT)
        if path.name.endswith(".md"):
            continue
        if os.access(path, os.X_OK):
            executables.append(relative)
        else:
            others.append(str(relative))
    if not executables:
        if others:
            fail(
                f"{SCRIPTS_DIR}/: {', '.join(others)} is not executable; the release step "
                "needs one executable entry point"
            )
        else:
            fail(f"{SCRIPTS_DIR}/: holds no executable release entry point")
    return executables


def write_stubs(stub_dir: Path) -> None:
    stub_dir.mkdir(parents=True, exist_ok=True)
    for tool in STUBBED_TOOLS:
        location = stub_dir / tool
        location.write_text(STUB, encoding="utf-8")
        location.chmod(0o755)


def manifest_version(tree: Path) -> str | None:
    location = tree / MANIFEST
    if not location.is_file():
        return None
    try:
        document = tomllib.loads(location.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError:
        return None
    package = document.get("package")
    if not isinstance(package, dict):
        return None
    version = package.get("version")
    return version if isinstance(version, str) else None


VERSION_LINE = re.compile(r'^(?P<lead>[ \t]*version[ \t]*=[ \t]*")(?P<value>[^"\n]*)(?P<tail>"[^\n]*)$')


def set_manifest_version(tree: Path, version: str) -> bool:
    """Put `version` in the sandbox manifest's [package] version, nothing else."""
    location = tree / MANIFEST
    if not location.is_file():
        return False
    lines = location.read_text(encoding="utf-8").splitlines(keepends=True)
    start = None
    for index, line in enumerate(lines):
        if line.strip() == "[package]":
            start = index + 1
            break
    if start is None:
        return False
    for index in range(start, len(lines)):
        stripped = lines[index].strip()
        if stripped.startswith("["):
            break
        match = VERSION_LINE.match(lines[index].rstrip("\n"))
        if match:
            newline = "\n" if lines[index].endswith("\n") else ""
            lines[index] = f"{match.group('lead')}{version}{match.group('tail')}{newline}"
            location.write_text("".join(lines), encoding="utf-8")
            return True
    return False


def set_recorded_token(tree: Path, token: str) -> bool:
    """Rewrite the sandbox's version record to hold `token`."""
    location = tree / VERSION_NODE
    if not location.is_file():
        return False
    document = json.loads(location.read_text(encoding="utf-8"))
    content = document.get("content")
    if not isinstance(content, dict):
        return False
    content["content"] = token
    location.write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    return True


def prepare(
    parent: Path,
    name: str,
    manifest_version_value: str,
    token: str | None,
    drop_record: bool = False,
) -> Path | None:
    """A copy of the tree as it stands, with the version record and manifest the scenario needs.

    Nothing is planted and nothing is removed, so a release that needs a file the
    tree does not carry cannot substitute here, and is observed failing to.
    """
    tree = parent / name
    try:
        shutil.copytree(ROOT, tree, ignore=SKIP, symlinks=True)
    except OSError as error:
        fail(f"could not prepare the {name} sandbox ({error})")
        return None
    if not set_manifest_version(tree, manifest_version_value):
        fail(
            f"{MANIFEST}: [package] declares no literal `version = \"...\"` line for the probe "
            "to place the version in"
        )
        return None
    record = tree / VERSION_NODE
    if drop_record:
        if record.is_file():
            record.unlink()
    elif token is not None:
        if not set_recorded_token(tree, token):
            fail(f"{VERSION_NODE}: could not be given the token {token!r} in the {name} sandbox")
            return None
    return tree


def parse_log(log: str) -> tuple[list[tuple[str, list[str]]], list[str]]:
    """Split the stub log into (tool, arguments) calls and publish-time manifest versions.

    `cargo dist ...` is folded into a `dist` invocation: either spelling drives the
    dist machinery the decision took out of the release.
    """
    calls: list[tuple[str, list[str]]] = []
    publishes: list[str] = []
    for line in log.splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        tool, argv = fields[0], [field for field in fields[1:] if field]
        if tool == OBSERVED_MARKER:
            publishes.append(argv[0] if argv else "")
            continue
        if tool == "cargo" and argv and argv[0] == "dist":
            tool, argv = "dist", argv[1:]
        calls.append((tool, argv))
    return calls, publishes


def run_entry(
    tree: Path,
    stub_dir: Path,
    log_path: Path,
    home: Path,
    entry: Path,
    argv: list[str],
    env_extra: dict[str, str],
    stdin_text: str | None,
    requested: str,
) -> Probe:
    """Run one sandboxed invocation for `requested`; return what it left behind."""
    command = [str(tree / entry)] + argv
    env = os.environ.copy()
    env["PATH"] = f"{stub_dir}{os.pathsep}{env.get('PATH', '')}"
    env["HOME"] = str(home)
    env["PROBE_LOG"] = str(log_path)
    env["PROBE_VERSION"] = requested
    env.pop("PROBE_FAIL_TOOL", None)
    env.pop("PROBE_FAIL_ARG", None)
    env.update(env_extra)
    options: dict = {
        "cwd": str(tree),
        "env": env,
        "capture_output": True,
        "text": True,
        "timeout": 120,
    }
    if stdin_text is None:
        options["stdin"] = subprocess.DEVNULL
    else:
        options["input"] = stdin_text
    rc: int | None = None
    note = ""
    try:
        completed = subprocess.run(command, **options)
        rc = completed.returncode
        note = f"exit {rc}"
        if completed.stderr.strip():
            note += f"; stderr: {completed.stderr.strip()[:400]}"
    except subprocess.TimeoutExpired:
        note = "timed out after 120s"
    except OSError as error:
        note = f"could not be run ({error})"
    log = log_path.read_text(encoding="utf-8") if log_path.is_file() else ""
    calls, publishes = parse_log(log)
    return Probe(
        log=log,
        rc=rc,
        version=manifest_version(tree),
        note=note,
        calls=calls,
        publishes=publishes,
    )


def words(argv: list[str]) -> list[str]:
    """The non-flag arguments of a recorded invocation."""
    return [arg for arg in argv if not arg.startswith("-")]


def publishes(calls: list[tuple[str, list[str]]]) -> list[int]:
    """Where the recorded `cargo publish` invocations sit in the log."""
    return [
        index
        for index, (tool, argv) in enumerate(calls)
        if tool == "cargo" and "publish" in words(argv)
    ]


def dist_machinery(calls: list[tuple[str, list[str]]]) -> list[str]:
    """The invocations the decision took out of the release: dist, and GitHub hosting."""
    found: list[str] = []
    for tool, argv in calls:
        if tool == "dist":
            found.append(" ".join(["dist", *argv]))
        elif tool == "gh" and "release" in words(argv):
            found.append(" ".join(["gh", *argv]))
    return found


def check_no_dist_machinery(entry: Path, probe: Probe) -> None:
    """The release drives no dist step and no GitHub hosting: both are gone."""
    found = dist_machinery(probe.calls)
    if not found:
        return
    fail(
        f"{entry}: drove {found[0]!r} during the release; a release is the version "
        "substitution and `cargo publish`, and the prebuilt binaries, the installers and "
        "the GitHub hosting went with the build platform machinery the decision took out "
        f"(log: {probe.log.strip()[:400]!r})"
    )


def check_publishes_after_substitution(entry: Path, probe: Probe, requested: str) -> None:
    """The crate is published, and it is published after the version lands."""
    positions = publishes(probe.calls)
    if not positions:
        fail(
            f"{entry}: the run recorded no `cargo publish`; the release publishes the crate "
            f"after substituting the version ({probe.note})"
        )
        return
    observed = probe.publishes[0] if probe.publishes else ""
    if observed != requested:
        fail(
            f"{entry}: recorded `cargo publish` while {MANIFEST} carried {observed!r}; the "
            "release substitutes the named version first and publishes after, so the crate "
            f"is published at the version the caller named ({requested!r}), not at the "
            "version the tree carried before"
        )


def check_stepped_off(entry: Path, probe: Probe) -> None:
    """A refusal drives no step at all: no dist invocation, no publish."""
    found = dist_machinery(probe.calls)
    if found:
        fail(
            f"{entry}: drove {found[0]!r} although the release was refused; a release that "
            "cannot perform the recorded substitution must stop before it publishes or "
            f"builds (log: {probe.log.strip()[:400]!r})"
        )
    if publishes(probe.calls):
        fail(
            f"{entry}: recorded `cargo publish` although the release was refused; publishing "
            "a tree whose version the record does not carry releases the wrong version "
            f"(log: {probe.log.strip()[:400]!r})"
        )


def discover(
    token: str, entries: list[Path], parent: Path, stub_dir: Path, home: Path
) -> Invocation | None:
    """Scenario A: every documented interface form substitutes the named version.

    One run per form, each with its own distinct named version. A form no entry
    point accepts is named as such: the release documents all six, so a caller
    who names the version any one of those ways has to be served.
    """
    attempt = 0
    reference: Invocation | None = None
    for label, builder, version in FORMS:
        notes: list[str] = []
        conforming: Invocation | None = None
        for entry in entries:
            attempt += 1
            argv, env_extra, stdin_text = builder(version)
            tree = prepare(parent, f"a-{attempt}", manifest_version_value=token, token=token)
            if tree is None:
                return reference
            probe = run_entry(
                tree,
                stub_dir,
                parent / f"a-{attempt}.log",
                home,
                entry,
                argv,
                env_extra,
                stdin_text,
                version,
            )
            if probe.version != version:
                notes.append(
                    f"{entry} given the version as {label}: {probe.note}, "
                    f"{MANIFEST} version {probe.version!r}"
                )
                continue

            print(
                f"ok: {entry} substituted the manifest version when given it as {label} "
                f"({version!r})"
            )
            conforming = Invocation(entry, label)
            if reference is None:
                reference = conforming

            if probe.rc is None:
                fail(
                    f"{entry}: did not complete after substituting the version given it "
                    f"as {label} ({probe.note}); a release that cannot finish is not a release"
                )
            elif probe.rc != 0:
                fail(
                    f"{entry}: exited with status {probe.rc} after substituting the version "
                    f"given it as {label}; a non-zero exit is a failed release, not a "
                    f"conforming one ({probe.note})"
                )
            else:
                check_no_dist_machinery(entry, probe)
                check_publishes_after_substitution(entry, probe, version)
            break
        if conforming is None:
            fail(
                f"no entry point accepts the version as {label}: "
                f"{', '.join(str(e) for e in entries)} did not put {version!r} in the "
                f"manifest's [package] version (the recorded token is {token!r}); the release "
                "documents the version as a positional argument, as --version <v>, as "
                "--tag v<v>, as a bare v<v>, as VERSION=<v> in the environment, and on "
                "standard input, and every one of those forms has to substitute it"
                + ("\n  " + "\n  ".join(notes) if notes else "")
            )
    return reference


def check_record_drives_substitution(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path
) -> None:
    """Scenario B: a different recorded token is still the one substituted."""
    tree = prepare(
        parent,
        "b-alternate-token",
        manifest_version_value=SENTINEL_ALTERNATE,
        token=SENTINEL_ALTERNATE,
    )
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED)
    probe = run_entry(
        tree,
        stub_dir,
        parent / "b-alternate-token.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        REQUESTED,
    )
    if probe.version != REQUESTED:
        fail(
            f"{invocation.entry}: with {VERSION_NODE} recording {SENTINEL_ALTERNATE!r} and the "
            f"manifest carrying it, the manifest version is {probe.version!r} ({probe.note}); "
            "the substitution replaces the token the version decision records, so it must "
            "follow the record rather than a literal the entry point carries"
        )
        return
    if probe.rc != 0:
        fail(
            f"{invocation.entry}: exited with status {probe.rc} after substituting a token "
            f"other than the one built into it ({SENTINEL_ALTERNATE!r}); a non-zero exit is a "
            f"failed release ({probe.note})"
        )
        return
    check_no_dist_machinery(invocation.entry, probe)
    check_publishes_after_substitution(invocation.entry, probe, REQUESTED)
    print(f"ok: {invocation.entry} substituted the token the record holds verbatim")


def check_refuses_unrecorded_manifest(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path, token: str
) -> None:
    """Scenario C: the manifest does not carry the recorded token, so there is nothing to substitute."""
    tree = prepare(
        parent, "c-unrecorded", manifest_version_value=UNRECORDED, token=token
    )
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED)
    probe = run_entry(
        tree,
        stub_dir,
        parent / "c-unrecorded.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        REQUESTED,
    )
    if probe.rc is None or probe.rc == 0:
        fail(
            f"{invocation.entry}: exited {probe.rc} with {MANIFEST} at {UNRECORDED!r} while "
            f"{VERSION_NODE} records {token!r}; the entry point substitutes the recorded token, "
            "so a manifest that does not carry it must be refused, not overwritten"
        )
    if probe.version != UNRECORDED:
        fail(
            f"{invocation.entry}: rewrote the manifest's [package] version from {UNRECORDED!r} "
            f"to {probe.version!r} although the recorded token is {token!r}; that is a blind "
            "overwrite, not the recorded substitution"
        )
    check_stepped_off(invocation.entry, probe)
    if probe.version == UNRECORDED and probe.rc:
        print(f"ok: {invocation.entry} refused a manifest that does not carry the recorded token")


def check_refuses_without_record(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path, token: str
) -> None:
    """Scenario D: no version record at all, so there is no token to substitute."""
    tree = prepare(
        parent,
        "d-no-record",
        manifest_version_value=token,
        token=None,
        drop_record=True,
    )
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED)
    probe = run_entry(
        tree,
        stub_dir,
        parent / "d-no-record.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        REQUESTED,
    )
    if probe.rc is None or probe.rc == 0:
        fail(
            f"{invocation.entry}: exited {probe.rc} with {VERSION_NODE} absent; there is no "
            "recorded token to substitute, so the entry point must refuse rather than choose "
            "a version itself"
        )
    if probe.version != token:
        fail(
            f"{invocation.entry}: rewrote the manifest's [package] version to {probe.version!r} "
            f"with {VERSION_NODE} absent; the version it substitutes is the recorded token and "
            "nothing else"
        )
    check_stepped_off(invocation.entry, probe)
    if probe.version == token and probe.rc:
        print(f"ok: {invocation.entry} refused to substitute without the version record")


def check_follows_the_caller(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path, token: str
) -> None:
    """Scenario E: a second, different named version is the one that lands."""
    tree = prepare(parent, "e-second-version", manifest_version_value=token, token=token)
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED_ALTERNATE)
    probe = run_entry(
        tree,
        stub_dir,
        parent / "e-second-version.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        REQUESTED_ALTERNATE,
    )
    if probe.version != REQUESTED_ALTERNATE:
        fail(
            f"{invocation.entry}: asked for {REQUESTED_ALTERNATE!r} over a tree whose "
            f"{MANIFEST} carries the recorded token {token!r}, the manifest version is "
            f"{probe.version!r} ({probe.note}); the version that lands is the one the "
            "caller names, so a version built into the entry point is not the version it "
            "releases"
        )
        return
    if probe.rc != 0:
        fail(
            f"{invocation.entry}: exited {probe.rc} after substituting {REQUESTED_ALTERNATE!r}; "
            f"a non-zero exit is a failed release ({probe.note})"
        )
        return
    check_no_dist_machinery(invocation.entry, probe)
    check_publishes_after_substitution(invocation.entry, probe, REQUESTED_ALTERNATE)
    print(
        f"ok: {invocation.entry} substituted the version the caller named "
        f"({REQUESTED_ALTERNATE!r}, not {REQUESTED!r})"
    )


def check_already_at_the_version(
    invocation: Invocation,
    parent: Path,
    stub_dir: Path,
    home: Path,
    token: str,
    version: str,
    name: str,
) -> None:
    """Scenario F: the manifest already carries the requested version — the allowed no-op.

    Run once per version the probe asks for, so a no-op keyed to one literal
    version is not mistaken for the already-substituted path.
    """
    tree = prepare(parent, name, manifest_version_value=version, token=token)
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, version)
    probe = run_entry(
        tree,
        stub_dir,
        parent / f"{name}.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        version,
    )
    if probe.version != version:
        fail(
            f"{invocation.entry}: with {MANIFEST} already at {version!r}, the manifest "
            f"version afterwards is {probe.version!r} ({probe.note}); an already "
            "substituted manifest carries the requested version and nothing else"
        )
    if probe.rc is None or probe.rc != 0:
        fail(
            f"{invocation.entry}: exited {probe.rc} with {MANIFEST} already at "
            f"{version!r} ({VERSION_NODE} records the token {token!r}); the decision "
            "allows the substitution to be a no-op there, so that tree must still be "
            f"released, not refused ({probe.note})"
        )
        return
    check_no_dist_machinery(invocation.entry, probe)
    check_publishes_after_substitution(invocation.entry, probe, version)
    print(
        f"ok: {invocation.entry} released a manifest that already carried {version!r}"
    )


def check_stops_on_failing_step(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path, token: str
) -> None:
    """Scenario G: a failing publish fails the release and nothing follows it."""
    tree = prepare(parent, "g-failing-publish", manifest_version_value=token, token=token)
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED)
    env_extra = dict(env_extra) | {"PROBE_FAIL_TOOL": "cargo", "PROBE_FAIL_ARG": "publish"}
    probe = run_entry(
        tree,
        stub_dir,
        parent / "g-failing-publish.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        REQUESTED,
    )
    positions = publishes(probe.calls)
    if not positions:
        fail(
            f"{invocation.entry}: with the `cargo publish` stub failing, the run recorded no "
            "publish invocation at all, so the failed step is not the one under test "
            f"({probe.note}; log: {probe.log.strip()[:400]!r})"
        )
        return
    failing_at = positions[0]
    if probe.rc is None or probe.rc == 0:
        fail(
            f"{invocation.entry}: exited {probe.rc} although `cargo publish` failed; a release "
            "that goes on after a step fails is not a release, so the entry point's own exit "
            f"status must be non-zero ({probe.note})"
        )
    afterwards = probe.calls[failing_at + 1 :]
    later = [
        " ".join([tool, *argv])
        for tool, argv in afterwards
        if tool in {"cargo", "dist", "gh"}
    ]
    if later:
        fail(
            f"{invocation.entry}: drove {later[0]!r} after `cargo publish` failed; the "
            "release stops on the first step that fails — stopped is observed as no later "
            "step recorded after the failing invocation "
            f"(log: {probe.log.strip()[:400]!r})"
        )
    if probe.rc not in (None, 0) and not later:
        print(
            f"ok: {invocation.entry} exited non-zero on a failing `cargo publish` and drove no "
            "later step"
        )


def main() -> int:
    if sys.version_info < (3, 11):
        print("FAIL: python3 >= 3.11 is required to read TOML (tomllib)")
        return 1

    token = recorded_token()
    entries = entry_points()

    if token is None or not entries:
        return report()

    named = [
        *[
            (f"the version the {label} form asks for", version)
            for label, _, version in FORMS
        ],
        ("the second version scenario E asks for", REQUESTED_ALTERNATE),
        ("the unrecorded version scenario C plants", UNRECORDED),
    ]
    collisions = [label for label, value in named if value == token]
    if collisions:
        fail(
            f"{VERSION_NODE}: the recorded token is {token!r}, which is also {collisions[0]}; "
            "the probe cannot tell a substitution from the version it replaces"
        )
        return report()
    if len({value for _, value in named}) != len(named):
        fail(
            "the probe's own versions are not all distinct ("
            + ", ".join(repr(value) for _, value in named)
            + "), so its scenarios do not discriminate"
        )
        return report()

    with tempfile.TemporaryDirectory(prefix="deccy-release-probe-") as temporary:
        parent = Path(temporary)
        stub_dir = parent / "bin"
        home = parent / "home"
        home.mkdir()
        write_stubs(stub_dir)

        invocation = discover(token, entries, parent, stub_dir, home)
        if invocation is not None:
            check_record_drives_substitution(invocation, parent, stub_dir, home)
            check_refuses_unrecorded_manifest(invocation, parent, stub_dir, home, token)
            check_refuses_without_record(invocation, parent, stub_dir, home, token)
            check_follows_the_caller(invocation, parent, stub_dir, home, token)
            check_already_at_the_version(
                invocation,
                parent,
                stub_dir,
                home,
                token,
                REQUESTED,
                "f-already-substituted",
            )
            check_already_at_the_version(
                invocation,
                parent,
                stub_dir,
                home,
                token,
                REQUESTED_ALTERNATE,
                "f-already-substituted-alt",
            )
            check_stops_on_failing_step(invocation, parent, stub_dir, home, token)

    return report()


def report() -> int:
    if failures:
        print(f"\n{len(failures)} release check(s) failed")
        return 1

    print(
        "release entry point substitutes the token the version decision records for the "
        "version its caller names in every documented form, refuses a manifest that carries "
        "neither, is a no-op that still publishes when the manifest already carries the "
        "requested version, publishes the crate with `cargo publish` after the substitution, "
        "exits 0 on success, stops with a non-zero exit on a failing publish, drives no dist "
        "step and no GitHub hosting, and needs no release-layout file to exist"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
