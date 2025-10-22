"""
Generate performance charts from benchmark results
"""

import json
import matplotlib.pyplot as plt
import seaborn as sns
import pandas as pd
import numpy as np

# Set style
sns.set_style("whitegrid")
plt.rcParams['figure.figsize'] = (12, 6)

# Load benchmark results
with open('/home/user/research/python-markdown-comparison/benchmark_results.json', 'r') as f:
    data = json.load(f)

results = data['results']
libraries = data['libraries']

# Prepare data for visualization
chart_data = []
for doc_name, doc_results in results.items():
    for lib_name, lib_results in doc_results.items():
        if lib_results:
            chart_data.append({
                'Library': lib_name,
                'Document': doc_name.replace('_', ' ').title(),
                'Mean Time (ms)': lib_results['mean'],
                'Std Dev': lib_results['stdev'],
                'Implementation': libraries[lib_name]['implementation']
            })

df = pd.DataFrame(chart_data)

# Chart 1: Performance by document size (bar chart)
fig, ax = plt.subplots(figsize=(14, 7))

doc_order = ['Small Basic', 'Medium Mixed', 'Large Complex']
lib_order = df.groupby('Library')['Mean Time (ms)'].mean().sort_values().index.tolist()

# Create grouped bar chart
x = np.arange(len(doc_order))
width = 0.12
colors = sns.color_palette("husl", len(lib_order))

for i, lib in enumerate(lib_order):
    lib_data = df[df['Library'] == lib].sort_values('Document',
                                                     key=lambda x: x.map({d: i for i, d in enumerate(doc_order)}))
    means = lib_data['Mean Time (ms)'].values
    ax.bar(x + i * width, means, width, label=lib, color=colors[i])

ax.set_xlabel('Document Type', fontsize=12, fontweight='bold')
ax.set_ylabel('Rendering Time (ms)', fontsize=12, fontweight='bold')
ax.set_title('Python Markdown Library Performance Comparison', fontsize=14, fontweight='bold', pad=20)
ax.set_xticks(x + width * (len(lib_order) - 1) / 2)
ax.set_xticklabels(doc_order)
ax.legend(loc='upper left', fontsize=10)
ax.grid(axis='y', alpha=0.3)

plt.tight_layout()
plt.savefig('/home/user/research/python-markdown-comparison/charts/performance_comparison.png', dpi=300, bbox_inches='tight')
print("Saved: performance_comparison.png")
plt.close()

# Chart 2: Relative performance (cmarkgfm as baseline)
fig, ax = plt.subplots(figsize=(12, 7))

# Calculate relative performance for large document
large_data = df[df['Document'] == 'Large Complex'].set_index('Library')
baseline = large_data.loc['cmarkgfm', 'Mean Time (ms)']
large_data['Relative Speed'] = large_data['Mean Time (ms)'] / baseline

large_sorted = large_data.sort_values('Relative Speed')

colors_relative = ['#2ecc71' if x == 'cmarkgfm' else '#3498db' if 'Pure Python' in large_data.loc[x, 'Implementation'] else '#e74c3c'
                   for x in large_sorted.index]

bars = ax.barh(large_sorted.index, large_sorted['Relative Speed'], color=colors_relative, edgecolor='black', linewidth=0.5)

# Add value labels
for i, (idx, row) in enumerate(large_sorted.iterrows()):
    ax.text(row['Relative Speed'] + 0.5, i, f"{row['Relative Speed']:.1f}x",
            va='center', fontsize=10, fontweight='bold')

ax.axvline(x=1, color='red', linestyle='--', linewidth=2, alpha=0.7, label='cmarkgfm baseline')
ax.set_xlabel('Relative Speed (lower is better)', fontsize=12, fontweight='bold')
ax.set_ylabel('Library', fontsize=12, fontweight='bold')
ax.set_title('Relative Performance vs cmarkgfm (Large Document)', fontsize=14, fontweight='bold', pad=20)
ax.grid(axis='x', alpha=0.3)
ax.legend()

plt.tight_layout()
plt.savefig('/home/user/research/python-markdown-comparison/charts/relative_performance.png', dpi=300, bbox_inches='tight')
print("Saved: relative_performance.png")
plt.close()

# Chart 3: Performance scaling with document size
fig, ax = plt.subplots(figsize=(12, 7))

# Get document sizes
doc_sizes = {
    'Small Basic': 104,
    'Medium Mixed': 647,
    'Large Complex': 10573
}

for lib in lib_order:
    lib_data = df[df['Library'] == lib].sort_values('Document',
                                                     key=lambda x: x.map({d: i for i, d in enumerate(doc_order)}))
    sizes = [doc_sizes[doc] for doc in lib_data['Document']]
    times = lib_data['Mean Time (ms)'].values

    ax.plot(sizes, times, marker='o', linewidth=2, markersize=8, label=lib)

ax.set_xlabel('Document Size (characters)', fontsize=12, fontweight='bold')
ax.set_ylabel('Rendering Time (ms)', fontsize=12, fontweight='bold')
ax.set_title('Performance Scaling by Document Size', fontsize=14, fontweight='bold', pad=20)
ax.set_xscale('log')
ax.set_yscale('log')
ax.legend(loc='upper left', fontsize=10)
ax.grid(True, alpha=0.3, which='both')

plt.tight_layout()
plt.savefig('/home/user/research/python-markdown-comparison/charts/performance_scaling.png', dpi=300, bbox_inches='tight')
print("Saved: performance_scaling.png")
plt.close()

# Chart 4: Performance distribution (box plot)
fig, ax = plt.subplots(figsize=(14, 7))

# Use only large document for distribution
large_df = df[df['Document'] == 'Large Complex'].copy()

# Create a more detailed visualization
positions = range(len(lib_order))
bp = ax.boxplot([large_df[large_df['Library'] == lib]['Mean Time (ms)'].values for lib in lib_order],
                 positions=positions,
                 labels=lib_order,
                 patch_artist=True,
                 showmeans=True,
                 meanprops=dict(marker='D', markerfacecolor='red', markersize=8))

# Color the boxes
for patch, color in zip(bp['boxes'], colors):
    patch.set_facecolor(color)
    patch.set_alpha(0.7)

ax.set_ylabel('Rendering Time (ms)', fontsize=12, fontweight='bold')
ax.set_xlabel('Library', fontsize=12, fontweight='bold')
ax.set_title('Performance Distribution (Large Document)', fontsize=14, fontweight='bold', pad=20)
ax.grid(axis='y', alpha=0.3)
plt.xticks(rotation=45, ha='right')

plt.tight_layout()
plt.savefig('/home/user/research/python-markdown-comparison/charts/performance_distribution.png', dpi=300, bbox_inches='tight')
print("Saved: performance_distribution.png")
plt.close()

# Chart 5: Speed comparison table (as an image)
fig, ax = plt.subplots(figsize=(12, 8))
ax.axis('tight')
ax.axis('off')

# Create comparison table
table_data = []
for lib in lib_order:
    lib_data = df[df['Library'] == lib]
    row = [lib]
    for doc in doc_order:
        doc_data = lib_data[lib_data['Document'] == doc]
        if not doc_data.empty:
            mean_time = doc_data['Mean Time (ms)'].values[0]
            row.append(f"{mean_time:.2f}")
        else:
            row.append("N/A")
    row.append(libraries[lib]['implementation'])
    table_data.append(row)

headers = ['Library'] + doc_order + ['Implementation']
table = ax.table(cellText=table_data, colLabels=headers, cellLoc='center', loc='center',
                colWidths=[0.15, 0.12, 0.12, 0.12, 0.3])

table.auto_set_font_size(False)
table.set_fontsize(10)
table.scale(1, 2)

# Color header
for i in range(len(headers)):
    table[(0, i)].set_facecolor('#3498db')
    table[(0, i)].set_text_props(weight='bold', color='white')

# Color cells based on performance (for large document column)
large_col_idx = 3
values = [float(row[large_col_idx]) for row in table_data]
min_val, max_val = min(values), max(values)

for i, row in enumerate(table_data):
    val = float(row[large_col_idx])
    # Normalize between 0 and 1
    norm_val = (val - min_val) / (max_val - min_val) if max_val > min_val else 0
    # Green for fast, red for slow
    color = plt.cm.RdYlGn_r(norm_val)
    table[(i + 1, large_col_idx)].set_facecolor(color)
    table[(i + 1, large_col_idx)].set_alpha(0.3)

plt.title('Performance Summary (times in milliseconds)', fontsize=14, fontweight='bold', pad=20)
plt.savefig('/home/user/research/python-markdown-comparison/charts/performance_table.png', dpi=300, bbox_inches='tight')
print("Saved: performance_table.png")
plt.close()

print("\nAll charts generated successfully!")
