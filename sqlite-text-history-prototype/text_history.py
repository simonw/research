"""Experimental SQLite text revision stores.

The main prototype stores the current text as ordinary TEXT and all previous
versions in one compressed BLOB.  A separate JSON array stores the timestamps
for those historical versions, matching the scheme discussed in the prompt.

Two payload encodings are supported:

* ``json``: a canonical UTF-8 JSON array of strings.  Updates append to the
  serialized JSON bytes directly, so they do not parse every previous string.
* ``framed``: UTF-8 strings prefixed by unsigned varint byte lengths.

Two compression codecs are supported:

* ``zlib`` from the Python standard library.
* ``zstd`` through Python 3.14's ``compression.zstd``, the third-party
  ``zstandard`` package, or (for this prototype) a tiny ctypes wrapper around
  an installed libzstd.

The chunked store is an alternative that only rewrites the most recent chunk,
while retaining cross-version compression within each chunk.
"""

from __future__ import annotations

import ctypes
import ctypes.util
import json
import sqlite3
import struct
import zlib
from dataclasses import dataclass
from pathlib import Path
from typing import Iterator, Literal, Protocol

CodecName = Literal["zlib", "zstd"]
FormatName = Literal["json", "framed"]


class CompressionCodec(Protocol):
    name: str

    def compress(self, data: bytes) -> bytes: ...

    def decompress(self, data: bytes) -> bytes: ...


class ZlibCodec:
    name = "zlib"

    def __init__(self, level: int = 6) -> None:
        self.level = level

    def compress(self, data: bytes) -> bytes:
        return zlib.compress(data, self.level)

    def decompress(self, data: bytes) -> bytes:
        return zlib.decompress(data)


class _StdlibZstdCodec:
    name = "zstd"

    def __init__(self, level: int = 3) -> None:
        from compression import zstd  # type: ignore[attr-defined]

        self._zstd = zstd
        self.level = level

    def compress(self, data: bytes) -> bytes:
        return self._zstd.compress(data, level=self.level)

    def decompress(self, data: bytes) -> bytes:
        return self._zstd.decompress(data)


class _PackageZstdCodec:
    name = "zstd"

    def __init__(self, level: int = 3) -> None:
        import zstandard  # type: ignore[import-not-found]

        self._compressor = zstandard.ZstdCompressor(level=level)
        self._decompressor = zstandard.ZstdDecompressor()

    def compress(self, data: bytes) -> bytes:
        return self._compressor.compress(data)

    def decompress(self, data: bytes) -> bytes:
        return self._decompressor.decompress(data)


class _CtypesZstdCodec:
    """Small libzstd wrapper used only to keep the experiment dependency-free."""

    name = "zstd"
    _CONTENTSIZE_UNKNOWN = (1 << 64) - 1
    _CONTENTSIZE_ERROR = (1 << 64) - 2

    def __init__(self, level: int = 3) -> None:
        library = ctypes.util.find_library("zstd")
        if not library:
            raise RuntimeError("libzstd was not found")
        self._lib = ctypes.CDLL(library)
        self.level = level

        self._lib.ZSTD_compressBound.argtypes = [ctypes.c_size_t]
        self._lib.ZSTD_compressBound.restype = ctypes.c_size_t
        self._lib.ZSTD_compress.argtypes = [
            ctypes.c_void_p,
            ctypes.c_size_t,
            ctypes.c_void_p,
            ctypes.c_size_t,
            ctypes.c_int,
        ]
        self._lib.ZSTD_compress.restype = ctypes.c_size_t
        self._lib.ZSTD_decompress.argtypes = [
            ctypes.c_void_p,
            ctypes.c_size_t,
            ctypes.c_void_p,
            ctypes.c_size_t,
        ]
        self._lib.ZSTD_decompress.restype = ctypes.c_size_t
        self._lib.ZSTD_getFrameContentSize.argtypes = [
            ctypes.c_void_p,
            ctypes.c_size_t,
        ]
        self._lib.ZSTD_getFrameContentSize.restype = ctypes.c_uint64
        self._lib.ZSTD_isError.argtypes = [ctypes.c_size_t]
        self._lib.ZSTD_isError.restype = ctypes.c_uint
        self._lib.ZSTD_getErrorName.argtypes = [ctypes.c_size_t]
        self._lib.ZSTD_getErrorName.restype = ctypes.c_char_p

    def _check(self, result: int) -> int:
        if self._lib.ZSTD_isError(result):
            message = self._lib.ZSTD_getErrorName(result).decode("utf-8", "replace")
            raise RuntimeError(f"libzstd error: {message}")
        return result

    @staticmethod
    def _source_buffer(data: bytes) -> ctypes.Array[ctypes.c_char]:
        # create_string_buffer adds a trailing NUL, which is harmless because the
        # exact byte count is passed separately.
        return ctypes.create_string_buffer(data)

    def compress(self, data: bytes) -> bytes:
        source = self._source_buffer(data)
        capacity = self._lib.ZSTD_compressBound(len(data))
        destination = ctypes.create_string_buffer(capacity)
        written = self._check(
            self._lib.ZSTD_compress(
                destination,
                capacity,
                source,
                len(data),
                self.level,
            )
        )
        return destination.raw[:written]

    def decompress(self, data: bytes) -> bytes:
        source = self._source_buffer(data)
        size = int(self._lib.ZSTD_getFrameContentSize(source, len(data)))
        if size in (self._CONTENTSIZE_UNKNOWN, self._CONTENTSIZE_ERROR):
            raise RuntimeError("Zstandard frame does not contain a known size")
        destination = ctypes.create_string_buffer(size)
        written = self._check(
            self._lib.ZSTD_decompress(destination, size, source, len(data))
        )
        if written != size:
            raise RuntimeError(f"Expected {size} decompressed bytes, got {written}")
        return destination.raw[:written]


def make_codec(name: CodecName, *, level: int | None = None) -> CompressionCodec:
    if name == "zlib":
        return ZlibCodec(level=6 if level is None else level)
    if name != "zstd":
        raise ValueError(f"Unknown codec: {name}")

    zstd_level = 3 if level is None else level
    errors: list[str] = []
    for implementation in (_StdlibZstdCodec, _PackageZstdCodec, _CtypesZstdCodec):
        try:
            return implementation(level=zstd_level)
        except (ImportError, ModuleNotFoundError, RuntimeError) as ex:
            errors.append(f"{implementation.__name__}: {ex}")
    raise RuntimeError(
        "Zstandard is unavailable. Use Python 3.14+, install the 'zstandard' "
        "package, or install libzstd. Attempts: " + "; ".join(errors)
    )


def _encode_uvarint(value: int) -> bytes:
    if value < 0:
        raise ValueError("uvarint cannot encode a negative value")
    output = bytearray()
    while value >= 0x80:
        output.append((value & 0x7F) | 0x80)
        value >>= 7
    output.append(value)
    return bytes(output)


def _decode_uvarint(data: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    while True:
        if offset >= len(data):
            raise ValueError("Truncated uvarint")
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, offset
        shift += 7
        if shift > 63:
            raise ValueError("uvarint is too large")


def empty_payload(format_name: FormatName) -> bytes:
    if format_name == "json":
        return b"[]"
    if format_name == "framed":
        return b""
    raise ValueError(f"Unknown format: {format_name}")


def append_payload(payload: bytes, text: str, format_name: FormatName) -> bytes:
    encoded_text = text.encode("utf-8")
    if format_name == "framed":
        return payload + _encode_uvarint(len(encoded_text)) + encoded_text
    if format_name == "json":
        if not payload.endswith(b"]"):
            raise ValueError("Malformed JSON history payload")
        item = json.dumps(text, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
        if payload == b"[]":
            return b"[" + item + b"]"
        return payload[:-1] + b"," + item + b"]"
    raise ValueError(f"Unknown format: {format_name}")


def decode_payload(payload: bytes, format_name: FormatName) -> list[str]:
    if format_name == "json":
        value = json.loads(payload)
        if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
            raise ValueError("History JSON must be an array of strings")
        return value
    if format_name == "framed":
        output: list[str] = []
        offset = 0
        while offset < len(payload):
            length, offset = _decode_uvarint(payload, offset)
            end = offset + length
            if end > len(payload):
                raise ValueError("Truncated framed text")
            output.append(payload[offset:end].decode("utf-8"))
            offset = end
        return output
    raise ValueError(f"Unknown format: {format_name}")


def append_timestamp_json(serialized: str, timestamp: int) -> str:
    if serialized == "[]":
        return f"[{timestamp}]"
    if not serialized.endswith("]"):
        raise ValueError("Malformed timestamp JSON")
    return f"{serialized[:-1]},{timestamp}]"


def decode_timestamp_json(serialized: str) -> list[int]:
    value = json.loads(serialized)
    if not isinstance(value, list) or not all(type(item) is int for item in value):
        raise ValueError("Timestamp JSON must be an array of integers")
    return value


@dataclass(frozen=True)
class Version:
    revision: int
    timestamp: int
    text: str
    is_current: bool = False


@dataclass(frozen=True)
class Document:
    id: int
    current_text: str
    current_timestamp: int
    revision_count: int
    codec: CodecName
    format: FormatName


def _open_connection(path: str | Path, *, wal: bool) -> sqlite3.Connection:
    connection = sqlite3.connect(path, isolation_level=None)
    connection.row_factory = sqlite3.Row
    connection.execute("PRAGMA foreign_keys = ON")
    connection.execute("PRAGMA busy_timeout = 5000")
    if wal:
        connection.execute("PRAGMA journal_mode = WAL")
    return connection


class WholeBlobHistoryStore:
    """One compressed history BLOB and one JSON timestamp array per document."""

    def __init__(self, path: str | Path, *, wal: bool = True) -> None:
        self.path = Path(path)
        self.connection = _open_connection(path, wal=wal)
        self.connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY,
                current_text TEXT NOT NULL,
                current_timestamp INTEGER NOT NULL,
                history_blob BLOB NOT NULL,
                history_timestamps TEXT NOT NULL DEFAULT '[]',
                history_codec TEXT NOT NULL CHECK (history_codec IN ('zlib', 'zstd')),
                history_format TEXT NOT NULL CHECK (history_format IN ('json', 'framed')),
                revision_count INTEGER NOT NULL DEFAULT 0 CHECK (revision_count >= 0)
            );
            """
        )
        self._codec_cache: dict[str, CompressionCodec] = {}

    def close(self) -> None:
        self.connection.close()

    def __enter__(self) -> "WholeBlobHistoryStore":
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        self.close()

    def _codec(self, name: CodecName) -> CompressionCodec:
        codec = self._codec_cache.get(name)
        if codec is None:
            codec = make_codec(name)
            self._codec_cache[name] = codec
        return codec

    def create_document(
        self,
        text: str,
        timestamp: int,
        *,
        codec: CodecName = "zstd",
        format_name: FormatName = "json",
    ) -> int:
        compressor = self._codec(codec)
        initial_blob = compressor.compress(empty_payload(format_name))
        cursor = self.connection.execute(
            """
            INSERT INTO documents (
                current_text, current_timestamp, history_blob,
                history_timestamps, history_codec, history_format
            ) VALUES (?, ?, ?, '[]', ?, ?)
            """,
            (text, timestamp, initial_blob, codec, format_name),
        )
        return int(cursor.lastrowid)

    def get_document(self, document_id: int) -> Document:
        row = self.connection.execute(
            """
            SELECT id, current_text, current_timestamp, revision_count,
                   history_codec, history_format
            FROM documents WHERE id = ?
            """,
            (document_id,),
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        return Document(
            id=int(row["id"]),
            current_text=str(row["current_text"]),
            current_timestamp=int(row["current_timestamp"]),
            revision_count=int(row["revision_count"]),
            codec=row["history_codec"],
            format=row["history_format"],
        )

    def replace(
        self,
        document_id: int,
        new_text: str,
        timestamp: int,
        *,
        skip_unchanged: bool = True,
    ) -> int:
        """Replace current text and return the new current revision number.

        The old current version and its *start* timestamp are appended to the
        historical arrays.  The supplied timestamp becomes the start timestamp
        for the new current version.
        """

        self.connection.execute("BEGIN IMMEDIATE")
        try:
            row = self.connection.execute(
                "SELECT * FROM documents WHERE id = ?", (document_id,)
            ).fetchone()
            if row is None:
                raise KeyError(document_id)
            current_text = str(row["current_text"])
            if skip_unchanged and current_text == new_text:
                self.connection.execute("COMMIT")
                return int(row["revision_count"])

            codec_name: CodecName = row["history_codec"]
            format_name: FormatName = row["history_format"]
            codec = self._codec(codec_name)
            payload = codec.decompress(bytes(row["history_blob"]))
            payload = append_payload(payload, current_text, format_name)
            history_blob = codec.compress(payload)
            timestamps = append_timestamp_json(
                str(row["history_timestamps"]), int(row["current_timestamp"])
            )
            new_revision = int(row["revision_count"]) + 1

            self.connection.execute(
                """
                UPDATE documents
                SET current_text = ?, current_timestamp = ?, history_blob = ?,
                    history_timestamps = ?, revision_count = ?
                WHERE id = ?
                """,
                (
                    new_text,
                    timestamp,
                    history_blob,
                    timestamps,
                    new_revision,
                    document_id,
                ),
            )
            self.connection.execute("COMMIT")
            return new_revision
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise

    def versions(self, document_id: int) -> list[Version]:
        row = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        codec = self._codec(row["history_codec"])
        payload = codec.decompress(bytes(row["history_blob"]))
        texts = decode_payload(payload, row["history_format"])
        timestamps = decode_timestamp_json(str(row["history_timestamps"]))
        if len(texts) != len(timestamps) or len(texts) != int(row["revision_count"]):
            raise ValueError("History text/timestamp/revision counts do not match")
        output = [
            Version(revision=index, timestamp=ts, text=text)
            for index, (ts, text) in enumerate(zip(timestamps, texts, strict=True))
        ]
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
        versions = self.versions(document_id)
        if revision < 0:
            revision += len(versions)
        if revision < 0 or revision >= len(versions):
            raise IndexError(revision)
        return versions[revision]

    def storage_stats(self, document_id: int) -> dict[str, int]:
        row = self.connection.execute(
            """
            SELECT length(CAST(current_text AS BLOB)) AS current_text_bytes,
                   length(history_blob) AS history_blob_bytes,
                   length(history_timestamps) AS timestamp_json_bytes,
                   revision_count
            FROM documents WHERE id = ?
            """,
            (document_id,),
        ).fetchone()
        if row is None:
            raise KeyError(document_id)
        return {key: int(row[key]) for key in row.keys()}


class ChunkedHistoryStore:
    """Compressed history split into immutable-ish chunks.

    Only the current, not-yet-full chunk is decompressed and rewritten for each
    edit.  Once a chunk reaches ``chunk_size`` historical versions it is never
    touched again.
    """

    def __init__(
        self,
        path: str | Path,
        *,
        chunk_size: int = 32,
        wal: bool = True,
    ) -> None:
        if chunk_size < 1:
            raise ValueError("chunk_size must be positive")
        self.path = Path(path)
        self.chunk_size = chunk_size
        self.connection = _open_connection(path, wal=wal)
        self.connection.executescript(
            """
            CREATE TABLE IF NOT EXISTS documents (
                id INTEGER PRIMARY KEY,
                current_text TEXT NOT NULL,
                current_timestamp INTEGER NOT NULL,
                history_codec TEXT NOT NULL CHECK (history_codec IN ('zlib', 'zstd')),
                history_format TEXT NOT NULL CHECK (history_format IN ('json', 'framed')),
                revision_count INTEGER NOT NULL DEFAULT 0 CHECK (revision_count >= 0),
                chunk_size INTEGER NOT NULL CHECK (chunk_size > 0)
            );

            CREATE TABLE IF NOT EXISTS history_chunks (
                document_id INTEGER NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                chunk_number INTEGER NOT NULL,
                first_revision INTEGER NOT NULL,
                revision_count INTEGER NOT NULL CHECK (revision_count > 0),
                history_blob BLOB NOT NULL,
                history_timestamps TEXT NOT NULL,
                PRIMARY KEY (document_id, chunk_number)
            );
            """
        )
        self._codec_cache: dict[str, CompressionCodec] = {}

    def close(self) -> None:
        self.connection.close()

    def __enter__(self) -> "ChunkedHistoryStore":
        return self

    def __exit__(self, exc_type: object, exc: object, tb: object) -> None:
        self.close()

    def _codec(self, name: CodecName) -> CompressionCodec:
        codec = self._codec_cache.get(name)
        if codec is None:
            codec = make_codec(name)
            self._codec_cache[name] = codec
        return codec

    def create_document(
        self,
        text: str,
        timestamp: int,
        *,
        codec: CodecName = "zstd",
        format_name: FormatName = "framed",
    ) -> int:
        # Ensure the requested codec is actually available before inserting.
        self._codec(codec)
        cursor = self.connection.execute(
            """
            INSERT INTO documents (
                current_text, current_timestamp, history_codec, history_format,
                revision_count, chunk_size
            ) VALUES (?, ?, ?, ?, 0, ?)
            """,
            (text, timestamp, codec, format_name, self.chunk_size),
        )
        return int(cursor.lastrowid)

    def replace(
        self,
        document_id: int,
        new_text: str,
        timestamp: int,
        *,
        skip_unchanged: bool = True,
    ) -> int:
        self.connection.execute("BEGIN IMMEDIATE")
        try:
            document = self.connection.execute(
                "SELECT * FROM documents WHERE id = ?", (document_id,)
            ).fetchone()
            if document is None:
                raise KeyError(document_id)
            current_text = str(document["current_text"])
            if skip_unchanged and current_text == new_text:
                self.connection.execute("COMMIT")
                return int(document["revision_count"])

            history_count = int(document["revision_count"])
            chunk_size = int(document["chunk_size"])
            chunk_number = history_count // chunk_size
            codec_name: CodecName = document["history_codec"]
            format_name: FormatName = document["history_format"]
            codec = self._codec(codec_name)

            chunk = self.connection.execute(
                """
                SELECT * FROM history_chunks
                WHERE document_id = ? AND chunk_number = ?
                """,
                (document_id, chunk_number),
            ).fetchone()
            if chunk is None:
                payload = empty_payload(format_name)
                timestamps = "[]"
                chunk_count = 0
            else:
                payload = codec.decompress(bytes(chunk["history_blob"]))
                timestamps = str(chunk["history_timestamps"])
                chunk_count = int(chunk["revision_count"])

            if chunk_count >= chunk_size:
                raise ValueError("Active history chunk is unexpectedly full")
            payload = append_payload(payload, current_text, format_name)
            timestamps = append_timestamp_json(
                timestamps, int(document["current_timestamp"])
            )
            history_blob = codec.compress(payload)
            chunk_count += 1

            self.connection.execute(
                """
                INSERT INTO history_chunks (
                    document_id, chunk_number, first_revision, revision_count,
                    history_blob, history_timestamps
                ) VALUES (?, ?, ?, ?, ?, ?)
                ON CONFLICT (document_id, chunk_number) DO UPDATE SET
                    revision_count = excluded.revision_count,
                    history_blob = excluded.history_blob,
                    history_timestamps = excluded.history_timestamps
                """,
                (
                    document_id,
                    chunk_number,
                    chunk_number * chunk_size,
                    chunk_count,
                    history_blob,
                    timestamps,
                ),
            )
            new_revision = history_count + 1
            self.connection.execute(
                """
                UPDATE documents
                SET current_text = ?, current_timestamp = ?, revision_count = ?
                WHERE id = ?
                """,
                (new_text, timestamp, new_revision, document_id),
            )
            self.connection.execute("COMMIT")
            return new_revision
        except BaseException:
            self.connection.execute("ROLLBACK")
            raise

    def versions(self, document_id: int) -> list[Version]:
        document = self.connection.execute(
            "SELECT * FROM documents WHERE id = ?", (document_id,)
        ).fetchone()
        if document is None:
            raise KeyError(document_id)
        codec = self._codec(document["history_codec"])
        format_name: FormatName = document["history_format"]
        output: list[Version] = []
        chunks = self.connection.execute(
            """
            SELECT * FROM history_chunks
            WHERE document_id = ? ORDER BY chunk_number
            """,
            (document_id,),
        )
        for chunk in chunks:
            texts = decode_payload(
                codec.decompress(bytes(chunk["history_blob"])), format_name
            )
            timestamps = decode_timestamp_json(str(chunk["history_timestamps"]))
            expected = int(chunk["revision_count"])
            if len(texts) != expected or len(timestamps) != expected:
                raise ValueError("Chunk text/timestamp counts do not match")
            first = int(chunk["first_revision"])
            output.extend(
                Version(revision=first + i, timestamp=ts, text=text)
                for i, (ts, text) in enumerate(zip(timestamps, texts, strict=True))
            )
        if len(output) != int(document["revision_count"]):
            raise ValueError("Total chunk revision count does not match document")
        output.append(
            Version(
                revision=int(document["revision_count"]),
                timestamp=int(document["current_timestamp"]),
                text=str(document["current_text"]),
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
        total = revision_count + 1
        if revision < 0:
            revision += total
        if revision < 0 or revision >= total:
            raise IndexError(revision)
        if revision == revision_count:
            return Version(
                revision=revision,
                timestamp=int(document["current_timestamp"]),
                text=str(document["current_text"]),
                is_current=True,
            )

        chunk_size = int(document["chunk_size"])
        chunk_number = revision // chunk_size
        chunk = self.connection.execute(
            """
            SELECT * FROM history_chunks
            WHERE document_id = ? AND chunk_number = ?
            """,
            (document_id, chunk_number),
        ).fetchone()
        if chunk is None:
            raise ValueError("Missing history chunk")
        codec = self._codec(document["history_codec"])
        texts = decode_payload(
            codec.decompress(bytes(chunk["history_blob"])),
            document["history_format"],
        )
        timestamps = decode_timestamp_json(str(chunk["history_timestamps"]))
        local_index = revision - int(chunk["first_revision"])
        return Version(
            revision=revision,
            timestamp=timestamps[local_index],
            text=texts[local_index],
        )

    def storage_stats(self, document_id: int) -> dict[str, int]:
        document = self.connection.execute(
            "SELECT revision_count, length(CAST(current_text AS BLOB)) AS current_text_bytes "
            "FROM documents WHERE id = ?",
            (document_id,),
        ).fetchone()
        if document is None:
            raise KeyError(document_id)
        chunks = self.connection.execute(
            """
            SELECT count(*) AS chunk_count,
                   coalesce(sum(length(history_blob)), 0) AS history_blob_bytes,
                   coalesce(sum(length(history_timestamps)), 0) AS timestamp_json_bytes
            FROM history_chunks WHERE document_id = ?
            """,
            (document_id,),
        ).fetchone()
        return {
            "revision_count": int(document["revision_count"]),
            "current_text_bytes": int(document["current_text_bytes"]),
            "chunk_count": int(chunks["chunk_count"]),
            "history_blob_bytes": int(chunks["history_blob_bytes"]),
            "timestamp_json_bytes": int(chunks["timestamp_json_bytes"]),
        }


__all__ = [
    "ChunkedHistoryStore",
    "Document",
    "Version",
    "WholeBlobHistoryStore",
    "append_payload",
    "decode_payload",
    "make_codec",
]
