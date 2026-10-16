#!/usr/bin/env python3
"""sparqlite benchmark harness (docs/SPEC.md §12.4).

Runs the benchmark queries (``py/queries/*.rq``) on the social and bsbm-lite datasets
with sparqlite-native, sparqlite-baseline, pyoxigraph and rdflib, cross-checks the
results, and writes a markdown table plus a JSON file with raw timings.  See
``py/README-bench.md``.

Examples::

    python py/gen_social.py --persons 200000 --out data/social-200k.nt
    python py/gen_bsbm.py --out data/bsbm-1m.nt
    python py/bench.py --social data/social-200k.nt --bsbm data/bsbm-1m.nt \\
        --ext ext/target/release/libsparqlite --out-md results.md
    python py/bench.py --self-test
"""

from __future__ import annotations

import os
import sys

PY_DIR = os.path.dirname(os.path.abspath(__file__))
if PY_DIR not in sys.path:
    sys.path.insert(0, PY_DIR)

from bench_lib.cli import main  # noqa: E402

if __name__ == "__main__":
    sys.exit(main())
