# SVG to PNG Renderer

A minimal Python library that renders SVG files to PNG images using only `xml.etree.ElementTree` and `Pillow`. No additional libraries required.

## Overview

This library provides a lightweight SVG renderer that can parse SVG XML data and convert it to raster PNG images. It implements the core SVG rendering features needed for most simple to moderately complex SVG files.

## Features

### Supported SVG Elements

- **Paths**: Full support for SVG path data with the following commands:
  - `M`, `m` - Move to
  - `L`, `l` - Line to
  - `H`, `h` - Horizontal line to
  - `V`, `v` - Vertical line to
  - `C`, `c` - Cubic Bezier curve
  - `S`, `s` - Smooth cubic Bezier curve
  - `Z`, `z` - Close path

- **Basic Shapes**:
  - `<rect>` - Rectangles
  - `<circle>` - Circles
  - `<ellipse>` - Ellipses
  - `<line>` - Lines
  - `<polyline>` - Polylines
  - `<polygon>` - Polygons

- **Containers**:
  - `<g>` - Groups (with attribute inheritance)
  - `<svg>` - Nested SVG elements

### Supported Attributes

- **Colors**:
  - Hex colors (`#RGB`, `#RRGGBB`)
  - Named colors (white, black, red, green, blue, etc.)

- **Styling**:
  - `fill` - Fill color for shapes and paths
  - `stroke` - Stroke color for outlines
  - `stroke-width` - Width of strokes

- **Transforms**:
  - `matrix(a,b,c,d,e,f)` - Affine transformation matrices

- **Dimensions**:
  - `viewBox` - SVG coordinate system
  - `width`, `height` - Canvas dimensions

## Installation

No installation needed beyond Python and Pillow:

```bash
pip install Pillow
```

## Usage

### As a Command-Line Tool

```bash
python svg_renderer.py input.svg output.png
```

### As a Library

```python
from svg_renderer import SVGRenderer

# Load SVG from file
with open('input.svg', 'r') as f:
    svg_data = f.read()

# Create renderer and render
renderer = SVGRenderer(svg_data)
image = renderer.render()

# Save to PNG
image.save('output.png')
```

### Example: Rendering the Tiger SVG

```bash
python svg_renderer.py tiger.svg tiger.png
```

This successfully renders a complex 900x900 SVG with 838+ path elements to a 40KB PNG file.

## Implementation Details

### Path Rendering

SVG paths are parsed into a list of coordinates and rendered using Pillow's drawing primitives:
- **Filled paths**: Rendered using `ImageDraw.polygon()`
- **Stroked paths**: Rendered using `ImageDraw.line()`

### Bezier Curve Approximation

Cubic Bezier curves are approximated using 20 line segments. The cubic Bezier formula is evaluated at equal intervals:

```
B(t) = (1-t)³P₀ + 3(1-t)²tP₁ + 3(1-t)t²P₂ + t³P₃
```

where `P₀` is the start point, `P₁` and `P₂` are control points, and `P₃` is the end point.

### Transformation Matrices

Transform matrices are applied to all coordinates before rendering:

```
[x']   [a c e]   [x]
[y'] = [b d f] × [y]
[1 ]   [0 0 1]   [1]
```

### Attribute Inheritance

Attributes (fill, stroke, etc.) are inherited from parent elements to children, allowing grouped styling.

## Limitations

### Not Supported

- Gradients and patterns
- Text elements
- Opacity/transparency (partial support only)
- Masks and filters
- Individual transform functions (translate, scale, rotate) - only matrix() supported
- Quadratic Bezier curves (Q, q, T, t)
- Elliptical arc curves (A, a)
- Advanced CSS styling
- Embedded images
- Animations

### Known Issues

- Bezier approximation uses fixed 20 steps (not adaptive)
- Transform parsing is limited to matrix() function
- Color support limited to basic hex and named colors
- No support for `fill-rule` or `clip-path`

## Architecture

The renderer is organized into several key components:

### `SVGRenderer` Class

Main class that orchestrates the rendering process.

**Key Methods**:
- `__init__(svg_data)` - Initialize with SVG XML data
- `render()` - Render SVG to PIL Image
- `_render_element(element, attrs)` - Recursively render elements
- `_parse_path_data(path_data)` - Parse SVG path commands
- `_parse_color(color_str)` - Parse color strings
- `_get_transform_matrix(transform)` - Parse transform matrices
- `_apply_transform(x, y, matrix)` - Apply transforms to coordinates

**Shape Rendering Methods**:
- `_render_path()` - Render path elements
- `_render_rect()` - Render rectangles
- `_render_circle()` - Render circles
- `_render_ellipse()` - Render ellipses
- `_render_line()` - Render lines
- `_render_polyline()` - Render polylines
- `_render_polygon()` - Render polygons

### Path Data Parser

Uses regular expressions to tokenize SVG path data strings:

```python
tokens = re.findall(
    r'[MmLlHhVvCcSsQqTtAaZz]|[-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?',
    path_data
)
```

This extracts both commands and numeric parameters.

## Testing

Tested with the "Ghostscript Tiger" SVG:
- **Source**: https://gist.githubusercontent.com/simonw/aedecb93564af13ac1596810d40cac3c/raw/83e7f3be5b65bba61124684700fa7925d37c36c3/tiger.svg
- **Complexity**: 838+ path elements
- **Result**: Successfully rendered to 900x900 PNG (40KB)

## Technical Notes

### Why Only elementtree and Pillow?

This project demonstrates that SVG rendering can be accomplished with minimal dependencies:
- **elementtree** - Standard library XML parser
- **Pillow** - Common image manipulation library

No need for heavy SVG libraries like `svglib`, `cairosvg`, or browser engines.

### Performance Considerations

- Path parsing is done using regex, which is fast but could be optimized
- Bezier approximation quality vs. performance trade-off (20 segments)
- All rendering is done in-memory with Pillow's drawing API

### Future Enhancements

Possible improvements:
1. Adaptive Bezier approximation based on curve complexity
2. Support for translate(), scale(), rotate() transforms
3. Quadratic Bezier and arc path commands
4. Text rendering (would require font handling)
5. Gradient support (complex, would need significant work)
6. Opacity and alpha channel support
7. CSS style parsing
8. Performance optimizations (caching, vectorization)

## Files

- `svg_renderer.py` - Main renderer implementation (~600 lines)
- `tiger.svg` - Test SVG file (Ghostscript Tiger)
- `tiger.png` - Rendered output (40KB, 900x900)
- `notes.md` - Development notes and learnings
- `README.md` - This file

## License

This is a research project created for educational purposes.

## Author

Created as a demonstration of minimal SVG rendering using only Python standard library and Pillow.
