# mquickjs Sandbox Benchmark Results

**Date**: 2024-12-23
**System**: Linux (x86_64)
**Python**: 3.11

## Summary

| Implementation | Startup | arithmetic | string_concat | loop_100 | loop_1000 | recursion | array_ops | json |
|---------------|---------|------------|---------------|----------|-----------|-----------|-----------|------|
| **C Extension** | 0.011ms | **0.003ms** | **0.002ms** | **0.014ms** | **0.033ms** | **0.083ms** | **0.030ms** | **0.008ms** |
| **FFI (ctypes)** | 0.041ms | 0.007ms | 0.008ms | 0.019ms | 0.039ms | 0.086ms | 0.036ms | 0.013ms |
| **Wasmtime** | 56.612ms | 2.773ms | 2.318ms | 5.612ms | 5.574ms | 6.165ms | 8.046ms | 3.508ms |
| **Subprocess** | 0.134ms | 4.850ms | 4.610ms | 4.562ms | 4.635ms | 4.477ms | 4.839ms | 4.608ms |

*Best results marked in bold*

## Performance Ratios (vs C Extension)

| Implementation | Startup | Simple Op | Recursion |
|---------------|---------|-----------|-----------|
| C Extension | 1x | 1x | 1x |
| FFI (ctypes) | ~4x | ~2-3x | ~1x |
| Wasmtime | ~5000x | ~900x | ~74x |
| Subprocess | ~12x | ~1600x | ~54x |

## Detailed Results

### C Extension

```
Startup time: 0.011ms (±0.029ms)

Benchmark             Mean      StdDev
arithmetic            0.003ms   ±0.001ms
string_concat         0.002ms   ±0.000ms
loop_100              0.014ms   ±0.001ms
loop_1000             0.033ms   ±0.002ms
recursion (fib 15)    0.083ms   ±0.004ms
array_ops             0.030ms   ±0.019ms
json                  0.008ms   ±0.001ms
```

### FFI (ctypes)

```
Startup time: 0.041ms (±0.077ms)

Benchmark             Mean      StdDev
arithmetic            0.007ms   ±0.002ms
string_concat         0.008ms   ±0.001ms
loop_100              0.019ms   ±0.002ms
loop_1000             0.039ms   ±0.003ms
recursion (fib 15)    0.086ms   ±0.003ms
array_ops             0.036ms   ±0.019ms
json                  0.013ms   ±0.001ms
```

### Wasmtime (Python)

```
Startup time: 56.612ms (±14.147ms)

Benchmark             Mean      StdDev
arithmetic            2.773ms   ±0.137ms
string_concat         2.318ms   ±0.150ms
loop_100              5.612ms   ±0.398ms
loop_1000             5.574ms   ±0.312ms
recursion (fib 15)    6.165ms   ±0.280ms
array_ops             8.046ms   ±0.245ms
json                  3.508ms   ±0.202ms
```

### Subprocess

```
Startup time: 0.134ms (±0.047ms)

Benchmark             Mean      StdDev
arithmetic            4.850ms   ±0.307ms
string_concat         4.610ms   ±0.386ms
loop_100              4.562ms   ±0.483ms
loop_1000             4.635ms   ±0.330ms
recursion (fib 15)    4.477ms   ±0.422ms
array_ops             4.839ms   ±1.267ms
json                  4.608ms   ±0.421ms
```

## Benchmark Descriptions

| Benchmark | Description |
|-----------|-------------|
| **arithmetic** | `1 + 2 * 3 - 4 / 2` |
| **string_concat** | `'hello' + ' ' + 'world'` |
| **loop_100** | Sum integers 0-99 in a loop |
| **loop_1000** | Sum integers 0-999 in a loop |
| **recursion** | Fibonacci(15) recursive |
| **array_ops** | Create array, push 100 items, map |
| **json** | `JSON.stringify({a: 1, b: [1,2,3], c: {d: 'test'}})` |

## Key Findings

1. **C Extension is fastest** for all operations
   - ~4x faster startup than FFI (no ctypes overhead)
   - Execution performance nearly identical to FFI

2. **FFI (ctypes) is the best balance** of performance and ease of use
   - No compilation step required
   - Microsecond-level execution times
   - ~2-3x slower than C Extension for simple ops

3. **Wasmtime has significant overhead** (~300-900x slower than FFI)
   - 57ms startup time for WASM compilation
   - Each operation crosses Python/WASM boundary
   - May be worthwhile for security-critical scenarios

4. **Subprocess is ~500x slower** than native methods
   - Process spawn overhead dominates (~4.5ms per execution)
   - Acceptable for one-off executions
   - Not suitable for interactive or high-throughput use

## WASM Runtime Notes

### Wasmer

The wasmer Python bindings require platform-specific native libraries not available in all environments. The wasmer CLI (v4.4.0) can run WASM modules but requires the same `invoke_*` imports as wasmtime due to mquickjs's use of setjmp/longjmp:

```
error: Instantiation failed - Error while importing "env"."invoke_iii": unknown import
```

### Wasmtime

Wasmtime works via custom `invoke_*` trampolines that:
1. Call through the `__indirect_function_table`
2. Catch WASM traps signaling longjmp
3. Manage emscripten's setjmp/longjmp state

See the main README for implementation details.

## Recommendations

| Use Case | Recommended Implementation |
|----------|---------------------------|
| Production server | C Extension |
| Development/prototyping | FFI (ctypes) |
| One-off scripts | Subprocess |
| Browser/Node.js | WASM |
| Maximum isolation | Wasmtime |
| Cross-platform | FFI (ctypes) |
