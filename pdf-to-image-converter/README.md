# PDF to Image Converter

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A Rust-based PDF-to-image renderer using the `pdfium-render` crate, with Python bindings packaged as a self-contained wheel.

## Overview

This project demonstrates:
1. Using the `pdfium-render` Rust crate to render PDF pages to JPEG images
2. Building Python bindings on top using PyO3 and maturin
3. Packaging the result as a wheel that bundles all binary dependencies (including `libpdfium.so`)

## Components

### Rust CLI (`rust-cli/`)

A command-line tool that converts PDF pages to JPEG images.

```bash
PDFIUM_LIB_PATH=./pdfium/lib ./rust-cli/target/release/pdf2img input.pdf output_dir [--dpi 150]
```

### Python Library (`pdf2img-py/`)

Python bindings built with PyO3 and maturin. The wheel bundles `libpdfium.so` so no external dependencies are needed.

**API:**

```python
import pdf2img_py

# Get page count
count = pdf2img_py.page_count("input.pdf")

# Render all pages to a directory
paths = pdf2img_py.pdf_to_images("input.pdf", "output_dir", dpi=150)

# Render a single page to JPEG bytes
jpeg_data = pdf2img_py.render_page("input.pdf", 0, dpi=150)
```

**Building the wheel:**

```bash
cd pdf2img-py
uv venv && source .venv/bin/activate
uv pip install maturin
maturin build --release
# Wheel output in target/wheels/
```

**Installing:**

```bash
pip install pdf2img_py-0.1.0-cp311-cp311-manylinux_2_34_x86_64.whl
# No other dependencies needed — libpdfium.so is bundled in the wheel
```

### Rendered Pages (`rendered_pages/`)

The 14 pages of [SimpleQA](https://cdn.openai.com/papers/simpleqa.pdf) rendered as JPEG images at 150 DPI.

## Architecture

```
pdf-to-image-converter/
├── rust-cli/                  # Standalone Rust CLI
│   ├── Cargo.toml
│   └── src/main.rs
├── pdf2img-py/                # Python bindings (PyO3 + maturin)
│   ├── Cargo.toml             # Rust dependencies
│   ├── pyproject.toml         # Python build config
│   ├── src/lib.rs             # Rust extension code
│   ├── python/pdf2img_py/     # Python wrapper package
│   │   └── __init__.py
│   └── bundled_libs/          # Bundled into wheel
│       └── libpdfium.so
├── rendered_pages/            # Demo output (14 JPGs)
├── notes.md                   # Investigation notes
└── README.md                  # This file
```

## Key Technical Details

- **PDFium**: The open-source PDF rendering engine from the Chromium project. Prebuilt binaries from [bblanchon/pdfium-binaries](https://github.com/bblanchon/pdfium-binaries).
- **pdfium-render**: Rust crate (v0.8) providing high-level bindings to PDFium's C API via dynamic loading.
- **RGBA→RGB conversion**: JPEG doesn't support alpha channels, so rendered bitmaps are converted from RGBA8 to RGB8 before saving.
- **Library discovery**: The Python package sets `PDFIUM_LIB_PATH` to its bundled libs directory; the Rust code also searches via `/proc/self/maps` and system library paths as fallbacks.
- **Wheel size**: ~17MB total (10MB compiled extension + 7.5MB libpdfium.so).
