# SVG to PNG Renderer - Development Notes

## Goal
Build a Python library that can read SVG data using elementtree and render it to a Pillow image using low-level drawing operations. No other libraries allowed.

## Test Case
- Tiger SVG from: https://gist.githubusercontent.com/simonw/aedecb93564af13ac1596810d40cac3c/raw/83e7f3be5b65bba61124684700fa7925d37c36c3/tiger.svg
- Dimensions: 900x900 viewBox
- Contains 838+ path elements with various colors and transforms

## Initial Analysis

### SVG Structure
- Root `<svg>` element with viewBox and version
- Nested `<g>` groups with:
  - fill attributes
  - stroke attributes
  - stroke-width attributes
  - transform matrices
- `<path>` elements with `d` attribute containing path data

### Key Challenges
1. **Path Data Parsing**: Need to parse SVG path commands (M, L, C, S, Z, etc.)
2. **Transformations**: Handle matrix transforms for scaling/positioning
3. **Color Parsing**: Parse hex colors, handle fill and stroke
4. **Coordinate Systems**: Map SVG coordinates to Pillow image
5. **Path Rendering**: Convert Bezier curves to points for Pillow

## Implementation Plan

### Phase 1: Basic Structure
- [ ] Create SVG parser using xml.etree.ElementTree
- [ ] Set up Pillow image with appropriate dimensions
- [ ] Parse viewBox to understand coordinate system

### Phase 2: Path Parsing
- [ ] Implement path data tokenizer
- [ ] Handle absolute commands: M, L, C, S, Z
- [ ] Handle relative commands: m, l, c, s, z
- [ ] Convert cubic Bezier curves to line segments

### Phase 3: Rendering
- [ ] Implement fill rendering
- [ ] Implement stroke rendering
- [ ] Handle stroke-width

### Phase 4: Transformations
- [ ] Parse transform matrices
- [ ] Apply transformations to coordinates

### Phase 5: Colors & Attributes
- [ ] Parse hex colors
- [ ] Parse named colors
- [ ] Handle opacity

## Development Log

### 2025-11-17

**Initial Setup**
- Created project folder: svg-to-png-renderer
- Downloaded tiger.svg test file
- Analyzed SVG structure

**Implementation**
- Built comprehensive SVG renderer in `svg_renderer.py` (~600 lines)
- Implemented full path data parser supporting:
  - Move commands: M, m
  - Line commands: L, l, H, h, V, v
  - Cubic Bezier: C, c, S, s
  - Close path: Z, z
- Implemented cubic Bezier approximation using 20 line segments
- Added support for basic shapes:
  - rect, circle, ellipse, line, polyline, polygon
- Transform matrix parsing and application
- Color parsing (hex colors and named colors)
- Attribute inheritance through element tree

**Testing**
- Successfully rendered tiger.svg to tiger.png
- Output: 900x900 PNG, 40KB file size
- All path elements rendered correctly
- Transforms applied properly

**Learned**
1. SVG path data is complex but can be tokenized with regex
2. Cubic Bezier curves need approximation for raster rendering
3. Pillow's ImageDraw doesn't support native Bezier curves
4. Transform matrices need to be applied to all coordinates
5. Attribute inheritance is important for grouped elements
6. Path fill uses polygon(), stroke uses line()

**Limitations**
- No support for gradients or patterns
- No support for text elements
- Limited opacity support
- No support for masks or filters
- Transform only supports matrix(), not individual translate/scale/rotate
- Bezier approximation is fixed at 20 steps (could be adaptive)

**What Worked Well**
- elementtree made XML parsing straightforward
- Regex tokenization handled path data effectively
- Pillow's drawing primitives were sufficient for basic rendering
- Recursive element processing with attribute inheritance

