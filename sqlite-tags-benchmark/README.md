# SQLite Tags Benchmark: Comparing 5 Tagging Strategies

Benchmarking different approaches to implementing tags in SQLite, using Python's `sqlite3` module.

## Test Setup

| Parameter | Value |
|-----------|-------|
| Rows | 100,000 |
| Tag vocabulary | 100 unique tags |
| Tags per row | 3–10 (avg 6.5) |
| Avg rows per tag | ~6,498 (6.5%) |
| Query iterations | 100 per query, 10 different tag combinations |

## Strategies Tested

### 1. JSON Arrays (no index)
Store tags as a JSON array column. Query using `json_each()`:
```sql
SELECT id FROM items
WHERE EXISTS (
    SELECT 1 FROM json_each(items.tags) WHERE value = ?
)
```

### 2. JSON + Lookup Table
Same JSON column, but also maintain a materialized lookup table `(tag, item_id)` for indexed queries:
```sql
SELECT item_id FROM lookup WHERE tag = ?
```

### 3. M2M (Many-to-Many) Tables
Classic relational: `items`, `tags`, and `item_tags` junction table with a composite primary key on `(tag_id, item_id)`:
```sql
SELECT it.item_id FROM item_tags it
JOIN tags t ON t.id = it.tag_id WHERE t.name = ?
```

### 4. FTS5 (Full-Text Search)
Store tags as space-separated text, index with FTS5:
```sql
SELECT rowid FROM items_fts WHERE tags_text MATCH '"tag_001"'
-- AND: MATCH '"tag_001" AND "tag_002"'
-- OR:  MATCH '"tag_001" OR "tag_002"'
```

### 5. LIKE (Delimited String)
Store tags as comma-delimited string (`,tag_001,tag_002,`), query with `LIKE`:
```sql
SELECT id FROM items WHERE tags_str LIKE '%,tag_001,%'
```

## Results

### Query Performance (median milliseconds)

| Strategy | Single Tag | AND (2 tags) | OR (2–5 tags) |
|----------|-----------|-------------|---------------|
| **JSON + lookup** | **1.37** | **1.88** | 11.02 |
| **M2M tables** | 1.41 | 2.26 | **10.69** |
| FTS5 | 3.28 | 2.59 | 13.54 |
| LIKE | 19.45 | 19.41 | 57.94 |
| JSON (no index) | 54.98 | 54.63 | 84.24 |

### Storage & Setup

| Strategy | Storage (KB) | Setup Time (ms) |
|----------|-------------|-----------------|
| FTS5 | 7,044 | 267 |
| LIKE | 7,244 | 167 |
| M2M tables | 8,772 | 1,313 |
| JSON (no index) | 9,124 | 295 |
| JSON + lookup | 20,344 | 1,002 |

### Correctness Check

All five strategies return identical result counts:
- Single tag: avg 6,535 matching rows
- AND (two tags): avg 411 matching rows
- OR (2–5 tags): avg 21,335 matching rows

## Analysis

### JSON (no index) — Avoid for queries
Raw JSON with `json_each()` is the slowest approach at **55ms per lookup**. Every query requires a full table scan with JSON parsing on each row. Fine for occasional queries on small datasets, but doesn't scale.

### JSON + Lookup Table — Fast but redundant
Adding a materialized lookup table makes JSON-stored tags just as fast as M2M for queries (**1.4ms**). The trade-off is doubled storage (20MB vs 9MB) and keeping the lookup table in sync with the JSON column. This might make sense if you want the JSON column for easy serialization/API responses but need fast queries too.

### M2M Tables — The solid default
Classic relational approach delivers **1.4ms single-tag lookups** with reasonable storage (8.7MB). Well-understood, well-indexed, and predictable. The only downside is setup complexity — three tables, foreign keys, and junction table inserts. The slightly slower setup time (1.3s) reflects the additional index creation.

### FTS5 — Surprisingly good
FTS5 is a strong contender at **3.3ms for single tags** and **2.6ms for AND queries**. It has the smallest storage footprint (7MB) and fast setup. AND/OR queries use natural search syntax. The main risk is tokenization: FTS treats text as natural language, so tag names must be chosen carefully to avoid false matches (tags containing spaces or punctuation could cause issues). Quoting tags in queries (`"tag_001"`) helps, but underscores in tag names get tokenized.

### LIKE — Simple but slow
LIKE queries are dead simple to implement — just one column, no extra tables. At **19ms per query**, they're 3x faster than raw JSON but 14x slower than indexed approaches. The comma-delimiter trick (`,tag,`) prevents partial matches. Good enough for small datasets or infrequent queries.

## Recommendations

| Use Case | Recommended Strategy |
|----------|---------------------|
| General purpose, production apps | **M2M tables** |
| Read-heavy, tags are search-like | **FTS5** |
| Need JSON API + fast queries | **JSON + lookup table** |
| Small dataset, simple code | **LIKE** |
| Never use for querying | JSON (no index) |

## Running the Benchmark

```bash
python benchmark.py
```

Results are printed to stdout and saved to `results.json`.
