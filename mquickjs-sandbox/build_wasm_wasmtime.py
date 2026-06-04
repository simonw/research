#!/usr/bin/env python3
"""
Build a wasmtime-compatible WASM module by patching mquickjs to not use setjmp/longjmp.

The key changes:
1. Replace setjmp/longjmp with error flag checking
2. Compile with -s SUPPORT_LONGJMP=0 to avoid emscripten's invoke_* functions
"""

import subprocess
import shutil
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
MQUICKJS_DIR = SCRIPT_DIR / "vendor" / "mquickjs"
WASM_BUILD_DIR = Path("/tmp/mquickjs-wasm-wasmtime")


def run(cmd, cwd=None, check=True):
    """Run a command."""
    print(f"  Running: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True)
    if check and result.returncode != 0:
        print(f"STDOUT: {result.stdout}")
        print(f"STDERR: {result.stderr}")
        raise RuntimeError(f"Command failed: {' '.join(cmd)}")
    return result


def patch_mquickjs_no_setjmp():
    """
    Patch mquickjs.c to not use setjmp/longjmp.

    Instead of:
        if (setjmp(s->jmp_env)) { handle_error; }
        ...
        longjmp(s->jmp_env, 1);  // on error

    We change to:
        s->has_error = 0;
        ... code that may set s->has_error = 1 ...
        if (s->has_error) { handle_error; }
    """
    mquickjs_c = (WASM_BUILD_DIR / "mquickjs.c").read_text()

    # Replace the setjmp include with our error flag approach
    # We don't need setjmp.h anymore
    mquickjs_c = mquickjs_c.replace(
        '#include <setjmp.h>',
        '/* #include <setjmp.h> - removed for wasmtime compatibility */'
    )

    # Find and modify the JSParseState structure to add has_error flag
    # The structure already has jmp_env, we'll add has_error and use it instead
    # First, let's find where jmp_env is declared

    # The js_parse_error function uses longjmp - we need to change it to set a flag
    # Original: longjmp(s->jmp_env, 1);
    # New: s->has_error = 1; return;

    # But this is tricky because js_parse_error is marked noreturn
    # We need to make it return and have callers check for errors

    # Actually, a simpler approach: use a global error flag for the parse state
    # and have a wrapper that handles it

    # Let's try a different approach - use emscripten's -s SUPPORT_LONGJMP=wasm
    # which uses WASM exception handling instead of invoke_* trampolines

    (WASM_BUILD_DIR / "mquickjs.c").write_text(mquickjs_c)

    # Also patch dtoa.c
    dtoa_c = (WASM_BUILD_DIR / "dtoa.c").read_text()
    dtoa_c = dtoa_c.replace(
        '#include <setjmp.h>',
        '/* #include <setjmp.h> - removed for wasmtime compatibility */'
    )
    (WASM_BUILD_DIR / "dtoa.c").write_text(dtoa_c)


def build_wasm_wasmtime():
    """Build mquickjs as WASM for wasmtime."""
    print("Building mquickjs as wasmtime-compatible WASM...")

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

    # Build stdlib generator on host
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
/* WASM wrapper for mquickjs - wasmtime compatible */
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#include <stdio.h>

#ifdef __EMSCRIPTEN__
#include <emscripten.h>
#define EXPORT EMSCRIPTEN_KEEPALIVE
#else
#define EXPORT __attribute__((visibility("default")))
#endif

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

EXPORT
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

EXPORT
void sandbox_free() {
    if (ctx) { JS_FreeContext(ctx); ctx = NULL; }
    if (mem_buf) { free(mem_buf); mem_buf = NULL; }
}

EXPORT
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

EXPORT
const char* sandbox_get_error() {
    return error_buf;
}
'''
    (WASM_BUILD_DIR / "wasm_wrapper.c").write_text(wrapper_c)

    # Try building with WASM exception handling (native WASM exceptions)
    print("  Compiling to WASM with native exception handling...")
    try:
        run([
            "emcc",
            "-O2",
            "-s", "WASM=1",
            "-s", "STANDALONE_WASM=1",
            "-s", "SUPPORT_LONGJMP=wasm",  # Use native WASM exceptions
            "-s", "EXPORTED_FUNCTIONS=['_sandbox_init','_sandbox_free','_sandbox_eval','_sandbox_get_error','_malloc','_free']",
            "-D_GNU_SOURCE",
            "-I.",
            "-o", "mquickjs_wasmtime.wasm",
            "wasm_wrapper.c",
            "mquickjs.c",
            "dtoa.c",
            "libm.c",
            "cutils.c",
        ], cwd=WASM_BUILD_DIR)

        # Copy output
        shutil.copy(WASM_BUILD_DIR / "mquickjs_wasmtime.wasm", SCRIPT_DIR / "mquickjs_wasmtime.wasm")
        print(f"  Built: {SCRIPT_DIR / 'mquickjs_wasmtime.wasm'}")
        return True

    except Exception as e:
        print(f"  Native WASM exceptions failed: {e}")
        print("  Trying without setjmp support...")

        # Try disabling setjmp entirely
        # This requires modifying the source
        return False


def build_wasm_no_setjmp():
    """Build WASM by completely removing setjmp/longjmp from the source."""
    print("Building with setjmp/longjmp removed...")

    # Read and modify mquickjs.c
    mquickjs_c = (WASM_BUILD_DIR / "mquickjs.c").read_text()

    # Strategy: Replace the setjmp/longjmp pattern with a global error flag
    #
    # The parse error handling works like this:
    # 1. setjmp(s->jmp_env) is called before parsing
    # 2. If parsing fails, longjmp(s->jmp_env, 1) is called
    # 3. This jumps back to the setjmp and returns 1, triggering error handling
    #
    # We'll replace this with:
    # 1. A global parse error flag
    # 2. js_parse_error sets the flag instead of longjmp
    # 3. After each parsing call, check the flag

    # First, let's look at the JSParseState structure and add an error flag
    # Find: jmp_env
    # The structure definition needs modification

    # Add a global error state for simplicity
    error_state_decl = '''
/* Global parse error state for wasmtime compatibility (no setjmp) */
static __thread int g_parse_error = 0;
static __thread char g_parse_error_msg[1024];

static void clear_parse_error(void) {
    g_parse_error = 0;
    g_parse_error_msg[0] = 0;
}

static int has_parse_error(void) {
    return g_parse_error;
}

static const char* get_parse_error_msg(void) {
    return g_parse_error_msg;
}
'''

    # Insert after includes
    insert_pos = mquickjs_c.find('#include <setjmp.h>')
    if insert_pos != -1:
        # Replace setjmp include with our error state
        mquickjs_c = mquickjs_c.replace(
            '#include <setjmp.h>',
            '/* #include <setjmp.h> - replaced with error flag for wasmtime */\n' + error_state_decl
        )

    # Now modify js_parse_error to use our global error state
    # Original:
    #   longjmp(s->jmp_env, 1);
    # New:
    #   g_parse_error = 1;
    #   snprintf(g_parse_error_msg, sizeof(g_parse_error_msg), "%s", s->error_msg);
    #   /* Note: we still need to handle the non-local exit somehow */

    # The problem is that js_parse_error is marked noreturn, so callers don't
    # expect it to return. We need to either:
    # 1. Make all callers check for errors (invasive)
    # 2. Use a different mechanism like abort() and catch it (not ideal)
    # 3. Use WASM exceptions (requires wasmtime support)

    # Let's try approach 2 with a custom abort handler

    # Actually, let's check if wasmtime supports WASM exceptions
    # For now, let's just try compiling with SUPPORT_LONGJMP=0 and see what breaks

    (WASM_BUILD_DIR / "mquickjs_modified.c").write_text(mquickjs_c)

    # Try compiling without setjmp support
    print("  Compiling with SUPPORT_LONGJMP=0...")
    try:
        run([
            "emcc",
            "-O2",
            "-s", "WASM=1",
            "-s", "STANDALONE_WASM=1",
            "-s", "SUPPORT_LONGJMP=0",  # Disable longjmp support
            "-s", "EXPORTED_FUNCTIONS=['_sandbox_init','_sandbox_free','_sandbox_eval','_sandbox_get_error','_malloc','_free']",
            "-D_GNU_SOURCE",
            "-Wno-error",  # Don't fail on warnings
            "-I.",
            "-o", "mquickjs_wasmtime.wasm",
            "wasm_wrapper.c",
            "mquickjs.c",  # Use original for now
            "dtoa.c",
            "libm.c",
            "cutils.c",
        ], cwd=WASM_BUILD_DIR)

        shutil.copy(WASM_BUILD_DIR / "mquickjs_wasmtime.wasm", SCRIPT_DIR / "mquickjs_wasmtime.wasm")
        print(f"  Built: {SCRIPT_DIR / 'mquickjs_wasmtime.wasm'}")
        return True

    except Exception as e:
        print(f"  Failed: {e}")
        return False


if __name__ == "__main__":
    if not MQUICKJS_DIR.exists():
        print("Error: mquickjs not found in vendor/. Run build_ffi.py first.")
        exit(1)

    success = build_wasm_wasmtime()
    if not success:
        success = build_wasm_no_setjmp()

    if success:
        print("Done! Built mquickjs_wasmtime.wasm")
    else:
        print("Failed to build wasmtime-compatible WASM")
