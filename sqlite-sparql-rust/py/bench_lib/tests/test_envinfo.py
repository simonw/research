import os
import tempfile
import unittest

from bench_lib import envinfo


class HarnessDigestTest(unittest.TestCase):
    def test_digest_pins_harness_files_only(self):
        with tempfile.TemporaryDirectory() as py:
            def write(rel, text):
                path = os.path.join(py, rel)
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w", encoding="utf-8") as fh:
                    fh.write(text)

            write("bench.py", "main()")
            write("queries/q01.rq", "SELECT * {}")
            write("bench_lib/runner.py", "x = 1")
            write("bench_lib/__pycache__/runner.cpython-313.pyc", "cache")
            write("tests/test_other_package.py", "not the harness")
            files = [os.path.relpath(f, py) for f in envinfo.harness_files(py)]
            self.assertEqual(files, ["bench.py", "bench_lib/runner.py", "queries/q01.rq"])
            before = envinfo.harness_digest(py)
            write("tests/test_other_package.py", "changed")
            self.assertEqual(envinfo.harness_digest(py), before)
            write("queries/q01.rq", "SELECT * { ?s ?p ?o }")
            self.assertNotEqual(envinfo.harness_digest(py), before)

    def test_collect_records_the_pins(self):
        repo = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
            os.path.abspath(__file__)))))
        env = envinfo.collect(repo)
        self.assertEqual(len(env["harness_sha256"]), 64)
        self.assertIn("git_harness_changes", env)
        changes = env["git_harness_changes"]
        self.assertTrue(changes is None or all("py/" in line for line in changes))
        self.assertIsNone(envinfo._git_changes(os.path.join(repo, "no-such-dir"), ["py"]))


if __name__ == "__main__":
    unittest.main()
