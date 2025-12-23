#!/usr/bin/env python3
"""
Benchmark script for comparing mquickjs sandbox implementations.

Compares:
1. FFI implementation (ctypes)
2. Subprocess implementation
3. C Extension (if available)

Run with: python3 benchmark.py
"""

import time
import statistics
from typing import Dict, Any, Callable, List
import sys

# Test code snippets
BENCHMARKS = {
    "arithmetic": "1 + 2 * 3 - 4 / 2",
    "string_concat": "'hello' + ' ' + 'world'",
    "loop_100": """
        (function() {
            var sum = 0;
            for (var i = 0; i < 100; i++) {
                sum += i;
            }
            return sum;
        })()
    """,
    "loop_1000": """
        (function() {
            var sum = 0;
            for (var i = 0; i < 1000; i++) {
                sum += i;
            }
            return sum;
        })()
    """,
    "recursion": """
        function fib(n) {
            if (n <= 1) return n;
            return fib(n-1) + fib(n-2);
        }
        fib(15)
    """,
    "array_ops": """
        (function() {
            var arr = [];
            for (var i = 0; i < 100; i++) {
                arr.push(i);
            }
            return arr.map(function(x) { return x * 2; }).length;
        })()
    """,
    "json": """
        JSON.stringify({a: 1, b: [1, 2, 3], c: {d: 'test'}})
    """,
}


def run_benchmark(execute_fn: Callable[[str], Any], name: str, iterations: int = 100) -> Dict[str, float]:
    """Run benchmarks for an execute function."""
    results = {}

    for bench_name, code in BENCHMARKS.items():
        times = []

        # Warm up
        try:
            execute_fn(code)
        except Exception as e:
            print(f"  {name}: {bench_name} - Error: {e}")
            continue

        # Measure
        for _ in range(iterations):
            start = time.perf_counter()
            try:
                execute_fn(code)
            except Exception:
                break
            end = time.perf_counter()
            times.append((end - start) * 1000)  # Convert to ms

        if times:
            results[bench_name] = {
                "mean_ms": statistics.mean(times),
                "stddev_ms": statistics.stdev(times) if len(times) > 1 else 0,
                "min_ms": min(times),
                "max_ms": max(times),
            }

    return results


def measure_startup_time(factory_fn: Callable, iterations: int = 10) -> Dict[str, float]:
    """Measure time to create a new sandbox instance."""
    times = []

    for _ in range(iterations):
        start = time.perf_counter()
        sandbox = factory_fn()
        end = time.perf_counter()
        times.append((end - start) * 1000)

        # Clean up
        if hasattr(sandbox, 'close'):
            sandbox.close()

    return {
        "mean_ms": statistics.mean(times),
        "stddev_ms": statistics.stdev(times) if len(times) > 1 else 0,
        "min_ms": min(times),
        "max_ms": max(times),
    }


def print_results(results: Dict[str, Dict], name: str):
    """Print benchmark results."""
    print(f"\n{'='*60}")
    print(f" {name}")
    print('='*60)

    for bench_name, stats in results.items():
        print(f"  {bench_name:20s}: {stats['mean_ms']:8.3f}ms (±{stats['stddev_ms']:.3f}ms)")


def main():
    print("mquickjs Sandbox Benchmarks")
    print("="*60)

    all_results = {}

    # Test FFI implementation
    print("\nTesting FFI implementation...")
    try:
        from mquickjs_ffi import MQuickJSFFI, execute_js as ffi_execute

        # Startup time
        startup = measure_startup_time(MQuickJSFFI)
        print(f"  Startup time: {startup['mean_ms']:.3f}ms")

        # Execution benchmarks
        sandbox = MQuickJSFFI()
        results = run_benchmark(sandbox.execute, "FFI")
        sandbox.close()

        results["_startup"] = startup
        all_results["FFI"] = results
        print_results(results, "FFI Implementation")

    except ImportError as e:
        print(f"  FFI not available: {e}")
    except Exception as e:
        print(f"  FFI error: {e}")

    # Test Subprocess implementation
    print("\nTesting Subprocess implementation...")
    try:
        from mquickjs_subprocess import MQuickJSSubprocess, execute_js as subprocess_execute

        # Startup time (includes building mqjs if needed)
        startup = measure_startup_time(MQuickJSSubprocess)
        print(f"  Startup time: {startup['mean_ms']:.3f}ms")

        # Execution benchmarks (subprocess is slow, use fewer iterations)
        sandbox = MQuickJSSubprocess()
        results = run_benchmark(sandbox.execute, "Subprocess", iterations=20)

        results["_startup"] = startup
        all_results["Subprocess"] = results
        print_results(results, "Subprocess Implementation")

    except ImportError as e:
        print(f"  Subprocess not available: {e}")
    except Exception as e:
        print(f"  Subprocess error: {e}")

    # Test C Extension
    print("\nTesting C Extension...")
    try:
        import mquickjs_ext

        # Startup time
        startup = measure_startup_time(mquickjs_ext.Sandbox)
        print(f"  Startup time: {startup['mean_ms']:.3f}ms")

        # Execution benchmarks
        sandbox = mquickjs_ext.Sandbox()
        results = run_benchmark(sandbox.execute, "C Extension")
        sandbox.close()

        results["_startup"] = startup
        all_results["C Extension"] = results
        print_results(results, "C Extension Implementation")

    except ImportError as e:
        print(f"  C Extension not available: {e}")
    except Exception as e:
        print(f"  C Extension error: {e}")

    # Summary comparison
    if len(all_results) > 1:
        print("\n" + "="*60)
        print(" Summary Comparison (mean execution time)")
        print("="*60)

        # Header
        implementations = list(all_results.keys())
        header = f"{'Benchmark':20s}"
        for impl in implementations:
            header += f" {impl:>12s}"
        print(header)
        print("-"*60)

        # Find all benchmark names
        bench_names = set()
        for impl_results in all_results.values():
            bench_names.update(k for k in impl_results.keys() if not k.startswith('_'))

        for bench_name in sorted(bench_names):
            row = f"{bench_name:20s}"
            best_time = float('inf')

            # Find best time
            for impl in implementations:
                if bench_name in all_results[impl]:
                    time_ms = all_results[impl][bench_name]['mean_ms']
                    best_time = min(best_time, time_ms)

            # Print times with highlighting
            for impl in implementations:
                if bench_name in all_results[impl]:
                    time_ms = all_results[impl][bench_name]['mean_ms']
                    # Mark best with *
                    marker = "*" if time_ms == best_time else " "
                    row += f" {time_ms:>10.3f}ms{marker}"
                else:
                    row += f" {'N/A':>12s}"
            print(row)

        # Startup times
        print("\nStartup times:")
        for impl in implementations:
            if '_startup' in all_results[impl]:
                startup = all_results[impl]['_startup']
                print(f"  {impl}: {startup['mean_ms']:.3f}ms")


if __name__ == "__main__":
    main()
