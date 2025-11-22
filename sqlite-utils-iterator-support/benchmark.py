"""
Performance benchmarks comparing dict-based vs list-based iteration
for sqlite-utils insert_all and upsert_all methods.
"""
import sys
sys.path.insert(0, '/tmp/sqlite-utils')

import time
import tempfile
import os
import json
from sqlite_utils import Database


def benchmark_insert(name, row_count, column_count, use_list_mode, batch_size=100):
    """
    Benchmark insert_all performance

    Args:
        name: Test name for reporting
        row_count: Number of rows to insert
        column_count: Number of columns per row
        use_list_mode: If True, use list mode; if False, use dict mode
        batch_size: Batch size for inserts

    Returns:
        dict with benchmark results
    """
    # Create temporary database
    with tempfile.NamedTemporaryFile(suffix='.db', delete=False) as f:
        db_path = f.name

    try:
        db = Database(db_path)

        # Generate column names
        columns = [f"col_{i}" for i in range(column_count)]

        if use_list_mode:
            def data_generator():
                yield columns
                for i in range(row_count):
                    yield [f"val_{i}_{j}" for j in range(column_count)]
        else:
            def data_generator():
                for i in range(row_count):
                    yield {col: f"val_{i}_{j}" for j, col in enumerate(columns)}

        # Time the insert
        start = time.time()
        db["benchmark"].insert_all(data_generator(), batch_size=batch_size)
        elapsed = time.time() - start

        # Verify row count
        count = db.execute("SELECT COUNT(*) as c FROM benchmark").fetchone()[0]
        assert count == row_count, f"Expected {row_count} rows, got {count}"

        # Get database size
        db_size = os.path.getsize(db_path)

        return {
            "name": name,
            "row_count": row_count,
            "column_count": column_count,
            "mode": "list" if use_list_mode else "dict",
            "batch_size": batch_size,
            "elapsed_seconds": elapsed,
            "rows_per_second": row_count / elapsed if elapsed > 0 else 0,
            "db_size_bytes": db_size,
        }
    finally:
        # Clean up
        if os.path.exists(db_path):
            os.unlink(db_path)


def benchmark_upsert(name, initial_rows, upsert_rows, column_count, use_list_mode):
    """
    Benchmark upsert_all performance

    Args:
        name: Test name
        initial_rows: Number of initial rows
        upsert_rows: Number of rows to upsert (mix of updates and inserts)
        column_count: Number of columns
        use_list_mode: If True, use list mode; if False, use dict mode

    Returns:
        dict with benchmark results
    """
    with tempfile.NamedTemporaryFile(suffix='.db', delete=False) as f:
        db_path = f.name

    try:
        db = Database(db_path)

        # Generate column names
        columns = ["id"] + [f"col_{i}" for i in range(column_count - 1)]

        # Initial insert
        if use_list_mode:
            def initial_data():
                yield columns
                for i in range(initial_rows):
                    yield [i] + [f"initial_{i}_{j}" for j in range(column_count - 1)]
            db["benchmark"].insert_all(initial_data(), pk="id")
        else:
            initial_data = [
                {"id": i, **{col: f"initial_{i}_{j}" for j, col in enumerate(columns[1:])}}
                for i in range(initial_rows)
            ]
            db["benchmark"].insert_all(initial_data, pk="id")

        # Prepare upsert data (50% updates, 50% inserts)
        update_count = upsert_rows // 2
        insert_count = upsert_rows - update_count

        if use_list_mode:
            def upsert_data():
                yield columns
                # Updates (existing IDs)
                for i in range(update_count):
                    yield [i] + [f"updated_{i}_{j}" for j in range(column_count - 1)]
                # Inserts (new IDs)
                for i in range(initial_rows, initial_rows + insert_count):
                    yield [i] + [f"new_{i}_{j}" for j in range(column_count - 1)]
        else:
            def upsert_data():
                # Updates
                for i in range(update_count):
                    yield {"id": i, **{col: f"updated_{i}_{j}" for j, col in enumerate(columns[1:])}}
                # Inserts
                for i in range(initial_rows, initial_rows + insert_count):
                    yield {"id": i, **{col: f"new_{i}_{j}" for j, col in enumerate(columns[1:])}}

        # Time the upsert
        start = time.time()
        db["benchmark"].upsert_all(upsert_data(), pk="id")
        elapsed = time.time() - start

        # Verify row count
        count = db.execute("SELECT COUNT(*) as c FROM benchmark").fetchone()[0]
        expected_count = initial_rows + insert_count
        assert count == expected_count, f"Expected {expected_count} rows, got {count}"

        return {
            "name": name,
            "initial_rows": initial_rows,
            "upsert_rows": upsert_rows,
            "column_count": column_count,
            "mode": "list" if use_list_mode else "dict",
            "elapsed_seconds": elapsed,
            "rows_per_second": upsert_rows / elapsed if elapsed > 0 else 0,
        }
    finally:
        if os.path.exists(db_path):
            os.unlink(db_path)


def run_benchmarks():
    """Run comprehensive benchmark suite"""
    results = []

    print("Running INSERT benchmarks...")
    print("=" * 80)

    # Scenario 1: Small rows, many columns (typical data export)
    print("\nScenario 1: 10K rows, 20 columns")
    for mode in [False, True]:
        mode_name = "list" if mode else "dict"
        print(f"  Testing {mode_name} mode...")
        result = benchmark_insert(
            f"10K_rows_20_cols_{mode_name}",
            row_count=10000,
            column_count=20,
            use_list_mode=mode,
            batch_size=100
        )
        results.append(result)
        print(f"    {result['elapsed_seconds']:.3f}s ({result['rows_per_second']:.0f} rows/sec)")

    # Scenario 2: Many rows, few columns (time series data)
    print("\nScenario 2: 100K rows, 5 columns")
    for mode in [False, True]:
        mode_name = "list" if mode else "dict"
        print(f"  Testing {mode_name} mode...")
        result = benchmark_insert(
            f"100K_rows_5_cols_{mode_name}",
            row_count=100000,
            column_count=5,
            use_list_mode=mode,
            batch_size=500
        )
        results.append(result)
        print(f"    {result['elapsed_seconds']:.3f}s ({result['rows_per_second']:.0f} rows/sec)")

    # Scenario 3: Moderate size (typical use case)
    print("\nScenario 3: 50K rows, 10 columns")
    for mode in [False, True]:
        mode_name = "list" if mode else "dict"
        print(f"  Testing {mode_name} mode...")
        result = benchmark_insert(
            f"50K_rows_10_cols_{mode_name}",
            row_count=50000,
            column_count=10,
            use_list_mode=mode,
            batch_size=200
        )
        results.append(result)
        print(f"    {result['elapsed_seconds']:.3f}s ({result['rows_per_second']:.0f} rows/sec)")

    # Scenario 4: Large batch size
    print("\nScenario 4: 20K rows, 15 columns, large batch")
    for mode in [False, True]:
        mode_name = "list" if mode else "dict"
        print(f"  Testing {mode_name} mode...")
        result = benchmark_insert(
            f"20K_rows_15_cols_large_batch_{mode_name}",
            row_count=20000,
            column_count=15,
            use_list_mode=mode,
            batch_size=1000
        )
        results.append(result)
        print(f"    {result['elapsed_seconds']:.3f}s ({result['rows_per_second']:.0f} rows/sec)")

    print("\n" + "=" * 80)
    print("Running UPSERT benchmarks...")
    print("=" * 80)

    # Upsert scenario 1: Moderate updates
    print("\nUpsert Scenario 1: 5K initial, 5K upsert, 10 columns")
    for mode in [False, True]:
        mode_name = "list" if mode else "dict"
        print(f"  Testing {mode_name} mode...")
        result = benchmark_upsert(
            f"upsert_5K_5K_10_cols_{mode_name}",
            initial_rows=5000,
            upsert_rows=5000,
            column_count=10,
            use_list_mode=mode
        )
        results.append(result)
        print(f"    {result['elapsed_seconds']:.3f}s ({result['rows_per_second']:.0f} rows/sec)")

    # Upsert scenario 2: Large updates
    print("\nUpsert Scenario 2: 20K initial, 10K upsert, 8 columns")
    for mode in [False, True]:
        mode_name = "list" if mode else "dict"
        print(f"  Testing {mode_name} mode...")
        result = benchmark_upsert(
            f"upsert_20K_10K_8_cols_{mode_name}",
            initial_rows=20000,
            upsert_rows=10000,
            column_count=8,
            use_list_mode=mode
        )
        results.append(result)
        print(f"    {result['elapsed_seconds']:.3f}s ({result['rows_per_second']:.0f} rows/sec)")

    return results


def calculate_improvements(results):
    """Calculate performance improvements from dict to list mode"""
    improvements = []

    # Group results by scenario
    scenarios = {}
    for r in results:
        base_name = r['name'].rsplit('_', 1)[0]
        if base_name not in scenarios:
            scenarios[base_name] = {}
        scenarios[base_name][r['mode']] = r

    for scenario_name, modes in scenarios.items():
        if 'dict' in modes and 'list' in modes:
            dict_time = modes['dict']['elapsed_seconds']
            list_time = modes['list']['elapsed_seconds']
            speedup = dict_time / list_time if list_time > 0 else 0
            improvement_pct = ((dict_time - list_time) / dict_time * 100) if dict_time > 0 else 0

            improvements.append({
                'scenario': scenario_name,
                'dict_time': dict_time,
                'list_time': list_time,
                'speedup': speedup,
                'improvement_percent': improvement_pct
            })

    return improvements


if __name__ == "__main__":
    print("SQLite-utils List-based Iterator Performance Benchmark")
    print("=" * 80)

    results = run_benchmarks()

    # Save results
    with open('/home/user/research/sqlite-utils-iterator-support/benchmark_results.json', 'w') as f:
        json.dump(results, f, indent=2)

    print("\n" + "=" * 80)
    print("SUMMARY")
    print("=" * 80)

    improvements = calculate_improvements(results)

    print("\nPerformance Improvements (List mode vs Dict mode):")
    print("-" * 80)
    for imp in improvements:
        print(f"\n{imp['scenario']}:")
        print(f"  Dict mode: {imp['dict_time']:.3f}s")
        print(f"  List mode: {imp['list_time']:.3f}s")
        print(f"  Speedup:   {imp['speedup']:.2f}x")
        print(f"  Improvement: {imp['improvement_percent']:.1f}%")

    # Calculate average improvement
    avg_speedup = sum(i['speedup'] for i in improvements) / len(improvements)
    avg_improvement = sum(i['improvement_percent'] for i in improvements) / len(improvements)

    print("\n" + "=" * 80)
    print(f"Average speedup: {avg_speedup:.2f}x")
    print(f"Average improvement: {avg_improvement:.1f}%")
    print("=" * 80)

    print("\nResults saved to benchmark_results.json")
