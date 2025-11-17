#!/usr/bin/env python3
"""
Method 4: Pillow with CairoSVG backend
Uses Pillow for image manipulation with CairoSVG to handle SVG rendering.
"""

from PIL import Image
import cairosvg
from io import BytesIO

def convert_svg_to_png(svg_path, png_path):
    # Convert SVG to PNG bytes using cairosvg
    png_data = cairosvg.svg2png(url=svg_path)

    # Open with Pillow for additional processing if needed
    img = Image.open(BytesIO(png_data))
    img.save(png_path)
    print(f"✓ Converted {svg_path} to {png_path}")

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_pillow_cairosvg.png")
