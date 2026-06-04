# SQLite Tags Benchmark - Working Notes

## Goal

Compare different approaches to implementing tags in SQLite:
1. **JSON arrays** - store tags as a JSON array in a column, query with `json_each()`
2. **JSON + lookup table** - JSON array column plus a materialized lookup table keyed on `(tag, item_id)`
3. **M2M tables** - classic relational many-to-many with `items`, `tags`, and `item_tags` tables
4. **FTS5** - Full-Text Search on space-separated tag strings
5. **LIKE queries** - comma-delimited tag string queried with `LIKE '%,tag,%'`

## Setup

- 100,000 rows
- 100 tags in vocabulary
- Each row gets 3-10 random tags (avg ~6.5)
- Each tag appears in ~6,498 rows (~6.5% of total)
- Each query type (single/AND/OR) tested with 10 different tag combinations, 100 iterations each

## Key Observations

### JSON (no index) is very slow
- ~55ms per single-tag lookup — it's doing a full table scan + `json_each()` for every row
- AND queries aren't much different since both conditions require the same scan
- OR queries even worse at ~84ms due to `DISTINCT` + cross-join with `json_each`

### JSON + lookup table is fast
- Essentially the same idea as M2M but derived from the JSON column
- Single tag: ~1.4ms, AND: ~1.9ms — competitive with M2M
- BUT: double the storage (20MB vs 9MB) because you're storing tags twice (JSON + lookup)
- Slower setup (1s vs 0.3s) due to populating the lookup table

### M2M tables perform well
- Single: ~1.4ms, AND: ~2.3ms
- Compact storage (8.7MB)
- Classic approach, well-understood indexing
- Slightly slower AND queries than JSON+lookup (join overhead through `tags` name table)

### FTS5 is surprisingly competitive
- Single: ~3.3ms — about 2x slower than M2M/indexed for single lookups
- AND: ~2.6ms — nearly as fast as indexed approaches!
- OR: ~13.5ms — in the same ballpark as M2M/indexed
- Smallest storage at 7MB
- Very fast setup (267ms)
- Native AND/OR query syntax is clean: `'"tag_001" AND "tag_002"'`
- Caveat: FTS tokenization could cause false matches if tag names contain words that are substrings of other tags. Using quoted phrases helps but isn't bulletproof.

### LIKE is middle ground
- ~19ms per query — full table scan but simpler than JSON parsing
- About 3x faster than raw JSON, 14x slower than indexed approaches
- Simplest setup, smallest code surface area
- Comma-delimited format (`,tag_001,tag_002,`) prevents partial matches

## Correctness Verification

All five strategies returned identical result counts:
- Single tag avg: 6,535 results
- AND (two tags) avg: 411 results
- OR (2-5 tags) avg: 21,335 results

## Summary Rankings

### Query Speed (fastest to slowest for single-tag lookup)
1. JSON + lookup table: 1.37ms
2. M2M tables: 1.41ms
3. FTS5: 3.28ms
4. LIKE: 19.45ms
5. JSON (no index): 54.98ms

### Storage Efficiency (smallest to largest)
1. FTS5: 7,044 KB
2. LIKE: 7,244 KB
3. M2M: 8,772 KB
4. JSON (no index): 9,124 KB
5. JSON + lookup: 20,344 KB

### Setup Speed
1. LIKE: 167ms
2. FTS5: 267ms
3. JSON: 295ms
4. JSON + lookup: 1,002ms
5. M2M: 1,313ms
