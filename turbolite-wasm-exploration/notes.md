# Turbolite WASM Exploration - Working Notes

## Goal
Investigate compiling turbolite (a Rust SQLite VFS for S3 with compression) to WebAssembly,
and using it against statically hosted files with HTTP range requests for read-only querying.

## What is Turbolite?

Turbolite is a high-performance SQLite VFS (Virtual File System) written in Rust. Key features:
- Page-level zstd compression (also lz4, snappy, gzip)
- S3-backed tiered storage with sub-250ms cold queries
- Page grouping and prefetching for efficient range requests
- AES-256 encryption
- B-tree introspection for intelligent page grouping

Repository: https://github.com/russellromney/turbolite

## Turbolite File Format (CompressedVfs)

The local compressed format:
- **Header** (64 bytes): magic `SQLCEvfS`, page_size(u32 LE), data_start(u64 LE), dict_size(u32 LE), flags(u32 LE)
- **Dictionary** (optional): zstd dictionary bytes immediately after header
- **Records**: sequential page records
  - page_num(u64 LE) + data_size(u32 LE) + compressed_data(data_size bytes)
- Pages are **0-indexed** (page 0 = SQLite's header page)
- On write, new versions of pages are appended; the last entry wins

## Attempt 1: Direct WASM Compilation

Tried `cargo build --lib --target wasm32-unknown-unknown --no-default-features --features zstd`

### Blockers Found:
1. **`getrandom` crate** - doesn't support wasm32-unknown-unknown by default (needs `js` feature)
2. **`fs2` crate** - file locking, uses OS-level fcntl calls
3. **`file-guard` crate** - byte-range file locking
4. **`std::fs::File`** - filesystem I/O throughout CompressedHandle
5. **`std::os::unix::fs::FileExt`** - pread/pwrite extensions
6. **`parking_lot`** - works on WASM but the Mutex usage patterns are problematic
7. **`sqlite-vfs` crate** - assumes native SQLite C library linked in

### Assessment:
Direct compilation of turbolite to WASM is **not feasible** without major refactoring.
The entire I/O layer (file reads, file locks, atomic writes) is deeply tied to OS primitives.
The tiered module additionally depends on AWS SDK (tokio, async runtime).

## Attempt 2: Hybrid Architecture (Successful)

Architecture that works:
1. **sql.js** (SQLite compiled to WASM via Emscripten) - handles SQL parsing and execution
2. **JavaScript VFS layer** - HTTP range requests + page assembly
3. **fzstd** (JS zstd decompressor) - decompresses turbolite pages in the browser
4. **Pre-built index files** - JSON index mapping page numbers to file offsets (avoids full scan)

### How it works:
- Static server hosts SQLite/.turbolite files with HTTP Range support
- Browser fetches index JSON, then uses Range requests in batches
- For turbolite: fetches compressed pages, decompresses with fzstd, assembles into buffer
- sql.js opens the assembled buffer as a regular SQLite database
- Queries execute entirely in WASM (no server round-trips after initial load)

### Key discovery: page indexing
- Turbolite uses 0-indexed pages, SQLite uses 1-indexed
- For plain SQLite, pages are at deterministic offsets: (pageNum-1) * pageSize
- For turbolite, we need a pre-built index since pages are variable-length compressed

## Benchmark Results

### Database sizes
| Name | Rows (users) | SQLite size | Turbolite size | Compression ratio |
|------|-------------|-------------|----------------|-------------------|
| tiny | 100 | 68 KB | 17 KB | 4.0x |
| small | 1,000 | 316 KB | 110 KB | 2.9x |
| medium | 10,000 | 2.7 MB | 958 KB | 2.9x |
| large | 100,000 | 28.1 MB | 10.1 MB | 2.8x |
| xlarge | 500,000 | 143 MB | 51 MB | 2.8x |

### Open times (loading DB into WASM memory)
| Size | SQLite Full | SQLite Range | Turbolite Full | Turbolite Range |
|------|------------|-------------|----------------|-----------------|
| tiny | 11ms | 7ms | 68ms | 57ms |
| small | 10ms | 14ms | 271ms | 264ms |
| medium | 20ms | 51ms | 1,864ms | 1,899ms |
| large | 89ms | 482ms | 19,793ms | 20,804ms |

### Key observations:
1. **Turbolite decompression dominates open time** - fzstd JS decompression is ~10-20x slower than native
2. **Query times are identical** once loaded - because both produce the same in-memory SQLite
3. **Transfer size savings are real** - turbolite transfers ~3x less data
4. **Range requests add minimal overhead** for small DBs but help for large ones
5. The range request mode for turbolite is actually worse because it decompresses page-by-page instead of streaming

### Query performance (same for both formats after loading)
| Query | Tiny | Small | Medium | Large |
|-------|------|-------|--------|-------|
| Point lookup | 0.5ms | 0.2ms | 0.1ms | 0.1ms |
| Filter (indexed) | 0.3ms | 0.2ms | 0.2ms | 0.2ms |
| Aggregation | 0.4ms | 0.8ms | 4.3ms | 72.9ms |
| Join | 0.3ms | 0.2ms | 0.1ms | 0.2ms |
| Complex join+agg | 0.5ms | 2.9ms | 23.0ms | 315ms |
| Count | 0.1ms | 0.1ms | 0.1ms | 1.0ms |

## Conclusions

1. **Direct WASM compilation of turbolite: not practical** - too many OS dependencies
2. **Hybrid approach works well** - sql.js + JS decompression + HTTP range requests
3. **Main bottleneck is decompression** - a WASM zstd decompressor would be much faster
4. **For bandwidth-constrained scenarios**, turbolite compression is valuable (3x smaller transfers)
5. **For latency-sensitive scenarios**, plain SQLite is better (no decompression overhead)
6. **True lazy page loading** would require patching sql.js's VFS layer (currently need full buffer)

## Ideas for Improvement
- Compile zstd to WASM for faster decompression (would eliminate the 10-20x overhead)
- Implement true lazy VFS in sql.js (only fetch pages as SQLite requests them)
- Use turbolite's page grouping format for more efficient range requests
- Web Worker for decompression to avoid blocking UI
- Cache decompressed pages in IndexedDB for repeat visits
