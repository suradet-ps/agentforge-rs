#!/usr/bin/env bash
# Reproducible-build verification (roadmap Phase 10).
#
# Runs `cargo-agentforge init` twice in fresh directories with the same
# SOURCE_DATE_EPOCH and requires the produced `AGENTS-RUST.md` and
# `.agentforge.json` to be byte-identical between runs.
#
# Run from the repo root after `cargo build`:
#   bash scripts/repro-check.sh [path/to/cargo-agentforge]
#
# Any difference exits non-zero so CI fails the build.

set -euo pipefail

BIN="${1:-target/debug/cargo-agentforge}"

# Resolve to an absolute path so the runs work from any cwd.
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

if [ ! -x "$BIN" ]; then
  echo "error: $BIN not found (run 'cargo build' first)" >&2
  exit 2
fi

# Pin the manifest timestamp; both runs must derive the same value from it.
export SOURCE_DATE_EPOCH=1700000000

failed=0

compare() {
  local label="$1"; shift
  local a b
  a=$(mktemp -d)
  b=$(mktemp -d)
  (cd "$a" && "$BIN" init "$@" >/dev/null)
  (cd "$b" && "$BIN" init "$@" >/dev/null)
  local f
  for f in AGENTS-RUST.md .agentforge.json; do
    if cmp -s "$a/$f" "$b/$f"; then
      echo "ok: $label: $f byte-identical"
    else
      echo "FAIL: $label: $f differs between runs" >&2
      diff -u "$a/$f" "$b/$f" || true
      failed=1
    fi
  done
  rm -rf "$a" "$b"
}

echo "== core-only =="
compare "core-only"
echo "== all templates =="
compare "all-templates" --template wasm,tauri,bevy,embedded,axum,cli,library

if [ "$failed" -ne 0 ]; then
  echo "reproducible-build check FAILED" >&2
  exit 1
fi
echo "reproducible-build check passed"
