Benchmarking five tagging strategies in SQLite reveals clear trade-offs between query speed, storage, and implementation complexity for workflows involving tags (100,000 rows, 100 tags, average 6.5 tags/row). Indexed approaches—materialized lookup tables on JSON and classic many-to-many tables—easily outperform others, handling single-tag queries in under 1.5 milliseconds, while raw JSON and LIKE-based solutions are much slower. FTS5 (full-text search) offers strong performance and minimal storage, but tag tokenization can cause subtle correctness issues unless carefully managed. The ideal strategy depends on your use case: M2M tables are best for most production apps, FTS5 suits search-oriented interfaces, and lookup tables complement JSON columns for API-centric designs. The benchmark code is available at [benchmark.py](https://github.com/simonw/sqlite-tags-benchmark/blob/main/benchmark.py), and FTS5 docs are [here](https://sqlite.org/fts5.html).

**Key Findings:**
- Indexed lookup (JSON+table) and M2M tables: fastest queries, <1.5ms per tag, but higher setup/storage.
- FTS5: 2–3ms queries, lowest storage, but demands careful tag design to avoid tokenization issues.
- Raw JSON (`json_each`) and LIKE: simple to implement, much slower, suitable only for small or occasional queries.
- All strategies handled correctness, but speed and scalability varied dramatically.
