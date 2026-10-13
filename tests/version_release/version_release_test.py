#!/usr/bin/env python3
"""Release probe for E-M-release (kzoqworr): one local script releases, no CI does.

The release decision (M-release, nodes/wtpwmkqr.json) is that replacing the
version token with a named version and driving build, release and publish is a
local step, not a generated workflow:

  * one executable entry point under `scripts/` takes the named version, puts it
    in the manifest's `[package] version` in place of *the token the version
    decision (nodes/nlomlqum.json) records*, and then drives `dist` — build,
    release and publish — reading `dist-workspace.toml`;
  * `dist-workspace.toml` stays and still names the four platforms and the two
    installers the decision lists;
  * the generated workflow set is gone. The decision removes the whole generated
    set, so the repository carries no CI; nothing in CI calls the entry point and
    this probe requires no workflow to exist or to call the script.

The probe runs the entry point in throwaway copies of the tree with `dist`,
`cargo`, `git`, `jj`, `gh`, `curl` and `wget` replaced by recording stubs: no
build, no network, no repository is touched. The copies differ in what the
version record and the manifest carry, so what the entry point does depends on
the recorded token and on nothing else:

  A. recorded token T in `nodes/nlomlqum.json` and T in the manifest — the state
     the version decision describes for a tree between releases. The entry point
     must substitute the requested version, exit 0, and record dist invocations
     that cover an actual build, a release and a publish. `dist plan` plans a
     build and builds nothing, so it is not credited as `dist build`. A non-zero
     exit is a failed release, named as such. Run once per documented interface
     form, each with its own distinct requested version, so an entry point that
     accepts only one of the forms the release documents is named as such.
  B. a token T' the record holds in place of T, with the manifest carrying T'.
     The substitution must still happen: it is driven by the recorded token, not
     by a literal in the entry point.
  C. the record still holds T but the manifest carries a named version the
     record does not carry. The entry point must refuse — non-zero exit, the
     manifest left as it was, no dist invocation — rather than blindly
     overwriting the version it finds.
  D. the version record absent. The entry point must refuse for the same reason:
     there is no recorded token to substitute.
  E. the substitution asked for a second, different named version over the tree
     scenario A releases. What lands is the version the caller names, so a
     version built into the entry point is not the version that is released.
  F. the manifest already carries the requested version — the one path the
     decision allows the entry point to be a no-op on, and the only path that
     can release the tree as it stands. The entry point must exit 0 and still
     drive build, release and publish; refusing that tree would leave it
     unreleasable. Run for each version A and E ask for, so a no-op keyed to
     one literal version is not mistaken for the already-substituted path.
  G. the `dist build` stub exits non-zero. The entry point must exit non-zero
     and stop. "Stopped" is observable, not inferred: after the failing
     invocation the recording stubs see no later step — no release step and no
     publish step — so the steps after the failure are never driven.

`dist-workspace.toml` is byte-identical after every run that substitutes or
no-ops: the machinery is read, never regenerated or rewritten.

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

The steps of a release are ordered: build, then the release step, then the
publish. A publish that runs before the build releases artifacts that were never
built, so the recorded order is checked and the pair that is out of order is
named as such.

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
DIST_CONFIG = Path("dist-workspace.toml")
MANIFEST = Path("Cargo.toml")
SCRIPTS_DIR = Path("scripts")
WORKFLOWS_DIR = Path(".github") / "workflows"

# The four target platforms and the two installers the release decision lists.
TARGETS = {
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "aarch64-unknown-linux-gnu",
    "x86_64-unknown-linux-gnu",
}
INSTALLERS = {"shell", "homebrew"}

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
STUBBED_TOOLS = ("dist", "cargo", "git", "jj", "gh", "curl", "wget")

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
# What one sandboxed run left behind.
Probe = namedtuple("Probe", "log rc version note")


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


def check_dist_plan() -> None:
    """dist-workspace.toml stays, with the platforms and installers the decision lists."""
    location = ROOT / DIST_CONFIG
    if not location.is_file():
        fail(
            f"{DIST_CONFIG}: missing; the release decision keeps the dist machinery there "
            "and the entry point reads it"
        )
        return
    try:
        document = tomllib.loads(location.read_text(encoding="utf-8"))
    except tomllib.TOMLDecodeError as error:
        fail(f"{DIST_CONFIG}: not valid TOML ({error})")
        return
    dist = document.get("dist")
    if not isinstance(dist, dict):
        fail(f"{DIST_CONFIG}: has no [dist] table for the release plan to read")
        return

    targets = dist.get("targets")
    if not isinstance(targets, list) or not all(isinstance(t, str) for t in targets):
        fail(f"{DIST_CONFIG}: [dist] declares no target list")
        return
    missing = sorted(TARGETS - set(targets))
    extra = sorted(set(targets) - TARGETS)
    if missing:
        fail(f"{DIST_CONFIG}: dist plan is missing the platform(s) {', '.join(missing)}")
    if extra:
        fail(f"{DIST_CONFIG}: dist plan names platform(s) the decision does not: {', '.join(extra)}")
    duplicated = sorted({t for t in targets if targets.count(t) > 1})
    if duplicated:
        fail(f"{DIST_CONFIG}: dist plan lists {', '.join(duplicated)} more than once")

    installers = dist.get("installers")
    if not isinstance(installers, list) or not all(isinstance(i, str) for i in installers):
        fail(f"{DIST_CONFIG}: [dist] declares no installer list")
        return
    missing = sorted(INSTALLERS - set(installers))
    extra = sorted(set(installers) - INSTALLERS)
    if missing:
        fail(f"{DIST_CONFIG}: dist plan is missing the installer(s) {', '.join(missing)}")
    if extra:
        fail(f"{DIST_CONFIG}: dist plan names installer(s) the decision does not: {', '.join(extra)}")


def check_no_ci_workflows() -> None:
    """The generated workflow set is gone: the decision removes all of it."""
    directory = ROOT / WORKFLOWS_DIR
    if not directory.is_dir():
        return
    generated = sorted(
        str(path.relative_to(ROOT)) for path in directory.rglob("*") if path.is_file()
    )
    if generated:
        fail(
            f"{WORKFLOWS_DIR}: still carries the generated workflow(s) "
            f"{', '.join(generated)}; the release is the local entry point's job and the "
            "decision removes the workflow set"
        )


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
    """A throwaway copy of the tree with the version record and manifest the scenario needs."""
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
    return Probe(log=log, rc=rc, version=manifest_version(tree), note=note)


def invocations(log: str) -> list[tuple[str, list[str]]]:
    """Parse the stub log into (tool, arguments), folding `cargo dist` into dist."""
    parsed: list[tuple[str, list[str]]] = []
    for line in log.splitlines():
        if not line.strip():
            continue
        fields = line.split("\t")
        tool, argv = fields[0], [field for field in fields[1:] if field]
        if tool == "cargo" and argv and argv[0] == "dist":
            tool, argv = "dist", argv[1:]
        parsed.append((tool, argv))
    return parsed


def words(argv: list[str]) -> list[str]:
    """The non-flag arguments of a recorded invocation."""
    return [arg for arg in argv if not arg.startswith("-")]


def covers(calls: list[tuple[str, list[str]]]) -> dict[str, bool]:
    """Which of build / release / publish the recorded invocations actually drive.

    A build is credited only to an invocation that names `build`:
    `dist build` builds, `dist plan` only plans one.
    """
    found = {"build": False, "release": False, "publish": False}
    for tool, argv in calls:
        arguments = words(argv)
        if tool == "dist":
            if "build" in arguments:
                found["build"] = True
            if "host" in arguments or "release" in arguments or any(
                arg.startswith("--steps") for arg in argv
            ):
                found["release"] = True
            if "publish" in arguments:
                found["publish"] = True
        elif tool == "cargo":
            if "publish" in arguments:
                found["publish"] = True
        elif tool == "gh":
            if "release" in arguments:
                found["release"] = True
    return found


def planned_only(calls: list[tuple[str, list[str]]]) -> bool:
    """Whether the run only planned a dist build (`dist plan`) and never built."""
    return any(
        tool == "dist" and arguments[:1] == ["plan"] for tool, arguments in calls
    )


# The steps a release drives, in the order it drives them.
ORDERED_STEPS = ("build", "release", "publish")


def step_positions(calls: list[tuple[str, list[str]]]) -> dict[str, int | None]:
    """Where each release step is first driven in the recorded invocations."""
    positions: dict[str, int | None] = {step: None for step in ORDERED_STEPS}
    for index, (tool, argv) in enumerate(calls):
        driven = covers([(tool, argv)])
        for step in ORDERED_STEPS:
            if driven[step] and positions[step] is None:
                positions[step] = index
    return positions


def check_step_order(
    entry: Path, calls: list[tuple[str, list[str]]], log: str
) -> None:
    """The steps are driven in the order build -> release -> publish."""
    positions = step_positions(calls)
    for earlier_index, earlier in enumerate(ORDERED_STEPS):
        for later in ORDERED_STEPS[earlier_index + 1 :]:
            first, second = positions[earlier], positions[later]
            if first is None or second is None or second > first:
                continue
            fail(
                f"{entry}: recorded the {later} step before the {earlier} step; the "
                "release drives build, then release, then publish, so a step that "
                "runs ahead of the one it depends on releases what was never built "
                f"(log: {log.strip()[:400]!r})"
            )
            return


def check_dist_config_untouched(entry: Path, tree: Path) -> None:
    """The dist machinery is read, never regenerated or rewritten."""
    dist_config = tree / DIST_CONFIG
    if not dist_config.is_file() or not (ROOT / DIST_CONFIG).is_file():
        return
    if dist_config.read_bytes() != (ROOT / DIST_CONFIG).read_bytes():
        fail(
            f"{entry}: rewrote {DIST_CONFIG}; the release reads the dist "
            "machinery rather than regenerating it"
        )


def check_steps_driven(entry: Path, calls: list[tuple[str, list[str]]], log: str) -> None:
    """The run drives an actual build, a release and a publish; `dist plan` is not a build."""
    found = covers(calls)
    if not found["build"]:
        if planned_only(calls):
            fail(
                f"{entry}: recorded only `dist plan`, which plans a build and builds "
                "nothing; the release drives an actual `dist build` "
                f"(log: {log.strip()[:400]!r})"
            )
        else:
            fail(
                f"{entry}: the run recorded no build invocation; the release drives "
                f"build, release and publish (log: {log.strip()[:400]!r})"
            )
    for step in ("release", "publish"):
        if not found[step]:
            fail(
                f"{entry}: the run recorded no dist step for {step}; the release "
                f"drives build, release and publish (log: {log.strip()[:400]!r})"
            )
    check_step_order(entry, calls, log)


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
            result = run_entry(
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
            if result.version != version:
                notes.append(
                    f"{entry} given the version as {label}: {result.note}, "
                    f"{MANIFEST} version {result.version!r}"
                )
                continue

            print(
                f"ok: {entry} substituted the manifest version when given it as {label} "
                f"({version!r})"
            )
            conforming = Invocation(entry, label)
            if reference is None:
                reference = conforming

            if result.rc is None:
                fail(
                    f"{entry}: did not complete after substituting the version given it "
                    f"as {label} ({result.note}); a release that cannot finish is not a release"
                )
            elif result.rc != 0:
                fail(
                    f"{entry}: exited with status {result.rc} after substituting the version "
                    f"given it as {label}; a non-zero exit is a failed release, not a "
                    f"conforming one ({result.note})"
                )
            else:
                check_dist_config_untouched(entry, tree)
                check_steps_driven(entry, invocations(result.log), result.log)
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
    result = run_entry(
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
    if result.version != REQUESTED:
        fail(
            f"{invocation.entry}: with {VERSION_NODE} recording {SENTINEL_ALTERNATE!r} and the "
            f"manifest carrying it, the manifest version is {result.version!r} ({result.note}); "
            "the substitution replaces the token the version decision records, so it must "
            "follow the record rather than a literal the entry point carries"
        )
        return
    if result.rc != 0:
        fail(
            f"{invocation.entry}: exited with status {result.rc} after substituting a token "
            f"other than the one built into it ({SENTINEL_ALTERNATE!r}); a non-zero exit is a "
            f"failed release ({result.note})"
        )
        return
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
    result = run_entry(
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
    if result.rc is None or result.rc == 0:
        fail(
            f"{invocation.entry}: exited {result.rc} with {MANIFEST} at {UNRECORDED!r} while "
            f"{VERSION_NODE} records {token!r}; the entry point substitutes the recorded token, "
            "so a manifest that does not carry it must be refused, not overwritten"
        )
    if result.version != UNRECORDED:
        fail(
            f"{invocation.entry}: rewrote the manifest's [package] version from {UNRECORDED!r} "
            f"to {result.version!r} although the recorded token is {token!r}; that is a blind "
            "overwrite, not the recorded substitution"
        )
    calls = invocations(result.log)
    if any(tool == "dist" for tool, _ in calls):
        fail(
            f"{invocation.entry}: drove dist although the recorded token was not in the "
            f"manifest; a release that cannot perform the recorded substitution must stop "
            f"before building (log: {result.log.strip()[:400]!r})"
        )
    if result.version == UNRECORDED and result.rc:
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
    result = run_entry(
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
    if result.rc is None or result.rc == 0:
        fail(
            f"{invocation.entry}: exited {result.rc} with {VERSION_NODE} absent; there is no "
            "recorded token to substitute, so the entry point must refuse rather than choose "
            "a version itself"
        )
    if result.version != token:
        fail(
            f"{invocation.entry}: rewrote the manifest's [package] version to {result.version!r} "
            f"with {VERSION_NODE} absent; the version it substitutes is the recorded token and "
            "nothing else"
        )
    if any(tool == "dist" for tool, _ in invocations(result.log)):
        fail(
            f"{invocation.entry}: drove dist with {VERSION_NODE} absent; a release that cannot "
            "perform the recorded substitution must stop before building"
        )
    if result.version == token and result.rc:
        print(f"ok: {invocation.entry} refused to substitute without the version record")


def check_follows_the_caller(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path, token: str
) -> None:
    """Scenario E: a second, different named version is the one that lands."""
    tree = prepare(parent, "e-second-version", manifest_version_value=token, token=token)
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED_ALTERNATE)
    result = run_entry(
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
    if result.version != REQUESTED_ALTERNATE:
        fail(
            f"{invocation.entry}: asked for {REQUESTED_ALTERNATE!r} over a tree whose "
            f"{MANIFEST} carries the recorded token {token!r}, the manifest version is "
            f"{result.version!r} ({result.note}); the version that lands is the one the "
            "caller names, so a version built into the entry point is not the version it "
            "releases"
        )
        return
    if result.rc is None or result.rc != 0:
        fail(
            f"{invocation.entry}: exited {result.rc} after substituting {REQUESTED_ALTERNATE!r}; "
            f"a non-zero exit is a failed release ({result.note})"
        )
        return
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
    result = run_entry(
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
    if result.version != version:
        fail(
            f"{invocation.entry}: with {MANIFEST} already at {version!r}, the manifest "
            f"version afterwards is {result.version!r} ({result.note}); an already "
            "substituted manifest carries the requested version and nothing else"
        )
    if result.rc is None or result.rc != 0:
        fail(
            f"{invocation.entry}: exited {result.rc} with {MANIFEST} already at "
            f"{version!r} ({VERSION_NODE} records the token {token!r}); the decision "
            "allows the substitution to be a no-op there, so that tree must still be "
            f"released, not refused ({result.note})"
        )
        return
    check_dist_config_untouched(invocation.entry, tree)
    check_steps_driven(invocation.entry, invocations(result.log), result.log)
    print(
        f"ok: {invocation.entry} released a manifest that already carried {version!r}"
    )


def check_stops_on_failing_step(
    invocation: Invocation, parent: Path, stub_dir: Path, home: Path, token: str
) -> None:
    """Scenario G: a failing step fails the release and the later steps are never driven."""
    tree = prepare(parent, "g-failing-build", manifest_version_value=token, token=token)
    if tree is None:
        return
    argv, env_extra, stdin_text = invocation_for(invocation, REQUESTED)
    env_extra = dict(env_extra) | {"PROBE_FAIL_TOOL": "dist", "PROBE_FAIL_ARG": "build"}
    result = run_entry(
        tree,
        stub_dir,
        parent / "g-failing-build.log",
        home,
        invocation.entry,
        argv,
        env_extra,
        stdin_text,
        REQUESTED,
    )
    calls = invocations(result.log)
    failing_at = None
    for index, (tool, arguments) in enumerate(calls):
        if tool == "dist" and "build" in words(arguments):
            failing_at = index
            break
    if failing_at is None:
        fail(
            f"{invocation.entry}: with the `dist build` stub failing, the run recorded no "
            "build invocation at all, so the failed step is not the one under test "
            f"({result.note}; log: {result.log.strip()[:400]!r})"
        )
        return
    if result.rc is None or result.rc == 0:
        fail(
            f"{invocation.entry}: exited {result.rc} although `dist build` failed; a release "
            "that goes on after a step fails is not a release, so the entry point's own "
            f"exit status must be non-zero ({result.note})"
        )
    afterwards = covers(calls[failing_at + 1 :])
    driven = [step for step in ("build", "release", "publish") if afterwards[step]]
    if driven:
        fail(
            f"{invocation.entry}: drove {', '.join(driven)} after `dist build` failed; the "
            "release stops on the first step that fails — stopped is observed as no later "
            f"step recorded after the failing invocation (log: {result.log.strip()[:400]!r})"
        )
    if result.rc not in (None, 0) and not driven:
        print(
            f"ok: {invocation.entry} exited non-zero on a failing `dist build` and drove no "
            "later step"
        )


def main() -> int:
    if sys.version_info < (3, 11):
        print("FAIL: python3 >= 3.11 is required to read TOML (tomllib)")
        return 1

    token = recorded_token()
    check_dist_plan()
    check_no_ci_workflows()
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
        "neither, is a no-op that still drives the steps when the manifest already carries "
        "the requested version, builds (not plans) and drives release and publish in the "
        "order build then release then publish from dist-workspace.toml, exits 0 on "
        "success, and stops with a non-zero exit on the first failing step; no generated "
        "workflow remains"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
