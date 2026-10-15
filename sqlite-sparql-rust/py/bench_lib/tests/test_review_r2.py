"""Adversarial review tests, round 2 (WP13a).  Each ``test_review_*`` pins a defect found
in review; it fails until the harness is fixed."""

import contextlib
import io
import json
import os
import statistics
import tempfile
import time
import unittest
from unittest import mock

import pyoxigraph as ox

import gen_social
from bench_lib.cli import build_parser, run_benchmark
from bench_lib.engines import isolated
from bench_lib.engines.base import Engine, EngineSettings, Timed
from bench_lib.engines.oxigraph import PyoxigraphEngine
from bench_lib.queries import BenchQuery, query_form
from bench_lib.runner import RunConfig, Runner
from bench_lib.tests.test_runner import DS, ScriptedEngine


def _query(text: str) -> BenchQuery:
    return BenchQuery("Q99", "review", "social", False, query_form(text), text, "<review>")


class _SlowToFree:
    """A raw result whose deallocation costs 50 ms, standing in for a large result
    (Q11 full at 200K persons: 127,794 rows x 2 strings take ~15 ms to free)."""

    def __del__(self):
        time.sleep(0.05)


class _InstantEngine(Engine):
    """Builds its result instantly; only freeing the result is slow."""

    name = short = "pyoxigraph"

    def open(self, dataset):  # pragma: no cover - not used
        raise AssertionError

    def close(self):
        pass

    def execute(self, query, mode):
        return _SlowToFree()

    def decode(self, query, mode, raw):  # pragma: no cover - not used
        return 0


class _ScriptedPipe:
    """The child's end of the pipe: replays requests and keeps only the measured times
    (like pickling, it drops the reference to the raw result)."""

    def __init__(self, requests):
        self._requests = list(requests)
        self.elapsed_ms = []

    def recv(self):
        if not self._requests:
            raise EOFError
        return self._requests.pop(0)

    def send(self, reply):
        if reply[0] == "ok" and isinstance(reply[1], Timed):
            self.elapsed_ms.append(reply[1].elapsed_ms)


class ReviewIsolatedTimingTest(unittest.TestCase):
    def test_review_isolated_worker_frees_previous_result_outside_timing(self):
        """``Runner._measure`` sets ``raw = None`` before every timed call, so the previous
        repetition's result is freed outside the timing.  The isolated worker
        (``isolated._serve``) keeps ``raw`` alive between requests and rebinds it inside
        ``start .. elapsed_ms``, so every pyoxigraph/rdflib execution (the default,
        isolated mode) also pays for freeing the previous execution's result, which the
        in-process SQLite engines never do."""
        q = _query("SELECT * WHERE { ?s ?p ?o }")
        pipe = _ScriptedPipe([("execute", q, "full"), ("execute", q, "full")])
        inner = lambda _name, settings: _InstantEngine(settings)  # noqa: E731
        with mock.patch.object(isolated, "_inner", inner):
            isolated._serve("pyoxigraph", EngineSettings(work_dir="/tmp"), pipe)
        self.assertEqual(len(pipe.elapsed_ms), 2)
        self.assertLess(max(pipe.elapsed_ms), 25.0,
                        f"worker-measured times {pipe.elapsed_ms} ms: the second execution "
                        "includes freeing the first one's result")


class ReviewCountModeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        filler = "lorem ipsum " * 8
        data = "".join(f'<http://e/s{i}> <http://e/p> "{filler}{i}" .\n' for i in range(100_000))
        cls.store = ox.Store()
        cls.store.bulk_load(data.encode(), format=ox.RdfFormat.N_TRIPLES)

    def test_review_pyoxigraph_count_mode_does_not_materialise_terms(self):
        """The ``count`` contract (engines/base.py, the report's "count mode (solutions
        counted, no result term materialised)" and README-bench.md "counts solutions
        without building any term") is what ``sparql_count`` does, but the pyoxigraph
        adapter counts by iterating ``Store.query(q)``: pyoxigraph decodes every term of
        every solution and builds a Python QuerySolution per row.  On this 100K-row,
        literal-heavy SELECT that is ~4.7x slower than pyoxigraph counting the same
        solutions itself; at 200K persons Q11 is 363 ms vs 174 ms and Q1 194 ms vs 143 ms,
        so the count table's ``×o`` ratios overstate sparqlite's lead by up to 2x."""
        engine = PyoxigraphEngine(EngineSettings(work_dir="/tmp"))
        engine.probe()
        engine._store = self.store
        q = _query("SELECT ?s ?o WHERE { ?s <http://e/p> ?o }")
        counted = ("SELECT (COUNT(*) AS ?n) WHERE { SELECT ?s ?o WHERE "
                   "{ ?s <http://e/p> ?o } }")

        def harness_ms():
            start = time.perf_counter()
            n = engine.decode(q, "count", engine.execute(q, "count"))
            self.assertEqual(n, 100_000)
            return (time.perf_counter() - start) * 1e3

        def engine_count_ms():
            start = time.perf_counter()
            n = int(next(iter(self.store.query(counted)))[0].value)
            self.assertEqual(n, 100_000)
            return (time.perf_counter() - start) * 1e3

        harness = statistics.median(harness_ms() for _ in range(5))
        reference = statistics.median(engine_count_ms() for _ in range(5))
        self.assertLess(harness / reference, 2.0,
                        f"pyoxigraph count mode {harness:.1f} ms vs {reference:.1f} ms for "
                        "counting the same solutions without materialising them")


class _ColdPathBug(ScriptedEngine):
    """Answers wrongly while its caches are cold (the first execution of a query), then
    correctly: the shape of a decode/lookup-cache bug (docs/SPEC.md §4.4)."""

    def execute(self, q, mode):
        rows = super().execute(q, mode)
        return rows[:1] if len(self.calls) == 1 else rows


class ReviewRepetitionCheckTest(unittest.TestCase):
    def test_review_every_repetition_is_checked(self):
        """``Runner._measure`` decodes only the raw result of the *last* repetition; the
        warm-up and the other timed runs are never looked at, not even their row count
        (free in count mode).  An engine whose first, cold-cache execution returns a
        wrong answer passes the cross-check, although checking every repetition's
        result (outside the timing) is what catches cache-invalidation bugs."""
        good = ScriptedEngine("good", {"Q99": ["<x>", "<y>"]})
        cold = _ColdPathBug("cold", {"Q99": ["<x>", "<y>"]})
        config = RunConfig(modes=["count"], warmup=1, repeat=3, timeout_s=60, cell_budget_s=60)
        result = Runner([good, cold], config, progress=None).run(
            [DS], [_query("SELECT ?x WHERE { ?x ?p ?o }")])
        cell = result.measurement("Q99", "cold", "count")
        self.assertTrue(result.checks[0].status == "mismatch" or cell.status != "ok",
                        f"check {result.checks[0].status}, cell {cell.status} {cell.rows} rows; "
                        "the warm-up returned 1 row, every later run 2")


class ReviewReproducibilityTest(unittest.TestCase):
    def test_review_json_pins_the_queries_that_ran(self):
        """The JSON records each query's id, title and file path, but neither its text nor
        a hash of it, and ``git_revision`` is only ``HEAD`` (no dirty flag) while the
        working tree is edited concurrently.  Datasets and the extension are pinned by
        SHA-256; the queries, which define what was measured, are not, so an edited
        ``.rq`` file is indistinguishable from the original in the results."""
        with tempfile.TemporaryDirectory() as tmp:
            social = os.path.join(tmp, "social.nt")
            gen_social.generate_file(social, 200, 13, 25)
            out_json = os.path.join(tmp, "out.json")
            args = build_parser().parse_args([
                "--social", social, "--engines", "pyoxigraph", "--only", "Q15",
                "--modes", "count", "--repeat", "1", "--no-isolate", "--quiet",
                "--work-dir", os.path.join(tmp, "work"),
                "--out-md", os.path.join(tmp, "out.md"), "--out-json", out_json])
            with contextlib.redirect_stderr(io.StringIO()):
                run_benchmark(args, [("social", social)], argv=["bench.py"])
            with open(out_json, encoding="utf-8") as fh:
                recorded = json.load(fh)["queries"][0]
        self.assertTrue({"text", "sha256"} & set(recorded),
                        f"query record without its text or hash: {recorded}")


if __name__ == "__main__":
    unittest.main()
