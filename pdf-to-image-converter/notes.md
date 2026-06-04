# PDF to Image Converter - Investigation Notes

## Goal
Build a Rust tool using `pdfium-render` to convert PDF pages to JPGs, then wrap it in Python bindings as a self-contained wheel.

## Progress

### 2026-03-19: Setup
- Created project folder
- Downloaded demo PDF: simpleqa.pdf from OpenAI (14 pages, 413KB)
- Rust 1.93.1, Python 3.11.14, uv 0.8.17 available

### Pdfium binary dependency
- `pdfium-render` is a Rust binding crate that requires the PDFium shared library at runtime
- PDFium is the open-source PDF rendering engine used by Chromium
- Prebuilt binaries available from https://github.com/bblanchon/pdfium-binaries
- Downloaded `pdfium-linux-x64.tgz` (chromium/7734 release) — contains `libpdfium.so` (7.5MB)
- The library can be loaded dynamically via `Pdfium::bind_to_library()` or `Pdfium::bind_to_system_library()`

### Rust CLI (rust-cli/)
- Created a simple CLI tool using `pdfium-render` v0.8 with `image` feature
- Key learning: JPEG format doesn't support RGBA8 — need to convert bitmap to RGB8 first via `DynamicImage::to_rgb8()`
- Renders each page at configurable DPI (default 150), calculates pixel width from page points
- Successfully rendered all 14 pages of simpleqa.pdf to JPGs (3.2MB total)

### Python bindings (pdf2img-py/)
- Used PyO3 v0.23 with maturin as build backend
- Three Python functions exposed:
  - `pdf_to_images(pdf_path, output_dir, dpi=150)` → list of output paths
  - `page_count(pdf_path)` → int
  - `render_page(pdf_path, page_number, dpi=150)` → bytes (JPEG data)
- Used maturin's `python-source` and `module-name` options to create a mixed Rust/Python package
- The `__init__.py` sets `PDFIUM_LIB_PATH` to find the bundled `libpdfium.so` before importing the native module

### Self-contained wheel
- Used maturin's `include` config to bundle `libpdfium.so` into the wheel alongside the extension
- The wheel contains:
  - `pdf2img_py/_native.cpython-311-x86_64-linux-gnu.so` (10MB) — the compiled Rust extension
  - `pdf2img_py/__init__.py` — Python wrapper that sets up library path
  - `bundled_libs/libpdfium.so` (7.5MB) — the PDFium shared library
- Total wheel size: ~17MB
- Tested in a completely clean virtualenv — works with no external dependencies or env vars needed
- The library finding logic: checks `PDFIUM_LIB_PATH` env var → checks directory of the .so file via `/proc/self/maps` → falls back to system library

### Key learnings
1. `pdfium-render` provides a nice high-level API but requires a separate PDFium binary
2. JPEG doesn't support alpha channels — must convert RGBA→RGB before saving
3. Maturin's `include` directive with `format = "wheel"` bundles extra files into the wheel
4. The `python-source` + `module-name` pattern lets you have a Python __init__.py that wraps the native module
5. PyO3 0.23 works well with pyo3-macros for clean Python bindings
