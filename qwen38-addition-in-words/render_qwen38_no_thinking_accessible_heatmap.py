#!/usr/bin/env python3
"""Render the completed Qwen3.8 no-thinking result grid accessibly."""

from __future__ import annotations

import csv
import json
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont


SOURCE_DIR = Path("runs/qwen3.8-27b-q4_k_m-seed-20260929")
SOURCE_CSV = SOURCE_DIR / "cell_accuracy.csv"
SOURCE_SUMMARY = SOURCE_DIR / "summary.json"
OUTPUT_DIR = SOURCE_DIR / "accessible-chart"
OUTPUT_PATH = OUTPUT_DIR / "heatmap.png"

LOW_ORANGE = (230, 159, 0)  # Okabe-Ito orange
MID_NEUTRAL = (247, 247, 247)
HIGH_BLUE = (0, 114, 178)  # Okabe-Ito blue
MISSING_GREY = (224, 228, 232)


def interpolate_color(value: float) -> tuple[int, int, int]:
    """Continuous colorblind-safe orange/neutral/blue accuracy scale."""
    value = max(0.0, min(1.0, value))
    if value <= 0.5:
        t = value / 0.5
        return tuple(
            round(LOW_ORANGE[i] + t * (MID_NEUTRAL[i] - LOW_ORANGE[i]))
            for i in range(3)
        )
    t = (value - 0.5) / 0.5
    return tuple(
        round(MID_NEUTRAL[i] + t * (HIGH_BLUE[i] - MID_NEUTRAL[i]))
        for i in range(3)
    )


def load_font(candidates: list[str], size: int) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    for candidate in candidates:
        if Path(candidate).exists():
            return ImageFont.truetype(candidate, size=size)
    return ImageFont.load_default()


def centered_text(
    draw: ImageDraw.ImageDraw,
    box: tuple[float, float, float, float],
    text: str,
    *,
    font: ImageFont.FreeTypeFont | ImageFont.ImageFont,
    fill: str | tuple[int, int, int],
) -> None:
    bbox = draw.textbbox((0, 0), text, font=font)
    width = bbox[2] - bbox[0]
    height = bbox[3] - bbox[1]
    left, top, right, bottom = box
    draw.text(
        (left + (right - left - width) / 2, top + (bottom - top - height) / 2 - 3),
        text,
        fill=fill,
        font=font,
    )


def main() -> None:
    with SOURCE_CSV.open(newline="", encoding="utf-8") as handle:
        rows = list(csv.DictReader(handle))
    summary = json.loads(SOURCE_SUMMARY.read_text(encoding="utf-8"))

    cell_by_key = {(int(row["a_digits"]), int(row["b_digits"])): row for row in rows}
    if len(rows) != 169 or len(cell_by_key) != 169:
        raise SystemExit("Expected exactly 169 unique ordered digit cells")
    if any(int(row["n"]) != 30 for row in rows):
        raise SystemExit("Expected n=30 in every digit cell")
    if sum(int(row["n"]) for row in rows) != 5_070:
        raise SystemExit("Cell counts do not sum to 5,070")
    if sum(int(row["numeric_correct"]) for row in rows) != 1_195:
        raise SystemExit("Cell correct counts do not sum to 1,195")
    if int(summary["completed_cases"]) != 5_070 or int(summary["numeric_correct"]) != 1_195:
        raise SystemExit("Summary and cell data disagree")

    width, height = 1720, 1580
    left, top = 185, 190
    cell_size = 92
    image = Image.new("RGB", (width, height), "white")
    draw = ImageDraw.Draw(image)

    regular = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    ]
    bold = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
    ]
    title_font = load_font(bold, 38)
    subtitle_font = load_font(regular, 25)
    label_font = load_font(regular, 23)
    cell_font = load_font(bold, 21)
    small_font = load_font(regular, 18)
    footnote_font = load_font(regular, 17)

    draw.text(
        (left, 28),
        "Addition in words — Qwen3.8 27B Q4_K_M",
        fill=(20, 20, 20),
        font=title_font,
    )
    draw.text(
        (left, 82),
        "Reasoning disabled · 30 fixed pairs per ordered digit-length cell (n = 5,070)",
        fill=(50, 50, 50),
        font=subtitle_font,
    )
    draw.text(
        (left, 124),
        "Overall numeric accuracy: 1,195 / 5,070 (23.57%)",
        fill=(70, 70, 70),
        font=small_font,
    )

    grid_right = left + 13 * cell_size
    grid_bottom = top + 13 * cell_size
    for a_digits in range(1, 14):
        x = left + (a_digits - 1) * cell_size
        centered_text(
            draw,
            (x, grid_bottom + 8, x + cell_size, grid_bottom + 48),
            str(a_digits),
            font=label_font,
            fill=(20, 20, 20),
        )

    for b_digits in range(1, 14):
        y = top + (13 - b_digits) * cell_size
        centered_text(
            draw,
            (left - 58, y, left - 8, y + cell_size),
            str(b_digits),
            font=label_font,
            fill=(20, 20, 20),
        )
        for a_digits in range(1, 14):
            x = left + (a_digits - 1) * cell_size
            row = cell_by_key[(a_digits, b_digits)]
            accuracy = float(row["numeric_accuracy"])
            color = interpolate_color(accuracy)
            draw.rectangle(
                (x, y, x + cell_size, y + cell_size),
                fill=color,
                outline=(255, 255, 255),
                width=2,
            )
            simple_luminance = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2]
            text_color = "white" if simple_luminance < 115 else "black"
            centered_text(
                draw,
                (x, y, x + cell_size, y + cell_size),
                f"{100 * accuracy:.0f}%",
                font=cell_font,
                fill=text_color,
            )

    x_label = "Number of digits in a"
    bbox = draw.textbbox((0, 0), x_label, font=label_font)
    draw.text(
        (left + (13 * cell_size - (bbox[2] - bbox[0])) / 2, grid_bottom + 58),
        x_label,
        fill=(20, 20, 20),
        font=label_font,
    )
    y_layer = Image.new("RGBA", (600, 60), (255, 255, 255, 0))
    y_draw = ImageDraw.Draw(y_layer)
    y_draw.text((0, 10), "Number of digits in b", fill=(20, 20, 20), font=label_font)
    y_layer = y_layer.rotate(90, expand=True)
    image.paste(y_layer, (28, top + 330), y_layer)

    legend_x = grid_right + 80
    legend_y = top + 170
    legend_height = 520
    draw.text((legend_x - 5, legend_y - 52), "Accuracy", fill=(20, 20, 20), font=label_font)
    for offset in range(legend_height):
        value = 1 - offset / (legend_height - 1)
        draw.line(
            (legend_x, legend_y + offset, legend_x + 38, legend_y + offset),
            fill=interpolate_color(value),
        )
    for value in [1.0, 0.75, 0.5, 0.25, 0.0]:
        y = legend_y + round((1 - value) * (legend_height - 1))
        draw.line((legend_x + 39, y, legend_x + 49, y), fill=(20, 20, 20), width=2)
        draw.text(
            (legend_x + 57, y - 10),
            f"{100 * value:.0f}%",
            fill=(20, 20, 20),
            font=small_font,
        )

    draw.text(
        (left, height - 48),
        "Colorblind-safe orange–blue scale; percentages provide a redundant non-color encoding.",
        fill=(70, 70, 70),
        font=footnote_font,
    )

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    image.save(OUTPUT_PATH)
    print(OUTPUT_PATH.resolve())


if __name__ == "__main__":
    main()
