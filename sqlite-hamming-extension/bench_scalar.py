"""
Benchmark the hamming_distance() scalar function extension.

Populates an in-memory SQLite database with 1M rows of 128-byte random BLOBs,
then measures:
  1. Full scan + sort (ORDER BY dist LIMIT 10)
  2. Full scan without sort (WHERE dist < 0)
  3. Pure Python reference for comparison
"""

import os
import sqlite3
import time
import statistics

EXTENSION_PATH = os.path.join(os.path.dirname(__file__), "hamming")
NUM_ROWS = 1_000_000
VECTOR_SIZE = 128  # bytes (1024 bits)
NUM_RUNS = 5


def setup_db():
    """Create in-memory DB, load extension, populate with random vectors."""
    conn = sqlite3.connect(":memory:")
    conn.enable_load_extension(True)
    conn.load_extension(EXTENSION_PATH)

    conn.execute("""
        CREATE TABLE documents (
            rowid INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        )
    """)

    # Insert in batches for speed
    batch_size = 10_000
    for start in range(0, NUM_ROWS, batch_size):
        rows = [(i + 1, os.urandom(VECTOR_SIZE))
                for i in range(start, min(start + batch_size, NUM_ROWS))]
        conn.executemany(
            "INSERT INTO documents (rowid, embedding) VALUES (?, ?)", rows
        )
    conn.commit()

    # Generate a query vector
    query_vec = os.urandom(VECTOR_SIZE)
    return conn, query_vec


def bench_scan_with_sort(conn, query_vec, runs=NUM_RUNS):
    """SELECT ORDER BY hamming_distance LIMIT 10."""
    times = []
    for _ in range(runs):
        t0 = time.perf_counter()
        results = conn.execute(
            "SELECT rowid, hamming_distance(?, embedding) as dist "
            "FROM documents ORDER BY dist LIMIT 10",
            (query_vec,),
        ).fetchall()
        t1 = time.perf_counter()
        times.append(t1 - t0)
    return times, results


def bench_scan_no_sort(conn, query_vec, runs=NUM_RUNS):
    """Full scan with impossible WHERE clause - no sorting overhead."""
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


def python_hamming(a: bytes, b: bytes) -> int:
    return bin(int.from_bytes(
        bytes(x ^ y for x, y in zip(a, b)), "big"
    )).count("1")


def bench_python_reference(conn, query_vec, runs=1):
    """Pure-Python scan for comparison (single run - it's slow)."""
    cursor = conn.execute("SELECT rowid, embedding FROM documents")
    all_rows = cursor.fetchall()

    times = []
    for _ in range(runs):
        t0 = time.perf_counter()
        scored = [(rowid, python_hamming(query_vec, emb))
                  for rowid, emb in all_rows]
        scored.sort(key=lambda x: x[1])
        top10 = scored[:10]
        t1 = time.perf_counter()
        times.append(t1 - t0)
    return times, top10


def fmt_times(times):
    median = statistics.median(times)
    mn = min(times)
    mx = max(times)
    return f"median={median*1000:.1f}ms  min={mn*1000:.1f}ms  max={mx*1000:.1f}ms  (n={len(times)})"


def main():
    print(f"Setting up in-memory DB with {NUM_ROWS:,} rows of {VECTOR_SIZE}-byte vectors...")
    conn, query_vec = setup_db()
    print("Done.\n")

    # 1. Scan + sort
    print("=== Scalar function: scan + ORDER BY + LIMIT 10 ===")
    times_sort, top10 = bench_scan_with_sort(conn, query_vec)
    print(f"  {fmt_times(times_sort)}")
    print(f"  Top-10 results (rowid, distance):")
    for rowid, dist in top10:
        print(f"    rowid={rowid:>8d}  dist={dist}")

    # 2. Scan only (no sort)
    print("\n=== Scalar function: scan only (no sort, WHERE dist < 0) ===")
    times_nosort = bench_scan_no_sort(conn, query_vec)
    print(f"  {fmt_times(times_nosort)}")

    # 3. Python reference (1 run)
    print("\n=== Pure Python reference (1 run, includes sort) ===")
    times_py, top10_py = bench_python_reference(conn, query_vec, runs=1)
    print(f"  {fmt_times(times_py)}")
    print(f"  Top-10 results (rowid, distance):")
    for rowid, dist in top10_py:
        print(f"    rowid={rowid:>8d}  dist={dist}")

    # Verify C extension and Python agree
    c_set = set((r, d) for r, d in top10)
    py_set = set((r, d) for r, d in top10_py)
    if c_set == py_set:
        print("\n  C extension and Python results MATCH.")
    else:
        print("\n  WARNING: Results differ!")
        print(f"    C:  {top10}")
        print(f"    Py: {top10_py}")

    conn.close()


if __name__ == "__main__":
    main()
