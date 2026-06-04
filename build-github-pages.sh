#!/usr/bin/env bash
set -euo pipefail

site_dir="_site"

bash ./run-github-pages-hooks.sh

rm -rf "$site_dir"
mkdir -p "$site_dir"

rsync -a ./ "$site_dir"/ \
  --exclude .git/ \
  --exclude .github/ \
  --exclude "$site_dir"/ \
  --exclude .DS_Store \
  --exclude __pycache__/ \
  --exclude .pytest_cache/

uv run python render-readme-index.py README.md "$site_dir/index.html"
touch "$site_dir/.nojekyll"
