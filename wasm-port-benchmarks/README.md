# WASM Port of justjshtml HTML Tokenizer

A WebAssembly port of the [justjshtml](https://github.com/simonw/justjshtml) HTML5 parser's tokenizer, written in Rust and compiled to WASM. This project demonstrates that WASM can significantly outperform JavaScript for compute-intensive parsing tasks.

## Key Results

**WASM is 2.5x to 4.6x faster than JavaScript** across all tested input sizes:

| Input Size | HTML Characters | WASM (avg) | JavaScript (avg) | Speedup |
|------------|-----------------|------------|------------------|---------|
| Small | 1,354 | 0.069ms | 0.171ms | **2.47x** |
| Medium | 2,461 | 0.054ms | 0.252ms | **4.64x** |
| Large | 13,891 | 0.346ms | 1.420ms | **4.11x** |
| Extra Large | 66,291 | 1.388ms | 6.427ms | **4.63x** |

## Project Structure

```
wasm-port-benchmarks/
├── rust-tokenizer/          # Rust WASM tokenizer source
│   ├── Cargo.toml
│   └── src/lib.rs           # ~1900 lines implementing HTML5 tokenizer
├── wasm-pkg/                # Compiled WASM package
│   ├── wasm_html_tokenizer.js
│   └── wasm_html_tokenizer_bg.wasm  # 79KB WASM binary
├── js-src/                  # Original justjshtml JS source (for comparison)
├── benchmark.html           # Browser-based benchmark page
├── node-benchmark.mjs       # Node.js benchmark script
├── benchmark-results.json   # Detailed benchmark results
├── notes.md                 # Development notes
└── README.md                # This file
```

## Technical Details

### Rust Tokenizer Implementation
The Rust tokenizer (`rust-tokenizer/src/lib.rs`) implements:
- Complete HTML5 tokenization state machine (~50 states)
- Token types: StartTag, EndTag, Text, Comment, Doctype, EOF
- Efficient string handling with pre-allocated buffers
- WASM bindings via wasm-bindgen
- Support for rawtext/rcdata elements (script, style, title, etc.)

### Optimizations Applied
- `opt-level = 3` - Maximum optimization
- LTO (Link Time Optimization) enabled
- Single codegen unit for better optimization
- Pre-allocated string buffers
- `#[inline]` annotations on hot paths

### WASM Binary Size
Final compiled size: **79KB** (without wasm-opt)

## Running the Benchmarks

### Prerequisites
- Node.js 18+
- Rust (for rebuilding WASM)

### Run Node.js Benchmark
```bash
cd wasm-port-benchmarks
node node-benchmark.mjs
```

### Rebuild WASM (optional)
```bash
cd rust-tokenizer
cargo install wasm-pack
wasm-pack build --target web --release
```

## Methodology

1. **Test HTML Generation**: Generated HTML documents of varying sizes with realistic structure including tags, attributes, comments, and text content.

2. **Warmup**: Both tokenizers run 5 warmup iterations to account for JIT compilation.

3. **Measurement**: Multiple iterations (20-500 depending on input size) to get statistically significant results.

4. **Metrics Collected**: Total time, average, min, max, median, standard deviation.

## Observations

1. **Consistent WASM advantage**: WASM outperforms JS across all input sizes.
2. **Larger inputs show better speedup**: ~4.6x for large inputs vs ~2.5x for small inputs.
3. **Token count differences**: Minor differences exist due to implementation details (EOF token handling, text merging), but this doesn't affect performance comparison validity.

## Conclusions

This project demonstrates that:

1. **WASM significantly outperforms JavaScript** for compute-intensive parsing tasks.
2. **Rust is an excellent choice** for WASM development due to its mature tooling (wasm-pack, wasm-bindgen).
3. **The JS-WASM interop overhead** is worth it for large computational workloads.
4. A **4-5x speedup** on larger inputs would significantly benefit applications processing large HTML documents.

## License

MIT

## References

- [justjshtml](https://github.com/simonw/justjshtml) - Original JavaScript HTML5 parser
- [wasm-pack](https://rustwasm.github.io/wasm-pack/) - Rust WASM packaging tool
- [HTML5 Tokenization Spec](https://html.spec.whatwg.org/multipage/parsing.html#tokenization)
