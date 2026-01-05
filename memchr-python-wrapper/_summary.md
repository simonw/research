pymemchr is a Python library that provides ultra-fast byte and substring search functions by binding to the [memchr](https://github.com/BurntSushi/memchr) Rust crate, leveraging SIMD optimizations for superior performance. Using [PyO3](https://pyo3.rs/) and Maturin for cross-language integration, pymemchr offers efficient routines for finding single bytes, searching for multiple bytes, and locating substring patterns, both forwards and backwards, with highly competitive speedup over native Python methods. It is ideal for processing large data, repeated searches, and performance-critical applications, with precompiled searchers that minimize overhead for repeated queries. Benchmarks show particularly strong gains (up to 20x) in substring and multi-byte search tasks for large datasets.

**Key findings:**
- Single-byte search is up to 1.8x faster than native Python; multi-byte and substring operations can reach 4–20x speedup.
- Precompiled Finder classes enable rapid repeated searches.
- SIMD acceleration covers x86_64, aarch64, wasm32; falls back to scalar methods if unavailable.
- Most beneficial for big data or workloads requiring many search operations.
