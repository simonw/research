"""A description of the machine and software a benchmark ran on."""

from __future__ import annotations

import datetime as _dt
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


def _git_revision(repo: str) -> str | None:
    """``HEAD`` of the repository (read-only ``git rev-parse``)."""
    try:
        out = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"], capture_output=True,
                             text=True, timeout=10, check=True)
    except (OSError, subprocess.SubprocessError):
        return None
    return out.stdout.strip() or None


def collect(repo_root: str, extension: dict | None = None) -> dict:
    """Versions, CPU, memory and SQLite of this run; ``extension`` describes sparqlite."""
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
    }
