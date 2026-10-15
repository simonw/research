"""Engine adapters and the registry the CLI selects from."""

from __future__ import annotations

from .base import (MODES, Engine, EngineSettings, EngineUnavailable, LoadInfo, QueryTimeout,
                   QueryUnsupported, Timed)
from .isolated import IsolatedEngine
from .oxigraph import PyoxigraphEngine
from .rdflib_engine import RdflibEngine
from .sparqlite import (ConnectFn, SparqliteBaseline, SparqliteDatabase, SparqliteLibrary,
                        SparqliteNative, default_connect)

#: Canonical engine names in table order, with the short aliases accepted by --engines.
ENGINE_ORDER = ("sparqlite-native", "sparqlite-baseline", "pyoxigraph", "rdflib")
ALIASES = {"native": "sparqlite-native", "baseline": "sparqlite-baseline",
           "oxigraph": "pyoxigraph", "ox": "pyoxigraph"}

__all__ = ["ALIASES", "ENGINE_ORDER", "MODES", "Engine", "EngineSettings", "EngineUnavailable",
           "IsolatedEngine", "LoadInfo", "QueryTimeout", "QueryUnsupported", "Timed",
           "create_engines", "describe_extension", "parse_engine_list"]


def parse_engine_list(spec: str) -> list[str]:
    """Canonical names from ``"native,baseline,pyoxigraph"`` (``all`` = every engine)."""
    names: list[str] = []
    for raw in filter(None, (p.strip().lower() for p in spec.split(","))):
        if raw == "all":
            names.extend(n for n in ENGINE_ORDER if n not in names)
            continue
        name = ALIASES.get(raw, raw)
        if name not in ENGINE_ORDER:
            raise ValueError(f"unknown engine {raw!r} (choose from {', '.join(ENGINE_ORDER)})")
        if name not in names:
            names.append(name)
    if not names:
        raise ValueError("no engines selected")
    return sorted(names, key=ENGINE_ORDER.index)


def create_engines(names: list[str], settings: EngineSettings,
                   connect: ConnectFn = default_connect) -> list[Engine]:
    """Instantiates the adapters.  Both sparqlite engines share one library and one
    database builder, so a dataset is loaded into SQLite once; they run in process
    (``sqlite3_interrupt`` enforces the timeout).  The Python engines run in a child
    process unless ``settings.isolate`` is off."""
    database = SparqliteDatabase(SparqliteLibrary(settings.ext_path, connect), settings)

    def python_engine(cls):
        return lambda: IsolatedEngine(cls.name, settings) if settings.isolate else cls(settings)

    factories = {
        "sparqlite-native": lambda: SparqliteNative(settings, database),
        "sparqlite-baseline": lambda: SparqliteBaseline(settings, database),
        "pyoxigraph": python_engine(PyoxigraphEngine),
        "rdflib": python_engine(RdflibEngine),
    }
    return [factories[name]() for name in names]


def describe_extension(engines: list[Engine]) -> dict:
    """What the run used as libsparqlite (for the environment record): path, file hash,
    ``sparql_version()``, and the probe's outcome per SQL call (``missing`` lists the
    calls that are not registered)."""
    for engine in engines:
        library = getattr(engine, "library", None)
        if isinstance(library, SparqliteLibrary):
            probe = library.probe()
            return {"requested": library.requested, "file": library.file,
                    "sha256": library.fingerprint()[:16] if library.file else None,
                    "version": probe.get("version"), "error": probe.get("error"),
                    "init_error": probe.get("init_error"), "calls": probe.get("calls", {}),
                    "missing": probe.get("missing", [])}
    return {}
