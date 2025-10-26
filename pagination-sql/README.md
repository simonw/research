# pagination-sql

Efficient keyset pagination utilities for SQL databases, extracted from [Datasette](https://github.com/simonw/datasette).

## Overview

This library provides utilities for implementing **keyset pagination** (also known as cursor-based pagination) in SQL databases. Unlike traditional OFFSET-based pagination, keyset pagination uses WHERE clauses to efficiently navigate large result sets, making it ideal for production applications.

### Why Keyset Pagination?

Traditional OFFSET pagination has significant performance problems:
- `SELECT * FROM users ORDER BY id LIMIT 10 OFFSET 10000` must scan through 10,000 rows
- Performance degrades linearly as you paginate deeper
- Inconsistent results if data changes between page requests

Keyset pagination solves these issues by using WHERE clauses:
- `SELECT * FROM users WHERE id > 10010 ORDER BY id LIMIT 10` uses indexes efficiently
- Consistent performance regardless of page depth
- Stable results even as data changes

## Features

- **Efficient keyset pagination** using WHERE clauses instead of OFFSET
- **Compound primary key support** for tables with multi-column keys
- **Sorting integration** - combine custom sorting with pagination
- **URL-safe tokens** using tilde encoding to avoid conflicts with percent-encoding
- **Type-safe and well-tested** with comprehensive test coverage

## Installation

```bash
pip install pagination-sql
```

Or install from source:

```bash
git clone https://github.com/yourusername/pagination-sql.git
cd pagination-sql
pip install -e .
```

## Quick Start

### Basic Usage (Single Primary Key)

```python
from pagination_sql import PaginationHelper
import sqlite3

# Create helper for a table with 'id' as primary key
helper = PaginationHelper(
    primary_keys=['id'],
    page_size=10
)

# First page - no token
where_clause, params = helper.get_where_clause()

conn = sqlite3.connect('database.db')
cursor = conn.cursor()

# Query with LIMIT page_size + 1 to detect if there's a next page
sql = f"""
    SELECT * FROM users
    {f'WHERE {where_clause}' if where_clause else ''}
    ORDER BY id
    LIMIT {helper.page_size + 1}
"""

cursor.execute(sql, params)
rows = cursor.fetchall()

# Check if there's a next page
if helper.has_next_page(rows):
    # Create token from second-to-last row (rows are dicts or tuples)
    row_dict = dict(zip([d[0] for d in cursor.description], rows[-2]))
    next_token = helper.create_next_token(row_dict)
    print(f"Next page token: {next_token}")

# Return only page_size rows (not the extra one)
page_rows = rows[:helper.page_size]
```

### Compound Primary Keys

For tables with multiple primary keys:

```python
helper = PaginationHelper(
    primary_keys=['org_id', 'user_id'],
    page_size=20
)

# With a next_token from a previous page
where_clause, params = helper.get_where_clause(
    next_token='123,456'  # org_id=123, user_id=456
)

sql = f"""
    SELECT * FROM members
    WHERE {where_clause}
    ORDER BY org_id, user_id
    LIMIT {helper.page_size + 1}
"""
```

This generates optimized SQL like:
```sql
WHERE (
    (org_id > :p0)
    or
    (org_id = :p0 and user_id > :p1)
)
```

### Pagination with Sorting

Combine custom sorting with pagination:

```python
helper = PaginationHelper(
    primary_keys=['id'],
    page_size=10
)

# Paginate while sorting by name
where_clause, params = helper.get_where_clause(
    next_token='Smith,100',  # name='Smith', id=100
    sort_column='name',
    sort_desc=False  # ascending
)

sql = f"""
    SELECT * FROM users
    WHERE {where_clause}
    ORDER BY name, id  -- Always include PK for stable ordering
    LIMIT {helper.page_size + 1}
"""

cursor.execute(sql, params)
rows = cursor.fetchall()

# When creating next token, include the sort value
if helper.has_next_page(rows):
    row_dict = dict(zip([d[0] for d in cursor.description], rows[-2]))
    next_token = helper.create_next_token(
        row_dict,
        sort_column='name',
        sort_value=row_dict['name']
    )
```

## API Reference

### `PaginationHelper`

Main class for managing pagination.

#### Constructor

```python
PaginationHelper(
    primary_keys: List[str],
    page_size: int = 100,
    use_rowid: bool = False
)
```

- `primary_keys`: List of primary key column names in order
- `page_size`: Number of rows per page (default: 100)
- `use_rowid`: Use SQLite's rowid instead of primary keys (default: False)

#### Methods

##### `get_where_clause(next_token=None, sort_column=None, sort_desc=False)`

Generate WHERE clause and parameters for pagination.

**Parameters:**
- `next_token` (str, optional): Pagination token from previous page
- `sort_column` (str, optional): Column to sort by
- `sort_desc` (bool): If True, sort descending (default: False)

**Returns:** `Tuple[str, Dict[str, Any]]` - (where_clause, params)

**Example:**
```python
where, params = helper.get_where_clause(next_token='123,456')
# where = '(([id] > :p0)\n  or\n([id] = :p0 and [created_at] > :p1))'
# params = {'p0': '123', 'p1': '456'}
```

##### `create_next_token(row, sort_column=None, sort_value=None)`

Create pagination token for the next page.

**Parameters:**
- `row` (dict): Row data (typically second-to-last row from results)
- `sort_column` (str, optional): Column being sorted by
- `sort_value` (Any, optional): Value of the sort column

**Returns:** `str` - Tilde-encoded pagination token

**Example:**
```python
token = helper.create_next_token({'id': 123, 'name': 'John'})
# token = '123,John'
```

##### `has_next_page(rows)`

Check if there are more pages.

**Parameters:**
- `rows` (list): List of rows returned from query

**Returns:** `bool` - True if more pages exist

**Example:**
```python
rows = cursor.fetchall()  # Fetched page_size + 1 rows
has_more = helper.has_next_page(rows)
```

### Utility Functions

#### `tilde_encode(s: str) -> str`

Encode string using tilde encoding (URL-safe alternative to percent encoding).

```python
from pagination_sql import tilde_encode
tilde_encode("/foo/bar")  # '~2Ffoo~2Fbar'
```

#### `tilde_decode(s: str) -> str`

Decode tilde-encoded string.

```python
from pagination_sql import tilde_decode
tilde_decode("~2Ffoo~2Fbar")  # '/foo/bar'
```

#### `urlsafe_components(token: str) -> List[str]`

Split and decode pagination token components.

```python
from pagination_sql import urlsafe_components
urlsafe_components("123,~2Ffoo,456")  # ['123', '/foo', '456']
```

#### `compound_keys_after_sql(pks: List[str], start_index: int = 0) -> str`

Generate WHERE clause for compound key pagination (low-level function).

```python
from pagination_sql import compound_keys_after_sql
clause = compound_keys_after_sql(['id', 'created_at'])
# '(([id] > :p0)\n  or\n([id] = :p0 and [created_at] > :p1))'
```

#### `escape_sqlite(s: str) -> str`

Escape SQLite identifier if needed.

```python
from pagination_sql import escape_sqlite
escape_sqlite("user_id")    # 'user_id'
escape_sqlite("select")     # '[select]'
escape_sqlite("my column")  # '[my column]'
```

## Complete Examples

### REST API Endpoint

```python
from flask import Flask, request, jsonify
from pagination_sql import PaginationHelper
import sqlite3

app = Flask(__name__)

@app.route('/api/users')
def get_users():
    # Get pagination token from query params
    next_token = request.args.get('next')
    page_size = int(request.args.get('size', 10))

    helper = PaginationHelper(['id'], page_size=page_size)
    where_clause, params = helper.get_where_clause(next_token=next_token)

    conn = sqlite3.connect('app.db')
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    sql = f"""
        SELECT id, name, email, created_at
        FROM users
        {f'WHERE {where_clause}' if where_clause else ''}
        ORDER BY id
        LIMIT {helper.page_size + 1}
    """

    cursor.execute(sql, params)
    rows = [dict(row) for row in cursor.fetchall()]

    # Determine if there's a next page
    has_next = helper.has_next_page(rows)
    next_token_value = None

    if has_next:
        next_token_value = helper.create_next_token(rows[-2])

    return jsonify({
        'data': rows[:helper.page_size],  # Don't include the extra row
        'next': next_token_value,
        'has_more': has_next
    })
```

### Async Database (aiosqlite)

```python
import aiosqlite
from pagination_sql import PaginationHelper

async def paginate_users(next_token=None):
    helper = PaginationHelper(['id'], page_size=10)
    where_clause, params = helper.get_where_clause(next_token=next_token)

    async with aiosqlite.connect('database.db') as db:
        db.row_factory = aiosqlite.Row

        sql = f"""
            SELECT * FROM users
            {f'WHERE {where_clause}' if where_clause else ''}
            ORDER BY id
            LIMIT {helper.page_size + 1}
        """

        async with db.execute(sql, params) as cursor:
            rows = await cursor.fetchall()
            rows = [dict(row) for row in rows]

            has_next = helper.has_next_page(rows)
            next_token_value = None

            if has_next:
                next_token_value = helper.create_next_token(rows[-2])

            return {
                'rows': rows[:helper.page_size],
                'next': next_token_value,
                'has_more': has_next
            }
```

### PostgreSQL with psycopg2

```python
import psycopg2
from psycopg2.extras import RealDictCursor
from pagination_sql import PaginationHelper

def get_posts(conn, next_token=None, sort_by='created_at'):
    helper = PaginationHelper(['id'], page_size=20)
    where_clause, params = helper.get_where_clause(
        next_token=next_token,
        sort_column=sort_by,
        sort_desc=True  # Most recent first
    )

    with conn.cursor(cursor_factory=RealDictCursor) as cursor:
        # PostgreSQL uses %(name)s placeholders, so convert
        pg_where = where_clause
        pg_params = {}
        for key, value in params.items():
            pg_where = pg_where.replace(f':{key}', f'%({key})s')
            pg_params[key] = value

        sql = f"""
            SELECT id, title, content, created_at
            FROM posts
            {f'WHERE {pg_where}' if pg_where else ''}
            ORDER BY {sort_by} DESC, id
            LIMIT {helper.page_size + 1}
        """

        cursor.execute(sql, pg_params)
        rows = cursor.fetchall()

        has_next = helper.has_next_page(rows)
        next_token_value = None

        if has_next:
            next_token_value = helper.create_next_token(
                rows[-2],
                sort_column=sort_by,
                sort_value=rows[-2][sort_by]
            )

        return rows[:helper.page_size], next_token_value, has_next
```

## How It Works

### The Keyset Pagination Algorithm

For a table with primary key `id`, instead of:
```sql
SELECT * FROM users ORDER BY id LIMIT 10 OFFSET 100
```

We use:
```sql
SELECT * FROM users WHERE id > 100 ORDER BY id LIMIT 11
```

Note: We fetch `page_size + 1` rows to efficiently detect if there's a next page.

### Compound Keys

For tables with compound keys like `(org_id, user_id)`, we generate:

```sql
WHERE (
    (org_id > 123)
    OR (org_id = 123 AND user_id > 456)
)
ORDER BY org_id, user_id
```

This maintains correct ordering across both key columns.

### With Sorting

When sorting by a non-primary-key column like `name`, we combine it:

```sql
WHERE (
    (name > 'Smith')
    OR (name = 'Smith' AND id > 100)
)
ORDER BY name, id
```

The primary key acts as a tie-breaker for consistent pagination.

## Testing

Run the test suite:

```bash
# Install dependencies
pip install pytest

# Run tests
pytest

# Run with coverage
pytest --cov=pagination_sql tests/
```

## Acknowledgments

This library extracts pagination logic from [Datasette](https://github.com/simonw/datasette) by Simon Willison. The implementation is based on the keyset pagination pattern described in [Datasette issue #190](https://github.com/simonw/datasette/issues/190).

## License

Apache License 2.0 (same as Datasette)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## See Also

- [Datasette](https://datasette.io/) - The original project this was extracted from
- [Efficient Pagination Using Deferred Joins](https://aaronfrancis.com/2022/efficient-pagination-using-deferred-joins)
- [We need tool support for keyset pagination](https://use-the-index-luke.com/no-offset)
