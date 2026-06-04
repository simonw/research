# SQLite Time Limit Extension

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

## Overview
This folder contains a small Python C extension that exposes a single function,
`execute_with_timeout`, which executes SQL against a SQLite database and enforces
a time limit using SQLite's progress handler. The timeout is expressed in
milliseconds and raises `TimeoutError` when exceeded.

## Layout
- `src/sqlite_time_limit/time_limit.c`: C extension implementation.
- `src/sqlite_time_limit/__init__.py`: Python export for the extension function.
- `tests/test_time_limit.py`: pytest coverage for success, timeout, and validation.
- `notes.md`: chronological notes captured during development.

## Usage
```python
import sqlite_time_limit

rows = sqlite_time_limit.execute_with_timeout("example.db", "SELECT 1", 1000)
```

## Development
```bash
python -m pip install -e .
pytest -q
```
