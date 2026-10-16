"""Benchmark dataset files (N-Triples)."""

from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass

_CHUNK = 1 << 24


@dataclass(frozen=True)
class Dataset:
    """An N-Triples file plus what the harness needs to know about it.

    ``fingerprint`` is the SHA-256 of the file contents (hex), so two runs, on any
    machine, can confirm they benchmarked the same bytes; it also keys the reusable
    SQLite databases built from the file.
    """

    name: str
    path: str
    triples: int
    bytes: int
    fingerprint: str

    def to_json(self) -> dict:
        return {"name": self.name, "path": self.path, "triples": self.triples,
                "bytes": self.bytes, "sha256": self.fingerprint}


def scan(path: str) -> tuple[int, str]:
    """``(statements, SHA-256 hex digest)`` of an N-Triples file, in one pass.

    Statements are the non-blank, non-comment lines.
    """
    count = 0
    digest = hashlib.sha256()
    with open(path, "rb") as fh:
        tail = b""
        while chunk := fh.read(_CHUNK):
            digest.update(chunk)
            lines = (tail + chunk).split(b"\n")
            tail = lines.pop()
            count += sum(1 for line in lines if _is_statement(line))
        if _is_statement(tail):
            count += 1
    return count, digest.hexdigest()


def count_triples(path: str) -> int:
    """Number of statements in an N-Triples file (non-blank, non-comment lines)."""
    return scan(path)[0]


def _is_statement(line: bytes) -> bool:
    s = line.strip()
    return bool(s) and not s.startswith(b"#")


def open_dataset(name: str, path: str) -> Dataset:
    if not os.path.isfile(path):
        hint = {"social": "py/gen_social.py", "bsbm": "py/gen_bsbm.py"}.get(name)
        extra = f" (generate it with {hint} --out {path})" if hint else ""
        raise FileNotFoundError(f"dataset {name}: {path} does not exist{extra}")
    path = os.path.abspath(path)
    triples, sha256 = scan(path)
    return Dataset(name=name, path=path, triples=triples, bytes=os.path.getsize(path),
                   fingerprint=sha256)
