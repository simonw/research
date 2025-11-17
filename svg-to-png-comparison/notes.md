# SVG to PNG Conversion Investigation Notes

## Goal
Explore different Python methods for rendering SVG files to PNG format.

## SVG Source
Using tiger.svg from: https://gist.githubusercontent.com/simonw/aedecb93564af13ac1596810d40cac3c/raw/83e7f3be5b65bba61124684700fa7925d37c36c3/tiger.svg

## Investigation Log

### Setup
- Created project folder: svg-to-png-comparison
- Starting investigation on 2025-11-17
- Downloaded tiger.svg (68KB SVG file)

### Methods Tested

#### Successful Methods

1. **CairoSVG** ✓
   - Pure Python library using cairo
   - Output: 315KB, 900x900, 8-bit RGBA
   - Installation: `pip install cairosvg`

2. **svglib + reportlab** ✓
   - Pure Python solution
   - Output: 285KB, 900x900, 8-bit RGB
   - Installation: `pip install svglib reportlab`

3. **Wand** ✓
   - ImageMagick Python bindings
   - Output: 622KB, 900x900, 16-bit RGBA
   - Installation: `pip install Wand` (requires ImageMagick)

4. **Pillow + CairoSVG** ✓
   - Pillow with CairoSVG backend
   - Output: 309KB, 900x900, 8-bit RGBA
   - Installation: `pip install Pillow cairosvg`

5. **rsvg-convert CLI** ✓
   - librsvg command-line tool via subprocess
   - Output: 315KB, 900x900, 8-bit RGBA
   - Requires: librsvg2-bin

6. **ImageMagick CLI** ✓
   - ImageMagick convert command via subprocess
   - Output: 622KB, 900x900, 16-bit RGBA
   - Requires: ImageMagick

#### Failed/Skipped Methods

7. **Playwright** ✗
   - Browser-based rendering
   - Issue: Timeout locating SVG element
   - Too complex for simple SVG conversion

## Key Findings

- **File sizes:** Range from 285KB (svglib) to 622KB (Wand/ImageMagick CLI)
- **Color depth:** Most use 8-bit, Wand/ImageMagick use 16-bit
- **Ease of use:** Pure Python solutions (CairoSVG, svglib) are easiest to deploy
- **Quality:** All methods produced good quality results; 16-bit methods offer higher precision

## Recommendations by Use Case

1. **Docker/Server deployments:** svglib+reportlab or CairoSVG (no system deps)
2. **Maximum quality:** Wand or ImageMagick CLI (16-bit color)
3. **Smallest files:** svglib+reportlab (285KB)
4. **Best balance:** CairoSVG (good quality, easy install, reasonable size)
5. **Batch processing:** rsvg-convert CLI (fastest)

## License Considerations

- LGPL libraries (CairoSVG, svglib, librsvg) require dynamic linking for commercial use
- MIT/Apache/BSD libraries (Wand, ImageMagick, Pillow, reportlab) are very permissive
- All can be used in commercial projects with appropriate compliance
