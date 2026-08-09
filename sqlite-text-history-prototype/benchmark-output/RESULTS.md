# Generated benchmark results

Workload: 20,000-byte starting document,
300 individually committed edits for the
SQLite benchmark (3 trials; medians reported), and
1,000 edits for final-size scaling.
The edits are mostly small replacements/inserts/deletes, plus one paragraph-scale rewrite
every 50 revisions.

## SQLite update benchmark

All times are milliseconds. “History payload” excludes the ordinary current-text column.
“WAL bytes” is the WAL left by individually committing every edit with automatic
checkpointing disabled, making it a useful approximation of SQLite write amplification.

| Strategy | Total write ms | Median edit | Last-25 median | Growth | History payload | WAL bytes | Compact DB | Random read ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| current_only_reference | 10.8 | 0.037 | 0.038 | 1.2× | 0 B | 13.2 MB | 28.0 KB | current only |
| plain_rows | 15.7 | 0.050 | 0.052 | 1.1× | 5.9 MB | 21.3 MB | 6.0 MB | 0.017 |
| zstd_per_row | 57.9 | 0.180 | 0.183 | 1.0× | 2.1 MB | 17.7 MB | 2.4 MB | 0.073 |
| whole_json_zstd | 1899.6 | 6.850 | 12.864 | 21.3× | 25.6 KB | 22.9 MB | 56.0 KB | 10.892 |
| whole_framed_zstd | 1665.7 | 3.251 | 12.185 | 27.4× | 27.2 KB | 23.3 MB | 56.0 KB | 5.244 |
| whole_json_zlib | 5076.9 | 14.814 | 31.946 | 17.2× | 56.6 KB | 32.9 MB | 88.0 KB | 12.151 |
| chunked_framed_zstd_32 | 147.2 | 0.468 | 0.515 | 1.2× | 82.4 KB | 16.1 MB | 124.0 KB | 0.244 |

## Final compressed size as history grows

| Historical versions | Raw snapshots | JSON + zstd | JSON + zlib | Timestamp JSON |
| --- | --- | --- | --- | --- |
| 1 | 19.5 KB | 6.9 KB | 6.8 KB | 12 B |
| 10 | 195.5 KB | 7.1 KB | 8.7 KB | 111 B |
| 50 | 984.8 KB | 8.3 KB | 15.4 KB | 551 B |
| 100 | 1.9 MB | 10.1 KB | 23.5 KB | 1.1 KB |
| 300 | 5.9 MB | 25.6 KB | 56.6 KB | 3.2 KB |
| 1000 | 20.4 MB | 80.3 KB | 176.4 KB | 10.7 KB |

## Zstandard chunk-size trade-off

These numbers use the longer scaling workload and independently compress each group of
versions. Smaller chunks bound update and random-read work, at the cost of repeating the
base document once per chunk.

| Versions/chunk | Chunks | Total compressed | Largest chunk |
| --- | --- | --- | --- |
| 1 | 1000 | 7.5 MB | 8.5 KB |
| 4 | 250 | 1.9 MB | 8.6 KB |
| 8 | 125 | 989.5 KB | 8.8 KB |
| 16 | 63 | 514.5 KB | 9.0 KB |
| 32 | 32 | 279.6 KB | 9.5 KB |
| 64 | 16 | 159.1 KB | 10.8 KB |
| 128 | 8 | 114.7 KB | 15.7 KB |
| 256 | 4 | 98.6 KB | 25.2 KB |
| 1000 | 1 | 85.5 KB | 85.5 KB |

Full machine-readable measurements are in `results.json`.
