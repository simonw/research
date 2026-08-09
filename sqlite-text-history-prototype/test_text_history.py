from __future__ import annotations

import sqlite3
import tempfile
import unittest
from pathlib import Path

from text_history import (
    ChunkedHistoryStore,
    WholeBlobHistoryStore,
    append_payload,
    decode_payload,
    make_codec,
)


class PayloadTests(unittest.TestCase):
    def test_json_and_framed_round_trip(self) -> None:
        values = [
            "",
            "plain text",
            'quotes: " and slash: \\',
            "Unicode: café, 雪, 🐕",
            "line one\nline two\n",
        ]
        for format_name, initial in (("json", b"[]"), ("framed", b"")):
            with self.subTest(format_name=format_name):
                payload = initial
                for value in values:
                    payload = append_payload(payload, value, format_name)
                self.assertEqual(values, decode_payload(payload, format_name))

    def test_codecs_round_trip(self) -> None:
        value = ("a document with repeated text\n" * 1_000).encode()
        for codec_name in ("zlib", "zstd"):
            with self.subTest(codec_name=codec_name):
                try:
                    codec = make_codec(codec_name)
                except RuntimeError as ex:
                    if codec_name == "zstd":
                        self.skipTest(str(ex))
                    raise
                compressed = codec.compress(value)
                self.assertEqual(value, codec.decompress(compressed))
                self.assertLess(len(compressed), len(value))


class WholeBlobStoreTests(unittest.TestCase):
    def test_round_trip_reopen_and_timestamp_semantics(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "history.db"
            with WholeBlobHistoryStore(path, wal=False) as store:
                document_id = store.create_document(
                    "first", 100, codec="zlib", format_name="json"
                )
                self.assertEqual(1, store.replace(document_id, "second", 200))
                self.assertEqual(2, store.replace(document_id, "third", 300))

            with WholeBlobHistoryStore(path, wal=False) as reopened:
                versions = reopened.versions(document_id)
                self.assertEqual(["first", "second", "third"], [v.text for v in versions])
                self.assertEqual([100, 200, 300], [v.timestamp for v in versions])
                self.assertEqual([False, False, True], [v.is_current for v in versions])
                self.assertEqual("second", reopened.get_version(document_id, 1).text)
                self.assertEqual("third", reopened.get_version(document_id, -1).text)

    def test_unchanged_text_is_not_a_revision_by_default(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "history.db"
            with WholeBlobHistoryStore(path, wal=False) as store:
                document_id = store.create_document("same", 100, codec="zlib")
                self.assertEqual(0, store.replace(document_id, "same", 200))
                self.assertEqual(1, len(store.versions(document_id)))
                self.assertEqual(1, store.replace(document_id, "same", 300, skip_unchanged=False))
                self.assertEqual(2, len(store.versions(document_id)))

    def test_detects_misaligned_timestamp_array(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "history.db"
            with WholeBlobHistoryStore(path, wal=False) as store:
                document_id = store.create_document("one", 1, codec="zlib")
                store.replace(document_id, "two", 2)
                store.connection.execute(
                    "UPDATE documents SET history_timestamps = '[]' WHERE id = ?",
                    (document_id,),
                )
                with self.assertRaisesRegex(ValueError, "counts do not match"):
                    store.versions(document_id)


class ChunkedStoreTests(unittest.TestCase):
    def test_chunk_boundaries_and_random_access(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "history.db"
            with ChunkedHistoryStore(path, chunk_size=3, wal=False) as store:
                document_id = store.create_document(
                    "version 0", 1_000, codec="zlib", format_name="json"
                )
                expected = ["version 0"]
                for revision in range(1, 11):
                    text = f"version {revision} — snowman ☃"
                    expected.append(text)
                    store.replace(document_id, text, 1_000 + revision)

                versions = store.versions(document_id)
                self.assertEqual(expected, [version.text for version in versions])
                self.assertEqual(10, versions[-1].revision)
                self.assertTrue(versions[-1].is_current)
                for revision, text in enumerate(expected):
                    self.assertEqual(text, store.get_version(document_id, revision).text)

                stats = store.storage_stats(document_id)
                self.assertEqual(4, stats["chunk_count"])
                self.assertEqual(10, stats["revision_count"])

    def test_database_transaction_rolls_back_on_bad_document_id(self) -> None:
        with tempfile.TemporaryDirectory() as temporary_directory:
            path = Path(temporary_directory) / "history.db"
            with ChunkedHistoryStore(path, chunk_size=3, wal=False) as store:
                with self.assertRaises(KeyError):
                    store.replace(999, "missing", 1)
                # A failed transaction must not leave the connection unusable.
                document_id = store.create_document("works", 1, codec="zlib")
                store.replace(document_id, "still works", 2)
                self.assertEqual(2, len(store.versions(document_id)))


if __name__ == "__main__":
    unittest.main()
