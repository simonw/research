# SQLite Hamming Distance Extension: Scalar vs Virtual Table

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

Based on ["Hamming Distance for Hybrid Search in SQLite"](https://notnotp.com/notes/hamming-distance-for-hybrid-search-in-sqlite/) by [notnotp.com](https://notnotp.com/). The original article implements a `hamming_distance()` scalar function as a SQLite extension for binary embedding search and suggests that a virtual table could improve performance by maintaining a top-k heap during scanning instead of sorting all results.

This investigation recreates the scalar function, benchmarks it, then builds
and benchmarks the virtual table concept to see how much faster it can go.

## Background

Binary embeddings represent text as bit vectors (e.g., 1024 bits = 128 bytes).
Similarity is measured by Hamming distance: XOR the vectors, count the 1-bits.
Lower distance = more similar. This is useful for semantic search in SQLite
without needing an external vector database.

## What was built

### 1. Scalar function extension (`hamming.c`)

A standard SQLite loadable extension registering `hamming_distance(blob1, blob2)`.
Processes data in 64-bit chunks using `__builtin_popcountll` for hardware-accelerated
bit counting.

```sql
SELECT rowid, hamming_distance(:query, embedding) as dist
FROM documents ORDER BY dist LIMIT 10;
```

### 2. Virtual table extension (`hamming_vtab.c`)

A virtual table `hamming_topk` that:
- **Lazy-loads** all embeddings from the source table into a contiguous C memory
  buffer on first query
- Scans the cached buffer with a **max-heap** of size k, avoiding a full sort
- Returns top-k results sorted by distance

```sql
CREATE VIRTUAL TABLE search USING hamming_topk(documents, embedding);

SELECT source_rowid, distance FROM search
WHERE query = :vec AND k = 10;
```

## Benchmark results

**Setup:** 1,000,000 rows, 128-byte random vectors, in-memory SQLite database,
5 runs per configuration.

### Scalar function

| Metric | Time |
|---|---|
| Scan + sort (ORDER BY LIMIT 10) | median 80ms, min 75ms |
| Scan only (no sort) | median 61ms, min 55ms |
| Pure Python reference | ~11,478ms |

The C extension is **~200x faster** than pure Python. Sort overhead is ~20-25ms.

### Virtual table (cache-backed)

| k | Scalar (scan+sort) | VTab (cached heap) | Speedup |
|---|---|---|---|
| 1 | 90ms | 16ms | 5.6x |
| 5 | 80ms | 12ms | 6.8x |
| 10 | 80ms | 12ms | 6.9x |
| 50 | 82ms | 14ms | 6.1x |
| 100 | 83ms | 12ms | 6.8x |
| 500 | 85ms | 14ms | 6.1x |
| 1000 | 88ms | 14ms | 6.5x |

One-time cache load: ~246ms. Cache memory overhead: ~136MB (1M x 136 bytes).

### Why the virtual table is faster

The **6-7x speedup** is not primarily from avoiding the sort (which only costs
~20ms). It comes from **memory layout**:

- **Scalar function path**: SQLite traverses its B-tree page structure, extracting
  each BLOB through the sqlite3 value API. Each row involves B-tree navigation,
  page reads, and value deserialization.

- **Virtual table path**: After the one-time cache load, the scan is a pure C
  loop over a contiguous memory buffer. This is extremely cache-friendly — the
  CPU prefetcher can predict the linear access pattern, and the data fits nicely
  in cache lines.

An earlier attempt at a virtual table that ran an internal SQL sub-query
(`SELECT rowid, embedding FROM documents`) for each scan was actually **40% slower**
than the scalar function (118ms vs 80ms), because the per-row overhead of
`sqlite3_step` + `sqlite3_column_blob` exceeded the sort savings.

## Trade-offs

| Aspect | Scalar function | Virtual table (cached) |
|---|---|---|
| Query speed (1M rows) | ~80ms | ~12ms |
| Memory overhead | None | ~136MB for 1M rows |
| Setup cost | None | ~246ms one-time cache load |
| Data freshness | Always current | Stale if source table changes |
| Complexity | Simple | More complex C code |

The virtual table is best suited for **read-heavy workloads** where the embedding
table doesn't change frequently. The cache could be invalidated and rebuilt on
writes, but this implementation doesn't handle that.

## Files

| File | Description |
|---|---|
| `hamming.c` | Scalar `hamming_distance()` extension |
| `hamming_vtab.c` | Virtual table extension with cache + heap |
| `test_hamming.py` | Tests for scalar function (11 tests) |
| `test_vtab.py` | Tests for virtual table (12 tests) |
| `bench_scalar.py` | Standalone scalar function benchmark |
| `bench_compare.py` | Head-to-head comparison benchmark |
| `notes.md` | Investigation notes and progress log |

## Building

```bash
# Scalar function
gcc -g -fPIC -shared hamming.c -o hamming.so -O3 -mpopcnt

# Virtual table
gcc -g -fPIC -shared hamming_vtab.c -o hamming_vtab.so -O3 -mpopcnt
```

## Testing

```bash
uv run --with pytest pytest test_hamming.py test_vtab.py -v
```
