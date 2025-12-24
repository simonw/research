#!/usr/bin/env python3
"""
Build optimized mquickjs WebAssembly with -O3 and --closure 1.
"""

import subprocess
import shutil
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
MQUICKJS_DIR = SCRIPT_DIR / "vendor" / "mquickjs"
WASM_BUILD_DIR = Path("/tmp/mquickjs-wasm-optimized")


def run(cmd, cwd=None, check=True):
    """Run a command."""
    print(f"  Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if check and result.returncode != 0:
        print(f"STDOUT: {result.stdout}")
        print(f"STDERR: {result.stderr}")
        raise RuntimeError(f"Command failed: {' '.join(cmd)}")
    return result


def ensure_mquickjs():
    """Ensure mquickjs is available in vendor directory."""
    if MQUICKJS_DIR.exists():
        print("mquickjs already available in vendor/")
        return
    print("Cloning mquickjs to vendor/...")
    MQUICKJS_DIR.parent.mkdir(parents=True, exist_ok=True)
    run(["git", "clone", "https://github.com/bellard/mquickjs.git", str(MQUICKJS_DIR)])


def build_wasm_optimized():
    """Build optimized mquickjs as WebAssembly with -O3."""
    print("Building OPTIMIZED mquickjs as WebAssembly (-O3)...")

    # Clean and recreate build directory
    if WASM_BUILD_DIR.exists():
        shutil.rmtree(WASM_BUILD_DIR)
    WASM_BUILD_DIR.mkdir()

    # Copy source files
    for f in ["mquickjs.c", "mquickjs.h", "mquickjs_priv.h", "mquickjs_opcode.h",
              "cutils.c", "cutils.h", "dtoa.c", "dtoa.h", "libm.c", "libm.h",
              "list.h", "mquickjs_build.c", "mquickjs_build.h", "mqjs_stdlib.c",
              "softfp_template.h", "softfp_template_icvt.h"]:
        src = MQUICKJS_DIR / f
        if src.exists():
            shutil.copy(src, WASM_BUILD_DIR / f)

    # First build the stdlib generator on the host
    print("  Building stdlib generator on host...")
    run(["gcc", "-Wall", "-g", "-O2", "-D_GNU_SOURCE", "-fno-math-errno",
         "-fno-trapping-math", "-c", "-o", "mqjs_stdlib.host.o", "mqjs_stdlib.c"],
        cwd=WASM_BUILD_DIR)
    run(["gcc", "-Wall", "-g", "-O2", "-D_GNU_SOURCE", "-fno-math-errno",
         "-fno-trapping-math", "-c", "-o", "mquickjs_build.host.o", "mquickjs_build.c"],
        cwd=WASM_BUILD_DIR)
    run(["gcc", "-g", "-o", "mqjs_stdlib_gen", "mqjs_stdlib.host.o", "mquickjs_build.host.o"],
        cwd=WASM_BUILD_DIR)

    # Generate headers for 32-bit target
    print("  Generating stdlib headers (32-bit)...")
    result = run(["./mqjs_stdlib_gen", "-m32"], cwd=WASM_BUILD_DIR)
    (WASM_BUILD_DIR / "mqjs_stdlib.h").write_text(result.stdout)

    result = run(["./mqjs_stdlib_gen", "-a", "-m32"], cwd=WASM_BUILD_DIR)
    (WASM_BUILD_DIR / "mquickjs_atom.h").write_text(result.stdout)

    # Create WASM wrapper
    wrapper_c = '''
/* WASM wrapper for mquickjs */
#include <emscripten.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdio.h>

#include "cutils.h"
#include "mquickjs.h"

/* Required by stdlib */
static JSValue js_print(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_UNDEFINED; }
static JSValue js_gc(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { JS_GC(ctx); return JS_UNDEFINED; }
static JSValue js_date_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_NewInt64(ctx, 0); }
static JSValue js_performance_now(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_NewInt64(ctx, 0); }
static JSValue js_load(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "disabled"); }
static JSValue js_setTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "disabled"); }
static JSValue js_clearTimeout(JSContext *ctx, JSValue *this_val, int argc, JSValue *argv) { return JS_ThrowTypeError(ctx, "disabled"); }

#include "mqjs_stdlib.h"

static uint8_t *mem_buf = NULL;
static JSContext *ctx = NULL;
static char result_buf[65536];
static char error_buf[4096];

EMSCRIPTEN_KEEPALIVE
int sandbox_init(int mem_size) {
    if (mem_buf) free(mem_buf);
    mem_buf = malloc(mem_size);
    if (!mem_buf) return 0;

    ctx = JS_NewContext(mem_buf, mem_size, &js_stdlib);
    if (!ctx) {
        free(mem_buf);
        mem_buf = NULL;
        return 0;
    }
    JS_SetRandomSeed(ctx, 12345);
    return 1;
}

EMSCRIPTEN_KEEPALIVE
void sandbox_free() {
    if (ctx) { JS_FreeContext(ctx); ctx = NULL; }
    if (mem_buf) { free(mem_buf); mem_buf = NULL; }
}

EMSCRIPTEN_KEEPALIVE
const char* sandbox_eval(const char *code) {
    if (!ctx) {
        strcpy(error_buf, "Not initialized");
        return NULL;
    }

    error_buf[0] = 0;
    result_buf[0] = 0;

    JSValue val = JS_Eval(ctx, code, strlen(code), "<sandbox>", JS_EVAL_RETVAL);

    if (JS_IsException(val)) {
        JS_GetErrorStr(ctx, error_buf, sizeof(error_buf));
        return NULL;
    }

    if (JS_IsUndefined(val)) {
        strcpy(result_buf, "undefined");
    } else if (JS_IsNull(val)) {
        strcpy(result_buf, "null");
    } else if (JS_IsBool(val)) {
        strcpy(result_buf, val == JS_TRUE ? "true" : "false");
    } else if (JS_IsInt(val)) {
        snprintf(result_buf, sizeof(result_buf), "%d", JS_VALUE_GET_INT(val));
    } else if (JS_IsNumber(ctx, val)) {
        double d;
        JS_ToNumber(ctx, &d, val);
        snprintf(result_buf, sizeof(result_buf), "%.17g", d);
    } else if (JS_IsString(ctx, val)) {
        JSCStringBuf buf;
        const char *str = JS_ToCString(ctx, val, &buf);
        if (str) {
            strncpy(result_buf, str, sizeof(result_buf) - 1);
            result_buf[sizeof(result_buf) - 1] = 0;
        }
    } else {
        JSValue str_val = JS_ToString(ctx, val);
        if (JS_IsString(ctx, str_val)) {
            JSCStringBuf buf;
            const char *str = JS_ToCString(ctx, str_val, &buf);
            if (str) {
                strncpy(result_buf, str, sizeof(result_buf) - 1);
                result_buf[sizeof(result_buf) - 1] = 0;
            } else {
                strcpy(result_buf, "[object]");
            }
        } else {
            strcpy(result_buf, "[object]");
        }
    }

    return result_buf;
}

EMSCRIPTEN_KEEPALIVE
const char* sandbox_get_error() {
    return error_buf;
}
'''
    (WASM_BUILD_DIR / "wasm_wrapper.c").write_text(wrapper_c)

    # Build 1: -O3 only (no closure)
    print("\n  === Build 1: -O3 optimization ===")
    run([
        "emcc",
        "-O3",
        "-s", "WASM=1",
        "-s", "ENVIRONMENT=node,web",
        "-s", "NODERAWFS=0",
        "-s", "EXPORTED_RUNTIME_METHODS=['ccall','cwrap','UTF8ToString','stringToUTF8']",
        "-s", "ALLOW_MEMORY_GROWTH=1",
        "-s", "MODULARIZE=1",
        "-s", "EXPORT_NAME='createMQuickJS'",
        "-s", "EXPORTED_FUNCTIONS=['_sandbox_init','_sandbox_free','_sandbox_eval','_sandbox_get_error','_malloc','_free']",
        "-D_GNU_SOURCE",
        "-I.",
        "-o", "mquickjs_o3.js",
        "wasm_wrapper.c",
        "mquickjs.c",
        "dtoa.c",
        "libm.c",
        "cutils.c",
    ], cwd=WASM_BUILD_DIR)

    # Build 2: -O3 + --closure 1
    print("\n  === Build 2: -O3 + --closure 1 ===")
    try:
        run([
            "emcc",
            "-O3",
            "--closure", "1",
            "-s", "WASM=1",
            "-s", "ENVIRONMENT=node,web",
            "-s", "NODERAWFS=0",
            "-s", "EXPORTED_RUNTIME_METHODS=['ccall','cwrap','UTF8ToString','stringToUTF8']",
            "-s", "ALLOW_MEMORY_GROWTH=1",
            "-s", "MODULARIZE=1",
            "-s", "EXPORT_NAME='createMQuickJS'",
            "-s", "EXPORTED_FUNCTIONS=['_sandbox_init','_sandbox_free','_sandbox_eval','_sandbox_get_error','_malloc','_free']",
            "-D_GNU_SOURCE",
            "-I.",
            "-o", "mquickjs_o3_closure.js",
            "wasm_wrapper.c",
            "mquickjs.c",
            "dtoa.c",
            "libm.c",
            "cutils.c",
        ], cwd=WASM_BUILD_DIR)
    except Exception as e:
        print(f"  WARNING: --closure 1 build failed: {e}")

    # Build 3: -Oz (size optimization)
    print("\n  === Build 3: -Oz (size optimization) ===")
    run([
        "emcc",
        "-Oz",
        "-s", "WASM=1",
        "-s", "ENVIRONMENT=node,web",
        "-s", "NODERAWFS=0",
        "-s", "EXPORTED_RUNTIME_METHODS=['ccall','cwrap','UTF8ToString','stringToUTF8']",
        "-s", "ALLOW_MEMORY_GROWTH=1",
        "-s", "MODULARIZE=1",
        "-s", "EXPORT_NAME='createMQuickJS'",
        "-s", "EXPORTED_FUNCTIONS=['_sandbox_init','_sandbox_free','_sandbox_eval','_sandbox_get_error','_malloc','_free']",
        "-D_GNU_SOURCE",
        "-I.",
        "-o", "mquickjs_oz.js",
        "wasm_wrapper.c",
        "mquickjs.c",
        "dtoa.c",
        "libm.c",
        "cutils.c",
    ], cwd=WASM_BUILD_DIR)

    # Compare sizes
    print("\n  === Size Comparison ===")
    import os
    original = SCRIPT_DIR / "mquickjs.wasm"
    if original.exists():
        print(f"  Original (-O2):     {original.stat().st_size:,} bytes")

    for name in ["mquickjs_o3.wasm", "mquickjs_o3_closure.wasm", "mquickjs_oz.wasm"]:
        f = WASM_BUILD_DIR / name
        if f.exists():
            print(f"  {name}: {f.stat().st_size:,} bytes")

    # Copy the best optimized version to the script directory
    # Use the -O3 build as the primary optimized version
    o3_wasm = WASM_BUILD_DIR / "mquickjs_o3.wasm"
    o3_js = WASM_BUILD_DIR / "mquickjs_o3.js"

    if o3_wasm.exists():
        shutil.copy(o3_wasm, SCRIPT_DIR / "mquickjs_optimized.wasm")
        shutil.copy(o3_js, SCRIPT_DIR / "mquickjs_optimized.js")
        print(f"\n  Built: {SCRIPT_DIR / 'mquickjs_optimized.wasm'}")
        print(f"  Built: {SCRIPT_DIR / 'mquickjs_optimized.js'}")

    # Also copy Oz version for comparison
    oz_wasm = WASM_BUILD_DIR / "mquickjs_oz.wasm"
    if oz_wasm.exists():
        shutil.copy(oz_wasm, SCRIPT_DIR / "mquickjs_oz.wasm")
        shutil.copy(WASM_BUILD_DIR / "mquickjs_oz.js", SCRIPT_DIR / "mquickjs_oz.js")
        print(f"  Built: {SCRIPT_DIR / 'mquickjs_oz.wasm'}")

    print("\nDone!")


if __name__ == "__main__":
    ensure_mquickjs()
    build_wasm_optimized()
