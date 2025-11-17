#!/usr/bin/env python3
"""
Method 6: ImageMagick CLI via subprocess
Uses the convert command from ImageMagick.
Requires ImageMagick to be installed.
"""

import subprocess
import sys

def convert_svg_to_png(svg_path, png_path):
    try:
        result = subprocess.run(
            ['convert', svg_path, png_path],
            check=True,
            capture_output=True,
            text=True
        )
        print(f"✓ Converted {svg_path} to {png_path}")
    except FileNotFoundError:
        print("✗ ImageMagick convert not found. Install with: apt-get install imagemagick")
        sys.exit(1)
    except subprocess.CalledProcessError as e:
        print(f"✗ Conversion failed: {e.stderr}")
        sys.exit(1)

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_imagemagick_cli.png")
