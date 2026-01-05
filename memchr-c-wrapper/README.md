# pymemchr-c: C Implementation of memchr Library

A Python C extension implementing the same API as the Rust-based [pymemchr](https://github.com/BurntSushi/memchr) library, providing high-performance byte and substring search functions.

## Overview

This project reimplements the functionality of the BurntSushi/memchr Rust library in pure C with SIMD optimizations. It provides Python bindings through the Python C API, offering a Rust-toolchain-free alternative for projects that need fast byte search operations.

## Features

- **Single byte search**: `memchr`, `memchr2`, `memchr3` and their reverse variants
- **Iterator variants**: Find all occurrences of bytes in a haystack
- **Substring search**: `memmem_find`, `memmem_rfind`, `memmem_find_iter`
- **Precompiled searchers**: `Finder` and `FinderRev` classes for repeated searches
- **SIMD optimizations**: Uses SSE2 on x86_64 and NEON on ARM64 where available

## Implementation Details

The C implementation uses:
- **glibc's optimized memchr/memrchr** for single byte search
- **Custom SSE2/NEON SIMD code** for multi-byte search (memchr2, memchr3)
- **glibc's memmem** for substring search (with Boyer-Moore-Horspool fallback)
- **Python C API** for Python bindings
- **setuptools** for building (compatible with uv, pip)

## Benchmark Results

Benchmarks were run comparing the C implementation (pymemchr_c), the Rust implementation (pymemchr), and native Python methods on 1MB data:

### Speedup vs Python (higher is better)

| Operation | C vs Python | Rust vs Python |
|-----------|-------------|----------------|
| memchr (first byte) | 1.37x | 1.27x |
| memrchr (last byte) | 1.37x | 1.55x |
| memchr2 (first of 2) | 2.40x | 2.43x |
| memchr3 (first of 3) | 4.74x | 5.60x |
| memmem (short needle) | 1.95x | 19.45x |
| memmem (medium needle) | 4.64x | 9.36x |
| memmem (long needle) | 4.01x | 5.06x |
| memchr_iter (all bytes) | 4.23x | 4.51x |
| memmem_find_iter | 0.95x | 19.30x |

### Benchmark Charts

![Single Byte Search - First Occurrence](benchmark_single_byte_first.png)
![Single Byte Search - Last Occurrence](benchmark_single_byte_last.png)
![Multi-Byte Search](benchmark_multi_byte.png)
![Substring Search](benchmark_substring.png)
![Iterator Benchmarks](benchmark_iter.png)
![Speedup Summary](benchmark_speedup.png)

## Key Findings

### 1. Single Byte Search (memchr, memrchr)
Both C and Rust implementations perform similarly, with modest speedups (~1.3-1.5x) over Python's native `bytes.find()` and `bytes.rfind()`. This is because glibc's memchr is already highly optimized with SIMD.

### 2. Multi-Byte Search (memchr2, memchr3)
Significant speedups (~2.5-5x) over Python. Python has to make multiple separate `find()` calls, while the C/Rust implementations use a single SIMD-optimized pass.

### 3. Substring Search (memmem)
**This is where Rust significantly outperforms C:**
- Rust: 10-20x faster than Python for short needles
- C: 2-4x faster than Python

The Rust memchr library uses advanced Two-Way algorithm with SIMD prefiltering, while our C implementation relies on glibc's memmem which is good but not as optimized.

### 4. Iterator Functions
Both implementations are ~4x faster than Python for finding all byte occurrences. For substring iteration, Rust maintains its advantage due to the superior substring search algorithm.

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

## Future Improvements

1. Implement Two-Way algorithm with SIMD prefiltering for substring search to match Rust's performance
2. Add AVX2 support for wider SIMD operations on modern x86_64 CPUs
3. Runtime CPU feature detection for optimal code path selection

## License

MIT
