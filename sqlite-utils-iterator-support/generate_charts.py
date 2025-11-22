"""
Generate performance charts from benchmark results
"""
import json
import matplotlib
matplotlib.use('Agg')  # Non-interactive backend
import matplotlib.pyplot as plt
import numpy as np


def load_results():
    """Load benchmark results from JSON file"""
    with open('/home/user/research/sqlite-utils-iterator-support/benchmark_results.json', 'r') as f:
        return json.load(f)


def create_comparison_chart(results, output_path):
    """Create a bar chart comparing dict vs list mode performance"""
    # Filter insert results only
    insert_results = [r for r in results if 'upsert' not in r['name']]

    # Group by scenario
    scenarios = {}
    for r in insert_results:
        base_name = r['name'].rsplit('_', 1)[0]
        if base_name not in scenarios:
            scenarios[base_name] = {}
        scenarios[base_name][r['mode']] = r

    # Prepare data for plotting
    scenario_names = []
    dict_times = []
    list_times = []

    for scenario_name in sorted(scenarios.keys()):
        modes = scenarios[scenario_name]
        if 'dict' in modes and 'list' in modes:
            # Create readable scenario name
            parts = scenario_name.split('_')
            readable_name = f"{parts[0]} rows\n{parts[2]} cols"
            scenario_names.append(readable_name)
            dict_times.append(modes['dict']['elapsed_seconds'])
            list_times.append(modes['list']['elapsed_seconds'])

    # Create the chart
    x = np.arange(len(scenario_names))
    width = 0.35

    fig, ax = plt.subplots(figsize=(12, 6))
    bars1 = ax.bar(x - width/2, dict_times, width, label='Dict Mode', color='#3498db')
    bars2 = ax.bar(x + width/2, list_times, width, label='List Mode', color='#2ecc71')

    ax.set_xlabel('Scenario', fontsize=12, fontweight='bold')
    ax.set_ylabel('Time (seconds)', fontsize=12, fontweight='bold')
    ax.set_title('INSERT Performance: Dict Mode vs List Mode', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels(scenario_names)
    ax.legend()
    ax.grid(axis='y', alpha=0.3)

    # Add value labels on bars
    def autolabel(bars):
        for bar in bars:
            height = bar.get_height()
            ax.annotate(f'{height:.2f}s',
                       xy=(bar.get_x() + bar.get_width() / 2, height),
                       xytext=(0, 3),
                       textcoords="offset points",
                       ha='center', va='bottom',
                       fontsize=9)

    autolabel(bars1)
    autolabel(bars2)

    plt.tight_layout()
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved comparison chart to {output_path}")
    plt.close()


def create_speedup_chart(results, output_path):
    """Create a chart showing speedup factors"""
    # Filter insert results
    insert_results = [r for r in results if 'upsert' not in r['name']]

    # Group by scenario
    scenarios = {}
    for r in insert_results:
        base_name = r['name'].rsplit('_', 1)[0]
        if base_name not in scenarios:
            scenarios[base_name] = {}
        scenarios[base_name][r['mode']] = r

    # Calculate speedups
    scenario_names = []
    speedups = []
    colors = []

    for scenario_name in sorted(scenarios.keys()):
        modes = scenarios[scenario_name]
        if 'dict' in modes and 'list' in modes:
            parts = scenario_name.split('_')
            readable_name = f"{parts[0]} rows, {parts[2]} cols"
            scenario_names.append(readable_name)

            dict_time = modes['dict']['elapsed_seconds']
            list_time = modes['list']['elapsed_seconds']
            speedup = dict_time / list_time if list_time > 0 else 0
            speedups.append(speedup)

            # Color based on speedup
            if speedup > 1:
                colors.append('#2ecc71')  # Green for improvement
            else:
                colors.append('#e74c3c')  # Red for regression

    # Create the chart
    fig, ax = plt.subplots(figsize=(12, 6))
    bars = ax.barh(scenario_names, speedups, color=colors)

    # Add reference line at 1.0
    ax.axvline(x=1.0, color='gray', linestyle='--', linewidth=2, alpha=0.7, label='No Change')

    ax.set_xlabel('Speedup Factor (Dict Time / List Time)', fontsize=12, fontweight='bold')
    ax.set_title('List Mode Speedup Over Dict Mode', fontsize=14, fontweight='bold')
    ax.legend()
    ax.grid(axis='x', alpha=0.3)

    # Add value labels
    for i, (bar, speedup) in enumerate(zip(bars, speedups)):
        improvement = (speedup - 1) * 100
        label = f'{speedup:.2f}x ({improvement:+.1f}%)'
        ax.text(speedup + 0.02, i, label, va='center', fontsize=10)

    plt.tight_layout()
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved speedup chart to {output_path}")
    plt.close()


def create_throughput_chart(results, output_path):
    """Create a chart showing rows per second throughput"""
    # Filter insert results
    insert_results = [r for r in results if 'upsert' not in r['name']]

    # Group by scenario
    scenarios = {}
    for r in insert_results:
        base_name = r['name'].rsplit('_', 1)[0]
        if base_name not in scenarios:
            scenarios[base_name] = {}
        scenarios[base_name][r['mode']] = r

    # Prepare data
    scenario_names = []
    dict_throughput = []
    list_throughput = []

    for scenario_name in sorted(scenarios.keys()):
        modes = scenarios[scenario_name]
        if 'dict' in modes and 'list' in modes:
            parts = scenario_name.split('_')
            readable_name = f"{parts[0]} rows\n{parts[2]} cols"
            scenario_names.append(readable_name)
            dict_throughput.append(modes['dict']['rows_per_second'])
            list_throughput.append(modes['list']['rows_per_second'])

    # Create the chart
    x = np.arange(len(scenario_names))
    width = 0.35

    fig, ax = plt.subplots(figsize=(12, 6))
    bars1 = ax.bar(x - width/2, dict_throughput, width, label='Dict Mode', color='#3498db')
    bars2 = ax.bar(x + width/2, list_throughput, width, label='List Mode', color='#2ecc71')

    ax.set_xlabel('Scenario', fontsize=12, fontweight='bold')
    ax.set_ylabel('Throughput (rows/second)', fontsize=12, fontweight='bold')
    ax.set_title('INSERT Throughput Comparison', fontsize=14, fontweight='bold')
    ax.set_xticks(x)
    ax.set_xticklabels(scenario_names)
    ax.legend()
    ax.grid(axis='y', alpha=0.3)

    # Add value labels
    def autolabel(bars):
        for bar in bars:
            height = bar.get_height()
            ax.annotate(f'{height:.0f}',
                       xy=(bar.get_x() + bar.get_width() / 2, height),
                       xytext=(0, 3),
                       textcoords="offset points",
                       ha='center', va='bottom',
                       fontsize=9)

    autolabel(bars1)
    autolabel(bars2)

    plt.tight_layout()
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved throughput chart to {output_path}")
    plt.close()


def create_column_count_analysis(results, output_path):
    """Analyze performance vs column count"""
    # Filter insert results
    insert_results = [r for r in results if 'upsert' not in r['name']]

    # Extract column counts and speedups
    data_points = []
    for r in insert_results:
        base_name = r['name'].rsplit('_', 1)[0]
        if r['mode'] == 'dict':
            # Find corresponding list mode result
            list_result = next((lr for lr in insert_results
                              if lr['name'].rsplit('_', 1)[0] == base_name and lr['mode'] == 'list'),
                             None)
            if list_result:
                speedup = r['elapsed_seconds'] / list_result['elapsed_seconds']
                data_points.append({
                    'columns': r['column_count'],
                    'speedup': speedup,
                    'name': base_name
                })

    # Sort by column count
    data_points.sort(key=lambda x: x['columns'])

    columns = [d['columns'] for d in data_points]
    speedups = [d['speedup'] for d in data_points]
    names = [d['name'].replace('_', ' ') for d in data_points]

    # Create the chart
    fig, ax = plt.subplots(figsize=(10, 6))
    scatter = ax.scatter(columns, speedups, s=200, alpha=0.6, c=speedups,
                        cmap='RdYlGn', vmin=0.5, vmax=1.5, edgecolors='black', linewidth=1.5)

    # Add horizontal line at 1.0
    ax.axhline(y=1.0, color='gray', linestyle='--', linewidth=2, alpha=0.7, label='No Change')

    # Add labels for each point
    for i, name in enumerate(names):
        ax.annotate(f'{speedups[i]:.2f}x',
                   xy=(columns[i], speedups[i]),
                   xytext=(10, 10),
                   textcoords='offset points',
                   fontsize=9,
                   bbox=dict(boxstyle='round,pad=0.3', facecolor='yellow', alpha=0.5))

    ax.set_xlabel('Number of Columns', fontsize=12, fontweight='bold')
    ax.set_ylabel('Speedup Factor (List / Dict)', fontsize=12, fontweight='bold')
    ax.set_title('Performance vs Column Count', fontsize=14, fontweight='bold')
    ax.legend()
    ax.grid(alpha=0.3)

    # Add colorbar
    cbar = plt.colorbar(scatter, ax=ax)
    cbar.set_label('Speedup Factor', fontsize=10)

    plt.tight_layout()
    plt.savefig(output_path, dpi=300, bbox_inches='tight')
    print(f"Saved column count analysis to {output_path}")
    plt.close()


if __name__ == "__main__":
    results = load_results()

    print("Generating performance charts...")
    create_comparison_chart(results, '/home/user/research/sqlite-utils-iterator-support/chart_comparison.png')
    create_speedup_chart(results, '/home/user/research/sqlite-utils-iterator-support/chart_speedup.png')
    create_throughput_chart(results, '/home/user/research/sqlite-utils-iterator-support/chart_throughput.png')
    create_column_count_analysis(results, '/home/user/research/sqlite-utils-iterator-support/chart_columns.png')

    print("\nAll charts generated successfully!")
