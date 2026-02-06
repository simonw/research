#!/usr/bin/env bash
# Download Pyodide runtime files for local testing
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PYODIDE_DIR="$SCRIPT_DIR/pyodide"
PYODIDE_VERSION="0.29.3"
BASE_URL="https://cdn.jsdelivr.net/pyodide/v${PYODIDE_VERSION}/full"

mkdir -p "$PYODIDE_DIR"

FILES=(
    "pyodide.js"
    "pyodide.asm.js"
    "pyodide.asm.wasm"
    "python_stdlib.zip"
    "pyodide-lock.json"
    "micropip-0.11.0-py3-none-any.whl"
    "packaging-24.2-py3-none-any.whl"
)

for f in "${FILES[@]}"; do
    if [ ! -f "$PYODIDE_DIR/$f" ]; then
        echo "Downloading $f..."
        curl -sL "$BASE_URL/$f" -o "$PYODIDE_DIR/$f"
    else
        echo "Already exists: $f"
    fi
done

echo "Pyodide runtime files ready in $PYODIDE_DIR/"
