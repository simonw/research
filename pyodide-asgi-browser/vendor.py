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

# Bundled packages (in the Pyodide lock) that make up the dependency closures of
# both FastAPI and Datasette, so micropip can resolve them from the *local* index
# instead of PyPI. (Their own deps are pulled in transitively from the lock.)
BUNDLED_SEED = [
    "micropip", "packaging",
    # FastAPI side
    "pydantic", "anyio", "idna", "sniffio",
    "typing-extensions", "annotated-types",
    # Datasette side (jinja2->markupsafe, httpx->httpcore/h11/certifi, etc.)
    "jinja2", "pyyaml", "httpx", "h11", "click",
    "pluggy", "platformdirs", "setuptools",
    # sqlite3 is unvendored from the stdlib in Pyodide; Datasette imports it.
    "sqlite3",
]

# Pure-Python packages NOT bundled by Pyodide; fetched from PyPI as wheels, then
# handed to micropip as direct URLs (so it never needs to reach PyPI). Each set
# gets its own install manifest consumed by the matching worker.
FASTAPI_PYPI = [
    "fastapi", "starlette", "python-multipart",
    "annotated-doc", "typing-inspection",
]
DATASETTE_PYPI = [
    "datasette", "aiofiles", "asgi-csrf", "asgiref", "click-default-group",
    "flexcache", "flexparser", "hupper", "itsdangerous", "janus", "mergedeep",
    "pip", "python-multipart", "uvicorn",
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


def download_pypi(packages, manifest):
    """Download pure-Python wheels for `packages` and write an install manifest
    listing exactly which wheels the worker should hand to micropip.install()."""
    print(f"PyPI wheels for {manifest}:")
    subprocess.check_call([
        sys.executable, "-m", "pip", "download",
        "--only-binary", ":all:", "--no-deps",
        "--dest", str(VENDOR), *packages,
    ])
    wheels = []
    for pkg in packages:
        prefix = pkg.replace("-", "_").lower()
        matches = sorted(
            p.name for p in VENDOR.glob("*.whl")
            if p.name.lower().startswith(prefix + "-")
        )
        assert matches, f"no wheel downloaded for {pkg}"
        wheels.append(matches[-1])
    (VENDOR / manifest).write_text(json.dumps({"wheels": wheels}, indent=2))
    print(f"  {manifest} wheels:", wheels)


def main():
    VENDOR.mkdir(exist_ok=True)
    print("Pyodide runtime:")
    for name in CORE_FILES:
        fetch(CDN + name, VENDOR / name)

    lock = json.loads((VENDOR / "pyodide-lock.json").read_text())
    print("Bundled dependency wheels:")
    for file_name in bundled_closure(lock):
        fetch(CDN + file_name, VENDOR / file_name)

    download_pypi(FASTAPI_PYPI, "install.json")
    download_pypi(DATASETTE_PYPI, "datasette.json")
    print("Done. vendor/ is ready.")


if __name__ == "__main__":
    main()
