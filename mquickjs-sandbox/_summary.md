Exploring [mquickjs](https://github.com/bellard/mquickjs), a highly minimal JavaScript engine, this project rigorously evaluates its suitability as a safe sandbox for running untrusted code. Various integration approaches are implemented, including Python FFI, C extensions, subprocess invocation, and WebAssembly runtimes—each tested for startup and execution performance, security isolation, and feature compatibility. The investigation finds mquickjs's strict memory and execution time limits effectively minimize risk, and its restricted runtime (no file/network APIs) bolsters safety in hostile environments. While FFI and C extension interfaces yield microsecond-level execution suitable for interactive workloads, WebAssembly runtimes like [wasmtime](https://github.com/bytecodealliance/wasmtime) offer platform-agnostic isolation at the cost of much slower startup. mquickjs's ES5-like dialect lacks newer JavaScript features but remains sufficient for most sandboxed uses.

Key findings:
- FFI and C Extension deliver near-native performance, with C Extension starting ~4x faster.
- Subprocess wrapper is simple but 500x slower due to process spawning.
- WASM runtimes (wasmtime) enhance isolation but are 300–900x slower than native bindings.
- Regex DoS is mitigated via time limits; still, pre-validation is recommended.
- For most workloads, FFI is preferred; WASM/wasmtime best for environments requiring strong sandboxing or portability.
