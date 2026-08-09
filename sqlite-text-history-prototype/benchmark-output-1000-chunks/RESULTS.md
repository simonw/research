# Generated benchmark results

Workload: 20,000-byte starting document,
1,000 individually committed edits for the
SQLite benchmark (1 trials; medians reported), and
1,000 edits for final-size scaling.
The edits are mostly small replacements/inserts/deletes, plus one paragraph-scale rewrite
every 50 revisions.

## SQLite update benchmark

All times are milliseconds. “History payload” excludes the ordinary current-text column.
“WAL bytes” is the WAL left by individually committing every edit with automatic
checkpointing disabled, making it a useful approximation of SQLite write amplification.

| Strategy | Total write ms | Median edit | Last-25 median | Growth | History payload | WAL bytes | Compact DB | Random read ms |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| current_only_reference | 42.2 | 0.039 | 0.040 | 1.2× | 0 B | 45.3 MB | 28.0 KB | current only |
| plain_rows | 70.3 | 0.061 | 0.061 | 1.0× | 20.4 MB | 74.0 MB | 20.8 MB | 0.020 |
| zstd_per_row | 218.2 | 0.189 | 0.241 | 1.3× | 7.5 MB | 61.2 MB | 8.0 MB | 0.080 |
| chunked_framed_zstd_32 | 518.6 | 0.505 | 0.559 | 1.1× | 279.6 KB | 56.6 MB | 336.0 KB | 0.256 |
| chunked_framed_zstd_64 | 758.8 | 0.711 | 0.652 | 1.3× | 159.1 KB | 57.3 MB | 224.0 KB | 0.647 |
| chunked_framed_zstd_128 | 1559.7 | 1.215 | 1.825 | 4.0× | 114.7 KB | 58.7 MB | 172.0 KB | 1.207 |
| chunked_framed_zstd_256 | 4690.5 | 2.234 | 9.944 | 21.2× | 98.6 KB | 64.4 MB | 148.0 KB | 3.885 |

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
