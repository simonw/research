"""Engine-independent query results and their checksums.

A :class:`ResultSet` holds canonical cell strings (see :mod:`bench_lib.terms`), so the
checksums below are equal across engines exactly when the results are equal as bags
(modulo blank-node labels):

* :func:`multiset_checksum` is order-insensitive and multiplicity-sensitive: the sum
  modulo 2^64 of a 64-bit hash of every row, plus the row count and, for non-empty
  results, the variable names.  Cells are keyed by variable name, so engines may
  project variables in any column order.
* :func:`sequence_checksum` also hashes the row order; it is compared only for queries
  whose ``ORDER BY`` is a total order (header ``# ordered: true``).
"""

from __future__ import annotations

import hashlib
from collections.abc import Iterable, Sequence
from dataclasses import dataclass, field

_MASK64 = (1 << 64) - 1
_CELL_SEP = "\x1f"
_ROW_SEP = "\x1e"


@dataclass
class ResultSet:
    """Canonicalised query results.

    ``form`` is ``select``, ``ask`` or ``graph`` (CONSTRUCT/DESCRIBE, with the
    variables ``subject``, ``predicate``, ``object``).  ``rows`` hold canonical cell
    strings in ``vars`` order; an ASK result has ``boolean`` set and no rows.
    """

    form: str
    vars: list[str] = field(default_factory=list)
    rows: list[tuple[str, ...]] = field(default_factory=list)
    boolean: bool | None = None

    def __len__(self) -> int:
        return 1 if self.form == "ask" else len(self.rows)


GRAPH_VARS = ["subject", "predicate", "object"]


def _row_key(row: Sequence[str], order: Sequence[int]) -> str:
    return _CELL_SEP.join(row[i] for i in order)


def _var_order(vars_: Sequence[str]) -> list[int]:
    if len(set(vars_)) != len(vars_):
        raise ValueError(f"duplicate variable names in {list(vars_)!r}")
    return sorted(range(len(vars_)), key=lambda i: vars_[i])


def _hash64(text: str) -> int:
    return int.from_bytes(hashlib.blake2b(text.encode("utf-8"), digest_size=8).digest(), "little")


def _header(rs: ResultSet) -> str:
    return _CELL_SEP.join(sorted(rs.vars))


def multiset_checksum(rs: ResultSet) -> str:
    """``"<rows>:<sum of row hashes>[:<vars hash>]"``, or ``"ask:true|false"``."""
    if rs.form == "ask":
        return f"ask:{'true' if rs.boolean else 'false'}"
    if not rs.rows:
        return "0:0000000000000000"
    order = _var_order(rs.vars)
    total = 0
    for row in rs.rows:
        total = (total + _hash64(_row_key(row, order))) & _MASK64
    return f"{len(rs.rows)}:{total:016x}:{_hash64(_header(rs)) & 0xFFFFFFFF:08x}"


def sequence_checksum(rs: ResultSet) -> str:
    """Like :func:`multiset_checksum` but sensitive to row order."""
    if rs.form == "ask":
        return multiset_checksum(rs)
    if not rs.rows:
        return "0:0000000000000000"
    order = _var_order(rs.vars)
    h = hashlib.blake2b(digest_size=8)
    h.update(_header(rs).encode("utf-8"))
    for row in rs.rows:
        h.update(_ROW_SEP.encode())
        h.update(_row_key(row, order).encode("utf-8"))
    return f"{len(rs.rows)}:{h.hexdigest()}"


def canonical_rows(rows: Iterable[Sequence[str | None]], canon) -> list[tuple[str, ...]]:
    """Applies ``canon`` (e.g. :func:`bench_lib.terms.canonical_nt`) to every cell."""
    return [tuple(canon(cell) for cell in row) for row in rows]
