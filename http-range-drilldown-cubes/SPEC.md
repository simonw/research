# DCB1 — a drilldown-cube binary format for HTTP range requests

A single-file format for precomputed dashboard aggregates (grouping sets),
designed so that a dependency-free browser client can answer any dashboard
interaction with **one in-memory binary search plus one `fetch()` range
request**. No Parquet reader, no Wasm, no query engine.

## Why this shape

The cube workload only ever needs three primitives:

1. Read a whole tiny section (leaderboard totals).
2. Read the contiguous slice of a big section matching an equality prefix of
   its sort key (`agency = 'NYPD'`), optionally narrowed by a range on the
   next key column (a time brush).
3. Sum small integers client-side.

Because every section is sorted by its filter columns, (2) is always one
contiguous row range. The format's only real job is letting the client turn
"filter values" into "byte offsets" without downloading the data. Two design
choices make that trivial:

- **Dictionary-encoded, fixed-width rows.** Every dimension value becomes a
  small integer; every row in a section is the same number of bytes. Row → byte
  offset is multiplication. Dictionary ids are assigned in sorted value order,
  so comparing ids is comparing values.
- **A sparse key index in the header.** The sort key of every Nth row (N = 256
  by default). The client binary-searches this array in memory to bound any
  query to a row range, over-fetching at most N-1 rows on each edge, which the
  exact post-decode filter discards.

A rolled-up dimension is simply *absent* from a section's schema — the format
expresses "aggregated over agency" structurally, so the `'*'` / NULL sentinel
problem from relational cube tables disappears.

## File layout

```
offset 0   4 bytes   magic "DCB1"
offset 4   4 bytes   uint32 LE: header length H
offset 8   H bytes   header, one UTF-8 JSON object
offset 8+H ...       section bodies, back to back, in header order
```

Section `offset` values are relative to `8 + H` (end of header), so the header
never needs to know its own length while being built.

## Header schema

```jsonc
{
  "format": "dcb1",
  "dicts": {                     // global, sorted: id order == value order
    "agency": ["DEP", "DOT", "DSNY", "HPD", "NYPD"],
    "borough": ["..."]
  },
  "sections": {
    "day:full": {
      "offset": 0,               // bytes from end of header
      "rows": 192633,
      "rowSize": 10,             // bytes; all rows fixed width
      "columns": [               // physical order within a row
        ["agency", "u8"], ["complaint_type", "u8"],
        ["submission_type", "u8"], ["borough", "u8"],
        ["period", "date-u16"], ["n", "u32"]
      ],
      "sortKey": ["agency", "complaint_type", "submission_type",
                  "borough", "period"],
      "index": { "every": 256, "keys": [[0,0,0,0,14610], ...] }
    }
  }
}
```

## Column types (all little-endian)

| type       | bytes | meaning                                          |
|------------|-------|--------------------------------------------------|
| `u8/u16/u32` | 1/2/4 | dictionary id (smallest that fits cardinality) or measure |
| `date-u16` | 2     | days since 1970-01-01 (good through 2149)        |
| `year-u16` | 2     | literal year                                     |

Measures are plain integers; add `f64` if you need non-count aggregates.
Distributive/algebraic aggregations only, same as any cube.

## Client query algorithm

```
open(url):
  fetch bytes 0..65535 (speculatively covers magic + header; one more
  request only if H > 64KB); parse JSON; build value→id maps.

query(section, filters, groupBy):
  1. Encode the leading equality filters (a prefix of sortKey) to ids;
     optionally encode a {gte, lt} bound on the next key column.
  2. lo = ids padded with -1;  hi = ids padded with +inf.
  3. Binary-search index.keys for lo and hi → [startRow, endRow),
     rounded outward to index granularity.
  4. fetch bytes [offset + startRow*rowSize, offset + endRow*rowSize).
  5. Decode with DataView; re-apply all predicates exactly (trims the
     ≤255-row over-read on each edge and handles non-prefix filters);
     group + sum; map ids and dates back to strings.
```

Filters that are *not* a sort-key prefix still work — they just scan the range
bounded by whatever prefix was given (worst case: the whole section) and filter
after decoding. That is the same contiguity contract the Parquet layout has;
the coarse-grain sections (week/year) exist to keep those scans small.

## HTTP deployment notes

- **Any static host with range support works**: S3, R2, GitHub Pages, nginx.
- **CORS**: allow the `Range` request header on the bucket
  (`AllowedHeaders: ["Range"]`) or fetches will fail preflight; expose
  `Content-Range` if you read it.
- **One range per request**: S3 and R2 don't honor multi-range requests, so
  issue parallel single-range fetches and coalesce adjacent ranges client-side.
- **No HTTP compression**: `Content-Encoding` and range requests don't
  compose. Store raw; the fixed-width encoding is your compression. Fine,
  because reads are sliced.
- **Rebuild consistency**: replacing the file mid-session can pair an old
  header with new data. Either write immutable versioned filenames plus a tiny
  `latest.json` pointer, or capture the `ETag` at open() and send
  `If-Match` on every range request, re-opening on 412.
- **Auth**: identical to the Parquet version — signed URL per customer file,
  or a thin Worker checking the session.

## Measured (200k-event demo, 3.6MB file, 8 sections)

| interaction                             | requests | bytes    |
|-----------------------------------------|----------|----------|
| open (header)                           | 1        | 64KB     |
| agency leaderboard                      | 1        | 25 B     |
| yearly line, agency=NYPD                | 1        | 15.7KB   |
| daily line, agency=NYPD                 | 1        | 390KB    |
| 4-dimension drilldown, daily rows       | 1        | 7.7KB    |

Results verified byte-for-byte identical to the same queries run in SQLite.

---

# DCB2 addendum — per-block compression

Transport compression (`Content-Encoding`) doesn't compose with range
requests, but **storage compression does**: compress each index-aligned block
of rows independently and record per-block byte offsets in the header.
Browsers decompress natively via `DecompressionStream("deflate-raw")`
(Chrome 103+, Safari 16.4+, Firefox 113+, Node 18+) — still zero dependencies.

## Layout changes vs DCB1

```
bytes 0-3   magic "DCB2"
bytes 4-7   uint32 LE: COMPRESSED header length H
bytes 8..   deflate-raw( JSON header )
8+H ..      compressed blocks, back to back, section by section
```

Per section: `"codec": "deflate-raw"`, `"layout": "columnar"`, `"bytes"`
(compressed section length), and `index.offsets` — the compressed byte offset
of each block, parallel to `index.keys`. A block covers `every` rows and is
stored **column-transposed** (all of column 0, then column 1, ...) before
compression. Block size floors at 1024 rows: measured, sub-KB blocks compress
at only ~1.6x while 4096-row blocks reach ~3.9x.

## Query algorithm changes

Identical binary search over `index.keys`, yielding a block range instead of a
row range. Blocks for a contiguous row range are contiguous in the file, so it
is **still one range request**; the client then inflates each block (they are
independent deflate streams) and scans column-major. Byte position comes from
`index.offsets` lookup rather than multiplication — the one aesthetic
casualty, still O(1).

## Measured on the NYC 311 cube (16.7M rows, 19 sections)

Raw 130.1MB -> 41.2MB (3.16x), within 4% of the source 40MB
zstd-era Parquet. Header 186KB JSON -> 72KB deflated. Typical interactions
cost 2-3x fewer bytes than DCB1 (leaderboard 42->13KB, 4-filter drilldown
108->35KB); very small slices can cost slightly more (2013 brush 2.5->3.3KB)
because the fetch floor is one compressed block. Choices that measurement
settled: deflate level 6 (level 9 and zstd-12 gained <5%, and zstd would
require shipping WASM), columnar transposition (helps most on the weekly
section, 3.0x -> 3.9x). Reader grows from 7.5KB to 9.1KB (3.3KB gzipped) and
reads both formats.
