#!/usr/bin/env python3
"""
Benchmark script for jq WASM sandbox library.

Compares performance of wasmtime vs wasmer backends and generates
charts for the report.
"""

import json
import time
import statistics
import sys
from pathlib import Path
from dataclasses import dataclass
from typing import List, Dict, Any, Optional

# Add the library to path
sys.path.insert(0, str(Path(__file__).parent))

# Path to jaq WASM binary
JAQ_WASM = Path(__file__).parent / "build" / "jaq.wasm"


@dataclass
class BenchmarkResult:
    """Result from a single benchmark."""
    name: str
    runner: str
    iterations: int
    times_ms: List[float]

    @property
    def mean_ms(self) -> float:
        return statistics.mean(self.times_ms)

    @property
    def median_ms(self) -> float:
        return statistics.median(self.times_ms)

    @property
    def stdev_ms(self) -> float:
        if len(self.times_ms) < 2:
            return 0.0
        return statistics.stdev(self.times_ms)

    @property
    def min_ms(self) -> float:
        return min(self.times_ms)

    @property
    def max_ms(self) -> float:
        return max(self.times_ms)


def run_benchmark(
    runner,
    name: str,
    program: str,
    input_json: str,
    iterations: int = 100,
    warmup: int = 10,
) -> BenchmarkResult:
    """Run a single benchmark."""
    # Warmup
    for _ in range(warmup):
        runner.run(program, input_json)

    # Benchmark
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        runner.run(program, input_json)
        elapsed = (time.perf_counter() - start) * 1000  # ms
        times.append(elapsed)

    return BenchmarkResult(
        name=name,
        runner=runner.__class__.__name__,
        iterations=iterations,
        times_ms=times,
    )


def run_all_benchmarks(runners: Dict[str, Any], iterations: int = 100) -> List[BenchmarkResult]:
    """Run all benchmarks for all runners."""
    results = []

    # Define benchmarks
    benchmarks = [
        ("Simple field access", ".foo", '{"foo": "bar"}'),
        ("Nested field access", ".a.b.c.d", '{"a": {"b": {"c": {"d": 42}}}}'),
        ("Array index", ".[50]", json.dumps(list(range(100)))),
        ("Array slice", ".[10:20]", json.dumps(list(range(100)))),
        ("Array map", "[.[] | . * 2]", json.dumps(list(range(50)))),
        ("Array filter", "[.[] | select(. > 25)]", json.dumps(list(range(50)))),
        ("Object construction", "{a: .x, b: .y, c: .z}", '{"x": 1, "y": 2, "z": 3}'),
        ("String operations", '.name | ascii_downcase | split(" ") | .[0]', '{"name": "HELLO WORLD"}'),
        ("Math operations", ". | sqrt | floor", "144"),
        ("Large object", "keys | length", json.dumps({f"key{i}": i for i in range(100)})),
        ("Multiple outputs", ".[] | . * 2", json.dumps(list(range(20)))),
        ("Recursive descent", ".. | numbers", '{"a": {"b": 1, "c": {"d": 2}}, "e": 3}'),
    ]

    for runner_name, runner in runners.items():
        print(f"\nBenchmarking {runner_name}...")
        for bench_name, program, input_json in benchmarks:
            try:
                result = run_benchmark(runner, bench_name, program, input_json, iterations)
                results.append(result)
                print(f"  {bench_name}: {result.mean_ms:.2f}ms (stdev: {result.stdev_ms:.2f}ms)")
            except Exception as e:
                print(f"  {bench_name}: FAILED - {e}")

    return results


def generate_chart_matplotlib(results: List[BenchmarkResult], output_path: str):
    """Generate benchmark charts using matplotlib."""
    try:
        import matplotlib.pyplot as plt
        import numpy as np
    except ImportError:
        print("matplotlib not installed, skipping chart generation")
        return

    # Group results by benchmark name
    benchmarks = {}
    for r in results:
        if r.name not in benchmarks:
            benchmarks[r.name] = {}
        benchmarks[r.name][r.runner] = r

    # Get unique runners
    runners = list(set(r.runner for r in results))
    runners.sort()

    # Create comparison bar chart
    fig, ax = plt.subplots(figsize=(14, 8))

    x = np.arange(len(benchmarks))
    width = 0.35
    multiplier = 0

    colors = ['#2ecc71', '#3498db', '#e74c3c', '#9b59b6']

    for i, runner in enumerate(runners):
        means = []
        errors = []
        for bench_name in benchmarks.keys():
            if runner in benchmarks[bench_name]:
                r = benchmarks[bench_name][runner]
                means.append(r.mean_ms)
                errors.append(r.stdev_ms)
            else:
                means.append(0)
                errors.append(0)

        offset = width * multiplier
        bars = ax.bar(x + offset, means, width, label=runner, color=colors[i % len(colors)],
                      yerr=errors, capsize=3)
        multiplier += 1

    ax.set_ylabel('Time (ms)')
    ax.set_title('jq WASM Sandbox - Runtime Comparison')
    ax.set_xticks(x + width * (len(runners) - 1) / 2)
    ax.set_xticklabels(list(benchmarks.keys()), rotation=45, ha='right')
    ax.legend(loc='upper left')
    ax.set_ylim(bottom=0)

    plt.tight_layout()
    plt.savefig(output_path, dpi=150, bbox_inches='tight')
    print(f"Chart saved to {output_path}")

    # Create a second chart showing relative performance
    fig2, ax2 = plt.subplots(figsize=(14, 6))

    if len(runners) >= 2:
        # Calculate speedup of first runner vs second
        speedups = []
        for bench_name in benchmarks.keys():
            if runners[0] in benchmarks[bench_name] and runners[1] in benchmarks[bench_name]:
                r1 = benchmarks[bench_name][runners[0]].mean_ms
                r2 = benchmarks[bench_name][runners[1]].mean_ms
                if r1 > 0:
                    speedup = r2 / r1
                else:
                    speedup = 1.0
                speedups.append(speedup)
            else:
                speedups.append(1.0)

        colors_speedup = ['#2ecc71' if s > 1 else '#e74c3c' for s in speedups]
        ax2.bar(list(benchmarks.keys()), speedups, color=colors_speedup)
        ax2.axhline(y=1.0, color='black', linestyle='--', linewidth=1)
        ax2.set_ylabel(f'Speedup ({runners[0]} / {runners[1]})')
        ax2.set_title(f'Relative Performance: {runners[0]} vs {runners[1]}')
        ax2.set_xticklabels(list(benchmarks.keys()), rotation=45, ha='right')

        plt.tight_layout()
        speedup_path = output_path.replace('.png', '_speedup.png')
        plt.savefig(speedup_path, dpi=150, bbox_inches='tight')
        print(f"Speedup chart saved to {speedup_path}")


def generate_chart_ascii(results: List[BenchmarkResult]):
    """Generate ASCII bar charts for terminal output."""
    # Group results by benchmark name
    benchmarks = {}
    for r in results:
        if r.name not in benchmarks:
            benchmarks[r.name] = {}
        benchmarks[r.name][r.runner] = r

    runners = list(set(r.runner for r in results))
    runners.sort()

    print("\n" + "=" * 70)
    print("BENCHMARK RESULTS (ASCII CHART)")
    print("=" * 70)

    max_time = max(r.mean_ms for r in results) if results else 1.0
    bar_width = 40

    for bench_name, bench_results in benchmarks.items():
        print(f"\n{bench_name}:")
        for runner in runners:
            if runner in bench_results:
                r = bench_results[runner]
                bar_len = int((r.mean_ms / max_time) * bar_width)
                bar = "█" * bar_len + "░" * (bar_width - bar_len)
                print(f"  {runner:25} |{bar}| {r.mean_ms:.2f}ms")


def save_results_json(results: List[BenchmarkResult], output_path: str):
    """Save benchmark results to JSON."""
    data = []
    for r in results:
        data.append({
            "name": r.name,
            "runner": r.runner,
            "iterations": r.iterations,
            "mean_ms": r.mean_ms,
            "median_ms": r.median_ms,
            "stdev_ms": r.stdev_ms,
            "min_ms": r.min_ms,
            "max_ms": r.max_ms,
        })

    with open(output_path, 'w') as f:
        json.dump(data, f, indent=2)
    print(f"Results saved to {output_path}")


def main():
    """Run benchmarks and generate reports."""
    print("=" * 60)
    print("jq WASM Sandbox Benchmarks")
    print("=" * 60)

    if not JAQ_WASM.exists():
        print(f"\nERROR: jaq.wasm not found at {JAQ_WASM}")
        print("Please build jaq for WASI first:")
        print("  ./build_jaq_wasm.sh")
        sys.exit(1)

    print(f"\nUsing WASM binary: {JAQ_WASM}")
    print(f"File size: {JAQ_WASM.stat().st_size / 1024:.1f} KB")

    # Initialize runners
    runners = {}

    try:
        from jq_wasm import WasmtimeJqRunner
        runners["WasmtimeJqRunner"] = WasmtimeJqRunner(str(JAQ_WASM))
        print("\n[OK] wasmtime initialized")
    except ImportError as e:
        print(f"\n[SKIP] wasmtime not available: {e}")

    try:
        from jq_wasm import WasmerJqRunner
        runners["WasmerJqRunner"] = WasmerJqRunner(str(JAQ_WASM))
        print("[OK] wasmer initialized")
    except ImportError as e:
        print(f"[SKIP] wasmer not available: {e}")

    if not runners:
        print("\nERROR: No WASM runtimes available!")
        sys.exit(1)

    # Run benchmarks
    iterations = 50  # Reduce for faster testing
    results = run_all_benchmarks(runners, iterations)

    # Generate ASCII chart (always works)
    generate_chart_ascii(results)

    # Save JSON results
    output_dir = Path(__file__).parent / "build"
    output_dir.mkdir(exist_ok=True)
    save_results_json(results, str(output_dir / "benchmark_results.json"))

    # Generate matplotlib charts if available
    try:
        generate_chart_matplotlib(results, str(output_dir / "benchmark_chart.png"))
    except Exception as e:
        print(f"Could not generate matplotlib charts: {e}")

    # Print summary table
    print("\n" + "=" * 70)
    print("SUMMARY TABLE")
    print("=" * 70)
    print(f"{'Benchmark':<30} {'Runner':<25} {'Mean (ms)':<12} {'Stdev (ms)':<12}")
    print("-" * 70)
    for r in sorted(results, key=lambda x: (x.name, x.runner)):
        print(f"{r.name:<30} {r.runner:<25} {r.mean_ms:<12.2f} {r.stdev_ms:<12.2f}")

    return 0


if __name__ == "__main__":
    sys.exit(main())
