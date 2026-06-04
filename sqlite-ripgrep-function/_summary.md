SQLite Ripgrep Function enables fast code and text search inside SQLite queries by integrating the powerful [ripgrep](https://github.com/BurntSushi/ripgrep) search tool as a custom SQL function. It offers both a pure Python implementation and a performant C extension, allowing users to search files within a configurable directory, restrict output with glob patterns (e.g., `*.py`), and enforce time limits to avoid runaway queries. While the Python version returns JSON for lightweight use, the C extension provides true table-valued virtual tables for flexible SQL integration, supporting constraints and column selection directly in queries. This project draws inspiration from [datasette-ripgrep](https://github.com/simonw/datasette-ripgrep) and is installable in both Python and SQLite environments.

**Key features:**
- Direct code/text search from SQL using ripgrep
- Configurable base directory and file filtering via glob patterns
- Time limit enforcement to prevent slow queries
- Both JSON (Python) and table-valued (C extension) results suitable for further SQL querying
- Easy integration with both Python and SQLite CLI environments
