#!/usr/bin/env bash
# Unsafe-code policy check (roadmap Phase 10).
#
# The workspace forbids `unsafe` code at the lint level. That only holds when
# both halves stay in place:
#   [workspace.lints.rust]  unsafe_code = "forbid"   (root Cargo.toml)
#   [lints] workspace = true                         (every crate)
#
# Run from the repo root:
#   bash scripts/unsafe-audit.sh
#
# Any violation exits non-zero so CI fails the build.

set -euo pipefail

failed=0

section_has() {
  # section_has <file> <section-header> <key-pattern>
  awk -v header="$2" -v key="$3" '
    $0 == header { in_section = 1; next }
    /^\[/ { in_section = 0 }
    in_section && $0 ~ key { found = 1 }
    END { exit(found ? 0 : 1) }
  ' "$1"
}

if ! section_has Cargo.toml "[workspace.lints.rust]" '^unsafe_code *= *"forbid"$'; then
  echo "FAIL: Cargo.toml is missing [workspace.lints.rust] unsafe_code = \"forbid\"" >&2
  failed=1
fi

for manifest in crates/*/Cargo.toml; do
  if ! section_has "$manifest" "[lints]" '^workspace *= *true$'; then
    echo "FAIL: $manifest is missing [lints] workspace = true" >&2
    failed=1
  fi
done

if [ "$failed" -ne 0 ]; then
  echo "unsafe-code policy FAILED" >&2
  exit 1
fi

echo "unsafe-code policy holds: every crate inherits unsafe_code = \"forbid\""
