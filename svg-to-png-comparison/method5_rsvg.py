#!/usr/bin/env python3
"""
Method 5: rsvg-convert (librsvg) via subprocess
Uses the rsvg-convert command-line tool from librsvg.
Requires librsvg2-bin to be installed.
"""

import subprocess
import sys

def convert_svg_to_png(svg_path, png_path):
    try:
        result = subprocess.run(
            ['rsvg-convert', '-o', png_path, svg_path],
            check=True,
            capture_output=True,
            text=True
        )
        print(f"✓ Converted {svg_path} to {png_path}")
    except FileNotFoundError:
        print("✗ rsvg-convert not found. Install with: apt-get install librsvg2-bin")
        sys.exit(1)
    except subprocess.CalledProcessError as e:
        print(f"✗ Conversion failed: {e.stderr}")
        sys.exit(1)

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_rsvg.png")
