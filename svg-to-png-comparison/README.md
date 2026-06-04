# SVG to PNG Conversion Methods in Python

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This investigation compares different methods for converting SVG files to PNG format using Python. We tested 6 successful approaches using the [tiger.svg](https://gist.githubusercontent.com/simonw/aedecb93564af13ac1596810d40cac3c/raw/83e7f3be5b65bba61124684700fa7925d37c36c3/tiger.svg) file as our test case.

## Test Results Summary

| Method | File Size | Dimensions | Color Depth | Installation Complexity |
|--------|-----------|------------|-------------|------------------------|
| CairoSVG | 315 KB | 900×900 | 8-bit RGBA | Easy (pip only) |
| svglib + reportlab | 285 KB | 900×900 | 8-bit RGB | Easy (pip only) |
| Wand | 622 KB | 900×900 | 16-bit RGBA | Medium (requires ImageMagick) |
| Pillow + CairoSVG | 309 KB | 900×900 | 8-bit RGBA | Easy (pip only) |
| rsvg-convert CLI | 315 KB | 900×900 | 8-bit RGBA | Medium (requires system package) |
| ImageMagick CLI | 622 KB | 900×900 | 16-bit RGBA | Medium (requires system package) |

## Detailed Method Comparison

### 1. CairoSVG

**Description:** Pure Python library that uses the cairo graphics library for rendering.

**Code:**
```python
import cairosvg

cairosvg.svg2png(url="tiger.svg", write_to="output_cairosvg.png")
```

**Installation:**
```bash
pip install cairosvg
```

**Pros:**
- Pure Python solution
- Simple API
- Good rendering quality
- Widely used and maintained

**Cons:**
- Requires cairo system libraries (usually available)
- Moderate file size

**License:** LGPL-3.0

**Output:** [output_cairosvg.png](output_cairosvg.png) - 315 KB, 8-bit RGBA

---

### 2. svglib + reportlab

**Description:** Pure Python SVG parser combined with reportlab for rendering.

**Code:**
```python
from svglib.svglib import svg2rlg
from reportlab.graphics import renderPM

drawing = svg2rlg("tiger.svg")
renderPM.drawToFile(drawing, "output_svglib.png", fmt='PNG')
```

**Installation:**
```bash
pip install svglib reportlab
```

**Pros:**
- Pure Python solution
- Smallest file size (285 KB)
- No system dependencies
- Good for server environments

**Cons:**
- May not support all SVG features
- RGB only (no alpha channel)
- More complex API

**License:** LGPL-3.0 (svglib), BSD-like (reportlab)

**Output:** [output_svglib.png](output_svglib.png) - 285 KB, 8-bit RGB

---

### 3. Wand (ImageMagick Python Bindings)

**Description:** Python bindings for ImageMagick, a powerful image manipulation library.

**Code:**
```python
from wand.image import Image

with Image(filename="tiger.svg") as img:
    img.format = 'png'
    img.save(filename="output_wand.png")
```

**Installation:**
```bash
pip install Wand
# Also requires: apt-get install imagemagick (or brew install imagemagick)
```

**Pros:**
- High quality rendering
- 16-bit color depth
- Part of mature ImageMagick ecosystem
- Supports many image formats

**Cons:**
- Requires ImageMagick system installation
- Largest file size (622 KB)
- Additional dependency

**License:** MIT (Wand), Apache-2.0 (ImageMagick)

**Output:** [output_wand.png](output_wand.png) - 622 KB, 16-bit RGBA

---

### 4. Pillow + CairoSVG

**Description:** Combines Pillow (PIL fork) with CairoSVG for SVG support.

**Code:**
```python
from PIL import Image
import cairosvg
from io import BytesIO

png_data = cairosvg.svg2png(url="tiger.svg")
img = Image.open(BytesIO(png_data))
img.save("output_pillow_cairosvg.png")
```

**Installation:**
```bash
pip install Pillow cairosvg
```

**Pros:**
- Leverages popular Pillow library
- Allows additional image processing
- Pure Python
- Good quality

**Cons:**
- Requires two libraries
- Slightly more complex than pure CairoSVG

**License:** HPND (Pillow), LGPL-3.0 (CairoSVG)

**Output:** [output_pillow_cairosvg.png](output_pillow_cairosvg.png) - 309 KB, 8-bit RGBA

---

### 5. rsvg-convert CLI

**Description:** Command-line tool from librsvg (GNOME's SVG library) called via subprocess.

**Code:**
```python
import subprocess

subprocess.run(['rsvg-convert', '-o', 'output_rsvg.png', 'tiger.svg'], check=True)
```

**Installation:**
```bash
apt-get install librsvg2-bin  # Debian/Ubuntu
# or: brew install librsvg     # macOS
```

**Pros:**
- Very fast and efficient
- High quality rendering
- Battle-tested in GNOME ecosystem
- Simple subprocess call

**Cons:**
- Requires system package installation
- Less portable across systems
- Less control from Python

**License:** LGPL-2.1+ (librsvg)

**Output:** [output_rsvg.png](output_rsvg.png) - 315 KB, 8-bit RGBA

---

### 6. ImageMagick CLI

**Description:** Using ImageMagick's convert command via subprocess.

**Code:**
```python
import subprocess

subprocess.run(['convert', 'tiger.svg', 'output_imagemagick_cli.png'], check=True)
```

**Installation:**
```bash
apt-get install imagemagick  # Debian/Ubuntu
# or: brew install imagemagick # macOS
```

**Pros:**
- Very powerful and flexible
- 16-bit color depth
- Can handle many image formats
- Well-documented

**Cons:**
- Requires system installation
- Largest file size
- Potential security policies may block SVG conversion

**License:** Apache-2.0 (ImageMagick)

**Output:** [output_imagemagick_cli.png](output_imagemagick_cli.png) - 622 KB, 16-bit RGBA

---

## Recommendations

**Best for pure Python environments:** CairoSVG or svglib+reportlab
- Both require only pip installation
- svglib produces smaller files but lacks alpha channel
- CairoSVG has better SVG feature support

**Best for highest quality:** Wand or ImageMagick CLI
- 16-bit color depth
- Most accurate rendering
- Worth the extra file size if quality is paramount

**Best for server/Docker environments:** svglib+reportlab or CairoSVG
- No system dependencies (or minimal)
- Easy to install in containers
- Predictable behavior

**Best for performance:** rsvg-convert CLI
- Very fast
- Low memory usage
- Great for batch processing

## License Summary

| Library/Tool | License | Commercial Use | Notes |
|--------------|---------|----------------|-------|
| CairoSVG | LGPL-3.0 | ✓ (with conditions) | Can link dynamically |
| svglib | LGPL-3.0 | ✓ (with conditions) | Can link dynamically |
| reportlab | BSD-like | ✓ | Very permissive |
| Wand | MIT | ✓ | Very permissive |
| ImageMagick | Apache-2.0 | ✓ | Very permissive |
| Pillow | HPND | ✓ | Very permissive |
| librsvg | LGPL-2.1+ | ✓ (with conditions) | Can link dynamically |

**LGPL Note:** LGPL libraries can be used in commercial applications, but you must allow users to replace the LGPL library with a different version. This is typically satisfied by dynamic linking (normal pip install).

## Files Generated

All methods successfully generated PNG files from the tiger.svg source:

- `output_cairosvg.png` (315 KB)
- `output_svglib.png` (285 KB)
- `output_wand.png` (622 KB)
- `output_pillow_cairosvg.png` (309 KB)
- `output_rsvg.png` (315 KB)
- `output_imagemagick_cli.png` (622 KB)

## Conclusion

The choice of method depends on your specific requirements:

- **Portability:** Choose CairoSVG or svglib+reportlab
- **Quality:** Choose Wand or ImageMagick CLI
- **File Size:** Choose svglib+reportlab
- **Simplicity:** Choose CairoSVG
- **Performance:** Choose rsvg-convert CLI

All methods produced acceptable results with the tiger.svg test file. The 16-bit methods (Wand, ImageMagick CLI) produced larger files but with higher color precision, which may matter for certain use cases.
