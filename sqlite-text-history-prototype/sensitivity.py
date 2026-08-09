#!/usr/bin/env python3
"""Storage sensitivity experiments for compressed snapshot histories.

This separates ordinary text compressibility from cross-version redundancy by
using deterministic high-entropy ASCII documents and varying how much of each
snapshot changes.
"""

from __future__ import annotations

import argparse
import json
import random
from pathlib import Path
from typing import Callable

from benchmark import encode_framed, format_bytes, make_workload
from text_history import make_codec

SAFE_ASCII = (
    "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    " !#$%&'()*+,-./:;<=>?@[]^_`{|}~\n"
)


def random_text(rng: random.Random, length: int) -> str:
    return "".join(rng.choice(SAFE_ASCII) for _ in range(length))


def high_entropy_history(
    *,
    document_bytes: int,
    revisions: int,
    replacement_bytes: int | None,
    independent: bool = False,
    seed: int = 90210,
) -> list[str]:
    rng = random.Random(seed)
    versions = [random_text(rng, document_bytes)]
    for revision in range(revisions):
        if independent:
            versions.append(random_text(rng, document_bytes))
            continue
        assert replacement_bytes is not None
        size = min(replacement_bytes, document_bytes)
        start = rng.randrange(0, document_bytes - size + 1)
        replacement = random_text(rng, size)
        previous = versions[-1]
        versions.append(previous[:start] + replacement + previous[start + size :])
    return versions


def encode_json(strings_: list[str]) -> bytes:
    return json.dumps(strings_, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def chunked_size(versions: list[str], chunk_size: int) -> int:
    codec = make_codec("zstd")
    history = versions[:-1]
    return sum(
        len(codec.compress(encode_json(history[index : index + chunk_size])))
        for index in range(0, len(history), chunk_size)
    )


def scenario_result(name: str, versions: list[str]) -> dict[str, int | float | str]:
    codec = make_codec("zstd")
    history = versions[:-1]
    raw_bytes = sum(len(value.encode("utf-8")) for value in history)
    json_payload = encode_json(history)
    whole = len(codec.compress(json_payload))
    per_row = sum(len(codec.compress(value.encode("utf-8"))) for value in history)
    return {
        "scenario": name,
        "versions": len(history),
        "raw_snapshot_bytes": raw_bytes,
        "zstd_per_row_bytes": per_row,
        "whole_json_zstd_bytes": whole,
        "chunked_json_zstd_64_bytes": chunked_size(versions, 64),
        "chunked_json_zstd_128_bytes": chunked_size(versions, 128),
        "whole_vs_raw_percent": whole * 100.0 / raw_bytes,
    }


def markdown_table(headers: list[str], rows: list[list[str]]) -> str:
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    lines.extend("| " + " | ".join(row) + " |" for row in rows)
    return "\n".join(lines)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--document-bytes", type=int, default=20_000)
    parser.add_argument("--revisions", type=int, default=1_000)
    parser.add_argument("--output", type=Path, default=Path("sensitivity-output"))
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)

    prose = make_workload(
        document_bytes=args.document_bytes,
        revisions=args.revisions,
    ).versions
    scenarios = [
        ("pseudo-English, small mixed edits", prose),
        (
            "high-entropy, 20-byte replacements",
            high_entropy_history(
                document_bytes=args.document_bytes,
                revisions=args.revisions,
                replacement_bytes=20,
            ),
        ),
        (
            "high-entropy, 1% replaced/edit",
            high_entropy_history(
                document_bytes=args.document_bytes,
                revisions=args.revisions,
                replacement_bytes=max(1, args.document_bytes // 100),
            ),
        ),
        (
            "high-entropy, 10% replaced/edit",
            high_entropy_history(
                document_bytes=args.document_bytes,
                revisions=args.revisions,
                replacement_bytes=max(1, args.document_bytes // 10),
            ),
        ),
        (
            "independent high-entropy snapshots",
            high_entropy_history(
                document_bytes=args.document_bytes,
                revisions=args.revisions,
                replacement_bytes=None,
                independent=True,
            ),
        ),
    ]

    results = [scenario_result(name, versions) for name, versions in scenarios]
    payload = {
        "config": {
            "document_bytes": args.document_bytes,
            "revisions": args.revisions,
        },
        "scenarios": results,
    }
    (args.output / "sensitivity-results.json").write_text(
        json.dumps(payload, indent=2), encoding="utf-8"
    )

    rows = [
        [
            str(result["scenario"]),
            format_bytes(int(result["raw_snapshot_bytes"])),
            format_bytes(int(result["zstd_per_row_bytes"])),
            format_bytes(int(result["whole_json_zstd_bytes"])),
            format_bytes(int(result["chunked_json_zstd_64_bytes"])),
            format_bytes(int(result["chunked_json_zstd_128_bytes"])),
            f"{float(result['whole_vs_raw_percent']):.2f}%",
        ]
        for result in results
    ]
    markdown = f"""# Compression sensitivity results

Each scenario stores {args.revisions:,} historical snapshots of an approximately
{args.document_bytes / 1000:.0f} KB document. The high-entropy scenarios deliberately
remove the ordinary compressibility of English text, isolating the value of similarity
between adjacent revisions.

{markdown_table(
    [
        'Scenario', 'Raw snapshots', 'Zstd each row', 'One JSON blob',
        'JSON chunks of 64', 'JSON chunks of 128', 'Whole/raw',
    ],
    rows,
)}
"""
    (args.output / "SENSITIVITY.md").write_text(markdown, encoding="utf-8")
    print(markdown)


if __name__ == "__main__":
    main()
