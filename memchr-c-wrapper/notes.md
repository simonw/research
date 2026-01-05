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

## Benchmark Results (1MB data) - After Improvements

| Operation | C vs Python | Rust vs Python |
|-----------|-------------|----------------|
| memchr | 1.87x | 1.12x |
| memrchr | 1.37x | 1.65x |
| memchr2 | 2.57x | 2.31x |
| memchr3 | 4.84x | 5.53x |
| memmem (short) | **7.91x** | 19.54x |
| memmem (medium) | 3.94x | 9.02x |
| memmem (long) | 1.75x | 5.29x |
| memchr_iter | 4.18x | 4.46x |
| memmem_iter | **5.27x** | 19.47x |

### Previous Results (for comparison)

| Operation | C vs Python (before) | C vs Python (after) | Improvement |
|-----------|---------------------|---------------------|-------------|
| memmem (short) | 1.95x | 7.91x | **4.1x better** |
| memmem (medium) | 4.64x | 3.94x | similar |
| memmem_iter | 0.95x | 5.27x | **5.5x better** |

## Key Findings

1. **Single byte search (memchr, memrchr)**: Both C and Rust are similar performance, slightly faster than Python's native bytes.find/rfind.

2. **Multi-byte search (memchr2, memchr3)**: Both C and Rust are much faster than Python (~2.5-5x faster). This is because Python has to do multiple separate find() calls.

3. **Substring search (memmem)**:
   - Rust is significantly faster than C, especially for short needles
   - This is because the Rust memchr library uses advanced SIMD-based substring search algorithms (Two-Way with SIMD prefiltering)
   - Our C implementation uses glibc's memmem which is good but not as optimized

4. **Iterator functions**:
   - Both C and Rust are much faster than Python for single byte iteration (4x faster)
   - For substring iteration, Rust maintains its advantage due to better substring search algorithm

## Lessons Learned

1. For single byte search, glibc's memchr is already highly optimized and competitive with Rust's implementation.

2. For multi-byte search (memchr2, memchr3), SIMD provides significant speedup over Python's approach of multiple find() calls.

3. For substring search, the algorithm matters more than just SIMD - Rust's Two-Way algorithm with SIMD prefiltering is superior to simple Boyer-Moore-Horspool or glibc's memmem.

4. Python's C API is straightforward for creating extension modules, and setuptools works well with uv for building wheels.

## Future Improvements

1. Implement a better substring search algorithm (e.g., Two-Way with SIMD prefiltering) to match Rust's performance.

2. Add AVX2 support for wider SIMD operations on modern x86_64 CPUs.

3. Consider using runtime CPU feature detection like Rust does.
