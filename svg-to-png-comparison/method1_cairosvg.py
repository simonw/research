#!/usr/bin/env python3
"""
Method 1: CairoSVG
A popular pure Python library that uses cairo for rendering.
"""

import cairosvg

def convert_svg_to_png(svg_path, png_path):
    cairosvg.svg2png(url=svg_path, write_to=png_path)
    print(f"✓ Converted {svg_path} to {png_path}")

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_cairosvg.png")
