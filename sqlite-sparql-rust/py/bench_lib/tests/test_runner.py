import gc
import time
import unittest

from bench_lib.checksum import ResultSet
from bench_lib.datasets import Dataset
from bench_lib.engines import Engine, EngineSettings, EngineUnavailable, LoadInfo, Timed
from bench_lib.engines.base import QueryTimeout, QueryUnsupported
from bench_lib.queries import BenchQuery
from bench_lib.runner import Measurement, RunConfig, Runner, cross_check

DS = Dataset("social", "/dev/null", 10, 100, "f" * 40)


def query(qid="Q1", ordered=False, dataset="social"):
    return BenchQuery(qid, "t", dataset, ordered, "select", "SELECT * {}", "<q>")


class ScriptedEngine(Engine):
    """An engine whose behaviour per query id is scripted: a value, or an exception
    class to raise."""

    def __init__(self, name, script, skip=None, unavailable_modes=(), probe_error=None,
                 open_error=None, probe_exception=None):
        super().__init__(EngineSettings(work_dir="/tmp"))
        self.name = self.short = name
        self.script = script
        self.skip = skip
        self.unavailable_modes = set(unavailable_modes)
        self.probe_error = probe_error
        self.probe_exception = probe_exception
        self.open_error = open_error
        self.calls = []
        self.opened = self.closed = 0
        self.gc_during_execute = []

    def probe(self):
        if self.probe_error:
            raise EngineUnavailable(self.probe_error)
        if self.probe_exception:
            raise self.probe_exception
        return f"{self.name} 1.0"

    def skip_reason(self, dataset):
        return self.skip

    def open(self, dataset):
        if self.open_error:
            raise self.open_error
        self.opened += 1
        return LoadInfo(seconds=0.5, triples=dataset.triples)

    def close(self):
        self.closed += 1

    def mode_unavailable(self, mode):
        return "nope" if mode in self.unavailable_modes else None

    def execute(self, q, mode):
        self.calls.append((q.id, mode))
        self.gc_during_execute.append(gc.isenabled())
        outcome = self.script[q.id]
        if isinstance(outcome, type) and issubclass(outcome, BaseException):
            raise outcome("scripted")
        return outcome

    def decode(self, q, mode, raw):
        if mode == "count":
            return len(raw)
        return ResultSet("select", ["x"], [(c,) for c in raw])


def runner(engines, **kw):
    cfg = dict(modes=["count", "full"], warmup=1, repeat=3, timeout_s=60, cell_budget_s=60)
    cfg.update(kw)
    return Runner(engines, RunConfig(**cfg), progress=None)


class RunnerTest(unittest.TestCase):
    def test_protocol_and_agreement(self):
        a = ScriptedEngine("a", {"Q1": ["<x>", "<y>"]})
        b = ScriptedEngine("b", {"Q1": ["<y>", "<x>"]})
        updates = []
        r = Runner([a, b], RunConfig(modes=["count", "full"], warmup=1, repeat=3),
                   progress=None, on_update=updates.append)
        result = r.run([DS], [query()])
        self.assertEqual(len(a.calls), 2 * (1 + 3))
        self.assertEqual((a.opened, a.closed), (1, 1))
        self.assertFalse(any(a.gc_during_execute))
        self.assertTrue(gc.isenabled())
        cells = result.measurements
        self.assertEqual(len(cells), 4)
        self.assertTrue(all(m.status == "ok" and len(m.times_ms) == 3 and len(m.warmup_ms) == 1
                            for m in cells))
        self.assertTrue(all(m.rows == 2 for m in cells))
        self.assertEqual(result.checks[0].status, "ok")
        self.assertEqual(len(result.checks[0].participants), 4)
        self.assertTrue(updates and updates[-1].finished)
        self.assertEqual([e.status for e in result.engines], ["ok", "ok"])

    def test_mismatches_are_detected(self):
        a = ScriptedEngine("a", {"Q1": ["<x>", "<y>"], "Q2": ["<x>"]})
        b = ScriptedEngine("b", {"Q1": ["<x>", "<z>"], "Q2": ["<x>", "<x>"]})
        result = runner([a, b]).run([DS], [query("Q1"), query("Q2")])
        q1, q2 = result.checks
        self.assertEqual(q1.status, "mismatch")
        self.assertTrue(any(d.startswith("checksum differ") for d in q1.details))
        self.assertEqual(q2.status, "mismatch")
        self.assertTrue(any(d.startswith("rows differ") for d in q2.details))
        self.assertEqual(len(result.mismatches()), 2)

    def test_order_is_checked_only_for_ordered_queries(self):
        a = ScriptedEngine("a", {"Q1": ["<x>", "<y>"], "Q2": ["<x>", "<y>"]})
        b = ScriptedEngine("b", {"Q1": ["<y>", "<x>"], "Q2": ["<y>", "<x>"]})
        result = runner([a, b], modes=["full"]).run([DS], [query("Q1"), query("Q2", ordered=True)])
        self.assertEqual([c.status for c in result.checks], ["ok", "mismatch"])
        self.assertTrue(result.checks[1].details[0].startswith("order differ"))

    def test_statuses(self):
        a = ScriptedEngine("a", {"Q1": QueryUnsupported, "Q2": QueryTimeout, "Q3": KeyError,
                                 "Q4": EngineUnavailable, "Q5": ["<x>"]})
        qs = [query(f"Q{i}") for i in range(1, 6)]
        result = runner([a], modes=["count"]).run([DS], qs[:4])
        statuses = [result.measurement(f"Q{i}", "a", "count").status for i in range(1, 5)]
        self.assertEqual(statuses, ["unsupported", "timeout", "error", "unavailable"])
        self.assertIn("KeyError", result.measurement("Q3", "a", "count").message)
        self.assertEqual(len(result.errors()), 1)
        b = ScriptedEngine("b", {"Q5": ["<x>"]}, unavailable_modes=["full"])
        result = runner([a, b]).run([DS], [qs[4]])
        self.assertEqual(result.measurement("Q5", "b", "full").status, "unavailable")
        self.assertEqual(result.measurement("Q5", "b", "count").status, "ok")
        self.assertEqual(result.checks[0].status, "ok")

    def test_single_engine_and_none(self):
        a = ScriptedEngine("a", {"Q1": ["<x>"], "Q2": QueryUnsupported})
        result = runner([a]).run([DS], [query("Q1"), query("Q2")])
        self.assertEqual([c.status for c in result.checks], ["single", "none"])

    def test_unavailable_skipped_and_failed_engines(self):
        ok = ScriptedEngine("ok", {"Q1": ["<x>"]})
        gone = ScriptedEngine("gone", {}, probe_error="no library")
        small = ScriptedEngine("small", {}, skip="too big")
        broken = ScriptedEngine("broken", {}, open_error=EngineUnavailable("cannot load"))
        crashing = ScriptedEngine("crashing", {}, open_error=OSError("disk"))
        buggy = ScriptedEngine("buggy", {}, probe_exception=RuntimeError("init failed"))
        result = runner([ok, gone, small, broken, crashing, buggy]).run([DS], [query()])
        status = {e.name: e.status for e in result.engines}
        self.assertEqual((status["gone"], status["buggy"]), ("unavailable", "error"))
        loads = {ld.engine: ld.status for ld in result.loads}
        self.assertEqual(loads, {"ok": "ok", "small": "skipped", "broken": "unavailable",
                                 "crashing": "error"})
        cell = {m.engine: m for m in result.measurements if m.mode == "count"}
        self.assertEqual(cell["gone"].status, "unavailable")
        self.assertEqual(cell["gone"].message, "no library")
        self.assertEqual(cell["small"].status, "skipped")
        self.assertEqual(cell["broken"].status, "unavailable")
        # A present engine that fails is an error everywhere it could not run.
        self.assertEqual(cell["crashing"].status, "error")
        self.assertEqual(cell["crashing"].message, "not run: OSError: disk")
        self.assertEqual(cell["buggy"].status, "error")
        self.assertEqual(result.setup_errors(), ["buggy: RuntimeError: init failed",
                                                 "crashing on social: OSError: disk"])
        self.assertEqual({m.engine for m in result.errors()}, {"crashing", "buggy"})
        self.assertEqual(small.calls, [])

    def test_datasets_without_queries_are_not_opened(self):
        a = ScriptedEngine("a", {"Q1": ["<x>"]})
        other = Dataset("bsbm", "/dev/null", 1, 1, "e" * 40)
        runner([a]).run([DS, other], [query()])
        self.assertEqual(a.opened, 1)

    def test_cell_budget_limits_repetitions(self):
        r = runner([], repeat=5, cell_budget_s=1.0)
        self.assertEqual(r._repetitions(100.0), 5)
        self.assertEqual(r._repetitions(400.0), 2)
        self.assertEqual(r._repetitions(5000.0), 1)
        self.assertEqual(runner([], cell_budget_s=0)._repetitions(1e9), 3)

    def test_warmup_over_budget_is_the_only_sample(self):
        a = ScriptedEngine("a", {"Q1": ["<x>"]})
        result = runner([a], cell_budget_s=1e-9).run([DS], [query()])
        m = result.measurements[0]
        self.assertEqual((len(m.times_ms), m.warmup_ms, m.status), (1, [], "ok"))
        self.assertEqual(len(a.calls), 2)  # one per mode

    def test_timeout_skips_remaining_modes(self):
        a = ScriptedEngine("a", {"Q1": QueryTimeout, "Q2": ["<x>"]})
        result = runner([a], modes=["count", "full", "json"]).run([DS], [query("Q1"), query("Q2")])
        statuses = [(m.query, m.mode, m.status) for m in result.measurements]
        self.assertEqual(statuses[:3], [("Q1", "count", "timeout"), ("Q1", "full", "timeout"),
                                        ("Q1", "json", "timeout")])
        self.assertIn("skipped: timed out in count mode", result.measurements[1].message)
        self.assertEqual(a.calls.count(("Q1", "full")), 0)
        self.assertEqual([s for _, _, s in statuses[3:]], ["ok"] * 3)

    def test_engine_measured_time_is_used(self):
        class SelfTimed(ScriptedEngine):
            def execute(self, q, mode):
                return Timed(1234.5, super().execute(q, mode))

        a = SelfTimed("a", {"Q1": ["<x>"]})
        result = runner([a], cell_budget_s=1e6).run([DS], [query()])
        self.assertEqual(result.measurements[0].median_ms, 1234.5)
        self.assertEqual(result.measurements[0].rows, 1)

    def test_slow_python_engine_counts_as_timeout(self):
        a = ScriptedEngine("a", {"Q1": ["<x>"]})
        result = runner([a], timeout_s=1e-9).run([DS], [query()])
        self.assertEqual(result.measurements[0].status, "timeout")
        self.assertIn("not interruptible", result.measurements[0].message)

    def test_cross_check_ignores_failed_cells(self):
        ok = Measurement("social", "Q1", "a", "full", "ok", [1.0], rows=1, checksum="c1")
        bad = Measurement("social", "Q1", "b", "full", "error", message="x")
        check = cross_check("social", query(), [ok, bad])
        self.assertEqual((check.status, check.rows, check.checksum), ("single", 1, "c1"))

    def test_every_execution_is_checked(self):
        """The first and last executions are decoded, the others only counted; a cell
        whose executions disagree in rows or content is an error that fails the run."""

        class Counting(ScriptedEngine):
            decodes = row_counts = 0

            def decode(self, q, mode, raw):
                self.decodes += 1
                return super().decode(q, mode, raw)

            def row_count(self, q, mode, raw):  # the cheap path adapters provide
                self.row_counts += 1
                return len(raw)

        a = Counting("a", {"Q1": ["<x>", "<y>"]})
        result = runner([a], modes=["full"], warmup=2, repeat=3).run([DS], [query()])
        self.assertEqual(result.measurements[0].status, "ok")
        self.assertEqual((a.decodes, a.row_counts), (2, 4))

        class Flaky(ScriptedEngine):
            """Same number of rows every time, different content on the last run."""

            def execute(self, q, mode):
                rows = super().execute(q, mode)
                return ["<z>", "<y>"] if len(self.calls) == 1 + 3 else rows

        flaky = Flaky("flaky", {"Q1": ["<x>", "<y>"]})
        good = ScriptedEngine("good", {"Q1": ["<x>", "<y>"]})
        result = runner([good, flaky], modes=["full"]).run([DS], [query()])
        cell = result.measurement("Q1", "flaky", "full")
        self.assertEqual(cell.status, "error")
        self.assertIn("results differ between executions: the first returned 2:", cell.message)
        self.assertEqual(len(cell.times_ms), 3)  # the timings are kept
        self.assertEqual(result.errors(), [cell])

        class Growing(ScriptedEngine):
            def execute(self, q, mode):
                return super().execute(q, mode) * len(self.calls)

        g = Growing("g", {"Q1": ["<x>"]})
        cell = runner([g], modes=["count"]).run([DS], [query()]).measurements[0]
        self.assertEqual(cell.status, "error")
        self.assertIn("rows per execution [1, 2, 3, 4] (warm-up first)", cell.message)

    def test_row_count_disagreeing_with_decode_is_an_adapter_error(self):
        class Miscounts(ScriptedEngine):
            def row_count(self, q, mode, raw):
                return len(raw) + 1

        cell = runner([Miscounts("m", {"Q1": ["<x>"]})], modes=["full"]).run(
            [DS], [query()]).measurements[0]
        self.assertEqual(cell.status, "error")
        self.assertEqual(cell.message,
                         "ValueError: Miscounts.row_count found 2 rows where decode found 1")

    def test_single_execution_is_decoded_once(self):
        a = ScriptedEngine("a", {"Q1": ["<x>"]})
        result = runner([a], warmup=0, repeat=1).run([DS], [query()])
        self.assertEqual([(m.status, m.rows) for m in result.measurements], [("ok", 1)] * 2)

    def test_results_are_freed_outside_the_timing(self):
        class SlowToFree(list):
            def __del__(self):
                time.sleep(0.03)

        class Engine_(ScriptedEngine):
            def execute(self, q, mode):
                return SlowToFree(super().execute(q, mode))

        a = Engine_("a", {"Q1": ["<x>"]})
        m = runner([a], modes=["full"], warmup=1, repeat=3).run([DS], [query()]).measurements[0]
        self.assertEqual(m.status, "ok")
        self.assertLess(max(m.warmup_ms + m.times_ms), 15.0, m.times_ms)

    def test_json_round_trip(self):
        a = ScriptedEngine("a", {"Q1": ["<x>"]})
        result = runner([a]).run([DS], [query()])
        data = result.to_json()
        self.assertEqual(data["config"]["repeat"], 3)
        self.assertEqual((data["config"]["engine_settings"], data["config"]["argv"]), ({}, []))
        self.assertEqual(data["measurements"][0]["median_ms"], result.measurements[0].median_ms)
        self.assertEqual(data["loads"][0]["info"]["triples"], 10)


if __name__ == "__main__":
    unittest.main()
