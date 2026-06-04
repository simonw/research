"""pdf2img_py - Convert PDF pages to images using pdfium-render."""

import os

# Set up library path to find bundled libpdfium.so before importing native module
_this_dir = os.path.dirname(os.path.abspath(__file__))
_bundled = os.path.join(_this_dir, "..", "bundled_libs")
if os.path.isdir(_bundled):
    os.environ.setdefault("PDFIUM_LIB_PATH", _bundled)

from ._native import pdf_to_images, page_count, render_page

__all__ = ["pdf_to_images", "page_count", "render_page"]
