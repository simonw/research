#!/usr/bin/env python3
"""
Benchmark script comparing pymemchr_c (C implementation) with
pymemchr (Rust implementation) and native Python string operations.

This script tests various search operations at different data sizes and generates
PNG charts showing the performance differences between all three implementations.
"""

import time
import random
import string
import sys
import os
from typing import Callable, Any
import matplotlib.pyplot as plt
import numpy as np

# Import C implementation
import pymemchr_c

# Try to import Rust implementation
# We need to add the path to the Rust wrapper
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'memchr-python-wrapper', '.venv', 'lib', 'python3.11', 'site-packages'))
try:
    import pymemchr
    HAS_RUST = True
except ImportError:
    HAS_RUST = False
    print("Warning: pymemchr (Rust) not available, will skip Rust benchmarks")


def generate_random_bytes(size: int, seed: int = 42) -> bytes:
    """Generate random bytes data of specified size."""
    random.seed(seed)
    return bytes(random.randint(0, 255) for _ in range(size))


def generate_text_bytes(size: int, seed: int = 42) -> bytes:
    """Generate random ASCII text of specified size."""
    random.seed(seed)
    chars = string.ascii_letters + string.digits + " " * 10  # Extra spaces for realism
    return "".join(random.choice(chars) for _ in range(size)).encode("ascii")


def benchmark(func: Callable[[], Any], iterations: int = 100) -> float:
    """Benchmark a function and return average time in microseconds."""
    # Warmup
    for _ in range(min(10, iterations)):
        func()

    # Actual benchmark
    times = []
    for _ in range(iterations):
        start = time.perf_counter()
        func()
        end = time.perf_counter()
        times.append((end - start) * 1_000_000)  # Convert to microseconds

    return np.median(times)


def run_single_byte_benchmarks(sizes: list[int]) -> dict:
    """Run benchmarks for single byte search."""
    results = {
        "sizes": sizes,
        "c_first": [],
        "rust_first": [],
        "python_find_first": [],
        "c_last": [],
        "rust_last": [],
        "python_rfind_last": [],
    }

    print("\n=== Single Byte Search Benchmarks ===")

    for size in sizes:
        print(f"\nTesting size: {size:,} bytes")
        data = generate_text_bytes(size)
        needle = ord("z")  # Common character

        # Make sure the needle exists in data
        data_list = bytearray(data)
        data_list[size // 2] = needle  # Put one in the middle
        data_list[size // 4] = needle  # And one earlier
        data_list[3 * size // 4] = needle  # And one later
        data = bytes(data_list)

        # C implementation - first occurrence
        t = benchmark(lambda: pymemchr_c.memchr(needle, data))
        results["c_first"].append(t)
        print(f"  pymemchr_c.memchr (first): {t:.2f} µs")

        # Rust implementation - first occurrence
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memchr(needle, data))
            results["rust_first"].append(t)
            print(f"  pymemchr.memchr (first): {t:.2f} µs")
        else:
            results["rust_first"].append(0)

        # Python find - first occurrence
        t = benchmark(lambda: data.find(bytes([needle])))
        results["python_find_first"].append(t)
        print(f"  bytes.find (first): {t:.2f} µs")

        # C implementation - last occurrence
        t = benchmark(lambda: pymemchr_c.memrchr(needle, data))
        results["c_last"].append(t)
        print(f"  pymemchr_c.memrchr (last): {t:.2f} µs")

        # Rust implementation - last occurrence
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memrchr(needle, data))
            results["rust_last"].append(t)
            print(f"  pymemchr.memrchr (last): {t:.2f} µs")
        else:
            results["rust_last"].append(0)

        # Python rfind - last occurrence
        t = benchmark(lambda: data.rfind(bytes([needle])))
        results["python_rfind_last"].append(t)
        print(f"  bytes.rfind (last): {t:.2f} µs")

    return results


def run_multi_byte_benchmarks(sizes: list[int]) -> dict:
    """Run benchmarks for multi-byte search (memchr2, memchr3)."""
    results = {
        "sizes": sizes,
        "c_memchr2": [],
        "rust_memchr2": [],
        "python_2byte": [],
        "c_memchr3": [],
        "rust_memchr3": [],
        "python_3byte": [],
    }

    print("\n=== Multi-Byte Search Benchmarks ===")

    for size in sizes:
        print(f"\nTesting size: {size:,} bytes")
        data = generate_text_bytes(size)

        # Needles
        n1, n2, n3 = ord("x"), ord("y"), ord("z")

        # Ensure needles exist
        data_list = bytearray(data)
        data_list[size // 3] = n1
        data_list[size // 2] = n2
        data_list[2 * size // 3] = n3
        data = bytes(data_list)

        # C memchr2
        t = benchmark(lambda: pymemchr_c.memchr2(n1, n2, data))
        results["c_memchr2"].append(t)
        print(f"  pymemchr_c.memchr2: {t:.2f} µs")

        # Rust memchr2
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memchr2(n1, n2, data))
            results["rust_memchr2"].append(t)
            print(f"  pymemchr.memchr2: {t:.2f} µs")
        else:
            results["rust_memchr2"].append(0)

        # Python equivalent for 2-byte search
        def python_2byte():
            idx1 = data.find(bytes([n1]))
            idx2 = data.find(bytes([n2]))
            if idx1 == -1:
                return idx2 if idx2 != -1 else None
            if idx2 == -1:
                return idx1
            return min(idx1, idx2)

        t = benchmark(python_2byte)
        results["python_2byte"].append(t)
        print(f"  Python 2-byte search: {t:.2f} µs")

        # C memchr3
        t = benchmark(lambda: pymemchr_c.memchr3(n1, n2, n3, data))
        results["c_memchr3"].append(t)
        print(f"  pymemchr_c.memchr3: {t:.2f} µs")

        # Rust memchr3
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memchr3(n1, n2, n3, data))
            results["rust_memchr3"].append(t)
            print(f"  pymemchr.memchr3: {t:.2f} µs")
        else:
            results["rust_memchr3"].append(0)

        # Python equivalent for 3-byte search
        def python_3byte():
            idx1 = data.find(bytes([n1]))
            idx2 = data.find(bytes([n2]))
            idx3 = data.find(bytes([n3]))
            indices = [i for i in [idx1, idx2, idx3] if i != -1]
            return min(indices) if indices else None

        t = benchmark(python_3byte)
        results["python_3byte"].append(t)
        print(f"  Python 3-byte search: {t:.2f} µs")

    return results


def run_substring_benchmarks(sizes: list[int]) -> dict:
    """Run benchmarks for substring search."""
    results = {
        "sizes": sizes,
        "c_short": [],
        "rust_short": [],
        "python_short": [],
        "c_medium": [],
        "rust_medium": [],
        "python_medium": [],
        "c_long": [],
        "rust_long": [],
        "python_long": [],
    }

    print("\n=== Substring Search Benchmarks ===")

    short_needle = b"xyz"
    medium_needle = b"hello world"
    long_needle = b"the quick brown fox jumps"

    for size in sizes:
        print(f"\nTesting size: {size:,} bytes")
        data = generate_text_bytes(size)

        # Insert needles at various positions
        data_list = bytearray(data)
        pos = size // 2

        # Insert short needle
        if pos + len(short_needle) < size:
            data_list[pos : pos + len(short_needle)] = short_needle

        # Insert medium needle
        pos2 = size // 3
        if pos2 + len(medium_needle) < size:
            data_list[pos2 : pos2 + len(medium_needle)] = medium_needle

        # Insert long needle
        pos3 = 2 * size // 3
        if pos3 + len(long_needle) < size:
            data_list[pos3 : pos3 + len(long_needle)] = long_needle

        data = bytes(data_list)

        # Short needle - C
        t = benchmark(lambda: pymemchr_c.memmem_find(short_needle, data))
        results["c_short"].append(t)
        print(f"  pymemchr_c short needle ({len(short_needle)} bytes): {t:.2f} µs")

        # Short needle - Rust
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memmem_find(short_needle, data))
            results["rust_short"].append(t)
            print(f"  pymemchr short needle: {t:.2f} µs")
        else:
            results["rust_short"].append(0)

        t = benchmark(lambda: data.find(short_needle))
        results["python_short"].append(t)
        print(f"  Python short needle: {t:.2f} µs")

        # Medium needle - C
        t = benchmark(lambda: pymemchr_c.memmem_find(medium_needle, data))
        results["c_medium"].append(t)
        print(f"  pymemchr_c medium needle ({len(medium_needle)} bytes): {t:.2f} µs")

        # Medium needle - Rust
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memmem_find(medium_needle, data))
            results["rust_medium"].append(t)
            print(f"  pymemchr medium needle: {t:.2f} µs")
        else:
            results["rust_medium"].append(0)

        t = benchmark(lambda: data.find(medium_needle))
        results["python_medium"].append(t)
        print(f"  Python medium needle: {t:.2f} µs")

        # Long needle - C
        t = benchmark(lambda: pymemchr_c.memmem_find(long_needle, data))
        results["c_long"].append(t)
        print(f"  pymemchr_c long needle ({len(long_needle)} bytes): {t:.2f} µs")

        # Long needle - Rust
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memmem_find(long_needle, data))
            results["rust_long"].append(t)
            print(f"  pymemchr long needle: {t:.2f} µs")
        else:
            results["rust_long"].append(0)

        t = benchmark(lambda: data.find(long_needle))
        results["python_long"].append(t)
        print(f"  Python long needle: {t:.2f} µs")

    return results


def run_iter_benchmarks(sizes: list[int]) -> dict:
    """Run benchmarks for finding all occurrences."""
    results = {
        "sizes": sizes,
        "c_iter": [],
        "rust_iter": [],
        "python_iter": [],
        "c_memmem_iter": [],
        "rust_memmem_iter": [],
        "python_memmem_iter": [],
    }

    print("\n=== Iterator (Find All) Benchmarks ===")

    for size in sizes:
        print(f"\nTesting size: {size:,} bytes")

        # Create data with many occurrences of target
        needle_byte = ord("a")
        needle_str = b"the"

        # Generate data with frequent 'a' characters
        data = generate_text_bytes(size)

        # Count occurrences
        count_byte = data.count(bytes([needle_byte]))
        count_str = data.count(needle_str)
        print(f"  Occurrences of 'a': {count_byte}, of 'the': {count_str}")

        # C memchr_iter for byte
        t = benchmark(lambda: pymemchr_c.memchr_iter(needle_byte, data))
        results["c_iter"].append(t)
        print(f"  pymemchr_c.memchr_iter: {t:.2f} µs")

        # Rust memchr_iter for byte
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memchr_iter(needle_byte, data))
            results["rust_iter"].append(t)
            print(f"  pymemchr.memchr_iter: {t:.2f} µs")
        else:
            results["rust_iter"].append(0)

        # Python equivalent
        def python_find_all_byte():
            indices = []
            start = 0
            needle = bytes([needle_byte])
            while True:
                idx = data.find(needle, start)
                if idx == -1:
                    break
                indices.append(idx)
                start = idx + 1
            return indices

        t = benchmark(python_find_all_byte)
        results["python_iter"].append(t)
        print(f"  Python find all byte: {t:.2f} µs")

        # C memmem_find_iter
        t = benchmark(lambda: pymemchr_c.memmem_find_iter(needle_str, data))
        results["c_memmem_iter"].append(t)
        print(f"  pymemchr_c.memmem_find_iter: {t:.2f} µs")

        # Rust memmem_find_iter
        if HAS_RUST:
            t = benchmark(lambda: pymemchr.memmem_find_iter(needle_str, data))
            results["rust_memmem_iter"].append(t)
            print(f"  pymemchr.memmem_find_iter: {t:.2f} µs")
        else:
            results["rust_memmem_iter"].append(0)

        # Python equivalent for substring
        def python_find_all_str():
            indices = []
            start = 0
            while True:
                idx = data.find(needle_str, start)
                if idx == -1:
                    break
                indices.append(idx)
                start = idx + 1
            return indices

        t = benchmark(python_find_all_str)
        results["python_memmem_iter"].append(t)
        print(f"  Python find all substring: {t:.2f} µs")

    return results


def create_charts(
    single_byte_results: dict,
    multi_byte_results: dict,
    substring_results: dict,
    iter_results: dict,
    output_dir: str = ".",
):
    """Create PNG charts from benchmark results."""

    os.makedirs(output_dir, exist_ok=True)

    # Chart style
    plt.style.use("seaborn-v0_8-whitegrid")
    colors = {"c": "#e74c3c", "rust": "#2ecc71", "python": "#3498db"}

    sizes = single_byte_results["sizes"]
    size_labels = [f"{s // 1000}K" if s >= 1000 else str(s) for s in sizes]

    # 1. Single Byte Search Chart - First Occurrence
    fig, ax = plt.subplots(figsize=(12, 6))
    x = np.arange(len(sizes))
    width = 0.25

    bars1 = ax.bar(x - width, single_byte_results["c_first"], width, label="C (pymemchr_c)", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x, single_byte_results["rust_first"], width, label="Rust (pymemchr)", color=colors["rust"])
    bars3 = ax.bar(x + width, single_byte_results["python_find_first"], width, label="Python bytes.find", color=colors["python"])

    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Single Byte Search - First Occurrence (memchr)")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_single_byte_first.png", dpi=150)
    plt.close()
    print(f"\nSaved: {output_dir}/benchmark_single_byte_first.png")

    # 2. Single Byte Search Chart - Last Occurrence
    fig, ax = plt.subplots(figsize=(12, 6))

    bars1 = ax.bar(x - width, single_byte_results["c_last"], width, label="C (pymemchr_c)", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x, single_byte_results["rust_last"], width, label="Rust (pymemchr)", color=colors["rust"])
    bars3 = ax.bar(x + width, single_byte_results["python_rfind_last"], width, label="Python bytes.rfind", color=colors["python"])

    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Single Byte Search - Last Occurrence (memrchr)")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_single_byte_last.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_single_byte_last.png")

    # 3. Multi-Byte Search Chart
    fig, axes = plt.subplots(1, 2, figsize=(14, 5))

    # memchr2
    ax = axes[0]
    bars1 = ax.bar(x - width, multi_byte_results["c_memchr2"], width, label="C", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x, multi_byte_results["rust_memchr2"], width, label="Rust", color=colors["rust"])
    bars3 = ax.bar(x + width, multi_byte_results["python_2byte"], width, label="Python", color=colors["python"])
    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("memchr2 - First of Two Bytes")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    # memchr3
    ax = axes[1]
    bars1 = ax.bar(x - width, multi_byte_results["c_memchr3"], width, label="C", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x, multi_byte_results["rust_memchr3"], width, label="Rust", color=colors["rust"])
    bars3 = ax.bar(x + width, multi_byte_results["python_3byte"], width, label="Python", color=colors["python"])
    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("memchr3 - First of Three Bytes")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_multi_byte.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_multi_byte.png")

    # 4. Substring Search Chart
    fig, axes = plt.subplots(1, 3, figsize=(15, 5))

    for idx, (needle_type, title) in enumerate(
        [("short", "Short Needle (3 bytes)"), ("medium", "Medium Needle (11 bytes)"), ("long", "Long Needle (25 bytes)")]
    ):
        ax = axes[idx]
        bars1 = ax.bar(x - width, substring_results[f"c_{needle_type}"], width, label="C", color=colors["c"])
        if HAS_RUST:
            bars2 = ax.bar(x, substring_results[f"rust_{needle_type}"], width, label="Rust", color=colors["rust"])
        bars3 = ax.bar(x + width, substring_results[f"python_{needle_type}"], width, label="Python", color=colors["python"])
        ax.set_xlabel("Data Size")
        ax.set_ylabel("Time (µs)")
        ax.set_title(title)
        ax.set_xticks(x)
        ax.set_xticklabels(size_labels)
        ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_substring.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_substring.png")

    # 5. Iterator Benchmark Chart
    fig, axes = plt.subplots(1, 2, figsize=(14, 5))

    # Byte iterator
    ax = axes[0]
    bars1 = ax.bar(x - width, iter_results["c_iter"], width, label="C", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x, iter_results["rust_iter"], width, label="Rust", color=colors["rust"])
    bars3 = ax.bar(x + width, iter_results["python_iter"], width, label="Python", color=colors["python"])
    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Find All Byte Occurrences")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    # Substring iterator
    ax = axes[1]
    bars1 = ax.bar(x - width, iter_results["c_memmem_iter"], width, label="C", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x, iter_results["rust_memmem_iter"], width, label="Rust", color=colors["rust"])
    bars3 = ax.bar(x + width, iter_results["python_memmem_iter"], width, label="Python", color=colors["python"])
    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Find All Substring Occurrences")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_iter.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_iter.png")

    # 6. Summary Speedup Chart - C vs Python on largest size
    fig, ax = plt.subplots(figsize=(14, 6))

    operations = []
    c_speedups = []
    rust_speedups = []

    # Calculate speedups for largest size
    tests = [
        ("memchr", single_byte_results["python_find_first"][-1], single_byte_results["c_first"][-1], single_byte_results["rust_first"][-1] if HAS_RUST else 0),
        ("memrchr", single_byte_results["python_rfind_last"][-1], single_byte_results["c_last"][-1], single_byte_results["rust_last"][-1] if HAS_RUST else 0),
        ("memchr2", multi_byte_results["python_2byte"][-1], multi_byte_results["c_memchr2"][-1], multi_byte_results["rust_memchr2"][-1] if HAS_RUST else 0),
        ("memchr3", multi_byte_results["python_3byte"][-1], multi_byte_results["c_memchr3"][-1], multi_byte_results["rust_memchr3"][-1] if HAS_RUST else 0),
        ("memmem\n(short)", substring_results["python_short"][-1], substring_results["c_short"][-1], substring_results["rust_short"][-1] if HAS_RUST else 0),
        ("memmem\n(medium)", substring_results["python_medium"][-1], substring_results["c_medium"][-1], substring_results["rust_medium"][-1] if HAS_RUST else 0),
        ("memmem\n(long)", substring_results["python_long"][-1], substring_results["c_long"][-1], substring_results["rust_long"][-1] if HAS_RUST else 0),
        ("memchr_iter", iter_results["python_iter"][-1], iter_results["c_iter"][-1], iter_results["rust_iter"][-1] if HAS_RUST else 0),
        ("memmem_iter", iter_results["python_memmem_iter"][-1], iter_results["c_memmem_iter"][-1], iter_results["rust_memmem_iter"][-1] if HAS_RUST else 0),
    ]

    for name, python_time, c_time, rust_time in tests:
        operations.append(name)
        c_speedup = python_time / c_time if c_time > 0 else 0
        c_speedups.append(c_speedup)
        rust_speedup = python_time / rust_time if rust_time > 0 else 0
        rust_speedups.append(rust_speedup)

    x = np.arange(len(operations))
    width = 0.35

    bars1 = ax.bar(x - width/2, c_speedups, width, label="C vs Python", color=colors["c"])
    if HAS_RUST:
        bars2 = ax.bar(x + width/2, rust_speedups, width, label="Rust vs Python", color=colors["rust"])

    # Add value labels on bars
    for bar, val in zip(bars1, c_speedups):
        if val > 0:
            ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height() + 0.1,
                    f"{val:.1f}x", ha="center", va="bottom", fontweight="bold", fontsize=8)
    if HAS_RUST:
        for bar, val in zip(bars2, rust_speedups):
            if val > 0:
                ax.text(bar.get_x() + bar.get_width() / 2, bar.get_height() + 0.1,
                        f"{val:.1f}x", ha="center", va="bottom", fontweight="bold", fontsize=8)

    ax.axhline(y=1, color="black", linestyle="--", label="Baseline (Python)", alpha=0.5)
    ax.set_ylabel("Speedup (x times faster)")
    ax.set_title(f"Speedup vs Python (Data Size: {sizes[-1] // 1000}KB)")
    ax.set_xticks(x)
    ax.set_xticklabels(operations)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_speedup.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_speedup.png")

    return c_speedups, rust_speedups, operations


def main():
    print("=" * 60)
    print("pymemchr_c Benchmark Suite")
    print("Comparing C, Rust, and Python implementations")
    print("=" * 60)

    if HAS_RUST:
        print("\nRust implementation available - will include in benchmarks")
    else:
        print("\nRust implementation NOT available - skipping Rust benchmarks")

    # Test sizes
    sizes = [1000, 10_000, 100_000, 500_000, 1_000_000]

    # Run all benchmarks
    single_byte_results = run_single_byte_benchmarks(sizes)
    multi_byte_results = run_multi_byte_benchmarks(sizes)
    substring_results = run_substring_benchmarks(sizes)
    iter_results = run_iter_benchmarks(sizes)

    # Create charts
    print("\n" + "=" * 60)
    print("Generating Charts")
    print("=" * 60)

    c_speedups, rust_speedups, operations = create_charts(
        single_byte_results,
        multi_byte_results,
        substring_results,
        iter_results,
        output_dir=".",
    )

    # Print summary
    print("\n" + "=" * 60)
    print("Summary: Speedup vs Python (on 1MB data)")
    print("=" * 60)
    print(f"\n{'Operation':<15} {'C vs Python':>15} {'Rust vs Python':>15}")
    print("-" * 45)
    for i, op in enumerate(operations):
        op_name = op.replace("\n", " ")
        c_str = f"{c_speedups[i]:.2f}x" if c_speedups[i] > 0 else "N/A"
        rust_str = f"{rust_speedups[i]:.2f}x" if rust_speedups[i] > 0 else "N/A"
        print(f"{op_name:<15} {c_str:>15} {rust_str:>15}")

    print("\n" + "=" * 60)
    print("Benchmark complete!")
    print("=" * 60)

    return c_speedups, rust_speedups


if __name__ == "__main__":
    main()
