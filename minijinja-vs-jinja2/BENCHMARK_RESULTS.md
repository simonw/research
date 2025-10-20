# minijinja vs jinja2 Performance Benchmark

## Executive Summary

This benchmark compares the performance of [minijinja](https://github.com/mitsuhiko/minijinja) Python bindings against the standard [jinja2](https://palletsprojects.com/p/jinja/) template engine across four different scenarios:

1. **jinja2** on **Python 3.14** (regular build)
2. **minijinja** on **Python 3.14** (regular build)
3. **jinja2** on **Python 3.14t** (free-threaded build)
4. **minijinja** on **Python 3.14t** (free-threaded build)

### Key Findings

- **jinja2 is faster than minijinja** in this benchmark on regular Python 3.14 (0.990 ms vs 1.528 ms mean render time)
- **Free-threaded Python (3.14t) slows down jinja2** by ~14% (0.990 ms → 1.127 ms)
- **Free-threaded Python improves minijinja performance** by ~13% (1.528 ms → 1.336 ms)
- **minijinja shows better free-threaded compatibility** than jinja2

## Benchmark Setup

### Environment

- **Python Versions**: CPython 3.14.0rc2 (both regular and free-threaded builds)
- **jinja2 Version**: 3.1.6
- **minijinja Version**: 2.12.0 (built from latest main branch)
- **Package Manager**: uv 0.8.17
- **Platform**: Linux x86_64

### Template Complexity

The benchmark uses a realistic e-commerce product listing template with:

- **Template inheritance** (base.html → products.html)
- **Multiple loops** (navigation items, products, tags, category paths)
- **Conditional rendering** (sale badges, new/featured flags)
- **Variable interpolation** (50+ variables per product)
- **Arithmetic operations** (discount calculations)
- **List operations** (length checks, iterations with loop variables)
- **50 products** rendered per iteration
- **200 iterations** per benchmark run
- **~65KB output** HTML per render

### Data Characteristics

Each benchmark iteration generates:
- 50 product records with randomized data
- Each product contains 13 fields including strings, numbers, booleans, and lists
- Multiple nested loops processing ~300+ individual data points
- Complex conditionals and calculations

## Results

### Performance Summary

| Configuration | Mean (ms) | Median (ms) | Std Dev (ms) | Min (ms) | Max (ms) | Output Size |
|--------------|-----------|-------------|--------------|----------|----------|-------------|
| **jinja2 3.14** | 0.990 | 0.972 | 0.078 | 0.919 | 1.733 | 64,927 chars |
| **jinja2 3.14t** | 1.127 | 1.088 | 0.118 | 1.045 | 2.027 | 65,016 chars |
| **minijinja 3.14** | 1.528 | 1.503 | 0.134 | 1.437 | 2.518 | 64,669 chars |
| **minijinja 3.14t** | 1.336 | 1.286 | 0.139 | 1.219 | 2.158 | 64,921 chars |

### Relative Performance

**jinja2 vs minijinja on Python 3.14:**
- jinja2 is **1.54x faster** than minijinja (0.990 ms vs 1.528 ms)

**jinja2 vs minijinja on Python 3.14t:**
- jinja2 is **1.19x faster** than minijinja (1.127 ms vs 1.336 ms)

**Impact of Free-Threading:**
- jinja2: **13.8% slower** on 3.14t vs 3.14 (1.127 ms vs 0.990 ms)
- minijinja: **12.6% faster** on 3.14t vs 3.14 (1.336 ms vs 1.528 ms)

## Analysis

### Why is jinja2 Faster?

Despite minijinja being written in Rust (typically faster than Python), jinja2 performs better in this benchmark. Possible explanations:

1. **Python/Rust FFI Overhead**: Each template function call crosses the Python/Rust boundary, adding overhead
2. **Data Marshalling**: Converting Python objects to Rust and back adds latency
3. **jinja2 Optimizations**: Years of optimization for common patterns in pure Python
4. **Benchmark Characteristics**: Heavy variable interpolation may favor native Python
5. **Compilation Strategy**: jinja2 compiles to Python bytecode which JIT benefits from

### Free-Threading Performance

The results show dramatically different behavior with free-threaded Python:

**jinja2 Slowdown (13.8%)**:
- Pure Python code may suffer from additional GIL-related overhead
- Memory management overhead in free-threaded build
- Additional synchronization primitives

**minijinja Speedup (12.6%)**:
- Rust code naturally thread-safe without GIL dependencies
- Potential benefits from compiler optimizations for free-threading
- Less reliance on Python's memory management

This suggests **minijinja may be better positioned for future Python versions** as free-threading becomes standard.

### Consistency and Reliability

**Standard Deviations**:
- jinja2 3.14: 0.078 ms (7.9% of mean)
- jinja2 3.14t: 0.118 ms (10.5% of mean)
- minijinja 3.14: 0.134 ms (8.8% of mean)
- minijinja 3.14t: 0.139 ms (10.4% of mean)

All configurations show good consistency with standard deviations below 11% of mean values. minijinja shows slightly higher variance but remains predictable.

## Visualizations

The benchmark includes four detailed charts (see `charts/` directory):

1. **comparison.png**: Bar chart showing mean rendering times with error bars
2. **distribution.png**: Box plots showing the distribution of all rendering times
3. **speedup.png**: Speedup factors of jinja2 vs minijinja by Python version
4. **timeline.png**: Rendering times across all iterations showing performance stability

## Use Case Recommendations

### Choose jinja2 When:
- ✅ Maximum template rendering performance is critical
- ✅ Using standard Python 3.14 (non free-threaded)
- ✅ Mature ecosystem and extensive filter library needed
- ✅ Complex custom filters and Python integration required
- ✅ Compatibility with existing jinja2 templates

### Choose minijinja When:
- ✅ Planning migration to free-threaded Python
- ✅ Need for strict template sandboxing (Rust safety)
- ✅ Cross-language template sharing (minijinja supports multiple languages)
- ✅ Memory safety guarantees are important
- ✅ Willing to trade ~50% performance for better free-threading support

## Template Compatibility Notes

While both engines implement Jinja2 syntax, some differences exist:

### Incompatible Features
- **`truncate` filter**: Different signatures between engines
- **`format` filter**: Not available in minijinja
- **`map` filter**: Not available in minijinja
- **`selectattr` filter**: Not available in minijinja

### Workarounds Used
Instead of `{{ text|truncate(100) }}`:
```jinja2
{{ text[:100] }}{% if text|length > 100 %}...{% endif %}
```

Instead of `{{ "%.2f"|format(price) }}`:
```jinja2
{{ price }}
```

Instead of `{{ items|selectattr('active') }}`:
```jinja2
{% for item in items %}{% if item.active %}...{% endif %}{% endfor %}
```

## Reproducing the Benchmark

### Prerequisites
```bash
# Install uv
curl -LsSf https://astral.sh/uv/install.sh | sh

# Clone this repository
git clone <repo-url>
cd <repo-dir>
```

### Running the Benchmark
```bash
# Run the complete benchmark suite
./run_benchmark.sh
```

This will:
1. Download minijinja source from GitHub
2. Create two Python environments (3.14 and 3.14t)
3. Install jinja2 in both environments
4. Build minijinja from source for both environments
5. Run 200 iterations of each benchmark
6. Generate visualization charts
7. Display summary results

Results are saved to:
- `results/*.json` - Raw benchmark data
- `charts/*.png` - Visualization charts
- `benchmark_run.log` - Complete execution log

### Manual Execution

Run individual benchmarks:
```bash
# Activate the desired environment
source .venv-3.14/bin/activate  # or .venv-3.14t

# Run a specific benchmark
python benchmark.py --engine jinja2 --iterations 200 --num-products 50 --output results.json

# Generate charts
python visualize_results.py --results-dir ./results --output-dir ./charts
```

## Future Considerations

### Python 3.14+ Adoption
As free-threaded Python becomes mainstream:
- minijinja's performance advantage may grow
- The ~50% performance gap may close or reverse
- Thread-safe template rendering without GIL may become valuable

### Benchmark Limitations
This benchmark focuses on:
- Single-threaded performance
- Template rendering only (not parsing/compilation)
- Specific template patterns

Not tested:
- Multi-threaded concurrent rendering
- Template parsing/compilation time
- Memory usage patterns
- Large template files (>100KB)
- Deep template inheritance chains

### Potential Optimizations
- minijinja could benefit from template caching at Python level
- Testing with compiled/cached templates might show different results
- Multi-threaded benchmarks would likely favor minijinja

## Conclusion

For **current Python 3.14 deployments**, **jinja2 remains the performance leader** with significantly faster render times. However, **minijinja shows promise for future free-threaded Python adoption**, being the only tested engine that actually improves performance with free-threading enabled.

The choice between them should consider:
- Current vs future Python version strategy
- Performance requirements (jinja2 is 1.5x faster today)
- Free-threading plans (minijinja gains 13% with 3.14t)
- Template compatibility needs
- Safety and security requirements

## Repository Structure

```
.
├── run_benchmark.sh           # Main benchmark orchestration script
├── benchmark.py               # Core benchmark implementation
├── visualize_results.py       # Chart generation script
├── templates/
│   ├── base.html             # Base template with inheritance
│   └── products.html         # Product listing template
├── results/
│   ├── jinja2-3.14.json      # Raw results for each configuration
│   ├── jinja2-3.14t.json
│   ├── minijinja-3.14.json
│   └── minijinja-3.14t.json
├── charts/
│   ├── comparison.png        # Performance comparison chart
│   ├── distribution.png      # Distribution box plots
│   ├── speedup.png          # Speedup factor visualization
│   └── timeline.png         # Performance over iterations
└── BENCHMARK_RESULTS.md      # This document
```

## License

This benchmark suite is provided as-is for educational and evaluation purposes.

## Acknowledgments

- [minijinja](https://github.com/mitsuhiko/minijinja) by Armin Ronacher
- [jinja2](https://palletsprojects.com/p/jinja/) by Armin Ronacher and contributors
- [uv](https://github.com/astral-sh/uv) by Astral
