A proof-of-concept implements a fully SQLite-based hierarchical permission system that computes allowed database/table pairs by cascading rules across child (table), parent (database), and global levels with DENY-over-ALLOW semantics; it uses only plain SQL (CTEs + SQLite JSON functions) and is built on SQLite (https://sqlite.org). Actor and token inputs are JSON-parsed inside the query so a single CTE-based SQL statement resolves per-resource decisions (child → parent → global) and then intersects results with optional token scope, ensuring tokens can only restrict, not grant, access; behavior is validated with a pytest test suite (https://pytest.org). The demo includes a minimal schema, multiple simulated “hook” rule sources, example data, and 11 test scenarios that show child-level ALLOW overriding parent DENY, child-level DENY blocking parent ALLOW, default-deny behavior, and token intersection semantics.

Key findings:
- Pure-SQL implementation (no UDFs/extensions) using CTEs and sqlite JSON helpers.
- Cascading precedence: child > parent > global; at the same level DENY beats ALLOW.
- Token scoping applied via INTERSECT; tokens cannot elevate permissions.
- Single-query engine returns final db/table pairs; schema and tests are compact and extensible.
- 11 pytest scenarios confirm intended conflict-resolution rules and edge cases.
