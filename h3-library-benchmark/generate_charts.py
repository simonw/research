"""
Generate comparison charts from benchmark results.
"""
import json
import matplotlib.pyplot as plt
import matplotlib.patches as mpatches
import numpy as np

# Load benchmark results
with open('benchmark_results.json', 'r') as f:
    results = json.load(f)

# Set up plotting style
plt.style.use('seaborn-v0_8-darkgrid')
colors = {'h3-py': '#1f77b4', 'h3o-python': '#ff7f0e'}

def create_speedup_comparison():
    """Create a bar chart comparing speedup across operations."""
    fig, ax = plt.subplots(figsize=(12, 7))

    operations = ['latlng_to_cell', 'cell_to_latlng', 'grid_disk_k1', 'grid_disk_k2', 'grid_disk_k5', 'cell_area']
    op_labels = ['latlng_to_cell', 'cell_to_latlng', 'grid_disk (k=1)', 'grid_disk (k=2)', 'grid_disk (k=5)', 'cell_area']

    # Use medium scale, resolution 9 for representative results
    scale = 'medium'
    resolution = 'res_9'

    speedups = []
    for op in operations:
        if op in results['benchmarks'][scale][resolution]:
            op_results = results['benchmarks'][scale][resolution][op]
            h3py_ops = op_results['h3-py']['ops_per_sec']
            h3o_ops = op_results['h3o-python']['ops_per_sec']
            speedup = h3o_ops / h3py_ops
            speedups.append(speedup)
        else:
            speedups.append(0)

    x = np.arange(len(op_labels))
    bars = ax.bar(x, speedups, color='#2ca02c', alpha=0.8)

    # Color bars differently based on performance
    for i, (bar, speedup) in enumerate(zip(bars, speedups)):
        if speedup > 1:
            bar.set_color('#2ca02c')  # Green for faster
        else:
            bar.set_color('#d62728')  # Red for slower

    # Add value labels on bars
    for i, (bar, speedup) in enumerate(zip(bars, speedups)):
        height = bar.get_height()
        ax.text(bar.get_x() + bar.get_width()/2., height,
                f'{speedup:.2f}x',
                ha='center', va='bottom', fontsize=10, fontweight='bold')

    # Add reference line at 1.0
    ax.axhline(y=1.0, color='black', linestyle='--', linewidth=1, alpha=0.5, label='Baseline (1.0x)')

    ax.set_ylabel('Speedup Factor (h3o-python vs h3-py)', fontsize=12, fontweight='bold')
    ax.set_xlabel('Operation', fontsize=12, fontweight='bold')
    ax.set_title('h3o-python Performance vs h3-py\n(Resolution 9, 1,000 operations)', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels(op_labels, rotation=30, ha='right')
    ax.legend()
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig('speedup_comparison.png', dpi=300, bbox_inches='tight')
    print("✓ Generated speedup_comparison.png")


def create_ops_per_sec_by_scale():
    """Create chart showing ops/sec across different scales."""
    fig, axes = plt.subplots(2, 2, figsize=(15, 12))
    axes = axes.flatten()

    operations = [
        ('latlng_to_cell', 'latlng_to_cell - Coordinate to Cell'),
        ('cell_to_latlng', 'cell_to_latlng - Cell to Coordinate'),
        ('grid_disk_k1', 'grid_disk (k=1) - Neighbor Query'),
        ('cell_area', 'cell_area - Area Calculation')
    ]

    scales = ['small', 'medium', 'large', 'xlarge']
    scale_labels = ['100', '1K', '10K', '100K']
    resolution = 'res_9'

    for idx, (op, title) in enumerate(operations):
        ax = axes[idx]

        h3py_values = []
        h3o_values = []

        for scale in scales:
            if op in results['benchmarks'][scale][resolution]:
                op_results = results['benchmarks'][scale][resolution][op]
                h3py_values.append(op_results['h3-py']['ops_per_sec'])
                h3o_values.append(op_results['h3o-python']['ops_per_sec'])
            else:
                h3py_values.append(0)
                h3o_values.append(0)

        x = np.arange(len(scales))
        width = 0.35

        bars1 = ax.bar(x - width/2, h3py_values, width, label='h3-py', color=colors['h3-py'], alpha=0.8)
        bars2 = ax.bar(x + width/2, h3o_values, width, label='h3o-python', color=colors['h3o-python'], alpha=0.8)

        ax.set_ylabel('Operations per Second', fontsize=11, fontweight='bold')
        ax.set_xlabel('Dataset Scale (operations)', fontsize=11, fontweight='bold')
        ax.set_title(title, fontsize=12, fontweight='bold')
        ax.set_xticks(x)
        ax.set_xticklabels(scale_labels)
        ax.legend()
        ax.grid(True, alpha=0.3, axis='y')

        # Use log scale for y-axis to handle wide range of values
        ax.set_yscale('log')

    plt.suptitle('Performance Across Different Scales (Resolution 9)', fontsize=16, fontweight='bold', y=1.00)
    plt.tight_layout()
    plt.savefig('ops_by_scale.png', dpi=300, bbox_inches='tight')
    print("✓ Generated ops_by_scale.png")


def create_resolution_comparison():
    """Create chart showing performance across resolutions."""
    fig, axes = plt.subplots(1, 2, figsize=(15, 6))

    operations = [
        ('latlng_to_cell', 'latlng_to_cell'),
        ('cell_to_latlng', 'cell_to_latlng')
    ]

    resolutions = [5, 7, 9, 11]
    scale = 'medium'

    for idx, (op, title) in enumerate(operations):
        ax = axes[idx]

        h3py_values = []
        h3o_values = []

        for res in resolutions:
            res_key = f'res_{res}'
            op_results = results['benchmarks'][scale][res_key][op]
            h3py_values.append(op_results['h3-py']['ops_per_sec'])
            h3o_values.append(op_results['h3o-python']['ops_per_sec'])

        x = np.arange(len(resolutions))
        width = 0.35

        bars1 = ax.bar(x - width/2, h3py_values, width, label='h3-py', color=colors['h3-py'], alpha=0.8)
        bars2 = ax.bar(x + width/2, h3o_values, width, label='h3o-python', color=colors['h3o-python'], alpha=0.8)

        ax.set_ylabel('Operations per Second', fontsize=12, fontweight='bold')
        ax.set_xlabel('Resolution', fontsize=12, fontweight='bold')
        ax.set_title(title, fontsize=13, fontweight='bold')
        ax.set_xticks(x)
        ax.set_xticklabels(resolutions)
        ax.legend()
        ax.grid(True, alpha=0.3, axis='y')

    plt.suptitle('Performance Across H3 Resolutions (1,000 operations)', fontsize=16, fontweight='bold')
    plt.tight_layout()
    plt.savefig('resolution_comparison.png', dpi=300, bbox_inches='tight')
    print("✓ Generated resolution_comparison.png")


def create_grid_disk_comparison():
    """Create detailed chart for grid_disk performance."""
    fig, ax = plt.subplots(figsize=(10, 7))

    k_values = [1, 2, 5]
    scale = 'medium'
    resolution = 'res_9'

    h3py_values = []
    h3o_values = []

    for k in k_values:
        op = f'grid_disk_k{k}'
        op_results = results['benchmarks'][scale][resolution][op]
        h3py_values.append(op_results['h3-py']['ops_per_sec'])
        h3o_values.append(op_results['h3o-python']['ops_per_sec'])

    x = np.arange(len(k_values))
    width = 0.35

    bars1 = ax.bar(x - width/2, h3py_values, width, label='h3-py', color=colors['h3-py'], alpha=0.8)
    bars2 = ax.bar(x + width/2, h3o_values, width, label='h3o-python', color=colors['h3o-python'], alpha=0.8)

    # Add value labels on bars
    for bars in [bars1, bars2]:
        for bar in bars:
            height = bar.get_height()
            ax.text(bar.get_x() + bar.get_width()/2., height,
                    f'{height:,.0f}',
                    ha='center', va='bottom', fontsize=9)

    ax.set_ylabel('Operations per Second', fontsize=12, fontweight='bold')
    ax.set_xlabel('k Value (Ring Distance)', fontsize=12, fontweight='bold')
    ax.set_title('grid_disk Performance by k Value\n(Resolution 9, 100 cells tested)', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels([f'k={k}' for k in k_values])
    ax.legend(fontsize=11)
    ax.grid(True, alpha=0.3, axis='y')

    plt.tight_layout()
    plt.savefig('grid_disk_comparison.png', dpi=300, bbox_inches='tight')
    print("✓ Generated grid_disk_comparison.png")


def create_summary_table_image():
    """Create an image of a summary table."""
    fig, ax = plt.subplots(figsize=(12, 8))
    ax.axis('tight')
    ax.axis('off')

    # Compile summary statistics
    scale = 'medium'
    resolution = 'res_9'

    table_data = [
        ['Operation', 'h3-py\n(ops/sec)', 'h3o-python\n(ops/sec)', 'Speedup'],
        ['─' * 20, '─' * 12, '─' * 15, '─' * 10],
    ]

    operations = [
        ('latlng_to_cell', 'latlng_to_cell'),
        ('cell_to_latlng', 'cell_to_latlng'),
        ('grid_disk_k1', 'grid_disk (k=1)'),
        ('grid_disk_k2', 'grid_disk (k=2)'),
        ('grid_disk_k5', 'grid_disk (k=5)'),
        ('cell_area', 'cell_area'),
    ]

    for op, label in operations:
        if op in results['benchmarks'][scale][resolution]:
            op_results = results['benchmarks'][scale][resolution][op]
            h3py_ops = op_results['h3-py']['ops_per_sec']
            h3o_ops = op_results['h3o-python']['ops_per_sec']
            speedup = h3o_ops / h3py_ops

            table_data.append([
                label,
                f'{h3py_ops:,.0f}',
                f'{h3o_ops:,.0f}',
                f'{speedup:.2f}x'
            ])

    table = ax.table(cellText=table_data, cellLoc='left', loc='center',
                     colWidths=[0.35, 0.2, 0.25, 0.2])

    table.auto_set_font_size(False)
    table.set_fontsize(11)
    table.scale(1, 2.5)

    # Style header row
    for i in range(4):
        cell = table[(0, i)]
        cell.set_facecolor('#4CAF50')
        cell.set_text_props(weight='bold', color='white')

    # Color speedup cells
    for i in range(2, len(table_data)):
        cell = table[(i, 3)]
        speedup_val = float(table_data[i][3].replace('x', ''))
        if speedup_val > 1:
            cell.set_facecolor('#e8f5e9')
        else:
            cell.set_facecolor('#ffebee')

    plt.title('Performance Summary (Resolution 9, 1,000 operations)',
              fontsize=14, fontweight='bold', pad=20)
    plt.savefig('summary_table.png', dpi=300, bbox_inches='tight')
    print("✓ Generated summary_table.png")


if __name__ == '__main__':
    print("Generating benchmark visualization charts...")
    print()

    create_speedup_comparison()
    create_ops_per_sec_by_scale()
    create_resolution_comparison()
    create_grid_disk_comparison()
    create_summary_table_image()

    print()
    print("All charts generated successfully!")
