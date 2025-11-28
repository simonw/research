#!/usr/bin/env python3
"""
Analyze all dependencies needed for a self-hosted Datasette Lite installation.
This script examines the webworker.js code and determines all external resources.
"""

import json
import re
from pathlib import Path

def analyze_webworker():
    """Analyze the webworker.js file to extract all external dependencies."""

    webworker_path = Path("/tmp/datasette-lite/webworker.js")
    content = webworker_path.read_text()

    dependencies = {
        "pyodide_core": {
            "description": "Core Pyodide runtime files from jsdelivr CDN",
            "files": []
        },
        "pyodide_packages": {
            "description": "Pyodide built-in packages loaded via loadPackage",
            "files": []
        },
        "pypi_packages": {
            "description": "Packages installed via micropip from PyPI",
            "files": []
        },
        "default_databases": {
            "description": "Default SQLite databases loaded when no URL params",
            "files": []
        },
        "analytics": {
            "description": "Analytics scripts (optional)",
            "files": []
        }
    }

    # 1. Pyodide core
    pyodide_version_match = re.search(r'pyodide/v([\d.]+)/full/', content)
    pyodide_version = pyodide_version_match.group(1) if pyodide_version_match else "0.27.2"

    dependencies["pyodide_core"]["files"] = [
        f"https://cdn.jsdelivr.net/pyodide/v{pyodide_version}/full/pyodide.js",
        f"https://cdn.jsdelivr.net/pyodide/v{pyodide_version}/full/pyodide.asm.js",
        f"https://cdn.jsdelivr.net/pyodide/v{pyodide_version}/full/pyodide.asm.wasm",
        f"https://cdn.jsdelivr.net/pyodide/v{pyodide_version}/full/python_stdlib.zip",
        f"https://cdn.jsdelivr.net/pyodide/v{pyodide_version}/full/pyodide-lock.json",
    ]
    dependencies["pyodide_core"]["version"] = pyodide_version

    # 2. Pyodide built-in packages (loaded via loadPackage)
    # From: await pyodide.loadPackage('micropip', ...)
    loadpackage_matches = re.findall(r"loadPackage\(['\"](\w+)['\"]", content)
    dependencies["pyodide_packages"]["files"] = list(set(loadpackage_matches))

    # 3. PyPI packages installed via micropip
    # Extract from micropip.install calls
    micropip_matches = re.findall(r'micropip\.install\(["\']([^"\']+)["\']', content)
    dependencies["pypi_packages"]["files"] = micropip_matches

    # Also note: datasette itself is installed
    if "datasetteToInstall" in content:
        dependencies["pypi_packages"]["note"] = "Datasette package name varies based on ?ref= parameter"

    # 4. Default databases
    db_matches = re.findall(r'"(https://[^"]+\.db)"', content)
    dependencies["default_databases"]["files"] = db_matches

    return dependencies

def analyze_index_html():
    """Analyze index.html for additional external resources."""

    index_path = Path("/tmp/datasette-lite/index.html")
    content = index_path.read_text()

    resources = []

    # Analytics script
    plausible_match = re.search(r'src="([^"]*plausible[^"]*)"', content)
    if plausible_match:
        resources.append({
            "type": "analytics",
            "url": plausible_match.group(1),
            "optional": True
        })

    return resources

def estimate_sizes():
    """Estimate file sizes for the complete bundle."""

    return {
        "pyodide_core": "~5-10 MB (core only) or ~327 MB (full distribution)",
        "pyodide_packages_needed": {
            "micropip": "~50 KB",
            "ssl": "~1 MB",
            "setuptools": "~1.5 MB"
        },
        "pypi_packages": {
            "note": "Pure Python wheels downloaded from PyPI",
            "datasette": "~3-5 MB (with dependencies)",
            "sqlite-utils": "~500 KB (optional, for CSV/JSON import)",
            "fastparquet": "~several MB (optional, for Parquet files)"
        },
        "total_estimated": "~50-100 MB for minimal, ~350 MB for full"
    }

def create_static_analysis():
    """Create a comprehensive static analysis of all dependencies."""

    print("=" * 70)
    print("DATASETTE LITE DEPENDENCY ANALYSIS")
    print("=" * 70)

    webworker_deps = analyze_webworker()
    index_resources = analyze_index_html()
    sizes = estimate_sizes()

    # Create comprehensive report
    report = {
        "webworker_dependencies": webworker_deps,
        "index_resources": index_resources,
        "size_estimates": sizes,
        "external_hosts": [
            "cdn.jsdelivr.net (Pyodide runtime and packages)",
            "files.pythonhosted.org (PyPI wheel downloads)",
            "pypi.org (package metadata)",
            "latest.datasette.io (default fixtures.db)",
            "datasette.io (default content.db)",
            "plausible.io (analytics - optional)"
        ],
        "self_hosting_requirements": {
            "minimum": [
                "Pyodide core files (pyodide.js, pyodide.asm.wasm, python_stdlib.zip, pyodide-lock.json)",
                "Required Pyodide packages: micropip, ssl, setuptools",
                "Datasette wheel and all its dependencies",
                "h11, httpx, python-multipart, typing-extensions wheels",
                "A static file server with proper CORS and MIME types"
            ],
            "optional": [
                "sqlite-utils wheel (for CSV/JSON import)",
                "fastparquet wheel (for Parquet support)",
                "Pre-downloaded default databases",
                "Full Pyodide distribution (for offline package access)"
            ]
        }
    }

    # Print summary
    print("\n1. PYODIDE CORE")
    print("-" * 40)
    print(f"   Version: {webworker_deps['pyodide_core']['version']}")
    for f in webworker_deps['pyodide_core']['files']:
        print(f"   - {f}")

    print("\n2. PYODIDE BUILT-IN PACKAGES (loadPackage)")
    print("-" * 40)
    for pkg in webworker_deps['pyodide_packages']['files']:
        print(f"   - {pkg}")

    print("\n3. PYPI PACKAGES (micropip.install)")
    print("-" * 40)
    for pkg in webworker_deps['pypi_packages']['files']:
        print(f"   - {pkg}")
    if 'note' in webworker_deps['pypi_packages']:
        print(f"   Note: {webworker_deps['pypi_packages']['note']}")

    print("\n4. DEFAULT DATABASES")
    print("-" * 40)
    for db in webworker_deps['default_databases']['files']:
        print(f"   - {db}")

    print("\n5. OPTIONAL RESOURCES")
    print("-" * 40)
    for res in index_resources:
        print(f"   - [{res['type']}] {res['url']}")

    print("\n6. SIZE ESTIMATES")
    print("-" * 40)
    print(f"   Pyodide core: {sizes['pyodide_core']}")
    print(f"   Total estimated: {sizes['total_estimated']}")

    print("\n7. EXTERNAL HOSTS TO ELIMINATE")
    print("-" * 40)
    for host in report['external_hosts']:
        print(f"   - {host}")

    # Save detailed report
    report_path = Path("/home/user/research/self-host-datasette-lite/dependency_analysis.json")
    with open(report_path, 'w') as f:
        json.dump(report, f, indent=2)
    print(f"\nDetailed report saved to: {report_path}")

    return report

if __name__ == "__main__":
    create_static_analysis()
