#!/usr/bin/env bash
# Build a live demo of the patched Datasette Lite into ./demo/ for GitHub Pages:
# check out upstream datasette-lite at the commit this work was based on and
# apply datasette-lite-asgi.diff to it.
set -euo pipefail

BASE_COMMIT=779b2d4da76a5dc2f1d9aea0d7c8d854f6e1497b
here="$(pwd)"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

git clone --quiet https://github.com/simonw/datasette-lite "$work/datasette-lite"
cd "$work/datasette-lite"
git checkout --quiet "$BASE_COMMIT"
git apply "$here/datasette-lite-asgi.diff"

rm -rf "$here/demo"
mkdir -p "$here/demo/tests"
cp index.html shell.js sw.js webworker.js "$here/demo/"
cp -r tests/fixtures "$here/demo/tests/"
echo "Built $here/demo"
