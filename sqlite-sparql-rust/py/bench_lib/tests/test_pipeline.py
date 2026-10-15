"""The whole CLI pipeline with all four engines (sparqlite through the fake extension)."""

import contextlib
import io
import json
import os
import tempfile
import unittest

import gen_bsbm
import gen_social
from bench_lib.cli import build_parser, main, resolve_extension, run_benchmark
from bench_lib.tests.fake_sparqlite import FakeSparqliteConnection, connect_factory


class PipelineTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.TemporaryDirectory()
        d = cls.tmp.name
        cls.social = os.path.join(d, "social.nt")
        cls.bsbm = os.path.join(d, "bsbm.nt")
        gen_social.generate_file(cls.social, 200, 11, 25)
        gen_bsbm.generate_file(cls.bsbm, 15, 11)
        cls.ext = os.path.join(d, "libsparqlite.so")
        with open(cls.ext, "wb") as fh:
            fh.write(b"fake")

    @classmethod
    def tearDownClass(cls):
        cls.tmp.cleanup()
        FakeSparqliteConnection.stores.clear()

    def run_quietly(self, args, datasets, **kw):
        """run_benchmark with the always-printed FAILED summary captured."""
        with contextlib.redirect_stderr(io.StringIO()) as err:
            out = run_benchmark(args, datasets, **kw)
        self.stderr = err.getvalue()
        return out

    def args(self, *extra):
        d = self.tmp.name
        return build_parser().parse_args([
            "--social", self.social, "--bsbm", self.bsbm, "--ext", self.ext,
            "--work-dir", os.path.join(d, "work"), "--repeat", "2", "--modes", "count,full,json",
            "--out-md", os.path.join(d, "out.md"), "--out-json", os.path.join(d, "out.json"),
            "--quiet", *extra])

    def test_four_engines(self):
        result, markdown, code = run_benchmark(self.args(), [("social", self.social), ("bsbm", self.bsbm)],
                                               connect=connect_factory())
        self.assertEqual(code, 0)
        self.assertEqual([e.status for e in result.engines], ["ok"] * 4)
        self.assertEqual({c.status for c in result.checks}, {"ok"})
        for c in result.checks:
            self.assertEqual(len({p.split("/")[0] for p in c.participants}), 4 if c.query != "Q13" else 3)
        q13 = result.measurement("Q13", "sparqlite-baseline", "count")
        self.assertEqual(q13.status, "unsupported")
        self.assertIn("×b", markdown)
        self.assertIn("(same DB as native)", markdown)
        with open(os.path.join(self.tmp.name, "out.json"), encoding="utf-8") as fh:
            data = json.load(fh)
        self.assertEqual(data["environment"]["sparqlite"]["version"], "sparqlite fake (SQLite 3.45.1)")
        self.assertEqual(len(data["measurements"]), 17 * 4 * 3)
        # What ran is pinned: every query's text and hash, and the harness files.
        for recorded, query in zip(data["queries"], result.queries, strict=True):
            self.assertEqual((recorded["text"], recorded["sha256"]), (query.text, query.sha256))
        self.assertEqual(len(data["environment"]["harness_sha256"]), 64)
        self.assertIn("git_harness_changes", data["environment"])

    def test_defaults_measure_every_mode(self):
        """The default run times all three modes, so the spec's sparql_query_sql
        (json mode) is in it, and counts engine-side on the reference engines."""
        args = build_parser().parse_args(["--social", self.social])
        self.assertEqual((args.modes, args.reference_count), ("count,full,json", "aggregate"))

    def test_mismatch_fails_the_run(self):
        # A "native" engine that silently drops the OPTIONAL part must be caught.
        class Broken(FakeSparqliteConnection):
            def _scalar(self, name, args, params):
                if name == "sparql_count" and "OPTIONAL" in params[0]:
                    from bench_lib.tests.fake_sparqlite import FakeCursor
                    return FakeCursor([(0,)])
                return super()._scalar(name, args, params)

        args = self.args("--only", "Q5,Q6", "--engines", "native,pyoxigraph")
        result, _md, code = self.run_quietly(args, [("social", self.social)],
                                             connect=lambda path: Broken(path))
        self.assertEqual(code, 1)
        self.assertIn("FAILED: 1 queries with mismatching results (Q5)", self.stderr)
        self.assertEqual([c.query for c in result.mismatches()], ["Q5"])
        args = self.args("--only", "Q5", "--engines", "native,pyoxigraph", "--allow-mismatch")
        self.assertEqual(self.run_quietly(args, [("social", self.social)],
                                          connect=lambda path: Broken(path))[2], 0)

    def test_errors_fail_unless_allowed(self):
        connect = connect_factory(rows_error="sparql: internal error: boom", rows_error_on=":p123")
        args = self.args("--only", "Q15", "--engines", "native,pyoxigraph", "--modes", "full")
        result, _md, code = self.run_quietly(args, [("social", self.social)], connect=connect)
        self.assertEqual((code, len(result.errors())), (1, 1))
        self.assertIn("sparqlite-native/full:Q15", self.stderr)
        args = self.args("--only", "Q15", "--engines", "native,pyoxigraph", "--modes", "full",
                         "--allow-errors")
        self.assertEqual(self.run_quietly(args, [("social", self.social)], connect=connect)[2], 0)

    def test_cli_errors(self):
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(main(["--social", "/nonexistent.nt", "--quiet"]), 2)
            self.assertEqual(main(["--social", self.social, "--modes", "fast", "--quiet",
                                   "--work-dir", os.path.join(self.tmp.name, "w2")]), 2)
            with self.assertRaises(SystemExit):
                main([])
        self.assertIn("does not exist", err.getvalue())

    def test_extension_resolution(self):
        self.assertEqual(resolve_extension("/x/lib"), "/x/lib")
        old = os.environ.get("SPARQLITE_EXT")
        os.environ["SPARQLITE_EXT"] = "/from/env"
        try:
            self.assertEqual(resolve_extension(None), "/from/env")
        finally:
            if old is None:
                del os.environ["SPARQLITE_EXT"]
            else:
                os.environ["SPARQLITE_EXT"] = old


if __name__ == "__main__":
    unittest.main()
