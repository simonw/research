#!/usr/bin/env python3
"""Inventory local Pyodide/Emscripten wheel artifacts in this research repo."""

from __future__ import annotations

import json
from pathlib import Path


EXCLUDED_DIRS = {
    ".git",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
    "_site",
    "pyodide-wasm-wheel-projects",
}


def iter_wheels(root: Path):
    for path in root.rglob("*.whl"):
        if any(part in EXCLUDED_DIRS for part in path.parts):
            continue
        yield path


def classify_wheel(path: Path) -> str:
    name = path.name.lower()
    if "emscripten" in name or "pyodide" in name or "wasm32" in name:
        return "pyodide-wasm"
    return "other"


def main() -> None:
    repo_root = Path(__file__).resolve().parents[1]
    rows = []
    for wheel in sorted(iter_wheels(repo_root)):
        rel = wheel.relative_to(repo_root)
        rows.append(
            {
                "project": rel.parts[0],
                "path": str(rel),
                "filename": wheel.name,
                "bytes": wheel.stat().st_size,
                "classification": classify_wheel(wheel),
            }
        )

    print(json.dumps(rows, indent=2))


if __name__ == "__main__":
    main()
