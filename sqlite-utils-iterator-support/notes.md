# sqlite-utils Iterator Support - Research Notes

## Objective
Modify simonw/sqlite-utils to allow `insert_all` and `upsert_all` methods to accept iterators yielding lists instead of only dicts, for improved performance with large datasets.

## Implementation Plan
1. Clone sqlite-utils repository to /tmp
2. Understand current implementation of insert_all/upsert_all
3. Run existing test suite
4. Implement list-based iterator support
5. Add comprehensive tests
6. Run performance benchmarks
7. Generate performance charts
8. Document findings

## Progress Log

### Setup Phase
- Created project folder: sqlite-utils-iterator-support
- Initialized notes.md
- Cloned simonw/sqlite-utils to /tmp/sqlite-utils

### Code Analysis
Examined the current implementation of insert_all and upsert_all:

**Current behavior:**
- `insert_all()` is at line 3294 in /tmp/sqlite-utils/sqlite_utils/db.py
- `upsert_all()` is at line 3502 - just wraps insert_all with upsert=True
- Both expect an iterable of dictionaries

**Key code locations:**
- Line 3360: Converts records to iterator
- Line 3363: Gets first_record via next()
- Line 3366: Calls `first_record.keys()` - assumes dict
- Line 3404: Iterates over `record.keys()` - assumes dict
- Lines 3027-3044 in build_insert_queries_and_params: Uses `record.get(key)` - assumes dict

**Implementation strategy:**
1. After getting first_record, detect if it's a list or dict
2. If list:
   - Validate it's a list of strings (column names)
   - Set a flag to use list mode
   - Get subsequent records as lists (data rows)
   - Adapt build_insert_queries_and_params to handle list mode
3. If dict: Continue with existing logic

### Testing Phase
- Ran existing test suite: 1001 tests passed, 16 skipped
- All tests passing on baseline code
- Ready to implement modifications

### Implementation Phase

**Changes made to /tmp/sqlite-utils/sqlite_utils/db.py:**

1. **insert_all method (lines 3357-3400):**
   - Added list_mode detection after getting first_record
   - If first record is a list, validate it contains column names (strings)
   - Extract column names and get actual first data record
   - Handle fix_square_braces differently for dict vs list mode

2. **insert_all chunk processing (lines 3406-3456):**
   - Changed to use records_iter instead of records
   - For list mode, convert lists to dicts for suggest_column_types
   - For list mode, use pre-determined column names instead of extracting from records
   - Skip column discovery in non-first chunks for list mode

3. **insert_chunk method (line 3160):**
   - Added list_mode parameter (default False)
   - Passed to build_insert_queries_and_params
   - Updated recursive calls for "too many SQL variables" case

4. **build_insert_queries_and_params method (lines 3013-3061):**
   - Added list_mode parameter (default False)
   - Added separate logic for list mode vs dict mode
   - In list mode, directly access values by index instead of using record.get()
   - Preserved all existing dict mode logic

### Testing New Functionality
- Created 10 comprehensive tests for list mode in test_list_mode.py
- All tests pass successfully
- Tests cover: basic usage, primary keys, upserts, type handling, error cases, batching
- Backward compatibility confirmed: all 1001 original tests still pass

### Benchmark Results
Ran comprehensive benchmarks comparing dict mode vs list mode:

**Key Findings:**
1. **Scenario with 100K rows, 5 columns:** List mode 21.6% faster (1.22x speedup)
2. **Scenario with 50K rows, 10 columns:** List mode 4.6% faster
3. **Scenario with 20K rows, 15 columns:** List mode 5.2% faster
4. **Scenario with 10K rows, 20 columns:** Dict mode 35.9% faster (list mode slower)
5. **Upsert scenarios:** Mixed results, generally similar performance

**Interpretation:**
- List mode excels with fewer columns (less dict overhead)
- Dict mode performs better with many columns (Python's dict optimization)
- Performance crossover appears around 10-15 columns
- For typical use cases (5-10 columns), list mode provides modest improvements
- Main benefit: Reduced memory overhead from not creating dict objects

