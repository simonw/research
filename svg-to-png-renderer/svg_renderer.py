"""
SVG to PNG Renderer
A minimal SVG renderer using only xml.etree.ElementTree and Pillow.
"""

import xml.etree.ElementTree as ET
import re
from PIL import Image, ImageDraw
from typing import List, Tuple, Optional, Dict
import math


class SVGRenderer:
    """Renders SVG files to PNG using Pillow."""

    def __init__(self, svg_data: str):
        """
        Initialize the renderer with SVG data.

        Args:
            svg_data: String containing SVG XML data
        """
        self.root = ET.fromstring(svg_data)
        self.width, self.height = self._get_dimensions()
        self.image = Image.new('RGB', (self.width, self.height), 'white')
        self.draw = ImageDraw.Draw(self.image, 'RGBA')

    def _get_dimensions(self) -> Tuple[int, int]:
        """Extract width and height from SVG viewBox or dimensions."""
        viewbox = self.root.get('viewBox')
        if viewbox:
            parts = viewbox.split()
            width = int(float(parts[2]))
            height = int(float(parts[3]))
            return width, height

        # Fallback to width/height attributes
        width = self.root.get('width', '100')
        height = self.root.get('height', '100')

        # Remove units if present
        width = int(float(re.sub(r'[^0-9.]', '', str(width))))
        height = int(float(re.sub(r'[^0-9.]', '', str(height))))

        return width, height

    def render(self) -> Image.Image:
        """
        Render the SVG to a PIL Image.

        Returns:
            PIL Image object
        """
        # Process all elements
        self._render_element(self.root, {})
        return self.image

    def _render_element(self, element: ET.Element, inherited_attrs: Dict):
        """
        Recursively render an SVG element and its children.

        Args:
            element: XML element to render
            inherited_attrs: Attributes inherited from parent elements
        """
        # Merge inherited attributes with current element's attributes
        attrs = inherited_attrs.copy()
        attrs.update(element.attrib)

        # Get the tag name without namespace
        tag = element.tag.split('}')[-1] if '}' in element.tag else element.tag

        # Render based on element type
        if tag == 'path':
            self._render_path(element, attrs)
        elif tag == 'rect':
            self._render_rect(element, attrs)
        elif tag == 'circle':
            self._render_circle(element, attrs)
        elif tag == 'ellipse':
            self._render_ellipse(element, attrs)
        elif tag == 'line':
            self._render_line(element, attrs)
        elif tag == 'polyline':
            self._render_polyline(element, attrs)
        elif tag == 'polygon':
            self._render_polygon(element, attrs)
        elif tag in ['g', 'svg']:
            # For groups and nested SVGs, just process children
            pass

        # Process children
        for child in element:
            self._render_element(child, attrs)

    def _parse_color(self, color_str: Optional[str], default: str = '#000000') -> Tuple[int, int, int]:
        """
        Parse SVG color string to RGB tuple.

        Args:
            color_str: Color string (hex, named color, or None)
            default: Default color if parsing fails

        Returns:
            RGB tuple (r, g, b)
        """
        if not color_str or color_str == 'none':
            return None

        color_str = color_str.strip().lower()

        # Handle hex colors
        if color_str.startswith('#'):
            color_str = color_str[1:]
            if len(color_str) == 3:
                # Short form: #RGB -> #RRGGBB
                color_str = ''.join([c*2 for c in color_str])
            if len(color_str) == 6:
                r = int(color_str[0:2], 16)
                g = int(color_str[2:4], 16)
                b = int(color_str[4:6], 16)
                return (r, g, b)

        # Handle named colors (basic set)
        named_colors = {
            'white': (255, 255, 255),
            'black': (0, 0, 0),
            'red': (255, 0, 0),
            'green': (0, 128, 0),
            'blue': (0, 0, 255),
            'yellow': (255, 255, 0),
            'cyan': (0, 255, 255),
            'magenta': (255, 0, 255),
            'gray': (128, 128, 128),
            'grey': (128, 128, 128),
        }

        if color_str in named_colors:
            return named_colors[color_str]

        # Default fallback
        return self._parse_color(default, '#000000')

    def _get_transform_matrix(self, transform_str: Optional[str]) -> List[float]:
        """
        Parse SVG transform string into a transformation matrix.

        Args:
            transform_str: SVG transform attribute value

        Returns:
            List of 6 floats representing the transform matrix [a, b, c, d, e, f]
        """
        if not transform_str:
            return [1, 0, 0, 1, 0, 0]  # Identity matrix

        # Match matrix(...) function
        matrix_match = re.search(r'matrix\(([-\d., ]+)\)', transform_str)
        if matrix_match:
            values = [float(x.strip()) for x in matrix_match.group(1).split(',')]
            if len(values) == 6:
                return values

        # For simplicity, start with identity matrix
        # Could extend to handle translate, scale, rotate separately
        return [1, 0, 0, 1, 0, 0]

    def _apply_transform(self, x: float, y: float, matrix: List[float]) -> Tuple[float, float]:
        """
        Apply transformation matrix to a point.

        Args:
            x, y: Point coordinates
            matrix: Transform matrix [a, b, c, d, e, f]

        Returns:
            Transformed (x, y) coordinates
        """
        a, b, c, d, e, f = matrix
        new_x = a * x + c * y + e
        new_y = b * x + d * y + f
        return new_x, new_y

    def _parse_path_data(self, path_data: str) -> List[Tuple[float, float]]:
        """
        Parse SVG path data into a list of points.

        Args:
            path_data: SVG path 'd' attribute value

        Returns:
            List of (x, y) coordinate tuples
        """
        if not path_data:
            return []

        # Tokenize the path data
        tokens = re.findall(r'[MmLlHhVvCcSsQqTtAaZz]|[-+]?[0-9]*\.?[0-9]+(?:[eE][-+]?[0-9]+)?', path_data)

        points = []
        current_pos = [0.0, 0.0]
        path_start = [0.0, 0.0]
        last_control = None
        i = 0

        while i < len(tokens):
            cmd = tokens[i]

            # Move commands
            if cmd in 'Mm':
                i += 1
                x = float(tokens[i])
                i += 1
                y = float(tokens[i])

                if cmd == 'M':  # Absolute
                    current_pos = [x, y]
                else:  # Relative
                    current_pos = [current_pos[0] + x, current_pos[1] + y]

                path_start = current_pos[:]
                points.append(tuple(current_pos))
                last_control = None

            # Line commands
            elif cmd in 'Ll':
                i += 1
                x = float(tokens[i])
                i += 1
                y = float(tokens[i])

                if cmd == 'L':  # Absolute
                    current_pos = [x, y]
                else:  # Relative
                    current_pos = [current_pos[0] + x, current_pos[1] + y]

                points.append(tuple(current_pos))
                last_control = None

            # Horizontal line
            elif cmd in 'Hh':
                i += 1
                x = float(tokens[i])

                if cmd == 'H':  # Absolute
                    current_pos[0] = x
                else:  # Relative
                    current_pos[0] += x

                points.append(tuple(current_pos))
                last_control = None

            # Vertical line
            elif cmd in 'Vv':
                i += 1
                y = float(tokens[i])

                if cmd == 'V':  # Absolute
                    current_pos[1] = y
                else:  # Relative
                    current_pos[1] += y

                points.append(tuple(current_pos))
                last_control = None

            # Cubic Bezier curve
            elif cmd in 'Cc':
                i += 1
                x1 = float(tokens[i])
                i += 1
                y1 = float(tokens[i])
                i += 1
                x2 = float(tokens[i])
                i += 1
                y2 = float(tokens[i])
                i += 1
                x = float(tokens[i])
                i += 1
                y = float(tokens[i])

                if cmd == 'c':  # Relative
                    x1 += current_pos[0]
                    y1 += current_pos[1]
                    x2 += current_pos[0]
                    y2 += current_pos[1]
                    x += current_pos[0]
                    y += current_pos[1]

                # Approximate cubic Bezier with line segments
                curve_points = self._cubic_bezier(
                    current_pos[0], current_pos[1],
                    x1, y1, x2, y2, x, y
                )
                points.extend(curve_points)

                current_pos = [x, y]
                last_control = [x2, y2]

            # Smooth cubic Bezier curve
            elif cmd in 'Ss':
                i += 1
                x2 = float(tokens[i])
                i += 1
                y2 = float(tokens[i])
                i += 1
                x = float(tokens[i])
                i += 1
                y = float(tokens[i])

                # Calculate reflected control point
                if last_control:
                    x1 = 2 * current_pos[0] - last_control[0]
                    y1 = 2 * current_pos[1] - last_control[1]
                else:
                    x1, y1 = current_pos[0], current_pos[1]

                if cmd == 's':  # Relative
                    x2 += current_pos[0]
                    y2 += current_pos[1]
                    x += current_pos[0]
                    y += current_pos[1]

                # Approximate cubic Bezier with line segments
                curve_points = self._cubic_bezier(
                    current_pos[0], current_pos[1],
                    x1, y1, x2, y2, x, y
                )
                points.extend(curve_points)

                current_pos = [x, y]
                last_control = [x2, y2]

            # Close path
            elif cmd in 'Zz':
                if points and tuple(current_pos) != tuple(path_start):
                    points.append(tuple(path_start))
                current_pos = path_start[:]
                last_control = None

            i += 1

        return points

    def _cubic_bezier(self, x0, y0, x1, y1, x2, y2, x3, y3, steps=20) -> List[Tuple[float, float]]:
        """
        Approximate a cubic Bezier curve with line segments.

        Args:
            x0, y0: Start point
            x1, y1: First control point
            x2, y2: Second control point
            x3, y3: End point
            steps: Number of line segments to use

        Returns:
            List of (x, y) points along the curve
        """
        points = []
        for i in range(1, steps + 1):
            t = i / steps
            # Cubic Bezier formula
            x = (1-t)**3 * x0 + 3*(1-t)**2*t * x1 + 3*(1-t)*t**2 * x2 + t**3 * x3
            y = (1-t)**3 * y0 + 3*(1-t)**2*t * y1 + 3*(1-t)*t**2 * y2 + t**3 * y3
            points.append((x, y))
        return points

    def _render_path(self, element: ET.Element, attrs: Dict):
        """Render an SVG path element."""
        path_data = element.get('d', '')
        if not path_data:
            return

        # Parse path into points
        points = self._parse_path_data(path_data)
        if len(points) < 2:
            return

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        points = [self._apply_transform(x, y, transform) for x, y in points]

        # Get fill and stroke attributes
        fill = attrs.get('fill', 'black')
        stroke = attrs.get('stroke')
        stroke_width = float(attrs.get('stroke-width', '1'))

        # Draw filled path
        if fill and fill != 'none':
            fill_color = self._parse_color(fill)
            if fill_color and len(points) >= 3:
                self.draw.polygon(points, fill=fill_color)

        # Draw stroked path
        if stroke and stroke != 'none':
            stroke_color = self._parse_color(stroke)
            if stroke_color and len(points) >= 2:
                self.draw.line(points, fill=stroke_color, width=int(stroke_width))

    def _render_rect(self, element: ET.Element, attrs: Dict):
        """Render an SVG rect element."""
        x = float(attrs.get('x', '0'))
        y = float(attrs.get('y', '0'))
        width = float(attrs.get('width', '0'))
        height = float(attrs.get('height', '0'))

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        x1, y1 = self._apply_transform(x, y, transform)
        x2, y2 = self._apply_transform(x + width, y + height, transform)

        # Get fill and stroke
        fill = attrs.get('fill', 'black')
        stroke = attrs.get('stroke')
        stroke_width = float(attrs.get('stroke-width', '1'))

        # Draw
        fill_color = self._parse_color(fill) if fill != 'none' else None
        stroke_color = self._parse_color(stroke) if stroke and stroke != 'none' else None

        if fill_color:
            self.draw.rectangle([x1, y1, x2, y2], fill=fill_color)
        if stroke_color:
            self.draw.rectangle([x1, y1, x2, y2], outline=stroke_color, width=int(stroke_width))

    def _render_circle(self, element: ET.Element, attrs: Dict):
        """Render an SVG circle element."""
        cx = float(attrs.get('cx', '0'))
        cy = float(attrs.get('cy', '0'))
        r = float(attrs.get('r', '0'))

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        cx, cy = self._apply_transform(cx, cy, transform)

        # Get fill and stroke
        fill = attrs.get('fill', 'black')
        stroke = attrs.get('stroke')
        stroke_width = float(attrs.get('stroke-width', '1'))

        # Draw
        fill_color = self._parse_color(fill) if fill != 'none' else None
        stroke_color = self._parse_color(stroke) if stroke and stroke != 'none' else None

        bbox = [cx - r, cy - r, cx + r, cy + r]

        if fill_color:
            self.draw.ellipse(bbox, fill=fill_color)
        if stroke_color:
            self.draw.ellipse(bbox, outline=stroke_color, width=int(stroke_width))

    def _render_ellipse(self, element: ET.Element, attrs: Dict):
        """Render an SVG ellipse element."""
        cx = float(attrs.get('cx', '0'))
        cy = float(attrs.get('cy', '0'))
        rx = float(attrs.get('rx', '0'))
        ry = float(attrs.get('ry', '0'))

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        cx, cy = self._apply_transform(cx, cy, transform)

        # Get fill and stroke
        fill = attrs.get('fill', 'black')
        stroke = attrs.get('stroke')
        stroke_width = float(attrs.get('stroke-width', '1'))

        # Draw
        fill_color = self._parse_color(fill) if fill != 'none' else None
        stroke_color = self._parse_color(stroke) if stroke and stroke != 'none' else None

        bbox = [cx - rx, cy - ry, cx + rx, cy + ry]

        if fill_color:
            self.draw.ellipse(bbox, fill=fill_color)
        if stroke_color:
            self.draw.ellipse(bbox, outline=stroke_color, width=int(stroke_width))

    def _render_line(self, element: ET.Element, attrs: Dict):
        """Render an SVG line element."""
        x1 = float(attrs.get('x1', '0'))
        y1 = float(attrs.get('y1', '0'))
        x2 = float(attrs.get('x2', '0'))
        y2 = float(attrs.get('y2', '0'))

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        x1, y1 = self._apply_transform(x1, y1, transform)
        x2, y2 = self._apply_transform(x2, y2, transform)

        # Get stroke
        stroke = attrs.get('stroke', 'black')
        stroke_width = float(attrs.get('stroke-width', '1'))

        stroke_color = self._parse_color(stroke) if stroke != 'none' else None

        if stroke_color:
            self.draw.line([x1, y1, x2, y2], fill=stroke_color, width=int(stroke_width))

    def _render_polyline(self, element: ET.Element, attrs: Dict):
        """Render an SVG polyline element."""
        points_str = attrs.get('points', '')
        if not points_str:
            return

        # Parse points
        coords = re.findall(r'[-+]?[0-9]*\.?[0-9]+', points_str)
        points = [(float(coords[i]), float(coords[i+1])) for i in range(0, len(coords)-1, 2)]

        if len(points) < 2:
            return

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        points = [self._apply_transform(x, y, transform) for x, y in points]

        # Get stroke
        stroke = attrs.get('stroke', 'black')
        stroke_width = float(attrs.get('stroke-width', '1'))

        stroke_color = self._parse_color(stroke) if stroke != 'none' else None

        if stroke_color:
            self.draw.line(points, fill=stroke_color, width=int(stroke_width))

    def _render_polygon(self, element: ET.Element, attrs: Dict):
        """Render an SVG polygon element."""
        points_str = attrs.get('points', '')
        if not points_str:
            return

        # Parse points
        coords = re.findall(r'[-+]?[0-9]*\.?[0-9]+', points_str)
        points = [(float(coords[i]), float(coords[i+1])) for i in range(0, len(coords)-1, 2)]

        if len(points) < 3:
            return

        # Apply transformation
        transform = self._get_transform_matrix(attrs.get('transform'))
        points = [self._apply_transform(x, y, transform) for x, y in points]

        # Get fill and stroke
        fill = attrs.get('fill', 'black')
        stroke = attrs.get('stroke')
        stroke_width = float(attrs.get('stroke-width', '1'))

        fill_color = self._parse_color(fill) if fill != 'none' else None
        stroke_color = self._parse_color(stroke) if stroke and stroke != 'none' else None

        if fill_color:
            self.draw.polygon(points, fill=fill_color)
        if stroke_color:
            self.draw.line(points + [points[0]], fill=stroke_color, width=int(stroke_width))


def render_svg_to_png(svg_path: str, output_path: str):
    """
    Render an SVG file to a PNG file.

    Args:
        svg_path: Path to input SVG file
        output_path: Path to output PNG file
    """
    with open(svg_path, 'r', encoding='utf-8') as f:
        svg_data = f.read()

    renderer = SVGRenderer(svg_data)
    image = renderer.render()
    image.save(output_path, 'PNG')
    print(f"Rendered {svg_path} to {output_path}")
    print(f"Image size: {image.size}")


if __name__ == '__main__':
    import sys

    if len(sys.argv) != 3:
        print("Usage: python svg_renderer.py <input.svg> <output.png>")
        sys.exit(1)

    render_svg_to_png(sys.argv[1], sys.argv[2])
