#!/usr/bin/env python3
"""
Setup script for wazero-python

This builds the Go shared library and packages it with the Python code.
"""

import os
import subprocess
import sys
from pathlib import Path
from setuptools import setup, Extension
from setuptools.command.build_ext import build_ext


class GoBuildExt(build_ext):
    """Custom build extension to compile Go shared library"""

    def run(self):
        """Build the Go library"""
        print("Building Go shared library...")

        # Check if Go is available
        try:
            result = subprocess.run(
                ["go", "version"],
                check=True,
                capture_output=True,
                text=True
            )
            print(f"Using {result.stdout.strip()}")
        except (subprocess.CalledProcessError, FileNotFoundError):
            print("ERROR: Go compiler not found. Please install Go 1.18 or later.")
            print("Visit: https://golang.org/doc/install")
            sys.exit(1)

        # Build the shared library
        source_dir = Path(__file__).parent
        lib_source = source_dir / "libwazero.go"

        if not lib_source.exists():
            print(f"ERROR: {lib_source} not found")
            sys.exit(1)

        # Determine output library name based on platform
        if sys.platform == "darwin":
            lib_name = "libwazero.dylib"
        elif sys.platform == "win32":
            lib_name = "libwazero.dll"
        else:
            lib_name = "libwazero.so"

        lib_output = source_dir / "wazero" / lib_name

        # Ensure output directory exists
        lib_output.parent.mkdir(parents=True, exist_ok=True)

        # Build command
        cmd = [
            "go", "build",
            "-buildmode=c-shared",
            "-o", str(lib_output),
            str(lib_source)
        ]

        print(f"Running: {' '.join(cmd)}")

        try:
            subprocess.run(cmd, check=True, cwd=source_dir)
            print(f"✓ Built {lib_output}")
        except subprocess.CalledProcessError as e:
            print(f"ERROR: Failed to build Go library: {e}")
            sys.exit(1)

        # Continue with normal build_ext
        super().run()


class GoExtension(Extension):
    """Dummy extension to trigger custom build"""

    def __init__(self, name):
        # Don't pass sources to Extension, we'll build them with Go
        super().__init__(name, sources=[])


# Read long description from README
readme_file = Path(__file__).parent / "README.md"
long_description = ""
if readme_file.exists():
    long_description = readme_file.read_text(encoding="utf-8")


setup(
    long_description=long_description,
    long_description_content_type="text/markdown",
    ext_modules=[GoExtension("wazero._libwazero")],
    cmdclass={
        "build_ext": GoBuildExt,
    },
    # Include the shared library in the package
    package_data={
        "wazero": ["*.so", "*.dll", "*.dylib", "*.h"],
    },
    include_package_data=True,
)
