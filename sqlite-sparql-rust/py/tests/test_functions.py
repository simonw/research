"""Smoke test of every SQL function of the extension, loaded into Python's sqlite3."""

import json
import sqlite3

import pytest

from conftest import DATA, TRIPLES, connect, scalar, status

NAMES = "SELECT ?n WHERE { ?s <http://ex/name> ?n } ORDER BY ?n"


def test_host_and_version(db):
    assert sqlite3.sqlite_version == "3.45.1"
    assert scalar(db, "SELECT sparql_version()").startswith("sparqlite ")
    assert scalar(db, "SELECT sparql_version()").endswith("(SQLite 3.45.1)")


def test_init_and_load_formats(db):
    assert scalar(db, "SELECT sparql_init()") == 1
    assert scalar(db, "SELECT sparql_init()") == 1
    assert scalar(db, "SELECT sparql_load(?, 'nt')", '<http://ex/a> <http://ex/p> "1" .\n') == 1
    assert scalar(db, "SELECT sparql_load(?, 'turtle', 'http://base/', 'http://ex/g')", b"<a> <p> <b> .") == 1
    assert scalar(db, "SELECT sparql_load(?, 'nquads')", "<http://ex/s> <http://ex/p> <http://ex/o> <http://ex/g2> .\n") == 1
    assert scalar(db, "SELECT sparql_load(?, 'trig')", "<http://ex/g3> { <http://ex/s> <http://ex/p> 3 }") == 1
    assert scalar(db, "SELECT sparql_load(?, 'jsonld')", '{"@id": "http://ex/j", "http://ex/p": "json"}') == 1
    assert scalar(db, "SELECT sparql_ask('ASK { GRAPH <http://ex/g> { <http://base/a> <http://base/p> <http://base/b> } }')") == 1
    with pytest.raises(sqlite3.OperationalError, match="^sparql: RDF parse error"):
        scalar(db, "SELECT sparql_load('<a> <b> .', 'nt')")
    with pytest.raises(sqlite3.OperationalError, match="unknown RDF format"):
        scalar(db, "SELECT sparql_load('', 'yaml')")


def test_load_file_needs_allow_files(db, tmp_path):
    f = tmp_path / "data.ttl"
    f.write_text(DATA)
    with pytest.raises(sqlite3.OperationalError, match="allow_files"):
        scalar(db, "SELECT sparql_load_file(?)", str(f))
    assert scalar(db, "SELECT sparql_config('allow_files', 1)") == "1"
    assert scalar(db, "SELECT sparql_load_file(?)", str(f)) == TRIPLES


def test_query_formats(loaded):
    result = json.loads(scalar(loaded, "SELECT sparql_query(?)", NAMES))
    assert [b["n"]["value"] for b in result["results"]["bindings"]] == ["Alice", "Bob", "Dave the very long name", "Carol"]
    assert scalar(loaded, "SELECT sparql_query(?, 'csv')", NAMES).startswith("n\r\nAlice\r\n")
    assert scalar(loaded, "SELECT sparql_query(?, 'tsv')", NAMES).startswith('?n\n"Alice"\n')
    assert "<literal>Alice</literal>" in scalar(loaded, "SELECT sparql_query(?, 'xml')", NAMES)
    assert json.loads(scalar(loaded, "SELECT sparql_query('ASK { ?s ?p ?o }')"))["boolean"] is True
    construct = "CONSTRUCT { ?s <http://ex/n> ?n } WHERE { ?s <http://ex/name> ?n FILTER(?n = 'Bob') }"
    assert scalar(loaded, "SELECT sparql_query(?)", construct) == '<http://ex/bob> <http://ex/n> "Bob" .\n'
    assert "<http://ex/bob>" in scalar(loaded, "SELECT sparql_query(?, 'turtle')", construct)
    with pytest.raises(sqlite3.OperationalError, match="^sparql: syntax error"):
        scalar(loaded, "SELECT sparql_query('SELEKT')")
    with pytest.raises(sqlite3.OperationalError, match="not a SELECT result format"):
        scalar(loaded, "SELECT sparql_query(?, 'ntriples')", NAMES)


def test_ask_count_value_and_native_types(loaded):
    assert scalar(loaded, "SELECT sparql_ask('ASK { <http://ex/alice> <http://ex/knows> <http://ex/bob> }')") == 1
    assert scalar(loaded, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    value = lambda q: scalar(loaded, "SELECT sparql_value(?)", q)  # noqa: E731
    assert value("SELECT ?a WHERE { <http://ex/alice> <http://ex/age> ?a }") == 30
    assert value("SELECT ?a WHERE { <http://ex/carol> <http://ex/score> ?a }") == 2.5
    assert value("SELECT ?n WHERE { <http://ex/carol> <http://ex/name> ?n }") == "Carol"
    assert value("SELECT ?s WHERE { ?s <http://ex/age> 25 }") == "http://ex/bob"
    assert value("SELECT ?x WHERE { BIND(false AS ?x) }") == 0
    assert value("SELECT ?x WHERE { ?s <http://ex/nothing> ?x }") is None


def test_update_explain_analyze_vacuum_check(loaded):
    assert scalar(loaded, "SELECT sparql_update('INSERT DATA { <http://ex/x> <http://ex/p> 1 }')") == 1
    assert scalar(loaded, "SELECT sparql_update('DELETE WHERE { <http://ex/x> ?p ?o }')") == 1
    q = "SELECT ?n WHERE { ?s <http://ex/name> ?n ; <http://ex/age> ?a FILTER(?a > 20) }"
    for mode in ("algebra", "logical", "plan", "analyze", "sql", "sqlplan"):
        assert scalar(loaded, "SELECT sparql_explain(?, ?)", q, mode), mode
    stats = json.loads(scalar(loaded, "SELECT sparql_analyze()"))
    assert stats["analyzed_quads"] == TRIPLES
    scalar(loaded, "SELECT sparql_update('DELETE WHERE { ?s <http://ex/name> \"Dave the very long name\" }')")
    # <http://ex/x>, <http://ex/p> and Dave's name are no longer referenced.
    assert scalar(loaded, "SELECT sparql_vacuum_terms()") == 3
    assert scalar(loaded, "SELECT sparql_check()").startswith("ok:")


def test_config(db):
    assert scalar(db, "SELECT sparql_config('engine')") == "native"
    assert scalar(db, "SELECT sparql_config('engine', 'sql')") == "sql"
    assert scalar(db, "SELECT sparql_config('timeout_ms', 500)") == "500"
    assert status(db, "config.timeout_ms") == 500
    with pytest.raises(sqlite3.OperationalError, match="unknown sparql_config key"):
        scalar(db, "SELECT sparql_config('bogus')")


def test_baseline_functions(loaded):
    q = "SELECT ?s ?n WHERE { ?s <http://ex/name> ?n ; <http://ex/age> ?a FILTER(?a >= 25) } ORDER BY ?n"
    assert scalar(loaded, "SELECT sparql_query_sql(?)", q) == scalar(loaded, "SELECT sparql_query(?)", q)
    assert scalar(loaded, "SELECT sparql_count_sql(?)", q) == 3
    assert "main.rdf_quad" in scalar(loaded, "SELECT sparql_to_sql(?)", q)


def test_term_and_helper_functions(loaded):
    alice = scalar(loaded, "SELECT sparql_term_id('<http://ex/alice>')")
    thirty = scalar(loaded, "SELECT sparql_term_id('30')")
    carol = scalar(loaded, "SELECT sparql_term_id('\"Carol\"@en')")
    assert scalar(loaded, "SELECT sparql_term_id('<http://ex/nobody>')") is None
    assert scalar(loaded, "SELECT sparql_term(?)", alice) == "<http://ex/alice>"
    assert scalar(loaded, "SELECT sparql_term_value(?)", thirty) == 30
    assert scalar(loaded, "SELECT sparql_term_kind(?)", alice) == "iri"
    assert scalar(loaded, "SELECT sparql_term_lex(?)", carol) == "Carol"
    assert scalar(loaded, "SELECT sparql_term_lang(?)", carol) == "en"
    assert scalar(loaded, "SELECT sparql_term_datatype(?)", thirty) == "http://www.w3.org/2001/XMLSchema#integer"
    assert scalar(loaded, "SELECT sparql_term(0)") is None
    assert scalar(loaded, "SELECT sparql_num(?)", thirty) == 30
    assert scalar(loaded, "SELECT sparql_str(?)", carol) == "Carol"
    assert scalar(loaded, "SELECT sparql_ebv(?)", thirty) == 1
    assert isinstance(scalar(loaded, "SELECT sparql_sortkey(?)", thirty), bytes)
    assert scalar(loaded, "SELECT sparql_regex(?, ?)", carol, scalar(loaded, "SELECT sparql_term_id('\"^C\"')")) == 1
    total = scalar(loaded, "SELECT sparql_eval('+', ?, ?)", thirty, thirty)
    assert scalar(loaded, "SELECT sparql_term_value(?)", total) == 60
    loaded.execute("CREATE TEMP TABLE ids(x INTEGER)")
    loaded.executemany("INSERT INTO ids VALUES (?)", [(thirty,), (thirty,)])
    assert scalar(loaded, "SELECT sparql_term_value(sparql_sum(x)) FROM ids") == 60
    assert scalar(loaded, "SELECT sparql_term_value(sparql_avg(x)) FROM ids") == 30
    assert scalar(loaded, "SELECT sparql_max(x) = sparql_min(x) FROM ids") == 1
    assert scalar(loaded, "SELECT sparql_sample(x) FROM ids") == thirty
    assert scalar(loaded, "SELECT sparql_group_concat(x, ?) FROM ids", scalar(loaded, "SELECT sparql_term_id('\",\"')")) is not None


def test_directonly_functions_are_rejected_in_triggers_and_views(loaded):
    loaded.executescript(
        """
        CREATE TABLE inbox(q TEXT);
        CREATE TRIGGER apply AFTER INSERT ON inbox BEGIN SELECT sparql_update(NEW.q); END;
        CREATE VIEW loader AS SELECT sparql_load('<http://ex/a> <http://ex/b> <http://ex/c> .', 'nt') AS n;
        CREATE VIEW counter AS SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }') AS n;
        """
    )
    with pytest.raises(sqlite3.OperationalError, match=r"unsafe use of sparql_update\(\)"):
        loaded.execute("INSERT INTO inbox VALUES ('INSERT DATA { <http://ex/a> <http://ex/b> 1 }')")
    with pytest.raises(sqlite3.OperationalError, match=r"unsafe use of sparql_load\(\)"):
        loaded.execute("SELECT n FROM loader").fetchall()
    assert scalar(loaded, "SELECT n FROM counter") == TRIPLES


def test_every_connection_needs_the_extension(tmp_path):
    path = str(tmp_path / "x.db")
    a = connect(path)
    scalar(a, "SELECT sparql_load(?)", DATA)
    plain = sqlite3.connect(path)
    with pytest.raises(sqlite3.OperationalError, match="no such function: sparql_count"):
        plain.execute("SELECT sparql_count('ASK {}')")
    assert plain.execute("SELECT count(*) FROM rdf_quad").fetchone()[0] == TRIPLES
    plain.close()
    b = connect(path)
    assert scalar(b, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    # Loading twice on one connection replaces the registration cleanly.
    b.enable_load_extension(True)
    from conftest import extension_path

    b.load_extension(extension_path())
    assert scalar(b, "SELECT sparql_count('SELECT * WHERE { ?s ?p ?o }')") == TRIPLES
    a.close()
    b.close()
