# Investigation notes: drilldown cubes over HTTP range requests

These notes reconstruct the chronology of an interactive claude.ai session on
2026-08-24 (times UTC); the work happened conversationally rather than as an
async agent run, so this log was assembled from the session rather than
appended live.

## ~15:25 — SQLite grouping-set cubes

Context: Hamilton Ulmer's post serves a drilldown dashboard from a 40MB
Parquet data cube via HTTP range requests (Hyparquet in the browser). First
task: express the same data cube idea in plain SQLite, no DuckDB.

- SQLite has no `GROUPING SETS` / `CUBE` / `ROLLUP` / `date_trunc`. The cube
  is a `UNION ALL` of one `GROUP BY` per grouping set, with an explicit `gset`
  label column — arguably clearer than decoding `GROUPING_ID()` bitmasks.
- Used a `'*'` sentinel instead of NULL for rolled-up dimensions: `WITHOUT
  ROWID` primary keys can't contain NULL, and it kills the "is NULL a rollup
  or missing data?" ambiguity.
- Key insight: `WITHOUT ROWID` with `PRIMARY KEY (gset, filter columns...,
  period)` makes the table a clustered B-tree in exactly that order — SQLite's
  equivalent of Parquet's file sort, with interior pages playing the role of
  the footer's min/max index. Insertion order doesn't even matter.
- Week truncation idiom: `date(x, 'weekday 0', '-6 days')` = Monday of x's
  week. Verified Mon 2026-08-24 maps to itself, Sun 2026-08-23 to 2026-08-17.
- Built a 200k-event synthetic 311-ish cube; `EXPLAIN QUERY PLAN` confirmed
  filtered queries are `SEARCH cube USING PRIMARY KEY (gset=? AND agency=?)` —
  one contiguous range.
- Gotcha: container had no sqlite3 CLI; first `apt-get install` 404'd on a
  stale package list, fixed with `apt-get update`.
- Cross-checked one series against the cube by hand; every grouping set sums
  to the full event count — the invariant that later became the converter's
  consistency check.

## ~15:35 — DCB1: a purpose-built binary format

Question: the cube workload needs so little from a file format (read a tiny
section whole; read a sort-key-prefix slice of a big section; sum ints) — can
a custom format beat Parquet-reader complexity with plain `fetch()` ranges?

- Design: fixed-width dictionary-encoded rows (ids assigned in sorted value
  order so id comparison == string comparison), one JSON header holding
  dictionaries + section table + a sparse key index (sort key of every Nth
  row). Client binary-searches the index in memory, then issues one range
  request. Rolled-up dimensions are simply absent from a section's schema —
  the `'*'` sentinel problem disappears structurally.
- Section offsets are relative to end-of-header so the header never needs to
  know its own length while being built.
- Wrote `pack_cube.py` (SQLite → DCB1, stdlib only), `cube-reader.js` (188
  lines, zero deps, browsers + Node 18+), `demo.js` (Node HTTP server with
  real Range support — `python3 -m http.server` doesn't do ranges).
- Measured on the 3.6MB synthetic cube: leaderboard = 25 bytes, yearly
  filtered line = 15.7KB, worst case (full NYPD daily line) = 390KB, all
  single requests. Results byte-for-byte identical to the same queries in
  SQLite.

## ~15:50 — converting the post's real Parquet cube

`nyc311-cube-v15.parquet` (39,759,867 bytes, 16,689,105 rows, 326 row
groups) — the actual file behind the post's dashboard.

- Schema: `d DATE, agency, complaint, borough, channel, n INT32, grouping_set
  TINYINT`. The explicit set id made discovery easy.
- **Finding: 19 grouping sets = all 2^4 subsets of the four dimensions at
  daily grain, plus full-dimension weekly, yearly, and all-time.** Every set
  sums to 34,163,328 events. This explains the post's latencies: an agency
  click reads the dedicated 82,036-row `(day, agency)` section, not the 3.27M
  row full section.
- Grain detection from data: weekly rows are all Mondays (`days % 7` single
  value; 1970-01-01 was a Thursday); the yearly set uses ISO-year start
  Mondays (min value 2008-12-29), so both grains encode as plain dates.
- Rewrote packing vectorized (pyarrow dictionary_encode + numpy lexsort +
  structured-array tobytes): 16.7M rows in 21.4s → 130.1MB DCB1.
- Real data forced two in-spec adaptations: per-section measure width (u16
  where max n < 65536, u32 where counts reach 406,026) and adaptive index
  granularity (`every` 256→4096, ≤1024 entries/section, header 149KB).
- Validation with the *unchanged* reader: NYPD click = 25.0KB single request
  (post cites ~260KB for its Parquet — mostly index resolution, 256 rows vs
  ~50K-row row groups, not format magic). DuckDB on the original Parquet
  returned byte-identical results for the daily series, the leaderboard, and
  the 4-filter drilldown.
- Fun red herring: two different queries both returned exactly 4,852 rows;
  verified real, not a bug — Illegal Parking was reported in Brooklyn by
  phone on 4,852 of 5,006 days.

## ~16:10 — DCB2: compression

Question: 130MB vs the 40MB Parquet — can compression help, and in browsers?

- Transport compression (`Content-Encoding`) doesn't compose with range
  requests; storage-level per-block compression does. Browsers inflate
  natively via `DecompressionStream("deflate-raw")` (Chrome 103+, Safari
  16.4+, Firefox 113+, Node 18+) — zero dependencies.
- Measured on real blocks before designing: big sections 3.4–3.9x with
  deflate; level 9 and zstd-12 gain <5% over level 6 (zstd's WASM not worth
  it); columnar transposition helps modestly (weekly 3.0x→3.9x — deflate's
  32KB window already spans a block, so the folklore win shrinks); sub-KB
  blocks manage only 1.6x → floor block size at 1024 rows.
- DCB2: blocks aligned to the index, column-transposed, deflate-raw level 6;
  `index.offsets` parallel to `index.keys`; header itself deflated (186KB →
  72KB). Contiguous blocks stay contiguous → still one range request.
- Gotcha: the uploaded Parquet had expired from the uploads mount by this
  point, so the converter became `dcb1_to_dcb2.py` reading the lossless DCB1
  instead.
- Result: 130.1MB → 41,225,608 bytes (3.16x) in 4.9s — within 4% of the
  source Parquet. Per-section ratios 1.1x (day:overall — sequential dates +
  varying counts, nothing for deflate without delta coding) to 4.7x.
- Mistake caught: a sed-derived validation script overwrote the saved
  DuckDB-verified ground-truth JSONs *before* comparing against them.
  Replaced with `compare_v1_v2.js`, which runs both readers against both
  files in one process and asserts equality on every interaction — all
  identical, and the v2 reader reads v1 files unchanged (back-compat).
- Interaction costs roughly halved-to-thirded (cold session 413KB → 230KB);
  one honest regression: the tiny 2013 brush 2.5KB → 3.3KB, because the
  fetch floor is one compressed block at coarser granularity.
- Readers: v1 7,520 bytes (2,847 gzipped), v2 9,115 bytes (3,336 gzipped).

## ~16:30 — packaging

Cloned simonw/research, read AGENTS.md and example folders, reorganized the
session's artifacts into this folder. Excluded files over 5MB per
instructions: the source Parquet (39.8MB), both converted cubes (130.1MB,
41.2MB), and the regenerable `cube_demo.db` (38MB). Included `cube.bin`
(3.6MB) so the synthetic demo runs as-is.

## ~17:00 — browser demo for GitHub Pages

Brief: a screenshot of the post's actual dashboard UI (daily line, four
leaderboards with proportional bars, brush-to-zoom, live fetch accounting) —
implement it as demo.html in this folder, against a smaller demo file.

- Checked the repo's Pages build first: `build-github-pages.sh` rsyncs the
  whole tree into `_site`, so a binary `.dcb2` publishes as-is; no
  `github-pages.sh` hook needed. GitHub Pages serves byte ranges.
- The synthetic cube.bin would make a boring demo (uniform random data — flat
  line, near-identical leaderboards), so carved a real-data cube from the
  130MB NYC DCB1 instead: `nyc311-demo.dcb2`, 3,753,834 bytes. Sections:
  all-time full-dimension (4,794 rows — every unbrushed leaderboard, any
  filter, held client-side after one fetch), day:overall, the four
  single-dimension daily sections, and `week:by-date`.
- `week:by-date` is the design addition: the weekly full-dimension section
  re-sorted with the date FIRST in the sort key. Brushes filter on time, so
  the brush-serving section puts time first — one contiguous range read per
  brush, dimension filters residual. Bonus: it compresses better than the
  dimension-first original (4.5x vs 4.0x) because each 1024-row block spans
  about one week and the date column collapses to a run.
- Verified the exact query patterns in Node before writing any UI: leaderboard
  aggregates off the all-time section reproduce the screenshot's numbers
  digit-for-digit (brooklyn 10,175,964; phone 15,552,187), and Monday-aligned
  brushes sum identically between the weekly slice and the daily lines, with
  and without a dimension filter. Chose to snap brushes to whole weeks in the
  UI so those counts are always exact rather than edge-approximated.
- demo.html is one dependency-free file: hand-rolled SVG chart (line + area +
  year ticks + pointer-capture brushing), leaderboards as buttons with
  proportional background bars, cross-filtering, and the live "N range
  requests · X kb · Y% of the cube" accounting. Single filter at a time —
  stacked filters are what the full cube's 2^4 subsets exist for. File size
  for the percentage comes from the header (max section offset+bytes), no
  HEAD request.
- Tested headlessly: extracted the page's module script and import-checked it
  in Node, then ran the full page under jsdom with fetch patched to a local
  range server — initial render (3 requests, 53.9KB, 1.5% of cube), chart SVG
  present, top agency NYPD 10,214,317, click NYPD → subtitle 10,214,317 and
  complaint board Noise - Residential 3,166,908, click again → cleared. All
  passed.

## ~17:20 — GitHub Pages as its own build cache

Brief: generate the full-size cube files during the Pages build itself (the
source Parquet is now published at
https://static.simonwillison.net/static/2026/nyc311-cube-v15.parquet), and be
clever about it: before downloading and converting, try to fetch the
previously *published* artifacts back from the live site — GitHub Pages as
the build cache.

- Read deploy-pages.yml first: ubuntu runner, uv installed, hooks run inside
  build-github-pages.sh before the rsync, so ::warning:: annotations work and
  anything the hook writes into the folder gets published.
- github-pages.sh publishes nyc311-cube-v15.dcb2 (41.2MB) and
  nyc311-demo.dcb2 (3.75MB), neither committed (per-folder .gitignore added;
  the demo cube came OUT of git — the hook mechanism is exactly AGENTS.md's
  "generated static assets" case).
- Cache-busting without downloading: the version stamp is a sha256 over the
  three converter scripts plus the Parquet's ETag + Content-Length from a
  HEAD request, published as cube-build-version.txt. A converter edit or a
  changed Parquet flips the stamp; a matching stamp means the hook just pulls
  the ~45MB back from the live site with curl — no Python at all.
- Made the converters `uv run`-able with PEP 723 inline dependency blocks and
  repo-local default paths (they had /mnt/... defaults from the session).
- Failure policy decision: the hook never exits nonzero. A broken data URL
  emitting ::warning:: and shipping a site without the cubes beats blocking
  deploys for every other folder in the repo. Documented in the script header.
- Tested all three paths in this container (simonw.github.io is unreachable
  from here, which conveniently forces the cache-miss branch): full rebuild
  downloads the Parquet and produces output byte-identical to the session's
  original conversions in 27.8s (the pipeline is deterministic — sorted
  dictionaries, stable lexsort, fixed deflate level); cache reuse against a
  localhost stand-in for Pages restores both artifacts byte-identically in
  0.25s; a second run is an instant no-op. First real deploy after this lands
  will take the rebuild path once, then every later deploy hits the cache.
- demo.html's failure message now tells local viewers to run
  `bash github-pages.sh` to generate the data file.
