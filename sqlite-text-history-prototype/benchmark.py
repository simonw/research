#!/usr/bin/env python3
"""Benchmark compressed text histories in SQLite.

The workload is deterministic and synthetic: a ~20 KB pseudo-English document
receives mostly small edits, with an occasional paragraph-sized replacement.
Every edit is committed as its own SQLite transaction.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import random
import shutil
import sqlite3
import statistics
import string
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Protocol

from text_history import (
    ChunkedHistoryStore,
    Version,
    WholeBlobHistoryStore,
    append_payload,
    decode_payload,
    make_codec,
)

WORDS = """
the of and to in a is that for it as with was on be by this are or from at an
not have has but which you one were all their can more when will would there
what about if who so no out up into do time some could them other than then now
only its over also after first new because these two may any like how our work
use data text document version edit system history sqlite storage compression
row column change previous current value simple efficient database write read
page small large each many most every another while between through before
where why people make made very much even well good way ideas design code python
table record timestamp array blob binary json zstd zlib same different around
keep without need should might still content paragraph sentence update revision
application user source field information project local server client process
result model state operation structure format memory disk transaction index
""".split()


@dataclass
class Workload:
    versions: list[str]
    timestamps: list[int]

    @property
    def revision_count(self) -> int:
        return len(self.versions) - 1


def make_base_document(target_bytes: int, seed: int = 417) -> str:
    rng = random.Random(seed)
    pieces: list[str] = []
    current_bytes = 0
    sentence_count = 0
    while current_bytes < target_bytes + 256:
        word_count = rng.randint(8, 22)
        sentence = " ".join(rng.choice(WORDS) for _ in range(word_count)).capitalize()
        sentence += rng.choices([".", "?", ":"], weights=[8, 1, 1], k=1)[0]
        suffix = "\n\n" if sentence_count % 6 == 5 else " "
        piece = sentence + suffix
        pieces.append(piece)
        current_bytes += len(piece.encode("utf-8"))
        sentence_count += 1
    document = "".join(pieces)
    # The generator is ASCII, so slicing characters also slices bytes.
    return document[:target_bytes]


def _replacement_text(rng: random.Random, minimum_words: int, maximum_words: int) -> str:
    return " ".join(rng.choice(WORDS) for _ in range(rng.randint(minimum_words, maximum_words)))


def mutate_document(text: str, rng: random.Random, revision: int) -> str:
    """Apply one deterministic, plausibly editor-like mutation."""

    if revision and revision % 50 == 0:
        # Occasional paragraph-scale rewrite.
        replaced = rng.randint(220, 520)
        start = rng.randrange(0, max(1, len(text) - replaced))
        replacement = _replacement_text(rng, 35, 75)
        return text[:start] + replacement + text[start + replaced :]

    choice = rng.random()
    if choice < 0.62:
        # Small replacement, similar to changing a word or phrase.
        replaced = rng.randint(1, 24)
        start = rng.randrange(0, max(1, len(text) - replaced))
        replacement = _replacement_text(rng, 1, 5)
        return text[:start] + replacement + text[start + replaced :]
    if choice < 0.82:
        # Insert a short phrase or sentence fragment.
        start = rng.randrange(0, len(text) + 1)
        insertion = " " + _replacement_text(rng, 3, 14) + " "
        return text[:start] + insertion + text[start:]

    # Delete a short span.
    deleted = rng.randint(4, 90)
    start = rng.randrange(0, max(1, len(text) - deleted))
    return text[:start] + text[start + deleted :]


def make_workload(
    *, document_bytes: int, revisions: int, seed: int = 8128
) -> Workload:
    versions = [make_base_document(document_bytes)]
    rng = random.Random(seed)
    for revision in range(1, revisions + 1):
        versions.append(mutate_document(versions[-1], rng, revision))
    start_timestamp = 1_800_000_000
    timestamps = [start_timestamp + revision * 60 for revision in range(revisions + 1)]
    return Workload(versions=versions, timestamps=timestamps)


def percentile(values: list[float], fraction: float) -> float:
    if not values:
        return math.nan
    ordered = sorted(values)
    index = min(len(ordered) - 1, max(0, math.ceil(fraction * len(ordered)) - 1))
    return ordered[index]


def file_size(path: Path) -> int:
    try:
        return path.stat().st_size
    except FileNotFoundError:
        return 0


def compact_database_size(connection: sqlite3.Connection, path: Path) -> int:
    compact_path = path.with_suffix(path.suffix + ".compact")
    compact_path.unlink(missing_ok=True)
    escaped = str(compact_path).replace("'", "''")
    connection.execute(f"VACUUM INTO '{escaped}'")
    size = file_size(compact_path)
    compact_path.unlink(missing_ok=True)
    return size


def sqlite_stats(connection: sqlite3.Connection) -> dict[str, int]:
    page_size = int(connection.execute("PRAGMA page_size").fetchone()[0])
    page_count = int(connection.execute("PRAGMA page_count").fetchone()[0])
    freelist_count = int(connection.execute("PRAGMA freelist_count").fetchone()[0])
    return {
        "page_size": page_size,
        "page_count": page_count,
        "freelist_count": freelist_count,
        "database_pages_bytes": page_size * page_count,
        "freelist_bytes": page_size * freelist_count,
    }


class BenchmarkStore(Protocol):
    connection: sqlite3.Connection

    def create_document(self, text: str, timestamp: int) -> int: ...

    def replace(self, document_id: int, text: str, timestamp: int) -> int: ...

    def versions(self, document_id: int) -> list[Version]: ...

    def get_version(self, document_id: int, revision: int) -> Version: ...

    def storage_stats(self, document_id: int) -> dict[str, int]: ...

    def close(self) -> None: ...


class CurrentOnlyStore:
    """Reference strategy measuring the unavoidable cost of replacing current TEXT."""

    history_available = False

    def __init__(self, path: Path) -> None:
        self.connection = sqlite3.connect(path, isolation_level=None)
        self.connection.row_factory = sqlite3.Row
        self.connection.execute("PRAGMA journal_mode = WAL")
        self.connection.execute("PRAGMA wal_autocheckpoint = 0")
        self.connection.execute("PRAGMA synchronous = NORMAL")
        self.connection.execute(
            """
            CREATE TABLE documents (
                id INTEGER PRIMARY KEY,
                current_text TEXT NOT NULL,
                current_timestamp INTEGER NOT NULL,
                revision_count INTEGER NOT NULL DEFAULT 0
            )
            """
        )

    def close(self) -> None:
        self.connection.close()

    def create_document(self, text: str, timestamp: int) -> int:
        cursor = self.connection.execute(
            "INSERT INTO documents (current_text, current_timestamp) VALUES (?, ?)",
            (text, timestamp),
        )
        return int(cursor.lastrowid)

    def replace(self, document_id: int, text: str, timestamp: int) -> int:
        cursor = self.connection.execute(
            """
            UPDATE documents
            SET current_text = ?, current_timestamp = ?, revision_count = revision_count + 1
            WHERE id = ?
            RETURNING revision_count
            """,
            (text, timestamp, document_id),
        )
        row = cursor.fetchone()
        if row is None:
            raise KeyError(document_id)
        return int(row[0])

    def versions(self, document_id: int) -> list[Version]:
        row = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        return [
            Version(
                revision=int(row["revision_count"]),
                timestamp=int(row["current_timestamp"]),
                text=str(row["current_text"]),
                is_current=True,
            )
        ]

    def get_version(self, document_id: int, revision: int) -> Version:
        version = self.versions(document_id)[0]
        if revision not in (version.revision, -1):
            raise IndexError(revision)
        return version

    def storage_stats(self, document_id: int) -> dict[str, int]:
        row = self.connection.execute(
            """
            SELECT revision_count, length(CAST(current_text AS BLOB)) AS current_text_bytes
            FROM documents WHERE id = ?
            """,
            (document_id,),
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        return {
            "revision_count": int(row["revision_count"]),
            "current_text_bytes": int(row["current_text_bytes"]),
            "history_blob_bytes": 0,
            "timestamp_json_bytes": 0,
        }


class PlainRowsStore:
    def __init__(self, path: Path) -> None:
        self.connection = sqlite3.connect(path, isolation_level=None)
        self.connection.row_factory = sqlite3.Row
        self.connection.execute("PRAGMA journal_mode = WAL")
        self.connection.execute("PRAGMA wal_autocheckpoint = 0")
        self.connection.execute("PRAGMA synchronous = NORMAL")
        self.connection.executescript(
            """
            CREATE TABLE documents (
                id INTEGER PRIMARY KEY,
                current_text TEXT NOT NULL,
                current_timestamp INTEGER NOT NULL,
                revision_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE revisions (
                document_id INTEGER NOT NULL,
                revision INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                text TEXT NOT NULL,
                PRIMARY KEY (document_id, revision)
            );
            """
        )

    def close(self) -> None:
        self.connection.close()

    def create_document(self, text: str, timestamp: int) -> int:
        cursor = self.connection.execute(
            "INSERT INTO documents (current_text, current_timestamp) VALUES (?, ?)",
            (text, timestamp),
        )
        return int(cursor.lastrowid)

    def replace(self, document_id: int, text: str, timestamp: int) -> int:
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            row = self.connection.execute(
                "SELECT * FROM documents WHERE id = ?", (document_id,)
            ).fetchone()
            if row is None:
                raise KeyError(document_id)
            revision = int(row["revision_count"])
            self.connection.execute(
                "INSERT INTO revisions VALUES (?, ?, ?, ?)",
                (document_id, revision, row["current_timestamp"], row["current_text"]),
            )
            revision += 1
            self.connection.execute(
                """
                UPDATE documents SET current_text = ?, current_timestamp = ?,
                                     revision_count = ?
                WHERE id = ?
                """,
                (text, timestamp, revision, document_id),
            )
            self.connection.execute("COMMIT")
            return revision
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise

    def versions(self, document_id: int) -> list[Version]:
        output = [
            Version(
                revision=int(row["revision"]),
                timestamp=int(row["timestamp"]),
                text=str(row["text"]),
            )
            for row in self.connection.execute(
                "SELECT * FROM revisions WHERE document_id = ? ORDER BY revision",
                (document_id,),
            )
        ]
        row = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        output.append(
            Version(
                revision=int(row["revision_count"]),
                timestamp=int(row["current_timestamp"]),
                text=str(row["current_text"]),
                is_current=True,
            )
        )
        return output

    def get_version(self, document_id: int, revision: int) -> Version:
        document = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if document is None:
            raise KeyError(document_id)
        revision_count = int(document["revision_count"])
        if revision < 0:
            revision += revision_count + 1
        if revision == revision_count:
            return Version(
                revision=revision,
                timestamp=int(document["current_timestamp"]),
                text=str(document["current_text"]),
                is_current=True,
            )
        row = self.connection.execute(
            "SELECT * FROM revisions WHERE document_id = ? AND revision = ?",
            (document_id, revision),
        ).fetchone()
        if row is None:
            raise IndexError(revision)
        return Version(
            revision=revision,
            timestamp=int(row["timestamp"]),
            text=str(row["text"]),
        )

    def storage_stats(self, document_id: int) -> dict[str, int]:
        document = self.connection.execute(
            """
            SELECT revision_count, length(CAST(current_text AS BLOB)) AS current_text_bytes
            FROM documents WHERE id = ?
            """,
            (document_id,),
        ).fetchone()
        rows = self.connection.execute(
            """
            SELECT count(*) AS row_count, coalesce(sum(length(CAST(text AS BLOB))), 0) AS history_text_bytes
            FROM revisions WHERE document_id = ?
            """,
            (document_id,),
        ).fetchone()
        if document is None:
            raise KeyError(document_id)
        return {
            "revision_count": int(document["revision_count"]),
            "current_text_bytes": int(document["current_text_bytes"]),
            "history_row_count": int(rows["row_count"]),
            "history_blob_bytes": int(rows["history_text_bytes"]),
            "timestamp_json_bytes": int(rows["row_count"]) * 8,
        }


class CompressedRowsStore(PlainRowsStore):
    def __init__(self, path: Path, codec_name: str = "zstd") -> None:
        self.codec = make_codec(codec_name)  # type: ignore[arg-type]
        self.connection = sqlite3.connect(path, isolation_level=None)
        self.connection.row_factory = sqlite3.Row
        self.connection.execute("PRAGMA journal_mode = WAL")
        self.connection.execute("PRAGMA wal_autocheckpoint = 0")
        self.connection.execute("PRAGMA synchronous = NORMAL")
        self.connection.executescript(
            """
            CREATE TABLE documents (
                id INTEGER PRIMARY KEY,
                current_text TEXT NOT NULL,
                current_timestamp INTEGER NOT NULL,
                revision_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE revisions (
                document_id INTEGER NOT NULL,
                revision INTEGER NOT NULL,
                timestamp INTEGER NOT NULL,
                text_blob BLOB NOT NULL,
                PRIMARY KEY (document_id, revision)
            );
            """
        )

    def replace(self, document_id: int, text: str, timestamp: int) -> int:
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            row = self.connection.execute(
                "SELECT * FROM documents WHERE id = ?", (document_id,)
            ).fetchone()
            if row is None:
                raise KeyError(document_id)
            revision = int(row["revision_count"])
            compressed = self.codec.compress(str(row["current_text"]).encode("utf-8"))
            self.connection.execute(
                "INSERT INTO revisions VALUES (?, ?, ?, ?)",
                (document_id, revision, row["current_timestamp"], compressed),
            )
            revision += 1
            self.connection.execute(
                """
                UPDATE documents SET current_text = ?, current_timestamp = ?,
                                     revision_count = ?
                WHERE id = ?
                """,
                (text, timestamp, revision, document_id),
            )
            self.connection.execute("COMMIT")
            return revision
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise

    def versions(self, document_id: int) -> list[Version]:
        output = [
            Version(
                revision=int(row["revision"]),
                timestamp=int(row["timestamp"]),
                text=self.codec.decompress(bytes(row["text_blob"])).decode("utf-8"),
            )
            for row in self.connection.execute(
                "SELECT * FROM revisions WHERE document_id = ? ORDER BY revision",
                (document_id,),
            )
        ]
        row = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        output.append(
            Version(
                revision=int(row["revision_count"]),
                timestamp=int(row["current_timestamp"]),
                text=str(row["current_text"]),
                is_current=True,
            )
        )
        return output

    def get_version(self, document_id: int, revision: int) -> Version:
        document = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if document is None:
            raise KeyError(document_id)
        revision_count = int(document["revision_count"])
        if revision < 0:
            revision += revision_count + 1
        if revision == revision_count:
            return Version(
                revision=revision,
                timestamp=int(document["current_timestamp"]),
                text=str(document["current_text"]),
                is_current=True,
            )
        row = self.connection.execute(
            "SELECT * FROM revisions WHERE document_id = ? AND revision = ?",
            (document_id, revision),
        ).fetchone()
        if row is None:
            raise IndexError(revision)
        return Version(
            revision=revision,
            timestamp=int(row["timestamp"]),
            text=self.codec.decompress(bytes(row["text_blob"])).decode("utf-8"),
        )

    def storage_stats(self, document_id: int) -> dict[str, int]:
        document = self.connection.execute(
            """
            SELECT revision_count, length(CAST(current_text AS BLOB)) AS current_text_bytes
            FROM documents WHERE id = ?
            """,
            (document_id,),
        ).fetchone()
        rows = self.connection.execute(
            """
            SELECT count(*) AS row_count,
                   coalesce(sum(length(text_blob)), 0) AS history_blob_bytes
            FROM revisions WHERE document_id = ?
            """,
            (document_id,),
        ).fetchone()
        if document is None:
            raise KeyError(document_id)
        return {
            "revision_count": int(document["revision_count"]),
            "current_text_bytes": int(document["current_text_bytes"]),
            "history_row_count": int(rows["row_count"]),
            "history_blob_bytes": int(rows["history_blob_bytes"]),
            "timestamp_json_bytes": int(rows["row_count"]) * 8,
        }


class ConfiguredWholeBlobStore(WholeBlobHistoryStore):
    def __init__(self, path: Path, codec_name: str, format_name: str) -> None:
        super().__init__(path)
        self.codec_name = codec_name
        self.format_name = format_name
        self.connection.execute("PRAGMA wal_autocheckpoint = 0")
        self.connection.execute("PRAGMA synchronous = NORMAL")

    def create_document(self, text: str, timestamp: int) -> int:
        return super().create_document(
            text,
            timestamp,
            codec=self.codec_name,  # type: ignore[arg-type]
            format_name=self.format_name,  # type: ignore[arg-type]
        )


class ConfiguredChunkedStore(ChunkedHistoryStore):
    def __init__(
        self,
        path: Path,
        codec_name: str,
        format_name: str,
        chunk_size: int,
    ) -> None:
        super().__init__(path, chunk_size=chunk_size)
        self.codec_name = codec_name
        self.format_name = format_name
        self.connection.execute("PRAGMA wal_autocheckpoint = 0")
        self.connection.execute("PRAGMA synchronous = NORMAL")

    def create_document(self, text: str, timestamp: int) -> int:
        return super().create_document(
            text,
            timestamp,
            codec=self.codec_name,  # type: ignore[arg-type]
            format_name=self.format_name,  # type: ignore[arg-type]
        )


@dataclass
class StrategyResult:
    strategy: str
    revisions: int
    history_available: bool
    write_total_ms: float
    write_mean_ms: float
    write_p50_ms: float
    write_p95_ms: float
    first_25_p50_ms: float
    last_25_p50_ms: float
    latency_growth_ratio: float
    read_all_median_ms: float
    random_read_median_ms: float
    db_bytes_before_checkpoint: int
    wal_bytes_before_checkpoint: int
    compact_db_bytes: int
    sqlite: dict[str, int]
    storage: dict[str, int]


def verify_versions(actual: list[Version], workload: Workload) -> None:
    if len(actual) != len(workload.versions):
        raise AssertionError(f"Expected {len(workload.versions)} versions, got {len(actual)}")
    for index, version in enumerate(actual):
        if version.revision != index:
            raise AssertionError((version.revision, index))
        if version.text != workload.versions[index]:
            raise AssertionError(f"Text differs at revision {index}")
        if version.timestamp != workload.timestamps[index]:
            raise AssertionError(f"Timestamp differs at revision {index}")
        if version.is_current != (index == len(actual) - 1):
            raise AssertionError(f"Current flag differs at revision {index}")


def benchmark_strategy(
    *,
    name: str,
    factory: Any,
    workload: Workload,
    output_dir: Path,
    random_reads: int,
) -> StrategyResult:
    path = output_dir / f"{name}.db"
    for suffix in ("", "-wal", "-shm", ".compact"):
        Path(str(path) + suffix).unlink(missing_ok=True)

    store: BenchmarkStore = factory(path)
    document_id = store.create_document(workload.versions[0], workload.timestamps[0])
    latencies: list[float] = []
    for revision in range(1, len(workload.versions)):
        started = time.perf_counter_ns()
        store.replace(
            document_id,
            workload.versions[revision],
            workload.timestamps[revision],
        )
        latencies.append((time.perf_counter_ns() - started) / 1_000_000)

    history_available = bool(getattr(store, "history_available", True))
    actual = store.versions(document_id)
    if history_available:
        verify_versions(actual, workload)
    else:
        if len(actual) != 1 or actual[0].text != workload.versions[-1]:
            raise AssertionError("Current-only reference did not preserve current text")

    read_all_samples: list[float] = []
    for _ in range(7):
        started = time.perf_counter_ns()
        store.versions(document_id)
        read_all_samples.append((time.perf_counter_ns() - started) / 1_000_000)

    rng = random.Random(9182)
    if history_available:
        revisions_to_read = [rng.randrange(0, len(workload.versions)) for _ in range(random_reads)]
    else:
        revisions_to_read = [workload.revision_count for _ in range(random_reads)]
    random_read_samples: list[float] = []
    for revision in revisions_to_read:
        started = time.perf_counter_ns()
        version = store.get_version(document_id, revision)
        elapsed = (time.perf_counter_ns() - started) / 1_000_000
        if version.text != workload.versions[revision]:
            raise AssertionError(f"Random read mismatch at {revision}")
        random_read_samples.append(elapsed)

    db_before = file_size(path)
    wal_before = file_size(Path(str(path) + "-wal"))
    compact_bytes = compact_database_size(store.connection, path)
    page_stats = sqlite_stats(store.connection)
    storage = store.storage_stats(document_id)
    store.close()

    first = latencies[: min(25, len(latencies))]
    last = latencies[-min(25, len(latencies)) :]
    first_p50 = statistics.median(first)
    last_p50 = statistics.median(last)
    return StrategyResult(
        strategy=name,
        revisions=workload.revision_count,
        history_available=history_available,
        write_total_ms=sum(latencies),
        write_mean_ms=statistics.mean(latencies),
        write_p50_ms=statistics.median(latencies),
        write_p95_ms=percentile(latencies, 0.95),
        first_25_p50_ms=first_p50,
        last_25_p50_ms=last_p50,
        latency_growth_ratio=last_p50 / first_p50 if first_p50 else math.inf,
        read_all_median_ms=statistics.median(read_all_samples),
        random_read_median_ms=statistics.median(random_read_samples),
        db_bytes_before_checkpoint=db_before,
        wal_bytes_before_checkpoint=wal_before,
        compact_db_bytes=compact_bytes,
        sqlite=page_stats,
        storage=storage,
    )


def aggregate_strategy_trials(name: str, trials: list[StrategyResult]) -> StrategyResult:
    if not trials:
        raise ValueError("At least one trial is required")

    def median(field: str) -> float:
        return statistics.median(float(getattr(trial, field)) for trial in trials)

    representative = trials[len(trials) // 2]
    return StrategyResult(
        strategy=name,
        revisions=representative.revisions,
        history_available=representative.history_available,
        write_total_ms=median("write_total_ms"),
        write_mean_ms=median("write_mean_ms"),
        write_p50_ms=median("write_p50_ms"),
        write_p95_ms=median("write_p95_ms"),
        first_25_p50_ms=median("first_25_p50_ms"),
        last_25_p50_ms=median("last_25_p50_ms"),
        latency_growth_ratio=median("latency_growth_ratio"),
        read_all_median_ms=median("read_all_median_ms"),
        random_read_median_ms=median("random_read_median_ms"),
        db_bytes_before_checkpoint=int(median("db_bytes_before_checkpoint")),
        wal_bytes_before_checkpoint=int(median("wal_bytes_before_checkpoint")),
        compact_db_bytes=int(median("compact_db_bytes")),
        sqlite=representative.sqlite,
        storage=representative.storage,
    )


def encode_json_array(strings_: list[str]) -> bytes:
    return json.dumps(strings_, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def encode_framed(strings_: list[str]) -> bytes:
    # Build once with bytearray. Repeated bytes concatenation would turn this
    # final-size measurement into an accidental O(n²) benchmark.
    payload = bytearray()
    for string_ in strings_:
        encoded = string_.encode("utf-8")
        length = len(encoded)
        while length >= 0x80:
            payload.append((length & 0x7F) | 0x80)
            length >>= 7
        payload.append(length)
        payload.extend(encoded)
    return bytes(payload)


def compression_scaling(workload: Workload, checkpoints: list[int]) -> list[dict[str, Any]]:
    output: list[dict[str, Any]] = []
    zstd = make_codec("zstd")
    zlib_codec = make_codec("zlib")
    for count in checkpoints:
        history = workload.versions[:count]
        raw_snapshot_bytes = sum(len(text.encode("utf-8")) for text in history)
        json_payload = encode_json_array(history)
        framed_payload = encode_framed(history)
        timestamp_json = json.dumps(
            workload.timestamps[:count], separators=(",", ":")
        ).encode("utf-8")
        output.append(
            {
                "versions": count,
                "raw_snapshot_bytes": raw_snapshot_bytes,
                "json_payload_bytes": len(json_payload),
                "framed_payload_bytes": len(framed_payload),
                "timestamp_json_bytes": len(timestamp_json),
                "json_zstd_bytes": len(zstd.compress(json_payload)),
                "framed_zstd_bytes": len(zstd.compress(framed_payload)),
                "json_zlib_bytes": len(zlib_codec.compress(json_payload)),
                "framed_zlib_bytes": len(zlib_codec.compress(framed_payload)),
            }
        )
    return output


def chunk_size_tradeoff(workload: Workload, chunk_sizes: list[int]) -> list[dict[str, Any]]:
    codec = make_codec("zstd")
    history = workload.versions[:-1]
    output: list[dict[str, Any]] = []
    for chunk_size in chunk_sizes:
        chunks = [history[index : index + chunk_size] for index in range(0, len(history), chunk_size)]
        blobs = [codec.compress(encode_framed(chunk)) for chunk in chunks]
        output.append(
            {
                "chunk_size": chunk_size,
                "chunk_count": len(chunks),
                "compressed_history_bytes": sum(map(len, blobs)),
                "largest_chunk_bytes": max(map(len, blobs), default=0),
                "raw_history_bytes": sum(len(text.encode("utf-8")) for text in history),
            }
        )
    return output


def format_bytes(value: int | float) -> str:
    value = float(value)
    units = ["B", "KB", "MB", "GB"]
    index = 0
    while value >= 1024 and index < len(units) - 1:
        value /= 1024
        index += 1
    if index == 0:
        return f"{value:.0f} {units[index]}"
    return f"{value:.1f} {units[index]}"


def markdown_table(headers: list[str], rows: list[list[str]]) -> str:
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    lines.extend("| " + " | ".join(row) + " |" for row in rows)
    return "\n".join(lines)


def write_results_markdown(results: dict[str, Any], path: Path) -> None:
    strategy_rows = []
    for result in results["strategies"]:
        strategy_rows.append(
            [
                result["strategy"],
                f"{result['write_total_ms']:.1f}",
                f"{result['write_p50_ms']:.3f}",
                f"{result['last_25_p50_ms']:.3f}",
                f"{result['latency_growth_ratio']:.1f}×",
                format_bytes(result["storage"]["history_blob_bytes"]),
                format_bytes(result["wal_bytes_before_checkpoint"]),
                format_bytes(result["compact_db_bytes"]),
                (f"{result['random_read_median_ms']:.3f}" if result["history_available"] else "current only"),
            ]
        )

    scaling_rows = []
    for row in results["compression_scaling"]:
        scaling_rows.append(
            [
                str(row["versions"]),
                format_bytes(row["raw_snapshot_bytes"]),
                format_bytes(row["json_zstd_bytes"]),
                format_bytes(row["json_zlib_bytes"]),
                format_bytes(row["timestamp_json_bytes"]),
            ]
        )

    chunk_rows = []
    for row in results["chunk_size_tradeoff"]:
        chunk_rows.append(
            [
                str(row["chunk_size"]),
                str(row["chunk_count"]),
                format_bytes(row["compressed_history_bytes"]),
                format_bytes(row["largest_chunk_bytes"]),
            ]
        )

    content = f"""# Generated benchmark results

Workload: {results['config']['document_bytes']:,}-byte starting document,
{results['config']['database_benchmark_revisions']:,} individually committed edits for the
SQLite benchmark ({results['config']['trials_per_strategy']} trials; medians reported), and
{results['config']['scaling_revisions']:,} edits for final-size scaling.
The edits are mostly small replacements/inserts/deletes, plus one paragraph-scale rewrite
every 50 revisions.

## SQLite update benchmark

All times are milliseconds. “History payload” excludes the ordinary current-text column.
“WAL bytes” is the WAL left by individually committing every edit with automatic
checkpointing disabled, making it a useful approximation of SQLite write amplification.

{markdown_table(
    ['Strategy', 'Total write ms', 'Median edit', 'Last-25 median', 'Growth',
     'History payload', 'WAL bytes', 'Compact DB', 'Random read ms'],
    strategy_rows,
)}

## Final compressed size as history grows

{markdown_table(
    ['Historical versions', 'Raw snapshots', 'JSON + zstd', 'JSON + zlib', 'Timestamp JSON'],
    scaling_rows,
)}

## Zstandard chunk-size trade-off

These numbers use the longer scaling workload and independently compress each group of
versions. Smaller chunks bound update and random-read work, at the cost of repeating the
base document once per chunk.

{markdown_table(
    ['Versions/chunk', 'Chunks', 'Total compressed', 'Largest chunk'],
    chunk_rows,
)}

Full machine-readable measurements are in `results.json`.
"""
    path.write_text(content, encoding="utf-8")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--document-bytes", type=int, default=20_000)
    parser.add_argument("--revisions", type=int, default=300)
    parser.add_argument("--scaling-revisions", type=int, default=1_000)
    parser.add_argument("--random-reads", type=int, default=31)
    parser.add_argument("--trials", type=int, default=3)
    parser.add_argument(
        "--strategies",
        help="Comma-separated strategy names; default is all strategies",
    )
    parser.add_argument("--output", type=Path, default=Path("benchmark-output"))
    args = parser.parse_args()

    args.output.mkdir(parents=True, exist_ok=True)
    database_workload = make_workload(
        document_bytes=args.document_bytes, revisions=args.revisions
    )
    scaling_workload = make_workload(
        document_bytes=args.document_bytes, revisions=args.scaling_revisions
    )

    factories = [
        ("current_only_reference", lambda path: CurrentOnlyStore(path)),
        ("plain_rows", lambda path: PlainRowsStore(path)),
        ("zstd_per_row", lambda path: CompressedRowsStore(path, "zstd")),
        (
            "whole_json_zstd",
            lambda path: ConfiguredWholeBlobStore(path, "zstd", "json"),
        ),
        (
            "whole_framed_zstd",
            lambda path: ConfiguredWholeBlobStore(path, "zstd", "framed"),
        ),
        (
            "whole_json_zlib",
            lambda path: ConfiguredWholeBlobStore(path, "zlib", "json"),
        ),
        (
            "chunked_framed_zstd_32",
            lambda path: ConfiguredChunkedStore(path, "zstd", "framed", 32),
        ),
        (
            "chunked_framed_zstd_64",
            lambda path: ConfiguredChunkedStore(path, "zstd", "framed", 64),
        ),
        (
            "chunked_framed_zstd_128",
            lambda path: ConfiguredChunkedStore(path, "zstd", "framed", 128),
        ),
        (
            "chunked_framed_zstd_256",
            lambda path: ConfiguredChunkedStore(path, "zstd", "framed", 256),
        ),
        (
            "chunked_json_zstd_64",
            lambda path: ConfiguredChunkedStore(path, "zstd", "json", 64),
        ),
        (
            "chunked_json_zstd_128",
            lambda path: ConfiguredChunkedStore(path, "zstd", "json", 128),
        ),
    ]

    if args.strategies:
        requested = {item.strip() for item in args.strategies.split(",") if item.strip()}
        known = {name for name, _ in factories}
        unknown = requested - known
        if unknown:
            parser.error("Unknown strategies: " + ", ".join(sorted(unknown)))
        factories = [(name, factory) for name, factory in factories if name in requested]

    if args.trials < 1:
        parser.error("--trials must be at least 1")

    trial_dir = args.output / "trial-databases"
    trial_dir.mkdir(exist_ok=True)
    strategy_results = []
    for name, factory in factories:
        print(f"Benchmarking {name} ({args.trials} trial(s))...", flush=True)
        trials = []
        for trial_number in range(1, args.trials + 1):
            trial = benchmark_strategy(
                name=f"{name}__trial_{trial_number}",
                factory=factory,
                workload=database_workload,
                output_dir=trial_dir,
                random_reads=args.random_reads,
            )
            trials.append(trial)
        result = aggregate_strategy_trials(name, trials)
        strategy_results.append(asdict(result))
        print(
            f"  median {result.write_total_ms:.1f} ms writes; "
            f"{result.storage['history_blob_bytes']} history bytes; "
            f"{result.wal_bytes_before_checkpoint} WAL bytes",
            flush=True,
        )
    shutil.rmtree(trial_dir)

    checkpoints = sorted(
        set(
            value
            for value in [1, 10, 50, 100, 300, 1_000, args.scaling_revisions]
            if value <= args.scaling_revisions
        )
    )
    results = {
        "config": {
            "document_bytes": args.document_bytes,
            "database_benchmark_revisions": args.revisions,
            "scaling_revisions": args.scaling_revisions,
            "random_reads_per_strategy": args.random_reads,
            "trials_per_strategy": args.trials,
            "python": os.sys.version,
            "sqlite": sqlite3.sqlite_version,
        },
        "strategies": strategy_results,
        "compression_scaling": compression_scaling(scaling_workload, checkpoints),
        "chunk_size_tradeoff": chunk_size_tradeoff(
            scaling_workload, [1, 4, 8, 16, 32, 64, 128, 256, 1_000]
        ),
    }
    json_path = args.output / "results.json"
    json_path.write_text(json.dumps(results, indent=2), encoding="utf-8")
    write_results_markdown(results, args.output / "RESULTS.md")
    print(f"Wrote {json_path}")


if __name__ == "__main__":
    main()
