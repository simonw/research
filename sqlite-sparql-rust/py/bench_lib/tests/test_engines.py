import json
import os
import sqlite3
import tempfile
import time
import unittest

import gen_bsbm
import gen_social
from bench_lib.checksum import multiset_checksum, sequence_checksum
from bench_lib.datasets import open_dataset
from bench_lib.engines import (EngineSettings, EngineUnavailable, IsolatedEngine, QueryTimeout,
                               QueryUnsupported, Timed, create_engines, parse_engine_list)
from bench_lib.engines.base import result_set_from_ntriples_doc, split_ntriples_line
from bench_lib.engines.sparqlite import (BASELINE_ROWS_OPTIONS, ExtensionError, SparqliteLibrary,
                                         classify_error, decode_sparql_rows, rows_columns)
from bench_lib.queries import BenchQuery, load_queries, query_form
from bench_lib.terms import Term
from bench_lib.tests.fake_sparqlite import ALL_FUNCTIONS, FakeSparqliteConnection, connect_factory
from bench_lib.tests.test_queries import QUERY_DIR


def adhoc(text, dataset="social", ordered=False, qid="Q99"):
    return BenchQuery(id=qid, title="adhoc", dataset=dataset, ordered=ordered,
                      form=query_form(text), text=text, path="<adhoc>")


EXTRA_QUERIES = [
    adhoc("PREFIX : <http://example.org/social/> ASK { :p1 :knows ?y }"),
    adhoc("PREFIX : <http://example.org/social/> "
          "CONSTRUCT { ?x :friend ?y } WHERE { :p2 :knows ?y . BIND(:p2 AS ?x) }"),
    adhoc("SELECT ?b ?n WHERE { BIND(BNODE() AS ?b) BIND(1 AS ?n) }"),
    adhoc("PREFIX : <http://example.org/social/> "
          "SELECT ?x ?missing WHERE { :p5 :knows ?x OPTIONAL { ?x :nope ?missing } }"),
]


class Fixture(unittest.TestCase):
    """Tiny datasets on disk, a dummy extension file and engine settings."""

    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        d = cls.tmp.name
        social = os.path.join(d, "social.nt")
        bsbm = os.path.join(d, "bsbm.nt")
        gen_social.generate_file(social, persons=160, seed=3, cities=30)
        gen_bsbm.generate_file(bsbm, products=12, seed=3)
        cls.social = open_dataset("social", social)
        cls.bsbm = open_dataset("bsbm", bsbm)
        cls.ext = os.path.join(d, "libsparqlite.so")
        with open(cls.ext, "wb") as fh:
            fh.write(b"not really a library")
        cls.queries = load_queries(QUERY_DIR)

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()
        FakeSparqliteConnection.stores.clear()

    def settings(self, **kw):
        base = dict(work_dir=os.path.join(self.tmp.name, "work"), ext_path=self.ext[:-3],
                    isolate=False)
        base.update(kw)
        return EngineSettings(**base)

    def engines(self, names, connect=None, **kw):
        return create_engines(parse_engine_list(names), self.settings(**kw),
                              connect or connect_factory())

    def run_all(self, engine, dataset, queries, modes=("count", "full", "json")):
        """Decoded results per (id, text, mode); also checks that the adapter's cheap
        ``row_count`` agrees with its ``decode``."""
        engine.probe()
        engine.open(dataset)
        out = {}
        try:
            for q in queries:
                for mode in modes:
                    raw = engine.execute(q, mode)
                    value = out[(q.id, q.text, mode)] = engine.decode(q, mode, raw)
                    expected = value if isinstance(value, int) else len(value)
                    self.assertEqual(engine.row_count(q, mode, raw), expected,
                                     (engine.name, q.id, mode))
        finally:
            engine.close()
        return out


class ReferenceEnginesTest(Fixture):
    def test_pyoxigraph_and_rdflib_agree_everywhere(self):
        ox, rd = self.engines("pyoxigraph,rdflib")
        for dataset in (self.social, self.bsbm):
            qs = [q for q in self.queries if q.dataset == dataset.name]
            if dataset is self.social:
                qs += EXTRA_QUERIES
            a = self.run_all(ox, dataset, qs)
            b = self.run_all(rd, dataset, qs)
            self.assertEqual(a.keys(), b.keys())
            for key in a:
                va, vb = a[key], b[key]
                if isinstance(va, int):
                    self.assertEqual(va, vb, key[0])
                else:
                    self.assertEqual(multiset_checksum(va), multiset_checksum(vb), key)
                    if key[0] in ("Q8", "Q9", "Q17"):
                        self.assertEqual(sequence_checksum(va), sequence_checksum(vb), key)

    def test_count_variants_agree(self):
        """``aggregate`` (the engine counts) and ``iterate`` (§12.4's len(list(q))) give
        the same counts on every query and form."""
        for names in ("pyoxigraph", "rdflib"):
            (agg,) = self.engines(names)
            (it,) = self.engines(names, reference_count="iterate")
            for dataset in (self.social, self.bsbm):
                qs = [q for q in self.queries if q.dataset == dataset.name]
                if dataset is self.social:
                    qs += EXTRA_QUERIES
                a = self.run_all(agg, dataset, qs, modes=("count",))
                b = self.run_all(it, dataset, qs, modes=("count",))
                self.assertEqual(a, b, names)

    def test_aggregate_count_builds_no_solutions(self):
        """In ``aggregate`` mode pyoxigraph is asked for the count, never for the
        solutions themselves (unless the rewrite does not apply)."""
        (ox,) = self.engines("pyoxigraph")
        ox.probe()
        ox.open(self.social)
        seen = []
        real_query = ox._store.query

        class Spy:
            def query(self, text):
                seen.append(text)
                return real_query(text)

        ox._store = Spy()
        try:
            q1 = next(q for q in self.queries if q.id == "Q1")
            self.assertEqual(ox.execute(q1, "count"), 16)
            self.assertEqual(seen, [q1.count_text])
            ask = EXTRA_QUERIES[0]
            self.assertEqual(ox.execute(ask, "count"), 1)
            self.assertEqual(seen[-1], ask.text)
        finally:
            ox.close()

    def test_forms(self):
        (ox,) = self.engines("pyoxigraph")
        out = self.run_all(ox, self.social, EXTRA_QUERIES, modes=("full", "json"))
        ask = out[("Q99", EXTRA_QUERIES[0].text, "full")]
        self.assertEqual((ask.form, ask.boolean), ("ask", True))
        graph = out[("Q99", EXTRA_QUERIES[1].text, "json")]
        self.assertEqual((graph.form, len(graph.rows)), ("graph", 3))
        bnode = out[("Q99", EXTRA_QUERIES[2].text, "full")]
        self.assertEqual(bnode.rows[0][bnode.vars.index("b")], "_:")
        optional = out[("Q99", EXTRA_QUERIES[3].text, "full")]
        self.assertTrue(all(row[optional.vars.index("missing")] == "UNDEF" for row in optional.rows))

    def test_rdflib_deep_paths(self):
        # A 3000-hop :knows chain: rdflib recurses once per hop when evaluating p+.
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "chain.nt")
            with open(path, "w") as fh:
                for i in range(3000):
                    fh.write(f"<http://example.org/social/p{i}> <http://example.org/social/knows> "
                             f"<http://example.org/social/p{i + 1}> .\n")
            (rd,) = self.engines("rdflib")
            q13 = next(q for q in self.queries if q.id == "Q13")
            out = self.run_all(rd, open_dataset("social", path), [q13], modes=("full",))
        (rs,) = out.values()
        self.assertEqual(rs.rows, [("integer:2999",)])

    def test_rdflib_size_limit(self):
        (rd,) = self.engines("rdflib", rdflib_max=100)
        self.assertIn("--rdflib-max", rd.skip_reason(self.social))
        (rd,) = self.engines("rdflib")
        self.assertIsNone(rd.skip_reason(self.social))

    def test_ntriples_document_parsing(self):
        doc = ('<http://e/a> <http://e/p> "x y . z"@en .\n# comment\n\n'
               '_:b1 <http://e/q> <http://e/o> .\n<http://e/a> <http://e/p> "1"^^<http://e/t> .\n')
        rs = result_set_from_ntriples_doc(doc)
        self.assertEqual(len(rs.rows), 3)
        self.assertEqual(rs.rows[0][2], '"x y . z"@en')
        self.assertEqual(split_ntriples_line("<a>\t<b> <c> .")[1], Term.iri("b"))
        with self.assertRaises(ValueError):
            split_ntriples_line("<a> <b> <c>")


class IsolatedEngineTest(Fixture):
    SLOW = adhoc("SELECT (COUNT(*) AS ?n) WHERE { ?a ?b ?c . ?d ?e ?f . ?g ?h ?i }")

    def test_same_results_as_in_process(self):
        (inproc,) = self.engines("pyoxigraph")
        (iso,) = self.engines("pyoxigraph", isolate=True)
        self.assertIsInstance(iso, IsolatedEngine)
        self.assertEqual((iso.name, iso.short), ("pyoxigraph", "pyoxigraph"))
        qs = [q for q in self.queries if q.dataset == "social"][:6] + EXTRA_QUERIES[:2]
        expected = self.run_all(inproc, self.social, qs, modes=("count", "full"))
        iso.probe()
        info = iso.open(self.social)
        self.assertEqual(info.triples, self.social.triples)
        try:
            for q in qs:
                for mode in ("count", "full"):
                    timed = iso.execute(q, mode)
                    self.assertIsInstance(timed, Timed)
                    self.assertGreater(timed.elapsed_ms, 0)
                    got = iso.decode(q, mode, timed.raw)
                    want = expected[(q.id, q.text, mode)]
                    if isinstance(want, int):
                        self.assertEqual(got, want)
                    else:
                        self.assertEqual(multiset_checksum(got), multiset_checksum(want), q.id)
        finally:
            iso.close()
        self.assertIsNone(iso._proc)

    def test_timeout_kills_and_restarts(self):
        (iso,) = self.engines("pyoxigraph", isolate=True, timeout_s=0.5)
        iso.probe()
        iso.open(self.social)
        try:
            start = time.perf_counter()
            with self.assertRaisesRegex(QueryTimeout, "killed"):
                iso.execute(self.SLOW, "count")
            self.assertLess(time.perf_counter() - start, 10)
            self.assertIsNone(iso._proc)
            q15 = next(q for q in self.queries if q.id == "Q15")
            self.assertEqual(iso.decode(q15, "count", iso.execute(q15, "count").raw), 8)
            self.assertEqual(iso.restarts, 1)
        finally:
            iso.close()

    def test_errors_cross_the_process_boundary(self):
        (iso,) = self.engines("pyoxigraph", isolate=True)
        iso.probe()
        iso.open(self.social)
        try:
            with self.assertRaisesRegex(RuntimeError, "SyntaxError|parse|expected"):
                iso.execute(adhoc("SELECT * WHERE { ?s ?p ?o"), "count")
            q4 = next(q for q in self.queries if q.id == "Q4")
            self.assertEqual(iso.decode(q4, "count", iso.execute(q4, "count").raw), 27)
        finally:
            iso.close()


class SparqliteAdapterTest(Fixture):
    def test_engine_names(self):
        self.assertEqual(parse_engine_list("rdflib,native,ox,native"),
                         ["sparqlite-native", "pyoxigraph", "rdflib"])
        self.assertEqual(len(parse_engine_list("all")), 4)
        with self.assertRaises(ValueError):
            parse_engine_list("virtuoso")
        with self.assertRaises(ValueError):
            parse_engine_list(" , ")

    def test_native_and_baseline_match_pyoxigraph(self):
        native, baseline, ox = self.engines("native,baseline,pyoxigraph")
        qs = [q for q in self.queries if q.dataset == "social"] + EXTRA_QUERIES
        ref = self.run_all(ox, self.social, qs)
        for star in ("star", "terms"):
            (n,) = self.engines("native", full_columns=star)
            got = self.run_all(n, self.social, qs)
            for key, value in ref.items():
                if isinstance(value, int):
                    self.assertEqual(got[key], value, key)
                else:
                    self.assertEqual(multiset_checksum(got[key]), multiset_checksum(value), (star, key))
        baseline.probe()
        baseline.open(self.social)
        try:
            for q in qs:
                if ":knows+" in q.text:
                    with self.assertRaises(QueryUnsupported):
                        baseline.execute(q, "count")
                    continue
                for mode in ("count", "full"):
                    value = baseline.decode(q, mode, baseline.execute(q, mode))
                    expected = ref[(q.id, q.text, mode)]
                    if isinstance(expected, int):
                        self.assertEqual(value, expected)
                    else:
                        self.assertEqual(multiset_checksum(value), multiset_checksum(expected), q.id)
        finally:
            baseline.close()
        native.close()

    def test_database_built_once_with_spec_settings(self):
        FakeSparqliteConnection.log.clear()
        native, baseline = self.engines("native,baseline")
        native.probe()
        baseline.probe()
        info = native.open(self.bsbm)
        shared = baseline.open(self.bsbm)
        native.close()
        baseline.close()
        self.assertEqual(info.triples, self.bsbm.triples)
        self.assertEqual(info.details["load_mode"], "file")
        self.assertIn("db_bytes", info.details)
        self.assertEqual(shared.shared_with, "sparqlite-native")
        log = FakeSparqliteConnection.log
        build = log[log.index("PRAGMA page_size=8192"):]
        order = [build.index(s) for s in ("PRAGMA page_size=8192", "PRAGMA cache_size=-524288",
                                           "SELECT sparql_init()",
                                           "SELECT sparql_config('allow_files', 1)",
                                           "SELECT sparql_load_file(?, 'ntriples')", "ANALYZE main",
                                           "SELECT sparql_analyze()")]
        self.assertEqual(order, sorted(order))
        self.assertEqual(log.count("SELECT sparql_load_file(?, 'ntriples')"), 1)

    def test_blob_load_and_reuse(self):
        work = os.path.join(self.tmp.name, "reuse")
        (native,) = self.engines("native", work_dir=work, sqlite_load="blob")
        native.probe()
        first = native.open(self.social)
        native.close()
        self.assertEqual(first.details["load_mode"], "blob")
        db = native.database.path_for(self.social)
        with open(db + ".json", encoding="utf-8") as fh:
            meta = json.load(fh)
        self.assertEqual(meta["key"]["dataset"], self.social.fingerprint)
        (again,) = self.engines("native", work_dir=work, reuse_db=True)
        again.probe()
        second = again.open(self.social)
        again.close()
        self.assertTrue(second.details["reused"])
        (rebuilt,) = self.engines("native", work_dir=work)
        rebuilt.probe()
        third = rebuilt.open(self.social)
        rebuilt.close()
        self.assertNotIn("reused", third.details)

    def test_failed_build_is_an_error_and_not_retried(self):
        """A loader that fails at run time is a bug (error), not a missing piece."""
        class LoadFails(FakeSparqliteConnection):
            builds = 0

            def _scalar(self, name, args, params):
                if name == "sparql_load_file" and self.path != ":memory:":
                    LoadFails.builds += 1
                    raise sqlite3.OperationalError("sparql: RDF parse error: line 1")
                return super()._scalar(name, args, params)

        work = os.path.join(self.tmp.name, "fails")
        native, baseline = self.engines("native,baseline", connect=lambda p: LoadFails(p),
                                        work_dir=work)
        native.probe()
        for engine in (native, baseline):
            with self.assertRaisesRegex(ExtensionError, "parse error") as ctx:
                engine.open(self.social)
            self.assertNotIsInstance(ctx.exception, EngineUnavailable)
        self.assertEqual(LoadFails.builds, 1)
        self.assertEqual(os.listdir(work), [])

    def test_build_without_a_registered_function_is_unavailable(self):
        connect = connect_factory(functions=ALL_FUNCTIONS - {"sparql_analyze"})
        (native,) = self.engines("native", connect=connect, work_dir=os.path.join(self.tmp.name, "na"))
        native.probe()
        with self.assertRaisesRegex(EngineUnavailable, "no such function: sparql_analyze"):
            native.open(self.social)

    def test_bad_native_config_is_an_error(self):
        class RejectsKey(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_config" and params and params[0] == "join_polcy":
                    raise sqlite3.OperationalError("sparql: invalid argument: unknown key join_polcy")
                return super()._scalar(name, args, params)

        (native,) = self.engines("native", connect=lambda p: RejectsKey(p),
                                 native_config={"join_polcy": "hash"},
                                 work_dir=os.path.join(self.tmp.name, "cfg"))
        self.assertEqual(native.short, "native[join_polcy=hash]")
        native.probe()
        with self.assertRaisesRegex(ExtensionError, "configuring the connection failed"):
            native.open(self.social)

    def test_probe_runtime_errors_keep_modes_measurable(self):
        class CountBug(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_count":
                    raise sqlite3.OperationalError("sparql: internal error: boom")
                return super()._scalar(name, args, params)

        (native,) = self.engines("native", connect=lambda p: CountBug(p))
        native.probe()
        self.assertIsNone(native.mode_unavailable("count"))
        probe = native.library.probe()
        self.assertEqual(probe["calls"]["sparql_count"], "sparql: internal error: boom")
        self.assertNotIn("sparql_count", probe["missing"])

        class InitBug(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_init":
                    raise sqlite3.OperationalError("sparql: SQLite error 8: readonly")
                return super()._scalar(name, args, params)

        (native,) = self.engines("native", connect=lambda p: InitBug(p))
        with self.assertRaisesRegex(ExtensionError, "sparql_init\\(\\) failed"):
            native.probe()

    def test_full_mode_fetches_only_projected_terms(self):
        q15 = next(q for q in self.queries if q.id == "Q15")
        self.assertEqual(rows_columns(q15, "terms"), ["t0", "t1"])
        self.assertIsNone(rows_columns(q15, "star"))
        self.assertEqual(rows_columns(EXTRA_QUERIES[0], "terms"), ["v0"])
        self.assertEqual(rows_columns(EXTRA_QUERIES[1], "terms"), ["t0", "t1", "t2"])
        star = adhoc("SELECT * WHERE { ?s ?p ?o }")
        self.assertEqual(rows_columns(star, "terms"), ["vars"] + [f"t{k}" for k in range(16)])
        wide = adhoc("SELECT " + " ".join(f"?v{k}" for k in range(17)) + " WHERE {}")
        self.assertEqual(rows_columns(wide, "terms"), ["vars", "json"])

        FakeSparqliteConnection.log.clear()
        native, baseline = self.engines("native,baseline", work_dir=os.path.join(self.tmp.name, "sh"))
        for engine in (native, baseline):
            engine.probe()
            engine.open(self.social)
        try:
            n = native.decode(q15, "full", native.execute(q15, "full"))
            b = baseline.decode(q15, "full", baseline.execute(q15, "full"))
        finally:
            native.close()
            baseline.close()
        self.assertEqual((n.vars, len(n.rows)), (["p", "o"], 8))
        self.assertEqual(multiset_checksum(n), multiset_checksum(b))
        log = FakeSparqliteConnection.log
        self.assertIn("SELECT t0, t1 FROM sparql_rows(?)", log)
        self.assertIn("SELECT t0, t1 FROM sparql_rows(?, ?)", log)
        self.assertEqual(BASELINE_ROWS_OPTIONS, '{"engine":"sql"}')

    def test_auto_load_falls_back_to_blob(self):
        connect = connect_factory(functions=ALL_FUNCTIONS - {"sparql_load_file"})
        (native,) = self.engines("native", connect=connect, work_dir=os.path.join(self.tmp.name, "fb"))
        native.probe()
        self.assertEqual(native.open(self.social).details["load_mode"], "blob")
        native.close()

    def test_missing_pieces_degrade(self):
        no_rows = connect_factory(functions=ALL_FUNCTIONS - {"sparql_rows", "sparql_query"})
        native, baseline = self.engines("native,baseline", connect=no_rows)
        self.assertTrue(native.probe())
        self.assertIsNone(native.mode_unavailable("count"))
        self.assertIn("sparql_rows", native.mode_unavailable("full"))
        self.assertIn("sparql_query", native.mode_unavailable("json"))
        self.assertIn("sparql_rows", baseline.mode_unavailable("full"))  # same output path
        self.assertIsNone(baseline.mode_unavailable("json"))
        self.assertIsNone(baseline.mode_unavailable("count"))

        no_sql_functions = connect_factory(
            functions=ALL_FUNCTIONS - {"sparql_count_sql", "sparql_query_sql"})
        native, baseline = self.engines("native,baseline", connect=no_sql_functions)
        native.probe()
        baseline.probe()  # sparql_rows(q, '{"engine":"sql"}') still reaches the translator
        self.assertIn("sparql_count_sql", baseline.mode_unavailable("count"))
        self.assertIsNone(baseline.mode_unavailable("full"))

        no_baseline = connect_factory(
            functions=ALL_FUNCTIONS - {"sparql_count_sql", "sparql_query_sql", "sparql_rows"})
        native, baseline = self.engines("native,baseline", connect=no_baseline)
        native.probe()
        with self.assertRaisesRegex(EngineUnavailable, "no measurable mode"):
            baseline.probe()

        # The baseline rejecting the probe's one-pattern BGP (inside every subset) is a
        # broken translator, not an "unsupported" query: the engine is an error.
        rejects_all = connect_factory(baseline_rejects="?s ?p ?o")
        (baseline,) = self.engines("baseline", connect=rejects_all)
        with self.assertRaisesRegex(ExtensionError, "rejects the probe query"):
            baseline.probe()

        stub = connect_factory(functions={"sparql_version"})
        (native,) = self.engines("native", connect=stub)
        with self.assertRaisesRegex(EngineUnavailable, "sparql_init"):
            native.probe()

        (missing,) = self.engines("native", ext_path=os.path.join(self.tmp.name, "nope"))
        with self.assertRaisesRegex(EngineUnavailable, "not found"):
            missing.probe()
        (unset,) = self.engines("native", ext_path=None)
        with self.assertRaisesRegex(EngineUnavailable, "SPARQLITE_EXT"):
            unset.probe()

    def test_real_sqlite_without_extension_is_unavailable(self):
        lib = SparqliteLibrary(self.ext[:-3])  # a file that is not a shared library
        with self.assertRaises(EngineUnavailable):
            lib.connect(":memory:")
        self.assertIn("cannot load", lib.probe()["error"])

    def test_runtime_errors_are_classified(self):
        failing = connect_factory(rows_error="sparql: internal error: boom")
        (native,) = self.engines("native", connect=failing, work_dir=os.path.join(self.tmp.name, "err"))
        native.probe()
        native.open(self.social)
        try:
            q = self.queries[0]
            with self.assertRaisesRegex(RuntimeError, "boom"):
                native.execute(q, "full")
            self.assertEqual(native.decode(q, "count", native.execute(q, "count")), 16)
        finally:
            native.close()

    def test_classify_error(self):
        # (message, fired, unsupported_ok) -> outcome
        cases = [
            ("interrupted", True, False, QueryTimeout),
            ("sparql: interrupted", True, False, QueryTimeout),
            ("sparql: internal error: x", True, False, QueryTimeout),  # it did overrun
            ("interrupted", False, False, RuntimeError),  # nobody asked for an interrupt
            ("sparql: query timeout", False, False, QueryTimeout),  # timeout_ms
            ("no such function: sparql_count", False, False, EngineUnavailable),
            ("no such function: sparql_count", True, True, EngineUnavailable),
            ("no such table: sparql_rows", False, False, EngineUnavailable),
            ("no such module: sparql_view", False, False, EngineUnavailable),
            ("sparql: unsupported: baseline: MINUS", False, True, QueryUnsupported),
            ("sparql: unsupported: property path OneOrMore", False, False, RuntimeError),
            ("sparql: internal error: feature not supported", False, True, RuntimeError),
            ("sparql: internal error: x", False, True, RuntimeError),
            ("no such table: people", False, False, RuntimeError),
        ]
        for msg, fired, unsupported_ok, kind in cases:
            got = classify_error(sqlite3.OperationalError(msg), fired=fired,
                                 unsupported_ok=unsupported_ok)
            self.assertIs(type(got), kind, (msg, fired, unsupported_ok))

    def test_native_unsupported_is_an_error_baseline_unsupported_is_not(self):
        q13 = next(q for q in self.queries if q.id == "Q13")
        native, baseline = self.engines("native,baseline", connect=connect_factory(),
                                        work_dir=os.path.join(self.tmp.name, "unsup"))
        for engine in (native, baseline):
            engine.probe()
            engine.open(self.social)
        try:
            with self.assertRaises(QueryUnsupported):
                baseline.execute(q13, "full")
            native._conn.baseline_rejects = ""

            def reject(sql, params):
                raise sqlite3.OperationalError("sparql: unsupported: property path OneOrMore")

            with self.assertRaisesRegex(RuntimeError, "OneOrMore") as ctx:
                native._guarded(reject, "", ())
            self.assertNotIsInstance(ctx.exception, QueryUnsupported)
        finally:
            native.close()
            baseline.close()

    def test_timeout_interrupts_connection(self):
        (native,) = self.engines("native", timeout_s=0.01, work_dir=os.path.join(self.tmp.name, "to"))
        native.probe()
        native.open(self.social)
        conn = native._conn
        self.assertIsNone(native._guarded(time.sleep, 0.05))  # finished despite the interrupt
        self.assertEqual(conn.interrupted, 1)

        def interrupted_late():
            time.sleep(0.05)
            raise sqlite3.OperationalError("interrupted")

        with self.assertRaisesRegex(QueryTimeout, "--timeout deadline"):
            native._guarded(interrupted_late)
        self.assertEqual(conn.interrupted, 2)

        def interrupted_early():
            raise sqlite3.OperationalError("interrupted")

        with self.assertRaisesRegex(RuntimeError, "interrupted"):
            native._guarded(interrupted_early)
        time.sleep(0.03)  # an execution that ended is never interrupted afterwards
        self.assertEqual(conn.interrupted, 2)
        watchdog = native._watchdog
        native.close()
        self.assertTrue(watchdog.closed)
        self.assertTrue(conn.closed)

    def test_decode_sparql_rows_layouts(self):
        names = ["n", "v0", "v1", "t0", "t1", "id0", "id1", "vars", "json"]
        row = (1, "http://e/a", 5, "<http://e/a>", '"5"^^<http://www.w3.org/2001/XMLSchema#integer>',
               17, 22, '["x","y"]', json.dumps({"x": {"type": "uri", "value": "http://e/a"},
                                                "y": {"type": "literal", "value": "5", "datatype":
                                                      "http://www.w3.org/2001/XMLSchema#integer"}}))
        via_t = decode_sparql_rows("select", names, [row])
        no_t = [n for n in names if not n.startswith("t")]
        without_t = tuple(v for n, v in zip(names, row, strict=True) if n in no_t)
        via_json = decode_sparql_rows("select", no_t, [without_t])
        self.assertEqual(multiset_checksum(via_t), multiset_checksum(via_json))
        self.assertEqual(via_t.rows, [("<http://e/a>", "integer:5")])
        self.assertEqual(decode_sparql_rows("select", names, []).rows, [])
        self.assertTrue(decode_sparql_rows("ask", ["v0"], [(1,)]).boolean)
        with self.assertRaises(ValueError):
            decode_sparql_rows("ask", ["v0"], [])
        with self.assertRaises(ValueError):
            decode_sparql_rows("select", ["n", "vars"], [(1, '["x"]')])
        graph = decode_sparql_rows("construct", ["t0", "t1", "t2"], [("<a>", "<b>", '"c"')])
        self.assertEqual(graph.vars, ["subject", "predicate", "object"])


if __name__ == "__main__":
    unittest.main()
