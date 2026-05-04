"""Python ctypes binding for the TRE regex library.

TRE (https://github.com/laurikari/tre) is a POSIX-compliant regex matcher
that uses a non-backtracking algorithm. Worst-case time is O(M^2 * N) for a
pattern of length M and input of length N — there is no exponential
blowup of the kind that PCRE/`re`-style backtrackers exhibit on
"catastrophic" patterns like `(a+)+$`.

This binding exposes a small subset of the TRE API:

    pat = tre_py.compile(r"a+", flags=tre_py.IGNORECASE)
    m = pat.search("AAA")
    m.group(0), m.span(), m.group(1), ...

Wide-character (`tre_regwexec`) is used internally so Python str inputs
work unchanged. Spans and group strings are reported in *Python str*
indices, not byte offsets.
"""
from __future__ import annotations

import ctypes
import ctypes.util
import os
import sys
from typing import Optional

__all__ = [
    "compile",
    "Pattern",
    "Match",
    "TreError",
    "IGNORECASE",
    "EXTENDED",
    "NEWLINE",
    "NOSUB",
]


# --- locate libtre ----------------------------------------------------------

def _find_libtre() -> str:
    # 1) explicit override
    env = os.environ.get("TRE_LIBRARY")
    if env:
        return env
    # 2) bundled next to the package or one directory up (dev layout)
    here = os.path.dirname(os.path.abspath(__file__))
    candidates = [
        os.path.join(here, "libtre.so"),
        os.path.join(here, "..", "..", "libtre.so"),
    ]
    for c in candidates:
        if os.path.isfile(c):
            return c
    # 3) system search
    found = ctypes.util.find_library("tre")
    if found:
        return found
    raise OSError(
        "Could not locate libtre.so. Set TRE_LIBRARY=/path/to/libtre.so."
    )


_lib = ctypes.CDLL(_find_libtre())


# --- C structs --------------------------------------------------------------

class _regex_t(ctypes.Structure):
    _fields_ = [
        ("re_nsub", ctypes.c_size_t),
        ("value", ctypes.c_void_p),
    ]


class _regmatch_t(ctypes.Structure):
    _fields_ = [
        ("rm_so", ctypes.c_int),  # regoff_t == int on TRE
        ("rm_eo", ctypes.c_int),
    ]


# --- function prototypes ----------------------------------------------------

_lib.tre_regwcomp.argtypes = [ctypes.POINTER(_regex_t), ctypes.c_wchar_p, ctypes.c_int]
_lib.tre_regwcomp.restype = ctypes.c_int

_lib.tre_regwnexec.argtypes = [
    ctypes.POINTER(_regex_t),
    ctypes.c_wchar_p,
    ctypes.c_size_t,
    ctypes.c_size_t,
    ctypes.POINTER(_regmatch_t),
    ctypes.c_int,
]
_lib.tre_regwnexec.restype = ctypes.c_int

_lib.tre_regfree.argtypes = [ctypes.POINTER(_regex_t)]
_lib.tre_regfree.restype = None

_lib.tre_regerror.argtypes = [
    ctypes.c_int,
    ctypes.POINTER(_regex_t),
    ctypes.c_char_p,
    ctypes.c_size_t,
]
_lib.tre_regerror.restype = ctypes.c_size_t


# --- flags (must match local_includes/tre.h) -------------------------------

EXTENDED = 1
IGNORECASE = 2
NEWLINE = 4
NOSUB = 8

# tre_regexec eflags (we do not currently expose these)
_REG_NOTBOL = 1
_REG_NOTEOL = 2

_REG_OK = 0
_REG_NOMATCH = 1


# --- public API -------------------------------------------------------------

class TreError(Exception):
    """Raised for compilation errors or unexpected matcher errors."""


def _strerror(code: int, preg: Optional[_regex_t]) -> str:
    buf = ctypes.create_string_buffer(256)
    _lib.tre_regerror(code, preg, buf, len(buf))
    return buf.value.decode("utf-8", errors="replace")


class Match:
    """A match object, mimicking a tiny piece of the `re.Match` API."""

    __slots__ = ("_string", "_spans")

    def __init__(self, string: str, spans: list[tuple[int, int]]):
        self._string = string
        self._spans = spans

    def span(self, group: int = 0) -> tuple[int, int]:
        return self._spans[group]

    def group(self, group: int = 0) -> Optional[str]:
        s, e = self._spans[group]
        if s == -1:
            return None
        return self._string[s:e]

    def __repr__(self) -> str:
        s, e = self._spans[0]
        return f"<tre_py.Match span=({s}, {e}) match={self._string[s:e]!r}>"


class Pattern:
    """A compiled TRE pattern."""

    __slots__ = ("_re", "_flags", "_pattern", "_n_subs")

    def __init__(self, pattern: str, flags: int = 0):
        # Default to extended POSIX, which is almost always what people mean.
        self._flags = flags | EXTENDED
        self._pattern = pattern
        self._re = _regex_t()
        rc = _lib.tre_regwcomp(ctypes.byref(self._re), pattern, self._flags)
        if rc != _REG_OK:
            msg = _strerror(rc, ctypes.byref(self._re))
            # Library docs say to call regfree even after a failed compile
            # only if the call partially succeeded; for safety, skip it.
            raise TreError(f"compile error: {msg}")
        self._n_subs = self._re.re_nsub

    def __del__(self):  # noqa: D401
        try:
            _lib.tre_regfree(ctypes.byref(self._re))
        except Exception:
            pass

    def search(self, string: str) -> Optional[Match]:
        """Search anywhere in `string`. Returns a Match or None."""
        n_groups = self._n_subs + 1  # group 0 + captures
        arr_t = _regmatch_t * n_groups
        arr = arr_t()
        rc = _lib.tre_regwnexec(
            ctypes.byref(self._re),
            string,
            len(string),
            n_groups,
            arr,
            0,
        )
        if rc == _REG_NOMATCH:
            return None
        if rc != _REG_OK:
            raise TreError(f"match error: {_strerror(rc, ctypes.byref(self._re))}")
        spans = [(arr[i].rm_so, arr[i].rm_eo) for i in range(n_groups)]
        return Match(string, spans)


def compile(pattern: str, flags: int = 0) -> Pattern:  # noqa: A001 (shadow)
    """Compile a regular expression pattern."""
    return Pattern(pattern, flags)
