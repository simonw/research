"""Close hook, panic safety, transactions and interrupts with the real extension."""

import os
import sqlite3
import subprocess
import sys
import textwrap
import threading
import time

import pytest

from conftest import DATA, DEBUG_HOOKS, TRIPLES, connect, extension_path, scalar, status

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


def run_isolated(script, env=None):
    """Runs `script` in a fresh Python process that loads the extension; returns it finished."""
    prologue = textwrap.dedent(f"""
        import sqlite3
        def connect(path=":memory:"):
            conn = sqlite3.connect(path, isolation_level=None)
            conn.enable_load_extension(True)
            conn.load_extension({extension_path()!r})
            return conn
    """)
    return subprocess.run(
        [sys.executable, "-c", prologue + textwrap.dedent(script)],
        env=env if env is not None else dict(os.environ),
        capture_output=True,
        text=True,
        timeout=300,
    )


def test_debug_hook_is_off_without_its_switch():
    env = {k: v for k, v in os.environ.items() if k != DEBUG_HOOKS}
    done = run_isolated(
        """
        conn = connect()
        try:
            conn.execute("SELECT sparql_config('debug_panic', 'xeof')")
            raise SystemExit("the hook was armed")
        except sqlite3.OperationalError as e:
            assert "testing hook" in str(e), e
        conn.execute("SELECT sparql_load(?, 'nt')", ("".join(f"<http://ex/s{i}> <http://ex/p> <http://ex/o> .\\n" for i in range(5000)),))
        n = conn.execute("SELECT count(*) FROM sparql_rows('SELECT ?s WHERE { ?s ?p ?o }')").fetchone()[0]
        assert n == 5000, n
        print("ok")
        """,
        env,
    )
    assert done.returncode == 0 and done.stdout.strip() == "ok", done.stderr
    assert "panicked" not in done.stderr


def test_deeply_nested_requests_do_not_crash_the_host():
    # Each of these overflowed the stack (SIGSEGV) before the nesting check; now they are
    # refused with an error and the process lives on.
    done = run_isolated(
        """
        conn = connect()
        conn.execute("SELECT sparql_init()")
        requests = [
            ("SELECT sparql_query(?)", "SELECT * WHERE { ?s ?p ?o FILTER(" + "(" * 5000 + "?o" + ")" * 5000 + ") }"),
            ("SELECT sparql_count(?)", "SELECT * WHERE " + "{ " * 5000 + "?s ?p ?o" + " }" * 5000),
            ("SELECT count(*) FROM sparql_rows(?)", "SELECT * WHERE { ?s ?p ?o FILTER(?o" + " + 1" * 20000 + " > 0) }"),
            ("SELECT sparql_update(?)", "INSERT DATA { <http://ex/a> <http://ex/p> " + "( " * 100000 + "1" + " )" * 100000 + " }"),
        ]
        for sql, text in requests:
            try:
                conn.execute(sql, (text,)).fetchall()
                raise SystemExit(f"accepted: {sql}")
            except sqlite3.OperationalError as e:
                assert "nests too deeply" in str(e), e
        print("ok")
        """
    )
    assert done.returncode == 0 and done.stdout.strip() == "ok", (done.returncode, done.stderr[-2000:])


def test_a_temp_view_scan_leaves_autocommit_writes_alone(tmp_path):
    path = str(tmp_path / "view.db")
    conn = connect(path)
    scalar(conn, "SELECT sparql_load(?)", DATA)
    conn.execute("CREATE VIRTUAL TABLE temp.people USING sparql_view('SELECT ?n WHERE { ?s <http://ex/name> ?n }')")
    conn.execute("CREATE TABLE names(n)")
    cur = conn.execute("SELECT n FROM people")
    assert cur.fetchone() is not None
    # No savepoint is held between rows: the connection is still in autocommit mode.
    assert not conn.in_transaction
    conn.execute("INSERT INTO names VALUES ('written mid-scan')")
    other = sqlite3.connect(path)
    assert other.execute("SELECT count(*) FROM names").fetchone() == (1,)
    other.close()
    conn.execute("BEGIN")
    conn.execute("COMMIT")
    # Closing with the cursor still open loses nothing.
    conn.close()
    again = sqlite3.connect(path)
    assert again.execute("SELECT count(*) FROM names").fetchone() == (1,)
    again.close()


def test_an_update_inside_insert_select(loaded):
    loaded.execute("CREATE TEMP TABLE r(n)")
    loaded.execute("INSERT INTO r SELECT sparql_update('INSERT DATA { <http://ex/x> <http://ex/p> 1 }')")
    assert loaded.execute("SELECT n FROM r").fetchall() == [(1,)]
    assert scalar(loaded, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES + 1
    with pytest.raises(sqlite3.OperationalError, match="own SELECT statement"):
        loaded.execute("INSERT INTO r SELECT sparql_load('<http://ex/a> <http://ex/b> <http://ex/c> .')")
    assert not loaded.in_transaction
