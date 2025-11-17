#!/usr/bin/env python3
"""
Method 3: Wand (ImageMagick Python bindings)
Requires ImageMagick to be installed on the system.
"""

from wand.image import Image

def convert_svg_to_png(svg_path, png_path):
    with Image(filename=svg_path) as img:
        img.format = 'png'
        img.save(filename=png_path)
    print(f"✓ Converted {svg_path} to {png_path}")

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_wand.png")
