#!/usr/bin/env python3
"""
Benchmark script comparing pymemchr with native Python string operations.

This script tests various search operations at different data sizes and generates
PNG charts showing the performance differences.
"""

import time
import random
import string
import os
from typing import Callable, Any
import matplotlib.pyplot as plt
import numpy as np

import pymemchr


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
        "pymemchr_first": [],
        "python_find_first": [],
        "python_index_first": [],
        "pymemchr_last": [],
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

        # pymemchr - first occurrence
        t = benchmark(lambda: pymemchr.memchr(needle, data))
        results["pymemchr_first"].append(t)
        print(f"  pymemchr.memchr (first): {t:.2f} µs")

        # Python find - first occurrence
        t = benchmark(lambda: data.find(bytes([needle])))
        results["python_find_first"].append(t)
        print(f"  bytes.find (first): {t:.2f} µs")

        # Python index - first occurrence
        t = benchmark(lambda: data.index(bytes([needle])))
        results["python_index_first"].append(t)
        print(f"  bytes.index (first): {t:.2f} µs")

        # pymemchr - last occurrence
        t = benchmark(lambda: pymemchr.memrchr(needle, data))
        results["pymemchr_last"].append(t)
        print(f"  pymemchr.memrchr (last): {t:.2f} µs")

        # Python rfind - last occurrence
        t = benchmark(lambda: data.rfind(bytes([needle])))
        results["python_rfind_last"].append(t)
        print(f"  bytes.rfind (last): {t:.2f} µs")

    return results


def run_multi_byte_benchmarks(sizes: list[int]) -> dict:
    """Run benchmarks for multi-byte search (memchr2, memchr3)."""
    results = {
        "sizes": sizes,
        "pymemchr2": [],
        "python_2byte": [],
        "pymemchr3": [],
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

        # pymemchr2
        t = benchmark(lambda: pymemchr.memchr2(n1, n2, data))
        results["pymemchr2"].append(t)
        print(f"  pymemchr.memchr2: {t:.2f} µs")

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

        # pymemchr3
        t = benchmark(lambda: pymemchr.memchr3(n1, n2, n3, data))
        results["pymemchr3"].append(t)
        print(f"  pymemchr.memchr3: {t:.2f} µs")

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
        "pymemchr_short": [],
        "python_short": [],
        "pymemchr_medium": [],
        "python_medium": [],
        "pymemchr_long": [],
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

        # Short needle
        t = benchmark(lambda: pymemchr.memmem_find(short_needle, data))
        results["pymemchr_short"].append(t)
        print(f"  pymemchr short needle ({len(short_needle)} bytes): {t:.2f} µs")

        t = benchmark(lambda: data.find(short_needle))
        results["python_short"].append(t)
        print(f"  Python short needle: {t:.2f} µs")

        # Medium needle
        t = benchmark(lambda: pymemchr.memmem_find(medium_needle, data))
        results["pymemchr_medium"].append(t)
        print(f"  pymemchr medium needle ({len(medium_needle)} bytes): {t:.2f} µs")

        t = benchmark(lambda: data.find(medium_needle))
        results["python_medium"].append(t)
        print(f"  Python medium needle: {t:.2f} µs")

        # Long needle
        t = benchmark(lambda: pymemchr.memmem_find(long_needle, data))
        results["pymemchr_long"].append(t)
        print(f"  pymemchr long needle ({len(long_needle)} bytes): {t:.2f} µs")

        t = benchmark(lambda: data.find(long_needle))
        results["python_long"].append(t)
        print(f"  Python long needle: {t:.2f} µs")

    return results


def run_iter_benchmarks(sizes: list[int]) -> dict:
    """Run benchmarks for finding all occurrences."""
    results = {
        "sizes": sizes,
        "pymemchr_iter": [],
        "python_iter": [],
        "pymemchr_memmem_iter": [],
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

        # pymemchr_iter for byte
        t = benchmark(lambda: pymemchr.memchr_iter(needle_byte, data))
        results["pymemchr_iter"].append(t)
        print(f"  pymemchr.memchr_iter: {t:.2f} µs")

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

        # pymemchr memmem_find_iter
        t = benchmark(lambda: pymemchr.memmem_find_iter(needle_str, data))
        results["pymemchr_memmem_iter"].append(t)
        print(f"  pymemchr.memmem_find_iter: {t:.2f} µs")

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


def run_finder_benchmarks() -> dict:
    """Run benchmarks for precompiled Finder (repeated searches)."""
    results = {
        "iterations": [],
        "pymemchr_finder": [],
        "pymemchr_direct": [],
        "python_find": [],
    }

    print("\n=== Precompiled Finder Benchmarks (1MB haystack) ===")

    size = 1_000_000
    data = generate_text_bytes(size)
    needle = b"pattern_to_find"

    # Insert needle
    data_list = bytearray(data)
    data_list[size // 2 : size // 2 + len(needle)] = needle
    data = bytes(data_list)

    iterations_list = [10, 100, 500, 1000]

    for num_searches in iterations_list:
        print(f"\n  Testing {num_searches} repeated searches:")
        results["iterations"].append(num_searches)

        # Using precompiled Finder
        finder = pymemchr.Finder(needle)
        start = time.perf_counter()
        for _ in range(num_searches):
            finder.find(data)
        t = (time.perf_counter() - start) * 1000  # ms
        results["pymemchr_finder"].append(t)
        print(f"    pymemchr.Finder: {t:.2f} ms")

        # Using direct memmem_find
        start = time.perf_counter()
        for _ in range(num_searches):
            pymemchr.memmem_find(needle, data)
        t = (time.perf_counter() - start) * 1000
        results["pymemchr_direct"].append(t)
        print(f"    pymemchr.memmem_find: {t:.2f} ms")

        # Using Python find
        start = time.perf_counter()
        for _ in range(num_searches):
            data.find(needle)
        t = (time.perf_counter() - start) * 1000
        results["python_find"].append(t)
        print(f"    Python bytes.find: {t:.2f} ms")

    return results


def create_charts(
    single_byte_results: dict,
    multi_byte_results: dict,
    substring_results: dict,
    iter_results: dict,
    finder_results: dict,
    output_dir: str = ".",
):
    """Create PNG charts from benchmark results."""

    os.makedirs(output_dir, exist_ok=True)

    # Chart style
    plt.style.use("seaborn-v0_8-whitegrid")
    colors = {"pymemchr": "#2ecc71", "python": "#3498db"}

    # 1. Single Byte Search Chart
    fig, axes = plt.subplots(1, 2, figsize=(14, 5))

    sizes = single_byte_results["sizes"]
    size_labels = [f"{s // 1000}K" if s >= 1000 else str(s) for s in sizes]

    # First occurrence
    ax = axes[0]
    x = np.arange(len(sizes))
    width = 0.35

    ax.bar(
        x - width / 2,
        single_byte_results["pymemchr_first"],
        width,
        label="pymemchr.memchr",
        color=colors["pymemchr"],
    )
    ax.bar(
        x + width / 2,
        single_byte_results["python_find_first"],
        width,
        label="bytes.find",
        color=colors["python"],
    )

    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Single Byte Search - First Occurrence")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    # Last occurrence
    ax = axes[1]
    ax.bar(
        x - width / 2,
        single_byte_results["pymemchr_last"],
        width,
        label="pymemchr.memrchr",
        color=colors["pymemchr"],
    )
    ax.bar(
        x + width / 2,
        single_byte_results["python_rfind_last"],
        width,
        label="bytes.rfind",
        color=colors["python"],
    )

    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Single Byte Search - Last Occurrence")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_single_byte.png", dpi=150)
    plt.close()
    print(f"\nSaved: {output_dir}/benchmark_single_byte.png")

    # 2. Multi-Byte Search Chart
    fig, ax = plt.subplots(figsize=(10, 6))

    x = np.arange(len(sizes))
    width = 0.2

    ax.bar(
        x - 1.5 * width,
        multi_byte_results["pymemchr2"],
        width,
        label="pymemchr.memchr2",
        color="#2ecc71",
    )
    ax.bar(
        x - 0.5 * width,
        multi_byte_results["python_2byte"],
        width,
        label="Python 2-byte",
        color="#3498db",
    )
    ax.bar(
        x + 0.5 * width,
        multi_byte_results["pymemchr3"],
        width,
        label="pymemchr.memchr3",
        color="#27ae60",
    )
    ax.bar(
        x + 1.5 * width,
        multi_byte_results["python_3byte"],
        width,
        label="Python 3-byte",
        color="#2980b9",
    )

    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Multi-Byte Search (Find First of 2 or 3 Bytes)")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_multi_byte.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_multi_byte.png")

    # 3. Substring Search Chart
    fig, axes = plt.subplots(1, 3, figsize=(15, 5))

    for idx, (needle_type, title) in enumerate(
        [("short", "Short Needle (3 bytes)"), ("medium", "Medium Needle (11 bytes)"), ("long", "Long Needle (25 bytes)")]
    ):
        ax = axes[idx]
        ax.bar(
            x - width / 2,
            substring_results[f"pymemchr_{needle_type}"],
            width,
            label="pymemchr.memmem_find",
            color=colors["pymemchr"],
        )
        ax.bar(
            x + width / 2,
            substring_results[f"python_{needle_type}"],
            width,
            label="bytes.find",
            color=colors["python"],
        )

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

    # 4. Iterator (Find All) Chart
    fig, axes = plt.subplots(1, 2, figsize=(14, 5))

    # Byte iterator
    ax = axes[0]
    ax.bar(
        x - width / 2,
        iter_results["pymemchr_iter"],
        width,
        label="pymemchr.memchr_iter",
        color=colors["pymemchr"],
    )
    ax.bar(
        x + width / 2,
        iter_results["python_iter"],
        width,
        label="Python loop",
        color=colors["python"],
    )

    ax.set_xlabel("Data Size")
    ax.set_ylabel("Time (µs)")
    ax.set_title("Find All Byte Occurrences")
    ax.set_xticks(x)
    ax.set_xticklabels(size_labels)
    ax.legend()

    # Substring iterator
    ax = axes[1]
    ax.bar(
        x - width / 2,
        iter_results["pymemchr_memmem_iter"],
        width,
        label="pymemchr.memmem_find_iter",
        color=colors["pymemchr"],
    )
    ax.bar(
        x + width / 2,
        iter_results["python_memmem_iter"],
        width,
        label="Python loop",
        color=colors["python"],
    )

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

    # 5. Precompiled Finder Chart
    fig, ax = plt.subplots(figsize=(10, 6))

    iterations = finder_results["iterations"]
    x = np.arange(len(iterations))
    width = 0.25

    ax.bar(
        x - width,
        finder_results["pymemchr_finder"],
        width,
        label="pymemchr.Finder",
        color="#2ecc71",
    )
    ax.bar(
        x,
        finder_results["pymemchr_direct"],
        width,
        label="pymemchr.memmem_find",
        color="#27ae60",
    )
    ax.bar(
        x + width,
        finder_results["python_find"],
        width,
        label="bytes.find",
        color="#3498db",
    )

    ax.set_xlabel("Number of Searches")
    ax.set_ylabel("Total Time (ms)")
    ax.set_title("Repeated Searches on 1MB Haystack")
    ax.set_xticks(x)
    ax.set_xticklabels(iterations)
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_finder.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_finder.png")

    # 6. Summary Speedup Chart
    fig, ax = plt.subplots(figsize=(12, 6))

    # Calculate speedups for largest size
    speedups = {
        "memchr": single_byte_results["python_find_first"][-1]
        / single_byte_results["pymemchr_first"][-1],
        "memrchr": single_byte_results["python_rfind_last"][-1]
        / single_byte_results["pymemchr_last"][-1],
        "memchr2": multi_byte_results["python_2byte"][-1]
        / multi_byte_results["pymemchr2"][-1],
        "memchr3": multi_byte_results["python_3byte"][-1]
        / multi_byte_results["pymemchr3"][-1],
        "memmem\n(short)": substring_results["python_short"][-1]
        / substring_results["pymemchr_short"][-1],
        "memmem\n(medium)": substring_results["python_medium"][-1]
        / substring_results["pymemchr_medium"][-1],
        "memmem\n(long)": substring_results["python_long"][-1]
        / substring_results["pymemchr_long"][-1],
        "memchr_iter": iter_results["python_iter"][-1]
        / iter_results["pymemchr_iter"][-1],
        "memmem_iter": iter_results["python_memmem_iter"][-1]
        / iter_results["pymemchr_memmem_iter"][-1],
    }

    operations = list(speedups.keys())
    values = list(speedups.values())

    bars = ax.bar(operations, values, color="#2ecc71")

    # Add value labels on bars
    for bar, val in zip(bars, values):
        ax.text(
            bar.get_x() + bar.get_width() / 2,
            bar.get_height() + 0.1,
            f"{val:.1f}x",
            ha="center",
            va="bottom",
            fontweight="bold",
        )

    ax.axhline(y=1, color="red", linestyle="--", label="Baseline (Python)")
    ax.set_ylabel("Speedup (x times faster)")
    ax.set_title(f"pymemchr Speedup vs Python (Data Size: {sizes[-1] // 1000}KB)")
    ax.legend()

    plt.tight_layout()
    plt.savefig(f"{output_dir}/benchmark_speedup.png", dpi=150)
    plt.close()
    print(f"Saved: {output_dir}/benchmark_speedup.png")

    return speedups


def main():
    print("=" * 60)
    print("pymemchr Benchmark Suite")
    print("=" * 60)

    # Test sizes
    sizes = [1000, 10_000, 100_000, 500_000, 1_000_000]

    # Run all benchmarks
    single_byte_results = run_single_byte_benchmarks(sizes)
    multi_byte_results = run_multi_byte_benchmarks(sizes)
    substring_results = run_substring_benchmarks(sizes)
    iter_results = run_iter_benchmarks(sizes)
    finder_results = run_finder_benchmarks()

    # Create charts
    print("\n" + "=" * 60)
    print("Generating Charts")
    print("=" * 60)

    speedups = create_charts(
        single_byte_results,
        multi_byte_results,
        substring_results,
        iter_results,
        finder_results,
        output_dir=".",
    )

    # Print summary
    print("\n" + "=" * 60)
    print("Summary: Speedup vs Python (on 1MB data)")
    print("=" * 60)
    for op, speedup in speedups.items():
        op_name = op.replace("\n", " ")
        print(f"  {op_name}: {speedup:.2f}x faster")

    print("\n" + "=" * 60)
    print("Benchmark complete!")
    print("=" * 60)

    return speedups


if __name__ == "__main__":
    main()
