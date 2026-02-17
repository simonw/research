"""Tests for the hamming_topk virtual table extension."""

import os
import sqlite3
import pytest

EXTENSION_PATH = os.path.join(os.path.dirname(__file__), "hamming_vtab")


def get_conn(num_rows=100, vec_size=128):
    """Create an in-memory DB with source table and virtual table."""
    conn = sqlite3.connect(":memory:")
    conn.enable_load_extension(True)
    conn.load_extension(EXTENSION_PATH)

    conn.execute("""
        CREATE TABLE documents (
            rowid INTEGER PRIMARY KEY,
            embedding BLOB NOT NULL
        )
    """)

    rows = [(i + 1, os.urandom(vec_size)) for i in range(num_rows)]
    conn.executemany(
        "INSERT INTO documents (rowid, embedding) VALUES (?, ?)", rows
    )
    conn.commit()

    conn.execute("""
        CREATE VIRTUAL TABLE search USING hamming_topk(documents, embedding)
    """)

    return conn, rows


class TestVtabBasic:

    def test_returns_results(self):
        conn, rows = get_conn(100)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ?",
            (query,),
        ).fetchall()
        assert len(results) == 10  # default k=10

    def test_custom_k(self):
        conn, rows = get_conn(100)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 5",
            (query,),
        ).fetchall()
        assert len(results) == 5

    def test_k_larger_than_table(self):
        conn, rows = get_conn(5)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 20",
            (query,),
        ).fetchall()
        assert len(results) == 5  # only 5 rows exist

    def test_results_sorted_by_distance(self):
        conn, rows = get_conn(1000)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 20",
            (query,),
        ).fetchall()
        distances = [r[1] for r in results]
        assert distances == sorted(distances)

    def test_distances_are_non_negative(self):
        conn, rows = get_conn(100)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ?",
            (query,),
        ).fetchall()
        for _, dist in results:
            assert dist >= 0

    def test_identical_vector_distance_zero(self):
        """If we search for a vector that's in the table, distance=0."""
        conn = sqlite3.connect(":memory:")
        conn.enable_load_extension(True)
        conn.load_extension(EXTENSION_PATH)

        known = b"\xAB" * 128
        conn.execute("""
            CREATE TABLE docs (rowid INTEGER PRIMARY KEY, embedding BLOB NOT NULL)
        """)
        conn.execute("INSERT INTO docs (rowid, embedding) VALUES (1, ?)", (known,))
        conn.execute("""
            CREATE VIRTUAL TABLE s USING hamming_topk(docs, embedding)
        """)
        conn.commit()

        results = conn.execute(
            "SELECT source_rowid, distance FROM s WHERE query = ? AND k = 1",
            (known,),
        ).fetchall()
        assert len(results) == 1
        assert results[0] == (1, 0)


class TestVtabMatchesScalar:

    def test_top10_matches_scalar_function(self):
        """The virtual table top-10 should match a brute-force scalar sort."""
        conn, rows = get_conn(1000)
        query = os.urandom(128)

        # Virtual table result
        vtab_results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 10",
            (query,),
        ).fetchall()

        # Scalar function brute force
        scalar_results = conn.execute(
            "SELECT rowid, hamming_distance(?, embedding) as dist "
            "FROM documents ORDER BY dist, rowid LIMIT 10",
            (query,),
        ).fetchall()

        # Compare sets of (rowid, distance)
        assert set(vtab_results) == set(scalar_results)

    def test_top50_matches_scalar(self):
        conn, rows = get_conn(500)
        query = os.urandom(128)

        vtab_results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 50",
            (query,),
        ).fetchall()

        scalar_results = conn.execute(
            "SELECT rowid, hamming_distance(?, embedding) as dist "
            "FROM documents ORDER BY dist, rowid LIMIT 50",
            (query,),
        ).fetchall()

        assert set(vtab_results) == set(scalar_results)


class TestVtabEdgeCases:

    def test_empty_table(self):
        conn, _ = get_conn(0)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ?",
            (query,),
        ).fetchall()
        assert results == []

    def test_k_equals_1(self):
        conn, _ = get_conn(100)
        query = os.urandom(128)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = ? AND k = 1",
            (query,),
        ).fetchall()
        assert len(results) == 1

    def test_null_query_returns_nothing(self):
        conn, _ = get_conn(10)
        results = conn.execute(
            "SELECT source_rowid, distance FROM search WHERE query = NULL",
        ).fetchall()
        assert results == []

    def test_scalar_function_also_available(self):
        """The vtab extension also registers hamming_distance()."""
        conn, _ = get_conn(0)
        result = conn.execute(
            "SELECT hamming_distance(X'FF', X'00')"
        ).fetchone()[0]
        assert result == 8
