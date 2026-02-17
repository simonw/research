# SQLite Hamming Distance Extension - Investigation Notes

Based on ["Hamming Distance for Hybrid Search in SQLite"](https://notnotp.com/notes/hamming-distance-for-hybrid-search-in-sqlite/) by [notnotp.com](https://notnotp.com/). The original article describes implementing binary embedding search in SQLite using a scalar `hamming_distance()` function and suggests that a virtual table could improve performance by using a heap instead of a full sort.

## Goal
Recreate the hamming distance SQLite extension from the article, benchmark it,
and build a virtual table variant to see if top-k heap selection can beat
full-sort performance.

## Plan
1. Implement the scalar `hamming_distance()` function as a loadable SQLite extension in C
2. Test it via Python (sqlite3 module) + pytest
3. Benchmark with 1M rows of 128-byte random vectors
4. Build a virtual table extension that maintains a min-heap during scan
5. Test and benchmark the virtual table approach
6. Compare results

## Progress Log

### Scalar function benchmarks (1M rows, 128-byte vectors, in-memory DB)

- **Scan + sort (ORDER BY LIMIT 10):** median 96.0ms, min 77.6ms
- **Scan only (no sort, WHERE dist < 0):** median 55.3ms, min 54.5ms
- **Pure Python reference:** ~11,478ms (single run)
- C extension is ~200x faster than pure Python
- Results match between C extension and Python reference
- Sort overhead is ~40ms on this machine (not Apple M4 like in article)
- Article reports 35ms with sort on M4, 28ms without — our numbers are
  ~2.5x slower which is expected on a different/slower CPU

### Virtual table - first attempt (internal sub-query per scan)

- Implemented vtab that does `SELECT rowid, embedding FROM documents`
  internally, then processes with heap
- Result: **slower** than scalar (118ms vs 80ms)!
- The per-row overhead of sqlite3_step + sqlite3_column_blob through
  the SQLite API (~60ms) exceeds the sort savings (~25ms)
- Lesson: the SQLite VM scan path used by scalar functions is much more
  efficient than going through the query API for a nested scan

### Virtual table - second attempt (preloaded embedding cache)

- Modified vtab to lazy-load all embeddings into contiguous memory on first query
- Cache load: 246ms (one-time cost)
- After caching, the scan is a pure C loop over contiguous memory — no
  SQLite API calls per row
- Results (1M rows, 128-byte vectors):
  - **VTab median: 12ms** (6-7x faster than scalar's 80ms)
  - VTab min: 8ms
  - Scalar+sort median: 80-90ms
  - Scalar scan-only median: 55-61ms
- The contiguous memory layout gives much better cache behavior than
  reading through SQLite's B-tree pages
- Trade-off: uses ~136MB extra RAM (1M * (128 + 8) bytes) for the cache
- The sort savings alone (25ms) wouldn't justify the vtab, but the
  cache effect (6-7x speedup on scan) dominates

### Key learnings

1. SQLite's B-tree page traversal has significant per-row overhead for BLOB access
2. Preloading data into contiguous C arrays bypasses this and gets ~6-7x speedup
3. The theoretical advantage of heap (O(n log k)) vs sort (O(n log n))
   is negligible at N=1M — the real win comes from memory layout
4. The cache approach trades RAM for speed — sensible for read-heavy workloads
