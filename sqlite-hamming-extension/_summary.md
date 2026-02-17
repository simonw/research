Exploring efficient Hamming distance search in SQLite for binary embeddings, this project implements both a scalar function extension and a virtual table extension as described in ["Hamming Distance for Hybrid Search in SQLite"](https://notnotp.com/notes/hamming-distance-for-hybrid-search-in-sqlite/). The scalar function scans and sorts rows to locate nearest matches, while the virtual table caches embeddings and leverages a max-heap to deliver top-k results up to seven times faster. Benchmarking with 1M embeddings shows the virtual table greatly outperforms the scalar function due to linear, memory-optimized scanning, though it introduces a modest memory overhead and possible staleness if source data changes. The virtual table is ideal for read-heavy workloads where embeddings change infrequently.

Key findings:
- [Scalar extension](https://notnotp.com/notes/hamming-distance-for-hybrid-search-in-sqlite/): ~80ms per search (1M rows), leveraging SIMD popcount.
- Virtual table: ~12ms per top-k query, after initial cache load (~246ms); memory usage ~136MB for 1M rows.
- Primary speedup is from efficient memory layout and cache utilization, not just avoidance of SQL sorting.
- Cache-backed virtual tables must be rebuilt if the embedding table changes; best suited for static/read-mostly use cases.
