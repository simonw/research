"""
Comprehensive tests for sqlite_linter module
"""

import pytest
import sqlite_linter
from sqlite_linter import (
    LintLevel, LintIssue, LintError, LintWarning,
    InvalidCastTypeRule, SelectStarRule, MissingWhereClauseRule,
    InvalidFunctionRule, DoubleQuotesForStringsRule
)


class TestInvalidCastTypeRule:
    """Tests for invalid CAST type detection"""

    def test_catches_string_type(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(bar AS STRING) AS baz FROM foo"
        issues = rule.check(query)

        assert len(issues) == 1
        assert issues[0].level == LintLevel.ERROR
        assert "STRING" in issues[0].message
        assert "TEXT" in issues[0].message

    def test_catches_str_type(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(name AS STR) FROM users"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "STR" in issues[0].message
        assert "TEXT" in issues[0].message

    def test_catches_bool_type(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(active AS BOOL) FROM accounts"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "BOOL" in issues[0].message
        assert "INTEGER" in issues[0].message

    def test_allows_valid_text_type(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(bar AS TEXT) AS baz FROM foo"
        issues = rule.check(query)

        assert len(issues) == 0

    def test_allows_valid_integer_type(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(value AS INTEGER) FROM data"
        issues = rule.check(query)

        assert len(issues) == 0

    def test_allows_valid_real_type(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(price AS REAL) FROM products"
        issues = rule.check(query)

        assert len(issues) == 0

    def test_case_insensitive(self):
        rule = InvalidCastTypeRule()
        query = "SELECT cast(bar as string) FROM foo"
        issues = rule.check(query)

        assert len(issues) == 1

    def test_multiple_casts_in_query(self):
        rule = InvalidCastTypeRule()
        query = "SELECT CAST(a AS STRING), CAST(b AS TEXT), CAST(c AS BOOL) FROM t"
        issues = rule.check(query)

        assert len(issues) == 2  # STRING and BOOL are invalid


class TestSelectStarRule:
    """Tests for SELECT * detection"""

    def test_catches_select_star(self):
        rule = SelectStarRule()
        query = "SELECT * FROM users"
        issues = rule.check(query)

        assert len(issues) == 1
        assert issues[0].level == LintLevel.WARNING
        assert "SELECT *" in issues[0].message

    def test_allows_explicit_columns(self):
        rule = SelectStarRule()
        query = "SELECT id, name FROM users"
        issues = rule.check(query)

        assert len(issues) == 0

    def test_case_insensitive(self):
        rule = SelectStarRule()
        query = "select * from users"
        issues = rule.check(query)

        assert len(issues) == 1


class TestMissingWhereClauseRule:
    """Tests for missing WHERE clause detection"""

    def test_catches_delete_without_where(self):
        rule = MissingWhereClauseRule()
        query = "DELETE FROM users"
        issues = rule.check(query)

        assert len(issues) == 1
        assert issues[0].level == LintLevel.WARNING
        assert "DELETE" in issues[0].message
        assert "WHERE" in issues[0].message

    def test_catches_update_without_where(self):
        rule = MissingWhereClauseRule()
        query = "UPDATE users SET active = 0"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "UPDATE" in issues[0].message

    def test_allows_delete_with_where(self):
        rule = MissingWhereClauseRule()
        query = "DELETE FROM users WHERE id = 5"
        issues = rule.check(query)

        assert len(issues) == 0

    def test_allows_update_with_where(self):
        rule = MissingWhereClauseRule()
        query = "UPDATE users SET active = 0 WHERE id > 100"
        issues = rule.check(query)

        assert len(issues) == 0


class TestInvalidFunctionRule:
    """Tests for invalid function detection"""

    def test_catches_concat(self):
        rule = InvalidFunctionRule()
        query = "SELECT CONCAT(first_name, last_name) FROM users"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "CONCAT" in issues[0].message
        assert "||" in issues[0].message

    def test_catches_len(self):
        rule = InvalidFunctionRule()
        query = "SELECT LEN(name) FROM products"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "LEN" in issues[0].message
        assert "LENGTH" in issues[0].message

    def test_catches_isnull(self):
        rule = InvalidFunctionRule()
        query = "SELECT ISNULL(value, 0) FROM data"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "ISNULL" in issues[0].message

    def test_catches_getdate(self):
        rule = InvalidFunctionRule()
        query = "SELECT GETDATE()"
        issues = rule.check(query)

        assert len(issues) == 1
        assert "GETDATE" in issues[0].message

    def test_allows_valid_functions(self):
        rule = InvalidFunctionRule()
        query = "SELECT LENGTH(name), COALESCE(value, 0), CURRENT_TIMESTAMP FROM data"
        issues = rule.check(query)

        assert len(issues) == 0


class TestDoubleQuotesForStringsRule:
    """Tests for double quote detection"""

    def test_catches_double_quotes(self):
        rule = DoubleQuotesForStringsRule()
        query = 'SELECT * FROM users WHERE name = "John"'
        issues = rule.check(query)

        assert len(issues) == 1
        assert issues[0].level == LintLevel.WARNING
        assert "single quotes" in issues[0].message.lower()


class TestLintingConnection:
    """Integration tests for LintingConnection"""

    def test_connection_creation(self):
        conn = sqlite_linter.connect(":memory:")
        assert conn is not None

    def test_blocks_invalid_cast(self):
        conn = sqlite_linter.connect(":memory:")
        conn.execute("CREATE TABLE foo (bar INTEGER)")

        with pytest.raises(LintError) as exc_info:
            conn.execute("SELECT CAST(bar AS STRING) FROM foo")

        assert "STRING" in str(exc_info.value)
        assert "TEXT" in str(exc_info.value)

    def test_allows_valid_query(self):
        conn = sqlite_linter.connect(":memory:")
        conn.execute("CREATE TABLE users (id INTEGER, name TEXT)")
        conn.execute("INSERT INTO users VALUES (1, 'Alice')")

        cursor = conn.execute("SELECT id, name FROM users")
        results = cursor.fetchall()

        assert len(results) == 1
        assert results[0] == (1, 'Alice')

    def test_valid_cast_works(self):
        conn = sqlite_linter.connect(":memory:")
        conn.execute("CREATE TABLE data (value INTEGER)")
        conn.execute("INSERT INTO data VALUES (42)")

        cursor = conn.execute("SELECT CAST(value AS TEXT) FROM data")
        result = cursor.fetchone()

        assert result[0] == '42'

    def test_warning_doesnt_block_by_default(self):
        conn = sqlite_linter.connect(":memory:", raise_on_warning=False)
        conn.execute("CREATE TABLE users (id INTEGER, name TEXT)")

        # SELECT * should warn but not block
        cursor = conn.execute("SELECT * FROM users")
        assert cursor is not None

    def test_warning_blocks_when_configured(self):
        conn = sqlite_linter.connect(":memory:", raise_on_warning=True)
        conn.execute("CREATE TABLE users (id INTEGER, name TEXT)")

        with pytest.raises(LintWarning):
            conn.execute("SELECT * FROM users")

    def test_can_disable_error_blocking(self):
        conn = sqlite_linter.connect(":memory:", raise_on_error=False)
        conn.execute("CREATE TABLE foo (bar INTEGER)")

        # This should not raise even though it's an error
        # (though it will fail at SQLite level)
        try:
            conn.execute("SELECT CAST(bar AS STRING) FROM foo")
        except LintError:
            pytest.fail("Should not raise LintError when raise_on_error=False")
        except:
            # Other errors (like from SQLite) are fine
            pass

    def test_custom_rules(self):
        # Create connection with only one rule
        rules = [InvalidCastTypeRule()]
        conn = sqlite_linter.connect(":memory:", rules=rules)
        conn.execute("CREATE TABLE users (name TEXT)")

        # This should work because SELECT * rule is not included
        cursor = conn.execute("SELECT * FROM users")
        assert cursor is not None

    def test_cursor_method(self):
        conn = sqlite_linter.connect(":memory:")
        cursor = conn.cursor()

        cursor.execute("CREATE TABLE test (id INTEGER)")
        cursor.execute("INSERT INTO test VALUES (1)")

        cursor.execute("SELECT id FROM test")
        assert cursor.fetchone() == (1,)

    def test_executemany(self):
        conn = sqlite_linter.connect(":memory:")
        conn.execute("CREATE TABLE test (id INTEGER, value TEXT)")

        data = [(1, 'a'), (2, 'b'), (3, 'c')]
        conn.executemany("INSERT INTO test VALUES (?, ?)", data)

        cursor = conn.execute("SELECT * FROM test ORDER BY id")
        results = cursor.fetchall()
        assert results == data

    def test_context_manager(self):
        with sqlite_linter.connect(":memory:") as conn:
            conn.execute("CREATE TABLE test (id INTEGER)")
            conn.execute("INSERT INTO test VALUES (1)")
            conn.commit()

    def test_last_issues_tracking(self):
        conn = sqlite_linter.connect(":memory:", raise_on_warning=False)
        conn.execute("CREATE TABLE users (id INTEGER, name TEXT)")

        cursor = conn.cursor()
        cursor.execute("SELECT * FROM users")  # Should generate warning

        assert len(cursor.last_issues) == 1
        assert cursor.last_issues[0].level == LintLevel.WARNING

    def test_multiple_issues(self):
        conn = sqlite_linter.connect(":memory:", raise_on_error=True)
        conn.execute("CREATE TABLE test (value TEXT)")

        # Query with multiple issues
        with pytest.raises(LintError) as exc_info:
            conn.execute("SELECT CAST(value AS STRING), CONCAT('a', 'b') FROM test")

        assert len(exc_info.value.issues) == 2


class TestEndToEnd:
    """End-to-end workflow tests"""

    def test_typical_workflow(self):
        # Connect with default rules
        conn = sqlite_linter.connect(":memory:")

        # Create schema
        conn.execute("""
            CREATE TABLE products (
                id INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                price REAL,
                active INTEGER DEFAULT 1
            )
        """)

        # Insert data
        conn.execute("INSERT INTO products (name, price) VALUES (?, ?)",
                    ("Widget", 9.99))

        conn.executemany("INSERT INTO products (name, price) VALUES (?, ?)", [
            ("Gadget", 19.99),
            ("Doodad", 29.99),
        ])

        # Valid queries work fine
        cursor = conn.execute("SELECT name, price FROM products WHERE active = 1")
        results = cursor.fetchall()
        assert len(results) == 3

        # Invalid queries are blocked
        with pytest.raises(LintError):
            conn.execute("SELECT CAST(price AS STRING) FROM products")

        conn.close()

    def test_selective_linting(self):
        # Only check for CAST errors
        rules = [InvalidCastTypeRule()]
        conn = sqlite_linter.connect(":memory:", rules=rules)

        conn.execute("CREATE TABLE data (value INTEGER)")

        # SELECT * is allowed now (no SelectStarRule)
        cursor = conn.execute("SELECT * FROM data")
        assert cursor is not None

        # But invalid CAST still blocked
        with pytest.raises(LintError):
            conn.execute("SELECT CAST(value AS STRING) FROM data")

    def test_permissive_mode(self):
        # Warnings only, no blocking
        conn = sqlite_linter.connect(":memory:",
                                     raise_on_error=False,
                                     raise_on_warning=False)

        conn.execute("CREATE TABLE test (value TEXT)")

        # Can check issues without blocking
        cursor = conn.cursor()
        cursor.execute("SELECT * FROM test")  # Warning but allowed

        assert len(cursor.last_issues) > 0
        assert any(i.level == LintLevel.WARNING for i in cursor.last_issues)


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
