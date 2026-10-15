"""sparql_rows, sparql_view and sparql_status through Python's sqlite3."""

import json
import sqlite3

import pytest

from conftest import TRIPLES, scalar, status


def test_sparql_rows_columns(loaded):
    rows = loaded.execute(
        "SELECT n, v0, v1, t1, id0 IS NOT NULL, vars, json "
        "FROM sparql_rows('SELECT ?s ?a WHERE { ?s <http://ex/age> ?a } ORDER BY ?a')"
    ).fetchall()
    assert [r[0] for r in rows] == [1, 2, 3]
    assert [(r[1], r[2]) for r in rows] == [("http://ex/bob", 25), ("http://ex/alice", 30), ("http://ex/carol", 41)]
    assert rows[0][3] == '"25"^^<http://www.w3.org/2001/XMLSchema#integer>'
    assert rows[0][4] == 1
    assert json.loads(rows[0][5]) == ["s", "a"]
    assert json.loads(rows[0][6])["s"] == {"type": "uri", "value": "http://ex/bob"}


def test_count_star_decodes_nothing(loaded):
    before = status(loaded, "terms_decoded"), status(loaded, "decode_calls")
    assert scalar(loaded, "SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    assert (status(loaded, "terms_decoded"), status(loaded, "decode_calls")) == before


def test_sparql_rows_options_and_forms(loaded):
    q = "SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n"
    native = loaded.execute("SELECT v0 FROM sparql_rows(?)", (q,)).fetchall()
    baseline = loaded.execute("SELECT v0 FROM sparql_rows(?, ?)", (q, '{"engine": "sql"}')).fetchall()
    assert native == baseline and len(native) == 4
    assert loaded.execute("SELECT v0 FROM sparql_rows('ASK { ?s ?p ?o }')").fetchall() == [(1,)]
    triples = loaded.execute(
        "SELECT v0, v1, v2 FROM sparql_rows('CONSTRUCT { ?s <http://ex/k> ?o } WHERE { ?s <http://ex/knows> ?o }')"
    ).fetchall()
    assert len(triples) == 3 and all(t[1] == "http://ex/k" for t in triples)


def test_sparql_rows_streams_a_large_result(db):
    lines = "".join(f'<http://ex/s{i}> <http://ex/v> "value {i}" .\n' for i in range(50_000))
    assert scalar(db, "SELECT sparql_load(?, 'nt')", lines) == 50_000
    cur = db.execute("SELECT n, v1 FROM sparql_rows('SELECT ?s ?v WHERE { ?s <http://ex/v> ?v }')")
    seen = 0
    while True:
        batch = cur.fetchmany(5000)
        if not batch:
            break
        for n, v in batch:
            seen += 1
            assert n == seen and v.startswith("value ")
    assert seen == 50_000
    assert status(db, "open_cursors") == 0


def test_writes_are_refused_while_a_scan_is_open(loaded):
    cur = loaded.execute("SELECT v0 FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')")
    cur.fetchone()
    with pytest.raises(sqlite3.OperationalError, match="sparql_rows cursor is open"):
        scalar(loaded, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')")
    cur.fetchall()
    assert scalar(loaded, "SELECT sparql_update('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')") == 1


def test_sparql_view(loaded):
    loaded.execute(
        """CREATE VIRTUAL TABLE temp.people USING sparql_view(
             'SELECT ?person ?name ?age WHERE { ?person <http://ex/name> ?name OPTIONAL { ?person <http://ex/age> ?age } }',
             timeout_ms=2000)"""
    )
    rows = loaded.execute("SELECT name, age FROM people ORDER BY name").fetchall()
    assert rows == [("Alice", 30), ("Bob", 25), ("Carol", 41), ("Dave the very long name", None)]
    assert loaded.execute("SELECT name FROM people WHERE person__term = '<http://ex/bob>'").fetchall() == [("Bob",)]
    bob = scalar(loaded, "SELECT sparql_term_id('<http://ex/bob>')")
    assert loaded.execute("SELECT age FROM people WHERE person__id = ?", (bob,)).fetchall() == [(25,)]
    assert loaded.execute("SELECT * FROM people WHERE person__term = '<http://ex/nobody>'").fetchall() == []
    loaded.execute("DROP TABLE people")


def test_sparql_status(db):
    rows = dict(db.execute("SELECT key, value FROM sparql_status").fetchall())
    assert rows["armed"] == 1
    assert rows["version"]
    assert rows["sqlite_version"] == "3.45.1"
    assert rows["config.engine"] == "native"
    assert rows["open_cursors"] == 0
