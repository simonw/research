# Drilldown dashboard cubes over HTTP range requests

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

An investigation into whether a purpose-built binary format can replace Parquet + [Hyparquet](https://github.com/hyparam/hyparquet) for the "customer dashboard served from one static file via HTTP range requests" pattern described in Hamilton Ulmer's August 2026 post *Fast drilldown dashboards from a single Parquet file*.

The answer: yes, comfortably. **DCB1** is a format whose entire reader is 188 lines of dependency-free JavaScript (7.5KB, 2.8KB gzipped) and answers every dashboard interaction with one in-memory binary search plus one `fetch()` range request. **DCB2** adds per-block compression using the browser-native `DecompressionStream` API — still zero dependencies — and lands within 4% of the source Parquet's size. Both were validated against the post's real 40MB NYC 311 cube (16.7M aggregate rows), with every query verified byte-identical to DuckDB reading the original Parquet.

## Live demo

**<https://simonw.github.io/research/http-range-drilldown-cubes/demo.html>** — a reimplementation of the post's dashboard UI (daily line chart, four clickable leaderboards, chart brushing) in one dependency-free HTML file, running against `nyc311-demo.dcb2`: a 3.75MB real-data cube carved from the full 130MB conversion by [`build_demo_cube.py`](build_demo_cube.py) — generated at deploy time by this folder's [`github-pages.sh`](github-pages.sh) hook rather than committed. GitHub Pages serves byte ranges, so the page works there as-is.

## The Pages build caches itself

[`github-pages.sh`](github-pages.sh) runs before the site rsync (via the repo's hook mechanism) and publishes two artifacts that are never committed: the **full real-data cube** `nyc311-cube-v15.dcb2` (41.2MB) and the demo cube (3.75MB). Producing them means downloading the 40MB source Parquet from `https://static.simonwillison.net/static/2026/nyc311-cube-v15.parquet` and converting it, so the hook first checks whether the *previous* deploy already published current copies — **GitHub Pages doubles as the build cache**. A version stamp (sha256 over the three converter scripts, plus the Parquet's `ETag` and `Content-Length` from a HEAD request) is published as `cube-build-version.txt`; when the live site's stamp matches, the hook downloads the published artifacts and ships them as-is — no Parquet download, no Python environment. Change a converter or replace the Parquet at that URL and the stamp changes, forcing a rebuild. The converter scripts carry PEP 723 inline metadata so `uv run` supplies pyarrow/numpy in CI.

All three paths are tested: full rebuild (28s end to end, output **byte-identical** to the original conversion — the pipeline is deterministic), cache reuse (0.25s against a stand-in server), and local no-op. The hook deliberately never fails the site build: if both the cache and a rebuild fail, it emits `::warning::` annotations and exits 0, so one broken data source can't block deploys of every other folder in the repo.

A side effect worth having: the published full cube means anyone can query all 34,163,328 NYC 311 events from a browser. From the demo page's devtools console:

```js
const { CubeReader } = await import("./cube-reader2.js");
const cube = await CubeReader.open("./nyc311-cube-v15.dcb2");
await cube.query("day:agency+complaint",
  { agency: "NYPD", complaint: "Illegal Parking" }, { groupBy: "d" });
```

The demo cube's section design is itself part of the research. Leaderboards for *any* filter come from the 4,794-row all-time section, fetched once and held in memory; each leaderboard click fetches one contiguous slice of a per-dimension daily section; and brushes read `week:by-date` — the full-dimension weekly section **re-sorted with the date first in the sort key**, which extends the post's "sorted by the columns its queries filter on" principle to time brushing: a brush becomes one contiguous range read, with dimension filters applied client-side after decode. The time-first sort also compresses better (4.5x vs 4.0x dimension-first, because each 1024-row block spans roughly one week and the date column collapses). Brushes snap to whole weeks so the weekly counts stay exact — verified: Monday-aligned brush totals match the daily lines to the request. First paint is 3 range requests, ~54KB, 1.5% of the file; the page shows its own fetch accounting live, like the original. It supports one leaderboard filter at a time — stacked filters are what the full-size cube's 2⁴ daily subsets are for.

## Why this is possible

A drilldown dashboard over precomputed grouping sets only ever needs three primitives:

1. Read a whole tiny section (leaderboard totals).
2. Read the contiguous slice of a big section matching an equality prefix of its sort key (`agency = 'NYPD'`), optionally narrowed by a range on the next key column (a time brush).
3. Sum small integers client-side.

Because every section is sorted by its filter columns, primitive 2 is always one contiguous row range. Parquet delivers that contiguity through its footer's row-group min/max statistics; a purpose-built format only has to solve the same narrow problem: turn *filter values* into *byte offsets* without downloading the data.

## DCB1: the raw format

```
bytes 0-3   magic "DCB1"
bytes 4-7   uint32 LE: header length H
bytes 8..   header: one JSON object (dictionaries, section table, sparse index)
then        section bodies, back to back: fixed-width dictionary-encoded rows,
            sorted by each section's sort key
```

Two design moves do all the work (full details in [SPEC.md](SPEC.md)):

- **Dictionary-encoded, fixed-width rows.** Every dimension value becomes a small integer, with ids assigned in sorted value order so comparing ids is comparing strings. Every row in a section is the same width, so row → byte offset is multiplication.
- **A sparse key index in the JSON header** — the sort key of every Nth row. The client binary-searches it in memory, bounds any query to a row range (over-reading at most N−1 rows per edge, discarded by the exact post-decode filter), and issues exactly one range request. It is a static two-level B-tree whose root lives in the header — the same role Parquet's footer and [PMTiles](https://github.com/protomaps/PMTiles)' root directory play.

A pleasant side effect: a rolled-up dimension is simply *absent* from a section's schema. The `NULL`-vs-rollup ambiguity that plagues relational `GROUPING SETS` output (and the `'*'` sentinel workaround needed in a SQLite cube table) disappears structurally.

The file is `curl -r`-and-`jq`-debuggable: bytes 8 onward are plain JSON.

## Validating against the real NYC 311 cube

The post's actual data cube (`nyc311-cube-v15.parquet`, 39,759,867 bytes, 16,689,105 rows, 326 row groups) revealed its design during conversion, via an explicit `grouping_set` TINYINT column. It contains **19 grouping sets**: all 2⁴ = 16 subsets of the four dimensions at daily grain, plus full-dimension weekly, yearly, and all-time sets. Every set independently covers all 34,163,328 source events — the conversion verifies that as its consistency invariant. This inventory explains the post's low latencies: clicking an agency reads a slice of the dedicated 82,036-row `(day, agency)` section, never the 3.27M-row full section.

| section | rows | rowSize | MB |
|---|---:|---:|---:|
| all:agency+complaint+borough+channel | 4,794 | 9 | 0.0 |
| week:agency+complaint+borough+channel | 796,645 | 9 | 7.2 |
| year:agency+complaint+borough+channel | 29,680 | 11 | 0.3 |
| day:overall | 5,006 | 4 | 0.0 |
| day:channel | 22,694 | 5 | 0.1 |
| day:borough | 30,032 | 5 | 0.2 |
| day:complaint | 635,575 | 6 | 3.8 |
| day:agency | 82,036 | 5 | 0.4 |
| day:borough+channel | 127,713 | 6 | 0.8 |
| day:complaint+channel | 1,090,675 | 7 | 7.6 |
| day:complaint+borough | 2,107,281 | 7 | 14.8 |
| day:agency+channel | 198,027 | 6 | 1.2 |
| day:agency+borough | 358,512 | 6 | 2.2 |
| day:agency+complaint | 642,976 | 7 | 4.5 |
| day:complaint+borough+channel | 3,264,874 | 8 | 26.1 |
| day:agency+borough+channel | 803,982 | 7 | 5.6 |
| day:agency+complaint+channel | 1,096,772 | 8 | 8.8 |
| day:agency+complaint+borough | 2,118,277 | 8 | 16.9 |
| day:agency+complaint+borough+channel | 3,273,554 | 9 | 29.5 |

Conversion (`parquet_to_dcb1.py`, vectorized with pyarrow + numpy) took 21.4s and produced a 130,120,633-byte file with a 149KB JSON header. Two adaptations the real data forced, both within the format: per-section measure width (`n` is u16 where the section's max count fits, u32 in the yearly/all-time sections where counts reach 406,026), and adaptive index granularity (`every` scales 256→4096 in multi-million-row sections, capping each at 1,024 index entries).

Measured over a real HTTP range server, each interaction is a single request:

| interaction | bytes fetched |
|---|---:|
| open() — header | 149.1 KB (2 requests) |
| all-time agency leaderboard | 42.1 KB |
| click "NYPD": full 13.7-year daily line | **25.0 KB** |
| complaint leaderboard while NYPD selected | 44.0 KB |
| brush 2013 on the NYPD line | 2.5 KB |
| NYPD + Illegal Parking daily line | 42.0 KB |
| 4-filter drilldown, raw daily rows | 108.0 KB |
| **cold 7-interaction session** | **413 KB = 0.32% of the file** |

The post prices the NYPD click at ~260KB against its Parquet; the 10x gap here is not format superiority — it is mostly the 256-row index resolution versus ~50K-row Parquet row groups, plus zero per-read metadata. The results themselves were verified byte-identical to DuckDB running the equivalent queries against the original Parquet (5,005-point daily series, 42-row leaderboard, 4,852-row drilldown).

## DCB2: compression, natively in the browser

Transport-level compression (`Content-Encoding`) does not compose with range requests — but *storage-level* compression does: compress each index-aligned block of rows independently, record per-block byte offsets in the index, and inflate client-side with `DecompressionStream("deflate-raw")` ([MDN](https://developer.mozilla.org/en-US/docs/Web/API/DecompressionStream)), native in Chrome 103+, Safari 16.4+, Firefox 113+, and Node 18+. Blocks covering a contiguous row range are themselves contiguous, so a query is **still one range request** — the fetched bytes are just denser. The JSON header is itself deflated (186KB → 72KB).

Measurement drove every decision:

- deflate on real blocks achieves 3.4–3.9x on the big sections; **level 9 and zstd-12 gain under 5% over level 6**, so zstd's WASM bundle is definitively not worth shipping;
- column-transposing each block before compression helps modestly (weekly section 3.0x → 3.9x) — less than folklore suggests, because deflate's 32KB window already spans a whole block;
- sub-KB blocks compress at only 1.6x, so DCB2 floors block size at 1,024 rows.

Result: **130.1MB → 41,225,608 bytes (3.16x) in 4.9s — within 4% of the 39.8MB source Parquet.** Fixed-width-sorted-plus-deflate lands almost exactly where Parquet's fancier encodings plus its compressor do on this data. Side-by-side interaction costs, with results asserted byte-identical between formats (`compare_v1_v2.js`):

| interaction | DCB1 raw | DCB2 deflate |
|---|---:|---:|
| open() — header | 149.1 KB | 128.0 KB |
| all-time agency leaderboard | 42.1 KB | 12.8 KB |
| click "NYPD" daily line | 25.0 KB | 19.6 KB |
| complaint leaderboard @ NYPD | 44.0 KB | 12.8 KB |
| brush 2013 on the NYPD line | 2.5 KB | 3.3 KB |
| NYPD + Illegal Parking daily | 42.0 KB | 18.6 KB |
| 4-filter drilldown, raw rows | 108.0 KB | 35.0 KB |
| **total cold session** | **412.7 KB** | **230.0 KB** |

The one honest regression: the tiny 2013 brush costs slightly *more*, because the fetch floor is now one compressed block and granularity coarsened from 256 to 1,024 rows — the compression-versus-random-access-granularity trade in miniature. The v2 reader (9.1KB, 3.3KB gzipped) reads both formats; back-compat on v1 files is asserted in the comparison harness.

## Files

| file | what it is |
|---|---|
| [SPEC.md](SPEC.md) | The DCB1 format spec plus the DCB2 compression addendum |
| [cube-reader.js](cube-reader.js) | v1 reader: 188 lines, zero dependencies, browsers + Node 18+ |
| [cube-reader2.js](cube-reader2.js) | v2 reader: adds DCB2 support (deflated header, columnar compressed blocks) |
| [pack_cube.py](pack_cube.py) | Packs a SQLite `cube` table into DCB1 (stdlib only) |
| [make_demo_db.sh](make_demo_db.sh) | Regenerates the synthetic 200k-event SQLite cube the demo uses |
| [demo.js](demo.js) | Range-request HTTP server + instrumented queries against `cube.bin` |
| [cube.bin](cube.bin) | The synthetic demo cube (3.6MB) so `node demo.js` runs as-is |
| [demo.html](demo.html) | Browser dashboard demo (line chart, leaderboards, brushing) — [live on Pages](https://simonw.github.io/research/http-range-drilldown-cubes/demo.html) |
| `nyc311-demo.dcb2` | 3.75MB real-data cube backing the browser demo — generated at deploy time, not committed |
| [github-pages.sh](github-pages.sh) | Pages build hook: generates and publishes both cubes, using the live site as its cache |
| [build_demo_cube.py](build_demo_cube.py) | Carves the demo cube out of the full 130MB DCB1 (adds the time-first `week:by-date` section) |
| [parquet_to_dcb1.py](parquet_to_dcb1.py) | Vectorized converter from the NYC 311 Parquet cube to DCB1 |
| [dcb1_to_dcb2.py](dcb1_to_dcb2.py) | Lossless DCB1 → DCB2 recompressor |
| [validate_nyc.js](validate_nyc.js) | Runs the post's dashboard interactions against the converted NYC cube |
| [compare_v1_v2.js](compare_v1_v2.js) | Side-by-side v1/v2 byte costs with result-equality assertions |
| [notes.md](notes.md) | Chronological work log |

Not committed (over this folder's 5MB size limit): the source `nyc311-cube-v15.parquet` (39.8MB, fetched from static.simonwillison.net at build time), the intermediate `nyc311-cube-v15.dcb1` (130.1MB, a build temp), and `cube_demo.db` (38MB, regenerable via `make_demo_db.sh`). `nyc311-cube-v15.dcb2` (41.2MB) and `nyc311-demo.dcb2` (3.75MB) are published on the site by the build hook without ever being committed.

## Reproducing

The synthetic pipeline is self-contained:

```bash
./make_demo_db.sh          # cube_demo.db: 200k fake 311-ish events, 8 grouping sets
python3 pack_cube.py       # -> cube.bin (already included here)
node demo.js               # serves it over HTTP ranges, prints bytes per interaction
```

To view the browser demo locally, serve this folder with any static server that supports `Range` headers and open `demo.html` — `npx http-server` works; `python3 -m http.server` does **not** (it ignores Range requests, and the page will tell you so). To produce both generated cubes locally, run `bash github-pages.sh` — it reuses the published copies when it can reach the live site, and otherwise downloads the Parquet and rebuilds (`uv` supplies the Python dependencies).

The NYC pipeline needs the source Parquet from the post's dashboard, then:

```bash
python3 parquet_to_dcb1.py nyc311-cube-v15.parquet nyc311-cube-v15.dcb1
python3 dcb1_to_dcb2.py    # -> nyc311-cube-v15.dcb2
node compare_v1_v2.js      # asserts identical results, prints the cost table
```

## Deployment notes and honest caveats

Any static host with range support works (S3, R2, GitHub Pages, nginx). Allow the `Range` header in the bucket's CORS config or fetches fail preflight; S3 and R2 honor only one range per request, so coalesce or parallelize; version filenames (or send `If-Match` per request) so a mid-session rebuild cannot pair an old header with new bytes; never rely on `Content-Encoding`.

Versus just using Hyparquet: you give up compression sophistication (Parquet's delta and bit-packing encodings are where its remaining ~2x lives), all ecosystem interop (DuckDB cannot open your cube for debugging), and you own the format forever. Distributive/algebraic aggregates only — the same limit any cube has. In exchange: a reader small enough to inline in a blog post, no parsing machinery, and a spec you can hold in your head. Over the course of the investigation the format reconstructed Parquet's column chunk one feature at a time — dictionary encoding, sorted pages, a min-key index, per-page compression — keeping only the slice this workload needs.

## Provenance

Carried out interactively in a Claude (claude.ai) session on 2026-08-24, prompted by Hamilton Ulmer's post; this folder reorganizes that session's artifacts. The NYC 311 cube used for validation is the actual Parquet file backing the post's embedded dashboard.
