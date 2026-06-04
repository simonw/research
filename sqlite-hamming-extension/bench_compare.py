"""
Benchmark comparison: scalar function vs virtual table for top-k Hamming search.

Tests with 1M rows of 128-byte random vectors at various k values.
The virtual table uses a preloaded embedding cache for maximum scan speed.
"""

import os
import sqlite3
import time
import statistics

SCALAR_EXT = os.path.join(os.path.dirname(__file__), "hamming")
VTAB_EXT = os.path.join(os.path.dirname(__file__), "hamming_vtab")
NUM_ROWS = 1_000_000
VECTOR_SIZE = 128
NUM_RUNS = 5


def populate_db(conn, num_rows=NUM_ROWS):
    """Insert random embeddings into the documents table."""
    conn.execute("""
        CREATE TABLE documents (
            rowid INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        )
    """)
    batch_size = 10_000
    for start in range(0, num_rows, batch_size):
        rows = [(i + 1, os.urandom(VECTOR_SIZE))
                for i in range(start, min(start + batch_size, num_rows))]
        conn.executemany(
            "INSERT INTO documents (rowid, embedding) VALUES (?, ?)", rows
        )
    conn.commit()


def bench_scalar(conn, query_vec, k, runs=NUM_RUNS):
    """Scalar function: ORDER BY + LIMIT k."""
    times = []
    results = None
    for _ in range(runs):
        t0 = time.perf_counter()
        results = conn.execute(
            "SELECT rowid, hamming_distance(?, embedding) as dist "
            "FROM documents ORDER BY dist LIMIT ?",
            (query_vec, k),
        ).fetchall()
        t1 = time.perf_counter()
        times.append(t1 - t0)
    return times, results


def bench_scalar_no_sort(conn, query_vec, runs=NUM_RUNS):
    """Scalar function: scan only, no sort (WHERE dist < 0)."""
    times = []
    for _ in range(runs):
        t0 = time.perf_counter()
        conn.execute(
            "SELECT rowid, hamming_distance(?, embedding) as dist "
            "FROM documents WHERE dist < 0",
            (query_vec,),
        ).fetchall()
        t1 = time.perf_counter()
        times.append(t1 - t0)
    return times


def bench_vtab(conn, query_vec, k, runs=NUM_RUNS):
    """Virtual table: cache-based heap top-k."""
    times = []
    results = None
    for _ in range(runs):
        t0 = time.perf_counter()
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = ?",
            (query_vec, k),
        ).fetchall()
        t1 = time.perf_counter()
        times.append(t1 - t0)
    return times, results


def fmt(times):
    med = statistics.median(times)
    mn = min(times)
    return f"median={med*1000:7.1f}ms  min={mn*1000:7.1f}ms"


def main():
    print(f"=== Benchmark: Scalar vs Virtual Table (cache-backed) ===")
    print(f"Rows: {NUM_ROWS:,}  |  Vec size: {VECTOR_SIZE} bytes  |  Runs per config: {NUM_RUNS}\n")

    # Use a single shared DB so both approaches scan the same data
    print("Populating shared in-memory DB...")
    conn = sqlite3.connect(":memory:")
    conn.enable_load_extension(True)
    conn.load_extension(VTAB_EXT)  # also registers hamming_distance()
    populate_db(conn)

    conn.execute("""
        CREATE VIRTUAL TABLE search USING hamming_topk(documents, embedding)
    """)

    query_vec = os.urandom(VECTOR_SIZE)

    # Warm up: trigger cache load and measure it
    print("Loading vtab cache (first query triggers lazy load)...")
    t0 = time.perf_counter()
    conn.execute(
        "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 1",
        (query_vec,),
    ).fetchall()
    cache_load_time = time.perf_counter() - t0
    print(f"  Cache load time: {cache_load_time*1000:.1f}ms\n")

    # Header
    print(f"{'k':>6s}  {'Scalar (scan+sort)':>22s}  {'VTab (cached heap)':>22s}  {'Speedup':>8s}")
    print("-" * 68)

    results_table = []
    for k in [1, 5, 10, 50, 100, 500, 1000]:
        ts_scalar, res_scalar = bench_scalar(conn, query_vec, k)
        ts_vtab, res_vtab = bench_vtab(conn, query_vec, k)

        med_scalar = statistics.median(ts_scalar)
        med_vtab = statistics.median(ts_vtab)
        speedup = med_scalar / med_vtab if med_vtab > 0 else float("inf")

        print(f"{k:>6d}  {fmt(ts_scalar)}  {fmt(ts_vtab)}  {speedup:>7.2f}x")
        results_table.append((k, med_scalar, med_vtab, speedup))

    # Scan-only baseline
    print()
    ts_nosort = bench_scalar_no_sort(conn, query_vec)
    print(f"Scalar scan-only (no sort, WHERE dist<0): {fmt(ts_nosort)}")

    # Correctness verification
    print("\n=== Correctness check (k=10) ===")
    _, res_s = bench_scalar(conn, query_vec, 10, runs=1)
    _, res_v = bench_vtab(conn, query_vec, 10, runs=1)
    if set(res_s) == set(res_v):
        print("  Scalar and VTab results MATCH.")
    else:
        print("  WARNING: Results differ!")
        print(f"    Scalar: {res_s[:5]}...")
        print(f"    VTab:   {res_v[:5]}...")

    print(f"\n  Scalar top-5: {res_s[:5]}")
    print(f"  VTab   top-5: {res_v[:5]}")

    conn.close()


if __name__ == "__main__":
    main()
