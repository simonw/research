"""Markdown and JSON output of a benchmark run."""

from __future__ import annotations

import json
import math
import os
import shlex
import tempfile

from .runner import (ERROR, NOT_RUN, OK, SKIPPED, TIMEOUT, UNAVAILABLE, UNSUPPORTED, Measurement,
                     RunResult)

BASELINE = "sparqlite-baseline"
OXIGRAPH = "pyoxigraph"
NATIVE = "sparqlite-native"

MODE_TITLES = {
    "count": "count mode (solutions counted, no result term materialised)",
    "full": "full mode (one Python string per projected result term, on every engine)",
    "json": "json mode (one serialised results document: SPARQL-JSON, N-Triples for graph forms)",
}
STAR_FULL_TITLE = ("full mode, `SELECT *` variant (the SQLite engines decode all 51 "
                   "`sparql_rows` columns per row, pyoxigraph and rdflib one string per "
                   "projected term, so `×o` is not like-for-like)")
MODE_HOW = {
    "sparqlite-native": {"count": "`sparql_count(q)`", "json": "`sparql_query(q, 'json')`"},
    "sparqlite-baseline": {"count": "`sparql_count_sql(q)`",
                           "json": "`sparql_query_sql(q, 'json')`"},
    "pyoxigraph": {"count": "count of `Store.query(q)`", "full": "`str()` of every term",
                   "json": "`.serialize(JSON)`"},
    "rdflib": {"count": "count of `Graph.query(q)`", "full": "N-Triples text of every term",
               "json": "`.serialize('json')`"},
}
_STATUS_CELL = {UNSUPPORTED: "unsupported", UNAVAILABLE: "n/a", SKIPPED: "skipped",
                ERROR: "**error**", TIMEOUT: "timeout"}


def fmt_ms(ms: float) -> str:
    if ms < 1:
        return f"{ms:.3f}"
    if ms < 10:
        return f"{ms:.2f}"
    if ms < 100:
        return f"{ms:.1f}"
    return f"{ms:,.0f}"


def fmt_ratio(r: float) -> str:
    if r < 0.1:
        return f"{r:.2g}"
    return f"{r:.2f}" if r < 10 else f"{r:.1f}" if r < 100 else f"{r:,.0f}"


def _geomean(values: list[float]) -> float | None:
    values = [v for v in values if v > 0]
    if not values:
        return None
    return math.exp(sum(math.log(v) for v in values) / len(values))


def _engine_columns(result: RunResult) -> list[tuple[str, str]]:
    return [(e.name, e.short) for e in result.engines]


def _full_columns(result: RunResult) -> str:
    return result.config.engine_settings.get("full_columns", "terms")


def mode_title(result: RunResult, mode: str) -> str:
    if mode == "full" and _full_columns(result) == "star":
        return STAR_FULL_TITLE
    return MODE_TITLES.get(mode, mode)


def mode_how(result: RunResult, engine: str, mode: str) -> str:
    """How ``engine`` runs ``mode`` in this run (full mode depends on the settings)."""
    if mode == "full" and engine in (NATIVE, BASELINE):
        cols = "*" if _full_columns(result) == "star" else "t0..t{n-1}"
        options = ", '{\"engine\":\"sql\"}'" if engine == BASELINE else ""
        return f"`SELECT {cols} FROM sparql_rows(q{options})`"
    return MODE_HOW.get(engine, {}).get(mode, "?")


def _cell(m: Measurement | None, base: float | None, oxi: float | None, flagged: bool,
          requested: int) -> str:
    if m is None:
        return ""
    if m.status != OK:
        return _STATUS_CELL.get(m.status, m.status)
    med = m.median_ms
    text = fmt_ms(med)
    if len(m.times_ms) < requested:
        text += "†"
    ratios = []
    if m.engine != BASELINE and base and med > 0:
        ratios.append(f"{fmt_ratio(base / med)}×b")
    if m.engine != OXIGRAPH and oxi and med > 0:
        ratios.append(f"{fmt_ratio(oxi / med)}×o")
    if ratios:
        text += f" ({', '.join(ratios)})"
    if flagged:
        text += " **MISMATCH**"
    return text


def _reference(cells: list[Measurement]) -> tuple[int | None, str | None]:
    """The values other cells are compared with when a query mismatches: pyoxigraph's
    when it ran, else the most common ones."""
    ok = [m for m in cells if m.status == OK]
    for m in ok:
        if m.engine == OXIGRAPH and m.checksum is not None:
            return m.rows, m.checksum
    rows = [m.rows for m in ok]
    sums = [m.checksum for m in ok if m.checksum is not None]
    return (max(set(rows), key=rows.count) if rows else None,
            max(set(sums), key=sums.count) if sums else None)


def _mode_table(result: RunResult, mode: str) -> list[str]:
    columns = _engine_columns(result)
    requested = result.config.repeat
    lines = [f"### {mode_title(result, mode)}", ""]
    hows = [f"{short}: {mode_how(result, name, mode)}" for name, short in columns]
    lines.append("Median ms of " + f"{requested} runs after {result.config.warmup} warm-up. "
                 + "; ".join(hows) + ".")
    lines.append("")
    lines.append("| Query | Data | Rows | " + " | ".join(short for _, short in columns) + " |")
    lines.append("|---|---|---:|" + "---:|" * len(columns))
    ratios: dict[tuple[str, str], list[float]] = {}
    checks = {c.query: c for c in result.checks}
    for q in result.queries:
        cells = [m for m in result.measurements if m.query == q.id and m.mode == mode]
        if not cells:
            continue
        by_engine = {m.engine: m for m in cells}
        base = by_engine.get(BASELINE)
        oxi = by_engine.get(OXIGRAPH)
        base_ms = base.median_ms if base else None
        oxi_ms = oxi.median_ms if oxi else None
        check = checks.get(q.id)
        mismatch = check is not None and check.status == "mismatch"
        ref_rows, ref_sum = _reference(
            [m for m in result.measurements if m.query == q.id]) if mismatch else (None, None)
        row = [q.label, q.dataset]
        ok_rows = [m.rows for m in cells if m.status == OK]
        row.append("?" if mismatch else (f"{ok_rows[0]:,}" if ok_rows else ""))
        for name, _ in columns:
            m = by_engine.get(name)
            flagged = bool(mismatch and m is not None and m.status == OK and
                           (m.rows != ref_rows or (m.checksum is not None and m.checksum != ref_sum)))
            row.append(_cell(m, base_ms, oxi_ms, flagged, requested))
            if m is not None and m.status == OK and m.median_ms:
                for ref_name, ref_ms in ((BASELINE, base_ms), (OXIGRAPH, oxi_ms)):
                    if ref_ms and name != ref_name:
                        ratios.setdefault((name, ref_name), []).append(ref_ms / m.median_ms)
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")
    summary = []
    for (name, ref), values in sorted(ratios.items()):
        g = _geomean(values)
        if g is not None:
            short = dict(columns).get(name, name)
            ref_short = dict(columns).get(ref, ref)
            summary.append(f"{short} vs {ref_short}: {fmt_ratio(g)}× over {len(values)} queries")
    if summary:
        lines.append("Geometric-mean speed-up (>1 = faster): " + "; ".join(summary) + ".")
        lines.append("")
    return lines


def _load_table(result: RunResult) -> list[str]:
    columns = _engine_columns(result)
    lines = ["### Datasets and load times", ""]
    lines.append("| Dataset | Triples | File MB | SHA-256 | " + " | ".join(s for _, s in columns) + " |")
    lines.append("|---|---:|---:|---|" + "---:|" * len(columns))
    notes = []
    for ds in result.datasets:
        if not any(q.dataset == ds.name for q in result.queries):
            continue
        row = [ds.name, f"{ds.triples:,}", f"{ds.bytes / 1e6:.1f}", f"`{ds.fingerprint[:12]}`"]
        for name, _ in columns:
            rec = next((r for r in result.loads if r.dataset == ds.name and r.engine == name), None)
            if rec is None:
                row.append("n/a")
            elif rec.status != OK:
                row.append(_STATUS_CELL.get(rec.status, rec.status))
            elif rec.info.shared_with:
                short = dict(columns).get(rec.info.shared_with, rec.info.shared_with)
                row.append(f"(same DB as {short})")
            else:
                info = rec.info
                cell = f"{info.seconds:.2f} s"
                d = info.details
                if "db_bytes" in d:
                    cell += f", {d['db_bytes'] / 1e6:.0f} MB"
                    notes.append(f"{name} on {ds.name}: load {d.get('load_s', 0):.2f} s "
                                 f"({d.get('load_mode', '?')}), ANALYZE {d.get('analyze_sql_s', 0):.2f} s, "
                                 f"sparql_analyze {d.get('sparql_analyze_s', 0):.2f} s"
                                 + (" (database reused from an earlier run)" if d.get("reused") else ""))
                if info.triples is not None and info.triples != ds.triples:
                    cell += f" ({info.triples:,} triples!)"
                row.append(cell)
        lines.append("| " + " | ".join(row) + " |")
    lines.append("")
    lines.extend(f"- {n}" for n in notes)
    if notes:
        lines.append("")
    return lines


def _checks_table(result: RunResult) -> list[str]:
    lines = ["### Result checks", "",
             "Row counts and order-insensitive checksums (canonical terms, blank nodes "
             "masked) must agree across every engine and mode; `ordered` queries also "
             "compare the row sequence.", "",
             "| Query | Status | Rows | Checksum | Cells compared |", "|---|---|---:|---|---|"]
    by_query = {q.id: q for q in result.queries}
    for c in sorted(result.checks, key=lambda c: by_query[c.query].number if c.query in by_query else 0):
        q = by_query.get(c.query)
        label = q.label if q else c.query
        status = {"mismatch": "**MISMATCH**", "single": "single engine",
                  "none": "no results"}.get(c.status, c.status)
        if q is not None and q.ordered and c.status == "ok":
            status += " (ordered)"
        rows = "" if c.rows is None or c.status == "mismatch" else f"{c.rows:,}"
        lines.append(f"| {label} | {status} | {rows} | `{c.checksum or ''}` | "
                     f"{len(c.participants)} |")
    lines.append("")
    for c in result.mismatches():
        lines.append(f"- **{c.query} MISMATCH**: " + "; ".join(c.details))
    if result.mismatches():
        lines.append("")
    return lines


def _notes(result: RunResult) -> list[str]:
    seen: dict[tuple[str, str, str], list[str]] = {}
    for e in result.engines:
        if e.status != OK:
            seen.setdefault((e.name, e.status, e.message or ""), []).append("all")
    for m in result.measurements:
        if m.status not in (OK,) and m.message:
            key = (m.engine, m.status, m.message)
            if m.message.startswith(NOT_RUN):
                engine_key = (m.engine, m.status, m.message[len(NOT_RUN):])
                if seen.get(engine_key) == ["all"]:
                    continue  # the engine's own note already covers its cells
            ids = seen.setdefault(key, [])
            tag = f"{m.query}/{m.mode}"
            if "all" not in ids and tag not in ids:
                ids.append(tag)
    if not seen:
        return []
    lines = ["### Notes", ""]
    for (engine, status, message), where in seen.items():
        scope = "" if where == ["all"] else f" ({', '.join(where[:12])}{' ...' if len(where) > 12 else ''})"
        lines.append(f"- {engine} {status}{scope}: {message}")
    lines.append("")
    return lines


def settings_line(result: RunResult) -> str:
    """One line with everything that changes what the columns measure."""
    cfg = result.config
    parts = [f"modes {', '.join(cfg.modes)}",
             f"median of {cfg.repeat} after {cfg.warmup} warm-up",
             f"timeout {cfg.timeout_s:g} s", f"cell budget {cfg.cell_budget_s:g} s"]
    s = cfg.engine_settings
    if s:
        native = s.get("native_config") or {}
        parts.append(f"full columns `{s.get('full_columns')}`")
        parts.append("native config " + (", ".join(f"`{k}={v}`" for k, v in native.items())
                                         if native else "default"))
        parts.append(f"SQLite load `{s.get('sqlite_load')}`"
                     + (" (databases reused)" if s.get("reuse_db") else ""))
        parts.append("pyoxigraph/rdflib " + ("in worker processes" if s.get("isolate")
                                             else "in process"))
        parts.append(f"rdflib up to {s.get('rdflib_max', 0):,} triples")
    line = "Settings: " + "; ".join(parts) + "."
    if cfg.argv:
        line += f" Command: `{shlex.join(cfg.argv)}`."
    return line


def render_markdown(result: RunResult) -> str:
    env = result.environment
    ext = env.get("sparqlite") or {}
    lines = ["## sparqlite benchmark", ""]
    versions = [f"SQLite {env.get('sqlite_version')}", f"Python {env.get('python')}"]
    for name in ("pyoxigraph", "rdflib"):
        if env.get(name):
            versions.append(f"{name} {env[name]}")
    if ext.get("version"):
        versions.append(str(ext["version"]))
    lines.append(f"Run {env.get('timestamp_utc', '?')} on {env.get('cpu_model', '?')} "
                 f"({env.get('cpu_count', '?')} CPUs, "
                 f"{(env.get('memory_bytes') or 0) / 2**30:.0f} GiB); " + ", ".join(versions)
                 + (f"; git {env['git_revision'][:12]}" if env.get("git_revision") else "") + ".")
    lines.append("")
    lines.append(settings_line(result))
    lines.append("")
    lines.append("Speed-ups in parentheses are the other engine's time divided by this "
                 "engine's (>1 = this engine is faster): `×b` vs sparqlite-baseline, "
                 "`×o` vs pyoxigraph. † = fewer repetitions than requested "
                 f"(cell budget {result.config.cell_budget_s:g} s).")
    lines.append("")
    lines.extend(_load_table(result))
    for mode in result.config.modes:
        lines.extend(_mode_table(result, mode))
    lines.extend(_checks_table(result))
    lines.extend(_notes(result))
    return "\n".join(lines).rstrip() + "\n"


def write_text(path: str, text: str) -> None:
    """Writes atomically (the JSON is rewritten after every query)."""
    directory = os.path.dirname(os.path.abspath(path))
    os.makedirs(directory, exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=directory, prefix=".bench-", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as fh:
            fh.write(text)
        os.replace(tmp, path)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise


def write_json(result: RunResult, path: str) -> None:
    write_text(path, json.dumps(result.to_json(), indent=1, default=str) + "\n")
