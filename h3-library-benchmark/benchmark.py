"""
Comprehensive benchmark comparing h3-py and h3o-python performance.
Tests key H3 operations at different scales.
"""
import time
import random
import h3
import h3o_python as h3o
import json
from typing import List, Tuple, Callable, Any

# Set random seed for reproducibility
random.seed(42)

# Benchmark configuration
SCALES = {
    'small': 100,
    'medium': 1_000,
    'large': 10_000,
    'xlarge': 100_000,
}

RESOLUTIONS = [5, 7, 9, 11]
GRID_DISK_K_VALUES = [1, 2, 5]


def generate_random_coords(n: int) -> List[Tuple[float, float]]:
    """Generate n random lat/lng coordinates."""
    coords = []
    for _ in range(n):
        lat = random.uniform(-85, 85)  # Avoid extreme poles
        lng = random.uniform(-180, 180)
        coords.append((lat, lng))
    return coords


def benchmark_function(func: Callable, *args, iterations: int = 1, **kwargs) -> dict:
    """
    Benchmark a function and return timing results.

    Returns dict with:
        - mean_time: average time per operation (seconds)
        - total_time: total time for all iterations
        - ops_per_sec: operations per second
    """
    start = time.perf_counter()
    for _ in range(iterations):
        func(*args, **kwargs)
    end = time.perf_counter()

    total_time = end - start
    mean_time = total_time / iterations
    ops_per_sec = iterations / total_time if total_time > 0 else float('inf')

    return {
        'mean_time': mean_time,
        'total_time': total_time,
        'ops_per_sec': ops_per_sec,
        'iterations': iterations
    }


def benchmark_latlng_to_cell(coords: List[Tuple[float, float]], resolution: int) -> dict:
    """Benchmark latlng_to_cell for both libraries."""
    results = {}
    n = len(coords)

    # h3-py
    def h3py_bench():
        for lat, lng in coords:
            h3.latlng_to_cell(lat, lng, resolution)

    results['h3-py'] = benchmark_function(h3py_bench)

    # h3o-python
    def h3o_bench():
        for lat, lng in coords:
            h3o.latlng_to_cell(lat, lng, resolution)

    results['h3o-python'] = benchmark_function(h3o_bench)

    return results


def benchmark_cell_to_latlng(cells_h3py: List[str], cells_h3o: List[int]) -> dict:
    """Benchmark cell_to_latlng for both libraries."""
    results = {}

    # h3-py
    def h3py_bench():
        for cell in cells_h3py:
            h3.cell_to_latlng(cell)

    results['h3-py'] = benchmark_function(h3py_bench)

    # h3o-python
    def h3o_bench():
        for cell in cells_h3o:
            h3o.cell_to_latlng(cell)

    results['h3o-python'] = benchmark_function(h3o_bench)

    return results


def benchmark_grid_disk(cells_h3py: List[str], cells_h3o: List[int], k: int) -> dict:
    """Benchmark grid_disk for both libraries."""
    results = {}

    # Use subset for grid_disk (it's expensive)
    subset_size = min(len(cells_h3py), 100)

    # h3-py
    def h3py_bench():
        for cell in cells_h3py[:subset_size]:
            h3.grid_disk(cell, k)

    results['h3-py'] = benchmark_function(h3py_bench)

    # h3o-python
    def h3o_bench():
        for cell in cells_h3o[:subset_size]:
            h3o.grid_disk(cell, k)

    results['h3o-python'] = benchmark_function(h3o_bench)
    results['note'] = f'Tested on {subset_size} cells'

    return results


def benchmark_cell_area(cells_h3py: List[str], cells_h3o: List[int]) -> dict:
    """Benchmark cell area calculation for both libraries."""
    results = {}

    # h3-py
    def h3py_bench():
        for cell in cells_h3py:
            h3.cell_area(cell, unit='km^2')

    results['h3-py'] = benchmark_function(h3py_bench)

    # h3o-python
    def h3o_bench():
        for cell in cells_h3o:
            h3o.cell_area_km2(cell)

    results['h3o-python'] = benchmark_function(h3o_bench)

    return results


def benchmark_string_conversion(cells_h3o: List[int]) -> dict:
    """Benchmark cell to string conversion (h3o only, h3-py uses strings natively)."""
    results = {}

    def h3o_bench():
        for cell in cells_h3o:
            h3o.cell_to_string(cell)

    results['h3o-python'] = benchmark_function(h3o_bench)
    results['note'] = 'h3-py returns strings natively, no conversion needed'

    return results


def run_comprehensive_benchmark():
    """Run all benchmarks and collect results."""
    all_results = {
        'config': {
            'scales': SCALES,
            'resolutions': RESOLUTIONS,
            'grid_disk_k_values': GRID_DISK_K_VALUES,
        },
        'benchmarks': {}
    }

    print("=" * 70)
    print("H3 LIBRARY BENCHMARK")
    print("Comparing h3-py vs h3o-python")
    print("=" * 70)

    for scale_name, n_ops in SCALES.items():
        print(f"\n{'=' * 70}")
        print(f"Scale: {scale_name.upper()} ({n_ops:,} operations)")
        print(f"{'=' * 70}")

        scale_results = {}

        # Generate test data
        print(f"\nGenerating {n_ops:,} random coordinates...")
        coords = generate_random_coords(n_ops)

        for resolution in RESOLUTIONS:
            print(f"\n  Resolution {resolution}:")
            print(f"  {'-' * 66}")

            res_key = f"res_{resolution}"
            scale_results[res_key] = {}

            # 1. latlng_to_cell
            print(f"    Testing latlng_to_cell...")
            results = benchmark_latlng_to_cell(coords, resolution)
            scale_results[res_key]['latlng_to_cell'] = results

            print(f"      h3-py:       {results['h3-py']['ops_per_sec']:>12,.0f} ops/sec")
            print(f"      h3o-python:  {results['h3o-python']['ops_per_sec']:>12,.0f} ops/sec")

            speedup = results['h3o-python']['ops_per_sec'] / results['h3-py']['ops_per_sec']
            print(f"      Speedup:     {speedup:>12.2f}x")

            # Generate cells for other benchmarks
            cells_h3py = [h3.latlng_to_cell(lat, lng, resolution) for lat, lng in coords]
            cells_h3o = [h3o.latlng_to_cell(lat, lng, resolution) for lat, lng in coords]

            # 2. cell_to_latlng
            print(f"    Testing cell_to_latlng...")
            results = benchmark_cell_to_latlng(cells_h3py, cells_h3o)
            scale_results[res_key]['cell_to_latlng'] = results

            print(f"      h3-py:       {results['h3-py']['ops_per_sec']:>12,.0f} ops/sec")
            print(f"      h3o-python:  {results['h3o-python']['ops_per_sec']:>12,.0f} ops/sec")

            speedup = results['h3o-python']['ops_per_sec'] / results['h3-py']['ops_per_sec']
            print(f"      Speedup:     {speedup:>12.2f}x")

            # 3. grid_disk (only for resolution 9 to save time)
            if resolution == 9:
                for k in GRID_DISK_K_VALUES:
                    print(f"    Testing grid_disk (k={k})...")
                    results = benchmark_grid_disk(cells_h3py, cells_h3o, k)
                    scale_results[res_key][f'grid_disk_k{k}'] = results

                    print(f"      h3-py:       {results['h3-py']['ops_per_sec']:>12,.0f} ops/sec")
                    print(f"      h3o-python:  {results['h3o-python']['ops_per_sec']:>12,.0f} ops/sec")

                    speedup = results['h3o-python']['ops_per_sec'] / results['h3-py']['ops_per_sec']
                    print(f"      Speedup:     {speedup:>12.2f}x")
                    print(f"      Note:        {results.get('note', '')}")

            # 4. cell_area
            print(f"    Testing cell_area...")
            results = benchmark_cell_area(cells_h3py, cells_h3o)
            scale_results[res_key]['cell_area'] = results

            print(f"      h3-py:       {results['h3-py']['ops_per_sec']:>12,.0f} ops/sec")
            print(f"      h3o-python:  {results['h3o-python']['ops_per_sec']:>12,.0f} ops/sec")

            speedup = results['h3o-python']['ops_per_sec'] / results['h3-py']['ops_per_sec']
            print(f"      Speedup:     {speedup:>12.2f}x")

            # 5. String conversion (h3o only, once per resolution at medium scale)
            if scale_name == 'medium':
                print(f"    Testing string conversion (h3o only)...")
                results = benchmark_string_conversion(cells_h3o)
                scale_results[res_key]['string_conversion'] = results
                print(f"      h3o-python:  {results['h3o-python']['ops_per_sec']:>12,.0f} ops/sec")
                print(f"      Note:        {results.get('note', '')}")

        all_results['benchmarks'][scale_name] = scale_results

    # Save results to JSON
    output_file = 'benchmark_results.json'
    with open(output_file, 'w') as f:
        json.dump(all_results, f, indent=2)

    print(f"\n{'=' * 70}")
    print(f"Benchmark complete! Results saved to {output_file}")
    print(f"{'=' * 70}")

    return all_results


if __name__ == '__main__':
    results = run_comprehensive_benchmark()
