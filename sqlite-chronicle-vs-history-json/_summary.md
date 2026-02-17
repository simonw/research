Using both [sqlite-chronicle](https://github.com/simonw/sqlite-chronicle) and [sqlite-history-json](https://github.com/simonw/sqlite-history-json) on the same SQLite table is feasible, as each library installs its own set of triggers and companion tables without interfering with standard CRUD operations. Chronicle focuses on efficient sync/versioning, while history-json offers a complete audit log, and both operate independently even with compound primary keys or concurrent audit groups. One major pitfall occurs when using `restore(swap=True)` from history-json, which deletes all triggers—including chronicle’s—requiring manual re-enabling to resume tracking. Performance overhead for using both is roughly additive (~2.3x), and behaviors like no-op update detection and handling of INSERT OR REPLACE differ between the libraries.

Key findings:
- Enabling order does not matter; both track changes independently.
- `restore(swap=True)` wipes all triggers—must manually re-enable them after.
- Chronicle provides smarter no-op detection; history-json records every operation.
- Handling of INSERT OR REPLACE depends on SQLite’s `recursive_triggers` setting; history-json may miss implicit deletes unless this is ON.
- Each library can be safely disabled/re-enabled without affecting the other’s tracking.
