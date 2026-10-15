"""A description of the machine and software a benchmark ran on."""

from __future__ import annotations

import datetime as _dt
import hashlib
import os
import platform
import sqlite3
import subprocess
import sys


def _cpu_model() -> str | None:
    try:
        with open("/proc/cpuinfo", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("model name"):
                    return line.split(":", 1)[1].strip()
    except OSError:
        pass
    return platform.processor() or None


def _memory_bytes() -> int | None:
    try:
        with open("/proc/meminfo", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("MemTotal:"):
                    return int(line.split()[1]) * 1024
    except (OSError, ValueError, IndexError):
        pass
    return None


def _package_version(name: str) -> str | None:
    try:
        module = __import__(name)
    except ImportError:
        return None
    return getattr(module, "__version__", "unknown")


def _git(repo: str, *args: str) -> str | None:
    """Output of a read-only git command, None if git or the repository is unavailable.
    ``--no-optional-locks`` keeps ``status`` from rewriting the index, which other
    processes working in the same tree may be using."""
    try:
        out = subprocess.run(["git", "--no-optional-locks", "-C", repo, *args],
                             capture_output=True, text=True, timeout=30, check=True)
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout


def _git_revision(repo: str) -> str | None:
    """``HEAD`` of the repository."""
    out = _git(repo, "rev-parse", "HEAD")
    return (out or "").strip() or None


#: The benchmark's own files, relative to ``py/`` (the rest of ``py/`` is not the harness).
HARNESS_PATHS = ("bench.py", "gen_social.py", "gen_bsbm.py", "bench_lib", "queries")


def _git_changes(repo: str, paths: list[str]) -> list[str] | None:
    """``git status --porcelain`` lines (modified, added, untracked) under ``paths``:
    empty when those files are exactly the ones of ``HEAD``."""
    out = _git(repo, "status", "--porcelain", "--untracked-files=all", "--", *paths)
    return None if out is None else [line for line in out.splitlines() if line.strip()]


def harness_files(py_dir: str) -> list[str]:
    """The harness's ``.py`` and ``.rq`` files under ``py_dir`` (caches excluded), sorted."""
    files = []
    for rel in HARNESS_PATHS:
        path = os.path.join(py_dir, rel)
        if os.path.isfile(path):
            files.append(path)
        for directory, subdirs, names in os.walk(path):
            subdirs[:] = [d for d in subdirs if not d.startswith((".", "__"))]
            files += [os.path.join(directory, n) for n in names if n.endswith((".py", ".rq"))]
    return sorted(files)


def harness_digest(py_dir: str) -> str:
    """SHA-256 over the relative paths and contents of :func:`harness_files`: the harness
    and the queries exactly as they ran, with or without git."""
    digest = hashlib.sha256()
    for path in harness_files(py_dir):
        with open(path, "rb") as fh:
            data = fh.read()
        rel = os.path.relpath(path, py_dir).replace(os.sep, "/")
        digest.update(f"{rel}\0{len(data)}\0".encode())
        digest.update(data)
    return digest.hexdigest()


def collect(repo_root: str, extension: dict | None = None) -> dict:
    """Versions, CPU, memory and SQLite of this run; ``extension`` describes sparqlite.

    The harness and the queries are pinned by ``harness_sha256``
    (:func:`harness_digest`) and by ``git_harness_changes``, the harness paths that
    differ from ``git_revision``; the results also hold every query's text and hash.
    """
    py_dir = os.path.join(repo_root, "py")
    return {
        "timestamp_utc": _dt.datetime.now(_dt.timezone.utc).isoformat(timespec="seconds"),
        "python": sys.version.split()[0],
        "platform": platform.platform(),
        "cpu_model": _cpu_model(),
        "cpu_count": os.cpu_count(),
        "memory_bytes": _memory_bytes(),
        "sqlite_version": sqlite3.sqlite_version,
        "pyoxigraph": _package_version("pyoxigraph"),
        "rdflib": _package_version("rdflib"),
        "sparqlite": extension or {},
        "git_revision": _git_revision(repo_root),
        "git_harness_changes": _git_changes(repo_root, [f"py/{p}" for p in HARNESS_PATHS]),
        "harness_sha256": harness_digest(py_dir),
    }
