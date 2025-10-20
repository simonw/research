# minijinja vs jinja2 Performance Benchmark

A comprehensive performance benchmark comparing [minijinja](https://github.com/mitsuhiko/minijinja) Python bindings against [jinja2](https://palletsprojects.com/p/jinja/) on Python 3.14 and Python 3.14t (free-threaded).

## Quick Start

```bash
# Prerequisites: uv must be installed
# Install: curl -LsSf https://astral.sh/uv/install.sh | sh

# Run the complete benchmark
./run_benchmark.sh
```

## Results Summary

| Engine | Python 3.14 | Python 3.14t |
|--------|-------------|--------------|
| **jinja2** | 0.990 ms | 1.127 ms |
| **minijinja** | 1.528 ms | 1.336 ms |

### Key Findings

- ⚡ **jinja2 is 1.54x faster** than minijinja on Python 3.14
- 📉 **jinja2 slows down 14%** on free-threaded Python
- 📈 **minijinja speeds up 13%** on free-threaded Python
- 🔮 **minijinja better positioned** for future free-threaded adoption

## What This Benchmark Tests

- **Template Complexity**: E-commerce product listing with inheritance, loops, conditionals
- **Data Volume**: 50 products per render, ~65KB HTML output
- **Iterations**: 200 iterations per configuration
- **Metrics**: Mean, median, std dev, min/max rendering times

## Repository Structure

```
├── run_benchmark.sh          # Main benchmark script
├── benchmark.py              # Benchmark implementation
├── visualize_results.py      # Chart generation
├── templates/                # Jinja2 templates
│   ├── base.html
│   └── products.html
├── results/                  # JSON benchmark results
├── charts/                   # Performance visualizations
├── README.md                 # This file
└── BENCHMARK_RESULTS.md      # Detailed analysis and findings
```

## Detailed Documentation

See [BENCHMARK_RESULTS.md](BENCHMARK_RESULTS.md) for:
- Comprehensive analysis of results
- Performance charts and visualizations
- Template compatibility notes
- Use case recommendations
- Future considerations

## Charts

The benchmark generates four visualization charts:

1. **comparison.png** - Bar chart comparing mean render times
2. **distribution.png** - Box plots showing performance distribution
3. **speedup.png** - Speedup factors by Python version
4. **timeline.png** - Performance stability across iterations

## Manual Execution

```bash
# Run individual benchmarks
source .venv-3.14/bin/activate
python benchmark.py --engine jinja2 --iterations 200 --output results.json

# Generate charts
python visualize_results.py --results-dir ./results --output-dir ./charts
```

## Requirements

- **uv** package manager
- Python 3.14 and 3.14t (auto-installed by uv)
- ~500MB disk space for Python environments
- ~15-20 minutes for complete benchmark run

## License

MIT

## Credits

Created with Claude Code for benchmarking template engine performance.
