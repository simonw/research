# Building PyO3/Maturin Rust Extension Modules as WebAssembly Wheels for Pyodide

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A comprehensive guide to compiling Rust Python extensions (via PyO3 and maturin) to WebAssembly for use in Pyodide.

---

## Table of Contents

1. [Overview](#overview)
2. [Pyodide Version and Python Version Mapping](#pyodide-version-and-python-version-mapping)
3. [Setting Up the Emscripten SDK](#setting-up-the-emscripten-sdk)
4. [Rust Toolchain Setup](#rust-toolchain-setup)
5. [The Sysroot Problem and Solutions](#the-sysroot-problem-and-solutions)
6. [Building with Maturin](#building-with-maturin)
7. [Required RUSTFLAGS and Configuration](#required-rustflags-and-configuration)
8. [Wheel Naming Convention](#wheel-naming-convention)
9. [Loading Custom Wheels in Pyodide](#loading-custom-wheels-in-pyodide)
10. [Known Issues and Pitfalls](#known-issues-and-pitfalls)
11. [Complete Build Workflow](#complete-build-workflow)
12. [GitHub Actions CI Example](#github-actions-ci-example)
13. [Sources](#sources)

---

## Overview

Pyodide is a Python runtime compiled to WebAssembly using Emscripten. It can load Python extension modules that have been compiled to the `wasm32-unknown-emscripten` target. PyO3 is the primary Rust-Python binding crate, and maturin is the build tool that packages PyO3 projects into Python wheels. Together, they can produce `.whl` files containing `.so` (actually `.wasm`) shared libraries that Pyodide can load in the browser or Node.js.

Key tools involved:
- **PyO3**: Rust bindings for CPython (supports wasm32-unknown-emscripten)
- **maturin** (>= 0.14.14, latest 1.11.2): Build backend that produces Python wheels from Rust/PyO3 projects
- **Emscripten SDK (emsdk)**: The C/C++ to WebAssembly compiler toolchain
- **Rust nightly**: Required for the `-Z emscripten-wasm-eh` flag (for Pyodide 0.28+)

---

## Pyodide Version and Python Version Mapping

| Pyodide Version | Python Version | Emscripten Version | ABI Platform Tag |
|-----------------|----------------|--------------------|----------------------------------|
| 0.26.x          | 3.12.1         | 3.1.45             | `emscripten_3_1_45_wasm32`       |
| 0.27.x          | 3.12.7         | 3.1.58             | `emscripten_3_1_58_wasm32`       |
| 0.28.x          | 3.13.0         | 4.0.9              | `emscripten_4_0_9_wasm32`        |
| 0.29.x (stable) | 3.13.2         | 4.0.9              | `emscripten_4_0_9_wasm32`        |

**Important**: Pyodide's plan is one ABI per Python version. Packages built for Pyodide 0.28.x should be compatible with 0.29.x since both use Python 3.13.

As of early 2026, Pyodide 0.29.3 is the current stable release.

---

## Setting Up the Emscripten SDK

The Emscripten version **must exactly match** the version used to build the target Pyodide release. Emscripten does not provide a stable ABI for SIDE_MODULES, so any version mismatch can cause crashes or undefined behavior.

### Manual Installation

```bash
# Clone emsdk
git clone https://github.com/emscripten-core/emsdk.git
cd emsdk

# For Pyodide 0.28.x / 0.29.x (Python 3.13)
./emsdk install 4.0.9
./emsdk activate 4.0.9
source ./emsdk_env.sh

# Verify
which emcc
emcc --version
```

### Using pyodide-build to Determine the Version

If you have `pyodide-build` installed:

```bash
pip install pyodide-build
PYODIDE_EMSCRIPTEN_VERSION=$(pyodide config get emscripten_version)
echo $PYODIDE_EMSCRIPTEN_VERSION
```

### For Pyodide 0.27.x (Python 3.12)

```bash
./emsdk install 3.1.58
./emsdk activate 3.1.58
source ./emsdk_env.sh
```

**Note**: If you restart your shell, you must run `source emsdk_env.sh` again.

---

## Rust Toolchain Setup

### For Pyodide 0.28+ (2025 ABI with emscripten-wasm-eh)

The 2025 ABI requires the `-Z emscripten-wasm-eh` Rust compiler flag, which was added on January 15, 2025 and is nightly-only. This flag switches from JavaScript-based exception handling to native WebAssembly exception handling, producing smaller and faster code.

```bash
# Install a specific nightly (nightly-2025-02-01 has a pre-built sysroot available)
rustup toolchain install nightly-2025-02-01
rustup target add wasm32-unknown-emscripten --toolchain nightly-2025-02-01

# Or install latest nightly
rustup toolchain install nightly
rustup target add wasm32-unknown-emscripten --toolchain nightly
```

### For Pyodide 0.27.x (Pre-2025 ABI)

Older Pyodide versions do not require `-Z emscripten-wasm-eh`, so a stable or older nightly Rust can work:

```bash
rustup toolchain install nightly
rustup target add wasm32-unknown-emscripten --toolchain nightly
```

---

## The Sysroot Problem and Solutions

This is the single biggest pain point when building Rust for Pyodide 0.28+.

**The problem**: Rust ships with an Emscripten sysroot (standard library) built **without** the `-Z emscripten-wasm-eh` flag. When you compile your crate with `-Z emscripten-wasm-eh`, the resulting object files use a different unwinding ABI than the standard library, causing linker errors. This happens even if your crate uses `-Cpanic=abort` because the Rust standard library itself is not built with `-Cpanic=abort`.

### Solution 1: Pre-built Sysroot (Recommended)

The Pyodide project provides a pre-built compatible sysroot at [pyodide/rust-emscripten-wasm-eh-sysroot](https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot).

A pre-built sysroot is available for **Rust nightly-2025-02-01 + Emscripten 4.0.9**:

```bash
# Install the matching Rust nightly
rustup toolchain install nightly-2025-02-01

# Determine the toolchain root
TOOLCHAIN_ROOT=$(rustup run nightly-2025-02-01 rustc --print sysroot)
RUSTLIB=$TOOLCHAIN_ROOT/lib/rustlib

# Download and install the pre-built sysroot
wget https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot/releases/download/emcc-4.0.9_nightly-2025-02-01/emcc-4.0.9_nightly-2025-02-01.tar.bz2
tar -xf emcc-4.0.9_nightly-2025-02-01.tar.bz2 --directory=$RUSTLIB
```

### Solution 2: Build the Sysroot Yourself

If you need a different Rust nightly version:

```bash
git clone https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot.git
cd rust-emscripten-wasm-eh-sysroot
./main.sh 4.0.9 2025-02-17   # <emscripten-version> <rust-nightly-date>
```

### Solution 3: Use `-Zbuild-std` (Alternative, Has Limitations)

```bash
# This rebuilds the Rust standard library from source with your flags
export CARGO_UNSTABLE_BUILD_STD=true
# or pass -Zbuild-std to cargo
cargo build --target wasm32-unknown-emscripten -Zbuild-std
```

**Limitations of `-Zbuild-std`**:
- Does not work with `panic=abort`
- Does not work with `cargo vendor`
- Inefficient for building many packages (rebuilds stdlib each time)
- Various known bugs

---

## Building with Maturin

Maturin version **0.14.14 or later** is required for wasm32-unknown-emscripten support. The latest version as of early 2026 is **1.11.2**.

### Basic Build Command

```bash
maturin build --release --target wasm32-unknown-emscripten --out dist -i python3.13
```

The `-i` / `--interpreter` flag specifies the Python version to build for. This must match the Python version in your target Pyodide release.

### Project Setup

Your `Cargo.toml` must specify a `cdylib` crate type:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.23", features = ["extension-module"] }
```

Your `pyproject.toml` should use maturin as the build backend:

```toml
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[tool.maturin]
# Optional configuration
features = ["pyo3/extension-module"]
```

### Maturin and RUSTFLAGS Interaction

**Important caveat**: maturin sets the `RUSTFLAGS` environment variable, which causes Cargo to **ignore** `.cargo/config.toml` files (Cargo prefers environment variables over config files). Starting with maturin 1.0.0 beta, maturin reads `.cargo` config files and merges them with RUSTFLAGS using `cargo-config2`.

You can debug this behavior with:

```bash
RUST_LOG=maturin=debug maturin build --target wasm32-unknown-emscripten
```

---

## Required RUSTFLAGS and Configuration

### For Pyodide 0.28+ (2025 ABI)

```bash
export RUSTFLAGS="\
  -Z emscripten-wasm-eh \
  -C link-arg=-sSIDE_MODULE=2 \
  -C link-arg=-sWASM_BIGINT \
  -C relocation-model=pic \
  -Z link-native-libraries=no"
```

### Explanation of Each Flag

| Flag | Purpose |
|------|---------|
| `-Z emscripten-wasm-eh` | Enables native WASM exception handling (required for 2025 ABI) |
| `-C link-arg=-sSIDE_MODULE=2` | Tells emcc to produce a shared library (SIDE_MODULE) instead of a standalone binary. `=2` is required for Rust (see below) |
| `-C link-arg=-sWASM_BIGINT` | Enables BigInt integration for i64 values. Default since Emscripten 4.0.0, but explicit is safe for all versions |
| `-C relocation-model=pic` | Produces position-independent code, needed for shared library loading |
| `-Z link-native-libraries=no` | Prevents Rust from trying to link native C libraries that don't exist in the Emscripten environment |

### Why SIDE_MODULE=2 and Not SIDE_MODULE=1

- `SIDE_MODULE=1` passes `-whole-archive` to `wasm-ld`, forcing inclusion of all object files. This **does not work with Rust** because Rust libraries contain a `lib.rmeta` metadata file which is not a valid object file.
- `SIDE_MODULE=2` only includes explicitly exported symbols. Rust produces the correct export list automatically.

### For Pyodide 0.27.x (Pre-2025 ABI)

```bash
export RUSTFLAGS="\
  -C link-arg=-sSIDE_MODULE=2 \
  -C link-arg=-sWASM_BIGINT \
  -C relocation-model=pic"
```

### Alternative: Using .cargo/config.toml

```toml
[target.wasm32-unknown-emscripten]
rustflags = [
  "-Z", "emscripten-wasm-eh",
  "-Clink-arg=-sSIDE_MODULE=2",
  "-Clink-arg=-sWASM_BIGINT",
  "-Crelocation-model=pic",
  "-Zlink-native-libraries=no",
]
```

**Note**: This will be ignored if maturin (or anything else) sets the `RUSTFLAGS` environment variable.

---

## Wheel Naming Convention

Pyodide wheels follow the standard PEP 427 naming convention:

```
{name}-{version}-{python_tag}-{abi_tag}-{platform_tag}.whl
```

### Platform Tag Format

The platform tag includes the Emscripten version with underscores replacing dots:

```
emscripten_{major}_{minor}_{patch}_wasm32
```

### Examples

```
# For Pyodide 0.27.x (Emscripten 3.1.58, Python 3.12)
my_package-1.0.0-cp312-cp312-emscripten_3_1_58_wasm32.whl

# For Pyodide 0.28.x/0.29.x (Emscripten 4.0.9, Python 3.13)
my_package-1.0.0-cp313-cp313-emscripten_4_0_9_wasm32.whl
```

### Real-World Example

The pydantic-core project produces wheels like:
```
pydantic_core-2.35.0-cp312-cp312-emscripten_3_1_58_wasm32.whl
```

### Future: PEP 783 Platform Tags

PEP 783 (Emscripten Packaging) proposes a new platform tag format:
```
pyodide_2025_0_wasm32
```
This would decouple the tag from specific Emscripten versions and allow one ABI per Python version. When approved, PyPI will accept uploads with this tag.

### PyPI Upload Status

As of early 2026, PyPI does **not** accept wasm32/emscripten wheels. Uploading results in:
```
ERROR: Binary wheel has an unsupported platform tag 'emscripten_X_Y_Z_wasm32'
```
PEP 783 is working to resolve this. Until then, wheels must be hosted on custom servers, GitHub Releases, or CDNs.

---

## Loading Custom Wheels in Pyodide

### Method 1: micropip.install() (Recommended)

`micropip` is the standard package installer for Pyodide. It supports installing wheels from URLs.

```javascript
// In JavaScript
const pyodide = await loadPyodide();
await pyodide.loadPackage("micropip");
const micropip = pyodide.pyimport("micropip");

// Install from a URL
await micropip.install("https://example.com/my_package-1.0.0-cp313-cp313-emscripten_4_0_9_wasm32.whl");

// Install from multiple URLs
await micropip.install([
  "https://example.com/dep1-1.0.0-py3-none-any.whl",
  "https://example.com/my_package-1.0.0-cp313-cp313-emscripten_4_0_9_wasm32.whl"
]);

// Install without resolving dependencies
await micropip.install("https://example.com/my_package-1.0.0-cp313-cp313-emscripten_4_0_9_wasm32.whl", {deps: false});
```

Or in Python (running inside Pyodide):

```python
import micropip
await micropip.install("https://example.com/my_package-1.0.0-cp313-cp313-emscripten_4_0_9_wasm32.whl")
```

**URL requirements**:
- If the requirement string ends in `.whl`, it is treated as a wheel URI
- The filename (after the last `/`) must be a valid PEP 427 wheel name
- URLs starting with `http:` or `https:` are fetched over the network
- URLs starting with `emfs:` are read from the Emscripten virtual filesystem
- In Node.js, `file:` URLs access the native filesystem

**Features**:
- Automatically resolves and installs dependencies (disable with `deps=False`)
- Can install pure Python wheels and binary wasm32/emscripten wheels

### Method 2: pyodide.loadPackage() (JavaScript API)

```javascript
// Load directly from URL (no dependency resolution)
await pyodide.loadPackage("https://example.com/my_package-1.0.0-cp313-cp313-emscripten_4_0_9_wasm32.whl");
```

**Differences from micropip**:
- No dependency resolution
- Does not require loading the micropip package first
- Faster if you don't need dependencies
- Good for pre-resolved package sets

### CORS Requirements

When serving wheels from a custom server, the server **must** set appropriate CORS headers:

```
Access-Control-Allow-Origin: *
```

Without CORS headers, the browser will block the fetch. Alternatives:
- Use a CORS proxy (security implications)
- Host on a service that supports CORS (GitHub Pages, Cloudflare Pages, S3 with CORS config)
- Use GitHub Releases URLs (GitHub sets CORS headers on release assets)

### Advanced: Custom Lock Files

For production deployments, you can generate a lock file and use `loadPackage()` for faster, deterministic loading:

```javascript
const pyodide = await loadPyodide({
  lockFileURL: "https://example.com/custom-pyodide-lock.json"
});
await pyodide.loadPackage("my_package");  // Uses lock file for resolution
```

---

## Known Issues and Pitfalls

### 1. `-pthread` Breaks Everything
**Never** use `-pthread` at compile or link time. If it is used, the resulting libraries will silently fail to load in Pyodide. There is no error message -- the library simply will not work.

### 2. SIDE_MODULE=1 Does Not Work with Rust
Rust crates contain `lib.rmeta` metadata files that are not valid object files. Since `-sSIDE_MODULE=1` passes `-whole-archive` to the linker, it tries to include `lib.rmeta` and fails. Always use `-sSIDE_MODULE=2`.

### 3. Sysroot ABI Mismatch (Pyodide 0.28+)
Using the default Rust sysroot with `-Z emscripten-wasm-eh` causes linker errors because the standard library was built with JS exception handling but your code uses WASM exception handling. You must use a compatible sysroot (see [The Sysroot Problem](#the-sysroot-problem-and-solutions)).

### 4. Relocation Model Errors
Error: `relocation R_WASM_TABLE_INDEX_SLEB` -- This occurs because Rust defaults to `relocation_model=static` for Emscripten. Fix with `-C relocation-model=pic` in RUSTFLAGS. When using `-Zbuild-std`, the standard library also needs to be PIC, which it will be if the flag is set.

### 5. `.wasm` vs `.so` Extension Mismatch
After successful linking, setuptools-rust may fail to create a wheel because it expects a `.so` file but gets a `.wasm` file. **Maturin handles this correctly** -- this is only an issue if using setuptools-rust directly.

### 6. Maturin Overrides .cargo/config.toml
Maturin sets `RUSTFLAGS` as an environment variable, which takes precedence over `.cargo/config.toml`. Set your flags in the environment or use maturin's config options. Debug with `RUST_LOG=maturin=debug`.

### 7. Rust Nightly Regression (April 2025)
Upgrading from Rust nightly v1.85 to v1.88 caused build failures with the error: "Unknown option '--enable-bulk-memory-opt'". Pin your Rust nightly version to avoid this (e.g., `nightly-2025-02-01` or `nightly-2025-02-17` as used by pydantic-core).

### 8. No PyPI Upload Support Yet
PyPI rejects wasm32/emscripten wheels. Host them on GitHub Releases, a CDN, or a custom server instead.

### 9. Cargo Workspace Config Not Read
When using Cargo workspaces, `.cargo/config.toml` files inside individual crates are not read if Cargo is invoked from the workspace root. Place your config at the workspace root.

---

## Complete Build Workflow

### Step-by-step for Pyodide 0.29.x (Stable, Python 3.13, Emscripten 4.0.9)

```bash
# 1. Install Rust nightly (pin to a known working version)
rustup toolchain install nightly-2025-02-01
rustup target add wasm32-unknown-emscripten --toolchain nightly-2025-02-01

# 2. Install the compatible sysroot
TOOLCHAIN_ROOT=$(rustup run nightly-2025-02-01 rustc --print sysroot)
RUSTLIB=$TOOLCHAIN_ROOT/lib/rustlib
wget https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot/releases/download/emcc-4.0.9_nightly-2025-02-01/emcc-4.0.9_nightly-2025-02-01.tar.bz2
tar -xf emcc-4.0.9_nightly-2025-02-01.tar.bz2 --directory=$RUSTLIB

# 3. Install Emscripten SDK 4.0.9
git clone https://github.com/emscripten-core/emsdk.git
cd emsdk
./emsdk install 4.0.9
./emsdk activate 4.0.9
source ./emsdk_env.sh
cd ..

# 4. Install maturin
pip install maturin>=1.0

# 5. Set RUSTFLAGS
export RUSTFLAGS="\
  -Z emscripten-wasm-eh \
  -C link-arg=-sSIDE_MODULE=2 \
  -C link-arg=-sWASM_BIGINT \
  -C relocation-model=pic \
  -Z link-native-libraries=no"

# 6. Build the wheel
# Use +nightly-2025-02-01 to select the specific toolchain
RUSTUP_TOOLCHAIN=nightly-2025-02-01 maturin build \
  --release \
  --target wasm32-unknown-emscripten \
  --out dist \
  -i python3.13

# 7. Verify the output
ls dist/
# Should show: my_package-x.y.z-cp313-cp313-emscripten_4_0_9_wasm32.whl
```

### Step-by-step for Pyodide 0.27.x (Python 3.12, Emscripten 3.1.58)

```bash
# 1. Install Rust nightly
rustup toolchain install nightly
rustup target add wasm32-unknown-emscripten --toolchain nightly

# 2. Install Emscripten SDK 3.1.58
git clone https://github.com/emscripten-core/emsdk.git
cd emsdk
./emsdk install 3.1.58
./emsdk activate 3.1.58
source ./emsdk_env.sh
cd ..

# 3. Install maturin
pip install maturin>=1.0

# 4. Set RUSTFLAGS (no -Z emscripten-wasm-eh needed for older ABI)
export RUSTFLAGS="\
  -C link-arg=-sSIDE_MODULE=2 \
  -C link-arg=-sWASM_BIGINT \
  -C relocation-model=pic"

# 5. Build the wheel
maturin build \
  --release \
  --target wasm32-unknown-emscripten \
  --out dist \
  -i python3.12
```

### Alternative: Using pyodide build

```bash
pip install pyodide-build
pyodide build
```

This is the simplest approach but provides less control over the build process.

---

## GitHub Actions CI Example

Based on the pydantic-core project's real-world CI workflow:

```yaml
name: Build WASM Wheel

on:
  push:
    branches: [main]
  pull_request:

jobs:
  build-wasm:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.13'

      - name: Install Rust nightly
        uses: dtolnay/rust-toolchain@master
        with:
          toolchain: nightly-2025-02-01
          targets: wasm32-unknown-emscripten

      - name: Cache Rust
        uses: Swatinem/rust-cache@v2

      - name: Install Emscripten sysroot
        run: |
          TOOLCHAIN_ROOT=$(rustup run nightly-2025-02-01 rustc --print sysroot)
          RUSTLIB=$TOOLCHAIN_ROOT/lib/rustlib
          wget -q https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot/releases/download/emcc-4.0.9_nightly-2025-02-01/emcc-4.0.9_nightly-2025-02-01.tar.bz2
          tar -xf emcc-4.0.9_nightly-2025-02-01.tar.bz2 --directory=$RUSTLIB

      - name: Setup Emscripten SDK
        uses: mymindstorm/setup-emsdk@v14
        with:
          version: '4.0.9'
          actions-cache-folder: emsdk-cache

      - name: Install maturin
        run: pip install maturin

      - name: Build WASM wheel
        env:
          RUSTUP_TOOLCHAIN: nightly-2025-02-01
          RUSTFLAGS: >-
            -Z emscripten-wasm-eh
            -C link-arg=-sSIDE_MODULE=2
            -C link-arg=-sWASM_BIGINT
            -C relocation-model=pic
            -Z link-native-libraries=no
        run: |
          maturin build --release --target wasm32-unknown-emscripten --out dist -i 3.13

      - name: Upload wheel artifact
        uses: actions/upload-artifact@v4
        with:
          name: wasm-wheel
          path: dist/*.whl

  test-wasm:
    needs: build-wasm
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'

      - name: Download wheel artifact
        uses: actions/download-artifact@v4
        with:
          name: wasm-wheel
          path: dist

      - name: Install Pyodide
        run: npm install pyodide

      - name: Test wheel in Pyodide
        run: |
          node -e "
          const { loadPyodide } = require('pyodide');
          async function main() {
            const pyodide = await loadPyodide();
            await pyodide.loadPackage('micropip');
            const micropip = pyodide.pyimport('micropip');
            // Install the wheel from the local filesystem
            const fs = require('fs');
            const wheels = fs.readdirSync('dist').filter(f => f.endsWith('.whl'));
            await micropip.install('file:dist/' + wheels[0]);
            // Test the import
            pyodide.runPython('import my_package; print(my_package)');
            console.log('WASM wheel loaded successfully!');
          }
          main().catch(e => { console.error(e); process.exit(1); });
          "
```

### Using maturin generate-ci

Maturin can also generate a CI workflow automatically:

```bash
maturin generate-ci github --platform emscripten > .github/workflows/CI.yml
```

---

## Sources

- [Pyodide Platform ABI (stable)](https://pyodide.org/en/stable/development/abi.html)
- [Pyodide Platform ABI (latest/dev)](https://pyodide.org/en/latest/development/abi.html)
- [Rust/PyO3 Support in Pyodide (blog post)](https://blog.pyodide.org/posts/rust-pyo3-support-in-pyodide/)
- [Pyodide 0.28 Release](https://blog.pyodide.org/posts/0.28-release/)
- [pyodide/rust-emscripten-wasm-eh-sysroot](https://github.com/pyodide/rust-emscripten-wasm-eh-sysroot)
- [PyO3 wasm32-emscripten support (issue #2412)](https://github.com/PyO3/pyo3/issues/2412)
- [Maturin PR #974: Add wasm32-unknown-emscripten support](https://github.com/PyO3/maturin/pull/974)
- [Maturin issue #2549: Build failure with newer Rust nightly](https://github.com/PyO3/maturin/issues/2549)
- [Maturin PR #1484: Emscripten in generate-ci](https://github.com/PyO3/maturin/pull/1484)
- [Loading packages in Pyodide](https://pyodide.org/en/stable/usage/loading-packages.html)
- [micropip API Reference](https://micropip.pyodide.org/en/stable/project/api.html)
- [wasm32-unknown-emscripten - The Rustc Book](https://doc.rust-lang.org/beta/rustc/platform-support/wasm32-unknown-emscripten.html)
- [Building Packages Using Recipe (Pyodide docs)](https://pyodide.org/en/latest/development/building-packages-using-recipe.html)
- [pydantic-core CI workflow](https://github.com/pydantic/pydantic-core/blob/main/.github/workflows/ci.yml)
- [Rust compiler-team MCP #920: Turn emscripten-wasm-eh on by default](https://github.com/rust-lang/compiler-team/issues/920)
- [Support WASM wheels on PyPI (Python discussion)](https://discuss.python.org/t/support-wasm-wheels-on-pypi/21924)
- [Emscripten SDK Documentation](https://emscripten.org/docs/tools_reference/emsdk.html)
- [cibuildwheel Emscripten support (issue #1454)](https://github.com/pypa/cibuildwheel/issues/1454)
- [Pyodide GitHub Releases](https://github.com/pyodide/pyodide/releases)
