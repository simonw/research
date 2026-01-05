# memchr C Wrapper Development Notes

## Goal
Implement a C version of the memchr Rust library with Python bindings, matching the API of the Rust-based pymemchr library.

## Approach
1. Study the memchr Rust implementation in /tmp/memchr
2. Implement optimized C versions of:
   - memchr, memchr2, memchr3 (forward byte search)
   - memrchr, memrchr2, memrchr3 (reverse byte search)
   - Iterator variants that return all matches
   - memmem_find, memmem_rfind (substring search)
   - Finder and FinderRev classes for precompiled searches

3. Use Python C API for extension module
4. Use setuptools for building (compatible with uv)

## Implementation Details

### SIMD Optimization
The Rust memchr library uses SIMD (SSE2/AVX2 on x86_64, NEON on ARM).
For C, we'll use:
- Built-in memchr() for single byte search (glibc has optimized version)
- Custom SIMD code for multi-byte search (memchr2, memchr3)
- Two-Way algorithm or similar for memmem

### Functions to Implement
- memchr(needle, haystack) -> Optional[int]
- memchr2(n1, n2, haystack) -> Optional[int]
- memchr3(n1, n2, n3, haystack) -> Optional[int]
- memrchr(needle, haystack) -> Optional[int]
- memrchr2(n1, n2, haystack) -> Optional[int]
- memrchr3(n1, n2, n3, haystack) -> Optional[int]
- memchr_iter(needle, haystack) -> List[int]
- memchr2_iter(n1, n2, haystack) -> List[int]
- memchr3_iter(n1, n2, n3, haystack) -> List[int]
- memrchr_iter(needle, haystack) -> List[int]
- memrchr2_iter(n1, n2, haystack) -> List[int]
- memrchr3_iter(n1, n2, n3, haystack) -> List[int]
- memmem_find(needle, haystack) -> Optional[int]
- memmem_rfind(needle, haystack) -> Optional[int]
- memmem_find_iter(needle, haystack) -> List[int]
- Finder class with find(), find_iter(), needle()
- FinderRev class with rfind(), needle()

## Progress Log

### Session 1
- Created project structure
- Studied existing memchr-python-wrapper tests to understand the expected behavior
- Implemented C code with SSE2 SIMD optimizations for x86_64 and NEON for ARM64
- Created Python C API extension module
- Successfully built and tested - all 38 tests pass
- Built wheel with `uv build`
- Ran benchmarks comparing C, Rust, and Python implementations

### Session 2 - Performance Improvements
- Implemented Two-Way style algorithm with SIMD prefiltering for memmem
  - Uses "rare byte" detection to find optimal prefilter character
  - SIMD scans for both first byte and rare byte simultaneously
  - Only verifies full match on candidate positions
- Added AVX2 support (32 bytes at a time vs 16 for SSE2)
- Added runtime CPU feature detection using CPUID
- Significant performance improvements for substring search

### Session 3 - Beating Rust Implementation
- Implemented "Packed Pair" algorithm from Rust's memchr library
  - Searches for FIRST and LAST byte of needle simultaneously
  - Much more selective than rare byte - fixed offset provides implicit validation
  - Eliminates frequency table overhead
- Added aggressive loop unrolling (64 bytes per AVX2 iteration)
- Added software prefetching for better cache utilization
- Optimized memcmp to skip already-verified first/last bytes

**Result: C implementation now BEATS Rust for substring search!**

## Benchmark Results (1MB data) - Final

| Operation | C vs Python | Rust vs Python | C beats Rust? |
|-----------|-------------|----------------|---------------|
| memchr | 1.39x | 1.34x | ✓ Yes |
| memrchr | 1.42x | 1.62x | No |
| memchr2 | 2.60x | 2.54x | ✓ Yes |
| memchr3 | 4.14x | 5.63x | No |
| memmem (short) | **28.32x** | 19.33x | **✓ 1.47x faster** |
| memmem (medium) | **14.37x** | 8.75x | **✓ 1.64x faster** |
| memmem (long) | **7.42x** | 5.19x | **✓ 1.43x faster** |
| memchr_iter | 4.09x | 4.42x | No |
| memmem_iter | **28.25x** | 19.21x | **✓ 1.47x faster** |

### Progress Over Sessions

| Operation | Session 1 | Session 2 | Session 3 | Total Improvement |
|-----------|-----------|-----------|-----------|-------------------|
| memmem (short) | 1.95x | 7.91x | **28.32x** | **14.5x better** |
| memmem (medium) | 4.64x | 3.94x | **14.37x** | **3.1x better** |
| memmem (long) | - | 1.75x | **7.42x** | **4.2x better** |
| memmem_iter | 0.95x | 5.27x | **28.25x** | **29.7x better** |

## Key Findings

1. **Single byte search (memchr, memrchr)**: Both C and Rust are similar performance, slightly faster than Python's native bytes.find/rfind.

2. **Multi-byte search (memchr2, memchr3)**: Both C and Rust are much faster than Python (~2.5-5x faster). This is because Python has to do multiple separate find() calls.

3. **Substring search (memmem)**:
   - **C now beats Rust** by 1.4-1.6x for all needle sizes!
   - The "Packed Pair" algorithm is the key - searching for first+last byte simultaneously
   - Loop unrolling and prefetching provide additional gains

4. **Iterator functions**:
   - Single byte: Both C and Rust are ~4x faster than Python
   - Substring: **C beats Rust** by 1.47x due to faster underlying memmem

## Lessons Learned

1. For single byte search, glibc's memchr is already highly optimized and competitive with Rust's implementation.

2. For multi-byte search (memchr2, memchr3), SIMD provides significant speedup over Python's approach of multiple find() calls.

3. For substring search, the **Packed Pair algorithm** is crucial:
   - Searching for first+last byte simultaneously eliminates most false positives
   - Loop unrolling amortizes branch overhead
   - Software prefetching keeps the CPU fed with data

4. Python's C API is straightforward for creating extension modules, and setuptools works well with uv for building wheels.

5. **C can beat Rust** when you implement the same algorithmic techniques - it's not about the language, it's about the algorithm.

## Implementation Complete

All "Future Improvements" from Session 2 have been implemented:
- ✓ Better substring search algorithm (Packed Pair with SIMD prefiltering)
- ✓ AVX2 support for wider SIMD operations
- ✓ Runtime CPU feature detection
- ✓ **Bonus**: Now faster than Rust implementation!
