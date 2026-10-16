"""Adversarial review tests (WP13a review).  Each ``test_review_*`` pins a defect found in
review; they fail until the harness is fixed."""

import contextlib
import io
import json
import os
import sqlite3
import statistics
import tempfile
import time
import unittest

import gen_social
from bench_lib.cli import build_parser, run_benchmark
from bench_lib.engines import EngineSettings
from bench_lib.engines.sparqlite import (
    SparqliteDatabase,
    SparqliteLibrary,
    SparqliteNative,
)
from bench_lib.queries import BenchQuery
from bench_lib.runner import ERROR, RunConfig, Runner
from bench_lib.terms import canonical_nt
from bench_lib.tests.fake_sparqlite import FakeSparqliteConnection


class ReviewTimingTest(unittest.TestCase):
    def test_review_sqlite_timeout_guard_is_not_timed(self):
        """The SQLite adapters start a ``threading.Timer`` per execution *inside* the timed
        region (``Runner._timed`` -> ``execute`` -> ``_deadline``).  Starting an OS thread
        costs ~0.3 ms, more than pyoxigraph's whole Q15 (0.078 ms) and Q4 (0.19 ms) at
        200K persons, so native can never be "within 2x of pyoxigraph" there.  The
        harness must add at most a few microseconds to a trivial call."""
        conn = sqlite3.connect(":memory:", isolation_level=None)
        conn.create_function("sparql_count", 1, lambda _q: 0)  # an instantaneous engine
        q = BenchQuery("Q15", "t", "social", False, "select", "SELECT * {}", "<q>")
        with tempfile.TemporaryDirectory() as tmp:
            settings = EngineSettings(work_dir=tmp, timeout_s=600.0)  # the CLI default
            engine = SparqliteNative(settings, SparqliteDatabase(SparqliteLibrary(None), settings))
            engine._conn = conn
            runner = Runner([engine], RunConfig(modes=["count"], timeout_s=600.0), progress=None)
            harness_ms = statistics.median(runner._timed(engine, q, "count")[0] for _ in range(101))

            def bare_ms():
                start = time.perf_counter_ns()
                conn.execute("SELECT sparql_count(?)", (q.text,)).fetchone()
                return (time.perf_counter_ns() - start) / 1e6

            direct_ms = statistics.median(bare_ms() for _ in range(101))
        conn.close()
        self.assertLess(harness_ms - direct_ms, 0.05,
                        f"harness adds {1000 * (harness_ms - direct_ms):.0f} us per timed "
                        f"execution (bare call {1000 * direct_ms:.1f} us)")


class ReviewCanonicalTest(unittest.TestCase):
    def test_review_datetime_end_of_day_is_canonicalised(self):
        """terms.py claims dateTimes are compared in canonical form, but ``T24:00:00`` is
        kept verbatim, while pyoxigraph stores it as the next day's ``T00:00:00``: the
        same value then checksums differently (a false MISMATCH)."""
        dt = "^^<http://www.w3.org/2001/XMLSchema#dateTime>"
        self.assertEqual(canonical_nt(f'"2008-01-01T24:00:00"{dt}'),
                         canonical_nt(f'"2008-01-02T00:00:00"{dt}'))


class ReviewPipelineTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        cls.social = os.path.join(cls.tmp.name, "social.nt")
        gen_social.generate_file(cls.social, 200, 13, 25)
        cls.ext = os.path.join(cls.tmp.name, "libsparqlite.so")
        with open(cls.ext, "wb") as fh:
            fh.write(b"fake")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()
        FakeSparqliteConnection.stores.clear()

    def run_cli(self, tag, connect, *extra):
        d = os.path.join(self.tmp.name, tag)
        args = build_parser().parse_args([
            "--social", self.social, "--ext", self.ext, "--work-dir", os.path.join(d, "work"),
            "--engines", "native,pyoxigraph", "--repeat", "1", "--no-isolate", "--quiet",
            "--out-md", os.path.join(d, "out.md"), "--out-json", os.path.join(d, "out.json"),
            *extra])
        with contextlib.redirect_stderr(io.StringIO()):
            result, _md, code = run_benchmark(args, [("social", self.social)], connect=connect)
        with open(os.path.join(d, "out.json"), encoding="utf-8") as fh:
            data = json.load(fh)
        return result, code, data

    def test_review_failed_data_load_fails_the_run(self):
        """The extension loads and every function exists, but loading the dataset fails
        (a loader bug).  ``SparqliteDatabase._build`` turns *any* sqlite3.Error into
        EngineUnavailable, both sparqlite engines become "n/a" and the run exits 0, so
        a broken bulk loader passes CI silently."""

        class LoaderBug(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_load_file" and self.path != ":memory:":
                    raise sqlite3.OperationalError("sparql: RDF parse error: line 7: bad IRI")
                return super()._scalar(name, args, params)

        result, code, _ = self.run_cli("loadbug", lambda p: LoaderBug(p), "--only", "Q15",
                                       "--modes", "count")
        load = next(r for r in result.loads if r.engine == "sparqlite-native")
        self.assertNotEqual(code, 0, f"load status {load.status!r}: {load.message}")

    def test_review_probe_runtime_error_is_not_unavailable(self):
        """``sparql_count`` is registered but fails at run time (e.g. a planner bug that
        also hits the one-triple probe query).  The probe records the message and the
        whole count mode becomes "unavailable" for every query; the run exits 0."""

        class CountBug(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_count":
                    raise sqlite3.OperationalError("sparql: internal error: plan validation failed")
                return super()._scalar(name, args, params)

        result, code, _ = self.run_cli("probebug", lambda p: CountBug(p), "--only", "Q15",
                                       "--modes", "count,full")
        m = result.measurement("Q15", "sparqlite-native", "count")
        self.assertEqual(m.status, ERROR, m.message)
        self.assertNotEqual(code, 0)

    def test_review_native_unsupported_is_an_error(self):
        """Only the baseline translator has a subset (§9); the native engine implements
        all of SPARQL 1.1.  ``classify_error`` maps any message containing "unsupported"
        (or "not supported") to *unsupported* for native too, so a native engine that
        rejects Q13's ``:knows+`` passes the run."""

        class NativeRejects(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_count" and params and ":knows+" in params[0]:
                    raise sqlite3.OperationalError("sparql: unsupported: property path OneOrMore")
                return super()._scalar(name, args, params)

        result, code, _ = self.run_cli("nativeunsup", lambda p: NativeRejects(p), "--only", "Q13",
                                       "--modes", "count")
        m = result.measurement("Q13", "sparqlite-native", "count")
        self.assertEqual(m.status, ERROR, m.message)
        self.assertNotEqual(code, 0)

    def test_review_engine_settings_are_recorded(self):
        """``--native-config`` (join_policy, reorder, reader=btree for the phase-2
        'native+btree' column), ``--native-full`` and ``--sqlite-load`` change what the
        native column measures, but neither the JSON nor the markdown records them (nor
        the command line), so two different runs produce indistinguishable reports."""
        _result, _code, data = self.run_cli("settings", lambda p: FakeSparqliteConnection(p),
                                            "--only", "Q15", "--modes", "count,full",
                                            "--native-config", "reorder=0",
                                            "--native-full", "terms")
        recorded = json.dumps({k: v for k, v in data.items() if k != "measurements"})
        self.assertIn("reorder", recorded)
        self.assertIn("terms", recorded)


if __name__ == "__main__":
    unittest.main()
