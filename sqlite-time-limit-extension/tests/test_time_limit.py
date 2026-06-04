import pytest
import sqlite_time_limit


def test_execute_with_timeout_returns_rows(tmp_path):
    db_path = tmp_path / "test.db"
    sqlite_time_limit.execute_with_timeout(
        db_path.as_posix(),
        "CREATE TABLE t (id INTEGER PRIMARY KEY, value TEXT);",
        1000,
    )
    sqlite_time_limit.execute_with_timeout(
        db_path.as_posix(),
        "INSERT INTO t (value) VALUES ('alpha'), ('beta');",
        1000,
    )

    rows = sqlite_time_limit.execute_with_timeout(
        db_path.as_posix(),
        "SELECT id, value FROM t ORDER BY id;",
        1000,
    )

    assert rows == [(1, "alpha"), (2, "beta")]


def test_execute_with_timeout_raises_timeout(tmp_path):
    db_path = tmp_path / "timeout.db"
    sqlite_time_limit.execute_with_timeout(
        db_path.as_posix(),
        "CREATE TABLE t (id INTEGER PRIMARY KEY);",
        1000,
    )

    with pytest.raises(TimeoutError):
        sqlite_time_limit.execute_with_timeout(
            db_path.as_posix(),
            "WITH RECURSIVE cnt(x) AS ("
            "  SELECT 1"
            "  UNION ALL"
            "  SELECT x + 1 FROM cnt LIMIT 5000000"
            ")"
            " SELECT sum(x) FROM cnt;",
            1,
        )


def test_execute_with_timeout_rejects_negative_timeout(tmp_path):
    db_path = tmp_path / "bad.db"
    with pytest.raises(ValueError, match="timeout_ms"):
        sqlite_time_limit.execute_with_timeout(db_path.as_posix(), "SELECT 1;", -1)
