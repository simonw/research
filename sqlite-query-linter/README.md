# SQLite Query Linter

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

A Python library that wraps `sqlite3` to provide configurable linting rules for SQL queries. Catch common SQLite mistakes before they cause runtime errors.

## Features

- **Drop-in replacement for sqlite3**: Minimal code changes required
- **Configurable rules**: Enable/disable rules or create custom ones
- **Sensible defaults**: Catches common SQLite pitfalls out of the box
- **Flexible severity levels**: ERROR (blocks execution), WARNING (notifies), INFO
- **Multiple modes**: Strict mode, permissive mode, or audit-only mode
- **Zero dependencies**: Only requires Python standard library (and pytest for testing)

## Installation

Simply copy `sqlite_linter.py` to your project:

```bash
cp sqlite_linter.py /path/to/your/project/
```

Or install in development mode:

```bash
pip install -e .
```

## Quick Start

```python
import sqlite_linter

# Create a linting connection (drop-in replacement for sqlite3.connect)
conn = sqlite_linter.connect(":memory:")

# Create table and insert data
conn.execute("CREATE TABLE products (id INTEGER, name TEXT, price REAL)")
conn.execute("INSERT INTO products VALUES (1, 'Widget', 9.99)")

# This works fine
cursor = conn.execute("SELECT name, price FROM products")
print(cursor.fetchall())

# This raises LintError - STRING is not a valid SQLite type!
try:
    conn.execute("SELECT CAST(price AS STRING) FROM products")
except sqlite_linter.LintError as e:
    print(f"Error: {e}")
    # Error: Invalid type 'STRING' in CAST. Did you mean 'TEXT'?
```

## Built-in Linting Rules

### 1. InvalidCastTypeRule (ERROR)

Catches invalid type names in CAST expressions.

**Problem:**
```sql
SELECT CAST(price AS STRING) FROM products  -- ✗ STRING is invalid
SELECT CAST(active AS BOOL) FROM users      -- ✗ BOOL is invalid
```

**Solution:**
```sql
SELECT CAST(price AS TEXT) FROM products    -- ✓ TEXT is correct
SELECT CAST(active AS INTEGER) FROM users   -- ✓ INTEGER is correct
```

**Valid SQLite types:** TEXT, INTEGER, REAL, BLOB, NUMERIC (and their variants)

### 2. InvalidFunctionRule (ERROR)

Detects SQL functions from other databases that don't exist in SQLite.

**Problem:**
```sql
SELECT CONCAT(first, last) FROM users  -- ✗ CONCAT doesn't exist
SELECT LEN(name) FROM products         -- ✗ Use LENGTH instead
SELECT GETDATE()                       -- ✗ Use CURRENT_TIMESTAMP
```

**Solution:**
```sql
SELECT first || ' ' || last FROM users  -- ✓ Use || operator
SELECT LENGTH(name) FROM products       -- ✓ Correct function name
SELECT CURRENT_TIMESTAMP                -- ✓ SQLite standard
```

### 3. SelectStarRule (WARNING)

Warns about `SELECT *` queries which can be inefficient and fragile.

```sql
SELECT * FROM users  -- ⚠ Warning: Consider explicit columns
SELECT id, name, email FROM users  -- ✓ Better
```

### 4. MissingWhereClauseRule (WARNING)

Warns about UPDATE/DELETE without WHERE clauses.

```sql
DELETE FROM users         -- ⚠ Warning: Will delete ALL rows
UPDATE users SET active=0 -- ⚠ Warning: Will update ALL rows

DELETE FROM users WHERE id = 5              -- ✓ Safe
UPDATE users SET active=0 WHERE id > 100    -- ✓ Safe
DELETE FROM users WHERE 1=1                 -- ✓ Explicit "all rows"
```

### 5. DoubleQuotesForStringsRule (WARNING)

Reminds you that double quotes are for identifiers, single quotes for strings.

```sql
SELECT * FROM users WHERE name = "John"   -- ⚠ Should use single quotes
SELECT * FROM users WHERE name = 'John'   -- ✓ Correct
SELECT "user name" FROM users             -- ✓ OK for identifiers with spaces
```

## Configuration

### Custom Rule Selection

Choose which rules to apply:

```python
from sqlite_linter import InvalidCastTypeRule, InvalidFunctionRule

# Only check for type and function errors
rules = [InvalidCastTypeRule(), InvalidFunctionRule()]
conn = sqlite_linter.connect(":memory:", rules=rules)
```

### Severity Levels

Control how strict the linting should be:

```python
# Default: Block on errors, allow warnings
conn = sqlite_linter.connect(":memory:")

# Strict mode: Block on both errors and warnings
conn = sqlite_linter.connect(":memory:",
                             raise_on_error=True,
                             raise_on_warning=True)

# Permissive mode: Never block, just track issues
conn = sqlite_linter.connect(":memory:",
                             raise_on_error=False,
                             raise_on_warning=False)

cursor = conn.cursor()
cursor.execute("SELECT * FROM users")
# Check what was found:
for issue in cursor.last_issues:
    print(f"{issue.level}: {issue.message}")
```

## Creating Custom Rules

Extend the `LintRule` base class:

```python
from sqlite_linter import LintRule, LintLevel, LintIssue
import re

class NoSelectCountStarRule(LintRule):
    """Warn about COUNT(*) - prefer COUNT(column_name)"""

    def __init__(self):
        super().__init__("no_count_star", LintLevel.WARNING)

    def check(self, query: str) -> list[LintIssue]:
        if re.search(r'COUNT\s*\(\s*\*\s*\)', query, re.IGNORECASE):
            return [LintIssue(
                level=self.level,
                rule_name=self.name,
                message="COUNT(*) can be slow. Consider COUNT(column_name) instead.",
                query=query
            )]
        return []

# Use your custom rule
rules = sqlite_linter.LintingConnection._default_rules()
rules.append(NoSelectCountStarRule())
conn = sqlite_linter.connect(":memory:", rules=rules)
```

## API Reference

### Functions

**`sqlite_linter.connect(database, rules=None, raise_on_error=True, raise_on_warning=False, **kwargs)`**

Creates a linting SQLite connection.

- `database`: Database file path or ":memory:"
- `rules`: List of LintRule objects (uses defaults if None)
- `raise_on_error`: Raise LintError on ERROR-level issues (default: True)
- `raise_on_warning`: Raise LintWarning on WARNING-level issues (default: False)
- `**kwargs`: Passed to sqlite3.connect()

Returns: `LintingConnection` instance

### Classes

**`LintingConnection`**

Wrapper around `sqlite3.Connection` that creates linting cursors.

Methods:
- `cursor()`: Create a LintingCursor
- `execute(sql, parameters=())`: Execute query with linting
- `executemany(sql, seq_of_parameters)`: Execute multiple times
- `executescript(sql_script)`: Execute SQL script
- All other `sqlite3.Connection` methods are proxied

**`LintingCursor`**

Wrapper around `sqlite3.Cursor` that lints queries before execution.

Properties:
- `last_issues`: List of LintIssue objects from the last query

Methods:
- Same as `sqlite3.Cursor`

**`LintRule`**

Base class for creating custom linting rules.

Methods to implement:
- `check(query: str) -> list[LintIssue]`: Check query for issues

**`LintIssue`**

Represents a linting issue.

Properties:
- `level`: LintLevel (ERROR, WARNING, INFO)
- `rule_name`: Name of the rule that found this issue
- `message`: Human-readable description
- `query`: The SQL query that has the issue
- `position`: Optional character position in query

### Exceptions

**`LintError`**

Raised when a query fails ERROR-level linting (if `raise_on_error=True`).

Properties:
- `issues`: List of LintIssue objects

**`LintWarning`**

Raised when a query fails WARNING-level linting (if `raise_on_warning=True`).

Properties:
- `issues`: List of LintIssue objects

## Use Cases

### Development Mode

Catch mistakes early during development:

```python
conn = sqlite_linter.connect("app.db")
# Strict checking helps catch bugs
```

### Testing

Ensure test queries use proper SQLite syntax:

```python
def test_user_query():
    conn = sqlite_linter.connect(":memory:")
    # Any bad SQL will fail the test
    conn.execute("SELECT id, name FROM users WHERE active = 1")
```

### Code Migration

Find incompatible SQL when migrating from other databases:

```python
# Audit mode - find all issues without blocking
conn = sqlite_linter.connect("legacy.db",
                             raise_on_error=False,
                             raise_on_warning=False)

cursor = conn.cursor()
for query in legacy_queries:
    cursor.execute(query)
    if cursor.last_issues:
        print(f"Query needs fixing: {query}")
        for issue in cursor.last_issues:
            print(f"  - {issue.message}")
```

### Production Monitoring

Track query quality in production (non-blocking):

```python
import logging

conn = sqlite_linter.connect("prod.db",
                             raise_on_error=False,
                             raise_on_warning=False)

cursor = conn.cursor()
cursor.execute(user_query)

for issue in cursor.last_issues:
    if issue.level == LintLevel.ERROR:
        logging.error(f"Bad query: {issue.message}")
    elif issue.level == LintLevel.WARNING:
        logging.warning(f"Query warning: {issue.message}")
```

## Running Tests

```bash
pytest test_sqlite_linter.py -v
```

All 37 tests should pass:

```
============================== 37 passed in 0.25s ===============================
```

## Running the Demo

See the library in action:

```bash
python demo.py
```

## Implementation Notes

- **Parser-free**: Uses regex pattern matching for simplicity and speed
- **Non-intrusive**: Wraps sqlite3 without modifying queries or results
- **Lazy evaluation**: Rules only run when queries are executed
- **Stateless rules**: Each rule is independent and reusable
- **Thread-safe**: Each connection has its own rule set

## Limitations

- Regex-based parsing may miss some edge cases (a full SQL parser would be more accurate)
- Dynamic SQL built at runtime can't be fully analyzed
- Comments within SQL may confuse some rules
- Multi-statement scripts are checked as a single unit

## Future Enhancements

Possible additions:

- Index usage recommendations
- Query performance hints
- Schema validation rules
- SQL injection pattern detection
- More comprehensive type checking
- Integration with SQL formatters
- VSCode/IDE integration

## License

MIT License - feel free to use in your projects!

## Contributing

To add a new rule:

1. Create a class inheriting from `LintRule`
2. Implement the `check(query)` method
3. Add tests in `test_sqlite_linter.py`
4. Add to default rules in `_default_rules()` if appropriate

## Example Projects

This library is particularly useful for:

- Web applications using SQLite
- Data science projects with SQLite databases
- Testing frameworks that verify SQL correctness
- Database migration tools
- Educational projects teaching SQL
- Code quality tools and linters

## Credits

Created as a demonstration of configurable SQL linting for Python's sqlite3 module.
