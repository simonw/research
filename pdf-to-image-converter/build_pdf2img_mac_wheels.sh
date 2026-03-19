#!/bin/bash
# Build macOS arm64 wheels for pdf2img-py (Python 3.10-3.14)
# Prerequisites: uv, Rust toolchain (rustc + cargo)
# Usage: bash build_pdf2img_wheels.sh
set -euo pipefail

WORK="/tmp/pdf2img-py-build"
PDFIUM_URL="https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/7734/pdfium-mac-arm64.tgz"
PYTHON_VERSIONS="3.10 3.11 3.12 3.13 3.14"

echo "==> Cleaning $WORK"
rm -rf "$WORK"
mkdir -p "$WORK"

# ── 1. Install Rust if needed ──────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
    echo "==> Installing Rust via rustup"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    . "$HOME/.cargo/env"
fi
echo "==> Using $(rustc --version)"

# ── 2. Install Python versions via uv ──────────────────────────────────────
echo "==> Installing Python versions via uv"
for v in $PYTHON_VERSIONS; do
    uv python install "$v"
done

# ── 3. Clone repo and copy source ─────────────────────────────────────────
echo "==> Cloning simonw/research"
git clone --depth 1 https://github.com/simonw/research.git "$WORK/research"
cp -r "$WORK/research/pdf-to-image-converter/pdf2img-py" "$WORK/pdf2img-py"
SRC="$WORK/pdf2img-py"
cd "$SRC"

# ── 4. Download pdfium macOS arm64 binary ──────────────────────────────────
echo "==> Downloading pdfium"
mkdir -p pdfium-mac
curl -sL "$PDFIUM_URL" | tar xz -C pdfium-mac
mkdir -p python/pdf2img_py/bundled_libs
cp pdfium-mac/lib/libpdfium.dylib python/pdf2img_py/bundled_libs/

# ── 5. Patch source files for macOS wheel build ───────────────────────────

# 5a. Upgrade pyo3 to 0.28 (needed for Python 3.14), add mach2 for macOS dyld
cat > Cargo.toml << 'CARGO_EOF'
[package]
name = "pdf2img_py"
version = "0.1.0"
edition = "2021"

[lib]
name = "pdf2img_py"
crate-type = ["cdylib"]

[dependencies]
pyo3 = { version = "0.28", features = ["extension-module"] }
pdfium-render = { version = "0.8", features = ["image"] }
image = "0.25"

[target.'cfg(target_os = "macos")'.dependencies]
mach2 = "0.4"
CARGO_EOF

# 5b. Fix find_self_dir() for macOS (uses mach2 dyld instead of /proc/self/maps)
cat > src/lib.rs << 'RUST_EOF'
use image::DynamicImage;
use pdfium_render::prelude::*;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::path::PathBuf;

/// Find and bind to the pdfium library.
/// Searches: PDFIUM_LIB_PATH env var, directory of this .so, then system libraries.
fn bind_pdfium() -> Result<Pdfium, PdfiumError> {
    // Try PDFIUM_LIB_PATH env var first
    if let Ok(lib_path) = std::env::var("PDFIUM_LIB_PATH") {
        if let Ok(bindings) = Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(&lib_path),
        ) {
            return Ok(Pdfium::new(bindings));
        }
    }

    // Try the directory where this shared library lives (for bundled wheels)
    let self_dir = find_self_dir();
    if let Some(ref dir) = self_dir {
        if let Ok(bindings) = Pdfium::bind_to_library(
            Pdfium::pdfium_platform_library_name_at_path(dir.to_string_lossy().as_ref()),
        ) {
            return Ok(Pdfium::new(bindings));
        }
    }

    // Fall back to system library
    Ok(Pdfium::new(Pdfium::bind_to_system_library()?))
}

/// Find the directory containing this shared library
fn find_self_dir() -> Option<PathBuf> {
    // On macOS, use the dylib path from dyld
    #[cfg(target_os = "macos")]
    {
        use mach2::dyld::{_dyld_image_count, _dyld_get_image_name};
        let count = unsafe { _dyld_image_count() };
        for i in 0..count {
            let name_ptr = unsafe { _dyld_get_image_name(i) };
            if !name_ptr.is_null() {
                let name = unsafe { std::ffi::CStr::from_ptr(name_ptr) };
                if let Ok(s) = name.to_str() {
                    if s.contains("pdf2img_py") {
                        let path = PathBuf::from(s);
                        return path.parent().map(|p| p.to_path_buf());
                    }
                }
            }
        }
    }

    // On Linux, use /proc/self/maps
    #[cfg(target_os = "linux")]
    {
        if let Ok(maps) = std::fs::read_to_string("/proc/self/maps") {
            for line in maps.lines() {
                if line.contains("pdf2img_py") {
                    if let Some(path_start) = line.find('/') {
                        let path = PathBuf::from(&line[path_start..]);
                        return path.parent().map(|p| p.to_path_buf());
                    }
                }
            }
        }
    }

    None
}

/// Convert a PDF file to a directory of JPEG images.
///
/// Args:
///     pdf_path: Path to the input PDF file
///     output_dir: Directory to save the rendered page images
///     dpi: Resolution in dots per inch (default: 150)
///
/// Returns:
///     List of output file paths
#[pyfunction]
#[pyo3(signature = (pdf_path, output_dir, dpi=150))]
fn pdf_to_images(
    pdf_path: &str,
    output_dir: &str,
    dpi: u16,
) -> PyResult<Vec<String>> {
    let output_dir = PathBuf::from(output_dir);
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to create output dir: {}", e)))?;

    let pdfium = bind_pdfium()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to bind pdfium: {}", e)))?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load PDF: {}", e)))?;

    let mut output_paths = Vec::new();

    for (i, page) in document.pages().iter().enumerate() {
        let width_points = page.width().value;
        let pixel_width = (width_points * dpi as f32 / 72.0) as i32;

        let config = PdfRenderConfig::new()
            .set_target_width(pixel_width)
            .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

        let bitmap = page
            .render_with_config(&config)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to render page {}: {}", i + 1, e)))?;

        let img = bitmap.as_image();
        let rgb_img = DynamicImage::ImageRgba8(
            img.as_rgba8()
                .ok_or_else(|| PyRuntimeError::new_err("Failed to get RGBA8 image"))?
                .clone(),
        )
        .to_rgb8();

        let output_path = output_dir.join(format!("page_{:03}.jpg", i + 1));
        rgb_img
            .save(&output_path)
            .map_err(|e| PyRuntimeError::new_err(format!("Failed to save image: {}", e)))?;

        output_paths.push(output_path.to_string_lossy().to_string());
    }

    Ok(output_paths)
}

/// Get the number of pages in a PDF file.
#[pyfunction]
fn page_count(pdf_path: &str) -> PyResult<u16> {
    let pdfium = bind_pdfium()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to bind pdfium: {}", e)))?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load PDF: {}", e)))?;

    Ok(document.pages().len())
}

/// Render a single page of a PDF to JPEG bytes.
///
/// Args:
///     pdf_path: Path to the input PDF file
///     page_number: 0-indexed page number
///     dpi: Resolution in dots per inch (default: 150)
///
/// Returns:
///     JPEG image data as bytes
#[pyfunction]
#[pyo3(signature = (pdf_path, page_number, dpi=150))]
fn render_page(pdf_path: &str, page_number: u16, dpi: u16) -> PyResult<Vec<u8>> {
    let pdfium = bind_pdfium()
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to bind pdfium: {}", e)))?;

    let document = pdfium
        .load_pdf_from_file(pdf_path, None)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to load PDF: {}", e)))?;

    let page = document
        .pages()
        .get(page_number)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to get page {}: {}", page_number, e)))?;

    let width_points = page.width().value;
    let pixel_width = (width_points * dpi as f32 / 72.0) as i32;

    let config = PdfRenderConfig::new()
        .set_target_width(pixel_width)
        .rotate_if_landscape(PdfPageRenderRotation::Degrees90, true);

    let bitmap = page
        .render_with_config(&config)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to render page: {}", e)))?;

    let img = bitmap.as_image();
    let rgb_img = DynamicImage::ImageRgba8(
        img.as_rgba8()
            .ok_or_else(|| PyRuntimeError::new_err("Failed to get RGBA8 image"))?
            .clone(),
    )
    .to_rgb8();

    let mut buf = std::io::Cursor::new(Vec::new());
    rgb_img
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| PyRuntimeError::new_err(format!("Failed to encode JPEG: {}", e)))?;

    Ok(buf.into_inner())
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(pdf_to_images, m)?)?;
    m.add_function(wrap_pyfunction!(page_count, m)?)?;
    m.add_function(wrap_pyfunction!(render_page, m)?)?;
    Ok(())
}
RUST_EOF

# 5c. Fix pyproject.toml (remove README.md ref, remove stale include directive)
cat > pyproject.toml << 'TOML_EOF'
[build-system]
requires = ["maturin>=1.0,<2.0"]
build-backend = "maturin"

[project]
name = "pdf2img-py"
version = "0.1.0"
description = "Python bindings for pdfium-render - convert PDF pages to images"
requires-python = ">=3.10"
dependencies = []

[tool.maturin]
features = ["pyo3/extension-module"]
python-source = "python"
module-name = "pdf2img_py._native"
TOML_EOF

# 5d. Fix __init__.py bundled_libs path (was "../bundled_libs", now "bundled_libs" inside package)
cat > python/pdf2img_py/__init__.py << 'PY_EOF'
"""pdf2img_py - Convert PDF pages to images using pdfium-render."""

import os

# Set up library path to find bundled libpdfium before importing native module
_this_dir = os.path.dirname(os.path.abspath(__file__))
_bundled = os.path.join(_this_dir, "bundled_libs")
if os.path.isdir(_bundled):
    os.environ.setdefault("PDFIUM_LIB_PATH", _bundled)

from ._native import pdf_to_images, page_count, render_page

__all__ = ["pdf_to_images", "page_count", "render_page"]
PY_EOF

# ── 6. Resolve native Python interpreter paths ────────────────────────────
echo "==> Resolving Python interpreters"
INTERPRETERS=()
for v in $PYTHON_VERSIONS; do
    # uv python find returns the path to the interpreter
    interp=$(uv python find "$v" 2>/dev/null) || {
        echo "ERROR: Could not find Python $v"
        exit 1
    }
    # Skip emscripten/pyodide builds — we need native Darwin
    platform=$("$interp" -c "import platform; print(platform.system())" 2>/dev/null) || platform="unknown"
    if [ "$platform" != "Darwin" ]; then
        echo "  SKIP Python $v ($interp) — platform is '$platform', not Darwin"
        # Try to find a native one by looking in uv's python dir
        for candidate in "$HOME/.local/share/uv/python"/cpython-${v}*-macos-aarch64-none/bin/python3.*; do
            if [ -x "$candidate" ]; then
                cp=$("$candidate" -c "import platform; print(platform.system())" 2>/dev/null) || continue
                if [ "$cp" = "Darwin" ]; then
                    interp="$candidate"
                    echo "  FOUND native Python $v: $interp"
                    break
                fi
            fi
        done
        # Re-check
        platform=$("$interp" -c "import platform; print(platform.system())" 2>/dev/null) || platform="unknown"
        if [ "$platform" != "Darwin" ]; then
            echo "  ERROR: No native Darwin Python $v found, skipping"
            continue
        fi
    fi
    INTERPRETERS+=("$interp")
    echo "  Python $v: $interp"
done

if [ ${#INTERPRETERS[@]} -eq 0 ]; then
    echo "ERROR: No usable Python interpreters found"
    exit 1
fi

# ── 7. Build wheels ───────────────────────────────────────────────────────
echo "==> Building wheels with maturin"
uvx maturin build --release --interpreter "${INTERPRETERS[@]}"

# ── 8. Report ─────────────────────────────────────────────────────────────
echo ""
echo "==> Built wheels:"
ls -lh target/wheels/*.whl
echo ""
echo "Wheels are in: $SRC/target/wheels/"
