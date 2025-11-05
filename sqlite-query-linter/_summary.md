The SQLite Query Linter is a lightweight Python library that wraps the standard `sqlite3` module to provide configurable linting and rule-based analysis of SQL queries before execution. Acting as a drop-in replacement, it helps catch common syntax errors and platform incompatibilities—such as invalid types in `CAST`, use of unsupported functions, `SELECT *`, missing `WHERE` clauses, and string quoting mistakes—helping developers avoid runtime errors and improve code quality. Users can choose built-in rules, set severity levels, and easily define custom rules via an extensible API. Designed for flexibility, it can block execution on critical issues or run in permissive/audit-only modes, with zero dependencies other than Python's standard library. Explore code and integration options at [GitHub](https://github.com/yourusername/sqlite-query-linter) or view usage in the included [`demo.py`](demo.py) script.

Key Features & Findings:
- Detects SQL mistakes commonly encountered when migrating between databases or writing raw SQLite queries
- Flexible configuration: Enable/disable rules, set strictness, and use audit-only monitoring
- Easy to extend for custom organizational or project rules
- Applicable to development, automated testing, database migrations, and production monitoring
