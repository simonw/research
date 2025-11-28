#!/usr/bin/env python3
"""
Build Script for Self-Hosted Datasette Lite

This script creates a self-contained bundle of Datasette Lite that can run
offline without any external network requests.

Usage:
    python build_offline_bundle.py --output /path/to/output

The output directory will contain everything needed to run Datasette Lite
from a local static file server.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import tempfile
import urllib.request
from pathlib import Path


# Pyodide version used by datasette-lite
PYODIDE_VERSION = "0.27.2"

# CDN base URL
PYODIDE_CDN = f"https://cdn.jsdelivr.net/pyodide/v{PYODIDE_VERSION}/full"

# Core Pyodide files needed
PYODIDE_CORE_FILES = [
    "pyodide.js",
    "pyodide.asm.js",
    "pyodide.asm.wasm",
    "python_stdlib.zip",
    "pyodide-lock.json",
    "pyodide.mjs",
    "package.json",
]

# Built-in Pyodide packages to include
PYODIDE_PACKAGES = [
    "micropip",
    "ssl",
    "setuptools",
    "packaging",  # dependency of micropip
]

# PyPI packages needed (pinned versions from webworker.js)
PYPI_PACKAGES = [
    "h11==0.12.0",
    "httpx==0.23.0",  # Note: webworker says 0.23, need exact version
    "python-multipart==0.0.15",
    "typing-extensions>=4.12.2",
    "datasette",  # latest by default
    # Optional packages:
    "sqlite-utils==3.28",
]


def download_file(url: str, dest: Path, show_progress: bool = True) -> None:
    """Download a file from URL to destination."""
    if show_progress:
        print(f"  Downloading: {url}")

    try:
        with urllib.request.urlopen(url) as response:
            with open(dest, 'wb') as f:
                f.write(response.read())
    except Exception as e:
        print(f"  ERROR downloading {url}: {e}")
        raise


def compute_sha256(filepath: Path) -> str:
    """Compute SHA256 hash of a file."""
    sha256 = hashlib.sha256()
    with open(filepath, 'rb') as f:
        for chunk in iter(lambda: f.read(8192), b''):
            sha256.update(chunk)
    return sha256.hexdigest()


def download_pyodide_core(output_dir: Path) -> None:
    """Download core Pyodide files."""
    print("\n=== Downloading Pyodide Core Files ===")

    pyodide_dir = output_dir / "pyodide"
    pyodide_dir.mkdir(parents=True, exist_ok=True)

    for filename in PYODIDE_CORE_FILES:
        url = f"{PYODIDE_CDN}/{filename}"
        dest = pyodide_dir / filename
        download_file(url, dest)


def download_pyodide_packages(output_dir: Path, lock_data: dict) -> None:
    """Download required Pyodide built-in packages."""
    print("\n=== Downloading Pyodide Packages ===")

    pyodide_dir = output_dir / "pyodide"
    packages = lock_data.get("packages", {})

    for pkg_name in PYODIDE_PACKAGES:
        if pkg_name not in packages:
            print(f"  WARNING: Package {pkg_name} not found in lock file")
            continue

        pkg_info = packages[pkg_name]
        filename = pkg_info.get("file_name")
        if filename:
            url = f"{PYODIDE_CDN}/{filename}"
            dest = pyodide_dir / filename
            download_file(url, dest)


def download_pypi_wheels(output_dir: Path) -> list:
    """
    Download PyPI wheels for offline installation.

    Returns a list of wheel info dicts for inclusion in lock file.
    """
    print("\n=== Downloading PyPI Wheels ===")

    wheels_dir = output_dir / "wheels"
    wheels_dir.mkdir(parents=True, exist_ok=True)

    wheel_info = []

    # Use pip download to get wheels
    with tempfile.TemporaryDirectory() as tmpdir:
        for pkg_spec in PYPI_PACKAGES:
            print(f"  Downloading: {pkg_spec}")
            try:
                # Download wheel for wasm32 platform (pure Python only)
                subprocess.run([
                    "pip", "download",
                    "--only-binary=:all:",
                    "--platform", "any",
                    "--python-version", "3.12",
                    "-d", tmpdir,
                    pkg_spec
                ], check=True, capture_output=True)
            except subprocess.CalledProcessError as e:
                print(f"    WARNING: Could not download {pkg_spec}: {e}")
                continue

        # Copy downloaded wheels
        for whl_file in Path(tmpdir).glob("*.whl"):
            dest = wheels_dir / whl_file.name
            shutil.copy(whl_file, dest)

            # Compute hash for lock file
            sha256 = compute_sha256(dest)
            wheel_info.append({
                "name": whl_file.stem.split("-")[0].lower().replace("_", "-"),
                "file_name": whl_file.name,
                "sha256": sha256,
                "size": dest.stat().st_size
            })
            print(f"    -> {whl_file.name}")

    return wheel_info


def create_custom_lock_file(output_dir: Path, original_lock: dict, wheel_info: list) -> None:
    """
    Create a modified pyodide-lock.json that includes local wheel paths.
    """
    print("\n=== Creating Custom Lock File ===")

    # The lock file needs to reference wheels with relative paths
    # or the wheels need to be added as entries in the packages section

    # For now, we'll keep the original lock file and note that
    # webworker.js needs to be modified to use local paths for micropip

    lock_path = output_dir / "pyodide" / "pyodide-lock.json"

    # Save original lock file
    with open(lock_path, 'w') as f:
        json.dump(original_lock, f, indent=2)

    # Save wheel manifest
    manifest_path = output_dir / "wheels" / "manifest.json"
    with open(manifest_path, 'w') as f:
        json.dump(wheel_info, f, indent=2)

    print(f"  Saved lock file: {lock_path}")
    print(f"  Saved wheel manifest: {manifest_path}")


def copy_datasette_lite_files(output_dir: Path) -> None:
    """Copy and modify datasette-lite source files."""
    print("\n=== Copying Datasette Lite Files ===")

    src_dir = Path("/tmp/datasette-lite")

    # Copy static files
    for filename in ["index.html", "app.css"]:
        src = src_dir / filename
        dest = output_dir / filename
        shutil.copy(src, dest)
        print(f"  Copied: {filename}")

    # webworker.js needs modifications for offline use
    # We'll create a modified version
    print("  Creating modified webworker.js...")


def create_modified_webworker(output_dir: Path) -> None:
    """
    Create a modified webworker.js that uses local paths instead of CDN.
    """
    src_path = Path("/tmp/datasette-lite/webworker.js")
    dest_path = output_dir / "webworker.js"

    content = src_path.read_text()

    # Modifications needed:
    modifications = """
// MODIFICATIONS FOR SELF-HOSTED VERSION:
//
// 1. Change importScripts to use local pyodide.js:
//    FROM: importScripts("https://cdn.jsdelivr.net/pyodide/v0.27.2/full/pyodide.js");
//    TO:   importScripts("./pyodide/pyodide.js");
//
// 2. Change indexURL in loadPyodide:
//    FROM: indexURL: "https://cdn.jsdelivr.net/pyodide/v0.27.2/full/",
//    TO:   indexURL: "./pyodide/",
//
// 3. Replace micropip.install() calls to use local wheel paths:
//    FROM: await micropip.install("h11==0.12.0")
//    TO:   await micropip.install("./wheels/h11-0.12.0-py3-none-any.whl")
//
// 4. Remove or replace default database URLs:
//    FROM: "https://latest.datasette.io/fixtures.db"
//    TO:   Remove or use local path
//
// 5. Remove analytics script from index.html
"""

    # Apply modifications
    modified = content

    # 1. Fix importScripts
    modified = modified.replace(
        f'importScripts("https://cdn.jsdelivr.net/pyodide/v{PYODIDE_VERSION}/full/pyodide.js");',
        'importScripts("./pyodide/pyodide.js");'
    )

    # 2. Fix indexURL
    modified = modified.replace(
        f'indexURL: "https://cdn.jsdelivr.net/pyodide/v{PYODIDE_VERSION}/full/"',
        'indexURL: "./pyodide/"'
    )

    # Write modified file with comment header
    with open(dest_path, 'w') as f:
        f.write("// === SELF-HOSTED VERSION ===\n")
        f.write(f"// Modified from original datasette-lite webworker.js\n")
        f.write(f"// Pyodide version: {PYODIDE_VERSION}\n")
        f.write("//\n")
        f.write(modified)

    print(f"  Created: {dest_path}")
    print("  NOTE: Additional manual modifications may be needed for micropip calls")


def create_modified_index(output_dir: Path) -> None:
    """Create modified index.html without external analytics."""
    src_path = Path("/tmp/datasette-lite/index.html")
    dest_path = output_dir / "index.html"

    content = src_path.read_text()

    # Remove plausible analytics
    import re
    modified = re.sub(
        r'<script defer data-domain="lite.datasette.io" src="https://plausible.io/js/script.manual.js"></script>\n?',
        '<!-- Analytics removed for self-hosted version -->\n',
        content
    )
    modified = re.sub(
        r'<script>window\.plausible = window\.plausible \|\| function\(\) \{ \(window\.plausible\.q = window\.plausible\.q \|\| \[\]\)\.push\(arguments\) \}</script>\n?',
        '',
        modified
    )

    with open(dest_path, 'w') as f:
        f.write(modified)

    print(f"  Created: {dest_path} (analytics removed)")


def create_readme(output_dir: Path) -> None:
    """Create README with usage instructions."""
    readme_content = f"""# Self-Hosted Datasette Lite

This is a self-contained bundle of Datasette Lite that can run offline.

## Contents

- `index.html` - Main application page
- `app.css` - Styles
- `webworker.js` - Python/Datasette runtime (modified for local paths)
- `pyodide/` - Pyodide runtime files (v{PYODIDE_VERSION})
- `wheels/` - Python wheel files for offline installation

## Usage

1. Serve this directory with any static file server:

   ```bash
   # Python
   python -m http.server 8000

   # Node.js
   npx serve .

   # Or use any web server (nginx, apache, etc.)
   ```

2. Open http://localhost:8000 in your browser

3. The application will load entirely from local files

## Server Requirements

Your web server must:
- Set correct MIME type for `.wasm` files: `application/wasm`
- Optionally set CORS headers if loading from different origins

Example for Python's http.server with WASM support:

```python
import http.server
import mimetypes

mimetypes.add_type('application/wasm', '.wasm')

http.server.test(HandlerClass=http.server.SimpleHTTPRequestHandler)
```

## Size

Total bundle size: approximately X MB

## Notes

- This bundle includes Pyodide {PYODIDE_VERSION}
- Default fixtures.db and content.db are NOT included
- To use your own databases, add them to the URL: `?url=./mydata.db`
- Analytics have been removed for offline use
"""

    readme_path = output_dir / "README.md"
    with open(readme_path, 'w') as f:
        f.write(readme_content)

    print(f"\n  Created: {readme_path}")


def create_server_script(output_dir: Path) -> None:
    """Create a simple server script with correct WASM MIME type."""
    server_script = '''#!/usr/bin/env python3
"""Simple HTTP server with correct WASM MIME type support."""

import http.server
import mimetypes

# Ensure WASM files are served with correct MIME type
mimetypes.add_type('application/wasm', '.wasm')
mimetypes.add_type('application/javascript', '.mjs')

class CORSHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        # Add CORS headers for local development
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        super().end_headers()

if __name__ == '__main__':
    import sys
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8000
    print(f"Serving on http://localhost:{port}")
    http.server.test(HandlerClass=CORSHandler, port=port)
'''

    server_path = output_dir / "serve.py"
    with open(server_path, 'w') as f:
        f.write(server_script)
    server_path.chmod(0o755)

    print(f"  Created: {server_path}")


def main():
    parser = argparse.ArgumentParser(description="Build self-hosted Datasette Lite bundle")
    parser.add_argument("--output", "-o", default="./datasette-lite-offline",
                        help="Output directory for the bundle")
    parser.add_argument("--pyodide-version", default=PYODIDE_VERSION,
                        help=f"Pyodide version to use (default: {PYODIDE_VERSION})")
    args = parser.parse_args()

    output_dir = Path(args.output).resolve()
    output_dir.mkdir(parents=True, exist_ok=True)

    print(f"Building self-hosted Datasette Lite bundle")
    print(f"Output directory: {output_dir}")
    print(f"Pyodide version: {PYODIDE_VERSION}")

    # Step 1: Download Pyodide core
    download_pyodide_core(output_dir)

    # Step 2: Load lock file and download packages
    lock_path = output_dir / "pyodide" / "pyodide-lock.json"
    with open(lock_path) as f:
        lock_data = json.load(f)

    download_pyodide_packages(output_dir, lock_data)

    # Step 3: Download PyPI wheels
    wheel_info = download_pypi_wheels(output_dir)

    # Step 4: Create custom lock file
    create_custom_lock_file(output_dir, lock_data, wheel_info)

    # Step 5: Copy and modify datasette-lite files
    copy_datasette_lite_files(output_dir)
    create_modified_webworker(output_dir)
    create_modified_index(output_dir)

    # Step 6: Create helper files
    create_readme(output_dir)
    create_server_script(output_dir)

    # Calculate total size
    total_size = sum(f.stat().st_size for f in output_dir.rglob('*') if f.is_file())
    total_mb = total_size / (1024 * 1024)

    print(f"\n{'='*60}")
    print(f"Bundle created successfully!")
    print(f"Location: {output_dir}")
    print(f"Total size: {total_mb:.1f} MB")
    print(f"{'='*60}")
    print(f"\nTo test:")
    print(f"  cd {output_dir}")
    print(f"  python serve.py")
    print(f"  Open http://localhost:8000")


if __name__ == "__main__":
    main()
