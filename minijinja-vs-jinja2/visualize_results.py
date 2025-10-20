#!/usr/bin/env python3
"""
Generate visualization charts from benchmark results.
"""
import json
import argparse
from pathlib import Path
import matplotlib.pyplot as plt
import matplotlib
import numpy as np

# Use Agg backend for headless environments
matplotlib.use('Agg')


def load_results(results_dir: str) -> dict:
    """Load all benchmark result JSON files."""
    results_path = Path(results_dir)
    results = {}

    for json_file in results_path.glob("*.json"):
        with open(json_file, "r") as f:
            data = json.load(f)
            # Create a key from engine and python version
            key = f"{data['engine']}"
            if data.get('free_threaded'):
                key += " (3.14t)"
            else:
                key += " (3.14)"
            results[key] = data

    return results


def create_comparison_chart(results: dict, output_file: str):
    """Create a bar chart comparing mean rendering times."""
    fig, ax = plt.subplots(figsize=(12, 6))

    labels = list(results.keys())
    means = [results[k]['mean_ms'] for k in labels]
    stdevs = [results[k]['stdev_ms'] for k in labels]

    x = np.arange(len(labels))
    bars = ax.bar(x, means, yerr=stdevs, capsize=5, alpha=0.8)

    # Color bars differently for minijinja vs jinja2
    for i, (label, bar) in enumerate(zip(labels, bars)):
        if 'minijinja' in label:
            bar.set_color('#ff6b6b')
        else:
            bar.set_color('#4ecdc4')

    ax.set_xlabel('Configuration', fontsize=12, fontweight='bold')
    ax.set_ylabel('Mean Rendering Time (ms)', fontsize=12, fontweight='bold')
    ax.set_title('Template Rendering Performance Comparison', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels(labels, rotation=45, ha='right')
    ax.grid(axis='y', alpha=0.3)

    # Add value labels on bars
    for i, (mean, stdev) in enumerate(zip(means, stdevs)):
        ax.text(i, mean + stdev, f'{mean:.2f}ms', ha='center', va='bottom', fontsize=9)

    plt.tight_layout()
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"Saved comparison chart to {output_file}")
    plt.close()


def create_distribution_chart(results: dict, output_file: str):
    """Create box plots showing distribution of rendering times."""
    fig, ax = plt.subplots(figsize=(12, 6))

    data = []
    labels = []
    colors = []

    for label, result in results.items():
        data.append(result['times_ms'])
        labels.append(label)
        if 'minijinja' in label:
            colors.append('#ff6b6b')
        else:
            colors.append('#4ecdc4')

    bp = ax.boxplot(data, labels=labels, patch_artist=True, showmeans=True)

    # Color the boxes
    for patch, color in zip(bp['boxes'], colors):
        patch.set_facecolor(color)
        patch.set_alpha(0.6)

    ax.set_ylabel('Rendering Time (ms)', fontsize=12, fontweight='bold')
    ax.set_title('Distribution of Rendering Times', fontsize=14, fontweight='bold')
    ax.set_xticklabels(labels, rotation=45, ha='right')
    ax.grid(axis='y', alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"Saved distribution chart to {output_file}")
    plt.close()


def create_speedup_chart(results: dict, output_file: str):
    """Create a chart showing speedup of minijinja vs jinja2."""
    fig, ax = plt.subplots(figsize=(10, 6))

    # Group by Python version
    speedups = {}

    for label, result in results.items():
        if '3.14t' in label:
            py_version = 'Python 3.14t (free-threaded)'
        else:
            py_version = 'Python 3.14'

        if py_version not in speedups:
            speedups[py_version] = {}

        if 'minijinja' in label:
            speedups[py_version]['minijinja'] = result['mean_ms']
        else:
            speedups[py_version]['jinja2'] = result['mean_ms']

    # Calculate speedup factors
    versions = list(speedups.keys())
    speedup_factors = []

    for version in versions:
        if 'jinja2' in speedups[version] and 'minijinja' in speedups[version]:
            speedup = speedups[version]['jinja2'] / speedups[version]['minijinja']
            speedup_factors.append(speedup)
        else:
            speedup_factors.append(0)

    x = np.arange(len(versions))
    bars = ax.bar(x, speedup_factors, alpha=0.8, color='#95e1d3')

    ax.set_xlabel('Python Version', fontsize=12, fontweight='bold')
    ax.set_ylabel('Speedup Factor (jinja2 time / minijinja time)', fontsize=12, fontweight='bold')
    ax.set_title('minijinja Performance Advantage', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels(versions)
    ax.axhline(y=1.0, color='red', linestyle='--', alpha=0.5, label='No speedup')
    ax.grid(axis='y', alpha=0.3)
    ax.legend()

    # Add value labels
    for i, speedup in enumerate(speedup_factors):
        if speedup > 0:
            ax.text(i, speedup, f'{speedup:.2f}x', ha='center', va='bottom', fontsize=11, fontweight='bold')

    plt.tight_layout()
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"Saved speedup chart to {output_file}")
    plt.close()


def create_timeline_chart(results: dict, output_file: str):
    """Create a chart showing rendering times over iterations."""
    fig, ax = plt.subplots(figsize=(14, 6))

    for label, result in results.items():
        times = result['times_ms']
        iterations = range(1, len(times) + 1)

        if 'minijinja' in label:
            linestyle = '-'
            alpha = 0.7
        else:
            linestyle = '--'
            alpha = 0.7

        ax.plot(iterations, times, label=label, linestyle=linestyle, alpha=alpha, linewidth=1.5)

    ax.set_xlabel('Iteration', fontsize=12, fontweight='bold')
    ax.set_ylabel('Rendering Time (ms)', fontsize=12, fontweight='bold')
    ax.set_title('Rendering Time Across Iterations', fontsize=14, fontweight='bold')
    ax.legend(loc='best')
    ax.grid(alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_file, dpi=300, bbox_inches='tight')
    print(f"Saved timeline chart to {output_file}")
    plt.close()


def main():
    parser = argparse.ArgumentParser(description="Generate benchmark visualization charts")
    parser.add_argument(
        "--results-dir",
        default="./results",
        help="Directory containing benchmark result JSON files"
    )
    parser.add_argument(
        "--output-dir",
        default="./charts",
        help="Directory to save chart images"
    )

    args = parser.parse_args()

    # Create output directory
    output_path = Path(args.output_dir)
    output_path.mkdir(exist_ok=True)

    # Load results
    print(f"Loading results from {args.results_dir}...")
    results = load_results(args.results_dir)

    if not results:
        print("No results found!")
        return

    print(f"Found {len(results)} result sets")

    # Generate charts
    create_comparison_chart(results, f"{args.output_dir}/comparison.png")
    create_distribution_chart(results, f"{args.output_dir}/distribution.png")
    create_speedup_chart(results, f"{args.output_dir}/speedup.png")
    create_timeline_chart(results, f"{args.output_dir}/timeline.png")

    print("\nAll charts generated successfully!")


if __name__ == "__main__":
    main()
