#!/usr/bin/env python3
"""
Method 7: Playwright (Browser-based rendering)
Uses a headless browser to render the SVG and take a screenshot.
Requires playwright and browser installation.
"""

from playwright.sync_api import sync_playwright
import os

def convert_svg_to_png(svg_path, png_path):
    # Read SVG content
    with open(svg_path, 'r') as f:
        svg_content = f.read()

    # Create HTML wrapper for SVG
    html_content = f"""
    <!DOCTYPE html>
    <html>
    <head>
        <meta charset="utf-8">
        <style>
            body {{ margin: 0; padding: 0; }}
            svg {{ display: block; }}
        </style>
    </head>
    <body>
        {svg_content}
    </body>
    </html>
    """

    with sync_playwright() as p:
        browser = p.chromium.launch()
        page = browser.new_page()
        page.set_content(html_content)

        # Get SVG dimensions
        svg_element = page.locator('svg')
        bbox = svg_element.bounding_box()

        # Take screenshot
        page.screenshot(path=png_path, clip={
            'x': bbox['x'],
            'y': bbox['y'],
            'width': bbox['width'],
            'height': bbox['height']
        })

        browser.close()

    print(f"✓ Converted {svg_path} to {png_path}")

if __name__ == "__main__":
    convert_svg_to_png("tiger.svg", "output_playwright.png")
