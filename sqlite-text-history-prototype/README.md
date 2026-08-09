# SQLite compressed text-history prototypes

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This directory contains working Python prototypes for storing every previous
version of an edited text value in SQLite.

The two implementations are:

1. **`WholeBlobHistoryStore`** — current text remains ordinary SQLite `TEXT`;
   every previous text is stored in one compressed BLOB containing a JSON array
   (or an experimental length-prefixed binary stream). Timestamps are a
   separate JSON integer array.
2. **`ChunkedHistoryStore`** — the same idea, but historical snapshots are split
   into sealed compressed chunks so each edit rewrites only the active chunk.

The detailed findings and recommendations are in [`REPORT.md`](REPORT.md).

## Minimal example

```python
from text_history import WholeBlobHistoryStore

with WholeBlobHistoryStore("documents.db") as history:
    document_id = history.create_document(
        "First draft",
        timestamp=1_800_000_000,
        codec="zstd",
        format_name="json",
    )

    history.replace(document_id, "Second draft", timestamp=1_800_000_060)
    history.replace(document_id, "Final draft", timestamp=1_800_000_120)

    for version in history.versions(document_id):
        print(version.revision, version.timestamp, version.text)
```

For a history that may grow indefinitely:

```python
from text_history import ChunkedHistoryStore

with ChunkedHistoryStore("documents.db", chunk_size=128) as history:
    document_id = history.create_document(
        "First draft",
        timestamp=1_800_000_000,
        codec="zstd",
        format_name="json",
    )
    history.replace(document_id, "Second draft", timestamp=1_800_000_060)
```

The prototype takes an integer timestamp but does not prescribe seconds versus
milliseconds. Use one convention consistently.

## Zstandard availability

The code tries these implementations in order:

1. Python 3.14's `compression.zstd` standard-library module.
2. The third-party `zstandard` package.
3. A small `ctypes` wrapper around an installed `libzstd`.

For a portable application on Python 3.10–3.13, install the package:

```bash
python -m pip install zstandard
```

`codec="zlib"` always works using the Python standard library, but the
benchmarks found Zstandard both smaller and faster for long histories.

## Run the tests

```bash
python -m unittest -v
```

## Reproduce the main benchmark

```bash
python benchmark.py \
  --document-bytes 20000 \
  --revisions 300 \
  --scaling-revisions 1000 \
  --trials 3 \
  --output benchmark-output
```

A focused 1,000-edit comparison:

```bash
python benchmark.py \
  --revisions 1000 \
  --scaling-revisions 1000 \
  --trials 3 \
  --strategies whole_json_zstd,chunked_json_zstd_64,chunked_json_zstd_128 \
  --output benchmark-output-focused
```

The monolithic strategy is deliberately slow at 1,000 edits, because the point
of the experiment is to expose its scaling behavior.

## Run the edit-entropy experiment

```bash
python sensitivity.py --output sensitivity-output
```

## Create databases to inspect in Datasette

```bash
python make_demo_databases.py
datasette demo-databases/*.db
```

The generated examples use 25 revisions of a 5 KB document.

## Important semantic choices

- The current version is not duplicated in history.
- A version timestamp means “this version became current at this time.”
- Replacing the current value appends the old current value and timestamp to
  history, then installs the new value and timestamp atomically.
- Unchanged replacements are skipped by default.
- `BEGIN IMMEDIATE` serializes writers to avoid losing a revision.

This is exploratory code rather than a published package, but it has no required
runtime dependencies beyond Python and SQLite when using zlib.
