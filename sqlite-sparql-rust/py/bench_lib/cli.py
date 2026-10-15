"""Command line of ``py/bench.py`` (see its docstring and ``py/README-bench.md``)."""

from __future__ import annotations

import argparse
import os
import sys
import tempfile

from . import envinfo, report
from .datasets import open_dataset
from .engines import (MODES, EngineSettings, create_engines, describe_extension,
                      parse_engine_list)
from .engines.sparqlite import FULL_COLUMNS, extension_candidates
from .queries import load_queries, select_queries
from .runner import RunConfig, Runner, RunResult

PY_DIR = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
REPO_ROOT = os.path.dirname(PY_DIR)

DEFAULT_QUERIES = os.path.join(PY_DIR, "queries")
DEFAULT_WORK_DIR = os.environ.get("SPARQLITE_BENCH_DIR") or os.path.join(
    tempfile.gettempdir(), "sparqlite-bench")


def resolve_extension(arg: str | None) -> str | None:
    """``--ext``, then ``$SPARQLITE_EXT``, then the default ``ext/`` build location."""
    if arg:
        return arg
    if os.environ.get("SPARQLITE_EXT"):
        return os.environ["SPARQLITE_EXT"]
    for candidate in extension_candidates(REPO_ROOT):
        for suffix in ("", ".so", ".dylib", ".dll"):
            if os.path.isfile(candidate + suffix):
                return candidate
    return extension_candidates(REPO_ROOT)[0]


def _key_value(text: str) -> tuple[str, str]:
    key, sep, value = text.partition("=")
    if not sep or not key:
        raise argparse.ArgumentTypeError(f"expected KEY=VALUE, got {text!r}")
    return key.strip(), value.strip()


def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(
        description="Benchmark sparqlite against its SQL baseline, pyoxigraph and rdflib.",
        formatter_class=argparse.ArgumentDefaultsHelpFormatter)
    data = ap.add_argument_group("data and queries")
    data.add_argument("--social", metavar="NT", help="social dataset (py/gen_social.py output)")
    data.add_argument("--bsbm", metavar="NT", help="bsbm-lite dataset (py/gen_bsbm.py output)")
    data.add_argument("--queries", default=DEFAULT_QUERIES, help="directory of .rq files")
    data.add_argument("--only", metavar="IDS", help="query subset, e.g. 'Q1,Q3-Q5'")
    eng = ap.add_argument_group("engines")
    eng.add_argument("--engines", default="native,baseline,pyoxigraph,rdflib",
                     help="comma-separated: native, baseline, pyoxigraph, rdflib (or 'all')")
    eng.add_argument("--modes", default="count,full",
                     help=f"comma-separated measurement modes from {', '.join(MODES)}")
    eng.add_argument("--ext", help="sparqlite extension path (default: $SPARQLITE_EXT, "
                     "then ext/target/release/libsparqlite)")
    eng.add_argument("--work-dir", default=DEFAULT_WORK_DIR,
                     help="SQLite databases and default outputs ($SPARQLITE_BENCH_DIR)")
    eng.add_argument("--reuse-db", action="store_true",
                     help="reuse a database built earlier from the same dataset and extension")
    eng.add_argument("--sqlite-load", choices=("auto", "file", "blob"), default="auto",
                     help="sparql_load_file (file) or sparql_load of the bytes (blob)")
    eng.add_argument("--full-columns", "--native-full", dest="full_columns",
                     choices=FULL_COLUMNS, default="terms",
                     help="full mode of both SQLite engines: 'terms' fetches one N-Triples "
                     "column per projected variable (like pyoxigraph's str() per term); "
                     "'star' fetches SELECT * from sparql_rows (all 51 columns decoded)")
    eng.add_argument("--native-config", metavar="KEY=VALUE", type=_key_value, action="append",
                     default=[], help="sparql_config(KEY, VALUE) on the native connection "
                     "(repeatable), e.g. join_policy=hash")
    eng.add_argument("--rdflib-max", type=int, default=100_000,
                     help="run rdflib only on datasets with at most this many triples")
    eng.add_argument("--no-isolate", action="store_true",
                     help="run pyoxigraph/rdflib in this process (then the timeout cannot stop them)")
    proto = ap.add_argument_group("protocol")
    proto.add_argument("--warmup", type=int, default=1, help="warm-up runs per cell")
    proto.add_argument("--repeat", type=int, default=5, help="timed runs per cell (median)")
    proto.add_argument("--timeout", type=float, default=600.0,
                       help="seconds per execution; SQLite engines are interrupted, Python "
                       "engines' worker processes killed (0 = none)")
    proto.add_argument("--cell-budget", type=float, default=120.0,
                       help="seconds per cell; slow queries get fewer repetitions (min 1)")
    out = ap.add_argument_group("output")
    out.add_argument("--out-md", help="markdown report (default: <work-dir>/bench-results.md)")
    out.add_argument("--out-json", help="raw JSON (default: <work-dir>/bench-results.json)")
    out.add_argument("--allow-mismatch", action="store_true",
                     help="exit 0 even if engines disagree on a result")
    out.add_argument("--allow-errors", action="store_true",
                     help="exit 0 even if an engine failed (probe, load or query)")
    out.add_argument("--quiet", action="store_true", help="no progress lines on stderr")
    ap.add_argument("--self-test", action="store_true",
                    help="run the unit tests and the whole pipeline on tiny generated data")
    return ap


def settings_from_args(args: argparse.Namespace) -> EngineSettings:
    return EngineSettings(
        work_dir=os.path.abspath(args.work_dir),
        ext_path=resolve_extension(args.ext),
        reuse_db=args.reuse_db,
        sqlite_load=args.sqlite_load,
        full_columns=args.full_columns,
        native_config=dict(args.native_config),
        rdflib_max=args.rdflib_max,
        timeout_s=args.timeout,
        isolate=not args.no_isolate,
    )


def parse_modes(spec: str) -> list[str]:
    modes = [m.strip() for m in spec.split(",") if m.strip()]
    bad = [m for m in modes if m not in MODES]
    if bad or not modes:
        raise ValueError(f"unknown modes {bad} (choose from {', '.join(MODES)})")
    return list(dict.fromkeys(modes))


def run_benchmark(args: argparse.Namespace, datasets: list[tuple[str, str]],
                  connect=None, argv: list[str] | None = None) -> tuple[RunResult, str, int]:
    """Runs the benchmark; returns (result, markdown, exit code).  ``argv`` is the
    command line to record (default ``sys.argv``)."""
    if args.repeat < 1 or args.warmup < 0:
        raise ValueError("--repeat must be >= 1 and --warmup >= 0")
    settings = settings_from_args(args)
    config = RunConfig(modes=parse_modes(args.modes), warmup=args.warmup, repeat=args.repeat,
                       timeout_s=args.timeout, cell_budget_s=args.cell_budget,
                       engine_settings=settings.to_json(),
                       argv=list(sys.argv if argv is None else argv))
    names = parse_engine_list(args.engines)
    queries = select_queries(load_queries(args.queries), args.only)
    os.makedirs(settings.work_dir, exist_ok=True)
    out_md = args.out_md or os.path.join(settings.work_dir, "bench-results.md")
    out_json = args.out_json or os.path.join(settings.work_dir, "bench-results.json")
    opened = [open_dataset(name, path) for name, path in datasets]
    engines = create_engines(names, settings, **({"connect": connect} if connect else {}))
    environment = envinfo.collect(REPO_ROOT, describe_extension(engines))
    runner = Runner(engines, config, progress=(lambda _msg: None) if args.quiet else _stderr,
                    on_update=lambda r: report.write_json(r, out_json))
    result = runner.run(opened, queries, environment)
    report.write_json(result, out_json)
    markdown = report.render_markdown(result)
    report.write_text(out_md, markdown)

    code = 0
    for failure in result.setup_errors():
        _stderr(f"FAILED: {failure}")
        code = code or (0 if args.allow_errors else 1)
    if result.mismatches():
        _stderr(f"FAILED: {len(result.mismatches())} queries with mismatching results "
                f"({', '.join(c.query for c in result.mismatches())})")
        code = code or (0 if args.allow_mismatch else 1)
    if result.errors():
        _stderr(f"FAILED: {len(result.errors())} cells with errors "
                f"({', '.join(sorted({m.where + ':' + m.query for m in result.errors()}))})")
        code = code or (0 if args.allow_errors else 1)
    if not args.quiet:
        _stderr(f"wrote {out_md} and {out_json}")
    return result, markdown, code


def _stderr(msg: str) -> None:
    print(msg, file=sys.stderr, flush=True)


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.self_test:
        from .selftest import run_self_test
        return run_self_test(args)
    datasets = [(name, getattr(args, name)) for name in ("social", "bsbm") if getattr(args, name)]
    if not datasets:
        build_parser().error("give --social and/or --bsbm (or --self-test)")
    command = [sys.argv[0], *(sys.argv[1:] if argv is None else argv)]
    try:
        _result, markdown, code = run_benchmark(args, datasets, argv=command)
    except (ValueError, FileNotFoundError) as e:
        _stderr(f"bench.py: {e}")
        return 2
    print(markdown)
    return code
