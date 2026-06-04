"""Tests for the syntaqlite Python C extension module."""

import pytest
import syntaqlite


class TestParse:
    def test_parse_simple_select(self):
        stmts = syntaqlite.parse("SELECT 1")
        assert len(stmts) == 1
        assert stmts[0]["ok"] is True
        assert "ast" in stmts[0]
        assert stmts[0]["ast"]  # non-empty string

    def test_parse_multiple_statements(self):
        stmts = syntaqlite.parse("SELECT 1; SELECT 2")
        assert len(stmts) == 2
        assert stmts[0]["ok"] is True
        assert stmts[1]["ok"] is True

    def test_parse_syntax_error(self):
        stmts = syntaqlite.parse("SELECT FROM")
        assert len(stmts) == 1
        assert stmts[0]["ok"] is False
        assert "error" in stmts[0]
        assert stmts[0]["error"]  # non-empty error message

    def test_parse_empty_input(self):
        stmts = syntaqlite.parse("")
        assert len(stmts) == 0

    def test_parse_semicolons_only(self):
        stmts = syntaqlite.parse(";;;")
        assert len(stmts) == 0

    def test_parse_create_table(self):
        stmts = syntaqlite.parse("CREATE TABLE foo (id INTEGER PRIMARY KEY, name TEXT)")
        assert len(stmts) == 1
        assert stmts[0]["ok"] is True

    def test_parse_complex_query(self):
        sql = """
        SELECT u.id, u.name, COUNT(o.id) AS order_count
        FROM users u
        LEFT JOIN orders o ON u.id = o.user_id
        WHERE u.active = 1
        GROUP BY u.id, u.name
        HAVING COUNT(o.id) > 5
        ORDER BY order_count DESC
        LIMIT 10
        """
        stmts = syntaqlite.parse(sql)
        assert len(stmts) == 1
        assert stmts[0]["ok"] is True


class TestFormatSql:
    def test_format_simple(self):
        result = syntaqlite.format_sql("select 1")
        assert "SELECT" in result
        assert "1" in result

    def test_format_lowercases_keywords(self):
        result = syntaqlite.format_sql("SELECT 1", keyword_case="lower")
        assert "select" in result

    def test_format_uppercases_keywords(self):
        result = syntaqlite.format_sql("select 1 from foo", keyword_case="upper")
        assert "SELECT" in result
        assert "FROM" in result

    def test_format_with_semicolons(self):
        result = syntaqlite.format_sql("select 1", semicolons=True)
        assert result.rstrip().endswith(";")

    def test_format_without_semicolons(self):
        result = syntaqlite.format_sql("select 1;", semicolons=False)
        assert not result.rstrip().endswith(";")

    def test_format_multiple_statements(self):
        result = syntaqlite.format_sql("select 1; select 2")
        assert result.count("SELECT") == 2 or result.count("select") == 2

    def test_format_preserves_semantics(self):
        sql = "select a,b from t where x=1 and y=2"
        result = syntaqlite.format_sql(sql)
        # Check key elements are preserved
        for token in ["a", "b", "t", "x", "1", "y", "2"]:
            assert token in result

    def test_format_syntax_error_raises(self):
        with pytest.raises(syntaqlite.FormatError):
            syntaqlite.format_sql("SELECT FROM WHERE")

    def test_format_default_config(self):
        result = syntaqlite.format_sql("select 1")
        # Default should uppercase keywords
        assert "SELECT" in result

    def test_format_custom_indent(self):
        sql = "select a from t where x = 1 and y = 2 and z = 3 and w = 4"
        result = syntaqlite.format_sql(sql, indent_width=4, line_width=30)
        # With narrow line width, should have some indentation
        assert "\n" in result


class TestValidate:
    def test_validate_valid_sql(self):
        diags = syntaqlite.validate("SELECT 1")
        assert isinstance(diags, list)

    def test_validate_unknown_table_with_schema(self):
        diags = syntaqlite.validate(
            "SELECT id FROM usr",
            tables=[{"name": "users", "columns": ["id", "name"]}],
        )
        assert len(diags) > 0
        assert any(d["severity"] == "error" for d in diags)
        assert any("usr" in d["message"] for d in diags)

    def test_validate_known_table(self):
        diags = syntaqlite.validate(
            "SELECT id FROM users",
            tables=[{"name": "users", "columns": ["id", "name"]}],
        )
        # No errors for valid reference
        errors = [d for d in diags if d["severity"] == "error"]
        assert len(errors) == 0

    def test_validate_did_you_mean(self):
        # The "did you mean?" suggestion appears in rendered output
        rendered = syntaqlite.validate(
            "SELECT id FROM usr",
            tables=[{"name": "users", "columns": ["id", "name"]}],
            render=True,
        )
        assert "did you mean" in rendered.lower()
        assert "users" in rendered

    def test_validate_returns_offsets(self):
        diags = syntaqlite.validate(
            "SELECT id FROM usr",
            tables=[{"name": "users", "columns": ["id", "name"]}],
        )
        assert len(diags) > 0
        d = diags[0]
        assert "start_offset" in d
        assert "end_offset" in d
        assert isinstance(d["start_offset"], int)
        assert isinstance(d["end_offset"], int)

    def test_validate_multiple_tables(self):
        diags = syntaqlite.validate(
            "SELECT u.id, o.total FROM users u JOIN orders o ON u.id = o.user_id",
            tables=[
                {"name": "users", "columns": ["id", "name"]},
                {"name": "orders", "columns": ["id", "user_id", "total"]},
            ],
        )
        errors = [d for d in diags if d["severity"] == "error"]
        assert len(errors) == 0

    def test_validate_render_diagnostics(self):
        rendered = syntaqlite.validate(
            "SELECT id FROM usr",
            tables=[{"name": "users", "columns": ["id", "name"]}],
            render=True,
        )
        assert isinstance(rendered, str)
        # Rendered diagnostics should contain the error
        assert "usr" in rendered

    def test_validate_empty_input(self):
        diags = syntaqlite.validate("")
        assert isinstance(diags, list)
        assert len(diags) == 0

    def test_validate_no_schema(self):
        # Without schema, unknown tables are warnings not errors
        diags = syntaqlite.validate("SELECT id FROM unknown_table")
        # Should still parse and analyze
        assert isinstance(diags, list)


class TestTokenize:
    def test_tokenize_simple(self):
        tokens = syntaqlite.tokenize("SELECT 1")
        assert len(tokens) > 0

    def test_tokenize_returns_text(self):
        tokens = syntaqlite.tokenize("SELECT 1")
        texts = [t["text"] for t in tokens]
        assert "SELECT" in texts
        assert "1" in texts

    def test_tokenize_returns_offsets(self):
        tokens = syntaqlite.tokenize("SELECT 1")
        for t in tokens:
            assert "offset" in t
            assert "length" in t
            assert "type" in t
            assert isinstance(t["offset"], int)
            assert isinstance(t["length"], int)
            assert isinstance(t["type"], int)

    def test_tokenize_includes_whitespace(self):
        tokens = syntaqlite.tokenize("SELECT 1")
        texts = [t["text"] for t in tokens]
        assert " " in texts

    def test_tokenize_multistatement(self):
        tokens = syntaqlite.tokenize("SELECT 1; SELECT 2")
        texts = [t["text"] for t in tokens]
        assert texts.count("SELECT") == 2

    def test_tokenize_empty(self):
        tokens = syntaqlite.tokenize("")
        assert len(tokens) == 0

    def test_tokenize_comments(self):
        tokens = syntaqlite.tokenize("-- comment\nSELECT 1")
        texts = [t["text"] for t in tokens]
        assert any("comment" in t for t in texts)


class TestModule:
    def test_has_format_error(self):
        assert hasattr(syntaqlite, "FormatError")
        assert issubclass(syntaqlite.FormatError, Exception)

    def test_has_all_functions(self):
        assert callable(syntaqlite.parse)
        assert callable(syntaqlite.format_sql)
        assert callable(syntaqlite.validate)
        assert callable(syntaqlite.tokenize)
