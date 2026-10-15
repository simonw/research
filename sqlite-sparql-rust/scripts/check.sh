#!/usr/bin/env bash
# The one command that runs every gate (docs/SPEC.md §1.7).
# Usage: scripts/check.sh [quick]     (quick = Rust tests + ext build + gates only)
set -euo pipefail
cd "$(dirname "$0")/.."
SCRATCH=${SCRATCH:-/tmp/claude-0/-home-user-research/82148577-c93b-5a93-8757-e7fe894ecb5e/scratchpad}
VENV=${VENV:-$SCRATCH/venv}
W3C=${W3C:-$SCRATCH/w3c/rdf-tests/sparql}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$SCRATCH/targets/check}
EXT_TARGET=${EXT_TARGET:-$SCRATCH/targets/ext}

cargo test --workspace --release                                   # native, system libsqlite3 3.45.1
CARGO_TARGET_DIR=$EXT_TARGET cargo build --release --manifest-path ext/Cargo.toml
if CARGO_TARGET_DIR=$EXT_TARGET cargo tree --manifest-path ext/Cargo.toml -e features -i libsqlite3-sys | grep -E 'bundled|modern_sqlite'; then
  echo "GATE FAILED: bundled/modern_sqlite leaked into the loadable extension" >&2; exit 1
fi
if grep -rnE 'total_changes64|is_interrupted|changes64|vtab_in\b|vtab_in_|rhs_value' crates/ ext/src tools/; then
  echo "GATE FAILED: SQLite API outside the 3.34.1 loadable-extension allow-list" >&2; exit 1
fi
export SPARQLITE_EXT=$EXT_TARGET/release/libsparqlite
[ "${1:-}" = quick ] && exit 0
"$VENV/bin/python" -m pip install -q pytest >/dev/null 2>&1 || true
"$VENV/bin/python" -m pytest py/tests -q
cargo run --release -p sq-w3c -- --suite "$W3C" --backend sqlite --known-failures tools/sq-w3c/known_failures.txt
cargo run --release -p sq-w3c -- --suite "$W3C" --backend mem    --known-failures tools/sq-w3c/known_failures.txt
"$VENV/bin/python" py/diff_oxigraph.py --ext "$SPARQLITE_EXT" --cases 2000 --seed 1
