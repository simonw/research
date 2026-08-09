#!/usr/bin/env python3
"""Create small SQLite databases that can be opened directly in Datasette."""

from __future__ import annotations

import argparse
from pathlib import Path

from benchmark import make_workload
from text_history import ChunkedHistoryStore, WholeBlobHistoryStore


def populate_whole(path: Path, versions: list[str], timestamps: list[int]) -> None:
    path.unlink(missing_ok=True)
    with WholeBlobHistoryStore(path, wal=False) as store:
        document_id = store.create_document(
            versions[0], timestamps[0], codec="zstd", format_name="json"
        )
        for text, timestamp in zip(versions[1:], timestamps[1:], strict=True):
            store.replace(document_id, text, timestamp)


def populate_chunked(path: Path, versions: list[str], timestamps: list[int]) -> None:
    path.unlink(missing_ok=True)
    with ChunkedHistoryStore(path, chunk_size=8, wal=False) as store:
        document_id = store.create_document(
            versions[0], timestamps[0], codec="zstd", format_name="json"
        )
        for text, timestamp in zip(versions[1:], timestamps[1:], strict=True):
            store.replace(document_id, text, timestamp)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=Path("demo-databases"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)

    workload = make_workload(document_bytes=5_000, revisions=25)
    whole = args.output / "whole-history.db"
    chunked = args.output / "chunked-history.db"
    populate_whole(whole, workload.versions, workload.timestamps)
    populate_chunked(chunked, workload.versions, workload.timestamps)
    print(whole)
    print(chunked)


if __name__ == "__main__":
    main()
