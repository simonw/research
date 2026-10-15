"""Close hook, panic safety, transactions and interrupts with the real extension."""

import os
import sqlite3
import threading
import time

import pytest

from conftest import DATA, TRIPLES, connect, scalar, status

ENDLESS = "SELECT * WHERE { ?a ?p ?b . ?c ?q ?d . ?e ?r ?f . ?g ?s ?h FILTER(?b != ?d && ?d != ?f && ?f != ?h) }"


def big(conn):
    lines = "".join(f"<http://ex/s{i}> <http://ex/p{i % 7}> <http://ex/o{i % 13}> .\n" for i in range(300))
    scalar(conn, "SELECT sparql_load(?, 'nt')", lines)
    return conn


def test_close_leaves_no_zombie(tmp_path):
    path = str(tmp_path / "wal.db")
    conn = connect(path)
    conn.execute("PRAGMA journal_mode=wal")
    scalar(conn, "SELECT sparql_load(?)", DATA)
    assert scalar(conn, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    assert list(conn.execute("SELECT count(*) FROM sparql_rows('SELECT * WHERE { ?s ?p ?o }')")) == [(TRIPLES,)]
    assert status(conn, "pool_statements") > 0
    assert os.path.exists(path + "-wal")
    conn.close()
    # The close hook finalized the pooled statements, so sqlite3_close closed the database
    # for real: the last connection checkpoints and removes the WAL file.
    assert not os.path.exists(path + "-wal")
    again = connect(path)
    assert scalar(again, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    again.close()


@pytest.mark.parametrize("site", ["xbestindex", "xopen", "xfilter", "xnext", "xcolumn"])
def test_panics_become_sql_errors(loaded, site):
    scalar(loaded, "SELECT sparql_config('debug_panic', ?)", site)
    with pytest.raises(sqlite3.OperationalError, match="panic"):
        loaded.execute("SELECT v0 FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')").fetchall()
    scalar(loaded, "SELECT sparql_config('debug_panic', 'off')")
    # The process survived and the connection still works.
    assert scalar(loaded, "SELECT count(*) FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')") == TRIPLES
    assert status(loaded, "open_cursors") == 0


def test_panic_in_a_scalar_function(loaded):
    scalar(loaded, "SELECT sparql_config('debug_panic', 'scalar')")
    with pytest.raises(sqlite3.OperationalError):
        scalar(loaded, "SELECT sparql_count('ASK {}')")
    scalar(loaded, "SELECT sparql_config('debug_panic', 'off')")
    assert scalar(loaded, "SELECT sparql_count('ASK {}')") == 1


def test_transactions(db):
    scalar(db, "SELECT sparql_init()")
    db.execute("BEGIN")
    assert scalar(db, "SELECT sparql_load(?)", DATA) == TRIPLES
    assert scalar(db, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    db.execute("ROLLBACK")
    assert scalar(db, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == 0
    # Reused dictionary rows decode as the new terms (no stale cache after the rollback).
    scalar(db, "SELECT sparql_load('<http://ex/z> <http://ex/p> \"a committed long literal\" .', 'nt')")
    assert scalar(db, "SELECT sparql_value('SELECT ?o WHERE { ?s ?p ?o }')") == "a committed long literal"
    db.execute("BEGIN")
    scalar(db, "SELECT sparql_update('INSERT DATA { <http://ex/y> <http://ex/p> 1 }')")
    db.execute("COMMIT")
    assert scalar(db, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == 2
    # A failing request is undone as a whole, the user's transaction goes on.
    db.execute("BEGIN")
    with pytest.raises(sqlite3.OperationalError):
        scalar(db, "SELECT sparql_update('INSERT DATA { <http://ex/w> <http://ex/p> 1 } ; CREATE GRAPH <http://ex/g> ; CREATE GRAPH <http://ex/g>')")
    assert db.in_transaction
    db.execute("COMMIT")
    assert scalar(db, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == 2


def test_interrupt_from_another_thread(db):
    big(db)
    timer = threading.Timer(0.2, db.interrupt)
    timer.start()
    start = time.monotonic()
    with pytest.raises(sqlite3.OperationalError, match="interrupt"):
        scalar(db, "SELECT sparql_count(?)", ENDLESS)
    timer.join()
    assert time.monotonic() - start < 20
    assert not db.in_transaction
    assert scalar(db, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == 300


def test_interrupt_stops_a_streaming_scan(db):
    big(db)
    timer = threading.Timer(0.2, db.interrupt)
    timer.start()
    with pytest.raises(sqlite3.OperationalError, match="interrupt"):
        db.execute("SELECT count(*) FROM sparql_rows(?)", (ENDLESS,)).fetchall()
    timer.join()
    assert status(db, "open_cursors") == 0


def test_timeout(db):
    big(db)
    scalar(db, "SELECT sparql_config('timeout_ms', 100)")
    with pytest.raises(sqlite3.OperationalError, match="sparql: query timeout"):
        scalar(db, "SELECT sparql_count(?)", ENDLESS)
