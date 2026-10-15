"""Fixtures for the sparqlite extension tests.

The extension is the cdylib built by `cargo build --release --manifest-path ext/Cargo.toml`.
Its path (with or without the `.so` suffix) comes from the SPARQLITE_EXT environment
variable, defaulting to ext/target/release/libsparqlite.
"""

import os
import pathlib
import sqlite3

import pytest

ROOT = pathlib.Path(__file__).resolve().parents[2]

DATA = """
@prefix ex: <http://ex/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .
ex:alice ex:name "Alice" ; ex:age 30 ; ex:knows ex:bob, ex:carol .
ex:bob ex:name "Bob" ; ex:age 25 ; ex:knows ex:carol .
ex:carol ex:name "Carol"@en ; ex:age "41"^^xsd:integer ; ex:score 2.5 .
ex:dave ex:name "Dave the very long name" ; ex:nick _:n1 .
_:n1 ex:label "nick" .
"""

TRIPLES = 13


def extension_path() -> str:
    path = os.environ.get("SPARQLITE_EXT") or str(ROOT / "ext" / "target" / "release" / "libsparqlite")
    if not (os.path.exists(path) or os.path.exists(path + ".so")):
        pytest.exit(f"sparqlite extension not found at {path} (set SPARQLITE_EXT)", returncode=2)
    return path


def connect(database=":memory:", **kwargs) -> sqlite3.Connection:
    """A connection with the extension loaded (autocommit: explicit BEGIN/COMMIT only)."""
    conn = sqlite3.connect(database, isolation_level=None, **kwargs)
    conn.enable_load_extension(True)
    conn.load_extension(extension_path())
    conn.enable_load_extension(False)
    return conn


def scalar(conn, sql, *params):
    return conn.execute(sql, params).fetchone()[0]


def status(conn, key):
    row = conn.execute("SELECT value FROM sparql_status WHERE key = ?", (key,)).fetchone()
    assert row is not None, f"no status key {key}"
    return row[0]


@pytest.fixture
def db():
    conn = connect()
    yield conn
    conn.close()


@pytest.fixture
def loaded(db):
    assert scalar(db, "SELECT sparql_load(?)", DATA) == TRIPLES
    return db
