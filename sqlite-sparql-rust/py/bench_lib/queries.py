"""Benchmark query files (``py/queries/*.rq``).

Each file holds one SPARQL query preceded by a comment header::

    # Q8: GROUP BY + COUNT + ORDER BY DESC LIMIT 10
    # dataset: social
    # ordered: true
    # free-form description lines ...
    PREFIX : <http://example.org/social/>
    SELECT ...

The first line names the query (``Q<n>: <title>``); ``dataset`` is ``social`` or
``bsbm``; ``ordered: true`` marks a total ``ORDER BY`` whose row sequence must match
across engines.  The header lines are ordinary SPARQL comments, so the whole file is
sent to the engines unchanged.
"""

from __future__ import annotations

import os
import re
from dataclasses import dataclass
from functools import lru_cache

DATASETS = ("social", "bsbm")
_TITLE_RE = re.compile(r"#\s*(Q\d+)\s*:\s*(.+)")
_KEY_RE = re.compile(r"#\s*([a-z_]+)\s*:\s*(.*)")
_ID_RE = re.compile(r"[Qq]?(\d+)")
_FORM_RE = re.compile(r"(?<![\w:?$])(SELECT|ASK|CONSTRUCT|DESCRIBE)(?![\w:])", re.IGNORECASE)
# Tokens of a SELECT clause: variables, brackets, and any other run of characters.
_SELECT_TOKEN_RE = re.compile(r"[?$][\w\u00B7]+|[(){}*]|[^\s(){}*]+")


@dataclass(frozen=True)
class BenchQuery:
    id: str
    title: str
    dataset: str
    ordered: bool
    form: str
    text: str
    path: str

    @property
    def number(self) -> int:
        return int(self.id[1:])

    @property
    def label(self) -> str:
        return f"{self.id} {self.title}"

    @property
    def projection(self) -> tuple[str, ...] | None:
        """The projected variables of a SELECT query (see :func:`query_projection`)."""
        return query_projection(self.text) if self.form == "select" else None


def _strip_lexical(text: str) -> str:
    """``text`` with IRIs, strings and comments blanked out, so keywords and variables
    inside them are not mistaken for syntax."""
    stripped = re.sub(r"<[^<>\s]*>", "<>", text)
    stripped = re.sub(r'"(?:[^"\\\n]|\\.)*"' + r"|'(?:[^'\\\n]|\\.)*'", '""', stripped)
    return re.sub(r"#[^\n]*", "", stripped)


def query_form(text: str) -> str:
    """``select``, ``ask``, ``construct`` or ``describe`` (the first query keyword
    outside IRIs, strings and comments)."""
    m = _FORM_RE.search(_strip_lexical(text))
    if not m:
        raise ValueError("no SELECT/ASK/CONSTRUCT/DESCRIBE keyword found")
    return m.group(1).lower()


@lru_cache(maxsize=256)
def query_projection(text: str) -> tuple[str, ...] | None:
    """The variables an explicit ``SELECT ?a (expr AS ?b) ...`` projects, in order.

    None for ``SELECT *``, for other query forms and for anything this small lexer does
    not recognise; callers then fall back to asking the engine.  The SQLite adapters
    use it to fetch exactly the projected ``t0..t{n-1}`` columns of ``sparql_rows``
    (the k-th column is the k-th projected variable, docs/SPEC.md §8.3).
    """
    stripped = _strip_lexical(text)
    m = _FORM_RE.search(stripped)
    if not m or m.group(1).upper() != "SELECT":
        return None
    out: list[str] = []
    depth = 0
    alias: str | None = None
    expect_alias = False
    tokens = _SELECT_TOKEN_RE.findall(stripped, m.end())
    for i, tok in enumerate(tokens):
        upper = tok.upper()
        if tok == "(":
            depth += 1
            if depth == 1:
                alias, expect_alias = None, False
        elif tok == ")":
            if depth == 0:
                return None
            if depth == 1:
                if alias is None:
                    return None
                out.append(alias)
            depth -= 1
        elif depth == 0:
            if tok[0] in "?$":
                out.append(tok[1:])
            elif upper in ("WHERE", "FROM") or tok == "{":
                break
            elif upper in ("DISTINCT", "REDUCED") and i == 0:
                continue
            else:  # "*" or something unexpected
                return None
        elif depth == 1:
            if upper == "AS":
                expect_alias = True
            elif expect_alias:
                if tok[0] not in "?$":
                    return None
                alias, expect_alias = tok[1:], False
    else:
        return None
    return tuple(out) if out and len(set(out)) == len(out) else None


def parse_query_file(path: str) -> BenchQuery:
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    lines = text.splitlines()
    if not lines:
        raise ValueError(f"{path}: empty query file")
    m = _TITLE_RE.fullmatch(lines[0].strip())
    if not m:
        raise ValueError(f"{path}: first line must be '# Q<n>: <title>'")
    qid, title = m.group(1), m.group(2).strip()
    meta: dict[str, str] = {}
    for line in lines[1:]:
        line = line.strip()
        if not line.startswith("#"):
            break
        km = _KEY_RE.fullmatch(line)
        if km and km.group(1) in ("dataset", "ordered"):
            meta[km.group(1)] = km.group(2).strip()
    dataset = meta.get("dataset")
    if dataset not in DATASETS:
        raise ValueError(f"{path}: header must name the dataset ({' or '.join(DATASETS)})")
    ordered_text = meta.get("ordered", "false").lower()
    if ordered_text not in ("true", "false", "yes", "no"):
        raise ValueError(f"{path}: 'ordered' must be true or false")
    return BenchQuery(id=qid, title=title, dataset=dataset,
                      ordered=ordered_text in ("true", "yes"), form=query_form(text),
                      text=text, path=os.path.abspath(path))


def load_queries(directory: str) -> list[BenchQuery]:
    """All ``*.rq`` files in ``directory``, sorted by query number."""
    queries = [parse_query_file(os.path.join(directory, name))
               for name in os.listdir(directory) if name.endswith(".rq")]
    queries.sort(key=lambda q: q.number)
    seen: set[str] = set()
    for q in queries:
        if q.id in seen:
            raise ValueError(f"duplicate query id {q.id} in {directory}")
        seen.add(q.id)
    return queries


def parse_selection(spec: str) -> set[int]:
    """Query numbers from ``"Q1,Q3-Q5,7"``."""
    numbers: set[int] = set()
    for part in filter(None, (p.strip() for p in spec.split(","))):
        bounds = part.split("-")
        if len(bounds) > 2:
            raise ValueError(f"bad query range {part!r}")
        ends = []
        for b in bounds:
            m = _ID_RE.fullmatch(b.strip())
            if not m:
                raise ValueError(f"bad query id {b!r}")
            ends.append(int(m.group(1)))
        lo, hi = ends[0], ends[-1]
        if lo > hi:
            raise ValueError(f"empty query range {part!r}")
        numbers.update(range(lo, hi + 1))
    return numbers


def select_queries(queries: list[BenchQuery], spec: str | None) -> list[BenchQuery]:
    if not spec:
        return list(queries)
    wanted = parse_selection(spec)
    known = {q.number for q in queries}
    missing = sorted(wanted - known)
    if missing:
        raise ValueError(f"unknown queries: {', '.join(f'Q{n}' for n in missing)}")
    return [q for q in queries if q.number in wanted]
