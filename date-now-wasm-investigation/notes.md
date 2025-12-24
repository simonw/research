# Date.now() WASM Investigation Notes

## Summary

**`Date.now()` is intentionally stubbed to return 0** in all mquickjs-sandbox builds (WASM, C extension, FFI) for determinism. This is by design, not a bug.

The original mquickjs C implementation (mqjs.c) has a working `Date.now()` using `gettimeofday()`.

## Investigation Log

### Step 1: Code Exploration

Found the key files:
- `build_wasm.py` line 88: `js_date_now` returns `JS_NewInt64(ctx, 0)`
- `build_ffi.py` lines 72-77: Same stubbed implementation
- `setup.py` line 93: C extension also stubs to 0
- Original `mqjs.c` lines 90-95: Uses `gettimeofday()` for real time

### Step 2: Testing WASM Build

```
$ node test_date_now.js
Testing Date.now():
  First call: 0
  Second call: 0
  Third call: 0
```

### Step 3: Testing Original C Implementation (with real time)

```
$ ./test_date_now_c
Testing Date.now():
  Call 1: 1766549725750
  Call 2: 1766549725760
  Call 3: 1766549725771
```

### Step 4: Testing C with Stubbed Implementation (sandbox)

```
$ ./test_date_now_stub
Testing Date.now():
  Call 1: 0
  Call 2: 0
  Call 3: 0
```

## Key Findings

1. **The stub is intentional** - All sandbox implementations (WASM, FFI, C ext) stub `Date.now()` and `performance.now()` to return 0 for determinism

2. **The original mquickjs works correctly** - `mqjs.c` implements `js_date_now` using `gettimeofday()` which returns real timestamps

3. **Why the stub exists** - Comments say "Return 0 for determinism in sandbox" - this is a security/reproducibility feature:
   - Prevents timing side-channel attacks
   - Ensures reproducible execution results
   - Common practice for sandboxed JS environments

4. **mquickjs core only supports Date.now()** - The Date constructor throws: "only Date.now() is supported" (line 15191 in mquickjs.c)

## Architecture

```
mqjs_stdlib.c (defines Date class)
       │
       ├── References js_date_now (external symbol)
       │
       ├── Original mqjs.c
       │     └── js_date_now() uses gettimeofday() → REAL TIME
       │
       └── Sandbox builds (WASM, FFI, C ext)
             └── js_date_now() returns 0 → STUBBED
```
