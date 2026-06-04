Demonstrating robust regex performance, this project offers a minimal Python ctypes binding to the [TRE regex library](https://github.com/laurikari/tre/), highlighting TRE’s immunity to regular expression denial-of-service (ReDoS) attacks that cripple Python's built-in `re` module. Key benchmarks show that TRE processes even notorious "evil" patterns on gigantic inputs (10 million characters) much faster than `re` on tiny ones, and scales linearly with input size instead of exponentially. The binding exposes compile and search functionality, includes rigorous ReDoS and scaling tests, and ensures bounded memory and reliable match-time behavior—with optional thread-based wall-clock timeouts. Limitations include the exclusion of back-references (a deliberate design tied to linear-time guarantees) and focus on core use-cases. The full test suite, benchmarks, and reproducible build scripts are provided, with results confirming linear performance and resilience against algorithmic blowup.

Key findings:
- TRE is robust against ReDoS patterns; Python `re` is not.
- Runtime with TRE grows linearly with input length, not exponentially.
- Memory usage during matching stays constant—even with enormous input.
- All tests (22 in total) pass quickly, confirming the claims.
- Project exposes a precise, safe API without exposing unstable features.

Relevant tools:
- [TRE regex library](https://github.com/laurikari/tre/)
- Project structure and tests: see `src/tre_py`, `tests/`, and `benchmark.py`
