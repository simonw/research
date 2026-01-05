# memchr Python Wrapper Development Notes

## Goal
Build a Python package using PyO3 that wraps the BurntSushi/memchr Rust library.

## Steps
1. Clone memchr repository to /tmp
2. Explore the memchr API to understand what to wrap
3. Create PyO3-based Python package
4. Write tests
5. Build and verify wheel
6. Create benchmarks comparing with native Python
7. Generate charts and README report

## Progress

### 2026-01-05: Project Started
- Created project folder at /home/user/research/memchr-python-wrapper
- Cloned https://github.com/BurntSushi/memchr to /tmp/memchr

### Package Structure Setup
- Created pyproject.toml with maturin build system
- Created Cargo.toml with pyo3 and memchr dependencies
- Used PyO3 0.22 and memchr 2.7

### Key Implementation Decisions

1. **Naming Conflicts**: Had to use `#[pyo3(name = "memchr")]` attributes to rename Rust functions
   internally to avoid conflicts with the `memchr` crate name.

2. **Function Wrapping**: Wrapped the following memchr functions:
   - `memchr`, `memchr2`, `memchr3` - single byte search
   - `memrchr`, `memrchr2`, `memrchr3` - reverse single byte search
   - `memchr_iter`, `memchr2_iter`, `memchr3_iter` - find all byte occurrences
   - `memrchr_iter`, `memrchr2_iter`, `memrchr3_iter` - find all byte occurrences (reverse)
   - `memmem::find`, `memmem::rfind`, `memmem::find_iter` - substring search
   - `memmem::Finder`, `memmem::FinderRev` - precompiled finder classes

3. **Return Types**:
   - Single search functions return `Option<usize>` (Python `int | None`)
   - Iterator functions return `Vec<usize>` (Python `list[int]`)
   - `needle()` methods return `Vec<u8>` which becomes `list[int]` in Python

### Tests
- Created 38 test cases covering all functions
- Tests include edge cases: empty haystack, not found, binary data with null bytes
- Large input tests (1MB) to verify performance

### Build Results
- `uv sync` - builds editable install successfully
- `uv build` - produces both .tar.gz source distribution and .whl wheel
- All 38 tests pass with `uv run pytest`

### Benchmark Results (1MB data)
- **memchr**: 1.84x faster than Python bytes.find
- **memrchr**: 1.63x faster than Python bytes.rfind
- **memchr2**: 4.27x faster than Python equivalent
- **memchr3**: 5.88x faster than Python equivalent
- **memmem (short needle)**: 19.55x faster
- **memmem (medium needle)**: 9.43x faster
- **memmem (long needle)**: 5.33x faster
- **memchr_iter**: 4.49x faster
- **memmem_find_iter**: 19.36x faster

### Lessons Learned
1. PyO3 makes Rust-Python bindings straightforward
2. The memchr library's SIMD optimizations provide significant speedups
3. Substring search shows the biggest performance gains (up to 20x)
4. For single byte search, the gains are smaller but still noticeable
5. Name collisions between crate names and function names require care

### Files Created
- `pyproject.toml` - Python package configuration
- `Cargo.toml` - Rust package configuration
- `src/lib.rs` - Rust implementation with PyO3 bindings
- `python/pymemchr/__init__.py` - Python package entry point
- `tests/__init__.py` - Test package
- `tests/test_pymemchr.py` - Comprehensive test suite
- `benchmark.py` - Benchmark script
- `benchmark_*.png` - Benchmark result charts
- `README.md` - Project documentation
- `notes.md` - Development notes (this file)
