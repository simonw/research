h3o-python delivers efficient Python bindings for the [h3o](https://github.com/HydroniumLabs/h3o) Rust library, enabling fast and convenient access to H3 geospatial indexing from Python. Utilizing [PyO3](https://pyo3.rs/) and packaged with maturin, it allows encoding geographic coordinates into 64-bit H3 cell indexes, decoding indexes, performing neighborhood queries, calculating great-circle distances, and retrieving surface area metrics—all without requiring a separate H3 installation. The module bundles its Rust extension in the distributable wheel for seamless deployment, and the API mirrors the upstream Rust crate for high performance and compatibility.

**Key capabilities:**
- Simple conversion between latitude/longitude and H3 cell indexes
- Neighborhood and adjacency checks, and disk queries
- Accurate area and distance calculations using H3 algorithms
- Lossless string/integer conversions of H3 indexes
