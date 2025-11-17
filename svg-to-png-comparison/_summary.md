Multiple Python-based approaches for converting SVG files to PNG were benchmarked using the [tiger.svg](https://gist.githubusercontent.com/simonw/aedecb93564af13ac1596810d40cac3c/raw/83e7f3be5b65bba61124684700fa7925d37c36c3/tiger.svg) image, evaluating file size, output quality, and ease of installation. Pure Python solutions like [CairoSVG](https://github.com/Kozea/CairoSVG) and svglib+reportlab offered simple pip-based installs with predictable PNGs, though svglib lacks alpha channel support. Wand (ImageMagick bindings) and ImageMagick CLI yielded the highest quality output (16-bit RGBA) at the cost of larger files and system-level dependencies. In contrast, rsvg-convert CLI stood out for speed and batch suitability, while Pillow+CairoSVG enabled further in-Python image manipulation. Ultimately, selection depends on priorities—portability (CairoSVG, svglib), maximal quality (Wand, ImageMagick), minimal footprint (svglib), or performance (rsvg-convert).

Key findings:
- svglib+reportlab produced the smallest files, but without alpha (RGB only).
- Wand and ImageMagick CLI generated 16-bit PNGs, twice the size of other methods.
- CairoSVG remains the simplest, most widely compatible pure Python solution.
- rsvg-convert CLI is fastest for batch or server-side use but needs system install.
