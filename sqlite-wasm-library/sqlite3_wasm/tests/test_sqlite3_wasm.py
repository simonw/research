"""
Tests for sqlite3_wasm module.

These tests verify that sqlite3_wasm provides an API compatible with
Python's standard sqlite3 module.
"""

import pytest
import sqlite3_wasm


class TestModuleAttributes:
    """Test module-level attributes and functions."""

    def test_apilevel(self):
        """Test that apilevel is set correctly."""
        assert sqlite3_wasm.apilevel == "2.0"

    def test_paramstyle(self):
        """Test that paramstyle is set correctly."""
        assert sqlite3_wasm.paramstyle == "qmark"

    def test_threadsafety(self):
        """Test that threadsafety is set correctly."""
        assert sqlite3_wasm.threadsafety == 0

    def test_version_is_string(self):
        """Test that sqlite_version is a string."""
        # Force initialization
        sqlite3_wasm.connect(":memory:").close()
        assert isinstance(sqlite3_wasm.sqlite_version, str)

    def test_version_info_is_tuple(self):
        """Test that sqlite_version_info is a tuple."""
        # Force initialization
        sqlite3_wasm.connect(":memory:").close()
        assert isinstance(sqlite3_wasm.sqlite_version_info, tuple)


class TestConnect:
    """Test the connect function."""

    def test_connect_memory(self):
        """Test connecting to an in-memory database."""
        conn = sqlite3_wasm.connect(":memory:")
        assert conn is not None
        conn.close()

    def test_connect_with_context_manager(self):
        """Test using connection as a context manager."""
        with sqlite3_wasm.connect(":memory:") as conn:
            cursor = conn.execute("SELECT 1")
            result = cursor.fetchone()
            assert result == (1,)


class TestConnection:
    """Test the Connection class."""

    def test_cursor_creation(self):
        """Test creating a cursor from a connection."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        assert cursor is not None
        conn.close()

    def test_execute_returns_cursor(self):
        """Test that execute returns a cursor."""
        conn = sqlite3_wasm.connect(":memory:")
        result = conn.execute("SELECT 1")
        assert isinstance(result, sqlite3_wasm.Cursor)
        conn.close()

    def test_executemany(self):
        """Test executemany method."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.executemany(
            "INSERT INTO test (id, name) VALUES (?, ?)",
            [(1, "Alice"), (2, "Bob"), (3, "Charlie")]
        )
        cursor = conn.execute("SELECT * FROM test ORDER BY id")
        results = cursor.fetchall()
        assert len(results) == 3
        assert results[0] == (1, "Alice")
        assert results[1] == (2, "Bob")
        assert results[2] == (3, "Charlie")
        conn.close()

    def test_executescript(self):
        """Test executescript method."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.executescript("""
            CREATE TABLE test1 (id INTEGER);
            CREATE TABLE test2 (id INTEGER);
            INSERT INTO test1 VALUES (1);
            INSERT INTO test2 VALUES (2);
        """)
        cursor = conn.execute("SELECT * FROM test1")
        assert cursor.fetchone() == (1,)
        cursor = conn.execute("SELECT * FROM test2")
        assert cursor.fetchone() == (2,)
        conn.close()

    def test_row_factory(self):
        """Test setting a row factory."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.row_factory = sqlite3_wasm.Row
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")
        cursor = conn.execute("SELECT * FROM test")
        row = cursor.fetchone()
        assert row["id"] == 1
        assert row["name"] == "Alice"
        assert row[0] == 1
        assert row[1] == "Alice"
        conn.close()

    def test_total_changes(self):
        """Test total_changes property."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER)")
        conn.execute("INSERT INTO test VALUES (1)")
        conn.execute("INSERT INTO test VALUES (2)")
        assert conn.total_changes >= 2
        conn.close()

    def test_in_transaction(self):
        """Test in_transaction property."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER)")
        assert not conn.in_transaction
        conn.execute("BEGIN")
        conn.execute("INSERT INTO test VALUES (1)")
        assert conn.in_transaction
        conn.commit()
        assert not conn.in_transaction
        conn.close()

    def test_close_twice_is_safe(self):
        """Test that closing a connection twice is safe."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.close()
        conn.close()  # Should not raise

    def test_operations_on_closed_connection_raise(self):
        """Test that operations on closed connection raise error."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.close()
        with pytest.raises(sqlite3_wasm.ProgrammingError):
            conn.execute("SELECT 1")

    def test_iterdump(self):
        """Test iterdump method."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")

        dump = list(conn.iterdump())
        assert "BEGIN TRANSACTION;" in dump
        assert "COMMIT;" in dump
        # Check that CREATE TABLE is in the dump
        assert any("CREATE TABLE" in stmt for stmt in dump)
        conn.close()


class TestCursor:
    """Test the Cursor class."""

    def test_execute_select(self):
        """Test executing a SELECT statement."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("SELECT 1 + 1 AS result")
        result = cursor.fetchone()
        assert result == (2,)
        conn.close()

    def test_execute_with_positional_params(self):
        """Test executing with positional parameters."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        cursor.execute("INSERT INTO test VALUES (?, ?)", (1, "Alice"))
        cursor.execute("SELECT * FROM test WHERE id = ?", (1,))
        result = cursor.fetchone()
        assert result == (1, "Alice")
        conn.close()

    def test_execute_with_named_params(self):
        """Test executing with named parameters."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        cursor.execute("INSERT INTO test VALUES (:id, :name)", {"id": 1, "name": "Alice"})
        cursor.execute("SELECT * FROM test WHERE id = :id", {"id": 1})
        result = cursor.fetchone()
        assert result == (1, "Alice")
        conn.close()

    def test_fetchone(self):
        """Test fetchone method."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("SELECT 1 UNION SELECT 2 UNION SELECT 3 ORDER BY 1")
        assert cursor.fetchone() == (1,)
        assert cursor.fetchone() == (2,)
        assert cursor.fetchone() == (3,)
        assert cursor.fetchone() is None
        conn.close()

    def test_fetchmany(self):
        """Test fetchmany method."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("SELECT 1 UNION SELECT 2 UNION SELECT 3 UNION SELECT 4 ORDER BY 1")
        results = cursor.fetchmany(2)
        assert len(results) == 2
        assert results[0] == (1,)
        assert results[1] == (2,)
        conn.close()

    def test_fetchall(self):
        """Test fetchall method."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("SELECT 1 UNION SELECT 2 UNION SELECT 3 ORDER BY 1")
        results = cursor.fetchall()
        assert len(results) == 3
        assert results[0] == (1,)
        assert results[1] == (2,)
        assert results[2] == (3,)
        conn.close()

    def test_description(self):
        """Test cursor description property."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        cursor.execute("SELECT id, name FROM test")
        assert cursor.description is not None
        assert len(cursor.description) == 2
        assert cursor.description[0][0] == "id"
        assert cursor.description[1][0] == "name"
        conn.close()

    def test_rowcount_insert(self):
        """Test rowcount for INSERT."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test (id INTEGER)")
        cursor.execute("INSERT INTO test VALUES (1)")
        assert cursor.rowcount == 1
        conn.close()

    def test_rowcount_update(self):
        """Test rowcount for UPDATE."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test (id INTEGER)")
        cursor.execute("INSERT INTO test VALUES (1)")
        cursor.execute("INSERT INTO test VALUES (2)")
        cursor.execute("UPDATE test SET id = id + 10")
        assert cursor.rowcount == 2
        conn.close()

    def test_lastrowid(self):
        """Test lastrowid property."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("CREATE TABLE test (id INTEGER PRIMARY KEY, name TEXT)")
        cursor.execute("INSERT INTO test (name) VALUES ('Alice')")
        assert cursor.lastrowid == 1
        cursor.execute("INSERT INTO test (name) VALUES ('Bob')")
        assert cursor.lastrowid == 2
        conn.close()

    def test_cursor_iteration(self):
        """Test iterating over cursor."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.execute("SELECT 1 UNION SELECT 2 UNION SELECT 3 ORDER BY 1")
        results = list(cursor)
        assert len(results) == 3
        conn.close()

    def test_cursor_context_manager(self):
        """Test cursor as context manager."""
        conn = sqlite3_wasm.connect(":memory:")
        with conn.cursor() as cursor:
            cursor.execute("SELECT 1")
            result = cursor.fetchone()
            assert result == (1,)
        conn.close()

    def test_arraysize(self):
        """Test arraysize property."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        assert cursor.arraysize == 1
        cursor.arraysize = 10
        assert cursor.arraysize == 10
        conn.close()


class TestDataTypes:
    """Test handling of different data types."""

    def test_integer(self):
        """Test integer storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val INTEGER)")
        conn.execute("INSERT INTO test VALUES (?)", (42,))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert result[0] == 42
        assert isinstance(result[0], int)
        conn.close()

    def test_large_integer(self):
        """Test large integer storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val INTEGER)")
        large_int = 9223372036854775807  # Max 64-bit signed int
        conn.execute("INSERT INTO test VALUES (?)", (large_int,))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert result[0] == large_int
        conn.close()

    def test_float(self):
        """Test float storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val REAL)")
        conn.execute("INSERT INTO test VALUES (?)", (3.14159,))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert abs(result[0] - 3.14159) < 0.00001
        assert isinstance(result[0], float)
        conn.close()

    def test_text(self):
        """Test text storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val TEXT)")
        conn.execute("INSERT INTO test VALUES (?)", ("Hello, World!",))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert result[0] == "Hello, World!"
        assert isinstance(result[0], str)
        conn.close()

    def test_unicode_text(self):
        """Test unicode text storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val TEXT)")
        test_text = "Hello, 世界! 🌍"
        conn.execute("INSERT INTO test VALUES (?)", (test_text,))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert result[0] == test_text
        conn.close()

    def test_blob(self):
        """Test blob storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val BLOB)")
        test_bytes = b"\x00\x01\x02\x03\xff\xfe\xfd"
        conn.execute("INSERT INTO test VALUES (?)", (test_bytes,))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert result[0] == test_bytes
        assert isinstance(result[0], bytes)
        conn.close()

    def test_null(self):
        """Test NULL storage and retrieval."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val TEXT)")
        conn.execute("INSERT INTO test VALUES (?)", (None,))
        cursor = conn.execute("SELECT val FROM test")
        result = cursor.fetchone()
        assert result[0] is None
        conn.close()


class TestExceptions:
    """Test exception handling."""

    def test_operational_error_bad_sql(self):
        """Test that bad SQL raises OperationalError."""
        conn = sqlite3_wasm.connect(":memory:")
        with pytest.raises(sqlite3_wasm.OperationalError):
            conn.execute("THIS IS NOT VALID SQL")
        conn.close()

    def test_programming_error_closed_cursor(self):
        """Test that operations on closed cursor raise ProgrammingError."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.cursor()
        cursor.close()
        with pytest.raises(sqlite3_wasm.ProgrammingError):
            cursor.execute("SELECT 1")
        conn.close()

    def test_not_supported_error_create_function(self):
        """Test that create_function raises NotSupportedError."""
        conn = sqlite3_wasm.connect(":memory:")
        with pytest.raises(sqlite3_wasm.NotSupportedError):
            conn.create_function("test", 0, lambda: None)
        conn.close()


class TestRow:
    """Test the Row class."""

    def test_row_access_by_index(self):
        """Test accessing row values by index."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.row_factory = sqlite3_wasm.Row
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")
        cursor = conn.execute("SELECT * FROM test")
        row = cursor.fetchone()
        assert row[0] == 1
        assert row[1] == "Alice"
        conn.close()

    def test_row_access_by_name(self):
        """Test accessing row values by column name."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.row_factory = sqlite3_wasm.Row
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")
        cursor = conn.execute("SELECT * FROM test")
        row = cursor.fetchone()
        assert row["id"] == 1
        assert row["name"] == "Alice"
        conn.close()

    def test_row_keys(self):
        """Test Row.keys() method."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.row_factory = sqlite3_wasm.Row
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")
        cursor = conn.execute("SELECT * FROM test")
        row = cursor.fetchone()
        keys = row.keys()
        assert "id" in keys
        assert "name" in keys
        conn.close()

    def test_row_iteration(self):
        """Test iterating over Row."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.row_factory = sqlite3_wasm.Row
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")
        cursor = conn.execute("SELECT * FROM test")
        row = cursor.fetchone()
        values = list(row)
        assert values == [1, "Alice"]
        conn.close()

    def test_row_len(self):
        """Test len() on Row."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.row_factory = sqlite3_wasm.Row
        conn.execute("CREATE TABLE test (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO test VALUES (1, 'Alice')")
        cursor = conn.execute("SELECT * FROM test")
        row = cursor.fetchone()
        assert len(row) == 2
        conn.close()


class TestTransactions:
    """Test transaction handling."""

    def test_commit(self):
        """Test explicit commit."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER)")
        conn.execute("BEGIN")
        conn.execute("INSERT INTO test VALUES (1)")
        conn.commit()
        cursor = conn.execute("SELECT * FROM test")
        assert cursor.fetchone() == (1,)
        conn.close()

    def test_rollback(self):
        """Test rollback."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER)")
        conn.execute("INSERT INTO test VALUES (1)")
        conn.execute("BEGIN")
        conn.execute("INSERT INTO test VALUES (2)")
        conn.rollback()
        cursor = conn.execute("SELECT COUNT(*) FROM test")
        assert cursor.fetchone() == (1,)
        conn.close()

    def test_context_manager_commit(self):
        """Test that context manager commits on success."""
        with sqlite3_wasm.connect(":memory:") as conn:
            conn.execute("CREATE TABLE test (id INTEGER)")
            conn.execute("INSERT INTO test VALUES (1)")
        # Connection is closed after context manager

    def test_context_manager_rollback_on_exception(self):
        """Test that context manager rolls back on exception."""
        try:
            with sqlite3_wasm.connect(":memory:") as conn:
                conn.execute("CREATE TABLE test (id INTEGER)")
                conn.execute("INSERT INTO test VALUES (1)")
                raise ValueError("Test exception")
        except ValueError:
            pass
        # The transaction should have been rolled back


class TestCompleteStatement:
    """Test the complete_statement function."""

    def test_complete_statement_true(self):
        """Test that complete statements return True."""
        assert sqlite3_wasm.complete_statement("SELECT 1;") is True

    def test_complete_statement_false(self):
        """Test that incomplete statements return False."""
        assert sqlite3_wasm.complete_statement("SELECT") is False


class TestSQLiteFunctions:
    """Test SQLite built-in functions."""

    def test_length_function(self):
        """Test SQLite LENGTH function."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.execute("SELECT LENGTH('hello')")
        assert cursor.fetchone() == (5,)
        conn.close()

    def test_upper_function(self):
        """Test SQLite UPPER function."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.execute("SELECT UPPER('hello')")
        assert cursor.fetchone() == ("HELLO",)
        conn.close()

    def test_lower_function(self):
        """Test SQLite LOWER function."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.execute("SELECT LOWER('HELLO')")
        assert cursor.fetchone() == ("hello",)
        conn.close()

    def test_sqlite_version_function(self):
        """Test SQLite sqlite_version() function."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.execute("SELECT sqlite_version()")
        version = cursor.fetchone()[0]
        assert isinstance(version, str)
        assert "." in version  # Version has format like "3.45.3"
        conn.close()

    def test_coalesce_function(self):
        """Test SQLite COALESCE function."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.execute("SELECT COALESCE(NULL, 'default')")
        assert cursor.fetchone() == ("default",)
        conn.close()

    def test_aggregate_count(self):
        """Test COUNT aggregate function."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER)")
        conn.execute("INSERT INTO test VALUES (1), (2), (3)")
        cursor = conn.execute("SELECT COUNT(*) FROM test")
        assert cursor.fetchone() == (3,)
        conn.close()

    def test_aggregate_sum(self):
        """Test SUM aggregate function."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE TABLE test (val INTEGER)")
        conn.execute("INSERT INTO test VALUES (1), (2), (3), (4), (5)")
        cursor = conn.execute("SELECT SUM(val) FROM test")
        assert cursor.fetchone() == (15,)
        conn.close()

    def test_json_function(self):
        """Test JSON functions (enabled with SQLITE_ENABLE_JSON1)."""
        conn = sqlite3_wasm.connect(":memory:")
        cursor = conn.execute("SELECT JSON('{\"a\": 1}')")
        result = cursor.fetchone()[0]
        assert "a" in result
        conn.close()


class TestFTS5:
    """Test FTS5 full-text search (enabled with SQLITE_ENABLE_FTS5)."""

    def test_fts5_create_table(self):
        """Test creating an FTS5 virtual table."""
        conn = sqlite3_wasm.connect(":memory:")
        conn.execute("CREATE VIRTUAL TABLE test_fts USING fts5(content)")
        conn.execute("INSERT INTO test_fts VALUES ('hello world')")
        cursor = conn.execute("SELECT * FROM test_fts WHERE test_fts MATCH 'hello'")
        result = cursor.fetchone()
        assert result[0] == "hello world"
        conn.close()


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
