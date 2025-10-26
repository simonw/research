"""
pagination-sql: Keyset pagination utilities for SQL databases

This library provides utilities for implementing efficient keyset (cursor-based)
pagination in SQL databases. Extracted from the Datasette project.

Key features:
- Keyset pagination using WHERE clauses (more efficient than OFFSET)
- Support for compound primary keys
- Integration with sorting
- URL-safe token encoding

Example:
    >>> from pagination_sql import PaginationHelper
    >>> helper = PaginationHelper(
    ...     primary_keys=['id', 'created_at'],
    ...     page_size=10
    ... )
    >>> where_clause, params = helper.get_where_clause(next_token='123,2024-01-01')
    >>> # Use where_clause and params in your SQL query
"""

import re
import secrets
import urllib.parse
from typing import List, Dict, Tuple, Optional, Any

__version__ = "0.1.0"

# SQLite reserved words - identifiers matching these need to be escaped
RESERVED_WORDS = set(
    (
        "abort action add after all alter analyze and as asc attach autoincrement "
        "before begin between by cascade case cast check collate column commit "
        "conflict constraint create cross current_date current_time "
        "current_timestamp database default deferrable deferred delete desc detach "
        "distinct drop each else end escape except exclusive exists explain fail "
        "for foreign from full glob group having if ignore immediate in index "
        "indexed initially inner insert instead intersect into is isnull join key "
        "left like limit match natural no not notnull null of offset on or order "
        "outer plan pragma primary query raise recursive references regexp reindex "
        "release rename replace restrict right rollback row savepoint select set "
        "table temp temporary then to transaction trigger union unique update using "
        "vacuum values view virtual when where with without"
    ).split()
)

_BORING_KEYWORD_RE = re.compile(r"^[a-zA-Z_][a-zA-Z0-9_]*$")

# Tilde encoding safe characters (like URL encoding but with ~ instead of %)
_TILDE_ENCODING_SAFE = frozenset(
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    b"abcdefghijklmnopqrstuvwxyz"
    b"0123456789_-"
)

_SPACE = ord(" ")


class TildeEncoder(dict):
    """Cache-based tilde encoder for efficient string encoding."""

    def __missing__(self, b):
        """Handle cache miss, store encoded string and return."""
        if b in _TILDE_ENCODING_SAFE:
            res = chr(b)
        elif b == _SPACE:
            res = "+"
        else:
            res = "~{:02X}".format(b)
        self[b] = res
        return res


_tilde_encoder = TildeEncoder().__getitem__


def tilde_encode(s: str) -> str:
    """
    Returns tilde-encoded string.

    Similar to URL encoding, but uses ~ instead of % for special characters.
    This avoids conflicts with actual percent-encoded URLs.

    Args:
        s: String to encode

    Returns:
        Tilde-encoded string

    Example:
        >>> tilde_encode("/foo/bar")
        '~2Ffoo~2Fbar'
    """
    return "".join(_tilde_encoder(char) for char in s.encode("utf-8"))


def tilde_decode(s: str) -> str:
    """
    Decodes a tilde-encoded string.

    Args:
        s: Tilde-encoded string

    Returns:
        Decoded string

    Example:
        >>> tilde_decode("~2Ffoo~2Fbar")
        '/foo/bar'
    """
    # Avoid accidentally decoding a %2f style sequence
    temp = secrets.token_hex(16)
    s = s.replace("%", temp)
    decoded = urllib.parse.unquote_plus(s.replace("~", "%"))
    return decoded.replace(temp, "%")


def urlsafe_components(token: str) -> List[str]:
    """
    Splits pagination token on commas and tilde-decodes each component.

    Args:
        token: Pagination token (comma-separated, tilde-encoded values)

    Returns:
        List of decoded component strings

    Example:
        >>> urlsafe_components("value1,~2Fvalue2,value3")
        ['value1', '/value2', 'value3']
    """
    return [tilde_decode(b) for b in token.split(",")]


def escape_sqlite(s: str) -> str:
    """
    Escapes SQLite identifier if needed.

    Identifiers that are simple (alphanumeric + underscore) and not reserved
    words don't need escaping. Others are wrapped in square brackets.

    Args:
        s: Identifier to escape

    Returns:
        Escaped identifier safe for SQL

    Example:
        >>> escape_sqlite("user_id")
        'user_id'
        >>> escape_sqlite("select")
        '[select]'
        >>> escape_sqlite("my column")
        '[my column]'
    """
    if _BORING_KEYWORD_RE.match(s) and (s.lower() not in RESERVED_WORDS):
        return s
    else:
        return f"[{s}]"


def path_from_row_pks(
    row: Dict[str, Any],
    pks: List[str],
    use_rowid: bool = False,
    quote: bool = True
) -> str:
    """
    Generate an optionally tilde-encoded unique identifier for a row from its primary keys.

    Args:
        row: Row data as a dictionary
        pks: List of primary key column names
        use_rowid: If True, use 'rowid' instead of primary keys
        quote: If True, tilde-encode the values

    Returns:
        Comma-separated string of primary key values

    Example:
        >>> row = {"id": 123, "name": "test"}
        >>> path_from_row_pks(row, ["id", "name"])
        '123,test'
    """
    if use_rowid:
        bits = [row["rowid"]]
    else:
        bits = [
            row[pk]["value"] if isinstance(row[pk], dict) else row[pk]
            for pk in pks
        ]
    if quote:
        bits = [tilde_encode(str(bit)) for bit in bits]
    else:
        bits = [str(bit) for bit in bits]

    return ",".join(bits)


def compound_keys_after_sql(pks: List[str], start_index: int = 0) -> str:
    """
    Generate SQL WHERE clause for keyset pagination with compound primary keys.

    This implements efficient keyset pagination by creating a WHERE clause that
    correctly orders results after a given position, even with compound keys.

    For a table with primary keys [pk1, pk2, pk3], this generates:

        ([pk1] > :p0)
          or
        ([pk1] = :p0 and [pk2] > :p1)
          or
        ([pk1] = :p0 and [pk2] = :p1 and [pk3] > :p2)

    This ensures that pagination continues from the exact point where it left off,
    maintaining proper ordering across all key columns.

    Args:
        pks: List of primary key column names in order
        start_index: Starting index for parameter placeholders (:p0, :p1, etc.)

    Returns:
        SQL WHERE clause string with parameter placeholders

    Example:
        >>> compound_keys_after_sql(['id', 'created_at'])
        '(([id] > :p0)\\n  or\\n([id] = :p0 and [created_at] > :p1))'

    See:
        https://github.com/simonw/datasette/issues/190
    """
    or_clauses = []
    pks_left = pks[:]
    while pks_left:
        and_clauses = []
        last = pks_left[-1]
        rest = pks_left[:-1]
        and_clauses = [
            f"{escape_sqlite(pk)} = :p{i + start_index}"
            for i, pk in enumerate(rest)
        ]
        and_clauses.append(f"{escape_sqlite(last)} > :p{len(rest) + start_index}")
        or_clauses.append(f"({' and '.join(and_clauses)})")
        pks_left.pop()
    or_clauses.reverse()
    return "({})".format("\n  or\n".join(or_clauses))


class PaginationHelper:
    """
    Helper class for implementing keyset pagination in SQL queries.

    This class helps build SQL WHERE clauses and manage pagination tokens
    for efficient keyset-based pagination.

    Example:
        >>> helper = PaginationHelper(
        ...     primary_keys=['id', 'name'],
        ...     page_size=50
        ... )
        >>>
        >>> # Parse next token from request
        >>> where_clause, params = helper.get_where_clause(
        ...     next_token='123,John'
        ... )
        >>>
        >>> # Build SQL query
        >>> sql = f"SELECT * FROM users WHERE {where_clause} ORDER BY id, name LIMIT {helper.page_size + 1}"
        >>>
        >>> # Execute query with params...
        >>> # Then generate next token from results
        >>> next_token = helper.create_next_token(rows[-2])
    """

    def __init__(
        self,
        primary_keys: List[str],
        page_size: int = 100,
        use_rowid: bool = False
    ):
        """
        Initialize pagination helper.

        Args:
            primary_keys: List of primary key column names in order
            page_size: Number of rows per page
            use_rowid: If True, use rowid for pagination instead of primary keys
        """
        self.primary_keys = primary_keys
        self.page_size = page_size
        self.use_rowid = use_rowid

    def get_where_clause(
        self,
        next_token: Optional[str] = None,
        sort_column: Optional[str] = None,
        sort_desc: bool = False
    ) -> Tuple[str, Dict[str, Any]]:
        """
        Generate WHERE clause and parameters for pagination.

        Args:
            next_token: Pagination token from previous page (comma-separated values)
            sort_column: Column to sort by (in addition to primary keys)
            sort_desc: If True, sort in descending order

        Returns:
            Tuple of (where_clause, params_dict)

        Example:
            >>> helper = PaginationHelper(['id', 'created_at'])
            >>> where, params = helper.get_where_clause(next_token='123,2024-01-01')
            >>> where
            '(([id] > :p0)\\n  or\\n([id] = :p0 and [created_at] > :p1))'
            >>> params
            {'p0': '123', 'p1': '2024-01-01'}
        """
        if not next_token:
            return "", {}

        components = urlsafe_components(next_token)
        params = {}

        # If sorting is applied, first component is the sort value
        if sort_column:
            if len(components) < 2:
                raise ValueError(
                    f"Invalid next_token for sorted query: {next_token}. "
                    f"Expected sort value + {len(self.primary_keys)} primary key values"
                )

            sort_value = components[0]
            if sort_value == "$null":
                sort_value = None

            pk_values = components[1:]

            # Build WHERE clause for sorted pagination
            next_by_pk_clauses = []
            for i, pk_value in enumerate(pk_values):
                params[f"p{i + 1}"] = pk_value

            pk_where = compound_keys_after_sql(self.primary_keys, start_index=1)

            op = "<" if sort_desc else ">"
            where_clause = (
                f"({escape_sqlite(sort_column)} {op} :p0 "
                f"or ({escape_sqlite(sort_column)} = :p0 and {pk_where}))"
            )
            params["p0"] = sort_value

        else:
            # Standard keyset pagination without sorting
            pk_values = components

            if len(pk_values) != len(self.primary_keys):
                raise ValueError(
                    f"Invalid next_token: {next_token}. "
                    f"Expected {len(self.primary_keys)} values, got {len(pk_values)}"
                )

            for i, pk_value in enumerate(pk_values):
                params[f"p{i}"] = pk_value

            where_clause = compound_keys_after_sql(self.primary_keys)

        return where_clause, params

    def create_next_token(
        self,
        row: Dict[str, Any],
        sort_column: Optional[str] = None,
        sort_value: Any = None
    ) -> str:
        """
        Create pagination token for the next page from the current row.

        Args:
            row: The row to create token from (typically second-to-last row)
            sort_column: If sorting is applied, the column being sorted
            sort_value: If sorting is applied, the value of the sort column

        Returns:
            Tilde-encoded pagination token

        Example:
            >>> helper = PaginationHelper(['id', 'name'])
            >>> row = {'id': 123, 'name': 'John Doe'}
            >>> helper.create_next_token(row)
            '123,John+Doe'
        """
        pk_path = path_from_row_pks(row, self.primary_keys, self.use_rowid, quote=True)

        if sort_column:
            # Prepend sort value to the token
            if sort_value is None:
                prefix = "$null"
            else:
                prefix = tilde_encode(str(sort_value))
            return f"{prefix},{pk_path}"
        else:
            return pk_path

    def has_next_page(self, rows: List[Any]) -> bool:
        """
        Check if there are more pages based on row count.

        To detect if there's a next page, query should request page_size + 1 rows.
        If that many rows are returned, there's another page.

        Args:
            rows: List of rows returned from query

        Returns:
            True if there are more pages

        Example:
            >>> helper = PaginationHelper(['id'], page_size=10)
            >>> rows = [{'id': i} for i in range(11)]  # Got 11 rows
            >>> helper.has_next_page(rows)
            True
        """
        return len(rows) > self.page_size
