#!/usr/bin/env bash
set -euo pipefail

scripts=()
while IFS= read -r script; do
  scripts+=("$script")
done < <(
  find . \
    -path ./.git -prune -o \
    -path ./_site -prune -o \
    -name github-pages.sh -type f -print \
    | sort
)

if (( ${#scripts[@]} == 0 )); then
  echo "No github-pages.sh scripts found"
fi

for script in "${scripts[@]}"; do
  dir="$(dirname "$script")"
  echo "::group::${script#./}"
  (cd "$dir" && bash ./github-pages.sh)
  echo "::endgroup::"
done
