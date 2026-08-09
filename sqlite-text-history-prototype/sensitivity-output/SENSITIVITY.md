# Compression sensitivity results

Each scenario stores 1,000 historical snapshots of an approximately
20 KB document. The high-entropy scenarios deliberately
remove the ordinary compressibility of English text, isolating the value of similarity
between adjacent revisions.

| Scenario | Raw snapshots | Zstd each row | One JSON blob | JSON chunks of 64 | JSON chunks of 128 | Whole/raw |
| --- | --- | --- | --- | --- | --- | --- |
| pseudo-English, small mixed edits | 20.4 MB | 7.5 MB | 80.3 KB | 154.9 KB | 109.9 KB | 0.38% |
| high-entropy, 20-byte replacements | 19.1 MB | 15.8 MB | 45.8 KB | 290.6 KB | 160.4 KB | 0.23% |
| high-entropy, 1% replaced/edit | 19.1 MB | 15.8 MB | 199.8 KB | 441.5 KB | 312.9 KB | 1.02% |
| high-entropy, 10% replaced/edit | 19.1 MB | 15.8 MB | 1.6 MB | 1.8 MB | 1.7 MB | 8.53% |
| independent high-entropy snapshots | 19.1 MB | 15.8 MB | 16.0 MB | 16.0 MB | 16.0 MB | 83.83% |
