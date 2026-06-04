import os
import subprocess
import sys
from setuptools import setup, Extension

# Paths
HERE = os.path.dirname(os.path.abspath(__file__))
SYNTAQLITE_SRC = os.environ.get("SYNTAQLITE_SRC", "/tmp/syntaqlite")

# Include directories
include_dirs = [
    os.path.join(SYNTAQLITE_SRC, "syntaqlite-syntax", "include"),
    os.path.join(SYNTAQLITE_SRC, "syntaqlite", "include"),
]

# When cross-compiling for emscripten (Pyodide), use WASM library paths
is_emscripten = os.environ.get("_PYTHON_HOST_PLATFORM", "").startswith("emscripten") or \
                "emscripten" in os.environ.get("CC", "")

# Check for explicit WASM library paths (set by build_wasm.sh)
wasm_release = os.environ.get("SYNTAQLITE_WASM_RELEASE", "")
wasm_syntax_out = os.environ.get("SYNTAQLITE_WASM_SYNTAX_OUT", "")

if is_emscripten and wasm_release:
    release_dir = wasm_release
    syntax_out_dir = wasm_syntax_out
else:
    # Pre-built static library paths from `cargo build --release`
    release_dir = os.path.join(SYNTAQLITE_SRC, "target", "release")
    syntax_out_dir = None
    build_dir = os.path.join(release_dir, "build")
    if os.path.isdir(build_dir):
        for entry in os.listdir(build_dir):
            if entry.startswith("syntaqlite-syntax-"):
                candidate = os.path.join(build_dir, entry, "out")
                if os.path.isfile(os.path.join(candidate, "libsyntaqlite_engine.a")):
                    syntax_out_dir = candidate
                    break

    if not syntax_out_dir:
        print("ERROR: Could not find pre-built syntaqlite static libraries.", file=sys.stderr)
        print(f"  Looked in: {build_dir}", file=sys.stderr)
        print("  Run 'cargo build --release' in the syntaqlite repo first.", file=sys.stderr)
        sys.exit(1)

library_dirs = [release_dir, syntax_out_dir]

# Libraries to link (order matters for static linking)
libraries = ["syntaqlite", "syntaqlite_engine", "syntaqlite_sqlite"]

# Platform-specific link flags
extra_link_args = []
if sys.platform == "darwin":
    extra_link_args += ["-framework", "Security"]
elif sys.platform == "linux":
    extra_link_args += ["-lpthread", "-ldl", "-lm"]

# When cross-compiling for emscripten (Pyodide), copy libs to a short path
if is_emscripten:
    import shutil
    wasm_lib_dir = "/tmp/sqlib"
    os.makedirs(wasm_lib_dir, exist_ok=True)
    for lib_name in ["libsyntaqlite.a"]:
        src = os.path.join(release_dir, lib_name)
        if os.path.isfile(src):
            shutil.copy2(src, os.path.join(wasm_lib_dir, lib_name))
    for lib_name in ["libsyntaqlite_engine.a", "libsyntaqlite_sqlite.a"]:
        src = os.path.join(syntax_out_dir, lib_name)
        if os.path.isfile(src):
            shutil.copy2(src, os.path.join(wasm_lib_dir, lib_name))
    library_dirs = [wasm_lib_dir]
    # Skip wasm-opt (version in emsdk 3.1.46 doesn't support flags from newer Rust LLVM)
    extra_link_args = ["-sWASM_OPT=0"]

ext = Extension(
    "syntaqlite",
    sources=["_syntaqlite.c"],
    include_dirs=include_dirs,
    library_dirs=library_dirs,
    libraries=libraries,
    extra_link_args=extra_link_args,
    extra_compile_args=["-std=c11", "-DSYNTAQLITE_STATIC"],
)

setup(
    name="syntaqlite",
    version="0.1.0",
    description="Python bindings for syntaqlite — parser, formatter, and validator for SQLite SQL",
    ext_modules=[ext],
    python_requires=">=3.10",
)
