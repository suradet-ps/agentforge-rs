#!/usr/bin/env bash
# Performance budgets for cargo-agentforge.
#
# Run from the repo root after `cargo build --release`:
#   bash scripts/perf-check.sh [path/to/cargo-agentforge]
#
# Budgets (roadmap Phase 9):
#   stripped binary size      < 5 MB
#   `version` startup         < 100 ms  (median of 5 runs)
#   `init` core-only          < 50 ms   (median, warm filesystem)
#   `init` all templates      < 200 ms  (median, warm filesystem)
#   `version` resident memory < 20 MB
#
# Any budget miss exits non-zero so CI fails the build.

set -euo pipefail

BIN="${1:-target/release/cargo-agentforge}"

# Resolve to an absolute path so timing works from any cwd.
BIN="$(cd "$(dirname "$BIN")" && pwd)/$(basename "$BIN")"

if [ ! -x "$BIN" ]; then
  echo "error: $BIN not found (run 'cargo build --release' first)" >&2
  exit 2
fi

SIZE_LIMIT_MB=5
STARTUP_LIMIT_MS=100
INIT_CORE_LIMIT_MS=50
INIT_TEMPLATES_LIMIT_MS=200
MEM_LIMIT_MB=20

failed=0

stat_bytes() {
  if stat -c %s "$1" >/dev/null 2>&1; then
    stat -c %s "$1"
  else
    stat -f %z "$1"
  fi
}

# Run a command and print its wall time in milliseconds (portable via python3).
run_ms() {
  python3 - "$@" <<'PY'
import subprocess, sys, time
t0 = time.perf_counter()
subprocess.run(sys.argv[1:], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
print(int((time.perf_counter() - t0) * 1000))
PY
}

median_ms() {
  sort -n | awk '{ a[NR] = $1 } END { if (NR % 2) print a[(NR + 1) / 2]; else print int((a[NR / 2] + a[NR / 2 + 1]) / 2) }'
}

echo "== binary size (stripped) =="
strip -o /tmp/ag-perf-stripped "$BIN" 2>/dev/null || true
if [ -f /tmp/ag-perf-stripped ]; then
  SIZE_BYTES=$(stat_bytes /tmp/ag-perf-stripped)
  SIZE_MB=$((SIZE_BYTES / 1024 / 1024))
  echo "stripped size: ${SIZE_BYTES} bytes (${SIZE_MB} MB, limit ${SIZE_LIMIT_MB} MB)"
  if [ "$SIZE_MB" -gt "$SIZE_LIMIT_MB" ]; then
    echo "FAIL: binary size exceeds ${SIZE_LIMIT_MB} MB" >&2
    failed=1
  fi
else
  echo "note: strip unavailable; skipping size check"
fi

echo "== startup: 'version' (median of 5) =="
STARTUP_MS=$(for _ in 1 2 3 4 5; do run_ms "$BIN" version; done | median_ms)
echo "startup: ${STARTUP_MS} ms (limit ${STARTUP_LIMIT_MS} ms)"
if [ "$STARTUP_MS" -gt "$STARTUP_LIMIT_MS" ]; then
  echo "FAIL: startup exceeds ${STARTUP_LIMIT_MS} ms" >&2
  failed=1
fi

measure_init() {
  local label="$1"; shift
  local tmp; tmp=$(mktemp -d)
  (cd "$tmp" && "$BIN" init "$@" >/dev/null 2>&1)   # warm the filesystem
  local med
  med=$(for _ in 1 2 3; do
    (cd "$tmp" && rm -f AGENTS-RUST.md .agentforge.json && run_ms "$BIN" init "$@")
  done | median_ms)
  rm -rf "$tmp"
  echo "$med"
}

echo "== init: core-only (median, warm fs) =="
INIT_CORE_MS=$(measure_init "core")
echo "init core-only: ${INIT_CORE_MS} ms (limit ${INIT_CORE_LIMIT_MS} ms)"
if [ "$INIT_CORE_MS" -gt "$INIT_CORE_LIMIT_MS" ]; then
  echo "FAIL: init core-only exceeds ${INIT_CORE_LIMIT_MS} ms" >&2
  failed=1
fi

echo "== init: all templates (median, warm fs) =="
INIT_TMPL_MS=$(measure_init "templates" --template wasm,tauri,bevy,embedded,axum,cli,library)
echo "init all-templates: ${INIT_TMPL_MS} ms (limit ${INIT_TEMPLATES_LIMIT_MS} ms)"
if [ "$INIT_TMPL_MS" -gt "$INIT_TEMPLATES_LIMIT_MS" ]; then
  echo "FAIL: init all-templates exceeds ${INIT_TEMPLATES_LIMIT_MS} ms" >&2
  failed=1
fi

echo "== resident memory: 'version' =="
# GNU time (Linux): "Maximum resident set size (kbytes): N"
MEM_KB=$(/usr/bin/time -v "$BIN" version 2>&1 | sed -n 's/.*Maximum resident set size (kbytes): *\([0-9][0-9]*\).*/\1/p' | head -n1 || true)
if [ -z "$MEM_KB" ]; then
  # BSD time (macOS): "N  maximum resident set size" in bytes.
  MEM_BYTES=$(/usr/bin/time -l "$BIN" version 2>&1 | sed -n 's/ *\([0-9][0-9]*\) *maximum resident set size.*/\1/p' | head -n1 || true)
  if [ -n "$MEM_BYTES" ]; then
    MEM_KB=$((MEM_BYTES / 1024))
  fi
fi
if [ -n "$MEM_KB" ]; then
  MEM_MB=$((MEM_KB / 1024))
  echo "resident memory: ${MEM_MB} MB (limit ${MEM_LIMIT_MB} MB)"
  if [ "$MEM_MB" -gt "$MEM_LIMIT_MB" ]; then
    echo "FAIL: resident memory exceeds ${MEM_LIMIT_MB} MB" >&2
    failed=1
  fi
else
  echo "note: /usr/bin/time lacks -v/-l support; skipping memory check"
fi

if [ "$failed" -ne 0 ]; then
  echo "perf budgets FAILED" >&2
  exit 1
fi
echo "all perf budgets pass"