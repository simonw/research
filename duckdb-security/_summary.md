Evaluating DuckDB’s sandboxing features for secure untrusted query execution, this project demonstrates how to configure read-only access, restrict file and network operations, and enforce query timeouts in Python environments. Native settings like `read_only`, `enable_external_access`, and `allowed_paths` effectively limit users to preapproved data sources, while locking configuration via `lock_configuration=true` ensures that these controls cannot be altered by malicious queries. Since DuckDB does not offer built-in query timeouts, a thread-based workaround using `connection.interrupt()` is verified and recommended. An integrated wrapper, [`sandboxed_duckdb.py`](sandboxed_duckdb.py), encapsulates these protections, serving as a template for running untrusted code safely—further supporting async use cases through [aioduckdb](https://pypi.org/project/aioduckdb/).

**Key findings:**
- DuckDB provides direct support for file/network allowlists, read-only operation, and config locking for robust isolation.
- Limiting external access and predefining allowed files prevents unauthorized data access.
- Resource limits and manual timeouts via connection interruption mitigate denial-of-service and runaway queries.
- No native async API, but third-party solutions like `aioduckdb` exist.
