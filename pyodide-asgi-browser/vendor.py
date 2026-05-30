#!/usr/bin/env python3
"""Download Pyodide + the wheels this demo needs into ./vendor (gitignored).

Why: in some sandboxes the *browser* has no outbound network even when the host
does, so loading Pyodide and the FastAPI wheels from a CDN fails inside the Web
Worker. Vendoring everything next to the app and serving it from the same
`http.server` makes the demo fully self-contained / offline-capable.

Run once before the browser tests:  python3 vendor.py
"""

import json
import subprocess
import sys
import urllib.request
from pathlib import Path

PYODIDE_VERSION = "0.28.3"
CDN = f"https://cdn.jsdelivr.net/pyodide/v{PYODIDE_VERSION}/full/"
VENDOR = Path(__file__).resolve().parent / "vendor"

# Pyodide runtime files loadPyodide() needs.
CORE_FILES = [
    "pyodide.js",
    "pyodide.asm.js",
    "pyodide.asm.wasm",
    "python_stdlib.zip",
    "pyodide-lock.json",
]

# Bundled packages (in the Pyodide lock) that make up FastAPI's dependency
# closure, so micropip can resolve them from the *local* index instead of PyPI.
BUNDLED_SEED = [
    "micropip", "packaging",
    "pydantic", "anyio", "idna", "sniffio",
    "typing-extensions", "annotated-types",
]

# Pure-Python packages NOT bundled by Pyodide; fetched from PyPI as wheels.
# (annotated-doc and typing-inspection are direct FastAPI 0.136 dependencies that
# Pyodide does not bundle, so micropip would otherwise try to reach PyPI for them.)
PYPI_PACKAGES = [
    "fastapi", "starlette", "python-multipart",
    "annotated-doc", "typing-inspection",
]


def fetch(url, dest):
    if dest.exists() and dest.stat().st_size > 0:
        print(f"  have {dest.name}")
        return
    print(f"  get  {dest.name}")
    with urllib.request.urlopen(url, timeout=120) as r:
        dest.write_bytes(r.read())


def bundled_closure(lock):
    pk = lock["packages"]
    seen = set()

    def visit(name):
        name = name.lower().replace("_", "-")
        if name in seen or name not in pk:
            return
        seen.add(name)
        for dep in pk[name].get("depends", []):
            visit(dep)

    for s in BUNDLED_SEED:
        visit(s)
    return sorted(pk[n]["file_name"] for n in seen)


def main():
    VENDOR.mkdir(exist_ok=True)
    print("Pyodide runtime:")
    for name in CORE_FILES:
        fetch(CDN + name, VENDOR / name)

    lock = json.loads((VENDOR / "pyodide-lock.json").read_text())
    print("Bundled dependency wheels:")
    for file_name in bundled_closure(lock):
        fetch(CDN + file_name, VENDOR / file_name)

    print("PyPI wheels (pure-Python):")
    subprocess.check_call([
        sys.executable, "-m", "pip", "download",
        "--only-binary", ":all:", "--no-deps",
        "--dest", str(VENDOR), *PYPI_PACKAGES,
    ])

    # Record exactly which wheels the worker should hand to micropip.install().
    wheels = []
    for pkg in PYPI_PACKAGES:
        prefix = pkg.replace("-", "_").lower()
        matches = sorted(
            p.name for p in VENDOR.glob("*.whl")
            if p.name.lower().startswith(prefix)
        )
        assert matches, f"no wheel downloaded for {pkg}"
        wheels.append(matches[-1])
    (VENDOR / "install.json").write_text(json.dumps({"wheels": wheels}, indent=2))
    print("install.json wheels:", wheels)
    print("Done. vendor/ is ready.")


if __name__ == "__main__":
    main()
