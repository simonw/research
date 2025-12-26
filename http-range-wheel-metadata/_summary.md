Leveraging ZIP file structure and HTTP range requests, tools like [uv](https://github.com/astral-sh/uv) efficiently extract wheel metadata for Python packages without downloading entire archives. By fetching just the last 16KB of the wheel (central directory and EOCD), parsing for the METADATA file offset, and then requesting exactly its byte range, uv and the accompanying Python prototype routinely reduce bandwidth usage by over 70%. This approach drastically speeds up dependency resolution for large wheels, provided PyPI or the package index supports range requests. In tandem, uv’s innovative packing of PEP 440 version information into a single `u64` integer accelerates version comparisons from O(n) string parsing to fast integer checks, affecting millions of operations during package resolution. Together, these methods showcase how protocol and data structure choices can compound to improve package manager performance.

**Key Findings:**
- Fetching only necessary ZIP sections saves significant bandwidth—typically over 70% per wheel.
- HTTP range request support is prevalent across major Python package repositories.
- Over 90% of PyPI versions fit uv’s compact integer representation, enabling efficient version sorting and comparison.
- Methodology is reproducible in Python (see [wheel_metadata.py](https://github.com/astral-sh/uv) for relevant tool).
