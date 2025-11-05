# SQLite Query Linter - Development Notes

## Project Goal
Build a Python library that wraps sqlite3 and provides configurable linting rules for SQL queries before execution.

## Initial Planning

### Architecture Design
- Wrapper around sqlite3 Connection and Cursor
- Rule-based system where each rule can analyze SQL queries
- Rules should be composable and configurable
- Should catch common SQLite mistakes before execution

### Example Rules to Implement
1. Invalid type names in CAST expressions (STRING vs TEXT, etc.)
2. Dangerous queries (SELECT * without WHERE on large tables)
3. Invalid SQLite keywords or syntax
4. Ambiguous column references
5. Missing indices warnings

### Development Steps
1. Create base linting framework
2. Implement rule system
3. Add default rules
4. Write tests
5. Document API

## Session Log

### Session Start
Starting implementation of sqlite-query-linter library...

### Architecture Decisions

1. **Wrapper Pattern**: Chose to wrap sqlite3.Connection and sqlite3.Cursor rather than subclass
   - Allows easy delegation of methods
   - Non-intrusive to existing sqlite3 behavior
   - Clean separation between linting and execution

2. **Rule-Based System**: Created LintRule base class
   - Each rule is independent and composable
   - Easy to add new rules
   - Configurable at connection time

3. **Severity Levels**: Implemented three levels (ERROR, WARNING, INFO)
   - ERROR: Should block execution (invalid SQL)
   - WARNING: May execute but not recommended
   - INFO: Informational messages
   - Configurable blocking behavior

4. **Regex vs Parser**: Used regex for pattern matching
   - Simpler implementation
   - Good enough for common cases
   - Faster than full SQL parser
   - May miss some edge cases but acceptable tradeoff

### Implementation Details

#### Core Classes

1. **LintRule**: Base class for all rules
   - `check(query)` method returns list of LintIssue objects
   - Each rule has a name and severity level

2. **LintIssue**: Data class for issues
   - Contains level, rule_name, message, query, optional position
   - Simple and clear representation

3. **LintingCursor**: Wraps sqlite3.Cursor
   - Intercepts execute(), executemany(), executescript()
   - Runs all rules before query execution
   - Tracks issues in `last_issues` attribute
   - Delegates all other methods to underlying cursor

4. **LintingConnection**: Wraps sqlite3.Connection
   - Creates LintingCursor instances
   - Manages rule configuration
   - Provides convenience execute methods
   - Delegates all other methods to underlying connection

#### Default Rules Implemented

1. **InvalidCastTypeRule**: Catches CAST(x AS STRING) and similar
   - Maps common mistakes: STRING→TEXT, BOOL→INTEGER, etc.
   - Knows all valid SQLite type affinities
   - Clear error messages with suggestions

2. **InvalidFunctionRule**: Detects non-SQLite functions
   - CONCAT → use || operator
   - LEN → use LENGTH()
   - ISNULL → use IFNULL() or COALESCE()
   - GETDATE/NOW → use CURRENT_TIMESTAMP

3. **SelectStarRule**: Warns about SELECT *
   - Common code smell
   - Fragile to schema changes
   - Can be inefficient

4. **MissingWhereClauseRule**: Catches DELETE/UPDATE without WHERE
   - Prevents accidental mass operations
   - Suggests adding WHERE 1=1 if intentional

5. **DoubleQuotesForStringsRule**: Reminds about SQLite quoting rules
   - Single quotes for string literals
   - Double quotes for identifiers

### Testing Strategy

Created comprehensive test suite with 37 tests:

1. **Unit tests** for each rule individually
   - Test positive cases (catches bad SQL)
   - Test negative cases (allows good SQL)
   - Test edge cases (case sensitivity, multiple issues)

2. **Integration tests** for LintingConnection
   - Test blocking behavior
   - Test warning vs error handling
   - Test custom rule configuration
   - Test cursor and connection methods

3. **End-to-end tests** for workflows
   - Typical usage patterns
   - Selective linting
   - Permissive mode

All tests pass successfully!

### Demo Script

Created `demo.py` showing:
1. Basic usage
2. Warning vs error levels
3. Custom rule configuration
4. Dangerous operations detection
5. Invalid function detection
6. Permissive (audit) mode

### Documentation

Created comprehensive README.md with:
- Quick start guide
- Detailed rule documentation
- Configuration examples
- Custom rule creation guide
- API reference
- Use cases and examples
- Running tests and demos

### Key Learnings

1. **SQLite Type System**: SQLite uses "type affinity" not strict types
   - Five affinities: TEXT, NUMERIC, INTEGER, REAL, BLOB
   - Many type names map to these (VARCHAR→TEXT, INT→INTEGER, etc.)
   - Common mistake: using types from other databases (STRING, BOOL)

2. **SQLite Function Differences**: Many SQL functions differ between databases
   - No CONCAT() - use || instead
   - LENGTH() not LEN()
   - No GETDATE() - use datetime() or CURRENT_TIMESTAMP

3. **Quote Rules**: SQLite follows SQL standard strictly
   - Single quotes for string literals: 'text'
   - Double quotes for identifiers: "column name"
   - Backticks also work for identifiers (MySQL compatibility)

4. **Wrapper Pattern Benefits**:
   - Clean code organization
   - Easy to maintain
   - Non-breaking changes to sqlite3
   - Clear separation of concerns

### Files Created

- `sqlite_linter.py`: Main library (400+ lines)
- `test_sqlite_linter.py`: Test suite (400+ lines, 37 tests)
- `demo.py`: Interactive demonstration
- `README.md`: Comprehensive documentation
- `notes.md`: This file

### Test Results

```
============================= test session starts ==============================
37 passed in 0.25s
```

### Demo Output

All 6 demos run successfully showing:
- Error detection and blocking
- Warning detection and optional blocking
- Custom rule configuration
- Dangerous operation detection
- Invalid function detection
- Permissive audit mode

### Conclusion

Successfully created a working, tested, documented SQLite query linter with:
- ✅ Configurable rule system
- ✅ 5 sensible default rules
- ✅ Flexible severity levels
- ✅ Comprehensive tests (37 passing)
- ✅ Clear documentation
- ✅ Working demo
- ✅ Drop-in replacement for sqlite3

The library successfully catches the example from the spec:
`SELECT CAST(bar AS STRING) AS baz FROM foo`
→ Error: "Invalid type 'STRING' in CAST. Did you mean 'TEXT'?"
