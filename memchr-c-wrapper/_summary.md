Offering a pure C reimplementation of the Rust-based [pymemchr](https://github.com/BurntSushi/memchr), pymemchr-c delivers high-performance byte and substring search functions to Python with extensive SIMD (SSE2/AVX2/NEON) optimizations and runtime CPU feature detection. Its unique "Packed Pair" substring search algorithm enables the C version to outperform both Python's built-in methods (up to 28x faster) and the original Rust extension (up to 1.5x faster for substring operations), all while removing the need for a Rust toolchain. The library provides a familiar API—including iterator and precompiled finder classes—and can be installed and built with standard Python tooling such as setuptools and [uv](https://github.com/astral-sh/uv). Benchmarks show major speedups for multi-byte and substring search tasks, making pymemchr-c an ideal choice for data-intensive byte and substring manipulation in Python.

**Key Findings:**
- C "Packed Pair" substring search makes pymemchr-c 1.4–1.6x faster than Rust, and up to ~29x faster than Python's native methods.
- Single-byte searches offer modest speedups (1.3–1.6x) over Python, thanks to highly optimized glibc routines.
- Multi-byte searches see 3–9x speedups over Python due to SIMD optimizations.
- Iterator functions and substring iterations are drastically faster in C than both Python and Rust implementations.
