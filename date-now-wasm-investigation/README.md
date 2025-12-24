# Date.now() Investigation in MicroQuickJS WebAssembly Builds

## Question

Does `Date.now()` work in the MicroQuickJS WebAssembly builds? Does it work in the C version?

## Answer

**`Date.now()` always returns 0 in the WASM builds (and C extension/FFI builds) by design.**

This is an intentional stub for sandbox determinism, not a bug. The original mquickjs C implementation (`mqjs.c`) has a fully working `Date.now()` that returns real timestamps.

## Evidence

### WASM Build Test
```
$ node test_date_now.js
Testing Date.now():
  First call: 0
  Second call: 0
  Third call: 0
```

### Original C Implementation (Real Time)
```
$ ./test_date_now_c
Testing Date.now():
  Call 1: 1766549725750
  Call 2: 1766549725760
  Call 3: 1766549725771
```

## Technical Details

### Where the Stub is Defined

| Build Type | File | Line | Implementation |
|------------|------|------|----------------|
| WASM | `build_wasm.py` | 88 | `return JS_NewInt64(ctx, 0);` |
| FFI | `build_ffi.py` | 72-77 | `return JS_NewInt64(ctx, 0);` |
| C Extension | `setup.py` | 93 | `return JS_NewInt64(ctx, 0);` |
| Original mqjs | `mqjs.c` | 90-95 | `gettimeofday()` → real time |

### Why the Stub Exists

The comment in `build_ffi.py` explains:
```c
/* Return 0 for determinism in sandbox */
```

This is a deliberate security/reproducibility feature:
1. **Prevents timing side-channel attacks** - Attackers can't use timing information
2. **Ensures reproducible execution** - Same code always produces same results
3. **Common sandboxing practice** - Many sandboxed JS environments do this

### mquickjs Date Limitations

The mquickjs core has limited Date support. From `mquickjs.c:15191`:
```c
return JS_ThrowTypeError(ctx, "only Date.now() is supported");
```

The `Date` constructor throws an error - only `Date.now()` is available, and in sandbox builds, it always returns 0.

## Architecture

```
mqjs_stdlib.c
    │
    ├── Defines Date class with Date.now property
    │   └── References external js_date_now function
    │
    ├── Original mqjs.c (REPL)
    │   └── js_date_now() uses gettimeofday() → REAL TIMESTAMPS
    │
    └── Sandbox builds (WASM/FFI/C Extension)
        └── js_date_now() returns 0 → STUBBED FOR DETERMINISM
```

## Recommendations

If real time is needed in the sandbox:

1. **Provide time externally** - Pass timestamps as function arguments
2. **Modify the build scripts** - Replace the stub with a real implementation
3. **Use a different approach** - For timing-sensitive code, consider alternatives

### Example: Adding Real Time Support

To enable real Date.now() in WASM, modify `build_wasm.py` line 88:

```c
// Instead of:
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    return JS_NewInt64(ctx, 0);
}

// Use (requires emscripten time support):
#include <sys/time.h>
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv)
{
    struct timeval tv;
    gettimeofday(&tv, NULL);
    return JS_NewInt64(ctx, (int64_t)tv.tv_sec * 1000 + (tv.tv_usec / 1000));
}
```

## Files in This Investigation

- `notes.md` - Detailed investigation notes
- `test_date_now.js` - WASM test script
- `test_date_now_c.c` - C test with real time implementation
- `test_date_now_stub.c` - C test with stubbed implementation
- `test_date_now_c` - Compiled C test (real time)
- `test_date_now_stub` - Compiled C test (stubbed)
