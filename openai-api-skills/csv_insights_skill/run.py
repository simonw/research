import argparse
from pathlib import Path

import pandas as pd
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt


def write_report(df: pd.DataFrame, outpath: Path) -> None:
    lines = []
    lines.append(f"# CSV Insights Report\n")
    lines.append(f"**Rows:** {len(df)}  \n**Columns:** {len(df.columns)}\n")
    lines.append("\n## Columns\n")
    lines.append("\n".join([f"- `{c}` ({df[c].dtype})" for c in df.columns]))

    missing = df.isna().sum()
    if missing.any():
        lines.append("\n## Missing values\n")
        for col, count in missing[missing > 0].items():
            lines.append(f"- `{col}`: {int(count)}")
    else:
        lines.append("\n## Missing values\nNo missing values detected.\n")

    numeric = df.select_dtypes(include="number")
    if not numeric.empty:
        lines.append("\n## Numeric summary (describe)\n")
        lines.append(numeric.describe().to_markdown())

    outpath.write_text("\n".join(lines), encoding="utf-8")


def make_plot(df: pd.DataFrame, outpath: Path) -> None:
    numeric = df.select_dtypes(include="number")
    if numeric.empty:
        return
    col = numeric.columns[0]
    plt.figure()
    df[col].dropna().hist(bins=30)
    plt.title(f"Histogram: {col}")
    plt.xlabel(col)
    plt.ylabel("Count")
    plt.tight_layout()
    plt.savefig(outpath)
    plt.close()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", required=True, help="Path to input CSV")
    parser.add_argument("--outdir", required=True, help="Directory for outputs")
    args = parser.parse_args()

    inpath = Path(args.input)
    outdir = Path(args.outdir)
    outdir.mkdir(parents=True, exist_ok=True)

    df = pd.read_csv(inpath)

    write_report(df, outdir / "report.md")
    make_plot(df, outdir / "plot.png")
    print(f"Report written to {outdir / 'report.md'}")
    print(f"Plot written to {outdir / 'plot.png'}")


if __name__ == "__main__":
    main()
