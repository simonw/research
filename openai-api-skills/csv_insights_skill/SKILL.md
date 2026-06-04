---
name: csv-insights
description: Summarize a CSV, compute basic stats, and produce a markdown report + a plot image.
---

# CSV Insights Skill

## When to use this
Use this skill when the user provides a CSV file and wants:
- a quick summary (row/col counts, missing values)
- basic numeric statistics
- a simple visualization
- results packaged into an output folder (or zip)

## Inputs
- A CSV file path (local) or a file mounted in the container.

## Outputs
- `output/report.md`
- `output/plot.png`

## How to run

python -m pip install -r requirements.txt
python run.py --input <csv_file> --outdir output
