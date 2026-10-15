import json
import os
import tempfile
import unittest

from bench_lib.datasets import Dataset
from bench_lib.engines import EngineSettings, LoadInfo
from bench_lib.queries import BenchQuery
from bench_lib.report import fmt_ms, fmt_ratio, render_markdown, write_json
from bench_lib.runner import (Check, EngineRecord, LoadRecord, Measurement, RunConfig, RunResult,
                              cross_check)

ENGINES = [("sparqlite-native", "native"), ("sparqlite-baseline", "baseline"),
           ("pyoxigraph", "pyoxigraph"), ("rdflib", "rdflib")]


def q(qid, dataset="social", ordered=False):
    return BenchQuery(qid, f"title {qid}", dataset, ordered, "select", "", "<q>")


def cell(qid, engine, mode, ms, rows=10, checksum="10:aa", status="ok", runs=5, message=None):
    return Measurement("social", qid, engine, mode, status, [ms] * runs if status == "ok" else [],
                       [ms], rows if status == "ok" else None,
                       checksum if status == "ok" and mode != "count" else None,
                       message=message)


def make_result(**settings):
    engine_settings = EngineSettings(work_dir="/w", **settings).to_json()
    r = RunResult(config=RunConfig(modes=["count", "full"], repeat=5,
                                   engine_settings=engine_settings,
                                   argv=["py/bench.py", "--social", "data/s 200k.nt"]),
                  environment={"sqlite_version": "3.45.1", "python": "3.13", "cpu_model": "CPU",
                               "cpu_count": 4, "memory_bytes": 2**34, "pyoxigraph": "0.5.11",
                               "timestamp_utc": "now", "git_revision": "abcdef1234567890",
                               "sparqlite": {"version": "sparqlite 0.1.0"}})
    r.engines = [EngineRecord(n, s, "ok", version="v") for n, s in ENGINES[:3]]
    r.engines.append(EngineRecord("rdflib", "rdflib", "unavailable", message="not installed"))
    r.datasets = [Dataset("social", "/x.nt", 1000, 2_000_000, "f" * 40)]
    r.queries = [q("Q1"), q("Q3", ordered=True)]
    r.loads = [
        LoadRecord("social", "sparqlite-native", "ok",
                   LoadInfo(3.0, 1000, {"load_s": 2.0, "analyze_sql_s": 0.5, "sparql_analyze_s": 0.5,
                                        "db_bytes": 5_000_000, "load_mode": "file"})),
        LoadRecord("social", "sparqlite-baseline", "ok",
                   LoadInfo(0.0, 1000, shared_with="sparqlite-native")),
        LoadRecord("social", "pyoxigraph", "ok", LoadInfo(1.5, 1000)),
    ]
    for mode in ("count", "full"):
        r.measurements += [cell("Q1", "sparqlite-native", mode, 10.0),
                           cell("Q1", "sparqlite-baseline", mode, 40.0, runs=2),
                           cell("Q1", "pyoxigraph", mode, 20.0),
                           cell("Q1", "rdflib", mode, 0, status="unavailable", message="not installed")]
        r.measurements += [cell("Q3", "sparqlite-native", mode, 5.0, checksum="10:bad"),
                           cell("Q3", "sparqlite-baseline", mode, 0, status="unsupported",
                                message="sparql: unsupported: baseline: paths"),
                           cell("Q3", "pyoxigraph", mode, 50.0),
                           cell("Q3", "rdflib", mode, 0, status="unavailable", message="not installed")]
    for qq in r.queries:
        r.checks.append(cross_check("social", qq, [m for m in r.measurements if m.query == qq.id]))
    return r


class FormatTest(unittest.TestCase):
    def test_numbers(self):
        self.assertEqual([fmt_ms(x) for x in (0.1234, 3.14159, 42.42, 1234.5)],
                         ["0.123", "3.14", "42.4", "1,234"])
        self.assertEqual([fmt_ratio(x) for x in (0.0045, 0.5, 3.333, 42.0, 1234.0)],
                         ["0.0045", "0.50", "3.33", "42.0", "1,234"])


class MarkdownTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.result = make_result()
        cls.md = render_markdown(cls.result)

    def row(self, prefix, section):
        part = self.md.split(section, 1)[1]
        return next(line for line in part.splitlines() if line.startswith(prefix))

    def test_speedups(self):
        line = self.row("| Q1 title Q1", "### count mode")
        cols = [c.strip() for c in line.strip("|").split("|")]
        self.assertEqual(cols[2], "10")
        self.assertEqual(cols[3], "10.0 (4.00×b, 2.00×o)")
        self.assertEqual(cols[4], "40.0† (0.50×o)")  # fewer runs than requested
        self.assertEqual(cols[5], "20.0 (2.00×b)")
        self.assertEqual(cols[6], "n/a")
        self.assertIn("native vs baseline: 4.00× over 1 queries", self.md)

    def test_mismatch_and_statuses(self):
        self.assertEqual(self.result.checks[1].status, "mismatch")
        line = self.row("| Q3 title Q3", "### full mode")
        self.assertIn("**MISMATCH**", line.split("|")[4])  # native differs from pyoxigraph
        self.assertNotIn("**MISMATCH**", line.split("|")[6])
        self.assertIn("unsupported", line)
        self.assertIn("| ? |", line)
        self.assertIn("**Q3 MISMATCH**", self.md)
        self.assertIn("sparqlite-baseline unsupported (Q3/count, Q3/full): sparql: unsupported", self.md)
        self.assertIn("rdflib unavailable: not installed", self.md)

    def test_loads_and_header(self):
        line = self.row("| social", "### Datasets and load times")
        self.assertIn("3.00 s, 5 MB", line)
        self.assertIn("(same DB as native)", line)
        self.assertIn("1.50 s", line)
        self.assertIn("load 2.00 s (file), ANALYZE 0.50 s, sparql_analyze 0.50 s", self.md)
        self.assertIn("git abcdef123456", self.md)
        self.assertIn("sparqlite 0.1.0", self.md)

    def test_settings_and_mode_descriptions(self):
        self.assertIn("Settings: modes count, full; median of 5 after 1 warm-up; timeout 600 s; "
                      "cell budget 120 s; full columns `terms`; native config default; "
                      "SQLite load `auto`; pyoxigraph/rdflib in worker processes; "
                      "rdflib up to 100,000 triples. "
                      "Command: `py/bench.py --social 'data/s 200k.nt'`.", self.md)
        self.assertIn("### full mode (one Python string per projected result term, on every engine)",
                      self.md)
        self.assertIn("native: `SELECT t0..t{n-1} FROM sparql_rows(q)`", self.md)
        self.assertIn("""baseline: `SELECT t0..t{n-1} FROM sparql_rows(q, '{"engine":"sql"}')`""", self.md)
        self.assertIn("| social | 1,000 | 2.0 | `ffffffffffff` |", self.md)
        star = render_markdown(make_result(full_columns="star", native_config={"reorder": "0"},
                                           isolate=False))
        self.assertIn("### full mode, `SELECT *` variant", star)
        self.assertIn("native: `SELECT * FROM sparql_rows(q)`", star)
        self.assertIn("native config `reorder=0`", star)
        self.assertIn("pyoxigraph/rdflib in process", star)

    def test_failed_engine_and_load_notes(self):
        r = make_result()
        r.engines[1] = EngineRecord("sparqlite-baseline", "baseline", "error",
                                    message="ExtensionError: sparql_init() failed: x")
        r.loads = [ld for ld in r.loads if ld.engine != "sparqlite-baseline"]
        r.loads[0] = LoadRecord("social", "sparqlite-native", "error",
                                message="ExtensionError: building failed: parse error")
        for m in r.measurements:
            if m.engine == "sparqlite-baseline":
                m.status, m.message = "error", "not run: ExtensionError: sparql_init() failed: x"
            if m.engine == "sparqlite-native":
                m.status, m.message = "error", "not run: ExtensionError: building failed: parse error"
        md = render_markdown(r)
        self.assertEqual(md.count("sparql_init() failed: x"), 1)  # one note for the engine
        self.assertIn("- sparqlite-baseline error: ExtensionError: sparql_init() failed: x", md)
        self.assertIn("- sparqlite-native error (Q1/count, Q1/full, Q3/count, Q3/full): "
                      "not run: ExtensionError: building failed: parse error", md)
        load = next(line for line in md.splitlines() if line.startswith("| social |"))
        self.assertIn("**error**", load)

    def test_checks_table(self):
        line = self.row("| Q1 title Q1", "### Result checks")
        self.assertIn("| ok | 10 | `10:aa` | 6 |", line)  # 3 engines x 2 modes

    def test_json(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = os.path.join(tmp, "sub", "r.json")
            write_json(self.result, path)
            with open(path, encoding="utf-8") as fh:
                data = json.load(fh)
            self.assertEqual(os.listdir(os.path.dirname(path)), ["r.json"])
        self.assertEqual(len(data["measurements"]), 16)
        self.assertEqual(data["checks"][1]["status"], "mismatch")
        self.assertEqual(data["environment"]["sqlite_version"], "3.45.1")

    def test_check_objects(self):
        c = Check("social", "Q1", "ok", rows=1, checksum="x", participants=["a/count"])
        self.assertEqual(c.to_json()["participants"], ["a/count"])


if __name__ == "__main__":
    unittest.main()
