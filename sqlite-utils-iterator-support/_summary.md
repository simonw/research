Enhancements to the [sqlite-utils](https://github.com/simonw/sqlite-utils) library now allow its `insert_all` and `upsert_all` methods to efficiently process Python iterators yielding lists, in addition to the original dict-based input. Detection of the iterator type is automatic and maintains full backward compatibility, streamlining bulk inserts from row-based data sources like CSV streams and reducing memory usage by avoiding dict construction. Performance benchmarks show list mode delivers up to 21.6% speed improvement for datasets with few columns, though gains diminish or reverse with wider tables. All 1001 existing tests pass, alongside 10 new tests for list mode, confirming robust and production-ready implementation.

**Key findings:**
- List mode is up to 21.6% faster for typical (5-10 column) datasets; dict mode regains advantage for 15+ columns.
- Memory usage drops for list mode due to lack of dict overhead.
- No breaking changes or new dependencies introduced; backwards compatibility is ensured.
- [sqlite-utils-list-mode.diff](sqlite-utils-list-mode.diff) provides the implementation patch.
