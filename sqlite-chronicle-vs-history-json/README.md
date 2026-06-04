# sqlite-chronicle + sqlite-history-json: Same Table Investigation

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Summary

**Can you use both [sqlite-chronicle](https://github.com/simonw/sqlite-chronicle) and [sqlite-history-json](https://github.com/simonw/sqlite-history-json) on the same SQLite table?**

**Yes.** Both libraries coexist cleanly for standard operations. They create separate triggers with distinct naming conventions (7 total per table) and separate companion tables. Enable order doesn't matter. There is one critical gotcha to be aware of: `restore(swap=True)` destroys all triggers.

## What Each Library Does

| | sqlite-chronicle (v0.6.1) | sqlite-history-json (v0.4) |
|---|---|---|
| **Purpose** | Sync/replication (what changed since version N?) | Full audit log (complete operation history) |
| **Companion table** | `_chronicle_{table}` | `_history_json_{table}` |
| **Shared table** | `_chroniclesnapshots` | `_history_json` (change groups) |
| **Triggers** | 4: BEFORE INSERT, AFTER INSERT/UPDATE/DELETE | 3: AFTER INSERT/UPDATE/DELETE |
| **Data model** | One row per PK with version + timestamps | One row per operation with JSON values |
| **Deleted rows** | Marked `__deleted=1`, kept in table | Recorded as "delete" operation entry |

## Key Findings

### 1. Basic Operations Work Perfectly Together

INSERT, UPDATE, and DELETE all trigger both libraries' tracking correctly. Each maintains its own companion table independently:

- Chronicle bumps version numbers and timestamps
- History-json appends operation entries with JSON values
- No interference between the two systems

### 2. No-op Detection Differs

| Scenario | Chronicle | History-json |
|---|---|---|
| UPDATE with no actual change | Skips (WHEN clause) | Records update with `{}` values |
| INSERT OR REPLACE, identical data | Skips (snapshot comparison) | Records new insert entry |

Chronicle is smarter about detecting no-ops. History-json is more conservative — it records everything.

### 3. INSERT OR REPLACE Behavior Depends on `recursive_triggers`

This is a subtle SQLite behavior: by default (`recursive_triggers = OFF`), the implicit DELETE during INSERT OR REPLACE **does not fire DELETE triggers**.

| Setting | History-json sees | Chronicle |
|---|---|---|
| `recursive_triggers = OFF` (default) | insert, insert | Correct (uses BEFORE INSERT snapshot) |
| `recursive_triggers = ON` | insert, delete, insert | Correct (AFTER DELETE suppressed by snapshot) |

Chronicle handles both cases correctly thanks to its BEFORE INSERT snapshot mechanism. History-json's recording depends on the pragma setting — with it OFF, you won't see the delete, only two inserts.

### 4. CRITICAL: `restore(swap=True)` Destroys All Triggers

When `sqlite_history_json.restore(swap=True)` atomically replaces the original table, it drops and renames, which **destroys all triggers** — including chronicle's triggers and history-json's own triggers.

After a swap:
- `is_chronicle_enabled()` returns `True` (misleading — it checks for the companion table, not triggers)
- Neither library is actually tracking changes
- Operations proceed silently without any tracking

**Fix:**
```python
sqlite_history_json.restore(db, "items", up_to_id=N, swap=True)
# Both libraries' triggers are now gone!

# Must re-create triggers:
sqlite_chronicle.disable_chronicle(db, "items")  # clear stale state
sqlite_chronicle.enable_chronicle(db, "items")    # recreate triggers
sqlite_history_json.enable_tracking(db, "items", populate_table=False)
```

### 5. Disable/Re-enable Works Cleanly

You can independently disable either library while the other continues working:

- Disabling chronicle removes its 4 triggers + companion table; history-json unaffected
- Disabling history-json removes its 3 triggers; audit data preserved; chronicle unaffected
- Re-enabling either one restores tracking without conflicts

### 6. Performance Overhead Is Roughly Additive

Benchmark: 1000 each of INSERT, UPDATE, DELETE operations:

| Configuration | Time | Overhead |
|---|---|---|
| No tracking | 24ms | 1.0x |
| Chronicle only | 37ms | 1.5x |
| History-json only | 41ms | 1.7x |
| Both libraries | 55ms | 2.3x |

The overhead of running both is roughly the sum of running each individually.

### 7. Compound Primary Keys Supported

Both libraries handle compound primary keys correctly:
- Chronicle mirrors the composite PK in its companion table
- History-json creates `pk_`-prefixed columns (e.g., `pk_user_id`, `pk_role_id`)

### 8. Change Groups Span Libraries

History-json's `change_group()` context manager works across multiple tables, and chronicle independently tracks the version bumps for each change within the group.

## Table/Trigger Inventory

For a single tracked table `items`, the database contains:

**Tables (5):**
- `items` (source)
- `_chronicle_items` (chronicle companion)
- `_chroniclesnapshots` (chronicle internal, shared)
- `_history_json_items` (history-json audit log)
- `_history_json` (history-json groups, shared)

**Triggers (7):**
- `chronicle_items_bi` — BEFORE INSERT (snapshot for INSERT OR REPLACE)
- `chronicle_items_ai` — AFTER INSERT (create/update chronicle entry)
- `chronicle_items_au` — AFTER UPDATE (bump version if data changed)
- `chronicle_items_ad` — AFTER DELETE (mark deleted)
- `_history_json_items_insert` — AFTER INSERT (record operation)
- `_history_json_items_update` — AFTER UPDATE (record changed values)
- `_history_json_items_delete` — AFTER DELETE (record deletion)

## Recommendations

1. **Safe to use both together** for standard CRUD operations
2. **After `restore(swap=True)`**, always re-enable both libraries' triggers
3. **Consider the `recursive_triggers` pragma** if you rely on history-json's audit log for INSERT OR REPLACE operations — the default OFF setting means deletes during REPLACE are invisible
4. **Performance is acceptable** — ~2.3x overhead for both vs no tracking
5. **Chronicle is better at detecting no-ops** — if this matters for your sync workload, it's a point in its favor

## Files

- `demo.md` — Executable showboat document with all tests and output
- `explore_chronicle.py` — Standalone chronicle exploration
- `explore_history_json.py` — Standalone history-json exploration
- `test_both_chronicle_first.py` — Both libraries, chronicle enabled first
- `test_both_history_first.py` — Both libraries, history-json enabled first
- `test_edge_cases.py` — Edge cases: INSERT OR REPLACE, delete+reinsert, NULLs, no-ops
- `test_compound_keys.py` — Compound primary key support
- `test_api_interop.py` — Python API interop: updates_since, restore, change_group
- `test_disable_reenable.py` — Disable/re-enable one while other is active
- `test_restore_swap.py` — restore(swap=True) initial discovery
- `test_restore_swap_detail.py` — Detailed proof that swap destroys triggers
- `test_swap_fix.py` — Correct recovery procedure after swap
- `test_recursive_triggers.py` — INSERT OR REPLACE with recursive_triggers ON vs OFF
- `test_bulk_performance.py` — Performance benchmark
- `test_multiple_tables.py` — Multiple tables with shared change groups
