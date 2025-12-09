Epsilon Python Wrapper provides seamless Python bindings to [Epsilon](https://github.com/ziggy42/epsilon), Google's pure Go WebAssembly 2.0 runtime, enabling efficient and dependency-free WASM execution within Python projects. The wrapper exposes a simple API for module instantiation, function calls (with type safety), memory operations, and export inspection, supporting advanced features like SIMD and resource limiting. While it allows for configurable memory restrictions and function timeouts, true execution interruption (context cancellation or instruction counting) is not supported; thus, alternative CPU limiting strategies are suggested. Epsilon prioritizes clean architecture, zero external dependencies, and ease of embedding, making it a practical choice for Python users needing Go-native WASM capabilities but does not offer WASI or multi-threading.

Key points:
- Enables direct loading, calling, and memory manipulation of WASM modules from Python.
- Supports WebAssembly 2.0—including SIMD, multiple memories (experimental), and host functions.
- Resource limits: configurable memory per module; fixed call stack (1000 frames); no preemptive timeout or instruction counting.
- For CPU/time limiting, subprocesses or system-level approaches are recommended.
- Inspired by projects like [wazero-python](https://github.com/user/wazero-python), but focused on Epsilon's Go-native strengths.
