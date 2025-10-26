"""
Tests for pagination-sql library.

These tests verify the keyset pagination logic works correctly for:
- Simple single-column primary keys
- Compound multi-column primary keys
- Pagination with sorting
- URL-safe token encoding/decoding
"""

import pytest
from pagination_sql import (
    PaginationHelper,
    compound_keys_after_sql,
    tilde_encode,
    tilde_decode,
    urlsafe_components,
    escape_sqlite,
    path_from_row_pks,
)


class TestTildeEncoding:
    """Test tilde encoding/decoding functions."""

    def test_tilde_encode_simple(self):
        assert tilde_encode("hello") == "hello"

    def test_tilde_encode_with_slash(self):
        assert tilde_encode("/foo/bar") == "~2Ffoo~2Fbar"

    def test_tilde_encode_with_space(self):
        assert tilde_encode("hello world") == "hello+world"

    def test_tilde_encode_special_chars(self):
        result = tilde_encode("hello@world.com")
        assert result == "hello~40world~2Ecom"

    def test_tilde_decode_simple(self):
        assert tilde_decode("hello") == "hello"

    def test_tilde_decode_with_slash(self):
        assert tilde_decode("~2Ffoo~2Fbar") == "/foo/bar"

    def test_tilde_decode_with_space(self):
        assert tilde_decode("hello+world") == "hello world"

    def test_tilde_encode_decode_roundtrip(self):
        original = "/path/to/file with spaces@special.chars"
        encoded = tilde_encode(original)
        decoded = tilde_decode(encoded)
        assert decoded == original

    def test_tilde_decode_preserves_percent(self):
        # Should not decode actual percent-encoded strings
        assert tilde_decode("%2F") == "%2F"


class TestUrlsafeComponents:
    """Test URL-safe token parsing."""

    def test_simple_components(self):
        assert urlsafe_components("a,b,c") == ["a", "b", "c"]

    def test_encoded_components(self):
        result = urlsafe_components("~2Ffoo,bar,~2Fbaz")
        assert result == ["/foo", "bar", "/baz"]

    def test_single_component(self):
        assert urlsafe_components("hello") == ["hello"]

    def test_empty_components(self):
        # Empty string between commas
        result = urlsafe_components("a,,c")
        assert result == ["a", "", "c"]


class TestEscapeSqlite:
    """Test SQLite identifier escaping."""

    def test_simple_identifier(self):
        assert escape_sqlite("user_id") == "user_id"
        assert escape_sqlite("UserName") == "UserName"
        assert escape_sqlite("_private") == "_private"

    def test_reserved_word(self):
        assert escape_sqlite("select") == "[select]"
        assert escape_sqlite("where") == "[where]"
        assert escape_sqlite("order") == "[order]"

    def test_special_characters(self):
        assert escape_sqlite("my column") == "[my column]"
        assert escape_sqlite("user-name") == "[user-name]"
        assert escape_sqlite("123column") == "[123column]"

    def test_case_insensitive_reserved(self):
        assert escape_sqlite("SELECT") == "[SELECT]"
        assert escape_sqlite("Select") == "[Select]"


class TestPathFromRowPks:
    """Test primary key path generation."""

    def test_simple_pk(self):
        row = {"id": 123}
        result = path_from_row_pks(row, ["id"], use_rowid=False, quote=False)
        assert result == "123"

    def test_compound_pks(self):
        row = {"id": 123, "name": "test"}
        result = path_from_row_pks(row, ["id", "name"], use_rowid=False, quote=False)
        assert result == "123,test"

    def test_quoted_pks(self):
        row = {"id": 123, "path": "/foo/bar"}
        result = path_from_row_pks(row, ["id", "path"], use_rowid=False, quote=True)
        assert result == "123,~2Ffoo~2Fbar"

    def test_dict_value(self):
        # Handle dict-style values (as Datasette sometimes returns)
        row = {"id": {"value": 123}}
        result = path_from_row_pks(row, ["id"], use_rowid=False, quote=False)
        assert result == "123"

    def test_use_rowid(self):
        row = {"rowid": 456, "id": 123}
        result = path_from_row_pks(row, ["id"], use_rowid=True, quote=False)
        assert result == "456"


class TestCompoundKeysAfterSql:
    """Test compound key WHERE clause generation."""

    def test_single_pk(self):
        result = compound_keys_after_sql(["id"])
        assert result == "([id] > :p0)"

    def test_two_pks(self):
        result = compound_keys_after_sql(["id", "created_at"])
        expected = "(([id] > :p0)\n  or\n([id] = :p0 and [created_at] > :p1))"
        assert result == expected

    def test_three_pks(self):
        result = compound_keys_after_sql(["pk1", "pk2", "pk3"])
        expected = (
            "(([pk1] > :p0)\n"
            "  or\n"
            "([pk1] = :p0 and [pk2] > :p1)\n"
            "  or\n"
            "([pk1] = :p0 and [pk2] = :p1 and [pk3] > :p2))"
        )
        assert result == expected

    def test_start_index(self):
        result = compound_keys_after_sql(["id", "name"], start_index=5)
        expected = "(([id] > :p5)\n  or\n([id] = :p5 and [name] > :p6))"
        assert result == expected

    def test_reserved_word_escaping(self):
        result = compound_keys_after_sql(["order", "select"])
        assert "[order]" in result
        assert "[select]" in result


class TestPaginationHelper:
    """Test the PaginationHelper class."""

    def test_init(self):
        helper = PaginationHelper(["id"], page_size=50)
        assert helper.primary_keys == ["id"]
        assert helper.page_size == 50
        assert helper.use_rowid is False

    def test_get_where_clause_no_token(self):
        helper = PaginationHelper(["id"])
        where, params = helper.get_where_clause()
        assert where == ""
        assert params == {}

    def test_get_where_clause_single_pk(self):
        helper = PaginationHelper(["id"])
        where, params = helper.get_where_clause(next_token="123")
        assert where == "([id] > :p0)"
        assert params == {"p0": "123"}

    def test_get_where_clause_compound_pks(self):
        helper = PaginationHelper(["id", "created_at"])
        where, params = helper.get_where_clause(next_token="123,2024-01-01")
        expected_where = "(([id] > :p0)\n  or\n([id] = :p0 and [created_at] > :p1))"
        assert where == expected_where
        assert params == {"p0": "123", "p1": "2024-01-01"}

    def test_get_where_clause_with_encoding(self):
        helper = PaginationHelper(["id", "path"])
        where, params = helper.get_where_clause(next_token="123,~2Ffoo~2Fbar")
        assert params == {"p0": "123", "p1": "/foo/bar"}

    def test_get_where_clause_with_sort_asc(self):
        helper = PaginationHelper(["id"])
        where, params = helper.get_where_clause(
            next_token="value123,456",
            sort_column="name",
            sort_desc=False
        )
        assert "[name] > :p0" in where
        assert "or ([name] = :p0 and ([id] > :p1))" in where
        assert params == {"p0": "value123", "p1": "456"}

    def test_get_where_clause_with_sort_desc(self):
        helper = PaginationHelper(["id"])
        where, params = helper.get_where_clause(
            next_token="value123,456",
            sort_column="name",
            sort_desc=True
        )
        assert "[name] < :p0" in where
        assert params == {"p0": "value123", "p1": "456"}

    def test_get_where_clause_with_null_sort(self):
        helper = PaginationHelper(["id"])
        where, params = helper.get_where_clause(
            next_token="$null,456",
            sort_column="name"
        )
        assert params["p0"] is None
        assert params["p1"] == "456"

    def test_get_where_clause_invalid_token_count(self):
        helper = PaginationHelper(["id", "created_at"])
        with pytest.raises(ValueError, match="Invalid next_token"):
            helper.get_where_clause(next_token="123")  # Only 1 value, need 2

    def test_get_where_clause_invalid_sorted_token(self):
        helper = PaginationHelper(["id"])
        with pytest.raises(ValueError, match="Invalid next_token for sorted query"):
            helper.get_where_clause(next_token="123", sort_column="name")

    def test_create_next_token_simple(self):
        helper = PaginationHelper(["id"])
        row = {"id": 123}
        token = helper.create_next_token(row)
        assert token == "123"

    def test_create_next_token_compound(self):
        helper = PaginationHelper(["id", "name"])
        row = {"id": 123, "name": "John Doe"}
        token = helper.create_next_token(row)
        assert token == "123,John+Doe"

    def test_create_next_token_with_sort(self):
        helper = PaginationHelper(["id"])
        row = {"id": 123}
        token = helper.create_next_token(row, sort_column="name", sort_value="Alice")
        assert token == "Alice,123"

    def test_create_next_token_with_null_sort(self):
        helper = PaginationHelper(["id"])
        row = {"id": 123}
        token = helper.create_next_token(row, sort_column="name", sort_value=None)
        assert token == "$null,123"

    def test_create_next_token_with_special_chars(self):
        helper = PaginationHelper(["id"])
        row = {"id": 123}
        token = helper.create_next_token(row, sort_column="path", sort_value="/foo/bar")
        assert token == "~2Ffoo~2Fbar,123"

    def test_has_next_page_true(self):
        helper = PaginationHelper(["id"], page_size=10)
        rows = [{"id": i} for i in range(11)]  # 11 rows, page_size is 10
        assert helper.has_next_page(rows) is True

    def test_has_next_page_false(self):
        helper = PaginationHelper(["id"], page_size=10)
        rows = [{"id": i} for i in range(10)]  # Exactly page_size
        assert helper.has_next_page(rows) is False

    def test_has_next_page_fewer_rows(self):
        helper = PaginationHelper(["id"], page_size=10)
        rows = [{"id": i} for i in range(5)]  # Fewer than page_size
        assert helper.has_next_page(rows) is False


class TestPaginationRoundtrip:
    """Test complete pagination workflows."""

    def test_simple_pagination_roundtrip(self):
        """Simulate a simple pagination scenario."""
        helper = PaginationHelper(["id"], page_size=10)

        # First page - no token
        where, params = helper.get_where_clause()
        assert where == ""

        # Simulate getting results
        page1_rows = [{"id": i} for i in range(1, 12)]  # 11 rows (page_size + 1)
        assert helper.has_next_page(page1_rows)

        # Create next token from second-to-last row
        next_token = helper.create_next_token(page1_rows[-2])
        assert next_token == "10"

        # Second page - use token
        where, params = helper.get_where_clause(next_token=next_token)
        assert where == "([id] > :p0)"
        assert params == {"p0": "10"}

    def test_compound_key_pagination_roundtrip(self):
        """Simulate pagination with compound keys."""
        helper = PaginationHelper(["user_id", "post_id"], page_size=5)

        # Simulate first page results
        page1_rows = [
            {"user_id": 1, "post_id": i} for i in range(1, 7)
        ]  # 6 rows

        assert helper.has_next_page(page1_rows)
        next_token = helper.create_next_token(page1_rows[-2])

        # Verify token
        where, params = helper.get_where_clause(next_token=next_token)
        assert params == {"p0": "1", "p1": "5"}

        # Should be able to decode and re-encode
        components = urlsafe_components(next_token)
        assert len(components) == 2

    def test_sorted_pagination_roundtrip(self):
        """Simulate pagination with sorting."""
        helper = PaginationHelper(["id"], page_size=10)

        # First page sorted by name
        page1_rows = [
            {"id": i, "name": f"User{i:03d}"} for i in range(1, 12)
        ]

        # Create token with sort value
        next_token = helper.create_next_token(
            page1_rows[-2],
            sort_column="name",
            sort_value=page1_rows[-2]["name"]
        )

        # Decode and use for next page
        where, params = helper.get_where_clause(
            next_token=next_token,
            sort_column="name"
        )

        assert params["p0"] == "User010"
        assert params["p1"] == "10"
        assert "[name] > :p0" in where


class TestSQLIntegration:
    """Test integration with actual SQL patterns."""

    def test_build_complete_query_single_pk(self):
        """Test building a complete SQL query."""
        helper = PaginationHelper(["id"], page_size=10)
        where, params = helper.get_where_clause(next_token="100")

        sql = f"SELECT * FROM users WHERE {where} ORDER BY id LIMIT {helper.page_size + 1}"
        expected = "SELECT * FROM users WHERE ([id] > :p0) ORDER BY id LIMIT 11"

        assert sql == expected
        assert params == {"p0": "100"}

    def test_build_complete_query_compound_pk(self):
        """Test building query with compound primary keys."""
        helper = PaginationHelper(["org_id", "user_id"], page_size=20)
        where, params = helper.get_where_clause(next_token="5,1000")

        sql = (
            f"SELECT * FROM members "
            f"WHERE {where} "
            f"ORDER BY org_id, user_id "
            f"LIMIT {helper.page_size + 1}"
        )

        assert "WHERE ((org_id" in sql
        assert "LIMIT 21" in sql
        assert params == {"p0": "5", "p1": "1000"}

    def test_build_query_with_additional_filters(self):
        """Test combining pagination WHERE with other filters."""
        helper = PaginationHelper(["id"], page_size=10)
        where, params = helper.get_where_clause(next_token="100")

        # Combine with additional filters
        full_where = f"status = :status AND {where}"
        params["status"] = "active"

        sql = (
            f"SELECT * FROM users "
            f"WHERE {full_where} "
            f"ORDER BY id "
            f"LIMIT {helper.page_size + 1}"
        )

        assert "status = :status" in sql
        assert "([id] > :p0)" in sql
        assert params == {"status": "active", "p0": "100"}

    def test_build_sorted_query(self):
        """Test building a sorted query with pagination."""
        helper = PaginationHelper(["id"], page_size=10)
        where, params = helper.get_where_clause(
            next_token="Smith,500",
            sort_column="last_name"
        )

        sql = (
            f"SELECT * FROM users "
            f"WHERE {where} "
            f"ORDER BY last_name, id "
            f"LIMIT {helper.page_size + 1}"
        )

        assert "last_name" in sql
        assert params == {"p0": "Smith", "p1": "500"}


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
