#!/usr/bin/env python3
"""
Setup script for epsilon-python

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
            print("ERROR: Go compiler not found. Please install Go 1.21 or later.")
            print("Visit: https://golang.org/doc/install")
            sys.exit(1)

        # Build the shared library
        source_dir = Path(__file__).parent
        lib_source = source_dir / "libepsilon.go"

        if not lib_source.exists():
            print(f"ERROR: {lib_source} not found")
            sys.exit(1)

        # Determine output library name based on platform
        if sys.platform == "darwin":
            lib_name = "libepsilon.dylib"
        elif sys.platform == "win32":
            lib_name = "libepsilon.dll"
        else:
            lib_name = "libepsilon.so"

        lib_output = source_dir / "epsilon" / lib_name

        # Ensure output directory exists
        lib_output.parent.mkdir(parents=True, exist_ok=True)

        # Clean up old library files from other platforms
        old_libs = [
            source_dir / "epsilon" / "libepsilon.so",
            source_dir / "epsilon" / "libepsilon.dylib",
            source_dir / "epsilon" / "libepsilon.dll",
            source_dir / "libepsilon.so",
            source_dir / "libepsilon.dylib",
            source_dir / "libepsilon.dll",
        ]
        for old_lib in old_libs:
            if old_lib.exists() and old_lib != lib_output:
                print(f"Removing old library: {old_lib}")
                old_lib.unlink()

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
            print(f"Built {lib_output}")
        except subprocess.CalledProcessError as e:
            print(f"ERROR: Failed to build Go library: {e}")
            sys.exit(1)

        # Continue with normal build_ext
        super().run()

        # Copy the library to the build directory
        if self.build_lib:
            build_epsilon_dir = Path(self.build_lib) / "epsilon"
            build_epsilon_dir.mkdir(parents=True, exist_ok=True)

            dest_lib = build_epsilon_dir / lib_name
            print(f"Copying {lib_output} to {dest_lib}")

            import shutil
            shutil.copy2(lib_output, dest_lib)


class GoExtension(Extension):
    """Dummy extension to trigger custom build"""

    def __init__(self, name):
        super().__init__(name, sources=[])


# Read long description from README
readme_file = Path(__file__).parent / "README.md"
long_description = ""
if readme_file.exists():
    long_description = readme_file.read_text(encoding="utf-8")


setup(
    long_description=long_description,
    long_description_content_type="text/markdown",
    ext_modules=[GoExtension("epsilon._libepsilon")],
    cmdclass={
        "build_ext": GoBuildExt,
    },
    package_data={
        "epsilon": ["*.so", "*.dll", "*.dylib", "*.h"],
    },
    include_package_data=True,
)
