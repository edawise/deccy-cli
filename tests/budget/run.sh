#!/usr/bin/env bash
# E-read-budget: measure the substrate crossings of `deccy log` and `deccy validate` on a
# generated fixture. The graph under measurement is the fixture; this checkout supplies only
# the binary. The assertion is that the `jj` and `git` invocation counts do not grow from N1
# to N2. Wall time is reported, not asserted.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TARGET="${CARGO_TARGET_DIR:-$ROOT/target}"
BIN="$TARGET/debug/deccy"
REAL_JJ="$(command -v jj)"
REAL_GIT="$(command -v git)"
N1=20
N2=100

cargo build --manifest-path "$ROOT/Cargo.toml" --quiet
if [ ! -x "$BIN" ]; then
  echo "missing binary: $BIN" >&2
  exit 1
fi

TMP="$(mktemp -d "${TMPDIR:-/tmp}/deccy-budget.XXXXXX")"
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT

# Counting shims first on PATH while the verbs run; fixture creation uses the real binaries.
mkdir -p "$TMP/bin"
cat > "$TMP/bin/jj" <<EOF
#!/bin/sh
printf 'jj\n' >> "$TMP/count"
exec "$REAL_JJ" "\$@"
EOF
cat > "$TMP/bin/git" <<EOF
#!/bin/sh
printf 'git\n' >> "$TMP/count"
exec "$REAL_GIT" "\$@"
EOF
chmod +x "$TMP/bin/jj" "$TMP/bin/git"

# `dir` gets `n` real changes, each owning `nodes/<its id>.json`; each node depends on up to
# four earlier nodes (the first four are shallower).
make_fixture() {
  local dir="$1" n="$2"
  mkdir -p "$dir/nodes"
  jj git init --colocate "$dir" >/dev/null 2>&1
  (
    cd "$dir"
    local -a ids=()
    local i j k depjson
    for i in $(seq 1 "$n"); do
      jj new -m "node $i" >/dev/null 2>&1
      local id
      id="$(jj log -r @ --no-graph -T 'change_id.shortest(8)')"
      local deps=()
      for j in 1 2 3 4; do
        k=$((i - j))
        if [ "$k" -ge 1 ]; then
          deps+=("\"${ids[$k]}\"")
        fi
      done
      if [ "${#deps[@]}" -eq 0 ]; then
        depjson='"zzzzzzzz"'
      else
        depjson="$(IFS=,; echo "${deps[*]}")"
      fi
      printf '{"kind":"decision","depends_on":[%s],"content":{"type":"text","ref":"inline","content":"node %s","rationale":"budget fixture"}}\n' \
        "$depjson" "$i" > "nodes/$id.json"
      ids[$i]="$id"
    done
    # Snapshot the last document into its own change.
    jj status >/dev/null 2>&1
  )
}

# Run both verbs in `dir` and print "<jj-count> <git-count>".
count_crossings() {
  local dir="$1"
  : > "$TMP/count"
  (
    cd "$dir"
    PATH="$TMP/bin:$PATH" "$BIN" log >/dev/null
    PATH="$TMP/bin:$PATH" "$BIN" validate >/dev/null
  )
  local jj_count git_count
  jj_count="$(grep -c '^jj$' "$TMP/count" || true)"
  git_count="$(grep -c '^git$' "$TMP/count" || true)"
  printf '%s %s\n' "$jj_count" "$git_count"
}

start="$(date +%s)"
make_fixture "$TMP/n1" "$N1"
make_fixture "$TMP/n2" "$N2"
read -r jj1 git1 <<< "$(count_crossings "$TMP/n1")"
read -r jj2 git2 <<< "$(count_crossings "$TMP/n2")"
end="$(date +%s)"

printf 'N1=%s nodes: jj=%s git=%s\n' "$N1" "$jj1" "$git1"
printf 'N2=%s nodes: jj=%s git=%s\n' "$N2" "$jj2" "$git2"
printf 'wall time: %ss\n' "$((end - start))"

if [ "$jj1" -ne "$jj2" ] || [ "$git1" -ne "$git2" ]; then
  echo "crossings grow with the graph: N1 jj=$jj1 git=$git1; N2 jj=$jj2 git=$git2" >&2
  exit 1
fi
