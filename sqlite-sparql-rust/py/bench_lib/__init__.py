"""Benchmark harness library for sparqlite (docs/SPEC.md §12.4).

Modules:
    generation  sampling/output helpers for ``gen_social.py`` and ``gen_bsbm.py``
    terms       N-Triples term parsing and engine-independent canonical forms
    checksum    order-insensitive multiset (and ordered) result checksums
    queries     benchmark query files (``py/queries/*.rq``) and their headers
    datasets    dataset files, triple counts and fingerprints
    engines     engine adapters: sparqlite-native, sparqlite-baseline, pyoxigraph, rdflib
    runner      the measurement protocol (warm-up, median of N, timeouts, cross-checks)
    report      markdown and JSON output
    envinfo     environment description (versions, CPU, SQLite)
    selftest    ``bench.py --self-test``
"""

__version__ = "0.1.0"
