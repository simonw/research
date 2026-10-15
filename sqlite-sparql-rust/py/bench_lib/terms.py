"""RDF terms as engines hand them to Python, and their engine-independent canonical form.

Engines disagree on surface syntax that does not change meaning: pyoxigraph and rdflib
canonicalise numeric and dateTime lexical forms on load (``"12.50"`` -> ``"12.5"``,
``"01"`` -> ``"1"``), lower-case language tags, and rename blank nodes, while
sparqlite keeps lexical forms exactly as loaded.  :func:`canonical` maps a term to a
string that is equal across engines exactly when the terms are the same value:

* IRIs stay as ``<iri>``;
* every blank node maps to the constant ``_:`` (labels cannot be compared across
  engines; a result-wide bijection is the W3C runner's job, not the benchmark's);
* numerics are normalised by value within their datatype family (all integer-derived
  types form one family; decimal, float and double are separate families);
* booleans, ``xsd:dateTime``, ``xsd:date`` and ``xsd:time`` are normalised to their
  canonical lexical forms (trailing fraction zeros dropped, ``+00:00`` -> ``Z``, the
  end-of-day ``24:00:00`` written as ``00:00:00`` of the next day);
* ``xsd:string`` and simple literals are the same term (RDF 1.1); language tags are
  compared case-insensitively;
* ill-typed literals keep their exact lexical form.
"""

from __future__ import annotations

import json
import math
import re
import struct
from dataclasses import dataclass
from decimal import Decimal, InvalidOperation
from functools import lru_cache

XSD = "http://www.w3.org/2001/XMLSchema#"
RDF = "http://www.w3.org/1999/02/22-rdf-syntax-ns#"
XSD_STRING = XSD + "string"
RDF_LANG_STRING = RDF + "langString"

INTEGER_TYPES = frozenset(XSD + t for t in (
    "integer", "nonPositiveInteger", "negativeInteger", "long", "int", "short", "byte",
    "nonNegativeInteger", "unsignedLong", "unsignedInt", "unsignedShort", "unsignedByte",
    "positiveInteger"))

UNBOUND = "UNDEF"
BNODE = "_:"

_INTEGER_RE = re.compile(r"[+-]?[0-9]+")
_DECIMAL_RE = re.compile(r"[+-]?([0-9]+(\.[0-9]*)?|\.[0-9]+)")
_DOUBLE_RE = re.compile(r"[+-]?(([0-9]+(\.[0-9]*)?|\.[0-9]+)([eE][+-]?[0-9]+)?|INF)|NaN")
_TZ = r"(Z|[+-](?:\d\d):(?:\d\d))?"
_DATETIME_RE = re.compile(r"(-?\d{4,}-\d\d-\d\d)T(\d\d:\d\d:\d\d)(\.\d+)?" + _TZ)
_DATE_RE = re.compile(r"(-?\d{4,}-\d\d-\d\d)" + _TZ)
_TIME_RE = re.compile(r"(\d\d:\d\d:\d\d)(\.\d+)?" + _TZ)

_ECHAR = {"t": "\t", "b": "\b", "n": "\n", "r": "\r", "f": "\f", '"': '"', "'": "'", "\\": "\\"}


@dataclass(frozen=True, slots=True)
class Term:
    """An RDF term: ``kind`` is ``iri``, ``bnode`` or ``literal``.

    Literals always carry a datatype (``xsd:string`` for simple literals,
    ``rdf:langString`` with a language tag).
    """

    kind: str
    value: str
    datatype: str | None = None
    lang: str | None = None

    @staticmethod
    def iri(value: str) -> Term:
        return Term("iri", value)

    @staticmethod
    def bnode(label: str) -> Term:
        return Term("bnode", label)

    @staticmethod
    def literal(value: str, datatype: str | None = None, lang: str | None = None) -> Term:
        if lang:
            return Term("literal", value, RDF_LANG_STRING, lang)
        return Term("literal", value, datatype or XSD_STRING, None)


# ---------------------------------------------------------------------------- parsing

def _unescape(text: str, i: int, end_char: str) -> tuple[str, int]:
    """Reads up to the unescaped ``end_char`` starting at ``i``; returns (text, index after)."""
    out: list[str] = []
    n = len(text)
    start = i
    while i < n:
        c = text[i]
        if c == end_char:
            out.append(text[start:i])
            return "".join(out), i + 1
        if c == "\\":
            out.append(text[start:i])
            if i + 1 >= n:
                break
            e = text[i + 1]
            if e in ("u", "U"):
                width = 4 if e == "u" else 8
                digits = text[i + 2:i + 2 + width]
                if len(digits) != width:
                    raise ValueError(f"truncated \\{e} escape in {text!r}")
                out.append(chr(int(digits, 16)))
                i += 2 + width
            elif e in _ECHAR and end_char == '"':
                out.append(_ECHAR[e])
                i += 2
            else:
                raise ValueError(f"invalid escape \\{e} in {text!r}")
            start = i
            continue
        i += 1
    raise ValueError(f"unterminated term {text!r}")


def parse_nt(text: str) -> Term:
    """Parses one N-Triples term (``<iri>``, ``_:label`` or a literal)."""
    s = text.strip()
    if not s:
        raise ValueError("empty term")
    head = s[0]
    if head == "<":
        value, end = _unescape(s, 1, ">")
        if end != len(s):
            raise ValueError(f"trailing characters after IRI in {text!r}")
        return Term.iri(value)
    if s.startswith("_:"):
        if len(s) == 2 or any(c.isspace() for c in s):
            raise ValueError(f"invalid blank node {text!r}")
        return Term.bnode(s[2:])
    if head == '"':
        value, end = _unescape(s, 1, '"')
        rest = s[end:]
        if not rest:
            return Term.literal(value)
        if rest.startswith("@") and len(rest) > 1:
            return Term.literal(value, lang=rest[1:])
        if rest.startswith("^^<") and rest.endswith(">"):
            dt, dt_end = _unescape(rest, 3, ">")
            if dt_end != len(rest):
                raise ValueError(f"trailing characters after datatype in {text!r}")
            return Term.literal(value, dt)
        raise ValueError(f"invalid literal suffix in {text!r}")
    raise ValueError(f"not an N-Triples term: {text!r}")


def from_sparql_json(binding: dict) -> Term:
    """Converts one SPARQL 1.1 Query Results JSON binding object."""
    kind = binding.get("type")
    value = binding.get("value")
    if not isinstance(value, str):
        raise ValueError(f"binding without a string value: {binding!r}")
    if kind == "uri":
        return Term.iri(value)
    if kind == "bnode":
        return Term.bnode(value)
    if kind in ("literal", "typed-literal"):
        return Term.literal(value, binding.get("datatype"), binding.get("xml:lang"))
    raise ValueError(f"unknown binding type {kind!r}")


# ---------------------------------------------------------------------- serialising

def escape_literal(value: str) -> str:
    """N-Triples string escaping (ECHARs for the characters that need them)."""
    return (value.replace("\\", "\\\\").replace('"', '\\"').replace("\n", "\\n")
            .replace("\r", "\\r"))


def to_nt(term: Term) -> str:
    if term.kind == "iri":
        return f"<{term.value}>"
    if term.kind == "bnode":
        return f"_:{term.value}"
    lex = f'"{escape_literal(term.value)}"'
    if term.lang:
        return f"{lex}@{term.lang}"
    if term.datatype and term.datatype != XSD_STRING:
        return f"{lex}^^<{term.datatype}>"
    return lex


# ---------------------------------------------------------------------- canonical form

def _fraction(frac: str | None) -> str:
    if not frac:
        return ""
    frac = frac.rstrip("0")
    return "" if frac == "." else frac


def _days_in_month(year: int, month: int) -> int:
    """Proleptic Gregorian, with XSD 1.1 year numbering (year 0 = 1 BCE is a leap year)."""
    if month == 2:
        return 29 if year % 4 == 0 and (year % 100 != 0 or year % 400 == 0) else 28
    return 30 if month in (4, 6, 9, 11) else 31


def _next_day(date: str) -> str:
    """``[-]YYYY-MM-DD`` plus one day (years may be negative or longer than 4 digits)."""
    negative = date.startswith("-")
    year_text, month_text, day_text = (date[1:] if negative else date).split("-")
    year = -int(year_text) if negative else int(year_text)
    month, day = int(month_text), int(day_text) + 1
    if day > _days_in_month(year, month):
        day, month = 1, month + 1
    if month > 12:
        month, year = 1, year + 1
    return f"{'-' if year < 0 else ''}{abs(year):04d}-{month:02d}-{day:02d}"


def _end_of_day(time: str, frac: str | None) -> bool | None:
    """True for the end-of-day ``24:00:00`` (XSD: same instant as the next day's
    ``00:00:00``), None when hour 24 comes with a non-zero fraction (ill-typed)."""
    if not time.startswith("24:"):
        return False
    return True if time == "24:00:00" and not _fraction(frac) else None


def _tz(tz: str | None) -> str:
    if tz in ("+00:00", "-00:00"):
        return "Z"
    return tz or ""


def _float_value(lex: str, single: bool) -> str:
    if lex == "NaN":
        return "NaN"
    v = float(lex.replace("INF", "inf"))
    if single and math.isfinite(v):
        try:
            v = struct.unpack("<f", struct.pack("<f", v))[0]
        except OverflowError:
            v = math.copysign(math.inf, v)
    if math.isinf(v):
        return "INF" if v > 0 else "-INF"
    return repr(v)


def _canonical_typed(lex: str, dt: str) -> str | None:
    """Canonical value key for a typed literal, or None when ill-typed/unknown."""
    if dt in INTEGER_TYPES:
        if _INTEGER_RE.fullmatch(lex):
            return f"integer:{int(lex)}"
        return None
    if dt == XSD + "decimal":
        if _DECIMAL_RE.fullmatch(lex):
            try:
                d = Decimal(lex)
            except InvalidOperation:
                return None
            if d == 0:
                return "decimal:0"
            return "decimal:" + format(d.normalize(), "f")
        return None
    if dt in (XSD + "double", XSD + "float"):
        if _DOUBLE_RE.fullmatch(lex):
            family = "float" if dt == XSD + "float" else "double"
            return f"{family}:{_float_value(lex, family == 'float')}"
        return None
    if dt == XSD + "boolean":
        if lex in ("true", "1"):
            return "boolean:true"
        if lex in ("false", "0"):
            return "boolean:false"
        return None
    if dt == XSD + "dateTime":
        m = _DATETIME_RE.fullmatch(lex)
        if not m:
            return None
        date, time, frac, tz = m.groups()
        end = _end_of_day(time, frac)
        if end is None:
            return None
        if end:
            date, time = _next_day(date), "00:00:00"
        return f"dateTime:{date}T{time}{_fraction(frac)}{_tz(tz)}"
    if dt == XSD + "date":
        m = _DATE_RE.fullmatch(lex)
        return f"date:{m.group(1)}{_tz(m.group(2))}" if m else None
    if dt == XSD + "time":
        m = _TIME_RE.fullmatch(lex)
        if not m:
            return None
        time, frac, tz = m.groups()
        end = _end_of_day(time, frac)
        if end is None:
            return None
        return f"time:{'00:00:00' if end else time}{_fraction(frac)}{_tz(tz)}"
    return None


def canonical(term: Term | None) -> str:
    """The engine-independent comparison key of a term (``UNDEF`` when unbound)."""
    if term is None:
        return UNBOUND
    if term.kind == "iri":
        return f"<{term.value}>"
    if term.kind == "bnode":
        return BNODE
    quoted = json.dumps(term.value, ensure_ascii=False)
    if term.lang:
        return f"{quoted}@{term.lang.lower()}"
    dt = term.datatype or XSD_STRING
    if dt == XSD_STRING:
        return quoted
    key = _canonical_typed(term.value, dt)
    return key if key is not None else f"{quoted}^^<{dt}>"


@lru_cache(maxsize=1 << 16)
def canonical_nt(text: str | None) -> str:
    """:func:`canonical` of an N-Triples term string (``None`` = unbound); memoised,
    because result columns repeat the same terms (classes, cities, predicates)."""
    return UNBOUND if text is None else canonical(parse_nt(text))
