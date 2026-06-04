Leveraging Rust's `pdfium-render` crate and Python's PyO3 bindings, this project enables fast and reliable conversion of PDF pages to JPEG images, packaged as a self-contained Python wheel. The CLI tool and Python library are both built to require no external dependencies, bundling the necessary PDFium binary for ease of installation and cross-platform compatibility. Users can retrieve page counts, render individual pages to byte streams, or batch convert PDFs to images at configurable DPI settings. Architecture is modular, separating the Rust CLI and Python API, with clear mechanisms for library discovery and efficient RGBA-to-RGB image processing.

**Key findings:**
- Self-contained installs: Bundles `libpdfium.so` in wheel, removing dependency headaches.
- Uses [pdfium-render](https://github.com/paulvollmer/pdfium-render) and prebuilt [pdfium-binaries](https://github.com/bblanchon/pdfium-binaries).
- Efficient conversion: JPEG images rendered with robust RGBA→RGB handling.
- Proven on SimpleQA: 14-page PDF converted at 150 DPI as demonstration.
