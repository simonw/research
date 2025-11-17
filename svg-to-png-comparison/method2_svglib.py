#!/usr/bin/env python3
"""
Method 2: svglib + reportlab
Pure Python solution using svglib to parse SVG and reportlab to render.
"""

from svglib.svglib import svg2rlg
from reportlab.graphics import renderPM

def convert_svg_to_png(svg_path, png_path):
    drawing = svg2rlg(svg_path)
    renderPM.drawToFile(drawing, png_path, fmt='PNG')
    print(f"✓ Converted {svg_path} to {png_path}")

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_svglib.png")
