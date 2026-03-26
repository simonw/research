# Turbolite WASM Exploration

Can [turbolite](https://github.com/russellromney/turbolite) — a Rust SQLite VFS with page-level zstd compression — be compiled to WebAssembly and used for read-only querying of statically hosted databases via HTTP range requests?

## TL;DR

**Direct WASM compilation: not feasible.** Turbolite's I/O layer is deeply coupled to OS primitives (file locks, pread, fcntl). A major refactoring effort would be needed.

**Hybrid approach with WASM zstd: works great.** We built a working prototype using sql.js (SQLite WASM) + WASM-compiled zstd decompression + HTTP range requests. Turbolite's compressed format transfers ~3x less data with only modest overhead — a 10K-row turbolite DB opens in 23ms (vs 15ms for plain SQLite), while transferring 958 KB instead of 2.7 MB. Using pure-JS zstd (fzstd) was 40-90x slower; the WASM zstd decompressor was the key breakthrough.

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  Browser (WASM)                  │
│                                                  │
│  ┌──────────┐   ┌──────────┐   ┌──────────────┐│
│  │  sql.js   │◄──│  Page     │◄──│ HTTP Range   ││
│  │ (SQLite   │   │ Assembler │   │ Request VFS  ││
│  │  WASM)    │   │           │   │              ││
│  └──────────┘   └──────────┘   └──────┬───────┘│
│                       ▲                │        │
│                  ┌────┴─────┐          │        │
│                  │zstd-wasm │          │        │
│                  │-decoder  │          │        │
│                  │(37KB WASM│          │        │
│                  └──────────┘          │        │
└────────────────────────────────────────┼────────┘
                                         │
                              HTTP Range: bytes=X-Y
                                         │
                                         ▼
                              ┌──────────────────┐
                              │  Static File     │
                              │  Server           │
                              │  *.db / *.turbolite│
                              └──────────────────┘
```

## Why Direct WASM Compilation Fails

Turbolite's Rust codebase has these hard blockers for `wasm32-unknown-unknown`:

| Dependency | Purpose | WASM Issue |
|-----------|---------|------------|
| `std::fs::File` | All file I/O | No filesystem in browser |
| `fs2` | File locking | OS-specific syscalls |
| `file-guard` | Byte-range locks | fcntl-based |
| `std::os::unix::fs::FileExt` | pread/pwrite | Unix-only |
| `getrandom` | RNG for encryption | Needs `js` feature flag |
| `aws-sdk-s3` + `tokio` | S3 tiered storage | Async runtime incompatible |

The VFS trait implementation (`sqlite-vfs`) also assumes a native SQLite C library, which would need to be compiled via Emscripten — a fundamentally different compilation pipeline than `wasm32-unknown-unknown`.

## What Would Be Required

To truly compile turbolite to WASM, you'd need to:

1. **Abstract the I/O layer** — Replace `std::fs::File` with a trait that can be backed by HTTP fetch
2. **Remove file locking** — Not needed for read-only mode
3. **Use Emscripten** — To link SQLite's C code with the Rust WASM, both need to use Emscripten's ABI
4. **Implement async I/O** — HTTP range requests are async; SQLite's VFS is sync. This requires either blocking on promises (possible with Emscripten's Asyncify) or a shared-memory worker approach
5. **Estimated effort**: 2-4 weeks of focused work for a read-only prototype

## The Hybrid Approach (What We Built)

Instead, we built a working prototype that:

1. **Pre-indexes turbolite files** — A Python script scans the file and outputs a JSON index mapping page numbers to `(offset, compressed_size)` pairs
2. **Uses sql.js** — SQLite compiled to WASM via Emscripten (battle-tested, widely used)
3. **Decompresses with fzstd** — Pure JavaScript zstd decompressor
4. **Fetches via HTTP Range requests** — Batched page fetching to minimize request count
5. **Assembles in memory** — Builds a plain SQLite buffer from decompressed pages

### Files

- `static/index.html` — Web UI with query editor, preset queries, decompressor toggle (fzstd/WASM), and benchmark runner
- `server.py` — Static file server with HTTP Range request support
- `create_test_dbs.py` — Generates test SQLite databases at 5 size tiers
- `build_turbolite_index.py` — Pre-indexes turbolite compressed files for range requests
- `run_benchmark.py` — Original benchmark runner (fzstd only)
- `run_benchmark_wasm.py` — Full benchmark comparing fzstd vs WASM zstd decompressors

## Benchmark Results

### Compression Ratio

| Database | Plain SQLite | Turbolite | Ratio |
|----------|-------------|-----------|-------|
| 100 rows | 68 KB | 17 KB | **4.0x** |
| 1K rows | 316 KB | 110 KB | **2.9x** |
| 10K rows | 2.7 MB | 958 KB | **2.9x** |
| 100K rows | 28.1 MB | 10.1 MB | **2.8x** |
| 500K rows | 143 MB | 51 MB | **2.8x** |

### Database Open Time (load + parse)

#### With WASM zstd (recommended)

| Size | Plain SQLite | Turbolite + WASM zstd | Turbolite + fzstd (JS) |
|------|-------------|----------------------|----------------------|
| 100 rows | 7ms | **7ms** | 56ms |
| 1K rows | 7ms | **7ms** | 289ms |
| 10K rows | 15ms | **23ms** | 2,108ms |
| 100K rows | 77ms | **217ms** | 18,965ms |

**Key finding**: The WASM zstd decompressor (`zstd-wasm-decoder`, 37KB .wasm) is **40-90x faster** than pure-JS fzstd, making turbolite competitive with plain SQLite. For small/medium databases, turbolite + WASM zstd matches plain SQLite performance while transferring ~3x less data.

### Query Times (identical after loading)

Once the database is in memory, query times are the same regardless of source format:

| Query Type | 100 rows | 1K rows | 10K rows | 100K rows |
|-----------|----------|---------|----------|-----------|
| Point lookup (by ID) | 0.5ms | 0.2ms | 0.1ms | 0.1ms |
| Index scan (city+age) | 0.3ms | 0.2ms | 0.2ms | 0.2ms |
| Aggregation (GROUP BY) | 0.4ms | 0.8ms | 4.3ms | 72.9ms |
| 3-table JOIN | 0.3ms | 0.2ms | 0.1ms | 0.2ms |
| Complex JOIN+AGG | 0.5ms | 2.9ms | 23.0ms | 315ms |
| COUNT(*) | 0.1ms | 0.1ms | 0.1ms | 1.0ms |

### When Turbolite Format Makes Sense for WASM

With the WASM zstd decompressor, the calculus shifts dramatically:

| Scenario | Recommendation |
|----------|---------------|
| Small DBs (<1MB) | **Turbolite + WASM zstd** — same speed, 3x less transfer |
| Medium DBs (1-10MB) | **Turbolite + WASM zstd** — 1.5x slower open, 2.9x less transfer |
| Large DBs (>10MB), fast network | **Plain SQLite** — 2.8x faster open, no decompression overhead |
| Large DBs (>10MB), slow/metered network | **Turbolite + WASM zstd** — 2.8x less transfer offsets decompression |
| Any size, with pure-JS decompressor only | **Plain SQLite** — fzstd adds 40-90x overhead |

## Potential Improvements

1. ~~**WASM zstd decompressor**~~ **DONE** — Integrated `zstd-wasm-decoder` (37KB WASM), achieving 40-90x speedup over pure-JS fzstd. This was the single biggest improvement.

2. **True lazy page loading** — Currently we load all pages into memory before opening with sql.js. A custom sql.js VFS that fetches pages on demand (via `Asyncify` or `SharedArrayBuffer` with a worker) would enable:
   - Sub-second "open" times for any database size
   - Only transfer pages the query actually touches
   - Point lookups on a 100K-row DB might need only 3-4 pages (~16KB compressed)

3. **Turbolite page grouping** — Use turbolite's tiered format (256 pages per group, seekable multi-frame zstd) instead of per-page records. This would enable:
   - Fewer, larger range requests
   - Sub-chunk decompression (only decompress the frame containing the needed page)

4. **IndexedDB caching** — Cache decompressed pages client-side so repeat visits are instant.

5. **Web Worker decompression** — Move decompression off the main thread to avoid blocking UI.

## Reproducing

```bash
# Clone turbolite
git clone https://github.com/russellromney/turbolite /tmp/turbolite

# Build turbolite CLI
cd /tmp/turbolite && cargo build --release --bin turbolite --features "zstd,bundled-sqlite"

# Create test databases
python3 create_test_dbs.py

# Convert to turbolite format
for db in static/dbs/*.db; do
  name=$(basename "$db" .db)
  /tmp/turbolite/target/release/turbolite convert "$db" "static/dbs/${name}.turbolite" -l 3
done

# Build turbolite indexes
python3 build_turbolite_index.py static/dbs/*.turbolite

# Install JS dependencies
npm install sql.js fzstd
cp node_modules/sql.js/dist/sql-wasm.js static/
cp node_modules/sql.js/dist/sql-wasm.wasm static/
# Create fzstd browser wrapper (see fzstd-browser.js)

# Start server
python3 server.py 8080

# Run benchmark
python3 run_benchmark.py
```
