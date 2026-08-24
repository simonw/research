#!/usr/bin/env bash
# GitHub Pages build hook for this folder (run by ../run-github-pages-hooks.sh
# with this directory as the working directory, before rsync into _site).
#
# Publishes one generated artifact alongside the committed files:
#
#   nyc311-cube-v15.dcb2   the full real-data cube in DCB2      (~43MB)
#
# Building it means downloading the 40MB source Parquet and converting it,
# so this script first tries to reuse the copy the *previous* deploy
# published -- GitHub Pages doubles as the build cache. A version stamp
# (hash of the two converter scripts + the Parquet's ETag/Content-Length
# from a HEAD request) is published as cube-build-version.txt. When the
# published stamp matches, the published artifact is downloaded and shipped
# as-is; when it doesn't -- a converter changed, or the source data changed --
# everything is rebuilt from the Parquet.
#
# Failure policy: this hook never fails the site build. If neither the cache
# nor a rebuild works it emits ::warning:: annotations and exits 0 (demo.html
# will 404 until a later build succeeds) rather than blocking deploys of every
# other folder in this repo.
set -uo pipefail

PAGES_BASE="${PAGES_BASE:-https://simonw.github.io/research/http-range-drilldown-cubes}"
PARQUET_URL="${PARQUET_URL:-https://static.simonwillison.net/static/2026/nyc311-cube-v15.parquet}"
FULL=nyc311-cube-v15.dcb2
VFILE=cube-build-version.txt
CURL="curl -fsSL --retry 2 --max-time 300"

warn() {
  echo "::warning file=http-range-drilldown-cubes/github-pages.sh::$*"
  echo "WARNING: $*" >&2
}

# a plausible DCB2 file: right magic, more than 1MB
ok_dcb2() {
  [ -f "$1" ] && [ "$(head -c 4 "$1" 2>/dev/null)" = "DCB2" ] && [ "$(wc -c < "$1")" -gt 1000000 ]
}

# ---- current build version -------------------------------------------------
script_hash=$(sha256sum parquet_to_dcb1.py dcb1_to_dcb2.py | sha256sum | cut -c1-12)
parquet_headers=$($CURL -I "$PARQUET_URL" 2>/dev/null | tr -d '\r' | tr '[:upper:]' '[:lower:]' \
  | grep -E '^(etag|content-length):' | sort || true)
if [ -n "$parquet_headers" ]; then
  parquet_id=$(printf '%s' "$parquet_headers" | sha256sum | cut -c1-12)
else
  parquet_id=unknown
fi
VERSION="dcb2-demo-2-${script_hash}-${parquet_id}"
echo "cube build version: $VERSION"

# ---- 1. artifacts already present and current? (local rebuild loops) --------
if [ "$(cat "$VFILE" 2>/dev/null)" = "$VERSION" ] && ok_dcb2 "$FULL"; then
  echo "artifact already present and current -- nothing to do"
  exit 0
fi

# ---- 2. the previously published copies as the cache -------------------------
published=$($CURL "$PAGES_BASE/$VFILE" 2>/dev/null || true)
if [ "$published" = "$VERSION" ]; then
  echo "published version matches -- reusing artifact from $PAGES_BASE"
  if $CURL -o "$FULL.tmp" "$PAGES_BASE/$FULL" && ok_dcb2 "$FULL.tmp"; then
    mv "$FULL.tmp" "$FULL"
    printf '%s' "$VERSION" > "$VFILE"
    echo "reused published artifact: $(wc -c < "$FULL") bytes"
    exit 0
  fi
  echo "cache download failed -- falling back to a full rebuild"
  rm -f "$FULL.tmp"
else
  echo "no usable published cache (published version: ${published:-none}) -- full rebuild"
fi

# ---- 3. full rebuild from the source Parquet --------------------------------
if command -v uv >/dev/null 2>&1; then run() { uv run "$@"; }; else run() { python3 "$@"; }; fi
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

if ! $CURL -o "$tmp/cube.parquet" "$PARQUET_URL"; then
  rm -f "$FULL" "$VFILE"
  warn "could not download $PARQUET_URL and no published cache was usable; the cube file will be missing from this deploy"
  exit 0
fi
echo "downloaded parquet: $(wc -c < "$tmp/cube.parquet") bytes"

if run parquet_to_dcb1.py "$tmp/cube.parquet" "$tmp/cube.dcb1" \
  && run dcb1_to_dcb2.py "$tmp/cube.dcb1" "$FULL" --add-week-by-date \
  && ok_dcb2 "$FULL"; then
  printf '%s' "$VERSION" > "$VFILE"
  echo "rebuilt cube artifact: $(wc -c < "$FULL") bytes"
else
  rm -f "$FULL" "$VFILE"
  warn "cube conversion failed; the cube file will be missing from this deploy"
fi
exit 0
