# h3o-python

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

`h3o-python` provides Python bindings for the [h3o](https://github.com/HydroniumLabs/h3o)
Rust implementation of the [H3](https://h3geo.org/) geospatial indexing system.
It exposes a concise set of high-level helpers for converting between latitude
and longitude pairs, H3 cell indexes, and derived metrics such as neighborhood
relationships and great-circle distances.

The bindings are implemented with [PyO3](https://pyo3.rs/) and packaged with
[`maturin`](https://github.com/PyO3/maturin).  Wheels produced by this project
bundle the compiled Rust extension, so no system-level H3 installation is
required at runtime.

## Features

- Encode latitude/longitude pairs into 64-bit H3 cell indexes.
- Convert cell indexes back to geographic coordinates.
- Compute neighborhood disks and test adjacency between cells.
- Report average and per-cell surface areas at different resolutions.
- Measure great-circle distances using the same algorithms as the Rust crate.
- Lossless conversion between integer and string encodings of H3 indexes.

## Project layout

```
h3o-python/
├── Cargo.toml          # Rust crate manifest for the extension module
├── pyproject.toml      # Python packaging metadata for maturin
├── python/
│   └── h3o_python/__init__.py  # Python shim that re-exports the Rust functions
├── src/lib.rs          # PyO3 module exposing the Rust bindings
├── tests/              # pytest-based functional coverage
└── notes.md            # Running log of implementation details
```

## Building from source

1. Install the prerequisites:
   - A stable Rust toolchain (tested with `rustc 1.89`).
   - Python 3.8 or newer.
   - `maturin` and `pytest` in the active virtual environment.

2. Install the package in editable mode:

   ```bash
   maturin develop
   ```

   The command downloads the upstream `h3o` crate from GitHub, compiles the
   extension module, and installs it into the current Python environment.

3. (Optional) Build a wheel ready for distribution:

   ```bash
   maturin build --release
   ```

   The resulting wheel inside `target/wheels/` bundles the compiled extension
   and the lightweight Python shim.

## Running the tests

The test suite relies on the compiled extension, so ensure `maturin develop` has
been executed first.  Then run:

```bash
pytest
```

The tests exercise coordinate conversions, neighborhood queries, numerical
calculations, and string/integer round-trips for H3 cell indexes.

## Usage example

```python
import h3o_python as h3o

# Convert a coordinate into an H3 cell at resolution 9.
cell = h3o.latlng_to_cell(37.7759387, -122.4179506, 9)
print(cell)  # 617700169958293503

# Inspect the cell center coordinate.
print(h3o.cell_to_latlng(cell))

# Fetch the cell's neighbors and check adjacency.
neighbors = h3o.grid_disk(cell, 1)
print(len(neighbors))  # 7
print(h3o.are_neighbors(cell, neighbors[1]))

# Compute the surface area represented by the cell in square kilometers.
print(h3o.cell_area_km2(cell))
```

## License

The Python bindings are distributed under the BSD 3-Clause license, mirroring
`h3o`'s upstream licensing.
