# pymemchr-c: C Implementation of memchr Library

A Python C extension implementing the same API as the Rust-based [pymemchr](https://github.com/BurntSushi/memchr) library, providing high-performance byte and substring search functions.

## Overview

This project reimplements the functionality of the BurntSushi/memchr Rust library in pure C with SIMD optimizations. It provides Python bindings through the Python C API, offering a Rust-toolchain-free alternative for projects that need fast byte search operations.

## Features

- **Single byte search**: `memchr`, `memchr2`, `memchr3` and their reverse variants
- **Iterator variants**: Find all occurrences of bytes in a haystack
- **Substring search**: `memmem_find`, `memmem_rfind`, `memmem_find_iter`
- **Precompiled searchers**: `Finder` and `FinderRev` classes for repeated searches
- **SIMD optimizations**: Uses SSE2/AVX2 on x86_64 and NEON on ARM64
- **Runtime CPU detection**: Automatically uses AVX2 when available

## Implementation Details

The C implementation uses:
- **glibc's optimized memchr/memrchr** for single byte search
- **Custom SSE2/AVX2/NEON SIMD code** for multi-byte search (memchr2, memchr3)
- **SIMD-accelerated substring search** with Two-Way style prefiltering:
  - Uses "rare byte" detection to find optimal prefilter character
  - SIMD scans for first byte and rare byte simultaneously
  - AVX2 processes 32 bytes at a time, SSE2 processes 16 bytes
- **Runtime CPU feature detection** using CPUID on x86-64
- **Python C API** for Python bindings
- **setuptools** for building (compatible with uv, pip)

## Benchmark Results

Benchmarks were run comparing the C implementation (pymemchr_c), the Rust implementation (pymemchr), and native Python methods on 1MB data:

### Speedup vs Python (higher is better)

| Operation | C vs Python | Rust vs Python |
|-----------|-------------|----------------|
| memchr (first byte) | 1.87x | 1.12x |
| memrchr (last byte) | 1.37x | 1.65x |
| memchr2 (first of 2) | 2.57x | 2.31x |
| memchr3 (first of 3) | 4.84x | 5.53x |
| memmem (short needle) | **7.91x** | 19.54x |
| memmem (medium needle) | 3.94x | 9.02x |
| memmem (long needle) | 1.75x | 5.29x |
| memchr_iter (all bytes) | 4.18x | 4.46x |
| memmem_find_iter | **5.27x** | 19.47x |

### Benchmark Charts

![Single Byte Search - First Occurrence](benchmark_single_byte_first.png)
![Single Byte Search - Last Occurrence](benchmark_single_byte_last.png)
![Multi-Byte Search](benchmark_multi_byte.png)
![Substring Search](benchmark_substring.png)
![Iterator Benchmarks](benchmark_iter.png)
![Speedup Summary](benchmark_speedup.png)

## Key Findings

### 1. Single Byte Search (memchr, memrchr)
Both C and Rust implementations perform similarly, with modest speedups (~1.4-1.9x) over Python's native `bytes.find()` and `bytes.rfind()`. This is because glibc's memchr is already highly optimized with SIMD.

### 2. Multi-Byte Search (memchr2, memchr3)
Significant speedups (~2.5-5x) over Python. Python has to make multiple separate `find()` calls, while the C/Rust implementations use a single SIMD-optimized pass. AVX2 provides additional benefit by processing 32 bytes at a time.

### 3. Substring Search (memmem)
**With SIMD prefiltering, C now achieves significant speedups:**
- C: **7.9x faster** than Python for short needles (up from 1.95x)
- C: 3.9x faster for medium needles
- Rust: 19.5x faster for short needles (still faster due to more sophisticated algorithms)

The SIMD prefiltering approach uses a "rare byte" heuristic to quickly eliminate candidate positions before verification.

### 4. Iterator Functions
Both implementations are ~4x faster than Python for finding all byte occurrences. For substring iteration:
- C: **5.3x faster** than Python (up from 0.95x - was actually slower before!)
- Rust: 19.5x faster than Python

## Installation

```bash
# Build from source
uv build

# Install the wheel
pip install dist/pymemchr_c-*.whl

# Or install directly
pip install .
```

## Usage

```python
import pymemchr_c

# Find first occurrence of a byte
haystack = b"hello world"
result = pymemchr_c.memchr(ord('o'), haystack)  # Returns 4

# Find first occurrence of any of 2 bytes
result = pymemchr_c.memchr2(ord('l'), ord('o'), haystack)  # Returns 2

# Find all occurrences of a byte
result = pymemchr_c.memchr_iter(ord('l'), haystack)  # Returns [2, 3, 9]

# Substring search
result = pymemchr_c.memmem_find(b"world", haystack)  # Returns 6

# Find all substring occurrences
result = pymemchr_c.memmem_find_iter(b"l", haystack)  # Returns [2, 3, 9]

# Precompiled finder for repeated searches
finder = pymemchr_c.Finder(b"foo")
finder.find(b"foo bar baz")  # Returns 0
finder.find(b"bar foo baz")  # Returns 4
```

## API Reference

### Single Byte Search

- `memchr(needle, haystack)` - Find first occurrence of byte
- `memchr2(n1, n2, haystack)` - Find first of two bytes
- `memchr3(n1, n2, n3, haystack)` - Find first of three bytes
- `memrchr(needle, haystack)` - Find last occurrence of byte
- `memrchr2(n1, n2, haystack)` - Find last of two bytes
- `memrchr3(n1, n2, n3, haystack)` - Find last of three bytes

### Iterator Functions

- `memchr_iter(needle, haystack)` - Find all byte occurrences (forward)
- `memchr2_iter(n1, n2, haystack)` - Find all occurrences of two bytes
- `memchr3_iter(n1, n2, n3, haystack)` - Find all occurrences of three bytes
- `memrchr_iter(needle, haystack)` - Find all occurrences (reverse order)
- `memrchr2_iter(n1, n2, haystack)` - Find all of two bytes (reverse)
- `memrchr3_iter(n1, n2, n3, haystack)` - Find all of three bytes (reverse)

### Substring Search

- `memmem_find(needle, haystack)` - Find first substring
- `memmem_rfind(needle, haystack)` - Find last substring
- `memmem_find_iter(needle, haystack)` - Find all substrings (non-overlapping)

### Precompiled Searchers

- `Finder(needle)` - Create precompiled finder
  - `.find(haystack)` - Find first occurrence
  - `.find_iter(haystack)` - Find all occurrences
  - `.needle()` - Get the needle bytes as list
- `FinderRev(needle)` - Create precompiled reverse finder
  - `.rfind(haystack)` - Find last occurrence
  - `.needle()` - Get the needle bytes

## When to Use This vs Rust Implementation

**Use pymemchr_c (C) when:**
- You don't have Rust toolchain available
- You need single-byte or multi-byte search functions
- You prefer simpler build dependencies

**Use pymemchr (Rust) when:**
- You need fastest substring search performance
- You're doing heavy memmem_find_iter operations
- You already have Rust toolchain available

## Running Tests

```bash
uv run pytest -v
```

All 38 tests pass, matching the behavior of the Rust implementation.

## Running Benchmarks

```bash
uv pip install pymemchr  # Install Rust version for comparison
uv run python benchmark.py
```

## Recent Improvements (v0.1.0)

The following optimizations were implemented to significantly boost performance:

1. **SIMD prefiltering for substring search** - Uses "rare byte" detection and dual-byte SIMD scanning to quickly eliminate candidate positions
2. **AVX2 support** - Processes 32 bytes at a time on modern x86_64 CPUs (vs 16 bytes for SSE2)
3. **Runtime CPU feature detection** - Automatically detects and uses AVX2 when available via CPUID

These improvements resulted in:
- **4x better** performance for short needle substring search (1.95x → 7.91x vs Python)
- **5.5x better** performance for substring iteration (0.95x → 5.27x vs Python)

## Potential Future Improvements

1. More sophisticated Two-Way algorithm implementation for long needle patterns
2. AVX-512 support for even wider SIMD operations
3. Precomputed skip tables in Finder class for repeated searches

## License

MIT
