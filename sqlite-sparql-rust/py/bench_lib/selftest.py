"""``bench.py --self-test``: unit tests, then the whole pipeline on tiny generated data.

The pipeline part generates a small social and bsbm-lite dataset, runs every query
with every engine that is available here (sparqlite engines degrade to *unavailable*
until the extension is built) in all three modes, and verifies that

* the run exits cleanly (no mismatches, no engine errors);
* every query returned at least one row, so the queries really hit the data;
* every query was answered by at least two engines that agree, whenever two engines
  are available;
* the markdown and JSON reports were written and are complete.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import tempfile
import unittest

import gen_bsbm
import gen_social

from .cli import run_benchmark
from .runner import OK

SELFTEST_PERSONS = 400  # >= 124 so the :p123 constants exist
SELFTEST_PRODUCTS = 40
SELFTEST_SEED = 7
TESTS_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "tests")
PY_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def _say(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def run_unit_tests() -> bool:
    suite = unittest.TestLoader().discover(TESTS_DIR, top_level_dir=PY_DIR)
    outcome = unittest.TextTestRunner(stream=sys.stderr, verbosity=1).run(suite)
    return outcome.wasSuccessful()


def validate(result, markdown: str, out_md: str, out_json: str) -> list[str]:
    """Problems found in a self-test run (empty = pass)."""
    problems = []
    usable = [e.name for e in result.engines if e.status == OK]
    if not usable:
        return ["no engine is available"]
    checks = {c.query: c for c in result.checks}
    for q in result.queries:
        c = checks.get(q.id)
        if c is None:
            problems.append(f"{q.id}: not run")
            continue
        if c.status == "mismatch":
            problems.append(f"{q.id}: engines disagree: {'; '.join(c.details)}")
        elif c.status == "none":
            problems.append(f"{q.id}: no engine produced a result")
        elif c.status == "single" and len(usable) > 1:
            problems.append(f"{q.id}: only one engine produced a result")
        if c.rows == 0:
            problems.append(f"{q.id}: returned no rows on the self-test data")
    problems.extend(result.setup_errors())
    for m in result.errors():
        problems.append(f"{m.query} {m.where}: {m.message}")
    for path in (out_md, out_json):
        if not os.path.isfile(path):
            problems.append(f"{path} was not written")
    with open(out_json, encoding="utf-8") as fh:
        data = json.load(fh)
    expected = len(result.queries) * len(result.engines) * len(result.config.modes)
    if len(data.get("measurements", [])) != expected:
        problems.append(f"JSON has {len(data.get('measurements', []))} measurements, "
                        f"expected {expected}")
    for q in result.queries:
        if f"| {q.label} |" not in markdown:
            problems.append(f"{q.id} missing from the markdown report")
    return problems


def run_self_test(args: argparse.Namespace) -> int:
    _say("== bench self-test: unit tests")
    if not run_unit_tests():
        _say("SELF-TEST FAILED: unit tests")
        return 1
    with tempfile.TemporaryDirectory(prefix="sparqlite-bench-selftest-") as tmp:
        _say(f"== bench self-test: pipeline in {tmp}")
        social = os.path.join(tmp, "social.nt")
        bsbm = os.path.join(tmp, "bsbm.nt")
        gen_social.generate_file(social, SELFTEST_PERSONS, SELFTEST_SEED)
        gen_bsbm.generate_file(bsbm, SELFTEST_PRODUCTS, SELFTEST_SEED)
        ns = argparse.Namespace(**vars(args))
        ns.work_dir = os.path.join(tmp, "work")
        ns.modes = "count,full,json"
        ns.only = None
        ns.warmup = 1
        ns.repeat = min(args.repeat, 2)
        ns.cell_budget = min(args.cell_budget, 30.0)
        ns.reuse_db = False
        ns.allow_mismatch = ns.allow_errors = False
        ns.out_md = os.path.join(tmp, "selftest.md")
        ns.out_json = os.path.join(tmp, "selftest.json")
        result, markdown, code = run_benchmark(ns, [("social", social), ("bsbm", bsbm)])
        problems = validate(result, markdown, ns.out_md, ns.out_json)
        if code and not problems:
            problems.append(f"benchmark exit code {code}")
    print(markdown)
    for rec in result.engines:
        state = rec.version if rec.status == OK else f"{rec.status}: {rec.message}"
        _say(f"   {rec.name:<20} {state}")
    if problems:
        for p in problems:
            _say(f"   PROBLEM: {p}")
        _say("SELF-TEST FAILED")
        return 1
    ran = sum(1 for e in result.engines if e.status == OK)
    _say(f"SELF-TEST PASSED ({len(result.queries)} queries, {ran} engines, "
         f"{len(result.measurements)} cells)")
    return 0
